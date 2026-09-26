//! Obligations are source-level requirements, never kernel axioms. Each selected
//! instance still passes ordinary elaboration, ownership and proof checking.
use super::*;

pub(super) fn requirements(d: &Declaration) -> Vec<WherePredicate> {
    let mut result = d.constraints.clone();
    for p in all_parameters(d)
        .iter()
        .filter(|p| !p.lifetime && !p.bounds.is_empty())
    {
        result.push(WherePredicate {
            subject: named_type(&p.name),
            bounds: p.bounds.clone(),
            span: p.span,
        });
    }
    result
}

impl Specializer<'_> {
    pub(super) fn validate_bound(&mut self, b: &GenericBound) {
        if b.text() == "Logical" {
            if !b.associated.is_empty() {
                self.error("Logical has no associated types", b.span);
            }
            return;
        }
        let definition = self
            .access
            .as_ref()
            .and_then(|a| a.trait_registry.definitions.get(&b.text()))
            .cloned();
        let Some(Declaration {
            kind: DeclarationKind::Trait { members, .. },
            ..
        }) = definition
        else {
            self.error(
                format!("unknown or unsupported trait bound `{}`", b.text()),
                b.span,
            );
            return;
        };
        let mut seen = HashSet::new();
        for (n, _) in &b.associated {
            if !seen.insert(&n.text) {
                self.error(
                    format!("associated type `{}` is constrained twice", n.text),
                    n.span,
                );
            }
            if !members.iter().any(|m| matches!(&m.kind,DeclarationKind::AssociatedType {name,..} if name.text == n.text)) {
                self.error(format!("trait `{}` has no associated type `{}`",b.text(),n.text),n.span);
            }
        }
    }

    pub(super) fn requirements(
        &mut self,
        d: &Declaration,
        substitutions: &Types,
        locals: &Types,
        report: bool,
    ) -> bool {
        let mut valid = true;
        for p in requirements(d) {
            let mut subject = p.subject;
            self.ty(&mut subject, substitutions, locals);
            for bound in p.bounds {
                if !self.satisfies(&subject, &bound, substitutions, locals) {
                    valid = false;
                    if report {
                        self.diagnostics.push(Diagnostic::error("L0281",format!("type `{}` does not satisfy trait bound `{}`",display(&subject),bound.text()),subject.span)
                            .label(bound.span,"required by this bound")
                            .note("supply a type with a matching implementation and associated types; each concrete implementation is checked before use"));
                    }
                }
            }
        }
        valid
    }

    fn implementation(
        &mut self,
        subject: &Type,
        interface: &str,
        locals: &Types,
    ) -> Option<(crate::project::traits::Implementation, Types)> {
        let mut nominal = subject;
        while let TypeKind::Group(inner) = &nominal.kind {
            nominal = inner;
        }
        let owner = match &nominal.kind {
            TypeKind::Named(n) => n.text.clone(),
            TypeKind::Path { path, arguments } if arguments.is_empty() => path.text(),
            _ => return None,
        };
        let (family, args) = self
            .origins
            .get(&owner)
            .cloned()
            .unwrap_or((owner.clone(), Vec::new()));
        let implementation = self
            .access
            .as_ref()?
            .trait_registry
            .implementations
            .iter()
            .find(|i| i.interface == interface && i.owner == family)?
            .clone();
        if implementation.generics.len() != args.len() {
            return None;
        }
        let mut bindings: Types = implementation
            .generics
            .iter()
            .zip(args)
            .map(|(p, a)| (p.name.text.clone(), a))
            .collect();
        bindings.insert("Self".into(), subject.clone());
        let dummy = Declaration {
            constraints: implementation.constraints.clone(),
            captures: vec![],
            doc: vec![],
            attributes: vec![],
            visibility: None,
            span: implementation.span,
            kind: DeclarationKind::Impl {
                generics: implementation.generics.clone(),
                model: None,
                target: Path {
                    segments: vec![Name {
                        text: family,
                        span: subject.span,
                    }],
                    span: subject.span,
                },
                methods: vec![],
            },
        };
        if !self.requirements(&dummy, &bindings, locals, false) {
            return None;
        }
        Some((implementation, bindings))
    }

    fn satisfies(
        &mut self,
        subject: &Type,
        bound: &GenericBound,
        substitutions: &Types,
        locals: &Types,
    ) -> bool {
        if bound.text() == "Logical" {
            return bound.associated.is_empty() && self.logical(subject);
        }
        let key = format!("{}:{}", type_key(subject), bound.text());
        if self.obligations.len() >= MAX_GENERIC_TYPE_DEPTH || self.obligations.contains(&key) {
            return false;
        }
        self.obligations.push(key);
        let result = if let Some((implementation, bindings)) =
            self.implementation(subject, &bound.text(), locals)
        {
            bound.associated.iter().all(|(name, expected)| {
                let Some(mut actual) = implementation.associated.get(&name.text).cloned() else {
                    return false;
                };
                self.ty(&mut actual, &bindings, locals);
                let mut expected = expected.clone();
                self.ty(&mut expected, substitutions, locals);
                type_key(&actual) == type_key(&expected)
            })
        } else {
            false
        };
        self.obligations.pop();
        result
    }

    pub(super) fn projection(
        &mut self,
        ty: &Type,
        substitutions: &Types,
        locals: &Types,
    ) -> Option<Type> {
        let TypeKind::Path { path, arguments } = &ty.kind else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        let [marker, owner, interface, member] = path.segments.as_slice() else {
            return None;
        };
        if marker.text != "<qualified>" {
            return None;
        }
        let mut subject = substitutions
            .get(&owner.text)
            .cloned()
            .unwrap_or_else(|| named_type(owner));
        self.ty(&mut subject, substitutions, locals);
        let key = format!(
            "projection:{}:{}:{}",
            type_key(&subject),
            interface.text,
            member.text
        );
        if self.obligations.contains(&key) {
            self.error("cyclic associated type projection", ty.span);
            return Some(ty.clone());
        }
        self.obligations.push(key);
        let result = self
            .implementation(&subject, &interface.text, locals)
            .and_then(|(i, b)| {
                let mut t = i.associated.get(&member.text)?.clone();
                self.ty(&mut t, &b, locals);
                Some(t)
            });
        self.obligations.pop();
        if result.is_none() {
            self.error(
                format!(
                    "cannot resolve associated type `<{} as {}>::{}`",
                    display(&subject),
                    interface.text,
                    member.text
                ),
                ty.span,
            );
        }
        result
    }
}

