//! Resolve calls in a template against its declared interfaces before concrete
//! substitution. This is a capability pass, not universal proof checking.
use super::specs::walk::{self, Walk};
use crate::{ast::*, diagnostic::Diagnostic, source::Span};
use std::collections::{BTreeMap, BTreeSet};
type Types = BTreeMap<String, Type>;

fn params(d: &Declaration) -> &[GenericParameter] {
    match &d.kind {
        DeclarationKind::Function { generics, .. }
        | DeclarationKind::Struct { generics, .. }
        | DeclarationKind::Enum { generics, .. }
        | DeclarationKind::Prop { generics, .. }
        | DeclarationKind::Impl { generics, .. }
        | DeclarationKind::SpecImpl { generics, .. } => generics,
        _ => &[],
    }
}
fn requirements(d: &Declaration) -> Vec<WherePredicate> {
    let mut ps = d.constraints.clone();
    for p in params(d).iter().filter(|p| !p.lifetime) {
        ps.push(WherePredicate {
            subject: named(&p.name.text, p.span),
            bounds: p.bounds.clone(),
            span: p.span,
        });
    }
    ps
}
fn named(s: &str, span: Span) -> Type {
    Type {
        kind: TypeKind::Named(Name {
            text: s.into(),
            span,
        }),
        span,
    }
}
fn key(t: &Type) -> String {
    match &t.kind {
        TypeKind::Named(n) => n.text.clone(),
        TypeKind::Path { path, arguments } if arguments.is_empty() => path.text(),
        TypeKind::Path { path, arguments } => format!(
            "{}<{}>",
            path.text(),
            arguments.iter().map(key).collect::<Vec<_>>().join(",")
        ),
        TypeKind::Group(t) => key(t),
        TypeKind::Ref {
            inner,
            mutable,
            lifetime,
        } => format!(
            "&{}{} {}",
            lifetime.as_ref().map(|n| n.text.as_str()).unwrap_or(""),
            if *mutable { "mut" } else { "" },
            key(inner)
        ),
        TypeKind::Tuple(fields) => format!(
            "({})",
            fields
                .iter()
                .map(|f| key(&f.ty))
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => {
            struct Canon;
            impl Walk for Canon {
                fn span(&mut self, span: &mut Span) {
                    *span = Span::new(crate::source::FileId(0), 0, 0);
                }
            }
            let mut t = t.clone();
            Canon.ty(&mut t);
            format!("{t:?}")
        }
    }
}
fn receiver_type(mut ty: &Type) -> &Type {
    while let TypeKind::Ref { inner, .. } | TypeKind::Group(inner) = &ty.kind {
        ty = inner;
    }
    ty
}
fn item_name(d: &Declaration) -> Option<&Name> {
    match &d.kind {
        DeclarationKind::Trait { name, .. }
        | DeclarationKind::Function { name, .. }
        | DeclarationKind::Struct { name, .. }
        | DeclarationKind::Enum { name, .. }
        | DeclarationKind::Prop { name, .. }
        | DeclarationKind::AssociatedType { name, .. }
        | DeclarationKind::Constant { name, .. } => Some(name),
        _ => None,
    }
}
fn substitute(t: &mut Type, bs: &Types) {
    struct Sub<'a>(&'a Types);
    impl Walk for Sub<'_> {
        fn ty(&mut self, t: &mut Type) {
            if let TypeKind::Named(n) = &t.kind
                && let Some(to) = self.0.get(&n.text)
            {
                *t = to.clone();
                return;
            }
            walk::ty(self, t);
        }
        fn path(&mut self, p: &mut Path) {
            let i = usize::from(p.segments.first().is_some_and(|n| n.text == "<qualified>"));
            if let Some(n) = p.segments.get_mut(i)
                && let Some(Type {
                    kind: TypeKind::Named(to),
                    ..
                }) = self.0.get(&n.text)
            {
                n.text = to.text.clone();
            }
        }
    }
    Sub(bs).ty(t);
}
fn infer(formal: &Type, actual: &Type, variables: &BTreeSet<String>, bs: &mut Types) {
    match (&formal.kind, &actual.kind) {
        (TypeKind::Named(n), _) if variables.contains(&n.text) => {
            bs.entry(n.text.clone()).or_insert_with(|| actual.clone());
        }
        (TypeKind::Ref { inner: f, .. }, TypeKind::Ref { inner: a, .. })
        | (TypeKind::Group(f), TypeKind::Group(a))
        | (TypeKind::Array { element: f, .. }, TypeKind::Array { element: a, .. })
        | (TypeKind::Slice(f), TypeKind::Slice(a)) => infer(f, a, variables, bs),
        (
            TypeKind::Path {
                path: p,
                arguments: f,
            },
            TypeKind::Path {
                path: q,
                arguments: a,
            },
        ) if p.text() == q.text() => {
            for (f, a) in f.iter().zip(a) {
                infer(f, a, variables, bs);
            }
        }
        (TypeKind::Tuple(fs), TypeKind::Tuple(as_)) => {
            for (f, a) in fs.iter().zip(as_) {
                infer(&f.ty, &a.ty, variables, bs);
            }
        }
        _ => {}
    }
}

