//! Declarations: the driver of elaboration.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast::{self, DeclarationKind, FunctionMode};
use crate::diagnostic::Diagnostic;
use crate::kernel::theory;
use crate::kernel::{Context, Definitions, FnId, Proof, PropVariant, Term, Type};
use crate::source::{SourceFile, Span};
use crate::typed::{Binder, EnumItem, FnItem, FnRef, Session, StructItem, VariantItem};

use super::env::{Elab, EnumInfo, Env, FnInfo, Global, PropInfo, PropVariantInfo, StructInfo};
use super::order::{declared_name, dependency_order};
use super::types::tuple_over;

/// One `_`, `prove!`, or conversion of evidence, or the range of a `for`:
/// whether it was filled, by which tier (`exact`, `computed`, `evaluation`,
/// or the lemma `<T>_zero_le` for a range from `0`), and what that cost.
#[derive(Clone, Debug)]
pub struct HoleReport {
    pub span: Span,
    pub solved: bool,
    pub tier: &'static str,
    /// The size of the proof found: roughly its number of nodes.
    pub proof_size: usize,
    /// Time to find the proof and check it once, in microseconds.
    pub micros: u128,
    /// What was found, when the kernel accepted it.
    pub found: Option<FoundProof>,
}

/// A proof the search found, the claim it was found for, and the context
/// the kernel accepted it in. Nothing in the compiler reads this; it is what
/// a test of the kernel needs to check the same proof again, or a changed
/// one, against a claim of its own.
#[derive(Clone, Debug)]
pub struct FoundProof {
    pub context: Context,
    pub claim: Term,
    pub proof: Proof,
}

/// What one function cost to accept.
#[derive(Clone, Debug)]
pub struct ItemReport {
    pub name: String,
    /// Elaboration, including the search for every proof in it.
    pub elaborate_micros: u128,
    /// Lowering, checking by the kernel, and erasure.
    pub check_micros: u128,
}

pub struct Elaborated {
    /// Every item that was accepted, lowered, checked, and erased.
    pub session: Session,
    /// The accepted functions, in source order.
    pub functions: Vec<(String, FnRef)>,
    pub diagnostics: Vec<Diagnostic>,
    pub holes: Vec<HoleReport>,
    pub items: Vec<ItemReport>,
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
        items: Vec::new(),
        ctx: Context::new(),
        names: Vec::new(),
        facts: Vec::new(),
        loops: Vec::new(),
        labels: HashMap::new(),
        total: false,
    };

    env.declare_builtin_props();
    env.declare_builtin_lemmas();
    env.report_unchecked_syntax(program);

    let mut seen: HashMap<&str, Span> = HashMap::new();
    let mut duplicates = HashSet::new();
    for (index, declaration) in program.declarations.iter().enumerate() {
        let Some(name) = declared_name(declaration) else {
            continue;
        };
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
        let name = declared_name(&program.declarations[index]).expect("an impl mentions nothing");
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
        // An `impl` block was reported as not checked yet.
        let Some(name) = declared_name(declaration) else {
            continue;
        };
        let name = name.text.clone();
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
        items: env.items,
    }
}

