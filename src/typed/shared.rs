//! Persistent shared-reference permissions, checked before lowering erases
//! references to immutable observations. A physical reference keeps its
//! referent's storage origin, not the identity of its current pointer holder.
//! Logical observations are uses too: Rust cannot check an erased observation.
//!
//! The current tier excludes interior mutation, stored mutable references and
//! references hidden in owned buffers. Writes in alternative branches are
//! conservatively joined; a use after any overlapping write is rejected.
use super::{Block, ErasureLayout, ErasureLayouts, Expr, FnItem, FnRef, LowerError, Pattern, Stmt};
use crate::{
    erased::{self, EType, Module},
    kernel::{Term, Type, VarId},
};
use std::collections::{HashMap, HashSet};

type Result<T> = std::result::Result<T, LowerError>;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Origin {
    root: VarId,
    path: Vec<usize>,
    since: usize,
    lifetime: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Leaf {
    path: Vec<usize>,
    origins: Vec<Origin>,
}
type Refs = Vec<Leaf>;
const REFERENT: usize = usize::MAX;
#[derive(Clone)]
struct Binding {
    root: VarId,
    refs: Refs,
    ty: EType,
    borrowed: bool,
}
struct Check<'a> {
    layouts: &'a ErasureLayouts,
    module: &'a Module,
    defs: &'a crate::kernel::Definitions,
    bindings: HashMap<VarId, Binding>,
    current: HashMap<VarId, VarId>,
    names: HashMap<VarId, String>,
    live: HashSet<VarId>,
    writes: Vec<(VarId, Vec<usize>)>,
    result: EType,
    exits: Vec<(VarId, EType)>,
    breaks: Vec<Refs>,
}
fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(LowerError::ReferencePermission(message.into()))
}
fn overlap(a: &[usize], b: &[usize]) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn prefixed(mut refs: Refs, index: usize) -> Refs {
    for leaf in &mut refs {
        leaf.path.insert(0, index);
    }
    refs
}
fn projected(refs: Refs, index: usize) -> Refs {
    refs.into_iter()
        .filter_map(|mut leaf| {
            if leaf.path.first() == Some(&index) {
                leaf.path.remove(0);
                Some(leaf)
            } else {
                None
            }
        })
        .collect()
}
fn merged(mut a: Refs, b: Refs) -> Refs {
    for leaf in b {
        if let Some(old) = a.iter_mut().find(|x| x.path == leaf.path) {
            for origin in leaf.origins {
                if !old.origins.contains(&origin) {
                    old.origins.push(origin);
                }
            }
        } else {
            a.push(leaf);
        }
    }
    a
}

