//! Concrete trait selection. This pass creates ordinary checked methods; it
//! does not grant the trait's contracts as axioms or bypass body checking.
use super::{
    Graph,
    specs::{
        self,
        walk::{self, Walk},
    },
};
use crate::{ast::*, diagnostic::Diagnostic, source::Span};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Method {
    pub owner: String,
    pub interface: String,
    pub name: String,
    pub lowered: String,
    pub scopes: BTreeSet<usize>,
}
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub definitions: BTreeMap<String, Declaration>,
    pub implementations: Vec<Implementation>,
}
#[derive(Clone, Debug)]
pub struct Implementation {
    pub generics: Vec<GenericParameter>,
    pub constraints: Vec<WherePredicate>,
    pub interface: String,
    pub owner: String,
    pub span: Span,
    pub members: Vec<Declaration>,
    pub associated: BTreeMap<String, Type>,
}
fn error(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic::error("L0515", message, span)
}
fn name(d: &Declaration) -> Option<&Name> {
    match &d.kind {
        DeclarationKind::Function { name, .. }
        | DeclarationKind::Constant { name, .. }
        | DeclarationKind::AssociatedType { name, .. } => Some(name),
        _ => None,
    }
}
fn name_mut(d: &mut Declaration) -> Option<&mut Name> {
    match &mut d.kind {
        DeclarationKind::Function { name, .. }
        | DeclarationKind::Constant { name, .. }
        | DeclarationKind::AssociatedType { name, .. } => Some(name),
        _ => None,
    }
}
fn type_name(t: &Type) -> Option<String> {
    match &t.kind {
        TypeKind::Named(n) => Some(n.text.clone()),
        TypeKind::Path { path, arguments } if arguments.is_empty() => Some(path.text()),
        TypeKind::Group(t) => type_name(t),
        _ => None,
    }
}
fn path(owner: &str, member: &str, span: Span) -> Path {
    Path {
        segments: [owner, member]
            .into_iter()
            .map(|s| Name {
                text: s.into(),
                span,
            })
            .collect(),
        span,
    }
}
pub(crate) fn lowered(interface: &str, member: &str) -> String {
    // Case-folding alone merges distinct Rust/Locus identifiers. Encode the
    // canonical identity injectively while keeping generated methods snake_case.
    let identity = interface
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("__locus_trait_{identity}_{member}")
}

/// Substitute only explicit Self paths and self receivers. Other receivers
/// retain type-directed method selection at elaboration.
struct Substitute<'a> {
    owner: &'a str,
    self_type: &'a Type,
    interface: &'a str,
    associated: &'a BTreeMap<String, Type>,
    names: &'a BTreeSet<String>,
}
impl Walk for Substitute<'_> {
    fn ty(&mut self, t: &mut Type) {
        if let Some(key) = type_name(t) {
            if key == "Self" {
                *t = self.self_type.clone();
                return;
            }
            if let Some(key) = key.strip_prefix("Self::")
                && let Some(to) = self.associated.get(key)
            {
                *t = to.clone();
                return;
            }
        }
        if let TypeKind::Path { path, arguments } = &t.kind
            && arguments.is_empty()
            && let [marker, owner, interface, member] = path.segments.as_slice()
            && marker.text == "<qualified>"
            && (owner.text == "Self" || owner.text == self.owner)
            && interface.text == self.interface
            && let Some(to) = self.associated.get(&member.text)
        {
            *t = to.clone();
            return;
        }
        walk::ty(self, t);
    }
    fn path(&mut self, p: &mut Path) {
        if let [marker, owner, interface, member] = p.segments.as_mut_slice()
            && marker.text == "<qualified>"
        {
            if owner.text == "Self" {
                owner.text = self.owner.into();
            }
            if owner.text == self.owner
                && interface.text == self.interface
                && self.names.contains(&member.text)
            {
                *p = path(self.owner, &lowered(self.interface, &member.text), p.span);
            }
            return;
        }
        if let [owner, member] = p.segments.as_mut_slice()
            && owner.text == "Self"
        {
            owner.text = self.owner.into();
            if self.names.contains(&member.text) {
                member.text = lowered(self.interface, &member.text);
            }
        } else if let [owner] = p.segments.as_mut_slice()
            && owner.text == "Self"
        {
            owner.text = self.owner.into();
        }
    }
    fn expr(&mut self, e: &mut Expr) {
        if let ExprKind::Member { value, name } = &mut e.kind
            && matches!(&value.kind, ExprKind::Name(n) if n.text == "self")
            && self.names.contains(&name.text)
        {
            name.text = lowered(self.interface, &name.text);
        }
        walk::expr(self, e);
    }
}