pub(super) fn display(t: &Type) -> String {
    match &t.kind {
        TypeKind::Named(n) | TypeKind::Scoped { name: n, .. } => n.text.clone(),
        TypeKind::Path { path, arguments } if arguments.is_empty() => path.text(),
        TypeKind::Path { path, arguments } => format!(
            "{}<{}>",
            path.text(),
            arguments.iter().map(display).collect::<Vec<_>>().join(", ")
        ),
        TypeKind::Group(t) => display(t),
        TypeKind::Ref { inner, mutable, .. } => {
            format!("&{}{}", if *mutable { "mut " } else { "" }, display(inner))
        }
        _ => "this type".into(),
    }
}

impl Specializer<'_> {
    /// Logical associated requirements remain checked even if no method is used.
    /// The generated identity is ordinary logic code, not a trusted assertion.
    pub(super) fn associated_obligations(
        &mut self,
        family: &str,
        instance: &str,
        locals: &Types,
        span: Span,
    ) {
        let entries = self
            .access
            .as_ref()
            .map(|a| {
                a.trait_registry
                    .implementations
                    .iter()
                    .filter(|i| i.owner == family)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let subject = named_type(&Name {
            text: instance.into(),
            span,
        });
        for entry in entries {
            let Some((implementation, bindings)) =
                self.implementation(&subject, &entry.interface, locals)
            else {
                continue;
            };
            let Some(Declaration {
                kind: DeclarationKind::Trait { members, .. },
                ..
            }) = self
                .access
                .as_ref()
                .and_then(|a| a.trait_registry.definitions.get(&entry.interface))
                .cloned()
            else {
                continue;
            };
            for member in members {
                let DeclarationKind::AssociatedType {
                    name,
                    logical: true,
                    ..
                } = &member.kind
                else {
                    continue;
                };
                let Some(mut ty) = implementation.associated.get(&name.text).cloned() else {
                    continue;
                };
                self.ty(&mut ty, &bindings, locals);
                let name = Name {
                    text: format!("__locus_associated_{}_{}", instance, name.text),
                    span: member.span,
                };
                let value = Name {
                    text: "value".into(),
                    span: member.span,
                };
                let d = Declaration {
                    constraints: vec![],
                    captures: vec![],
                    doc: vec![],
                    attributes: vec![],
                    visibility: None,
                    span: member.span,
                    kind: DeclarationKind::Function {
                        logical: true,
                        generics: vec![],
                        name,
                        self_param: None,
                        parameters: vec![Parameter {
                            mutable: false,
                            name: value.clone(),
                            ty: ty.clone(),
                            span: member.span,
                        }],
                        result: ty,
                        body: Block {
                            statements: vec![],
                            tail: Some(Box::new(Expr {
                                kind: ExprKind::Name(value),
                                span: member.span,
                            })),
                            span: member.span,
                        },
                    },
                };
                self.queue.push_back((d, Types::new()));
            }
        }
    }
}
