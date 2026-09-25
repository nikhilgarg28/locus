//! Checked opaque spec types. Generated adapters are ordinary checked Locus AST.
//! Neither declarations nor implementation matching introduce kernel assumptions.
mod walk;
use super::resolve::Graph;
use crate::{ast::*, diagnostic::Diagnostic, source::Span};
use std::collections::{BTreeMap, BTreeSet};
use walk::Walk;

pub fn present(program: &Program) -> bool {
    program.declarations.iter().any(|d| match &d.kind {
        DeclarationKind::Spec { .. }
        | DeclarationKind::SpecImpl { .. }
        | DeclarationKind::ModuleImpl { .. } => true,
        DeclarationKind::Module { body: Some(p), .. } => present(p),
        _ => false,
    })
}
fn error(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic::error("L0511", message, span)
}
fn name(d: &Declaration) -> Option<&Name> {
    match &d.kind {
        DeclarationKind::Function { name, .. }
        | DeclarationKind::Constant { name, .. }
        | DeclarationKind::AssociatedType { name, .. } => Some(name),
        _ => None,
    }
}
fn n(s: impl Into<String>, span: Span) -> Name {
    Name {
        text: s.into(),
        span,
    }
}
fn path(s: impl Into<String>, span: Span) -> Path {
    Path {
        segments: vec![n(s, span)],
        span,
    }
}
fn named(s: impl Into<String>, span: Span) -> Type {
    Type {
        kind: TypeKind::Named(n(s, span)),
        span,
    }
}
fn type_name(t: &Type) -> Option<String> {
    match &t.kind {
        TypeKind::Named(n) => Some(n.text.clone()),
        TypeKind::Path { path, .. } => Some(path.text()),
        TypeKind::Group(t) => type_name(t),
        _ => None,
    }
}
fn arguments(t: &Type) -> &[Type] {
    match &t.kind {
        TypeKind::Path { arguments, .. } => arguments,
        _ => &[],
    }
}
fn var(s: impl Into<String>, span: Span) -> Expr {
    Expr {
        kind: ExprKind::Name(n(s, span)),
        span,
    }
}
fn block(e: Expr) -> Block {
    Block {
        span: e.span,
        statements: vec![],
        tail: Some(Box::new(e)),
    }
}
fn declaration(kind: DeclarationKind, span: Span) -> Declaration {
    Declaration {
        captures: Vec::new(),
        kind,
        span,
        doc: vec![],
        attributes: vec![],
        visibility: None,
    }
}
fn public(span: Span) -> Option<Visibility> {
    Some(Visibility {
        scope: VisibilityScope::Public,
        span,
    })
}
const REPR: &str = "__locus_repr";