pub(super) fn prepare(program: &mut Program, errors: &mut Vec<Diagnostic>) {
    let mut items = BTreeMap::new();
    for d in &program.declarations {
        if let Some(n) = item_name(d) {
            items.insert(n.text.clone(), d.clone());
        }
        if let DeclarationKind::Impl {
            target, methods, ..
        } = &d.kind
        {
            for m in methods {
                if let Some(n) = item_name(m) {
                    items.insert(format!("{}::{}", target.text(), n.text), m.clone());
                }
            }
        }
    }
    let families = program.declarations.iter().filter(|d| {
        matches!(&d.kind, DeclarationKind::SpecImpl { target, .. }
            if items.get(&key(target)).is_some_and(|d| matches!(d.kind, DeclarationKind::Trait { .. })))
            || matches!(d.kind, DeclarationKind::Impl { .. })
    }).cloned().collect::<Vec<_>>();
    for d in &mut program.declarations {
        let generic_names = params(d)
            .iter()
            .filter(|p| !p.lifetime)
            .map(|p| p.name.text.clone())
            .collect::<BTreeSet<_>>();
        let context = requirements(d);
        let self_type = match &d.kind {
            DeclarationKind::SpecImpl { representation, .. } => Some(representation.clone()),
            DeclarationKind::Impl {
                target, generics, ..
            } => Some(Type {
                kind: TypeKind::Path {
                    path: Box::new(target.clone()),
                    arguments: generics
                        .iter()
                        .map(|p| named(&p.name.text, p.span))
                        .collect(),
                },
                span: d.span,
            }),
            _ => None,
        };
        if let DeclarationKind::Impl { methods, .. }
        | DeclarationKind::SpecImpl {
            members: methods, ..
        } = &mut d.kind
        {
            for method in methods {
                let mut names = generic_names.clone();
                names.extend(params(method).iter().map(|p| p.name.text.clone()));
                let mut ps = context.clone();
                ps.extend(requirements(method));
                if !names.is_empty() {
                    Contract {
                        items: &items,
                        families: &families,
                        variables: names,
                        requirements: ps,
                        locals: self_type
                            .clone()
                            .map(|t| Types::from([("self".into(), t)]))
                            .unwrap_or_default(),
                        errors,
                        loop_results: Vec::new(),
                        type_depth: 0,
                    }
                    .declaration(method);
                }
            }
        } else if !generic_names.is_empty() {
            Contract {
                items: &items,
                families: &families,
                variables: generic_names,
                requirements: context,
                locals: Types::new(),
                errors,
                loop_results: Vec::new(),
                type_depth: 0,
            }
            .declaration(d);
        }
    }
}
struct Contract<'a> {
    items: &'a BTreeMap<String, Declaration>,
    families: &'a [Declaration],
    variables: BTreeSet<String>,
    requirements: Vec<WherePredicate>,
    locals: Types,
    loop_results: Vec<Option<Type>>,
    type_depth: usize,
    errors: &'a mut Vec<Diagnostic>,
}
impl Contract<'_> {
    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(Diagnostic::error("L0281",message,span).note("generic operations must be justified by the declared bounds, independently of a particular instantiation"));
    }
    fn abstract_type(&self, t: &Type) -> bool {
        let k = key(t);
        self.variables.contains(&k)
            || self.requirements.iter().any(|p| key(&p.subject) == k)
            || matches!(&t.kind,TypeKind::Path{path,..} if path.segments.first().is_some_and(|n|n.text=="<qualified>" && path.segments.get(1).is_some_and(|n|self.variables.contains(&n.text))))
    }
    fn bound_for(&mut self, t: &Type, member: &str, span: Span) -> Option<GenericBound> {
        let mut found = BTreeMap::new();
        for p in &self.requirements {
            if key(&p.subject) != key(t) {
                continue;
            }
            for b in &p.bounds {
                if let Some(Declaration {
                    kind: DeclarationKind::Trait { members, .. },
                    ..
                }) = self.items.get(&b.text())
                    && members
                        .iter()
                        .any(|m| item_name(m).is_some_and(|n| n.text == member))
                {
                    found.insert(b.text(), b.clone());
                }
            }
        }
        if found.len() == 1 {
            return found.into_values().next();
        }
        self.error(
            if found.is_empty() {
                format!("no declared bound on `{}` supplies `{member}`", key(t))
            } else {
                format!(
                    "multiple bounds supply `{member}`; qualify it with `<T as Trait>::{member}`"
                )
            },
            span,
        );
        None
    }
    fn signature(&self, b: &GenericBound, member: &str) -> Option<Declaration> {
        let DeclarationKind::Trait { members, .. } = &self.items.get(&b.text())?.kind else {
            return None;
        };
        members
            .iter()
            .find(|m| item_name(m).is_some_and(|n| n.text == member))
            .cloned()
    }
    fn result(
        &mut self,
        d: &Declaration,
        owner: Option<(&Type, &GenericBound)>,
        bindings: &Types,
    ) -> Option<Type> {
        let mut result = match &d.kind {
            DeclarationKind::Function { result, .. }
            | DeclarationKind::Constant { ty: result, .. } => result.clone(),
            _ => return None,
        };
        substitute(&mut result, bindings);
        if let Some((t, b)) = owner {
            struct SelfTypes<'a> {
                subject: &'a Type,
                bound: &'a GenericBound,
            }
            impl Walk for SelfTypes<'_> {
                fn ty(&mut self, t: &mut Type) {
                    if matches!(&t.kind,TypeKind::Named(n) if n.text=="Self") {
                        *t = self.subject.clone();
                        return;
                    }
                    if let TypeKind::Path { path, arguments } = &t.kind
                        && arguments.is_empty()
                        && path.segments.first().is_some_and(|n| n.text == "Self")
                        && path.segments.len() == 2
                    {
                        let member = path.segments[1].clone();
                        if let Some((_, value)) = self
                            .bound
                            .associated
                            .iter()
                            .find(|(n, _)| n.text == member.text)
                        {
                            *t = value.clone();
                            return;
                        }
                        if let TypeKind::Named(n) = &receiver_type(self.subject).kind {
                            t.kind = TypeKind::Path {
                                path: Box::new(Path {
                                    segments: vec![
                                        Name {
                                            text: "<qualified>".into(),
                                            span: t.span,
                                        },
                                        n.clone(),
                                        Name {
                                            text: self.bound.text(),
                                            span: t.span,
                                        },
                                        member,
                                    ],
                                    span: t.span,
                                }),
                                arguments: vec![],
                            };
                            return;
                        }
                    }
                    walk::ty(self, t);
                }
            }
            SelfTypes {
                subject: receiver_type(t),
                bound: b,
            }
            .ty(&mut result);
        }
        Some(result)
    }
    fn declaration(&mut self, d: &mut Declaration) {
        let mut requirements = self.requirements.clone();
        for p in &mut requirements {
            self.ty(&mut p.subject);
        }
        self.requirements = requirements;
        for p in &mut d.constraints {
            self.ty(&mut p.subject);
        }
        if let DeclarationKind::Function {
            parameters,
            result,
            body,
            ..
        } = &mut d.kind
        {
            for p in parameters {
                self.ty(&mut p.ty);
                self.locals.insert(p.name.text.clone(), p.ty.clone());
            }
            self.ty(result);
            self.block(body);
        } else {
            walk::member(self, d);
        }
    }
    fn bind(&mut self, p: &Pattern, ty: Option<&Type>) {
        match &p.kind {
            PatternKind::Name { name, .. } => {
                if let Some(t) = ty {
                    self.locals.insert(name.text.clone(), t.clone());
                } else {
                    self.locals.remove(&name.text);
                }
            }
            PatternKind::Group(p) => self.bind(p, ty),
            PatternKind::Binding { name, pattern, .. } => {
                if let Some(t) = ty {
                    self.locals.insert(name.text.clone(), t.clone());
                }
                self.bind(pattern, ty);
            }
            PatternKind::Tuple(ps) => {
                for (i, p) in ps.iter().enumerate() {
                    self.bind(
                        p,
                        ty.and_then(|t| {
                            if let TypeKind::Tuple(fs) = &t.kind {
                                fs.get(i).map(|f| &f.ty)
                            } else {
                                None
                            }
                        }),
                    );
                }
            }
            PatternKind::Variant {
                path,
                arguments: Some(ps),
            } => {
                let fields = ty.and_then(|t| self.variant_fields(t, &path.last().text));
                for (i, p) in ps.iter().enumerate() {
                    self.bind(p, fields.as_ref().and_then(|fs| fs.get(i)).map(|f| &f.ty));
                }
            }
            PatternKind::Struct { fields, .. } => {
                let types = ty.and_then(|t| self.fields(t));
                for f in fields {
                    let name = f.name.as_ref().or({
                        if let PatternKind::Name { name, .. } = &f.pattern.kind {
                            Some(name)
                        } else {
                            None
                        }
                    });
                    self.bind(
                        &f.pattern,
                        types
                            .as_ref()
                            .and_then(|ts| {
                                ts.iter().find(|t| {
                                    t.name.as_ref().map(|n| &n.text) == name.map(|n| &n.text)
                                })
                            })
                            .map(|f| &f.ty),
                    );
                }
            }
            PatternKind::Evidence {
                constructor,
                evidence,
                ..
            } => {
                self.bind(constructor, ty);
                self.bind(evidence, None);
            }
            _ => {}
        }
    }
    fn variant_fields(&self, t: &Type, variant: &str) -> Option<Vec<TypeField>> {
        let (name, args) = match &receiver_type(t).kind {
            TypeKind::Named(n) => (n.text.clone(), vec![]),
            TypeKind::Path { path, arguments } => (path.text(), arguments.clone()),
            _ => return None,
        };
        if let Some(index) = match (name.as_str(), variant) {
            ("Option", "Some") | ("Result", "Ok") => Some(0),
            ("Result", "Err") => Some(1),
            _ => None,
        } {
            return args.get(index).map(|ty| {
                vec![TypeField {
                    name: None,
                    ty: ty.clone(),
                    span: t.span,
                }]
            });
        }
        let d = self.items.get(&name)?;
        let DeclarationKind::Enum { variants, .. } = &d.kind else {
            return None;
        };
        let mut fields = variants
            .iter()
            .find(|v| v.name.text == variant)?
            .fields
            .clone();
        let bs = params(d)
            .iter()
            .zip(args)
            .map(|(p, a)| (p.name.text.clone(), a))
            .collect::<Types>();
        for f in &mut fields {
            substitute(&mut f.ty, &bs);
        }
        Some(fields)
    }
    fn fields(&self, t: &Type) -> Option<Vec<TypeField>> {
        let (name, args) = match &receiver_type(t).kind {
            TypeKind::Named(n) => (n.text.clone(), vec![]),
            TypeKind::Path { path, arguments } => (path.text(), arguments.clone()),
            TypeKind::Tuple(fs) => return Some(fs.clone()),
            _ => return None,
        };
        let d = self.items.get(&name)?;
        let bs = params(d)
            .iter()
            .zip(args)
            .map(|(p, a)| (p.name.text.clone(), a))
            .collect::<Types>();
        let mut fields = match &d.kind {
            DeclarationKind::Struct { fields, .. } => fields
                .iter()
                .map(|f| TypeField {
                    name: Some(f.name.clone()),
                    ty: f.ty.clone(),
                    span: f.span,
                })
                .collect::<Vec<_>>(),
            _ => return None,
        };
        for f in &mut fields {
            substitute(&mut f.ty, &bs);
        }
        Some(fields)
    }
    fn constructor_type(
        &mut self,
        name: &str,
        actual: &[Option<Type>],
        explicit: &[Type],
        span: Span,
    ) -> Option<Type> {
        let (owner, variant) = name.rsplit_once("::")?;
        let arguments = if owner == "Option" && variant == "Some" {
            explicit
                .first()
                .cloned()
                .or_else(|| actual.first().cloned().flatten())
                .map(|t| vec![t])?
        } else {
            let d = self.items.get(owner)?.clone();
            let DeclarationKind::Enum { variants, .. } = &d.kind else {
                return None;
            };
            let fields = &variants.iter().find(|v| v.name.text == variant)?.fields;
            let variables = params(&d).iter().map(|p| p.name.text.clone()).collect();
            let mut bindings = params(&d)
                .iter()
                .zip(explicit)
                .map(|(p, a)| (p.name.text.clone(), a.clone()))
                .collect();
            for (field, ty) in fields.iter().zip(actual) {
                if let Some(ty) = ty {
                    infer(&field.ty, ty, &variables, &mut bindings);
                }
            }
            self.require_declared(&d, &bindings, span);
            params(&d)
                .iter()
                .map(|p| bindings.get(&p.name.text).cloned())
                .collect::<Option<Vec<_>>>()?
        };
        Some(Type {
            kind: TypeKind::Path {
                path: Box::new(Path {
                    segments: vec![Name {
                        text: owner.into(),
                        span,
                    }],
                    span,
                }),
                arguments,
            },
            span,
        })
    }
    fn value(&mut self, e: &mut Expr) -> Option<Type> {
        let span = e.span;
        match &mut e.kind {
            ExprKind::Name(n) => self.locals.get(&n.text).cloned().or_else(|| {
                self.items.get(&n.text).and_then(|d| {
                    if let DeclarationKind::Constant { ty, .. } = &d.kind {
                        Some(ty.clone())
                    } else {
                        None
                    }
                })
            }),
            ExprKind::Path(p) => {
                if p.segments.len() == 2 && self.variables.contains(&p.segments[0].text) {
                    let t = named(&p.segments[0].text, span);
                    let member = p.segments[1].text.clone();
                    let b = self.bound_for(&t, &member, span)?;
                    let d = self.signature(&b, &member)?;
                    p.segments[1].text = super::traits::lowered(&b.text(), &member);
                    return self.result(&d, Some((&t, &b)), &Types::new());
                }
                if p.segments.first().is_some_and(|n| n.text == "<qualified>")
                    && self.variables.contains(&p.segments[1].text)
                {
                    let t = named(&p.segments[1].text, span);
                    let interface = p.segments[2].text.clone();
                    let member = p.segments[3].text.clone();
                    let b = self
                        .requirements
                        .iter()
                        .filter(|r| key(&r.subject) == key(&t))
                        .flat_map(|r| &r.bounds)
                        .find(|b| b.text() == interface)
                        .cloned();
                    let Some(b) = b else {
                        self.error(
                            format!(
                                "qualified call requires the bound `{}: {interface}`",
                                key(&t)
                            ),
                            span,
                        );
                        return None;
                    };
                    let d = self.signature(&b, &member)?;
                    p.segments = vec![
                        p.segments[1].clone(),
                        Name {
                            text: super::traits::lowered(&interface, &member),
                            span,
                        },
                    ];
                    return self.result(&d, Some((&t, &b)), &Types::new());
                }
                self.items
                    .get(&p.text())
                    .cloned()
                    .and_then(|d| self.result(&d, None, &Types::new()))
            }
            ExprKind::Call { callee, arguments } => {
                let mut explicit = Vec::new();
                let target = if let ExprKind::GenericApply { callee, arguments } = &mut callee.kind
                {
                    explicit = arguments.clone();
                    &mut **callee
                } else {
                    &mut **callee
                };
                if let ExprKind::Member { value, name } = &mut target.kind {
                    let receiver = self.value(value);
                    for a in arguments {
                        self.value(a);
                    }
                    if let Some(t) = receiver {
                        if self.abstract_type(receiver_type(&t)) {
                            let b = self.bound_for(receiver_type(&t), &name.text, name.span)?;
                            let d = self.signature(&b, &name.text)?;
                            name.text = super::traits::lowered(&b.text(), &name.text);
                            return self.result(&d, Some((&t, &b)), &Types::new());
                        }
                        let receiver = receiver_type(&t);
                        if let TypeKind::Path { path, arguments } = &receiver.kind {
                            for implementation in self.families {
                                let DeclarationKind::Impl {
                                    target, methods, ..
                                } = &implementation.kind
                                else {
                                    continue;
                                };
                                if target.text() != path.text() {
                                    continue;
                                }
                                if let Some(method) = methods
                                    .iter()
                                    .find(|m| item_name(m).is_some_and(|n| n.text == name.text))
                                {
                                    let mut bindings: Types = params(implementation)
                                        .iter()
                                        .zip(arguments)
                                        .map(|(p, a)| (p.name.text.clone(), a.clone()))
                                        .collect();
                                    bindings.insert("Self".into(), receiver.clone());
                                    self.require_declared(implementation, &bindings, span);
                                    self.require_declared(method, &bindings, span);
                                    return self.result(method, None, &bindings);
                                }
                            }
                        }
                        let k = format!("{}::{}", key(receiver), name.text);
                        return self
                            .items
                            .get(&k)
                            .cloned()
                            .and_then(|d| self.result(&d, None, &Types::new()));
                    }
                    return None;
                }
                let name = match &target.kind {
                    ExprKind::Name(n) => Some(n.text.clone()),
                    ExprKind::Path(p) => Some(p.text()),
                    _ => None,
                };
                let selected = self.value(target);
                let actual = arguments
                    .iter_mut()
                    .map(|a| self.value(a))
                    .collect::<Vec<_>>();
                if let Some(d) = name.as_ref().and_then(|n| self.items.get(n)).cloned() {
                    let variables = params(&d)
                        .iter()
                        .map(|p| p.name.text.clone())
                        .collect::<BTreeSet<_>>();
                    let mut bs = params(&d)
                        .iter()
                        .zip(explicit.clone())
                        .map(|(p, a)| (p.name.text.clone(), a))
                        .collect::<Types>();
                    if let DeclarationKind::Function { parameters, .. } = &d.kind {
                        for (f, a) in parameters.iter().zip(&actual) {
                            if let Some(a) = a {
                                infer(&f.ty, a, &variables, &mut bs);
                            }
                        }
                    }
                    self.require_declared(&d, &bs, span);
                    return self.result(&d, None, &bs);
                }
                if let Some(name) = &name
                    && let Some(ty) = self.constructor_type(name, &actual, &explicit, span)
                {
                    return Some(ty);
                }
                selected.map(|t| match t.kind {
                    TypeKind::Function { result, .. }
                    | TypeKind::LogicalFunction { result, .. } => *result,
                    _ => t,
                })
            }
            ExprKind::Member { value, name } => {
                let t = self.value(value)?;
                if self.abstract_type(receiver_type(&t)) {
                    self.error(
                        format!(
                            "cannot access field `{}` through an abstract type",
                            name.text
                        ),
                        span,
                    );
                    return None;
                }
                self.fields(&t)?
                    .into_iter()
                    .find(|f| f.name.as_ref().is_some_and(|n| n.text == name.text))
                    .map(|f| f.ty)
            }
            ExprKind::Group(v) => self.value(v),
            ExprKind::Ref {
                expr: value,
                mutable,
                ..
            } => self.value(value).map(|t| Type {
                kind: TypeKind::Ref {
                    inner: Box::new(t),
                    mutable: *mutable,
                    lifetime: None,
                },
                span,
            }),
            ExprKind::Unary {
                operator: UnaryOp::Deref,
                expr: v,
                ..
            } => self.value(v).map(|t| receiver_type(&t).clone()),
            ExprKind::Tuple(vs) => {
                let fields = vs
                    .iter_mut()
                    .map(|v| {
                        self.value(v).map(|ty| TypeField {
                            name: None,
                            ty,
                            span: v.span,
                        })
                    })
                    .collect::<Vec<_>>();
                fields
                    .into_iter()
                    .collect::<Option<Vec<_>>>()
                    .map(|fields| Type {
                        kind: TypeKind::Tuple(fields),
                        span,
                    })
            }
            ExprKind::Array(values) => {
                let count = values.len();
                let types = values.iter_mut().map(|v| self.value(v)).collect::<Vec<_>>();
                types.into_iter().flatten().next().map(|element| Type {
                    kind: TypeKind::Array {
                        element: Box::new(element),
                        length: Box::new(Expr {
                            kind: ExprKind::Integer(IntegerLiteral {
                                value: (count as u64).into(),
                                suffix: None,
                            }),
                            span,
                        }),
                    },
                    span,
                })
            }
            ExprKind::Subscript { value, index } => {
                let ty = self.value(value);
                self.value(index);
                ty.and_then(|t| match &receiver_type(&t).kind {
                    TypeKind::Array { element, .. } | TypeKind::Slice(element) => {
                        Some(*element.clone())
                    }
                    TypeKind::Path { path, arguments } if path.text() == "Vec" => {
                        arguments.first().cloned()
                    }
                    _ => None,
                })
            }
            ExprKind::Loop { body } => {
                self.loop_results.push(None);
                self.block_value(body);
                self.loop_results.pop().flatten()
            }
            ExprKind::Break(value) => {
                let ty = value.as_mut().and_then(|v| self.value(v));
                if let Some(slot) = self.loop_results.last_mut()
                    && slot.is_none()
                {
                    *slot = ty;
                }
                None
            }
            ExprKind::Index { value, index, .. } => {
                let t = self.value(value)?;
                if let TypeKind::Tuple(fs) = t.kind {
                    fs.get(index.parse::<usize>().ok()?).map(|f| f.ty.clone())
                } else {
                    None
                }
            }
            ExprKind::Block(b) | ExprKind::Logic(b) => self.block_value(b),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.value(condition);
                let t = self.block_value(then_branch);
                self.value(else_branch).or(t)
            }
            ExprKind::Match { scrutinee, arms } => {
                let t = self.value(scrutinee);
                let old = self.locals.clone();
                let mut result = None;
                for a in arms {
                    self.locals = old.clone();
                    self.bind(&a.pattern, t.as_ref());
                    let found = self.value(&mut a.body);
                    if result.is_none() {
                        result = found;
                    }
                }
                self.locals = old;
                result
            }
            ExprKind::While {
                pattern,
                condition,
                body,
            } => {
                let ty = self.value(condition);
                let old = self.locals.clone();
                if let Some(p) = pattern {
                    self.bind(p, ty.as_ref());
                }
                self.block_value(body);
                self.locals = old;
                None
            }
            ExprKind::Closure { parameters, body } => {
                let old = self.locals.clone();
                for p in &mut *parameters {
                    self.ty(&mut p.ty);
                    self.locals.insert(p.name.text.clone(), p.ty.clone());
                }
                let result = self.value(body);
                self.locals = old;
                result.map(|t| Type {
                    kind: TypeKind::LogicalFunction {
                        parameters: parameters
                            .iter()
                            .map(|p| TypeField {
                                name: Some(p.name.clone()),
                                ty: p.ty.clone(),
                                span: p.span,
                            })
                            .collect(),
                        result: Box::new(t),
                    },
                    span,
                })
            }
            ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } => {
                let old = self.locals.clone();
                for p in parameters {
                    self.ty(&mut p.ty);
                    self.locals.insert(p.name.text.clone(), p.ty.clone());
                }
                self.block_value(body);
                self.locals = old;
                Some(named("Prop", span))
            }
            ExprKind::Struct { path, fields } => {
                let declaration = self.items.get(&path.text()).cloned();
                let variables = declaration
                    .as_ref()
                    .map(|d| params(d).iter().map(|p| p.name.text.clone()).collect())
                    .unwrap_or_default();
                let mut bs = Types::new();
                for f in fields {
                    let actual = self.value(&mut f.value);
                    let name = f.name.as_ref().or({
                        if let ExprKind::Name(n) = &f.value.kind {
                            Some(n)
                        } else {
                            None
                        }
                    });
                    if let Some(Declaration {
                        kind: DeclarationKind::Struct { fields, .. },
                        ..
                    }) = &declaration
                        && let Some(formal) = fields
                            .iter()
                            .find(|f| Some(&f.name.text) == name.map(|n| &n.text))
                        && let Some(actual) = actual
                    {
                        infer(&formal.ty, &actual, &variables, &mut bs);
                    }
                }
                let arguments = declaration
                    .as_ref()
                    .map(|d| {
                        params(d)
                            .iter()
                            .filter_map(|p| bs.get(&p.name.text).cloned())
                            .collect()
                    })
                    .unwrap_or_default();
                Some(Type {
                    kind: TypeKind::Path {
                        path: Box::new(path.clone()),
                        arguments,
                    },
                    span,
                })
            }
            ExprKind::Cast {
                expr: value, ty, ..
            } => {
                self.value(value);
                self.ty(ty);
                Some(ty.clone())
            }
            ExprKind::Integer(lit) => lit.suffix.map(|s| named(s.name(), span)),
            ExprKind::Bool(_) => Some(named("bool", span)),
            ExprKind::Form {
                form, arguments, ..
            } => {
                if matches!(*form,Form::Fold|Form::Unfold) && arguments.first().is_some_and(|a|matches!(&a.kind,ExprKind::Path(p) if p.segments.first().is_some_and(|n|self.variables.contains(&n.text)) || p.segments.first().is_some_and(|n|n.text=="<qualified>" && self.variables.contains(&p.segments[1].text)))) {
                    self.error("cannot unfold an abstract trait operation; call a proof-bearing law from its interface",span);
                }
                for a in arguments {
                    self.value(a);
                }
                match form {
                    Form::Prop => Some(named("Prop", span)),
                    _ => None,
                }
            }
            _ => {
                walk::expr(self, e);
                None
            }
        }
    }
    fn block_value(&mut self, b: &mut Block) -> Option<Type> {
        let old = self.locals.clone();
        for s in &mut b.statements {
            match &mut s.kind {
                StatementKind::Let {
                    pattern,
                    annotation,
                    value,
                    ..
                } => {
                    if let Some(t) = annotation {
                        self.ty(t);
                    }
                    let t = self.value(value);
                    self.bind(pattern, annotation.as_ref().or(t.as_ref()));
                }
                StatementKind::Expression(e) => {
                    self.value(e);
                }
                StatementKind::Assign { place, value } => {
                    self.value(place);
                    self.value(value);
                }
                _ => {}
            }
        }
        let t = b.tail.as_mut().and_then(|e| self.value(e));
        self.locals = old;
        t
    }
}
impl Walk for Contract<'_> {
    fn expr(&mut self, e: &mut Expr) {
        self.value(e);
    }
    fn block(&mut self, b: &mut Block) {
        self.block_value(b);
    }
    fn ty(&mut self, t: &mut Type) {
        if self.type_depth >= crate::limits::MAX_GENERIC_TYPE_DEPTH {
            self.error("generic bound expansion exceeds MAX_GENERIC_TYPE_DEPTH; check for a recursive requirement", t.span);
            return;
        }
        self.type_depth += 1;
        self.check_type(t);
        self.type_depth -= 1;
    }
}
impl Contract<'_> {
    fn check_type(&mut self, t: &mut Type) {
        if let TypeKind::Path { path, arguments } = &t.kind
            && !arguments.is_empty()
            && let Some(d) = self.items.get(&path.text()).cloned()
        {
            let bs = params(&d)
                .iter()
                .zip(arguments)
                .map(|(p, a)| (p.name.text.clone(), a.clone()))
                .collect();
            self.require_declared(&d, &bs, t.span);
        }
        if let TypeKind::Path { path, .. } = &t.kind
            && path
                .segments
                .first()
                .is_some_and(|n| n.text == "<qualified>")
            && self.variables.contains(&path.segments[1].text)
        {
            let owner = &path.segments[1];
            let interface = &path.segments[2];
            if !self
                .requirements
                .iter()
                .filter(|p| key(&p.subject) == owner.text)
                .flat_map(|p| &p.bounds)
                .any(|b| b.text() == interface.text)
            {
                self.error(
                    format!(
                        "associated projection requires the bound `{}: {}`",
                        owner.text, interface.text
                    ),
                    t.span,
                );
            }
        }
        if let TypeKind::Path { path, arguments } = &mut t.kind
            && arguments.is_empty()
            && path.segments.len() == 2
            && self.variables.contains(&path.segments[0].text)
        {
            let owner = path.segments[0].clone();
            let member = path.segments[1].clone();
            if let Some(b) = self.bound_for(&named(&owner.text, owner.span), &member.text, t.span) {
                if let Some((_, value)) = b.associated.iter().find(|(n, _)| n.text == member.text) {
                    *t = value.clone();
                    return;
                }
                path.segments = vec![
                    Name {
                        text: "<qualified>".into(),
                        span: t.span,
                    },
                    owner,
                    Name {
                        text: b.text(),
                        span: t.span,
                    },
                    member,
                ];
            }
        }
        walk::ty(self, t);
    }
}

