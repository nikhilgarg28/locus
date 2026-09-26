//! Check the complete reachable Rust interface before granting any Rust visibility.
//! Source visibility never directly grants visibility in the generated backend.
mod traits;
use super::resolve::is_public;
use super::{Checked, Error};
use crate::{
    ast::{Declaration, DeclarationKind},
    diagnostic::Diagnostic,
    erased::{self, EType, Module, ProofOutput, Visibilities},
    source::Span,
};
use std::collections::{BTreeMap, BTreeSet};

pub fn rust(unit: Checked) -> Result<String, Error> {
    let graph = &unit.loaded.graph;
    let module = unit.checked.session.erased();
    let mut export = Interface {
        module,
        declarations: &unit.loaded.program.declarations,
        visibility: Visibilities::default(),
        visited: BTreeSet::new(),
        errors: Vec::new(),
        origin: graph.scopes[graph.export_root].span,
    };
    let exports = graph.exports(graph.export_root);
    for root in &exports {
        let item = &graph.items[root.item];
        let path = root.path.join("::");
        if !matches!(item.declaration.kind, DeclarationKind::Trait { .. }) {
            export.item(item, &path, root.span);
        }
    }
    let trait_source = traits::interfaces(&unit, &mut export, &exports);
    let mut external_names = BTreeMap::new();
    let mut foreign = BTreeSet::new();
    export
        .errors
        .extend(dependency_target_errors(&unit, module));
    if let Some(workspace) = &unit.loaded.cargo {
        // A direct Cargo dependency supplies the Rust path. Transitive values
        // need a public re-export through a direct dependency to be nameable.
        let host = &workspace.packages[workspace.host];
        for (alias, package) in &host.dependencies {
            let Some(index) = unit.loaded.packages.iter().position(|p| p == package) else {
                continue;
            };
            let dependency = &workspace.packages[*package];
            for (target, root) in dependency.targets.iter().zip(&graph.export_roots[index]) {
                for exposed in graph.exports(*root) {
                    let item = &graph.items[exposed.item];
                    if matches!(item.declaration.kind, DeclarationKind::Foreign { .. }) {
                        continue;
                    }
                    let prefix = if target.rust_module.is_empty() {
                        format!("::{alias}")
                    } else {
                        format!("::{alias}::{}", target.rust_module)
                    };
                    external_names
                        .entry(item.canonical.clone())
                        .or_insert_with(|| format!("{prefix}::{}", exposed.path.join("::")));
                    export.item(item, &exposed.path.join("::"), exposed.span);
                }
            }
        }
        for item in &graph.items {
            // Plain native imports already carry a validated host Rust path.
            // Their one-line call adapters are not dependency implementations.
            if matches!(item.declaration.kind, DeclarationKind::Foreign { .. }) {
                continue;
            }
            if graph.package_of(item.module) != 0 {
                foreign.insert(item.canonical.clone());
                export.visibility.hide(&item.canonical);
                // The source monomorphizer names each instantiated template.
                // Runtime dependency specializations need an ABI mapping; do
                // not silently copy their implementation into the consumer.
                let prefix = format!("__locus_{}_", item.canonical.trim_start_matches('_'));
                if module.fns.iter().any(|f| f.name.starts_with(&prefix))
                    || module.structs.iter().any(|s| s.name.starts_with(&prefix))
                    || module.enums.iter().any(|e| e.name.starts_with(&prefix))
                {
                    export.errors.push(Diagnostic::error(
                        "L0504",
                        format!("generic runtime dependency `{}` has no supported cross-package ABI mapping", item.original),
                        item.declaration.span,
                    ).note("export a concrete runtime wrapper from the dependency; generic logical helpers remain available because they erase"));
                }
            }
        }
        for f in &module.fns {
            if foreign.iter().any(|name| {
                f.name.starts_with(&format!("{name}::")) || f.name.starts_with(&format!("{name}__"))
            }) {
                export.visibility.hide(&f.name);
            }
        }
    }
    for f in &module.fns {
        if export.visited.contains(&format!("fn:{}", f.name))
            && (foreign.contains(&f.name) || f.owner.as_ref().is_some_and(|o| foreign.contains(o)))
        {
            if ProofOutput::new(&f.result).changed() {
                export.error(
                    &f.name,
                    graph.scopes[graph.export_root].span,
                    graph.scopes[graph.export_root].span,
                    "a proof-returning dependency function has a projected Rust result; cross-package runtime proof interfaces are not supported yet",
                );
            }
            for ty in f
                .params
                .iter()
                .map(|(_, _, t)| t)
                .chain(std::iter::once(&f.result))
            {
                if let Some(reason) =
                    export.dependency_abi(ty, &external_names, &mut BTreeSet::new())
                {
                    export.error(
                        &f.name,
                        graph.scopes[graph.export_root].span,
                        graph.scopes[graph.export_root].span,
                        &reason,
                    );
                }
            }
        }
    }
    if !export.errors.is_empty() {
        return Err(Error {
            diagnostics: export
                .errors
                .iter()
                .map(|d| unit.loaded.diagnostic(d))
                .collect(),
            sources: unit.loaded.sources,
        });
    }
    let mut body = erased::print_module_with(module, &export.visibility, erased::Markers::Here);
    body.push_str(&trait_source);
    // Any surviving foreign symbol must name an actual exported Rust item.
    // Hidden dependency bodies are never copied into the consumer.
    let mut sources = crate::source::SourceMap::default();
    let file = sources.add("generated", body.clone());
    for token in crate::lexer::lex(sources.get(file)).tokens {
        if token.kind == crate::lexer::TokenKind::Name {
            let name = &body[token.span.start..token.span.end];
            if foreign.contains(name) && !external_names.contains_key(name) {
                let item = graph.items.iter().find(|i| i.canonical == name).unwrap();
                return Err(Error{diagnostics:vec![unit.loaded.diagnostic(&Diagnostic::error("L0504",format!("dependency runtime item `{}` has no reachable Rust export; expose it through a declared target",item.original),item.declaration.span))],sources:unit.loaded.sources});
            }
        }
    }
    // All original names are scoped. Keep unambiguous names readable; retain
    // deterministic qualified identifiers where two modules use the same name.
    let mut counts = BTreeMap::<String, usize>::new();
    for item in &graph.items {
        *counts.entry(item.original.clone()).or_default() += 1;
    }
    let mut names = external_names;
    for item in &graph.items {
        if !foreign.contains(&item.canonical)
            && item.namespace == super::resolve::Namespace::Type
            && !item.original.is_empty()
            && counts[&item.original] == 1
            && item.original != "Erased"
        {
            names.insert(item.canonical.clone(), item.original.clone());
        }
    }
    let mut facade = Facade::default();
    for root in &exports {
        let canonical = &graph.items[root.item].canonical;
        facade.insert(
            &root.path,
            names.get(canonical).unwrap_or(canonical).clone(),
        );
    }
    let mut source = String::from("// Generated by Locus. Do not edit.\nmod __locus_impl {\n");
    for line in rename_identifiers(&body, &names).lines() {
        source.push_str("    ");
        source.push_str(line);
        source.push('\n');
    }
    source.push_str("    pub mod __exports {\n");
    facade.render(&mut source, 2);
    source.push_str(
        "    }\n}\n#[allow(unused_imports)]\npub use self::__locus_impl::__exports::*;\n",
    );
    Ok(source)
}