impl Env<'_> {
    /// What S4 parses and no commit has given a meaning yet: the promises
    /// and `derive`, once per file each, and every `impl` block. Doc comments
    /// and visibility need no report, since ignoring them changes nothing a
    /// program says.
    fn report_unchecked_syntax(&mut self, program: &ast::Program) {
        let mut promise: Option<Span> = None;
        let mut derive: Option<Span> = None;
        let mut note = |attributes: &[ast::Attribute]| {
            for attribute in attributes {
                let slot = if attribute.kind.is_promise() {
                    &mut promise
                } else {
                    &mut derive
                };
                slot.get_or_insert(attribute.span);
            }
        };
        note(&program.attributes);
        for declaration in &program.declarations {
            note(&declaration.attributes);
            if let DeclarationKind::Impl { methods, .. } = &declaration.kind {
                for method in methods {
                    note(&method.attributes);
                }
            }
        }
        if let Some(span) = promise {
            self.diagnostics.push(Diagnostic::error(
                "L0290",
                "promises (`#[terminates]`, `#[no_panic]`, `#[no_alloc]`, `#[no_io]`) are parsed but not checked yet; E2 adds them",
                span,
            ));
        }
        if let Some(span) = derive {
            self.diagnostics.push(Diagnostic::error(
                "L0290",
                "`#[derive(...)]` is parsed but not checked yet; O1 adds it",
                span,
            ));
        }
        for declaration in &program.declarations {
            if let DeclarationKind::Impl { target, .. } = &declaration.kind {
                self.diagnostics.push(Diagnostic::error(
                    "L0290",
                    format!(
                        "`impl {}` is parsed but not checked yet; O4 adds impl blocks",
                        target.text()
                    ),
                    target.span,
                ));
            }
        }
    }

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
                    if variant.shape == ast::VariantShape::Struct {
                        return self.fail(
                            "L0290",
                            "a variant with named fields is not in Locus yet; E9 adds it",
                            variant.span,
                        );
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
                ..
            } => {
                let math = *mode == FunctionMode::Math;
                self.function(name, math, parameters, result, Body::Block(body), false)
            }
            DeclarationKind::Constant { name, ty, value } => {
                self.function(name, true, &[], ty, Body::Expr(value), true)
            }
            DeclarationKind::Prop {
                name,
                parameters,
                variants,
            } => self.prop(name, parameters, variants),
            DeclarationKind::Impl { .. } => {
                unreachable!("an impl block is reported before the declarations are elaborated")
            }
        }
    }

    fn function(
        &mut self,
        name: &ast::Name,
        math: bool,
        parameters: &[ast::Parameter],
        result: &ast::Type,
        body: Body<'_>,
        constant: bool,
    ) -> Elab<Global> {
        let started = std::time::Instant::now();
        self.start_item(math);
        let mut params: Vec<Binder> = Vec::new();
        for parameter in parameters {
            if params
                .iter()
                .any(|earlier| earlier.name == parameter.name.text)
            {
                let message = format!("parameter `{}` is declared twice", parameter.name.text);
                return self.fail("L0202", message, parameter.name.span);
            }
            let ty = self.ty(&parameter.ty)?;
            let binder = Binder::new(&parameter.name.text, ty);
            self.declare(&binder, false, parameter.span)?;
            params.push(binder);
        }
        let result_ty = self.ty(result)?;
        let block = match body {
            Body::Block(block) => self.block(block, Some(&result_ty))?.0,
            Body::Expr(value) => {
                let value = self.check(value, &result_ty)?;
                crate::typed::Block {
                    stmts: Vec::new(),
                    tail: Some(Box::new(value.expr)),
                }
            }
        };
        let item = FnItem {
            name: name.text.clone(),
            math,
            params: params.clone(),
            result: result_ty.clone(),
            body: block,
        };
        let elaborate_micros = started.elapsed().as_micros();
        let started = std::time::Instant::now();
        let reference = match self.session.declare_fn(&item) {
            Ok(reference) => reference,
            Err(error) => return self.internal(error, name.span),
        };
        self.items.push(ItemReport {
            name: name.text.clone(),
            elaborate_micros,
            check_micros: started.elapsed().as_micros(),
        });
        Ok(Global::Fn(Rc::new(FnInfo {
            reference,
            name: name.text.clone(),
            params,
            result: result_ty,
            constant,
        })))
    }

    fn prop(
        &mut self,
        name: &ast::Name,
        parameters: &[ast::Parameter],
        variants: &[ast::PropVariant],
    ) -> Elab<Global> {
        self.start_item(true);
        let mut params: Vec<Binder> = Vec::new();
        for parameter in parameters {
            let ty = self.ty(&parameter.ty)?;
            if matches!(ty, Type::Proof(_)) {
                return self.fail(
                    "L0225",
                    "a proposition's parameter is data or a `Prop`; evidence belongs in a variant",
                    parameter.ty.span,
                );
            }
            params.push(Binder::new(&parameter.name.text, ty));
        }
        let mut infos: Vec<PropVariantInfo> = Vec::new();
        let mut kernel_variants = Vec::new();
        for variant in variants {
            if infos
                .iter()
                .any(|earlier| earlier.name == variant.name.text)
            {
                let message = format!("variant `{}` is declared twice", variant.name.text);
                return self.fail("L0202", message, variant.name.span);
            }
            self.start_item(true);
            let fields = variant
                .fields
                .iter()
                .map(|field| (field.name.as_ref(), &field.ty, field.span));
            match &variant.target {
                None => {
                    // The parameters are in scope in the payload.
                    for param in &params {
                        self.declare(param, true, variant.span)?;
                    }
                    let payload = self.telescope(fields)?;
                    let mut telescope = params.clone();
                    telescope.extend(payload.iter().cloned());
                    kernel_variants.push(PropVariant::Params(tuple_over(&telescope)));
                    infos.push(PropVariantInfo {
                        name: variant.name.text.clone(),
                        payload,
                        conclusion: None,
                    });
                }
                Some(target) => {
                    let payload = self.telescope(fields)?;
                    let arguments = self.stated_conclusion(name, target)?;
                    let mut conclusion = Vec::new();
                    if arguments.len() != params.len() {
                        let message = format!(
                            "`{}` takes {} argument(s), and {} were given",
                            name.text,
                            params.len(),
                            arguments.len()
                        );
                        return self.fail("L0208", message, target.span);
                    }
                    for (argument, param) in arguments.iter().zip(&params) {
                        let value = self.check(argument, &param.ty)?;
                        conclusion.push(self.term(&value, argument.span)?);
                    }
                    let ids: Vec<_> = payload.iter().map(|binder| binder.id).collect();
                    let stated = conclusion.clone();
                    kernel_variants.push(PropVariant::indexed(tuple_over(&payload), |given| {
                        stated
                            .iter()
                            .map(|term| {
                                ids.iter()
                                    .zip(given)
                                    .fold(term.clone(), |term, (id, given)| {
                                        term.replace_var(*id, given)
                                    })
                            })
                            .collect()
                    }));
                    infos.push(PropVariantInfo {
                        name: variant.name.text.clone(),
                        payload,
                        conclusion: Some(conclusion),
                    });
                }
            }
        }
        let param_types = params.iter().map(|param| param.ty.clone()).collect();
        let id = match self.session.declare_prop(param_types, kernel_variants) {
            Ok(id) => id,
            Err(error) => return self.internal(error, name.span),
        };
        Ok(Global::Prop(Rc::new(PropInfo {
            id,
            name: name.text.clone(),
            params,
            variants: infos,
        })))
    }

    /// The arguments of `: @Name(arguments)` on a variant of `Name`.
    fn stated_conclusion<'e>(
        &mut self,
        name: &ast::Name,
        target: &'e ast::Expr,
    ) -> Elab<&'e [ast::Expr]> {
        let (callee, arguments): (&ast::Expr, &[ast::Expr]) = match &target.kind {
            ast::ExprKind::Call { callee, arguments } => (callee, arguments),
            _ => (target, &[]),
        };
        match &callee.kind {
            ast::ExprKind::Name(written) if written.text == name.text => Ok(arguments),
            _ => self.fail(
                "L0225",
                format!("a variant of `{0}` proves `{0}(...)`", name.text),
                target.span,
            ),
        }
    }

    /// The checked lemmas about `Int` and about every machine integer type,
    /// callable by name: what a step between two claims is written with,
    /// since a hole takes none by itself. The table is the theory's.
    pub(super) fn builtin_lemmas(&self) -> Vec<(&'static str, FnId)> {
        self.theory.lemma_names()
    }

    pub(super) fn declare_builtin_lemmas(&mut self) {
        for (name, id) in self.builtin_lemmas() {
            let signature = self
                .session
                .program()
                .definitions()
                .signature(id)
                .expect("the theory declared it");
            let Type::Fn(declared, _) = &signature else {
                unreachable!("a signature is a function type")
            };
            let mut params: Vec<Binder> = Vec::new();
            for index in 0..=declared.len() {
                let earlier: Vec<_> = params.iter().map(Binder::term).collect();
                let ty = crate::kernel::telescope_entry(&signature, index, &earlier)
                    .expect("the index is within the signature");
                params.push(Binder::new(&format!("x{index}"), ty));
            }
            let result = params.pop().expect("the result was pushed last").ty;
            self.globals.insert(
                name.to_string(),
                Global::Fn(Rc::new(FnInfo {
                    reference: FnRef::Math(id),
                    name: name.to_string(),
                    params,
                    result,
                    constant: false,
                })),
            );
        }
    }

    /// The propositions the language itself provides, under their source names.
    pub(super) fn declare_builtin_props(&mut self) {
        let prelude = self.prelude;
        let prop = |name: &str| Binder::new(name, Type::Prop);
        let evidence = |name: &str, of: &Binder| Binder::new(name, Type::proof(of.term()));
        let variant = |name: &str, payload: Vec<Binder>| PropVariantInfo {
            name: name.to_string(),
            payload,
            conclusion: None,
        };
        let (p, q) = (prop("p"), prop("q"));
        let and = vec![variant(
            "Intro",
            vec![evidence("left", &p), evidence("right", &q)],
        )];
        let or = vec![
            variant("Left", vec![evidence("left", &p)]),
            variant("Right", vec![evidence("right", &q)]),
        ];
        let builtins = [
            (
                "True",
                prelude.truth,
                Vec::new(),
                vec![variant("Intro", Vec::new())],
            ),
            ("False", prelude.falsehood, Vec::new(), Vec::new()),
            ("And", prelude.and, vec![p.clone(), q.clone()], and),
            ("Or", prelude.or, vec![p, q], or),
        ];
        for (name, id, params, variants) in builtins {
            self.globals.insert(
                name.to_string(),
                Global::Prop(Rc::new(PropInfo {
                    id,
                    name: name.to_string(),
                    params,
                    variants,
                })),
            );
        }
    }
}

enum Body<'a> {
    Block(&'a ast::Block),
    Expr(&'a ast::Expr),
}