/// Capture-free type substitution; terms retain original version/binder structure.
#[derive(Default, Clone)]
struct Rewrite {
    types: BTreeMap<String, Type>,
    paths: BTreeMap<String, Path>,
    values: BTreeMap<String, String>,
    receivers: BTreeMap<String, (Path, SelfKind)>,
}
impl Walk for Rewrite {
    fn ty(&mut self, t: &mut Type) {
        if let Some(key) = type_name(t)
            && let Some(to) = self.types.get(&key)
        {
            let span = t.span;
            *t = to.clone();
            t.span = span;
            return;
        }
        walk::ty(self, t);
    }
    fn path(&mut self, p: &mut Path) {
        if let Some(to) = self.paths.get(&p.text()) {
            let span = p.span;
            *p = to.clone();
            p.span = span;
            return;
        }
        if let Some(first) = p.segments.first()
            && let Some(to) = self.types.get(&first.text)
            && let Some(repr) = type_name(to)
        {
            p.segments[0].text = repr;
        }
    }
    fn expr(&mut self, e: &mut Expr) {
        if let ExprKind::Call { callee, arguments } = &mut e.kind
            && let ExprKind::Member { value, name } = &callee.kind
            && matches!(&value.kind,ExprKind::Name(n) if n.text=="self")
            && let Some((target, kind)) = self.receivers.get(&name.text).cloned()
        {
            let mut receiver = *value.clone();
            if matches!(kind, SelfKind::Ref | SelfKind::RefMut) {
                receiver = Expr {
                    span: receiver.span,
                    kind: ExprKind::Ref {
                        mutable: kind == SelfKind::RefMut,
                        expr: Box::new(receiver),
                    },
                };
            }
            arguments.insert(0, receiver);
            *callee = Box::new(Expr {
                span: callee.span,
                kind: ExprKind::Path(Box::new(target)),
            });
        }
        if let ExprKind::Name(name) = &mut e.kind
            && let Some(to) = self.values.get(&name.text)
        {
            name.text = to.clone();
        }
        walk::expr(self, e);
        if let ExprKind::Path(p) = &e.kind
            && let Some(name) = p.single()
        {
            e.kind = ExprKind::Name(name.clone());
        }
    }
}
/// Canonical comparison discards source locations, not proof contracts.
#[derive(Default)]
struct Canon {
    values: BTreeMap<String, String>,
    types: BTreeMap<String, String>,
    serial: usize,
}
impl Canon {
    fn bind(&mut self, n: &mut Name) {
        let replacement = format!("v{}", self.serial);
        self.serial += 1;
        self.values.insert(n.text.clone(), replacement.clone());
        n.text = replacement;
    }
}
impl Walk for Canon {
    fn span(&mut self, s: &mut Span) {
        *s = Span::new(crate::source::FileId(0), 0, 0);
    }
    fn ty(&mut self, t: &mut Type) {
        while let TypeKind::Group(inner) = &t.kind {
            *t = *inner.clone();
        }
        match &mut t.kind {
            TypeKind::Named(n) | TypeKind::Lifetime(n) => {
                if let Some(to) = self.types.get(&n.text) {
                    n.text = to.clone();
                }
            }
            TypeKind::Ref {
                lifetime: Some(n), ..
            } => {
                if let Some(to) = self.types.get(&n.text) {
                    n.text = to.clone();
                }
            }
            _ => {}
        }
        match &mut t.kind {
            TypeKind::Tuple(fs) => {
                let saved = self.values.clone();
                self.span(&mut t.span);
                self.fields(fs);
                self.values = saved;
                return;
            }
            TypeKind::Function { parameters, result }
            | TypeKind::LogicalFunction { parameters, result } => {
                let saved = self.values.clone();
                self.span(&mut t.span);
                self.fields(parameters);
                self.ty(result);
                self.values = saved;
                return;
            }
            _ => {}
        }
        walk::ty(self, t);
    }
    fn expr(&mut self, e: &mut Expr) {
        while let ExprKind::Group(inner) = &e.kind {
            *e = *inner.clone();
        }
        match &mut e.kind {
            ExprKind::Name(n) => {
                if let Some(to) = self.values.get(&n.text) {
                    n.text = to.clone();
                }
            }
            ExprKind::Closure { parameters, body } => {
                let saved = self.values.clone();
                self.span(&mut e.span);
                for p in parameters {
                    self.parameter(p);
                }
                self.expr(body);
                self.values = saved;
                return;
            }
            ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } => {
                let saved = self.values.clone();
                self.span(&mut e.span);
                for p in parameters {
                    self.parameter(p);
                }
                self.block(body);
                self.values = saved;
                return;
            }
            ExprKind::For {
                pattern,
                iterable,
                body,
            } => {
                let saved = self.values.clone();
                self.span(&mut e.span);
                self.expr(iterable);
                self.pattern(pattern);
                self.block(body);
                self.values = saved;
                return;
            }
            ExprKind::While {
                pattern,
                condition,
                body,
            } => {
                let saved = self.values.clone();
                self.span(&mut e.span);
                self.expr(condition);
                if let Some(p) = pattern {
                    self.pattern(p);
                }
                self.block(body);
                self.values = saved;
                return;
            }
            ExprKind::Match { scrutinee, arms } => {
                self.span(&mut e.span);
                self.expr(scrutinee);
                for a in arms {
                    let saved = self.values.clone();
                    self.pattern(&mut a.pattern);
                    self.expr(&mut a.body);
                    self.span(&mut a.span);
                    self.values = saved;
                }
                return;
            }
            _ => {}
        }
        walk::expr(self, e);
    }
    fn block(&mut self, b: &mut Block) {
        let saved = self.values.clone();
        walk::block(self, b);
        self.values = saved;
    }
    fn pattern(&mut self, p: &mut Pattern) {
        match &mut p.kind {
            PatternKind::Name { name, .. } | PatternKind::Binding { name, .. } => self.bind(name),
            _ => {}
        }
        walk::pattern(self, p);
    }
}
impl Canon {
    fn fields(&mut self, fs: &mut [TypeField]) {
        for f in fs {
            self.ty(&mut f.ty);
            if let Some(n) = &mut f.name {
                self.bind(n);
                self.name(n);
            }
            self.span(&mut f.span);
        }
    }
    fn parameter(&mut self, p: &mut Parameter) {
        self.ty(&mut p.ty);
        self.bind(&mut p.name);
        self.name(&mut p.name);
        self.span(&mut p.span);
    }
}

