//! Complete concrete specs with checked local definitions before elaboration.
//! Headers are never turned into kernel declarations or native assumptions.
use crate::{
    ast::*,
    diagnostic::Diagnostic,
    lexer,
    source::{SourceFile, Span},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(super) struct Guard {
    pub name: String,
    pub representation: Span,
    pub implementation: Span,
    pub header: Span,
}

pub fn present(program: &Program) -> bool {
    program.declarations.iter().any(|d| match &d.kind {
        DeclarationKind::Spec { .. } | DeclarationKind::ModuleImpl { .. } => true,
        DeclarationKind::Module { body: Some(p), .. } => present(p),
        _ => false,
    })
}

fn error(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic::error("L0511", message, span)
}
fn public(span: Span) -> Option<Visibility> {
    Some(Visibility {
        scope: VisibilityScope::Public,
        span,
    })
}
fn name(d: &Declaration) -> Option<&Name> {
    match &d.kind {
        DeclarationKind::Function { name, .. }
        | DeclarationKind::Constant { name, .. }
        | DeclarationKind::Struct { name, .. }
        | DeclarationKind::Enum { name, .. }
        | DeclarationKind::Prop { name, .. }
        | DeclarationKind::Module { name, .. } => Some(name),
        _ => None,
    }
}
fn visibility(v: Option<&Visibility>) -> String {
    match v.map(|v| &v.scope) {
        None | Some(VisibilityScope::SelfModule) => "private".into(),
        Some(VisibilityScope::Public) => "pub".into(),
        Some(VisibilityScope::Crate) => "crate".into(),
        Some(VisibilityScope::Super) => "super".into(),
        Some(VisibilityScope::In(p)) => p.text(),
    }
}
fn signature(d: &Declaration) -> Option<(bool, Span)> {
    match &d.kind {
        DeclarationKind::Function {
            logical,
            name,
            body,
            ..
        } => Some((
            *logical,
            Span::new(name.span.file, name.span.start, body.span.start),
        )),
        DeclarationKind::Constant { name, value, .. } => Some((
            false,
            Span::new(name.span.file, name.span.start, value.span.start),
        )),
        _ => None,
    }
}
pub(super) struct Matcher<'a> {
    source: &'a SourceFile,
    tokens: Vec<lexer::Token>,
}
impl<'a> Matcher<'a> {
    pub fn new(source: &'a SourceFile) -> Self {
        Self {
            source,
            tokens: lexer::lex(source).tokens,
        }
    }
    fn key(&self, d: &Declaration) -> Option<(bool, Vec<&str>)> {
        let (logical, span) = signature(d)?;
        let start = self.tokens.partition_point(|t| t.span.start < span.start);
        let mut parts: Vec<_> = self.tokens[start..]
            .iter()
            .take_while(|t| t.span.start < span.end)
            .filter(|t| {
                !matches!(
                    t.kind,
                    lexer::TokenKind::OuterDoc | lexer::TokenKind::InnerDoc
                )
            })
            .filter_map(|t| self.source.slice(t.span))
            .collect();
        if matches!(d.kind, DeclarationKind::Constant { .. }) && parts.last() == Some(&"=") {
            parts.pop();
        }
        Some((logical, parts))
    }
    fn members(
        &self,
        headers: &[Declaration],
        bodies: &mut [Declaration],
        errors: &mut Vec<Diagnostic>,
    ) {
        let mut matched = BTreeSet::new();
        for body in bodies {
            let header = name(body).and_then(|n| {
                headers
                    .iter()
                    .find(|h| name(h).is_some_and(|hn| hn.text == n.text))
            });
            if let Some(header) = header {
                let n = name(header).unwrap();
                if !matched.insert(n.text.clone()) {
                    errors.push(
                        error(
                            format!("spec member `{}` has multiple definitions", n.text),
                            body.span,
                        )
                        .label(header.span, "declared here"),
                    );
                }
                let kind_matches = matches!(
                    (&header.kind, &body.kind),
                    (
                        DeclarationKind::Function { .. },
                        DeclarationKind::Function { .. }
                    ) | (
                        DeclarationKind::Constant { .. },
                        DeclarationKind::Constant { .. }
                    )
                );
                if !kind_matches || self.key(header) != self.key(body) {
                    errors.push(error(format!("implementation signature of `{}` differs from its spec", n.text), body.span)
                        .label(header.span, "required signature")
                        .note("repeat the same signature, including parameter names and proof propositions; whitespace and comments may differ"));
                }
                if body
                    .attributes
                    .iter()
                    .any(|a| matches!(a.kind, AttributeKind::Trusted { .. }))
                {
                    errors.push(error("a manual spec implementation must have a checked body, not a trusted native binding", body.span));
                }
                if body
                    .visibility
                    .as_ref()
                    .is_some_and(|v| v.scope != VisibilityScope::Public)
                {
                    errors.push(
                        error(
                            "a spec member cannot have restricted visibility in its implementation",
                            body.span,
                        )
                        .label(header.span, "this member is public"),
                    );
                }
                body.visibility = public(body.span);
                for attribute in &header.attributes {
                    if !body
                        .attributes
                        .iter()
                        .any(|a| a.kind.name() == attribute.kind.name())
                    {
                        body.attributes.push(attribute.clone());
                    }
                }
            } else if body.visibility.is_some() {
                errors.push(error(
                    "an implementation member absent from the spec must be private",
                    body.span,
                ));
            }
        }
        for header in headers {
            if let Some(n) = name(header)
                && !matched.contains(&n.text)
            {
                errors.push(error(
                    format!("spec member `{}` has no implementation", n.text),
                    header.span,
                ));
            }
        }
    }
}

