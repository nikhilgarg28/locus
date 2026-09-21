//! Declarations: the driver of elaboration.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast::{self, DeclarationKind, FunctionMode};
use crate::diagnostic::Diagnostic;
use crate::kernel::theory;
use crate::kernel::{Context, Definitions};
use crate::source::{SourceFile, Span};
use crate::typed::{Binder, EnumItem, FnItem, FnRef, Session, StructItem, VariantItem};

use super::env::{Elab, EnumInfo, Env, FnInfo, Global, StructInfo};
use super::order::{declared_name, dependency_order};

/// One `_`, or one conversion of evidence: whether it was filled, by which
/// tier of the search, and what that cost.
#[derive(Clone, Debug)]
pub struct HoleReport {
    pub span: Span,
    pub solved: bool,
    pub tier: &'static str,
    /// The size of the proof found, as the length of its debug text.
    pub proof_size: usize,
    /// Time to find the proof and check it once, in microseconds.
    pub micros: u128,
}

pub struct Elaborated {
    /// Every item that was accepted, lowered, checked, and erased.
    pub session: Session,
    /// The accepted functions, in source order.
    pub functions: Vec<(String, FnRef)>,
    pub diagnostics: Vec<Diagnostic>,
    pub holes: Vec<HoleReport>,
}

impl Elaborated {
    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn function(&self, name: &str) -> Option<FnRef> {
        self.functions
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, reference)| *reference)
    }
}

pub fn elaborate(source: &SourceFile, program: &ast::Program) -> Elaborated {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory is checked");
    let mut env = Env {
        source,
        session: Session::new(definitions),
        prelude,
        theory,
        globals: HashMap::new(),
        failed: HashSet::new(),
        diagnostics: Vec::new(),
        holes: Vec::new(),
        ctx: Context::new(),
        names: Vec::new(),
        facts: Vec::new(),
        loops: Vec::new(),
        labels: HashMap::new(),
        total: false,
    };

    let mut seen: HashMap<&str, Span> = HashMap::new();
    let mut duplicates = HashSet::new();
    for (index, declaration) in program.declarations.iter().enumerate() {
        let name = declared_name(declaration);
        if let Some(first) = seen.get(name.text.as_str()) {
            env.diagnostics.push(
                Diagnostic::error(
                    "L0202",
                    format!("`{}` is declared twice", name.text),
                    name.span,
                )
                .label(*first, "first declared here"),
            );
            duplicates.insert(index);
        } else {
            seen.insert(&name.text, name.span);
        }
    }

    let (order, cyclic) = dependency_order(program);
    for index in cyclic {
        let name = declared_name(&program.declarations[index]);
        env.diagnostics.push(
            Diagnostic::error(
                "L0203",
                format!("`{}` is defined in terms of itself", name.text),
                name.span,
            )
            .note("recursion is not part of the core language; a bounded `for` repeats a step a known number of times"),
        );
        env.failed.insert(name.text.clone());
    }
    let mut accepted: Vec<(usize, String, FnRef)> = Vec::new();
    for index in order {
        if duplicates.contains(&index) {
            continue;
        }
        let declaration = &program.declarations[index];
        let name = declared_name(declaration).text.clone();
        match env.declaration(declaration) {
            Ok(global) => {
                if let Global::Fn(info) = &global {
                    accepted.push((index, name.clone(), info.reference));
                }
                env.globals.insert(name, global);
            }
            // Either reported, or a consequence of a failure that was.
            Err(()) => {
                env.failed.insert(name);
            }
        }
    }
    accepted.sort_by_key(|(index, _, _)| *index);
    env.diagnostics
        .sort_by_key(|diagnostic| diagnostic.labels[0].span.start);
    Elaborated {
        session: env.session,
        functions: accepted
            .into_iter()
            .map(|(_, name, reference)| (name, reference))
            .collect(),
        diagnostics: env.diagnostics,
        holes: env.holes,
    }
}