fn signature(d: &Declaration) -> DeclarationKind {
    let mut d = d.clone();
    let mut c = Canon::default();
    if let DeclarationKind::Function {
        generics,
        name,
        self_param,
        parameters,
        result,
        body,
        ..
    } = &mut d.kind
    {
        name.text.clear();
        c.name(name);
        for (i, g) in generics.iter_mut().enumerate() {
            let key = format!("T{i}");
            c.types.insert(g.name.text.clone(), key.clone());
            g.name.text = key;
            c.name(&mut g.name);
            c.span(&mut g.span);
            for b in &mut g.bounds {
                c.path(b);
            }
        }
        if let Some(s) = self_param {
            c.span(&mut s.span);
        }
        for p in parameters {
            c.ty(&mut p.ty);
            c.bind(&mut p.name);
            c.name(&mut p.name);
            c.span(&mut p.span);
        }
        c.ty(result);
        *body = Block {
            statements: vec![],
            tail: None,
            span: Span::new(crate::source::FileId(0), 0, 0),
        };
    } else if let DeclarationKind::Constant { name, ty, value } = &mut d.kind {
        name.text.clear();
        c.name(name);
        c.ty(ty);
        *value = Expr {
            kind: ExprKind::Hole,
            span: Span::new(crate::source::FileId(0), 0, 0),
        };
    }
    d.kind
}
fn qualified(owner: &str, member: &str, span: Span) -> Path {
    Path {
        segments: vec![n(owner, span), n(member, span)],
        span,
    }
}

