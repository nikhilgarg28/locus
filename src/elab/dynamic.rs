//! Shared dynamic interfaces, with physical signatures and no logical laws.
use super::{
    calls::Argument,
    env::{Elab, Env, FnInfo, Global, StructInfo},
    exprs::Value,
    types::Written,
};
use crate::{
    ast::*,
    exec::{DynMethod, Promises},
    kernel::{StructId, Type as KType},
    source::Span,
    typed::{Binder, ErasureLayout, Passing},
};
use std::{collections::BTreeMap, rc::Rc};

#[derive(Clone, Debug)]
pub(super) struct Dynamic {
    pub id: StructId,
    pub interface: String,
    pub associated: BTreeMap<String, KType>,
    pub names: Vec<String>,
    pub packs: Vec<(KType, Rc<FnInfo>)>,
}

pub(super) fn borrowed_dyn(ty: &Type) -> Option<(&GenericBound, bool)> {
    match &ty.kind {
        TypeKind::Group(t) => borrowed_dyn(t),
        TypeKind::Ref { mutable, inner, .. } => {
            let mut inner = inner.as_ref();
            while let TypeKind::Group(group) = &inner.kind {
                inner = group;
            }
            match &inner.kind {
                TypeKind::Dyn(bound) => Some((bound, *mutable)),
                _ => None,
            }
        }
        _ => None,
    }
}