impl Env<'_> {
    /// A fresh function scope over the declarations accepted so far.
    fn start_item(&mut self, total: bool) {
        let definitions = self.session.program().definitions().clone();
        self.ctx = Context::with_definitions(Rc::new(definitions));
        self.names.clear();
        self.facts.clear();
        self.loops.clear();
        self.total = total;
    }

    fn declaration(&mut self, declaration: &ast::Declaration) -> Elab<Global> {
        match &declaration.kind {
            DeclarationKind::Struct { name, fields } => {
                self.start_item(true);
                let fields = self.telescope(
                    fields
                        .iter()
                        .map(|field| (Some(&field.name), &field.ty, field.span)),
                )?;
                let item = StructItem {
                    name: name.text.clone(),
                    fields: fields.clone(),
                };
                let id = match self.session.declare_struct(&item) {
                    Ok(id) => id,
                    Err(error) => return self.internal(error, name.span),
                };
                Ok(Global::Struct(Rc::new(StructInfo {
                    id,
                    name: name.text.clone(),
                    fields,
                })))
            }
            DeclarationKind::Enum { name, variants } => {
                let mut items = Vec::new();
                for variant in variants {
                    if items
                        .iter()
                        .any(|(earlier, _): &(String, Vec<Binder>)| *earlier == variant.name.text)
                    {
                        let message = format!("variant `{}` is declared twice", variant.name.text);
                        return self.fail("L0202", message, variant.name.span);
                    }
                    self.start_item(true);
                    let payload = self.telescope(
                        variant
                            .fields
                            .iter()
                            .map(|field| (field.name.as_ref(), &field.ty, field.span)),
                    )?;
                    items.push((variant.name.text.clone(), payload));
                }
                let item = EnumItem {
                    name: name.text.clone(),
                    variants: items
                        .iter()
                        .map(|(name, payload)| VariantItem {
                            name: name.clone(),
                            payload: payload.clone(),
                        })
                        .collect(),
                };
                let id = match self.session.declare_enum(&item) {
                    Ok(id) => id,
                    Err(error) => return self.internal(error, name.span),
                };
                Ok(Global::Enum(Rc::new(EnumInfo {
                    id,
                    name: name.text.clone(),
                    variants: items,
                })))
            }
            DeclarationKind::Function {
                mode,
                name,
                parameters,
                result,
                body,
            } => {
                let math = *mode == FunctionMode::Math;
                self.start_item(math);
                let mut params: Vec<Binder> = Vec::new();
                for parameter in parameters {
                    if params
                        .iter()
                        .any(|earlier| earlier.name == parameter.name.text)
                    {
                        let message =
                            format!("parameter `{}` is declared twice", parameter.name.text);
                        return self.fail("L0202", message, parameter.name.span);
                    }
                    let ty = self.ty(&parameter.ty)?;
                    let binder = Binder::new(&parameter.name.text, ty);
                    self.declare(&binder, false, parameter.span)?;
                    params.push(binder);
                }
                let result_ty = self.ty(result)?;
                let (block, _, _) = self.block(body, Some(&result_ty))?;
                let item = FnItem {
                    name: name.text.clone(),
                    math,
                    params: params.clone(),
                    result: result_ty.clone(),
                    body: block,
                };
                let reference = match self.session.declare_fn(&item) {
                    Ok(reference) => reference,
                    Err(error) => return self.internal(error, name.span),
                };
                Ok(Global::Fn(Rc::new(FnInfo {
                    reference,
                    name: name.text.clone(),
                    params,
                    result: result_ty,
                })))
            }
            DeclarationKind::Prop { name, .. } => self.fail(
                "L0290",
                "`prop` declarations are not supported by the elaborator yet",
                name.span,
            ),
            DeclarationKind::Constant { name, .. } => self.fail(
                "L0290",
                "`const` declarations are not supported by the elaborator yet",
                name.span,
            ),
        }
    }
}