pub fn lower(program: &mut Program, graph: &mut Graph, errors: &mut Vec<Diagnostic>) {
    let declarations = std::mem::take(&mut program.declarations);
    let specs: BTreeMap<_, _> = declarations
        .iter()
        .filter_map(|d| {
            if let DeclarationKind::Spec { name, .. } = &d.kind {
                Some((name.text.clone(), d.clone()))
            } else {
                None
            }
        })
        .collect();
    let mut implementations: BTreeMap<String, Vec<Declaration>> = BTreeMap::new();
    for d in &declarations {
        if let DeclarationKind::SpecImpl { target, .. } = &d.kind
            && let Some(key) = type_name(target)
        {
            if !specs.contains_key(&key) {
                errors.push(error(
                    "implementation target is not a spec type",
                    target.span,
                ));
            }
            implementations.entry(key).or_default().push(d.clone());
        }
    }
    let mut associated = BTreeMap::new();
    let mut generated = Vec::new();
    for (owner, spec) in &specs {
        let DeclarationKind::Spec {
            module,
            name: spec_name,
            generics,
            members,
        } = &spec.kind
        else {
            unreachable!()
        };
        if *module {
            errors.push(Diagnostic::error(
                "L0510",
                "module specs are deferred; use `spec type`",
                spec.span,
            ));
            continue;
        }
        if generics.iter().any(|g| g.lifetime) {
            errors.push(Diagnostic::error(
                "L0510",
                "lifetime-parameterized spec families are deferred",
                spec.span,
            ));
            continue;
        }
        for member in members {
            check_self_family(member, owner, generics, errors);
        }
        let impls = implementations.get(owner).map_or(&[][..], Vec::as_slice);
        if impls.len() != 1 {
            let mut e = error(
                format!(
                    "spec type `{}` requires exactly one implementation; found {}",
                    display(graph, owner),
                    impls.len()
                ),
                spec.span,
            );
            for i in impls {
                e = e.label(i.span, "implementation declared here");
            }
            errors.push(e);
            continue;
        }
        let implementation = &impls[0];
        let DeclarationKind::SpecImpl {
            generics: ig,
            target,
            representation,
            members: definitions,
        } = &implementation.kind
        else {
            unreachable!()
        };
        let spec_item = graph
            .items
            .iter()
            .find(|i| i.canonical == *owner)
            .expect("resolved spec");
        let impl_item = graph
            .items
            .iter()
            .find(|i| i.declaration.span == implementation.span)
            .expect("resolved implementation");
        let impl_module = impl_item.module;
        if graph.package_of(spec_item.module) != graph.package_of(impl_item.module) {
            errors.push(
                error(
                    "a spec and its implementation must belong to the same Cargo package",
                    implementation.span,
                )
                .label(spec.span, "spec owned here"),
            );
            continue;
        }
        if implementation.visibility.is_some() || !implementation.attributes.is_empty() {
            errors.push(error(
                "put visibility on the spec and effect promises on methods",
                implementation.span,
            ));
        }
        if generics.len() != ig.len()
            || generics.iter().zip(ig).any(|(a, b)| {
                a.lifetime != b.lifetime
                    || a.bounds.iter().map(Path::text).collect::<Vec<_>>()
                        != b.bounds.iter().map(Path::text).collect::<Vec<_>>()
            })
            || arguments(target).len() != generics.len()
            || arguments(target).iter().zip(ig).any(|(t, g)| {
                type_name(t).as_deref() != Some(&g.name.text) || !arguments(t).is_empty()
            })
        {
            errors.push(error("implementation must cover the complete spec family with the same bounds, in declaration order",target.span).label(spec.span,"family declared here"));
            continue;
        }
        if type_name(representation).as_deref() == Some(owner) {
            errors.push(error(
                "a spec cannot be its own representation",
                representation.span,
            ));
            continue;
        }
        let mut reb = Rewrite::default();
        for (g, i) in generics.iter().zip(ig) {
            reb.types
                .insert(i.name.text.clone(), named(g.name.text.clone(), g.span));
        }
        let mut representation = representation.clone();
        reb.ty(&mut representation);
        let mut definitions = definitions.clone();
        for d in &mut definitions {
            walk::member(&mut reb, d);
        }
        let mut found = BTreeMap::new();
        let mut expected = BTreeSet::new();
        for d in &definitions {
            if let Some(n) = name(d) {
                if let Some(prev) = found.insert(n.text.clone(), d) {
                    errors.push(
                        error(
                            format!("duplicate implementation member `{}`", n.text),
                            d.span,
                        )
                        .label(prev.span, "first definition"),
                    );
                }
                if d.visibility.is_some() {
                    errors.push(error("spec implementation members are implicitly public; put helpers in an ordinary backing-type impl",d.span));
                }
            }
        }
        let mut subst = Rewrite::default();
        for h in members {
            let Some(n) = name(h) else {
                errors.push(error("unsupported spec member", h.span));
                continue;
            };
            if !expected.insert(n.text.clone()) {
                errors.push(error(format!("duplicate spec member `{}`", n.text), h.span));
            }
            let Some(d) = found.get(&n.text) else {
                errors.push(error(
                    format!("spec member `{}` has no implementation", n.text),
                    h.span,
                ));
                continue;
            };
            if let DeclarationKind::AssociatedType { .. } = &h.kind {
                if let DeclarationKind::AssociatedType { value: Some(t), .. } = &d.kind {
                    subst.types.insert(format!("Self::{}", n.text), t.clone());
                    subst
                        .types
                        .insert(format!("{owner}::{}", n.text), t.clone());
                    associated.insert(format!("{owner}::{}", n.text), t.clone());
                } else {
                    errors.push(
                        error("associated type requires a type binding", d.span)
                            .label(h.span, "declared here"),
                    );
                }
            }
        }
        for (n, d) in &found {
            if !expected.contains(n) {
                errors.push(error(
                    format!(
                        "member `{n}` is absent from the spec; put helpers on the backing type"
                    ),
                    d.span,
                ));
            }
        }
        if !generics.is_empty()
            && members
                .iter()
                .any(|m| matches!(m.kind, DeclarationKind::AssociatedType { .. }))
        {
            errors.push(Diagnostic::error("L0510", "associated types in generic spec families are deferred; use the family type parameter directly", spec.span));
            continue;
        }
        for value in subst.types.values() {
            if contains_associated(value, owner) {
                errors.push(Diagnostic::error("L0510", "associated type bindings must be concrete types; chained or recursive associated aliases are deferred", value.span));
            }
        }
        let mut raw = Rewrite {
            types: subst.types.clone(),
            ..Rewrite::default()
        };
        raw.types.insert("Self".into(), representation.clone());
        raw.values.insert("self".into(), "__locus_self".into());
        for h in members {
            if let Some(n) = name(h) {
                let to = if matches!(h.kind, DeclarationKind::Function { .. })
                    && !direct_logic(h, owner)
                {
                    path(
                        format!("__locus_spec_{}_{}", owner.to_lowercase(), n.text),
                        h.span,
                    )
                } else {
                    qualified(owner, &n.text, h.span)
                };
                if let DeclarationKind::Function {
                    self_param: Some(receiver),
                    ..
                } = &h.kind
                {
                    raw.receivers
                        .insert(n.text.clone(), (to.clone(), receiver.kind));
                }
                raw.paths.insert(format!("Self::{}", n.text), to.clone());
            }
        }
        let mut methods = Vec::new();
        for h in members {
            let Some(n) = name(h) else {
                continue;
            };
            let Some(d) = found.get(&n.text) else {
                continue;
            };
            if matches!(h.kind, DeclarationKind::AssociatedType { .. }) {
                continue;
            }
            let mut header = h.clone();
            walk::member(&mut subst, &mut header);
            let mut body = (*d).clone();
            walk::member(&mut subst, &mut body);
            // Compare through the same representation mapping, retaining all evidence.
            let mut rh = header.clone();
            let mut rb = body.clone();
            let mut header_raw = raw.clone();
            header_raw
                .types
                .insert(owner.clone(), representation.clone());
            walk::member(&mut header_raw, &mut rh);
            walk::member(&mut raw, &mut rb);
            if signature(&rh) != signature(&rb) {
                errors.push(
                    error(
                        format!(
                            "implementation signature of `{}` differs from its spec",
                            n.text
                        ),
                        d.span,
                    )
                    .label(h.span, "required signature"),
                );
                continue;
            }
            if body
                .attributes
                .iter()
                .any(|a| matches!(a.kind, AttributeKind::Trusted { .. }))
            {
                errors.push(error(
                    "a manual spec implementation requires a checked body",
                    d.span,
                ));
                continue;
            }
            for a in &h.attributes {
                if !body
                    .attributes
                    .iter()
                    .any(|b| a.kind.name() == b.kind.name())
                {
                    body.attributes.push(a.clone());
                }
            }
            if let DeclarationKind::Function { generics: mg, .. } = &body.kind
                && mg.iter().any(|g| !g.lifetime)
            {
                errors.push(Diagnostic::error(
                    "L0510",
                    "generic methods are deferred; generic spec families are supported",
                    body.span,
                ));
                continue;
            }
            if matches!(body.kind, DeclarationKind::Constant { .. }) {
                body.visibility = public(body.span);
                methods.push(body);
                continue;
            }
            if direct_logic(&body, owner) {
                // Preserve the reviewed logical definition itself, so folding
                // the public name does not expose an unnameable adapter helper.
                let mut public_paths = Rewrite::default();
                for member in members {
                    if let Some(member) = name(member) {
                        public_paths.paths.insert(
                            format!("Self::{}", member.text),
                            qualified(owner, &member.text, member.span),
                        );
                    }
                }
                walk::member(&mut public_paths, &mut body);
                body.visibility = public(body.span);
                methods.push(body);
                continue;
            }
            match adapter(
                owner,
                generics,
                &representation,
                &header,
                &body,
                &mut raw,
                errors,
            ) {
                Some((native, wrapper)) => {
                    // Keep generated helper ownership in the resolved graph.
                    // Otherwise a consumer could copy a dependency helper, or
                    // need its private representation when emitting Rust.
                    graph.items.push(super::resolve::Item {
                        derived_model: false,
                        module: impl_module,
                        canonical: name(&native).expect("function adapter").text.clone(),
                        original: format!("{}::{} implementation", spec_name.text, n.text),
                        namespace: super::resolve::Namespace::Value,
                        declaration: native.clone(),
                    });
                    generated.push(native);
                    methods.push(wrapper);
                }
                None => continue,
            }
        }
        let wrapper = Declaration {
            kind: DeclarationKind::Struct {
                generics: generics.clone(),
                name: spec_name.clone(),
                fields: vec![Field {
                    doc: vec![],
                    visibility: None,
                    name: n(REPR, spec.span),
                    ty: representation,
                    span: spec.span,
                }],
            },
            ..spec.clone()
        };
        if let Some(item) = graph.items.iter_mut().find(|i| i.canonical == *owner) {
            item.declaration = wrapper.clone();
        }
        generated.push(wrapper);
        generated.push(declaration(
            DeclarationKind::Impl {
                generics: generics.clone(),
                target: path(owner, spec.span),
                model: None,
                methods,
            },
            implementation.span,
        ));
    }
    for d in declarations {
        match &d.kind{
  DeclarationKind::Spec{..}|DeclarationKind::SpecImpl{..}=>{},
  DeclarationKind::ModuleImpl{..}=>errors.push(Diagnostic::error("L0510","module spec implementations are deferred",d.span)),
  DeclarationKind::Impl{target,..} if specs.contains_key(&target.text())=>errors.push(error("spec methods belong in the unique `impl Spec for Representation`; extra inherent impls are forbidden",d.span)),
  _=>generated.push(d)
 }
    }
    let mut aliases = Rewrite {
        types: associated,
        ..Rewrite::default()
    };
    for d in &mut generated {
        rewrite_declaration(&mut aliases, d);
    }
    program.declarations = generated;
}
fn display<'a>(g: &'a Graph, name: &'a str) -> &'a str {
    g.items
        .iter()
        .find(|i| i.canonical == name)
        .map_or(name, |i| i.original.as_str())
}
fn rewrite_declaration(w: &mut Rewrite, d: &mut Declaration) {
    match &mut d.kind {
        DeclarationKind::Impl { methods, .. } => {
            for m in methods {
                walk::member(w, m);
            }
        }
        DeclarationKind::Struct { fields, .. } => {
            for f in fields {
                w.ty(&mut f.ty);
            }
        }
        DeclarationKind::Enum { variants, .. } => {
            for v in variants {
                walk::fields(w, &mut v.fields);
            }
        }
        DeclarationKind::Function { .. } | DeclarationKind::Constant { .. } => walk::member(w, d),
        _ => {}
    }
}

