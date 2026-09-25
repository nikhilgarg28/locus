//! Plain Rust imports. Metadata acquisition is separate from spec realization
//! and from the small set of foreign operations admitted by the checking IR.
pub mod command;
mod extract;
pub mod model;
use crate::{
    ast::*,
    diagnostic::Diagnostic,
    project::cargo::{CargoOptions, Workspace},
};
pub use extract::Extraction;
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Foreign {
    pub entity: Arc<model::Entity>,
    /// A public native path, not necessarily its private defining path.
    pub path: String,
    pub package: usize,
    pub members: Vec<String>,
}
impl Foreign {
    pub fn is_type(&self) -> bool {
        matches!(
            self.entity.kind.as_str(),
            "struct"
                | "enum"
                | "union"
                | "trait"
                | "trait_alias"
                | "type_alias"
                | "primitive"
                | "module"
        )
    }
}
#[derive(Debug, Default)]
pub struct Imports {
    pub extraction: Option<Extraction>,
}
impl Imports {
    pub fn expand(
        &mut self,
        program: &mut Program,
        workspace: Option<&Workspace>,
        owner: usize,
        options: &CargoOptions,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();
        for d in &mut program.declarations {
            if let DeclarationKind::Module {
                body: Some(body), ..
            } = &mut d.kind
            {
                if let Err(mut e) = self.expand(body, workspace, owner, options) {
                    errors.append(&mut e)
                }
                continue;
            }
            let DeclarationKind::RustImport { path, alias } = &d.kind else {
                continue;
            };
            let fail = |message: String| Diagnostic::error("L0512", message, d.span);
            if d.visibility.is_some() || !d.attributes.is_empty() {
                errors.push(fail("`import` has no visibility or attributes; use `pub use` to re-export a supported imported item".into()));
                continue;
            }
            let Some(workspace) = workspace else {
                errors.push(fail("Rust imports currently require a Cargo.toml host; pass --manifest-path to select it".into()));
                continue;
            };
            let mut segments: Vec<String> = path.segments.iter().map(|n| n.text.clone()).collect();
            let Some(first) = segments.first() else {
                continue;
            };
            let package = if first == "crate" {
                Some(owner)
            } else {
                workspace.packages[owner].dependencies.get(first).copied()
            };
            let Some(package) = package else {
                errors.push(Diagnostic::error("L0513",format!("Rust crate `{first}` is not an active ordinary Cargo dependency. Use its Cargo alias, or `crate` for the host's native library. Sysroot imports need a separate adapter and are not available yet."),path.span));
                continue;
            };
            // Emission occurs in the host Rust crate. Resolve dependency-owned
            // imports by package identity, never by an unrelated local alias.
            if owner != workspace.host {
                if let Some((alias, _)) = workspace.packages[workspace.host]
                    .dependencies
                    .iter()
                    .find(|(_, p)| **p == package)
                {
                    segments[0] = alias.clone();
                } else {
                    errors.push(fail("this Locus dependency imports a Rust package that the host does not directly depend on; add that package to the host Cargo.toml so generated Rust can name it".into()));
                    continue;
                }
            }
            if self.extraction.is_none() {
                match Extraction::new(workspace, options) {
                    Ok(e) => self.extraction = Some(e),
                    Err(e) => {
                        errors.push(fail(e));
                        continue;
                    }
                }
            }
            let extraction = self.extraction.as_mut().unwrap();
            let native = match extraction.get(workspace, package) {
                Ok(n) => n,
                Err(e) => {
                    errors.push(fail(e));
                    continue;
                }
            };
            let id = match native
                .interface
                .find(&segments[1..].iter().map(String::as_str).collect::<Vec<_>>())
            {
                Ok(id) => id,
                Err(e) => {
                    errors.push(Diagnostic::error("L0513", e, path.span));
                    continue;
                }
            };
            let name = alias.clone().unwrap_or_else(|| path.last().clone());
            match bind(
                &native.interface,
                &id,
                segments,
                name,
                package,
                false,
                &mut BTreeSet::new(),
            ) {
                Ok(declaration) => *d = declaration,
                Err(e) => errors.push(fail(e)),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    pub fn validate(&self, program: &Program) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(extraction) = &self.extraction {
            for d in &program.declarations {
                if let DeclarationKind::Foreign { foreign, .. } = &d.kind
                    && let Some(signature) = &foreign.entity.signature
                    && seen.insert((foreign.package, foreign.path.clone()))
                    && let Err(e) =
                        extraction.validate_function(foreign.package, &foreign.members, signature)
                {
                    errors.push(Diagnostic::error(
                        "L0512",
                        format!("cannot use imported `{}`: {e}", foreign.path),
                        d.span,
                    ));
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    pub fn receipt(&self) -> serde_json::Value {
        self.extraction.as_ref().map_or(serde_json::Value::Null,|e|serde_json::json!({"context":e.context,"interfaces":e.interfaces.iter().map(|(id,n)|(id.to_string(),n.interface.context.clone())).collect::<std::collections::BTreeMap<_,_>>()}))
    }
}
fn bind(
    interface: &model::Interface,
    id: &str,
    path: Vec<String>,
    name: Name,
    package: usize,
    public: bool,
    ancestors: &mut BTreeSet<String>,
) -> Result<Declaration, String> {
    let span = name.span;
    let mut entity = interface
        .entities
        .get(id)
        .ok_or_else(|| format!("public Rust reference {id} has no metadata"))?
        .clone();
    let kind = if entity.kind == "module"
        && entity.unavailable.is_none()
        && ancestors.insert(id.into())
    {
        let mut body = Program::default();
        for (child, child_id) in interface.children(id)? {
            let mut native_path = path.clone();
            native_path.push(child.clone());
            body.declarations.push(bind(
                interface,
                &child_id,
                native_path,
                Name { text: child, span },
                package,
                true,
                ancestors,
            )?);
        }
        ancestors.remove(id);
        DeclarationKind::ImportedModule {
            name,
            path: path.join("::"),
            body,
        }
    } else {
        if entity.kind == "module" && entity.unavailable.is_none() {
            entity.unavailable=Some("recursive Rust module re-exports are retained but cannot yet be traversed in Locus".into());
        }
        DeclarationKind::Foreign {
            name,
            foreign: Foreign {
                entity: Arc::new(entity),
                path: if path[0] == "crate" {
                    path.join("::")
                } else {
                    format!("::{}", path.join("::"))
                },
                package,
                members: path[1..].to_vec(),
            },
        }
    };
    Ok(Declaration {
        captures: Vec::new(),
        doc: vec![],
        attributes: vec![],
        visibility: public.then_some(Visibility {
            scope: VisibilityScope::Public,
            span,
        }),
        kind,
        span,
    })
}