impl Env<'_> {
    pub(super) fn initialize_dynamics(&mut self, program: &Program) {
        use crate::project::specs::walk::{self, Walk};
        struct Collect(Vec<GenericBound>);
        impl Walk for Collect {
            fn ty(&mut self, ty: &mut Type) {
                if let TypeKind::Dyn(bound) = &ty.kind {
                    self.0.push((**bound).clone());
                }
                walk::ty(self, ty);
            }
        }
        let mut collect = Collect(vec![]);
        for mut d in program.declarations.clone() {
            if let DeclarationKind::Impl { methods, .. } = &mut d.kind {
                for m in methods {
                    walk::member(&mut collect, m);
                }
            } else {
                walk::member(&mut collect, &mut d);
            }
        }
        for bound in collect.0 {
            let _ = self.dynamic_type(&bound);
        }
    }

    fn dyn_scalar(&mut self, ty: &Type, associated: &BTreeMap<String, KType>) -> Elab<KType> {
        match &ty.kind {
            TypeKind::Group(t) => self.dyn_scalar(t, associated),
            TypeKind::Unit => Ok(KType::Tuple(vec![])),
            TypeKind::Named(n) if n.text == "bool" => Ok(KType::Bool),
            TypeKind::Named(n) if self.machine_type(&n.text).is_some() => Ok(KType::machine(self.machine_type(&n.text).unwrap())),
            TypeKind::Path { path, arguments } if arguments.is_empty() && path.segments.len() == 2 && path.segments[0].text == "Self" => {
                associated.get(&path.segments[1].text).cloned().ok_or_else(|| {
                    let _: Elab<()> = self.fail("L0518", "every associated type of a dyn interface must be fixed explicitly", ty.span);
                })
            }
            TypeKind::Tuple(fields) => {
                let mut types = vec![];
                for f in fields { types.push(self.dyn_scalar(&f.ty, associated)?); }
                Ok(KType::Tuple(types))
            }
            _ => self.fail("L0518", "this dyn tier requires physical scalar/tuple method signatures and associated types; proofs, logical values, references and Self values are deferred", ty.span),
        }
    }

    pub(super) fn dynamic_type(&mut self, bound: &GenericBound) -> Elab<KType> {
        let interface = bound.path.text();
        let Some(definition) = self
            .module_access
            .as_ref()
            .and_then(|a| a.trait_registry.definitions.get(&interface))
            .cloned()
        else {
            return self.fail(
                "L0518",
                format!("`{interface}` is not an available trait"),
                bound.path.span,
            );
        };
        let DeclarationKind::Trait {
            native, members, ..
        } = &definition.kind
        else {
            unreachable!()
        };
        if native.is_some() {
            return self.fail("L0518", "objects of imported Rust traits are deferred; concrete imported trait calls remain available", bound.path.span);
        }
        let mut associated = BTreeMap::new();
        for (name, ty) in &bound.associated {
            let resolved = self.dyn_scalar(ty, &BTreeMap::new())?;
            if associated.insert(name.text.clone(), resolved).is_some() {
                return self.fail(
                    "L0518",
                    format!("associated type `{}` is fixed twice", name.text),
                    name.span,
                );
            }
        }
        if let Some(found) = self
            .dynamics
            .iter()
            .find(|d| d.interface == interface && d.associated == associated)
        {
            return Ok(KType::Struct(found.id));
        }
        let mut slots = vec![];
        let mut declared = std::collections::BTreeSet::new();
        for member in members {
            if !member.constraints.is_empty()
                || member.attributes.iter().any(|a| a.kind.is_promise())
            {
                return self.fail(
                    "L0518",
                    "dyn methods with bounds or effect promises are deferred",
                    member.span,
                );
            }
            match &member.kind {
                DeclarationKind::AssociatedType { name, logical: false, .. } => {
                    declared.insert(name.text.clone());
                    if !associated.contains_key(&name.text) { return self.fail("L0518", format!("write `dyn {interface}<{} = Type>` to fix this associated type", name.text), bound.path.span); }
                }
                DeclarationKind::Function { name, logical: false, generics, self_param: Some(receiver), parameters, result, .. }
                    if generics.is_empty() && receiver.kind == SelfKind::Ref => {
                    let mut params = vec![];
                    for p in parameters { params.push(self.dyn_scalar(&p.ty, &associated)?); }
                    let result = self.dyn_scalar(result, &associated)?;
                    slots.push(DynMethod {name: name.text.clone(), params, result});
                }
                _ => return self.fail("L0518", "this trait cannot be used as dyn: only ordinary &self methods and fixed physical associated types are supported; constants, logical methods, owned/mutable receivers and constructors are deferred", member.span),
            }
        }
        if let Some(name) = associated.keys().find(|n| !declared.contains(*n)) {
            return self.fail(
                "L0518",
                format!("trait `{interface}` has no associated type `{name}`"),
                bound.path.span,
            );
        }
        let serial = self.dynamics.len();
        let name = format!("LocusMDyn{serial}");
        let id = match self
            .session
            .declare_dynamic_interface(name.clone(), slots.clone())
        {
            Ok(id) => id,
            Err(error) => return self.internal(error, bound.path.span),
        };
        self.types.insert(
            name.clone(),
            Global::Struct(Rc::new(StructInfo {
                shape: VariantShape::Struct,
                captures: vec![],
                origin: None,
                id,
                name: name.clone(),
                fields: vec![],
                derives: vec![],
                visibility: None,
                field_visibility: vec![],
            })),
        );
        for (slot, method) in slots.iter().enumerate() {
            let helper = format!("__locus_dyn_{serial}_{}", method.name);
            let mut params = vec![Binder::new("value", KType::Struct(id))];
            params.extend(
                method
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, t)| Binder::new(&format!("arg{i}"), t.clone())),
            );
            let reference = match self.session.declare_dynamic_method(
                helper.clone(),
                id,
                slot,
                &params,
                &method.result,
            ) {
                Ok(f) => f,
                Err(e) => return self.internal(e, bound.path.span),
            };
            self.values.insert(
                format!("{name}::{}", method.name),
                Global::Fn(Rc::new(FnInfo {
                    origin: None,
                    logical: false,
                    result_logical: false,
                    reference,
                    name: helper,
                    params,
                    result: method.result.clone(),
                    constant: false,
                    promises: Promises::default(),
                    passing: vec![Passing::Value],
                    visibility: None,
                    receiver: true,
                })),
            );
        }
        self.dynamics.push(Dynamic {
            id,
            interface,
            associated,
            names: slots.into_iter().map(|m| m.name).collect(),
            packs: vec![],
        });
        Ok(KType::Struct(id))
    }

    pub(super) fn dynamic_reference(&mut self, ty: &Type) -> Elab<Written> {
        let (bound, mutable) = borrowed_dyn(ty).expect("dyn reference");
        if mutable {
            return self.fail(
                "L0518",
                "mutable trait objects are deferred; use &dyn Trait",
                ty.span,
            );
        }
        let ty = self.dynamic_type(bound)?;
        Ok(Written { ty, ghost: false })
    }

    pub(super) fn dynamic_coercion(
        &mut self,
        value: Value,
        expected: &KType,
        span: Span,
    ) -> Elab<Value> {
        let Some(index) = self
            .dynamics
            .iter()
            .position(|d| *expected == KType::Struct(d.id))
        else {
            unreachable!()
        };
        if !matches!(
            self.session.expression_layout(&value.expr),
            ErasureLayout::Shared { .. }
        ) {
            return self.fail(
                "L0518",
                "a dyn coercion requires a shared borrow; write `&value`",
                span,
            );
        }
        let d = self.dynamics[index].clone();
        let info = if let Some((_, info)) = d.packs.iter().find(|(t, _)| *t == value.ty) {
            info.clone()
        } else {
            let owner = self.type_name(&value.ty).unwrap_or_default();
            let implementation = self
                .module_access
                .as_ref()
                .and_then(|a| {
                    a.trait_registry
                        .implementations
                        .iter()
                        .find(|i| i.interface == d.interface && i.owner == owner)
                })
                .cloned();
            let Some(implementation) = implementation else {
                return self.fail(
                    "L0518",
                    format!(
                        "`{owner}` has no checked implementation of `{}` for this dyn coercion",
                        d.interface
                    ),
                    span,
                );
            };
            if !implementation.generics.is_empty() || !implementation.constraints.is_empty() {
                return self.fail(
                    "L0518",
                    "dyn coercions from generic implementation families are deferred",
                    span,
                );
            }
            for (name, expected) in &d.associated {
                let Some(binding) = implementation.associated.get(name) else {
                    return self.fail(
                        "L0518",
                        format!("implementation is missing associated type `{name}`"),
                        span,
                    );
                };
                let actual = self.dyn_scalar(binding, &BTreeMap::new())?;
                if actual != *expected {
                    return self.fail(
                        "L0518",
                        format!("associated type `{name}` does not match the dyn binding"),
                        span,
                    );
                }
            }
            let mut methods = vec![];
            for method in &d.names {
                let selected = format!(
                    "{owner}::{}",
                    crate::project::traits::lowered(&d.interface, method)
                );
                let Some(Global::Fn(f)) = self.values.get(&selected) else {
                    return self.fail(
                        "L0518",
                        format!("dynamic implementation `{selected}` has not been checked"),
                        span,
                    );
                };
                let crate::typed::FnRef::Exec(id) = f.reference else {
                    return self.fail(
                        "L0518",
                        "a dynamic implementation must have runtime code",
                        span,
                    );
                };
                methods.push(id);
            }
            let name = format!("__locus_dyn_pack_{index}_{}", d.packs.len());
            let input = Binder::new("value", value.ty.clone());
            let reference = match self.session.declare_dynamic_pack(
                name.clone(),
                d.id,
                &value.ty,
                methods,
                &input,
            ) {
                Ok(r) => r,
                Err(e) => return self.internal(e, span),
            };
            let info = Rc::new(FnInfo {
                origin: None,
                logical: false,
                result_logical: false,
                reference,
                name,
                params: vec![input],
                result: expected.clone(),
                constant: false,
                promises: Promises::default(),
                passing: vec![Passing::Value],
                visibility: None,
                receiver: false,
            });
            self.dynamics[index]
                .packs
                .push((value.ty.clone(), info.clone()));
            info
        };
        self.call_fn_with(&info, &[Argument::Value(Box::new(value), span)], span)
    }
}