fn adapter(
    owner: &str,
    generics: &[GenericParameter],
    representation: &Type,
    header: &Declaration,
    body: &Declaration,
    raw: &mut Rewrite,
    errors: &mut Vec<Diagnostic>,
) -> Option<(Declaration, Declaration)> {
    let mut native = body.clone();
    walk::member(raw, &mut native);
    let DeclarationKind::Function {
        name: raw_name,
        self_param,
        parameters,
        generics: raw_generics,
        ..
    } = &mut native.kind
    else {
        return None;
    };
    raw_name.text = format!("__locus_spec_{}_{}", owner.to_lowercase(), raw_name.text);
    raw_generics.splice(0..0, generics.iter().cloned());
    if let Some(receiver) = self_param.take() {
        let ty = match receiver.kind {
            SelfKind::Value | SelfKind::MutValue => representation.clone(),
            SelfKind::Ref | SelfKind::RefMut => Type {
                span: receiver.span,
                kind: TypeKind::Ref {
                    lifetime: None,
                    mutable: receiver.kind == SelfKind::RefMut,
                    inner: Box::new(representation.clone()),
                },
            },
        };
        parameters.insert(
            0,
            Parameter {
                name: n("__locus_self", receiver.span),
                mutable: receiver.kind == SelfKind::MutValue,
                ty,
                span: receiver.span,
            },
        );
    }
    native.visibility = None;
    let mut wrapper = header.clone();
    wrapper.visibility = public(header.span);
    wrapper.attributes = body.attributes.clone();
    let DeclarationKind::Function {
        self_param,
        parameters,
        result,
        body: wrapper_body,
        ..
    } = &mut wrapper.kind
    else {
        return None;
    };
    let mut args = Vec::new();
    if let Some(receiver) = self_param {
        let mut e = Expr {
            span: receiver.span,
            kind: ExprKind::Member {
                value: Box::new(var("self", receiver.span)),
                name: n(REPR, receiver.span),
            },
        };
        if matches!(receiver.kind, SelfKind::Ref | SelfKind::RefMut) {
            e = Expr {
                span: receiver.span,
                kind: ExprKind::Ref {
                    mutable: receiver.kind == SelfKind::RefMut,
                    expr: Box::new(e),
                },
            };
        }
        args.push(e);
    }
    for p in parameters {
        args.push(unwrap_argument(
            var(&p.name.text, p.span),
            &p.ty,
            owner,
            errors,
        )?);
    }
    let raw_name = name(&native)?.text.clone();
    let mut callee = var(raw_name, body.span);
    if !generics.is_empty() {
        callee = Expr {
            span: body.span,
            kind: ExprKind::GenericApply {
                callee: Box::new(callee),
                arguments: generics
                    .iter()
                    .map(|g| named(&g.name.text, g.span))
                    .collect(),
            },
        };
    }
    let call = Expr {
        span: body.span,
        kind: ExprKind::Call {
            callee: Box::new(callee),
            arguments: args,
        },
    };
    let mut serial = 0;
    *wrapper_body = block(wrap_result(call, result, owner, errors, &mut serial)?);
    Some((native, wrapper))
}
fn is_self(t: &Type, owner: &str) -> bool {
    type_name(t).is_some_and(|s| s == "Self" || s == owner)
}
fn mentions_self(t: &Type, owner: &str) -> bool {
    if is_self(t, owner) {
        return true;
    }
    match &t.kind {
        TypeKind::Ref { inner, .. } | TypeKind::Group(inner) | TypeKind::Slice(inner) => {
            mentions_self(inner, owner)
        }
        TypeKind::Array { element, .. } => mentions_self(element, owner),
        TypeKind::Path { arguments, .. } => arguments.iter().any(|t| mentions_self(t, owner)),
        TypeKind::Tuple(fs) => fs.iter().any(|f| mentions_self(&f.ty, owner)),
        TypeKind::Function { parameters, result }
        | TypeKind::LogicalFunction { parameters, result } => {
            parameters.iter().any(|f| mentions_self(&f.ty, owner)) || mentions_self(result, owner)
        }
        _ => false,
    }
}
fn unwrap_argument(e: Expr, t: &Type, owner: &str, errors: &mut Vec<Diagnostic>) -> Option<Expr> {
    if is_self(t, owner) {
        return Some(Expr {
            span: e.span,
            kind: ExprKind::Member {
                value: Box::new(e),
                name: n(REPR, t.span),
            },
        });
    }
    if let TypeKind::Group(inner) = &t.kind {
        return unwrap_argument(e, inner, owner, errors);
    }
    if let TypeKind::Ref { mutable, inner, .. } = &t.kind
        && is_self(inner, owner)
    {
        return Some(Expr {
            span: e.span,
            kind: ExprKind::Ref {
                mutable: *mutable,
                expr: Box::new(Expr {
                    span: e.span,
                    kind: ExprKind::Member {
                        value: Box::new(e),
                        name: n(REPR, t.span),
                    },
                }),
            },
        });
    }
    if mentions_self(t, owner) {
        errors.push(Diagnostic::error(
            "L0510",
            "Self nested in an input container is not supported by spec adapters",
            t.span,
        ));
        return None;
    }
    if let TypeKind::Ref { mutable, .. } = &t.kind {
        return Some(Expr {
            span: e.span,
            kind: ExprKind::Ref {
                mutable: *mutable,
                expr: Box::new(e),
            },
        });
    }
    Some(e)
}
fn wrap_result(
    e: Expr,
    t: &Type,
    owner: &str,
    errors: &mut Vec<Diagnostic>,
    serial: &mut usize,
) -> Option<Expr> {
    if is_self(t, owner) {
        return Some(Expr {
            span: t.span,
            kind: ExprKind::Struct {
                path: path(owner, t.span),
                fields: vec![ValueField {
                    name: Some(n(REPR, t.span)),
                    span: t.span,
                    value: e,
                }],
            },
        });
    }
    if let TypeKind::Group(inner) = &t.kind {
        return wrap_result(e, inner, owner, errors, serial);
    }
    if let TypeKind::Tuple(fs) = &t.kind
        && fs.iter().any(|f| mentions_self(&f.ty, owner))
    {
        let mut ps = Vec::new();
        let mut es = Vec::new();
        for f in fs {
            let name = n(format!("__locus_result_{}", *serial), f.span);
            *serial += 1;
            ps.push(Pattern {
                kind: PatternKind::Name {
                    name: name.clone(),
                    mutable: false,
                },
                span: f.span,
            });
            es.push(wrap_result(
                var(&name.text, f.span),
                &f.ty,
                owner,
                errors,
                serial,
            )?);
        }
        return Some(Expr {
            span: e.span,
            kind: ExprKind::Block(Block {
                span: e.span,
                statements: vec![Statement {
                    span: e.span,
                    kind: StatementKind::Let {
                        mutable: false,
                        pattern: Pattern {
                            kind: PatternKind::Tuple(ps),
                            span: e.span,
                        },
                        annotation: None,
                        value: e,
                    },
                }],
                tail: Some(Box::new(Expr {
                    kind: ExprKind::Tuple(es),
                    span: t.span,
                })),
            }),
        });
    }
    if mentions_self(t, owner) {
        errors.push(Diagnostic::error("L0510","borrowed or container Self results need an explicit checked adapter; automatic casts or allocation are not supported",t.span));
        return None;
    }
    Some(e)
}