pub(super) fn check(
    item: &FnItem,
    result_layout: &ErasureLayout,
    layouts: &ErasureLayouts,
    module: &Module,
    defs: &crate::kernel::Definitions,
) -> Result<()> {
    let result = erased::type_with_layout(&item.result, result_layout, &erased::erase_type);
    let mut checker = Check {
        layouts,
        module,
        defs,
        bindings: HashMap::new(),
        current: HashMap::new(),
        names: HashMap::new(),
        live: HashSet::new(),
        writes: Vec::new(),
        result,
        exits: Vec::new(),
        breaks: Vec::new(),
    };
    for (i, param) in item.params.iter().enumerate() {
        let layout = layouts.binding(param.id);
        let ty = erased::type_with_layout(&param.ty, &layout, &erased::erase_type);
        let borrowed = item.passing_of(i).is_reference();
        let (ty, lifetime) = match ty {
            EType::Ref(l, t) if borrowed => (*t, l),
            t => (t, None),
        };
        let refs = checker
            .paths(&ty)?
            .into_iter()
            .map(|(path, lifetime)| {
                let root = VarId::fresh();
                checker.live.insert(root);
                checker
                    .names
                    .insert(root, format!("referent inside {}", param.name));
                Leaf {
                    path: path.clone(),
                    origins: vec![Origin {
                        root,
                        path: Vec::new(),
                        since: 0,
                        lifetime,
                    }],
                }
            })
            .collect();
        if item.passing_of(i) == super::Passing::RefMut {
            checker.exits.push((param.id, ty.clone()));
        }
        checker.current.insert(param.id, param.id);
        checker.bindings.insert(
            param.id,
            Binding {
                root: param.id,
                refs,
                ty,
                borrowed,
            },
        );
        checker.live.insert(param.id);
        checker.names.insert(param.id, param.name.clone());
        if borrowed {
            checker.names.insert(
                param.id,
                format!(
                    "{} (input {})",
                    param.name,
                    lifetime.as_deref().unwrap_or("without a named lifetime")
                ),
            );
        }
    }
    let refs = checker.block(&item.body)?;
    if !super::block_leaves(&item.body) {
        checker.returned(&refs)?;
    }
    for (i, param) in item.params.iter().enumerate() {
        if item.passing_of(i) == super::Passing::RefMut
            && let Some(current) = checker
                .current
                .get(&param.id)
                .and_then(|id| checker.bindings.get(id))
        {
            checker.within(&current.refs, &checker.bindings[&param.id].ty)?;
        }
    }
    Ok(())
}
impl Check<'_> {
    fn ty(&self, ty: &Type, layout: &ErasureLayout) -> EType {
        erased::type_with_layout(ty, layout, &erased::erase_type)
    }
    fn binding_ty(&self, id: VarId, ty: &Type) -> EType {
        self.ty(ty, &self.layouts.binding(id))
    }
    fn paths(&self, ty: &EType) -> Result<Vec<(Vec<usize>, Option<String>)>> {
        fn walk(
            c: &Check<'_>,
            ty: &EType,
            path: &mut Vec<usize>,
            seen: &mut HashSet<String>,
            out: &mut Vec<(Vec<usize>, Option<String>)>,
        ) -> Result<()> {
            match ty {
                EType::Ref(l, inner) => {
                    out.push((path.clone(), l.clone()));
                    path.push(REFERENT);
                    walk(c, inner, path, seen, out)?;
                    path.pop();
                }
                EType::Tuple(fields) => {
                    for (i, f) in fields.iter().enumerate() {
                        path.push(i);
                        walk(c, f, path, seen, out)?;
                        path.pop();
                    }
                }
                EType::Struct(id) | EType::StructApplied(id, _) => {
                    let Some(item) = c.module.structs.iter().find(|s| s.id == *id) else {
                        return Ok(());
                    };
                    if !seen.insert(item.name.clone()) {
                        return Ok(());
                    }
                    let fields =
                        applied_fields(ty, item.fields.iter().map(|(_, t)| t.clone()).collect());
                    for (i, f) in fields.iter().enumerate() {
                        path.push(i);
                        walk(c, f, path, seen, out)?;
                        path.pop();
                    }
                    seen.remove(&item.name);
                }
                EType::Enum(id) | EType::EnumApplied(id, _) => {
                    let Some(item) = c.module.enums.iter().find(|e| e.id == *id) else {
                        return Ok(());
                    };
                    if !seen.insert(item.name.clone()) {
                        return Ok(());
                    }
                    let mut declared = Vec::new();
                    for v in &item.variants {
                        for f in &v.payload {
                            lifetimes(f, &mut declared)
                        }
                    }
                    for (variant, v) in item.variants.iter().enumerate() {
                        path.push(variant);
                        for (i, mut f) in v.payload.clone().into_iter().enumerate() {
                            if let EType::EnumApplied(_, args) = ty {
                                rename(&mut f, &declared, args)
                            }
                            path.push(i);
                            walk(c, &f, path, seen, out)?;
                            path.pop();
                        }
                        path.pop();
                    }
                    seen.remove(&item.name);
                }
                EType::Boxed(t) | EType::Buffer(t) | EType::Array(t, _) | EType::Slice(t) => {
                    let mut inside = Vec::new();
                    walk(c, t, &mut Vec::new(), seen, &mut inside)?;
                    if !inside.is_empty() {
                        return fail(
                            "references inside owned buffers are not supported in this reference tier",
                        );
                    }
                }
                _ => {}
            }
            Ok(())
        }
        let mut out = Vec::new();
        walk(self, ty, &mut Vec::new(), &mut HashSet::new(), &mut out)?;
        Ok(out)
    }
    fn required_paths(&self, ty: &EType) -> Result<Vec<Vec<usize>>> {
        match ty {
            EType::Enum(_) | EType::EnumApplied(..) => Ok(Vec::new()),
            EType::Ref(_, inner) => {
                let mut paths = vec![Vec::new()];
                paths.extend(self.required_paths(inner)?.into_iter().map(|mut p| {
                    p.insert(0, REFERENT);
                    p
                }));
                Ok(paths)
            }
            EType::Tuple(fields) => {
                let mut paths = Vec::new();
                for (i, f) in fields.iter().enumerate() {
                    paths.extend(self.required_paths(f)?.into_iter().map(|mut p| {
                        p.insert(0, i);
                        p
                    }));
                }
                Ok(paths)
            }
            EType::Struct(id) | EType::StructApplied(id, _) => {
                let fields = self
                    .module
                    .structs
                    .iter()
                    .find(|s| s.id == *id)
                    .map(|s| s.fields.iter().map(|(_, t)| t.clone()).collect())
                    .unwrap_or_default();
                self.required_paths(&EType::Tuple(applied_fields(ty, fields)))
            }
            _ => Ok(Vec::new()),
        }
    }
    fn valid(&self, refs: &Refs) -> Result<()> {
        for leaf in refs {
            for origin in &leaf.origins {
                if !self.live.contains(&origin.root) {
                    return fail("shared reference outlives the storage it borrows");
                }
                if self
                    .writes
                    .iter()
                    .skip(origin.since)
                    .any(|(root, path)| *root == origin.root && overlap(path, &origin.path))
                {
                    return fail(format!(
                        "shared reference to `{}` is used after an overlapping write or move; logical observations also require an available shared borrow",
                        self.names
                            .get(&origin.root)
                            .map(String::as_str)
                            .unwrap_or("storage")
                    ));
                }
            }
        }
        Ok(())
    }
    fn returned(&self, refs: &Refs) -> Result<()> {
        self.valid(refs)?;
        self.within(refs, &self.result)?;
        for (root, expected) in &self.exits {
            if let Some(binding) = self.current.get(root).and_then(|id| self.bindings.get(id)) {
                self.within(&binding.refs, expected)?;
            }
        }
        Ok(())
    }
    fn within(&self, refs: &Refs, result: &EType) -> Result<()> {
        self.valid(refs)?;
        let expected = self.paths(result)?;
        for leaf in refs {
            if leaf.origins.is_empty() {
                return fail("a reference has no checked storage origin");
            };
            let Some((_, Some(lifetime))) = expected.iter().find(|(p, _)| *p == leaf.path) else {
                return fail(
                    "a returned shared reference needs an explicit input lifetime in its result type",
                );
            };
            for origin in &leaf.origins {
                if origin.lifetime.as_ref() != Some(lifetime) {
                    return fail(format!(
                        "returned reference must originate from an input with lifetime {lifetime}; a local or a different input lifetime cannot escape"
                    ));
                }
            }
        }
        if self
            .required_paths(result)?
            .iter()
            .any(|path| !refs.iter().any(|r| r.path == *path))
        {
            return fail("returned reference has no checked storage origin");
        }
        Ok(())
    }
    fn origin_of(&mut self, value: &Expr) -> Result<Vec<Origin>> {
        match value {
            Expr::BoxDeref { value, .. } => self.origin_of(value),
            Expr::Var { id, .. } => {
                let Some(binding) = self.bindings.get(id) else {
                    return fail("shared borrow has no live storage root");
                };
                let lifetime = if binding.borrowed {
                    match self.layouts.binding(*id) {
                        ErasureLayout::Borrowed { lifetime, .. } => lifetime,
                        _ => None,
                    }
                } else {
                    None
                };
                Ok(vec![Origin {
                    root: binding.root,
                    path: Vec::new(),
                    since: self.writes.len(),
                    lifetime,
                }])
            }
            Expr::Field { target, index, .. } => {
                let mut origins = if matches!(
                    self.layouts.expression(target, self.defs),
                    ErasureLayout::Shared { .. }
                ) {
                    self.expr(target, false)?
                        .into_iter()
                        .filter(|leaf| leaf.path.is_empty())
                        .flat_map(|leaf| leaf.origins)
                        .collect()
                } else {
                    self.origin_of(target)?
                };
                for o in &mut origins {
                    o.path.push(*index)
                }
                Ok(origins)
            }
            Expr::Deref(inner) => {
                let refs = self.expr(inner, false)?;
                self.valid(&refs)?;
                Ok(refs
                    .into_iter()
                    .filter(|l| l.path.is_empty())
                    .flat_map(|l| l.origins)
                    .collect())
            }
            _ => fail("a shared borrow must refer to existing storage, not a temporary value"),
        }
    }
    fn bind(&mut self, pattern: &Pattern, refs: Refs) -> Result<()> {
        match pattern {
            Pattern::Bind { binder, .. } => {
                let ty = self.binding_ty(binder.id, &binder.ty);
                self.paths(&ty)?;
                self.current.insert(binder.id, binder.id);
                self.live.insert(binder.id);
                self.names.insert(binder.id, binder.name.clone());
                self.bindings.insert(
                    binder.id,
                    Binding {
                        root: binder.id,
                        refs,
                        ty,
                        borrowed: false,
                    },
                );
            }
            Pattern::Tuple(fields) => {
                for (i, f) in fields.iter().enumerate() {
                    self.bind(f, projected(refs.clone(), i))?
                }
            }
            Pattern::Wildcard => {}
        }
        Ok(())
    }
    fn block(&mut self, block: &Block) -> Result<Refs> {
        let live = self.live.clone();
        for stmt in &block.stmts {
            match stmt {
                Stmt::Let { pattern, value } => {
                    let refs = self.expr(value, true)?;
                    self.bind(pattern, refs)?;
                }
                Stmt::Assign {
                    place,
                    value,
                    version,
                    ..
                } => {
                    let refs = self.expr(value, true)?;
                    self.writes
                        .push((place.binding, place.path.iter().map(|s| s.index).collect()));
                    let previous = self
                        .current
                        .get(&place.binding)
                        .and_then(|id| self.bindings.get(id))
                        .cloned();
                    let mut all = previous
                        .as_ref()
                        .map(|b| b.refs.clone())
                        .unwrap_or_default();
                    let path: Vec<_> = place.path.iter().map(|s| s.index).collect();
                    all.retain(|l| !l.path.starts_with(&path));
                    for mut leaf in refs {
                        let mut p = path.clone();
                        p.extend(leaf.path);
                        leaf.path = p;
                        all.push(leaf)
                    }
                    self.current.insert(place.binding, version.id);
                    self.bindings.insert(
                        version.id,
                        Binding {
                            root: place.binding,
                            refs: all,
                            ty: self.binding_ty(version.id, &version.ty),
                            borrowed: previous.is_some_and(|b| b.borrowed),
                        },
                    );
                }
                Stmt::Expr(expr) => {
                    self.expr(expr, true)?;
                }
            }
        }
        let refs = match &block.tail {
            Some(t) => self.expr(t, true)?,
            None => Vec::new(),
        };
        if refs
            .iter()
            .any(|l| l.origins.iter().any(|o| !live.contains(&o.root)))
        {
            return fail("shared reference escapes the block that owns its referent");
        }
        self.live = live;
        Ok(refs)
    }
    fn copy(&self, ty: &EType) -> bool {
        match ty {
            EType::Ref(..)
            | EType::Bool
            | EType::Int(_)
            | EType::Ghost
            | EType::Proved
            | EType::Fn(..) => true,
            EType::Tuple(fields) => fields.iter().all(|t| self.copy(t)),
            EType::Array(t, _) => self.copy(t),
            EType::Struct(id) | EType::StructApplied(id, _) => self
                .module
                .structs
                .iter()
                .find(|s| s.id == *id)
                .is_some_and(|s| s.derives.contains(&super::Derive::Copy)),
            EType::Enum(id) | EType::EnumApplied(id, _) => self
                .module
                .enums
                .iter()
                .find(|s| s.id == *id)
                .is_some_and(|s| s.derives.contains(&super::Derive::Copy)),
            _ => false,
        }
    }
    fn term(&self, term: &Term) -> Result<()> {
        for (id, b) in &self.bindings {
            if !b.refs.is_empty() && term.find(&|t| matches!(t,Term::Free(x) if x==id)).is_some() {
                self.valid(&b.refs)?
            }
        }
        Ok(())
    }
    fn expr(&mut self, expr: &Expr, consume: bool) -> Result<Refs> {
        let refs = self.expr_form(expr, consume)?;
        if let Some(ty) = super::layout::expression_type(expr) {
            let physical = self.ty(&ty, &self.layouts.expression(expr, self.defs));
            if matches!(physical, EType::Ref(..))
                && self
                    .required_paths(&physical)?
                    .iter()
                    .any(|p| !refs.iter().any(|l| l.path == *p))
            {
                return fail("reference expression has no checked storage origin");
            }
        }
        Ok(refs)
    }
    fn expr_form(&mut self, expr: &Expr, consume: bool) -> Result<Refs> {
        match expr {
            Expr::BoxNew {
                value,
                logical_payload,
                ..
            } => {
                if *logical_payload != self.layouts.expression(value, self.defs).is_logical() {
                    return fail(
                        "Box payload erasure flag disagrees with its checked expression layout",
                    );
                };
                let refs = self.expr(value, true)?;
                if !refs.is_empty() {
                    return fail("references inside Box are not supported in this tier");
                }
                Ok(Vec::new())
            }
            Expr::BoxDeref { value, ty } => {
                self.expr(value, false)?;
                if consume
                    && !self.copy(&erased::erase_type(ty))
                    && let Ok(origins) = self.origin_of(value)
                {
                    for origin in origins {
                        self.writes.push((origin.root, origin.path));
                    }
                }
                Ok(Vec::new())
            }
            Expr::Shared { value, .. } => {
                let inside = self.expr(value, false)?;
                let mut refs = vec![Leaf {
                    path: Vec::new(),
                    origins: self.origin_of(value)?,
                }];
                refs.extend(prefixed(inside, REFERENT));
                Ok(refs)
            }
            Expr::Deref(value) => {
                if !matches!(
                    self.layouts.expression(value, self.defs),
                    ErasureLayout::Shared { .. }
                ) {
                    return fail("dereference requires a checked shared reference");
                };
                let refs = self.expr(value, false)?;
                self.valid(&refs)?;
                if let Some(ty) = super::layout::expression_type(expr) {
                    let physical = self.ty(&ty, &self.layouts.expression(expr, self.defs));
                    if consume && !self.copy(&physical) {
                        return fail("cannot move a non-Copy value through a shared reference");
                    }
                }
                Ok(projected(refs, REFERENT))
            }
            Expr::Var { id, .. } => {
                let Some(b) = self.bindings.get(id).cloned() else {
                    return Ok(Vec::new());
                };
                self.valid(&b.refs)?;
                if consume && !b.borrowed && !self.copy(&b.ty) {
                    self.writes.push((b.root, Vec::new()));
                }
                Ok(b.refs)
            }
            Expr::Tuple { fields, .. } => {
                let mut refs = Vec::new();
                for (i, f) in fields.iter().enumerate() {
                    refs.extend(prefixed(self.expr(f, consume)?, i));
                }
                Ok(refs)
            }
            Expr::Struct { fields, .. } => {
                let mut refs = Vec::new();
                for (i, (_, f)) in fields.iter().enumerate() {
                    refs.extend(prefixed(self.expr(f, consume)?, i));
                }
                Ok(refs)
            }
            Expr::Variant { payload, index, .. } => {
                let mut refs = Vec::new();
                for (i, f) in payload.iter().enumerate() {
                    refs.extend(prefixed(self.expr(f, consume)?, i));
                }
                Ok(prefixed(refs, *index))
            }
            Expr::Field {
                target, index, ty, ..
            } => {
                let mut refs = self.expr(target, false)?;
                if matches!(
                    self.layouts.expression(target, self.defs),
                    ErasureLayout::Shared { .. }
                ) {
                    refs = projected(refs, REFERENT);
                }
                let layout = self.layouts.expression(expr, self.defs);
                let physical = self.ty(ty, &layout);
                if consume && !self.copy(&physical) {
                    if matches!(
                        self.layouts.expression(target, self.defs),
                        ErasureLayout::Shared { .. }
                    ) {
                        return fail("cannot move a non-Copy field through a shared reference");
                    }
                    if let Ok(origins) = self.origin_of(target) {
                        for mut origin in origins {
                            origin.path.push(*index);
                            self.writes.push((origin.root, origin.path));
                        }
                    }
                }
                Ok(projected(refs, *index))
            }
            Expr::Block(block) => self.block(block),
            Expr::Lend { value, .. } => self.expr(value, false),
            Expr::Ghost(value) => {
                self.expr(value, false)?;
                Ok(Vec::new())
            }
            Expr::Prop(term) => {
                self.term(term)?;
                Ok(Vec::new())
            }
            Expr::Proof(proof) | Expr::Absurd { proof, .. } => {
                let wrapped = Term::Proof(Box::new(proof.clone()));
                for (id, b) in &self.bindings {
                    if !b.refs.is_empty()
                        && wrapped.replace_var(*id, &Term::var(VarId::fresh())) != wrapped
                    {
                        self.valid(&b.refs)?
                    }
                }
                Ok(Vec::new())
            }
            Expr::CallFn {
                id,
                arguments,
                lends,
                ..
            } => {
                let signature = self
                    .module
                    .fns
                    .iter()
                    .find(|f| f.reference == FnRef::Exec(*id));
                let needed = signature
                    .map(|f| self.paths(&f.result))
                    .transpose()?
                    .unwrap_or_default();
                let mut inputs: Vec<(Option<String>, Vec<Origin>)> = Vec::new();
                for (i, arg) in arguments.iter().enumerate() {
                    let refs = self.expr(arg, true)?;
                    if let Some(function) = signature {
                        if !needed.is_empty()
                            && function.passing_of(i).is_reference()
                            && let Expr::Lend { value, .. } = arg
                        {
                            let deref_coercion = matches!(
                                self.layouts.expression(value, self.defs),
                                ErasureLayout::Shared { .. }
                            ) && !matches!(&function.params[i].2,EType::Ref(_,inner) if matches!(&**inner,EType::Ref(..)));
                            let origins = if deref_coercion {
                                self.expr(value, false)?
                                    .into_iter()
                                    .filter(|l| l.path.is_empty())
                                    .flat_map(|l| l.origins)
                                    .collect()
                            } else {
                                self.origin_of(value)?
                            };
                            let life = match &function.params[i].2 {
                                EType::Ref(l, _) => l.clone(),
                                _ => None,
                            };
                            inputs.push((life, origins));
                        }
                        let param_ty = function.params.get(i).map(|p| match &p.2 {
                            EType::Ref(_, inner) if function.passing_of(i).is_reference() => {
                                &**inner
                            }
                            other => other,
                        });
                        if let Some(ty) = param_ty {
                            for (path, life) in self.paths(ty)? {
                                if let Some(leaf) = refs.iter().find(|l| l.path == path) {
                                    inputs.push((life, leaf.origins.clone()));
                                }
                            }
                        }
                    }
                }
                for lend in lends {
                    if let Some(Expr::Lend { place, .. }) = arguments.get(lend.argument) {
                        let path: Vec<_> = place.path.iter().map(|s| s.index).collect();
                        self.writes.push((place.binding, path.clone()));
                        if let Some(mut binding) = self
                            .current
                            .get(&place.binding)
                            .and_then(|id| self.bindings.get(id))
                            .cloned()
                        {
                            if let Some(function) = signature {
                                let parameter = &function.params[lend.argument].2;
                                let target = match parameter {
                                    EType::Ref(_, inner) => &**inner,
                                    other => other,
                                };
                                let exits = self.paths(target)?;
                                binding.refs.retain(|leaf| !leaf.path.starts_with(&path));
                                for (mut field, life) in exits {
                                    let origins = inputs
                                        .iter()
                                        .filter(|(l, _)| life.is_some() && *l == life)
                                        .flat_map(|(_, o)| o.clone())
                                        .collect();
                                    let mut full = path.clone();
                                    full.append(&mut field);
                                    binding.refs.push(Leaf {
                                        path: full,
                                        origins,
                                    });
                                }
                            }
                            binding.ty = self.binding_ty(lend.version.id, &lend.version.ty);
                            self.current.insert(place.binding, lend.version.id);
                            self.bindings.insert(lend.version.id, binding);
                        }
                    }
                }
                let mut refs = Vec::new();
                if let Some(function) = signature {
                    for (path, life) in self.paths(&function.result)? {
                        let origins = inputs
                            .iter()
                            .filter(|(l, _)| life.is_some() && *l == life)
                            .flat_map(|(_, o)| o.clone())
                            .collect::<Vec<_>>();
                        if origins.is_empty() {
                            return fail(
                                "reference returned by a call has no matching input lifetime",
                            );
                        };
                        refs.push(Leaf { path, origins });
                    }
                }
                self.valid(&refs)?;
                Ok(refs)
            }
            Expr::CallMath { arguments, .. } | Expr::LogicalApply { arguments, .. } => {
                for a in arguments {
                    self.expr(a, false)?;
                }
                Ok(Vec::new())
            }
            Expr::If {
                condition,
                then_block,
                else_block,
                joined,
                ..
            } => {
                self.expr(condition, true)?;
                let a = self.block(then_block)?;
                let b = self.block(else_block)?;
                self.joins(joined.as_ref().map(|j| j.joins.as_slice()).unwrap_or(&[]));
                Ok(merged(a, b))
            }
            Expr::Match {
                scrutinee,
                arms,
                joined,
                ..
            } => {
                let input = self.expr(scrutinee, true)?;
                let mut refs = Vec::new();
                for (variant, arm) in arms.iter().enumerate() {
                    let arm_entry = self.live.clone();
                    for (i, binder) in arm.payload.iter().enumerate() {
                        self.bindings.insert(
                            binder.id,
                            Binding {
                                root: binder.id,
                                refs: projected(projected(input.clone(), variant), i),
                                ty: self.binding_ty(binder.id, &binder.ty),
                                borrowed: false,
                            },
                        );
                        self.live.insert(binder.id);
                    }
                    let result = self.block(&arm.body)?;
                    // A payload binding owns arm-local storage even though it
                    // is installed before checking the arm's block. Returning
                    // a reference contained in it is fine; borrowing the
                    // payload binding itself cannot outlive the arm.
                    if result.iter().any(|leaf| {
                        leaf.origins
                            .iter()
                            .any(|origin| !arm_entry.contains(&origin.root))
                    }) {
                        return fail(
                            "shared reference escapes the match arm that owns its referent",
                        );
                    }
                    self.live = arm_entry;
                    refs = merged(refs, result);
                }
                self.joins(joined.as_ref().map(|j| j.joins.as_slice()).unwrap_or(&[]));
                Ok(refs)
            }
            Expr::Loop {
                body,
                carried,
                state,
                ..
            }
            | Expr::While {
                body,
                carried,
                state,
                ..
            }
            | Expr::For {
                body,
                carried,
                state,
                ..
            } => {
                let outer = self.live.clone();
                if let Expr::For { index, lo, hi, .. } = expr {
                    self.expr(lo, true)?;
                    self.expr(hi, true)?;
                    self.live.insert(index.id);
                    self.bindings.insert(
                        index.id,
                        Binding {
                            root: index.id,
                            refs: Vec::new(),
                            ty: self.binding_ty(index.id, &index.ty),
                            borrowed: false,
                        },
                    );
                }
                self.breaks.push(Vec::new());
                for _ in 0..2 {
                    for (inside, join) in state.iter().zip(&carried.joins) {
                        if let Some(binding) = self
                            .current
                            .get(&join.binding)
                            .and_then(|id| self.bindings.get(id))
                            .cloned()
                        {
                            self.bindings.insert(inside.id, binding);
                        }
                    }
                    // A while condition runs again on every back-edge. Its
                    // erased observations still need valid permissions after
                    // the preceding iteration's writes.
                    if let Expr::While { condition, .. } = expr {
                        self.expr(condition, true)?;
                    }
                    self.block(body)?;
                }
                self.joins(&carried.joins);
                self.live = outer;
                let refs = self.breaks.pop().unwrap_or_default();
                self.valid(&refs)?;
                Ok(refs)
            }
            Expr::Break(value) => {
                let refs = match value {
                    Some(v) => self.expr(v, true)?,
                    None => Vec::new(),
                };
                if let Some(current) = self.breaks.last_mut() {
                    *current = merged(std::mem::take(current), refs);
                }
                Ok(Vec::new())
            }
            Expr::Return { value, .. } => {
                let refs = match value {
                    Some(v) => self.expr(v, true)?,
                    None => Vec::new(),
                };
                self.returned(&refs)?;
                Ok(Vec::new())
            }
            Expr::Method {
                receiver,
                arguments,
                ..
            } => {
                self.expr(receiver, true)?;
                for a in arguments {
                    self.expr(a, true)?;
                }
                Ok(Vec::new())
            }
            Expr::Operate { operands, .. } | Expr::IntArith { operands, .. } => {
                for a in operands {
                    self.expr(a, true)?;
                }
                Ok(Vec::new())
            }
            Expr::Compare { left, right, .. } => {
                self.expr(left, true)?;
                self.expr(right, true)?;
                Ok(Vec::new())
            }
            Expr::Cast { expr, .. } => {
                self.expr(expr, false)?;
                Ok(Vec::new())
            }
            Expr::Assert { condition, .. } => {
                self.expr(condition, true)?;
                Ok(Vec::new())
            }
            Expr::Bool(_)
            | Expr::Literal(..)
            | Expr::Int(_)
            | Expr::Continue
            | Expr::Panic { .. } => Ok(Vec::new()),
        }
    }
    fn joins(&mut self, joins: &[super::Join]) {
        for join in joins {
            let entries: Vec<_> = self
                .bindings
                .values()
                .filter(|b| b.root == join.binding)
                .cloned()
                .collect();
            if let Some(mut last) = entries.last().cloned() {
                last.refs = entries
                    .into_iter()
                    .fold(Vec::new(), |r, b| merged(r, b.refs));
                self.current.insert(join.binding, join.version.id);
                self.bindings.insert(join.version.id, last);
            }
        }
    }
}
fn lifetimes(ty: &EType, out: &mut Vec<String>) {
    match ty {
        EType::Ref(l, inner) => {
            if let Some(l) = l
                && !out.contains(l)
            {
                out.push(l.clone());
            }
            lifetimes(inner, out)
        }
        EType::Tuple(fs) => {
            for f in fs {
                lifetimes(f, out)
            }
        }
        EType::StructApplied(_, args) | EType::EnumApplied(_, args) => {
            for l in args {
                if !out.contains(l) {
                    out.push(l.clone());
                }
            }
        }
        _ => {}
    }
}
fn rename(ty: &mut EType, names: &[String], args: &[String]) {
    match ty {
        EType::Ref(l, inner) => {
            if let Some(name) = l
                && let Some(i) = names.iter().position(|n| n == name)
                && let Some(arg) = args.get(i)
            {
                *name = arg.clone();
            }
            rename(inner, names, args)
        }
        EType::Tuple(fs) => {
            for f in fs {
                rename(f, names, args)
            }
        }
        EType::StructApplied(_, nested) | EType::EnumApplied(_, nested) => {
            for n in nested {
                if let Some(i) = names.iter().position(|s| s == n)
                    && let Some(a) = args.get(i)
                {
                    *n = a.clone();
                }
            }
        }
        _ => {}
    }
}
fn applied_fields(ty: &EType, mut fields: Vec<EType>) -> Vec<EType> {
    if let EType::StructApplied(_, args) | EType::EnumApplied(_, args) = ty {
        let mut names = Vec::new();
        for f in &fields {
            lifetimes(f, &mut names)
        }
        for f in &mut fields {
            rename(f, &names, args)
        }
    }
    fields
}