/// Lower just this lexical scope. Nested modules are prepared when collected.
pub(super) fn prepare(
    mut program: Program,
    matcher: &Matcher<'_>,
    errors: &mut Vec<Diagnostic>,
) -> (Program, Vec<Guard>) {
    if !program.declarations.iter().any(|d| {
        matches!(
            d.kind,
            DeclarationKind::Spec { .. } | DeclarationKind::ModuleImpl { .. }
        )
    }) {
        return (program, Vec::new());
    }
    let mut specs: BTreeMap<String, Declaration> = BTreeMap::new();
    let mut definitions = Vec::new();
    for d in program.declarations {
        if let DeclarationKind::Spec {
            module,
            name: n,
            members,
        } = &d.kind
        {
            if !d.attributes.is_empty() {
                errors.push(Diagnostic::error(
                    "L0510",
                    "attributes on a spec are not supported yet",
                    d.span,
                ));
            }
            for member in members {
                if let DeclarationKind::Function { generics, .. } = &member.kind
                    && generics.iter().any(|g| !g.lifetime)
                {
                    errors.push(Diagnostic::error(
                        "L0510",
                        "generic spec members are not supported yet",
                        member.span,
                    ));
                }
                if member
                    .attributes
                    .iter()
                    .any(|a| matches!(a.kind, AttributeKind::Terminates { decreases: Some(_) }))
                {
                    errors.push(Diagnostic::error("L0510", "put the decreasing measure on the implementation; the header may require `#[terminates]`", member.span));
                }
            }
            if let Some(first) = specs.get_mut(&n.text) {
                if let DeclarationKind::Spec {
                    module: old_module,
                    members: old_members,
                    ..
                } = &mut first.kind
                {
                    if old_module != module
                        || visibility(first.visibility.as_ref())
                            != visibility(d.visibility.as_ref())
                    {
                        errors.push(
                            error(
                                "all blocks of a spec must agree on kind and visibility",
                                d.span,
                            )
                            .label(first.span, "first block"),
                        );
                    }
                    old_members.extend(members.clone());
                }
            } else {
                specs.insert(n.text.clone(), d);
            }
        } else {
            definitions.push(d);
        }
    }
    let mut guards = Vec::new();
    for (spec_name, spec) in specs {
        let DeclarationKind::Spec {
            module,
            name: spec_ident,
            members,
        } = &spec.kind
        else {
            unreachable!()
        };
        let mut seen = BTreeMap::new();
        for member in members {
            if let Some(n) = name(member)
                && let Some(first) = seen.insert(n.text.clone(), member.span)
            {
                errors.push(
                    error(format!("duplicate spec member `{}`", n.text), member.span)
                        .label(first, "first declaration; spec blocks do not override"),
                );
            }
        }
        if *module {
            let indices: Vec<_> = definitions
                .iter()
                .enumerate()
                .filter_map(|(i, d)| match &d.kind {
                    DeclarationKind::ModuleImpl { name, .. } if name.text == spec_name => Some(i),
                    _ => None,
                })
                .collect();
            if indices.len() != 1 {
                let at = indices.last().map_or(spec.span, |i| definitions[*i].span);
                errors.push(
                    error(
                        format!(
                            "spec module `{spec_name}` requires exactly one `impl mod {spec_name}`"
                        ),
                        at,
                    )
                    .label(spec.span, "module spec"),
                );
            }
            for i in indices {
                let d = &mut definitions[i];
                if !d.attributes.is_empty() || d.visibility.is_some() {
                    errors.push(error("put module visibility on the spec; put promises inside the implementation body", d.span));
                }
                let DeclarationKind::ModuleImpl { body, .. } = &mut d.kind else {
                    unreachable!()
                };
                if let Some(body) = body {
                    matcher.members(members, &mut body.declarations, errors);
                    d.kind = DeclarationKind::Module {
                        name: spec_ident.clone(),
                        body: Some(body.clone()),
                    };
                    d.visibility = spec.visibility.clone();
                    // Keep the implementation span, not the hull from header to body:
                    // intervening sibling items must retain their original privacy scope.
                } else {
                    errors.push(error(
                        "external spec implementation has not been loaded",
                        d.span,
                    ));
                }
            }
        } else {
            let representations: Vec<_> = definitions
                .iter()
                .enumerate()
                .filter_map(|(i, d)| match &d.kind {
                    DeclarationKind::Struct { name, .. } if name.text == spec_name => Some(i),
                    _ => None,
                })
                .collect();
            let implementations: Vec<_> = definitions
                .iter()
                .enumerate()
                .filter_map(|(i, d)| match &d.kind {
                    DeclarationKind::Impl {
                        model: None,
                        target,
                        ..
                    } if target.single().is_some_and(|n| n.text == spec_name) => Some(i),
                    _ => None,
                })
                .collect();
            if representations.len() != 1 || implementations.len() != 1 {
                errors.push(error(format!("spec type `{spec_name}` requires one local struct representation and one `impl {spec_name}`"), spec.span));
                continue;
            }
            let r = representations[0];
            let i = implementations[0];
            let representation = &mut definitions[r];
            if representation.visibility.is_some() || !representation.attributes.is_empty() {
                errors.push(Diagnostic::error("L0510", "a spec type's representation has no visibility or derive attributes; put visibility on the spec", representation.span));
            }
            if let DeclarationKind::Struct {
                generics, fields, ..
            } = &representation.kind
                && (!generics.is_empty() || fields.iter().any(|f| f.visibility.is_some()))
            {
                errors.push(Diagnostic::error(
                    "L0510",
                    "spec type representations are concrete structs with private fields",
                    representation.span,
                ));
            }
            representation.visibility = spec.visibility.clone();
            guards.push(Guard {
                name: spec_name,
                representation: representation.span,
                implementation: definitions[i].span,
                header: spec.span,
            });
            if let DeclarationKind::Impl { methods, .. } = &mut definitions[i].kind {
                matcher.members(members, methods, errors);
            }
        }
    }
    for d in &definitions {
        if matches!(d.kind, DeclarationKind::ModuleImpl { .. }) {
            errors.push(error(
                "module implementation has no matching complete spec",
                d.span,
            ));
        }
    }
    program.declarations = definitions;
    (program, guards)
}