fn contains_associated(ty: &Type, owner: &str) -> bool {
    struct Inspect<'a> {
        owner: &'a str,
        found: bool,
    }
    impl Walk for Inspect<'_> {
        fn ty(&mut self, t: &mut Type) {
            if type_name(t).is_some_and(|s| {
                s == "Self"
                    || s.starts_with("Self::")
                    || s.starts_with(&format!("{}::", self.owner))
            }) {
                self.found = true;
            }
            walk::ty(self, t);
        }
    }
    let mut inspect = Inspect {
        owner,
        found: false,
    };
    inspect.ty(&mut ty.clone());
    inspect.found
}

fn direct_logic(d: &Declaration, owner: &str) -> bool {
    matches!(&d.kind, DeclarationKind::Function{logical:true,self_param:None,parameters,result,..} if parameters.iter().all(|p|!mentions_self(&p.ty,owner)) && !mentions_self(result,owner))
}

fn check_self_family(
    member: &Declaration,
    owner: &str,
    generics: &[GenericParameter],
    errors: &mut Vec<Diagnostic>,
) {
    struct Check<'a> {
        owner: &'a str,
        generics: &'a [GenericParameter],
        errors: &'a mut Vec<Diagnostic>,
    }
    impl Walk for Check<'_> {
        fn ty(&mut self, t: &mut Type) {
            if type_name(t).as_deref() == Some(self.owner) {
                let args = arguments(t);
                if args.len() != self.generics.len()
                    || args.iter().zip(self.generics).any(|(t, g)| {
                        type_name(t).as_deref() != Some(&g.name.text) || !arguments(t).is_empty()
                    })
                {
                    self.errors.push(Diagnostic::error("L0510","a spec signature may mention Self or its current family; cross-specialization Self adapters are deferred",t.span));
                }
            }
            walk::ty(self, t);
        }
    }
    let mut c = Check {
        owner,
        generics,
        errors,
    };
    walk::member(&mut c, &mut member.clone());
}