impl Contract<'_> {
    fn contains_parameter(&self, t: &Type) -> bool {
        match &t.kind {
            TypeKind::Named(n) => self.variables.contains(&n.text),
            TypeKind::Path { path, arguments } => {
                path.segments
                    .iter()
                    .any(|n| self.variables.contains(&n.text))
                    || arguments.iter().any(|t| self.contains_parameter(t))
            }
            TypeKind::Group(t) | TypeKind::Ref { inner: t, .. } | TypeKind::Slice(t) => {
                self.contains_parameter(t)
            }
            TypeKind::Array { element, .. } => self.contains_parameter(element),
            TypeKind::Tuple(fs) => fs.iter().any(|f| self.contains_parameter(&f.ty)),
            _ => false,
        }
    }
    fn require_declared(&mut self, d: &Declaration, bs: &Types, span: Span) {
        for mut p in requirements(d) {
            substitute(&mut p.subject, bs);
            if !self.contains_parameter(&p.subject) {
                continue;
            }
            // Normalize the common T::Item spelling before comparing obligations.
            self.ty(&mut p.subject);
            for mut b in p.bounds {
                for (_, t) in &mut b.associated {
                    substitute(t, bs);
                }
                if !self.entails(&p.subject, &b, &mut BTreeSet::new()) {
                    self.error(
                        format!(
                            "use requires the undeclared bound `{}: {}`",
                            key(&p.subject),
                            b.text()
                        ),
                        span,
                    );
                }
            }
        }
    }
    fn entails(&self, subject: &Type, bound: &GenericBound, active: &mut BTreeSet<String>) -> bool {
        let request = format!("{}:{}", key(subject), bound.text());
        if active.len() >= crate::limits::MAX_GENERIC_TYPE_DEPTH || !active.insert(request.clone())
        {
            return false;
        }
        // Associated classification belongs to the interface, not to the
        // eventual representation selected for a concrete type.
        let implied = bound.text() == "Logical"
            && bound.associated.is_empty()
            && matches!(&subject.kind, TypeKind::Path { path, arguments }
            if arguments.is_empty() && path.segments.len() == 4
            && path.segments[0].text == "<qualified>"
            && self.requirements.iter().filter(|p| key(&p.subject) == path.segments[1].text)
                .flat_map(|p| &p.bounds).any(|b| b.text() == path.segments[2].text)
            && self.items.get(&path.segments[2].text).is_some_and(|d| {
                matches!(&d.kind, DeclarationKind::Trait { members, .. }
                    if members.iter().any(|m| matches!(&m.kind,
                        DeclarationKind::AssociatedType { name, logical: true, .. }
                            if name.text == path.segments[3].text)))
            }));
        let explicit = implied
            || self
                .requirements
                .iter()
                .filter(|p| key(&p.subject) == key(subject))
                .flat_map(|p| &p.bounds)
                .any(|given| {
                    given.text() == bound.text()
                        && bound.associated.iter().all(|(n, t)| {
                            given
                                .associated
                                .iter()
                                .any(|(gn, gt)| n.text == gn.text && key(t) == key(gt))
                        })
                });
        let derived = if explicit {
            true
        } else {
            self.families.iter().any(|d| {
                let DeclarationKind::SpecImpl {
                    target,
                    representation,
                    members,
                    ..
                } = &d.kind
                else {
                    return false;
                };
                if key(target) != bound.text() {
                    return false;
                }
                let (
                    TypeKind::Path {
                        path: family,
                        arguments: formal,
                    },
                    TypeKind::Path {
                        path: actual,
                        arguments: args,
                    },
                ) = (&representation.kind, &subject.kind)
                else {
                    return false;
                };
                if family.text() != actual.text() || formal.len() != args.len() {
                    return false;
                }
                let variables = params(d).iter().map(|p| p.name.text.clone()).collect();
                let mut bs = Types::new();
                infer(representation, subject, &variables, &mut bs);
                bs.insert("Self".into(), subject.clone());
                if !bound.associated.iter().all(|(name, expected)| {
                    members.iter().any(|m| {
                        if let DeclarationKind::AssociatedType {
                            name: n,
                            value: Some(t),
                            ..
                        } = &m.kind
                        {
                            let mut t = t.clone();
                            substitute(&mut t, &bs);
                            n.text == name.text && key(&t) == key(expected)
                        } else {
                            false
                        }
                    })
                }) {
                    return false;
                }
                requirements(d).into_iter().all(|mut p| {
                    substitute(&mut p.subject, &bs);
                    p.bounds.into_iter().all(|mut b| {
                        for (_, t) in &mut b.associated {
                            substitute(t, &bs);
                        }
                        self.entails(&p.subject, &b, active)
                    })
                })
            })
        };
        active.remove(&request);
        derived
    }
}