// Every independent target emits its own nominal types. A source identity
// therefore needs one owning target before an importing package can use it.
// Include types reached through signatures and methods, not only named exports.
fn dependency_target_errors(unit: &Checked, module: &Module) -> Vec<Diagnostic> {
    let Some(workspace) = &unit.loaded.cargo else {
        return Vec::new();
    };
    let graph = &unit.loaded.graph;
    let mut errors = Vec::new();
    for (index, &package) in unit.loaded.packages.iter().enumerate().skip(1) {
        let mut owners = BTreeMap::<String, (String, Span)>::new();
        for (target, root) in workspace.packages[package]
            .targets
            .iter()
            .zip(&graph.export_roots[index])
        {
            let at = graph.scopes[*root].span;
            let mut interface = Interface {
                module,
                declarations: &unit.loaded.program.declarations,
                visibility: Visibilities::default(),
                visited: BTreeSet::new(),
                errors: Vec::new(),
                origin: at,
            };
            for exposed in graph.exports(*root) {
                interface.item(
                    &graph.items[exposed.item],
                    &exposed.path.join("::"),
                    exposed.span,
                );
            }
            for key in &interface.visited {
                let Some(name) = key.strip_prefix("type:") else {
                    continue;
                };
                let Some(item) = graph.items.iter().find(|i| i.canonical == name) else {
                    continue;
                };
                // Re-exporting a dependency's existing Rust type does not copy it.
                if graph.package_of(item.module) != index {
                    continue;
                }
                if let Some((previous, previous_at)) = owners.get(name) {
                    errors.push(Diagnostic::error("L0504", format!("runtime type `{}` is exposed by independent targets `{previous}` and `{}`", item.original, target.name), item.declaration.span)
                        .label(*previous_at, "first independently generated interface")
                        .label(at, "second independently generated interface")
                        .note("these builds create distinct Rust types; expose shared types through one target with multiple public modules"));
                } else {
                    owners.insert(name.into(), (target.name.clone(), at));
                }
            }
            // Export validation below reports direct-dependency errors. This
            // pass also checks interfaces used by transitive dependencies.
            if !workspace.packages[workspace.host]
                .dependencies
                .values()
                .any(|p| *p == package)
            {
                errors.extend(interface.errors);
            }
        }
    }
    errors
}