pub fn lower(program: &mut Program, graph: &mut Graph, errors: &mut Vec<Diagnostic>) {
    let declarations = std::mem::take(&mut program.declarations);
    let definitions: BTreeMap<_, _> = declarations
        .iter()
        .filter_map(|d| {
            if let DeclarationKind::Trait { name, .. } = &d.kind {
                Some((name.text.clone(), d.clone()))
            } else {
                None
            }
        })
        .collect();
    if definitions.is_empty() {
        program.declarations = declarations;
        return;
    }
    let mut generated = Vec::new();
    let mut instances = BTreeMap::new();
    for (key, d) in &definitions {
        let DeclarationKind::Trait { members, .. } = &d.kind else {
            unreachable!()
        };
        if !d.attributes.is_empty() {
            errors.push(error(
                "put promises on trait methods, not on the trait",
                d.span,
            ));
        }
        if matches!(d.kind, DeclarationKind::Trait { native: None, .. })
            && graph.items.iter().any(|i| {
                i.canonical == *key
                    && matches!(
                        i.original.as_str(),
                        "Logical" | "Model" | "Copy" | "Clone" | "Debug" | "PartialEq" | "Eq"
                    )
            })
        {
            errors.push(error(
                "compiler-integrated trait names cannot be redeclared",
                d.span,
            ));
        }
        let mut seen = BTreeSet::new();
        for m in members {
            if let Some(n) = name(m)
                && !seen.insert(n.text.clone())
            {
                errors.push(error(
                    format!("duplicate trait member `{}`", n.text),
                    n.span,
                ));
            }
            if matches!(&m.kind,DeclarationKind::Function { generics, .. } if !generics.is_empty())
            {
                errors.push(error("generic trait methods are deferred", m.span));
            }
        }
    }
    for d in &declarations {
        let DeclarationKind::SpecImpl {
            generics,
            target,
            representation,
            members: supplied,
        } = &d.kind
        else {
            continue;
        };
        let Some(interface) = type_name(target) else {
            continue;
        };
        let Some(definition) = definitions.get(&interface) else {
            continue;
        };
        let DeclarationKind::Trait {
            members, required, ..
        } = &definition.kind
        else {
            unreachable!()
        };
        let owner_name = match &representation.kind {
            TypeKind::Path { path, arguments } if !generics.is_empty() => {
                if arguments.len() != generics.len()
                    || arguments.iter().zip(generics).any(
                        |(t, g)| !matches!(&t.kind,TypeKind::Named(n) if n.text == g.name.text),
                    )
                {
                    errors.push(error("a generic trait implementation must cover one complete named family: `impl<T> Trait for Name<T>`", representation.span));
                    continue;
                }
                Some(path.text())
            }
            _ => type_name(representation),
        };
        let Some(owner) = owner_name else {
            errors.push(error(
                "trait implementations currently require a concrete named type",
                representation.span,
            ));
            continue;
        };
        let type_item = graph.items.iter().find(|i| i.canonical == owner);
        let Some(type_item) = type_item.filter(|i| {
            matches!(
                i.declaration.kind,
                DeclarationKind::Struct { .. }
                    | DeclarationKind::Enum { .. }
                    | DeclarationKind::Spec { .. }
            )
        }) else {
            errors.push(error(
                "implement this trait for a concrete Locus struct, enum or spec type",
                representation.span,
            ));
            continue;
        };
        let trait_item = graph
            .items
            .iter()
            .find(|i| i.canonical == interface)
            .unwrap();
        let impl_item = graph
            .items
            .iter()
            .find(|i| i.declaration.span == d.span)
            .unwrap();
        let package = graph.package_of(impl_item.module);
        if package != graph.package_of(type_item.module)
            && (matches!(&definition.kind, DeclarationKind::Trait {native:Some(f),..} if !f.path.starts_with("crate::"))
                || package != graph.package_of(trait_item.module))
        {
            errors.push(error("a trait implementation must own either the trait or the implementing type in this Cargo package",d.span));
        }
        if package != graph.package_of(type_item.module) {
            errors.push(error("implementing traits for another Locus package's runtime types needs cross-package method ABI support, which is deferred",representation.span));
            continue;
        }
        if let Some(previous) = instances.insert((interface.clone(), owner.clone()), d.span) {
            errors.push(
                error(
                    "conflicting implementations of the same trait for this type",
                    d.span,
                )
                .label(previous, "first implementation"),
            );
            continue;
        }
        let names: BTreeSet<_> = members
            .iter()
            .filter_map(name)
            .map(|n| n.text.clone())
            .collect();
        let mut provided = BTreeMap::new();
        for m in supplied {
            let Some(n) = name(m) else {
                errors.push(error("unsupported trait implementation member", m.span));
                continue;
            };
            if m.visibility.is_some() {
                errors.push(error("trait implementation members inherit visibility; put helpers in an inherent impl",m.span));
            }
            if !names.contains(&n.text) {
                errors.push(error(
                    format!("`{}` is not a member of this trait", n.text),
                    n.span,
                ));
            }
            if let Some(old) = provided.insert(n.text.clone(), m) {
                errors.push(
                    error(
                        format!("duplicate implementation member `{}`", n.text),
                        m.span,
                    )
                    .label(old.span, "first member"),
                );
            }
            if m.attributes
                .iter()
                .any(|a| matches!(a.kind, AttributeKind::Trusted { .. }))
            {
                errors.push(error(
                    "a native trusted binding cannot satisfy a checked trait implementation",
                    m.span,
                ));
            }
        }
        let mut associated = BTreeMap::new();
        for header in members {
            let n = name(header).unwrap();
            if let DeclarationKind::AssociatedType { .. } = &header.kind {
                match provided.get(&n.text).map(|d| &d.kind) {
                    Some(DeclarationKind::AssociatedType { value: Some(t), .. }) => {
                        associated.insert(n.text.clone(), t.clone());
                    }
                    _ => errors.push(
                        error(format!("missing associated type `{}`", n.text), d.span)
                            .label(header.span, "required here"),
                    ),
                }
            }
        }
        // Alias cycles and associated aliases require normalization that this
        // slice deliberately does not claim to implement.
        struct RejectSelf<'a>(&'a mut Vec<Diagnostic>);
        impl Walk for RejectSelf<'_> {
            fn ty(&mut self, t: &mut Type) {
                if type_name(t).is_some_and(|s| s.starts_with("Self::")) {
                    self.0.push(error(
                        "chained associated type bindings are deferred",
                        t.span,
                    ));
                }
                walk::ty(self, t);
            }
        }
        for t in associated.values_mut() {
            RejectSelf(errors).ty(t);
        }
        let mut methods = Vec::new();
        for h in members {
            if let DeclarationKind::AssociatedType {
                name,
                logical: true,
                ..
            } = &h.kind
                && let Some(ty) = associated.get(&name.text)
                && generics.is_empty()
            {
                let parameter = Name {
                    text: "value".into(),
                    span: h.span,
                };
                generated.push(Declaration {
                    kind: DeclarationKind::Function {
                        logical: true,
                        generics: vec![],
                        name: Name {
                            text: format!("{}_{}_logical", lowered(&interface, &name.text), owner),
                            span: h.span,
                        },
                        self_param: None,
                        parameters: vec![Parameter {
                            name: parameter.clone(),
                            mutable: false,
                            ty: ty.clone(),
                            span: h.span,
                        }],
                        result: ty.clone(),
                        body: Block {
                            statements: vec![],
                            tail: Some(Box::new(Expr {
                                kind: ExprKind::Name(parameter),
                                span: h.span,
                            })),
                            span: h.span,
                        },
                    },
                    attributes: vec![],
                    visibility: None,
                    ..h.clone()
                });
            }
        }
        for header in members {
            let n = name(header).unwrap();
            if matches!(header.kind, DeclarationKind::AssociatedType { .. }) {
                continue;
            }
            let selected = match provided.get(&n.text) {
                Some(m) => (*m).clone(),
                None if !required.contains(&n.text) => header.clone(),
                None => {
                    errors.push(
                        error(format!("missing implementation of `{}`", n.text), d.span)
                            .label(header.span, "required trait member"),
                    );
                    continue;
                }
            };
            let mut expected = header.clone();
            let mut actual = selected;
            let mut substitution = Substitute {
                owner: if generics.is_empty() { &owner } else { "Self" },
                self_type: representation,
                interface: &interface,
                associated: &associated,
                names: &names,
            };
            walk::member(&mut substitution, &mut expected);
            walk::member(&mut substitution, &mut actual);
            if specs::signature(&expected) != specs::signature(&actual)
                || constraints(&expected) != constraints(&actual)
            {
                errors.push(
                    error(
                        format!(
                            "implementation of `{}` does not match its trait signature",
                            n.text
                        ),
                        actual.span,
                    )
                    .label(
                        header.span,
                        "required signature, including receiver, logical mode and proof slots",
                    ),
                );
                continue;
            }
            // Inherited promises are obligations of this actual body.
            for a in &header.attributes {
                if a.kind.is_promise()
                    && !actual
                        .attributes
                        .iter()
                        .any(|b| b.kind.name() == a.kind.name())
                {
                    actual.attributes.push(a.clone());
                }
            }
            let internal = lowered(&interface, &n.text);
            name_mut(&mut actual).unwrap().text = internal.clone();
            actual.visibility = Some(Visibility {
                scope: VisibilityScope::Public,
                span: actual.span,
            });
            let scopes = (0..graph.scopes.len())
                .filter(|m| graph.traits_in_scope(*m).contains(&interface))
                .collect();
            graph.access.traits.push(Method {
                owner: owner.clone(),
                interface: interface.clone(),
                name: n.text.clone(),
                lowered: internal,
                scopes,
            });
            methods.push(actual);
        }
        graph.traits.implementations.push(Implementation {
            generics: generics.clone(),
            constraints: d.constraints.clone(),
            interface: interface.clone(),
            owner: owner.clone(),
            span: d.span,
            members: methods.clone(),
            associated,
        });
        generated.push(Declaration {
            kind: DeclarationKind::Impl {
                generics: generics.clone(),
                target: Path {
                    segments: vec![Name {
                        text: owner,
                        span: representation.span,
                    }],
                    span: representation.span,
                },
                model: None,
                methods,
            },
            ..d.clone()
        });
    }
    graph.traits.definitions = definitions;
    program.declarations = declarations
        .into_iter()
        .filter(|d| match &d.kind {
            DeclarationKind::Trait { .. } => false,
            DeclarationKind::SpecImpl { target, .. } => {
                !type_name(target).is_some_and(|s| graph.traits.definitions.contains_key(&s))
            }
            _ => true,
        })
        .chain(generated)
        .collect();
    // Explicit qualified names resolve without depending on which traits happen
    // to be in scope. No fallback from a missing implementation is permitted.
    let inherent: BTreeSet<String> = program
        .declarations
        .iter()
        .filter_map(|d| {
            if let DeclarationKind::Impl {
                target, methods, ..
            } = &d.kind
            {
                Some(
                    methods
                        .iter()
                        .filter_map(name)
                        .map(|n| format!("{}::{}", target.text(), n.text))
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        })
        .flatten()
        .collect();
    struct Qualified<'a> {
        registry: &'a Registry,
        abstract_types: BTreeSet<String>,
        access: &'a super::Access,
        inherent: &'a BTreeSet<String>,
        errors: &'a mut Vec<Diagnostic>,
    }
    impl Walk for Qualified<'_> {
        fn path(&mut self, p: &mut Path) {
            if p.segments.first().is_none_or(|n| n.text != "<qualified>") {
                if let Some((owner, member)) = p.pair()
                    && !self.inherent.contains(&p.text())
                {
                    let module = self.access.module_at(p.span);
                    let candidates: Vec<_> = self
                        .access
                        .traits
                        .iter()
                        .filter(|m| {
                            m.owner == owner.text
                                && m.name == member.text
                                && m.scopes.contains(&module)
                        })
                        .collect();
                    match candidates.as_slice() {
                        [m] => {
                            *p = path(&m.owner, &m.lowered, p.span);
                        }
                        [] => {}
                        _ => self.errors.push(
                            Diagnostic::error(
                                "L0516",
                                "multiple traits supply this associated item",
                                p.span,
                            )
                            .note("select with `<Type as Trait>::member`"),
                        ),
                    }
                }
                return;
            }
            let owner = &p.segments[1].text;
            if self.abstract_types.contains(owner) {
                return;
            }
            let interface = &p.segments[2].text;
            let member = &p.segments[3].text;
            let Some(implementation) = self
                .registry
                .implementations
                .iter()
                .find(|i| i.owner == *owner && i.interface == *interface)
            else {
                self.errors.push(error(
                    "no implementation for this qualified trait selection",
                    p.span,
                ));
                return;
            };
            if implementation
                .members
                .iter()
                .all(|m| name(m).is_none_or(|n| n.text != lowered(interface, member)))
            {
                self.errors.push(error(
                    format!("trait has no callable/constant member `{member}`"),
                    p.span,
                ));
                return;
            }
            *p = path(owner, &lowered(interface, member), p.span);
        }
        fn ty(&mut self, t: &mut Type) {
            if let TypeKind::Path { path: p, .. } = &t.kind
                && p.segments.first().is_some_and(|n| n.text == "<qualified>")
            {
                if self.abstract_types.contains(&p.segments[1].text) {
                    return;
                }
                let found = self
                    .registry
                    .implementations
                    .iter()
                    .find(|i| i.owner == p.segments[1].text && i.interface == p.segments[2].text)
                    .and_then(|i| i.associated.get(&p.segments[3].text));
                if let Some(found) = found {
                    *t = found.clone();
                } else {
                    self.errors
                        .push(error("unknown qualified associated type", t.span));
                }
                return;
            }
            if type_name(t).is_some_and(|n| self.registry.definitions.contains_key(&n)) {
                self.errors.push(error("a trait is not a concrete type; use an implementing type (generic bounds and dyn are deferred)",t.span));
            }
            walk::ty(self, t);
        }
    }
    let mut rewrite = Qualified {
        registry: &graph.traits,
        abstract_types: BTreeSet::new(),
        access: &graph.access,
        inherent: &inherent,
        errors,
    };
    for d in &mut program.declarations {
        rewrite.abstract_types = match &d.kind {
            DeclarationKind::Function { generics, .. }
            | DeclarationKind::Struct { generics, .. }
            | DeclarationKind::Enum { generics, .. }
            | DeclarationKind::Prop { generics, .. }
            | DeclarationKind::Impl { generics, .. } => {
                generics.iter().map(|g| g.name.text.clone()).collect()
            }
            _ => BTreeSet::new(),
        };
        if let DeclarationKind::Impl { methods, .. } = &mut d.kind {
            for m in methods {
                walk::member(&mut rewrite, m);
            }
        } else {
            walk::member(&mut rewrite, d);
        }
    }
}

fn constraints(d: &Declaration) -> Vec<WherePredicate> {
    struct Clear;
    impl Walk for Clear {
        fn span(&mut self, s: &mut Span) {
            *s = Span::new(crate::source::FileId(0), 0, 0);
        }
    }
    let mut d = d.clone();
    walk::member(&mut Clear, &mut d);
    d.constraints
}