struct Interface<'a> {
    module: &'a Module,
    declarations: &'a [Declaration],
    visibility: Visibilities,
    visited: BTreeSet<String>,
    errors: Vec<Diagnostic>,
    origin: Span,
}
impl Interface<'_> {
    fn item(&mut self, item: &super::resolve::Item, path: &str, at: Span) {
        if let Some(f) = self.module.fns.iter().find(|f| f.name == item.canonical) {
            self.function(f, path, at);
        } else if let Some(s) = self
            .module
            .structs
            .iter()
            .find(|s| s.name == item.canonical)
        {
            self.ty(&EType::Struct(s.id), path, at);
        } else if let Some(e) = self.module.enums.iter().find(|e| e.name == item.canonical) {
            self.ty(&EType::Enum(e.id), path, at);
        } else {
            self.error(path, at, item.declaration.span,
                "this declaration has no concrete runtime interface (logical declarations and uninstantiated generic templates cannot be exported)");
        }
    }

    fn error(&mut self, path: &str, export: Span, origin: Span, reason: &str) {
        self.errors.push(Diagnostic::error("L0504", format!("cannot export `{path}`: {reason}"), export)
            .label(origin, "the incompatible interface is declared here")
            .note("keep this item in the Locus library interface, or export a wrapper using only runtime inputs and outputs; private logical fields may protect a runtime struct"));
    }
    fn declaration(&self, canonical: &str) -> Option<&Declaration> {
        self.declarations
            .iter()
            .find(|d| super::resolve::declared_name(&d.kind).is_some_and(|n| n.text == canonical))
    }
    fn function(&mut self, f: &erased::EFn, path: &str, at: Span) {
        if !self.visited.insert(format!("fn:{}", f.name)) {
            return;
        }
        self.visibility.set_value(&f.name, "pub");
        let saved = self.origin;
        let declaration = self.declaration(&f.name).or_else(|| {
            self.declarations.iter().find_map(|d| {
                if let DeclarationKind::Impl {
                    target, methods, ..
                } = &d.kind
                {
                    methods.iter().find(|m| {
                        super::resolve::declared_name(&m.kind)
                            .is_some_and(|n| format!("{}::{}", target.text(), n.text) == f.name)
                    })
                } else {
                    None
                }
            })
        });
        let parameter_spans: BTreeMap<_, _> = declaration
            .and_then(|d| {
                if let DeclarationKind::Function { parameters, .. } = &d.kind {
                    Some(
                        parameters
                            .iter()
                            .map(|p| (p.name.text.clone(), p.ty.span))
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let result_span = declaration
            .map(|d| match &d.kind {
                DeclarationKind::Function { result, .. } => result.span,
                DeclarationKind::Constant { ty, .. } => ty.span,
                _ => d.span,
            })
            .unwrap_or(at);
        for (_, name, ty) in &f.params {
            self.origin = parameter_spans.get(name).copied().unwrap_or(result_span);
            self.ty(ty, &format!("{path} -> parameter `{name}`"), at);
        }
        self.origin = result_span;
        let projection = ProofOutput::new(&f.result);
        if !f.constant && projection.changed() {
            self.proof_result(&f.result, &format!("{path} -> result"), at);
            self.visibility.project_proof_output(&f.name, projection);
        } else {
            self.ty(&f.result, &format!("{path} -> result"), at);
        }
        self.origin = saved;
    }
    // Keep original source tuple indices in diagnostics even when the facade
    // removes earlier proof positions. Nominal/container/callback checks stay strict.
    fn proof_result(&mut self, ty: &EType, path: &str, at: Span) {
        match ty {
            EType::Proved => {}
            EType::Tuple(fields) => {
                for (i, field) in fields.iter().enumerate() {
                    self.proof_result(field, &format!("{path} -> tuple field {i}"), at);
                }
            }
            _ => self.ty(ty, path, at),
        }
    }

    fn ty(&mut self, ty: &EType, path: &str, at: Span) {
        match ty {
            EType::Ghost | EType::Proved => self.error(
                path,
                at,
                self.origin,
                "a logical type reaches the Rust interface",
            ),
            EType::Boxed(t)
            | EType::Buffer(t)
            | EType::Array(t, _)
            | EType::Slice(t)
            | EType::Ref(_, t) => self.ty(t, &format!("{path} -> element"), at),
            EType::Tuple(ts) => {
                for (i, t) in ts.iter().enumerate() {
                    self.ty(t, &format!("{path} -> tuple field {i}"), at);
                }
            }
            EType::Fn(ps, r) => {
                for (i, t) in ps.iter().enumerate() {
                    self.ty(t, &format!("{path} -> callback parameter {i}"), at);
                }
                self.ty(r, &format!("{path} -> callback result"), at);
            }
            EType::Struct(id) | EType::StructApplied(id, _) => {
                if self.module.dynamics.iter().any(|d| d.id == *id) {
                    self.error(path, at, at, "trait objects are internal to Locus in this tier; Rust export of dyn signatures is deferred");
                    return;
                }
                let Some(s) = self.module.structs.iter().find(|s| s.id == *id) else {
                    return;
                };
                if !self.visited.insert(format!("type:{}", s.name)) {
                    return;
                }
                self.visibility.set_type(&s.name, "pub");
                let fields = self.declaration(&s.name).and_then(|d| {
                    if let DeclarationKind::Struct { fields, .. } = &d.kind {
                        Some(fields.clone())
                    } else {
                        None
                    }
                });
                // Specializations lacking declaration metadata are closed by
                // default. Their public API needs an explicit monomorphic wrapper.
                if fields.is_none() {
                    self.error(
                        path,
                        at,
                        at,
                        "exporting a specialized generic struct is not supported yet",
                    );
                    return;
                }
                let fields = fields.unwrap();
                let carries_logic = s
                    .fields
                    .iter()
                    .any(|(_, t)| self.contains_logic(t, &mut BTreeSet::new()));
                for (name, t) in &s.fields {
                    let f = fields
                        .iter()
                        .find(|f| f.name.text == *name)
                        .expect("source field");
                    if !is_public(f.visibility.as_ref()) {
                        continue;
                    }
                    if carries_logic {
                        self.error(&format!("{path} -> field `{name}`"),at,f.span,"a public field would expose or permit mutation of a value carrying logical evidence");
                    }
                    self.visibility.set_field(&s.name, name, "pub");
                    self.ty(t, &format!("{path} -> field `{name}`"), at);
                }
                self.methods(&s.name, path, at);
            }
            EType::Enum(id) | EType::EnumApplied(id, _) => {
                let Some(e) = self.module.enums.iter().find(|e| e.id == *id) else {
                    return;
                };
                if !self.visited.insert(format!("type:{}", e.name)) {
                    return;
                }
                self.visibility.set_type(&e.name, "pub");
                for v in &e.variants {
                    for (i, t) in v.payload.iter().enumerate() {
                        self.ty(
                            t,
                            &format!("{path} -> variant `{}` -> field {i}", v.name),
                            at,
                        );
                    }
                }
                self.methods(&e.name, path, at);
            }
            EType::Bool | EType::Int(_) => {}
        }
    }
    fn methods(&mut self, owner: &str, path: &str, at: Span) {
        for d in self.declarations {
            if let DeclarationKind::Impl {
                target, methods, ..
            } = &d.kind
                && target.segments[0].text == owner
            {
                for m in methods {
                    if !is_public(m.visibility.as_ref()) {
                        continue;
                    }
                    let Some(n) = super::resolve::declared_name(&m.kind) else {
                        continue;
                    };
                    if n.text.starts_with("__locus_trait_") {
                        continue;
                    }
                    let name = format!("{owner}::{}", n.text);
                    if let Some(f) = self.module.fns.iter().find(|f| f.name == name) {
                        self.function(f, &format!("{path} -> method `{}`", n.text), at);
                    } else {
                        self.error(
                            &format!("{path} -> method `{}`", n.text),
                            at,
                            m.span,
                            "a logical or generic public method cannot be exported",
                        );
                    }
                }
            }
        }
    }
    fn dependency_abi(
        &self,
        ty: &EType,
        paths: &BTreeMap<String, String>,
        seen: &mut BTreeSet<String>,
    ) -> Option<String> {
        match ty {
            EType::Tuple(ts) => ts.iter().find_map(|t| self.dependency_abi(t, paths, seen)),
            EType::Fn(ps, r) => ps
                .iter()
                .chain(std::iter::once(r.as_ref()))
                .find_map(|t| self.dependency_abi(t, paths, seen)),
            EType::Boxed(t)
            | EType::Buffer(t)
            | EType::Array(t, _)
            | EType::Slice(t)
            | EType::Ref(_, t) => self.dependency_abi(t, paths, seen),
            EType::Struct(id) | EType::StructApplied(id, _) => {
                let item = self.module.structs.iter().find(|s| s.id == *id)?;
                if !paths.contains_key(&item.name) {
                    return Some(format!(
                        "dependency type `{}` needs an explicit public re-export through a declared Rust target",
                        item.name
                    ));
                }
                if !seen.insert(item.name.clone()) {
                    return None;
                }
                let fields = self.declaration(&item.name).and_then(|d| {
                    if let DeclarationKind::Struct { fields, .. } = &d.kind {
                        Some(fields)
                    } else {
                        None
                    }
                })?;
                item.fields
                    .iter()
                    .filter(|(name, _)| {
                        fields
                            .iter()
                            .any(|f| f.name.text == *name && is_public(f.visibility.as_ref()))
                    })
                    .find_map(|(_, t)| self.dependency_abi(t, paths, seen))
            }
            EType::Enum(id) | EType::EnumApplied(id, _) => {
                let item = self.module.enums.iter().find(|e| e.id == *id)?;
                if !paths.contains_key(&item.name) {
                    return Some(format!(
                        "dependency enum `{}` has no stable exported Rust path; specialized collection enums are not a cross-package ABI yet",
                        item.name
                    ));
                }
                if !seen.insert(item.name.clone()) {
                    return None;
                }
                item.variants
                    .iter()
                    .flat_map(|v| &v.payload)
                    .find_map(|t| self.dependency_abi(t, paths, seen))
            }
            _ => None,
        }
    }
    fn contains_logic(&self, ty: &EType, seen: &mut BTreeSet<String>) -> bool {
        match ty {
            EType::Ghost | EType::Proved => true,
            EType::Boxed(t)
            | EType::Buffer(t)
            | EType::Array(t, _)
            | EType::Slice(t)
            | EType::Ref(_, t) => self.contains_logic(t, seen),
            EType::Tuple(ts) => ts.iter().any(|t| self.contains_logic(t, seen)),
            EType::Fn(ps, r) => {
                ps.iter().any(|t| self.contains_logic(t, seen)) || self.contains_logic(r, seen)
            }
            EType::Struct(id) | EType::StructApplied(id, _) => self
                .module
                .structs
                .iter()
                .find(|s| s.id == *id)
                .is_some_and(|s| {
                    seen.insert(s.name.clone())
                        && s.fields.iter().any(|(_, t)| self.contains_logic(t, seen))
                }),
            EType::Enum(id) | EType::EnumApplied(id, _) => self
                .module
                .enums
                .iter()
                .find(|e| e.id == *id)
                .is_some_and(|e| {
                    seen.insert(e.name.clone())
                        && e.variants
                            .iter()
                            .any(|v| v.payload.iter().any(|t| self.contains_logic(t, seen)))
                }),
            _ => false,
        }
    }
}

#[derive(Default)]
struct Facade {
    modules: BTreeMap<String, Facade>,
    items: Vec<(String, String)>,
}
impl Facade {
    fn insert(&mut self, path: &[String], canonical: String) {
        if path.len() == 1 {
            self.items.push((path[0].clone(), canonical));
        } else {
            self.modules
                .entry(path[0].clone())
                .or_default()
                .insert(&path[1..], canonical);
        }
    }
    fn render(&self, out: &mut String, depth: usize) {
        let indent = "    ".repeat(depth);
        let prefix = "super::".repeat(depth - 1);
        for (name, canonical) in &self.items {
            let path = if canonical.starts_with("::") {
                canonical.clone()
            } else {
                format!("{prefix}{canonical}")
            };
            out.push_str(&format!(
                "{indent}#[allow(unused_imports)]\n{indent}pub use {path} as {name};\n"
            ));
        }
        for (name, module) in &self.modules {
            out.push_str(&format!("{indent}pub mod {name} {{\n"));
            module.render(out, depth + 1);
            out.push_str(&format!("{indent}}}\n"));
        }
    }
}

fn rename_identifiers(source: &str, names: &BTreeMap<String, String>) -> String {
    let mut sources = crate::source::SourceMap::default();
    let file = sources.add("generated", source);
    let tokens = crate::lexer::lex(sources.get(file));
    let mut result = String::new();
    let mut end = 0;
    for token in tokens.tokens {
        if token.kind == crate::lexer::TokenKind::Name
            && let Some(replacement) = names.get(&source[token.span.start..token.span.end])
        {
            result.push_str(&source[end..token.span.start]);
            result.push_str(replacement);
            end = token.span.end;
        }
    }
    result.push_str(&source[end..]);
    result
}
