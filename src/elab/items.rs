//! Declarations: the driver of elaboration.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast::{self, AttributeKind, DeclarationKind, VisibilityScope};
use crate::diagnostic::{Applicability, Diagnostic, Suggestion};
use crate::erased::Visibilities;
use crate::exec::Promises;
use crate::kernel::theory;
use crate::kernel::{Context, Definitions, FnId, Proof, PropVariant, Term, Type};
use crate::source::{SourceFile, Span};
use crate::typed::{Binder, Derive, EnumItem, FnItem, FnRef, Session, StructItem, VariantItem};

use super::env::{
    Elab, EnumInfo, Env, FnInfo, Global, LOGICAL, PropInfo, PropVariantInfo, StructInfo,
    VariantInfo,
};
use super::order::{declared_name, dependency_order};
use super::types::tuple_over;

/// One `_`, `prove!`, or conversion of evidence, an obligation of an
/// operator under `no_panic`, or the range of a `for`: whether it was
/// filled, by which tier (`exact`, `computed`, `evaluation`, `arithmetic`,
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
    /// What each accepted item and field is marked with, for the printer.
    pub visibilities: Visibilities,
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

/// Elaborates a program, moves checked.
pub fn elaborate(source: &SourceFile, program: &ast::Program) -> Elaborated {
    elaborate_with(source, program, true)
}

/// Elaborates a program. `check_moves` is the test hook of `moves.rs`: `false`
/// skips the move analysis, so that a program with a use after a move is
/// printed as Rust and handed to rustc, which must reject it.
pub fn elaborate_with(
    source: &SourceFile,
    program: &ast::Program,
    check_moves: bool,
) -> Elaborated {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory is checked");
    let mut env = Env {
        source,
        session: Session::new(definitions),
        prelude,
        theory,
        types: HashMap::new(),
        values: HashMap::new(),
        failed: HashSet::new(),
        file_promises: Promises::default(),
        diagnostics: Vec::new(),
        holes: Vec::new(),
        items: Vec::new(),
        ctx: Context::new(),
        names: Vec::new(),
        facts: Vec::new(),
        loops: Vec::new(),
        labels: HashMap::new(),
        total: false,
        item_name: String::new(),
        promises: Promises::default(),
        formula: None,
        not_a_term: None,
        moves: super::moves::Moves::new(check_moves),
    };

    env.declare_builtin_props();
    env.declare_builtin_lemmas();
    env.report_unchecked_syntax(program);
    env.file_promises = env.promises_of(&program.attributes, Promises::default());

    // A name is declared once per namespace: a type and a value may share it.
    let mut seen: HashMap<(bool, &str), Span> = HashMap::new();
    let mut duplicates = HashSet::new();
    for (index, declaration) in program.declarations.iter().enumerate() {
        let Some(name) = declared_name(declaration) else {
            continue;
        };
        let is_type = matches!(
            declaration.kind,
            DeclarationKind::Struct { .. }
                | DeclarationKind::Enum { .. }
                | DeclarationKind::Prop { .. }
        );
        if let Some(first) = seen.get(&(is_type, name.text.as_str())) {
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
            seen.insert((is_type, &name.text), name.span);
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
                env.insert_global(name, global);
            }
            // Either reported, or a consequence of a failure that was.
            Err(()) => {
                env.failed.insert(name);
            }
        }
    }
    accepted.sort_by_key(|(index, _, _)| *index);
    let visibilities = env.export_boundary(program);
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
        visibilities,
    }
}

/// A visibility as Rust spells it, and as the printer writes it: `pub`,
/// `pub(crate)`, `pub(super)`, `pub(in path)`, and nothing for private,
/// which `pub(self)` also is.
fn spelled(visibility: Option<&ast::Visibility>) -> String {
    match visibility.map(|visibility| &visibility.scope) {
        None | Some(VisibilityScope::SelfModule) => String::new(),
        Some(VisibilityScope::Public) => "pub".into(),
        Some(VisibilityScope::Crate) => "pub(crate)".into(),
        Some(VisibilityScope::Super) => "pub(super)".into(),
        Some(VisibilityScope::In(path)) => format!("pub(in {})", path.text()),
    }
}

fn is_public(visibility: Option<&ast::Visibility>) -> bool {
    matches!(
        visibility,
        Some(ast::Visibility {
            scope: VisibilityScope::Public,
            ..
        })
    )
}

/// Why a Rust caller could supply a value of a type whose evidence does not
/// hold: the path into the value, `.1` or `.bounded` and so on, and what is
/// found there.
struct Forgery {
    path: String,
    reason: String,
}

impl Forgery {
    fn under(mut self, segment: &str) -> Self {
        self.path = format!("{segment}{}", self.path);
        self
    }

    /// The sentence: "`lock.bounded` is evidence".
    fn sentence(&self, root: &str) -> String {
        format!("`{root}{}` {}", self.path, self.reason)
    }

    /// " at `1.bounded`" for a path into a value that has no name, or
    /// nothing for an empty path.
    fn located(&self) -> String {
        if self.path.is_empty() {
            String::new()
        } else {
            format!(" at `{}`", self.path.trim_start_matches('.'))
        }
    }
}

impl Env<'_> {
    /// The export boundary (Target language: What is generated), checked
    /// once every item is declared, and what each accepted item and field
    /// is marked with, for the printer.
    ///
    /// Plain `pub` means exported to Rust, and a Rust caller can obtain a
    /// marker honestly, from any function that returns evidence, so the
    /// guarantee rests on what is exported taking no evidence:
    ///
    /// - A `pub` function may take no evidence, directly or inside a
    ///   parameter whose type a Rust caller could build or alter, which is a
    ///   tuple, an enum, or a struct with a `pub` field (`L0244`). It may be
    ///   `pub(crate)`, or any restricted form, which Rust never sees; or it
    ///   may take a validated type, a struct whose fields are all private,
    ///   which Rust can hold and cannot make or change.
    /// - A `pub` struct that carries evidence has every field private
    ///   (`L0245`): a `pub` evidence field could be written with a marker,
    ///   and a `pub` data field could be changed under the evidence that
    ///   speaks of it.
    ///
    /// Within one file everything is in scope, so a use of a private item
    /// is never an error here; and a proposition or a function of the logic
    /// has no runtime form and is emitted under no visibility at all.
    fn export_boundary(&mut self, program: &ast::Program) -> Visibilities {
        let mut visibilities = Visibilities::default();
        for declaration in &program.declarations {
            let Some(name) = declared_name(declaration) else {
                continue;
            };
            if self.failed.contains(&name.text) {
                continue;
            }
            match &declaration.kind {
                DeclarationKind::Struct { fields, .. } => {
                    let Some(Global::Struct(info)) = self.types.get(&name.text) else {
                        continue;
                    };
                    let info = Rc::clone(info);
                    visibilities.set_type(&name.text, &spelled(info.visibility.as_ref()));
                    for (binder, visibility) in info.fields.iter().zip(&info.field_visibility) {
                        visibilities.set_field(
                            &name.text,
                            &binder.name,
                            &spelled(visibility.as_ref()),
                        );
                    }
                    if is_public(info.visibility.as_ref()) {
                        self.check_exported_struct(&info, fields);
                    }
                }
                DeclarationKind::Enum { .. } => {
                    if let Some(Global::Enum(info)) = self.types.get(&name.text) {
                        visibilities.set_type(&name.text, &spelled(info.visibility.as_ref()));
                    }
                }
                DeclarationKind::Function { parameters, .. } => {
                    let Some(Global::Fn(info)) = self.values.get(&name.text) else {
                        continue;
                    };
                    let info = Rc::clone(info);
                    visibilities.set_value(&name.text, &spelled(info.visibility.as_ref()));
                    if is_public(info.visibility.as_ref()) {
                        self.check_exported_fn(&info, parameters);
                    }
                }
                DeclarationKind::Constant { .. } => {
                    if let Some(Global::Fn(info)) = self.values.get(&name.text) {
                        visibilities.set_value(&name.text, &spelled(info.visibility.as_ref()));
                    }
                }
                DeclarationKind::Prop { .. } | DeclarationKind::Impl { .. } => {}
            }
        }
        visibilities
    }

    /// Rule one: a `pub` function takes no evidence a Rust caller could
    /// supply.
    fn check_exported_fn(&mut self, info: &FnInfo, parameters: &[ast::Parameter]) {
        let Some(visibility) = &info.visibility else {
            return;
        };
        for (parameter, binder) in parameters.iter().zip(&info.params) {
            let Some(forgery) = self.forgery(&binder.ty) else {
                continue;
            };
            let name = &info.name;
            self.diagnostics.push(
                Diagnostic::error(
                    "L0244",
                    format!(
                        "`{name}` takes evidence and cannot be `pub`: a Rust caller could pass any marker, since {}",
                        forgery.sentence(&binder.name)
                    ),
                    visibility.span,
                )
                .label(parameter.span, "the evidence is taken here")
                .note("a marker cannot be made outside the generated code, and a Rust caller can obtain one honestly, from any function that returns evidence, and pass it on")
                .suggest(Suggestion {
                    message: "make it `pub(crate)`, visible throughout the generated crate and never to Rust, or take a validated type, a struct whose fields are all private, and check plain data at runtime".into(),
                    span: visibility.span,
                    replacement: "pub(crate)".into(),
                    applicability: Applicability::MaybeIncorrect,
                }),
            );
            return;
        }
    }

    /// Rule two: a `pub` struct that carries evidence has every field
    /// private.
    fn check_exported_struct(&mut self, info: &StructInfo, fields: &[ast::Field]) {
        let Some(visibility) = &info.visibility else {
            return;
        };
        if !info
            .fields
            .iter()
            .any(|field| self.carries_evidence(&field.ty))
        {
            return;
        }
        let speaking = self.speaking_field(info).map(|field| field.name.clone());
        for (field, binder) in fields.iter().zip(&info.fields) {
            let Some(field_visibility) = &field.visibility else {
                continue;
            };
            if matches!(field_visibility.scope, VisibilityScope::SelfModule) {
                continue;
            }
            let (item, name) = (&info.name, &binder.name);
            let message = match (self.forgery(&binder.ty), &speaking) {
                (Some(forgery), _) if forgery.path.is_empty() => format!(
                    "evidence in the `pub` field `{item}.{name}` of a `pub` struct can be forged by a Rust caller; make the field private"
                ),
                (Some(forgery), _) => format!(
                    "evidence in the `pub` field `{item}.{name}` of a `pub` struct can be forged by a Rust caller, since {}; make the field private",
                    forgery.sentence(name)
                ),
                (None, Some(speaking)) if speaking != name => format!(
                    "`{item}` carries evidence, in `{speaking}`, and its `pub` field `{name}` can be written by a Rust caller, under that evidence; make the field private"
                ),
                // A field the evidence of no sibling can speak of, such as
                // a validated struct: one valid value for another.
                (None, _) => continue,
            };
            self.diagnostics.push(
                Diagnostic::error("L0245", message, field_visibility.span)
                    .label(visibility.span, "the struct is exported to Rust here")
                    .note("a validated type has every field private: Rust can hold one and pass it back, and can neither make one nor change what its evidence speaks of"),
            );
        }
    }

    /// Whether a value of the type holds evidence anywhere in it.
    fn carries_evidence(&self, ty: &Type) -> bool {
        match ty {
            Type::Proof(_) => true,
            Type::Tuple(fields) => fields.iter().any(|field| self.carries_evidence(field)),
            Type::Struct(id) => self.struct_by_id(*id).is_some_and(|info| {
                info.fields
                    .iter()
                    .any(|field| self.carries_evidence(&field.ty))
            }),
            Type::Enum(id) => self.enum_by_id(*id).is_some_and(|info| {
                info.variants.iter().any(|variant| {
                    variant
                        .payload
                        .iter()
                        .any(|field| self.carries_evidence(&field.ty))
                })
            }),
            Type::Fn(_, result) => self.carries_evidence(result),
            Type::Bool | Type::U8 | Type::Machine(_) | Type::Prop | Type::Nat | Type::Int => false,
        }
    }

    /// A field of the struct whose type is evidence, or holds evidence
    /// where its claim can mention the fields beside it: a proof, or a tuple
    /// with one in it, whose type is written over the fields before it. The
    /// evidence inside a struct or an enum speaks of that type's own fields
    /// alone, so one valid value of it can stand in for another.
    fn speaking_field<'i>(&self, info: &'i StructInfo) -> Option<&'i Binder> {
        info.fields
            .iter()
            .find(|field| self.speaks_of_siblings(&field.ty))
    }

    fn speaks_of_siblings(&self, ty: &Type) -> bool {
        match ty {
            Type::Proof(_) => true,
            Type::Tuple(fields) => fields.iter().any(|field| self.speaks_of_siblings(field)),
            Type::Fn(_, result) => self.carries_evidence(result),
            Type::Struct(_)
            | Type::Enum(_)
            | Type::Bool
            | Type::U8
            | Type::Machine(_)
            | Type::Prop
            | Type::Nat
            | Type::Int => false,
        }
    }

    /// Why a Rust caller could supply a value of the type whose evidence
    /// does not hold, or `None` when it could not. Evidence itself can be
    /// any marker; a tuple or an enum is built from its parts; a struct is
    /// safe when every field is private, since a `pub` field is a way in,
    /// to write evidence or to change the data its siblings' evidence
    /// speaks of; and a function value could return a marker.
    fn forgery(&self, ty: &Type) -> Option<Forgery> {
        match ty {
            Type::Proof(_) => Some(Forgery {
                path: String::new(),
                reason: "is evidence".into(),
            }),
            Type::Tuple(fields) => fields.iter().enumerate().find_map(|(index, field)| {
                self.forgery(field)
                    .map(|forgery| forgery.under(&format!(".{index}")))
            }),
            Type::Struct(id) => {
                let info = self.struct_by_id(*id)?;
                if !info
                    .fields
                    .iter()
                    .any(|field| self.carries_evidence(&field.ty))
                {
                    return None;
                }
                let public: Vec<&Binder> = info
                    .fields
                    .iter()
                    .zip(&info.field_visibility)
                    .filter(|(_, visibility)| {
                        visibility.as_ref().is_some_and(|visibility| {
                            !matches!(visibility.scope, VisibilityScope::SelfModule)
                        })
                    })
                    .map(|(field, _)| field)
                    .collect();
                // A `pub` field that is itself a way in, then a `pub` data
                // field under the evidence of a sibling.
                if let Some(forgery) = public.iter().find_map(|field| {
                    Some(self.forgery(&field.ty)?.under(&format!(".{}", field.name)))
                }) {
                    return Some(forgery);
                }
                let speaking = self.speaking_field(&info)?;
                let field = public.iter().find(|field| field.name != speaking.name)?;
                Some(Forgery {
                    path: String::new(),
                    reason: format!(
                        "is a `{}`, which carries evidence, in `{}`, and whose field `{}` is `pub`, so that a Rust caller can change what the evidence speaks of",
                        info.name, speaking.name, field.name
                    ),
                })
            }
            Type::Enum(id) => {
                let info = self.enum_by_id(*id)?;
                info.variants.iter().find_map(|variant| {
                    let forgery =
                        variant
                            .payload
                            .iter()
                            .enumerate()
                            .find_map(|(index, field)| {
                                let position = if variant.named {
                                    field.name.clone()
                                } else {
                                    index.to_string()
                                };
                                Some(self.forgery(&field.ty)?.under(&format!(".{position}")))
                            })?;
                    Some(Forgery {
                        path: String::new(),
                        reason: format!(
                            "may be `{}::{}`, whose payload{} {}",
                            info.name,
                            variant.name,
                            forgery.located(),
                            forgery.reason
                        ),
                    })
                })
            }
            Type::Fn(_, result) => self.forgery(result).map(|forgery| Forgery {
                path: String::new(),
                reason: format!(
                    "is a function whose result{} {}",
                    forgery.located(),
                    forgery.reason
                ),
            }),
            Type::Bool | Type::U8 | Type::Machine(_) | Type::Prop | Type::Nat | Type::Int => None,
        }
    }
}

impl Env<'_> {
    /// What S4 parses and no commit has given a meaning yet: every `impl`
    /// block. Doc comments and visibility need no report, since ignoring
    /// them changes nothing a program says. A `derive` at the top of the
    /// file or on a method is out of place.
    fn report_unchecked_syntax(&mut self, program: &ast::Program) {
        self.refuse_derive(&program.attributes, "the file");
        for declaration in &program.declarations {
            if let DeclarationKind::Impl { methods, .. } = &declaration.kind {
                for method in methods {
                    self.refuse_derive(&method.attributes, "a method");
                }
            }
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

    /// The promises a list of attributes makes, added to `defaults`: the
    /// file's for a function, and none for the file itself. A promise is
    /// never inferred, and nothing takes one away. `decreases` is accepted
    /// as the bare promise until recursion arrives.
    fn promises_of(&mut self, attributes: &[ast::Attribute], defaults: Promises) -> Promises {
        let mut promises = defaults;
        for attribute in attributes {
            match &attribute.kind {
                AttributeKind::Terminates { decreases } => {
                    if let Some(measure) = decreases {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "L0290",
                                "`decreases` is parsed but not checked yet; the Recursion project adds it with recursion",
                                measure.span,
                            )
                            .note("until then a function that calls itself is rejected, and the attribute counts as the bare `#[terminates]`"),
                        );
                    }
                    promises.terminates = true;
                }
                AttributeKind::NoPanic => promises.no_panic = true,
                AttributeKind::NoAlloc => promises.no_alloc = true,
                AttributeKind::NoIo => promises.no_io = true,
                AttributeKind::Derive(_) => {}
            }
        }
        promises
    }

    /// A promise on an item that is not a function: nothing runs it.
    fn refuse_promises(&mut self, attributes: &[ast::Attribute], name: &ast::Name, what: &str) {
        for attribute in attributes {
            if attribute.kind.is_promise() {
                self.diagnostics.push(
                    Diagnostic::error(
                        "L0233",
                        format!(
                            "`#[{}]` is a promise about what a function does when it runs, and `{}` is {what}",
                            attribute.kind.name(),
                            name.text
                        ),
                        attribute.span,
                    )
                    .note("a promise goes before a `fn`, or at the top of the file as `#![...]` for every function in it"),
                );
            }
        }
    }

    /// `#[derive(...)]` where there is nothing to derive for: `what` is not
    /// a struct or an enum.
    fn refuse_derive(&mut self, attributes: &[ast::Attribute], what: &str) {
        for attribute in attributes {
            if let AttributeKind::Derive(_) = &attribute.kind {
                self.diagnostics.push(
                    Diagnostic::error(
                        "L0242",
                        format!("`#[derive(...)]` goes before a `struct` or an `enum`, and this is {what}"),
                        attribute.span,
                    )
                    .note("the traits a type may derive are `Clone`, `Copy`, `PartialEq`, `Eq`, and `Debug`"),
                );
            }
        }
    }

    /// The traits a struct or an enum derives (O1): each `#[derive(...)]`
    /// before it, read in order, from the closed list `Clone`, `Copy`,
    /// `PartialEq`, `Eq`, and `Debug`, none twice; `Copy` with `Clone` and
    /// `Eq` with `PartialEq`, as Rust demands (`L0242`). Each trait must
    /// hold of every field (`L0243`): a struct or enum field must derive it
    /// too, since the generated Rust would not compile otherwise; `Copy`
    /// needs `Copy` fields; and `PartialEq` and `Eq` need data with no
    /// logic-only part at any depth, since erased data has no equality at
    /// runtime (every `Proved` marker would compare equal, and lie). A
    /// type with no runtime form is `Copy`, `Clone`, and `Debug` (the
    /// marker prints as its name). `fields` are the field groups: one for a
    /// struct, one per variant for an enum, each named for a message.
    fn derives(
        &mut self,
        attributes: &[ast::Attribute],
        type_name: &str,
        fields: &[(String, &[Binder])],
    ) -> Elab<Vec<Derive>> {
        let mut derives: Vec<(Derive, Span)> = Vec::new();
        let mut failed = false;
        let closed =
            "the traits a type may derive are `Clone`, `Copy`, `PartialEq`, `Eq`, and `Debug`";
        for attribute in attributes {
            let AttributeKind::Derive(paths) = &attribute.kind else {
                continue;
            };
            for path in paths {
                let name = path.text();
                let Some(derive) = path.single().and_then(|name| Derive::from_name(&name.text))
                else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "L0242",
                            format!("`{name}` cannot be derived"),
                            path.span,
                        )
                        .note(closed),
                    );
                    failed = true;
                    continue;
                };
                if let Some((_, first)) = derives.iter().find(|(earlier, _)| *earlier == derive) {
                    self.diagnostics.push(
                        Diagnostic::error("L0242", format!("`{name}` is derived twice"), path.span)
                            .label(*first, "first derived here"),
                    );
                    failed = true;
                    continue;
                }
                derives.push((derive, path.span));
            }
        }
        for (needs, needed) in [
            (Derive::Copy, Derive::Clone),
            (Derive::Eq, Derive::PartialEq),
        ] {
            if let Some((_, span)) = derives.iter().find(|(derive, _)| *derive == needs)
                && !derives.iter().any(|(derive, _)| *derive == needed)
            {
                self.diagnostics.push(
                    Diagnostic::error(
                        "L0242",
                        format!(
                            "the trait bound `{type_name}: {}` is not satisfied, and `{}` requires it",
                            needed.name(),
                            needs.name()
                        ),
                        *span,
                    )
                    .note(format!(
                        "write `#[derive({}, {})]`; a type is `{}` only if it is `{}`, as in Rust",
                        needed.name(),
                        needs.name(),
                        needs.name(),
                        needed.name()
                    )),
                );
                failed = true;
            }
        }
        for (derive, span) in &derives {
            for (group, binders) in fields {
                for (index, field) in binders.iter().enumerate() {
                    let field_name = if field.name == "_" {
                        format!("{group}.{index}")
                    } else {
                        format!("{group}.{}", field.name)
                    };
                    let shown = self.show_type(&field.ty);
                    let why = if matches!(derive, Derive::PartialEq | Derive::Eq)
                        && let Some(inner) = self.logic_only_data(&field.ty)
                    {
                        Some((
                            format!(
                                "`{field_name}{inner}` is logic-only data, which has no equality at runtime"
                            ),
                            "erased data has no equality at runtime: every `Proved` marker would compare equal, and lie",
                        ))
                    } else if !self.derives_trait(&field.ty, *derive) {
                        Some((
                            format!(
                                "field `{field_name}` is `{shown}`, which does not derive `{}`",
                                derive.name()
                            ),
                            "a struct or an enum has a trait only by deriving it, and its fields must have it too",
                        ))
                    } else {
                        None
                    };
                    if let Some((message, note)) = why {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "L0243",
                                format!(
                                    "`{type_name}` cannot derive `{}`: {message}",
                                    derive.name()
                                ),
                                *span,
                            )
                            .note(note),
                        );
                        failed = true;
                    }
                }
            }
        }
        if failed {
            return Err(());
        }
        Ok(derives.into_iter().map(|(derive, _)| derive).collect())
    }

    /// A fresh function scope over the declarations accepted so far.
    fn start_item(&mut self, name: &str, total: bool, promises: Promises) {
        let definitions = self.session.program().definitions().clone();
        self.ctx = Context::with_definitions(Rc::new(definitions));
        self.names.clear();
        self.facts.clear();
        self.loops.clear();
        self.total = total;
        self.item_name = name.to_string();
        self.promises = promises;
        self.formula = None;
        self.moves.start_item();
    }

    fn declaration(&mut self, declaration: &ast::Declaration) -> Elab<Global> {
        let attributes = &declaration.attributes;
        match &declaration.kind {
            DeclarationKind::Struct { name, fields } => {
                self.refuse_promises(attributes, name, "a struct");
                self.start_item(&name.text, true, LOGICAL);
                let field_visibility: Vec<Option<ast::Visibility>> = fields
                    .iter()
                    .map(|field| field.visibility.clone())
                    .collect();
                let fields = self.telescope(
                    fields
                        .iter()
                        .map(|field| (Some(&field.name), &field.ty, field.span)),
                )?;
                let derives =
                    self.derives(attributes, &name.text, &[(name.text.clone(), &fields)])?;
                let item = StructItem {
                    name: name.text.clone(),
                    fields: fields.clone(),
                    derives: derives.clone(),
                };
                let id = match self.session.declare_struct(&item) {
                    Ok(id) => id,
                    Err(error) => return self.internal(error, name.span),
                };
                Ok(Global::Struct(Rc::new(StructInfo {
                    id,
                    name: name.text.clone(),
                    fields,
                    derives,
                    visibility: declaration.visibility.clone(),
                    field_visibility: field_visibility.clone(),
                })))
            }
            DeclarationKind::Enum { name, variants } => {
                self.refuse_promises(attributes, name, "an enum");
                let mut items: Vec<VariantInfo> = Vec::new();
                for variant in variants {
                    if items
                        .iter()
                        .any(|earlier| earlier.name == variant.name.text)
                    {
                        let message = format!("variant `{}` is declared twice", variant.name.text);
                        return self.fail("L0202", message, variant.name.span);
                    }
                    self.start_item(&name.text, true, LOGICAL);
                    let payload = self.telescope(
                        variant
                            .fields
                            .iter()
                            .map(|field| (field.name.as_ref(), &field.ty, field.span)),
                    )?;
                    items.push(VariantInfo {
                        name: variant.name.text.clone(),
                        payload,
                        named: variant.shape == ast::VariantShape::Struct,
                    });
                }
                let groups: Vec<(String, &[Binder])> = items
                    .iter()
                    .map(|variant| {
                        (
                            format!("{}::{}", name.text, variant.name),
                            variant.payload.as_slice(),
                        )
                    })
                    .collect();
                let derives = self.derives(attributes, &name.text, &groups)?;
                let item = EnumItem {
                    name: name.text.clone(),
                    variants: items
                        .iter()
                        .map(|variant| VariantItem {
                            name: variant.name.clone(),
                            payload: variant.payload.clone(),
                            named: variant.named,
                        })
                        .collect(),
                    derives: derives.clone(),
                };
                let id = match self.session.declare_enum(&item) {
                    Ok(id) => id,
                    Err(error) => return self.internal(error, name.span),
                };
                Ok(Global::Enum(Rc::new(EnumInfo {
                    id,
                    name: name.text.clone(),
                    variants: items,
                    derives,
                    visibility: declaration.visibility.clone(),
                })))
            }
            DeclarationKind::Function {
                name,
                parameters,
                result,
                body,
                ..
            } => {
                self.refuse_derive(attributes, "a function");
                let promises = self.promises_of(attributes, self.file_promises);
                let takes_mut = parameters.iter().any(|parameter| {
                    matches!(parameter.ty.kind, ast::TypeKind::Ref { mutable: true, .. })
                });
                let function = Function {
                    promises,
                    takes_mut,
                    body: Body::Block(body),
                    constant: false,
                    visibility: declaration.visibility.clone(),
                };
                self.function(name, parameters, result, function)
            }
            DeclarationKind::Constant { name, ty, value } => {
                self.refuse_promises(attributes, name, "a constant");
                self.refuse_derive(attributes, "a constant");
                let function = Function {
                    promises: LOGICAL,
                    takes_mut: false,
                    body: Body::Expr(value),
                    constant: true,
                    visibility: declaration.visibility.clone(),
                };
                self.function(name, &[], ty, function)
            }
            DeclarationKind::Prop {
                name,
                parameters,
                variants,
            } => {
                self.refuse_promises(attributes, name, "a proposition");
                self.refuse_derive(attributes, "a proposition");
                self.prop(name, parameters, variants)
            }
            DeclarationKind::Impl { .. } => {
                unreachable!("an impl block is reported before the declarations are elaborated")
            }
        }
    }

    fn function(
        &mut self,
        name: &ast::Name,
        parameters: &[ast::Parameter],
        result: &ast::Type,
        function: Function<'_>,
    ) -> Elab<Global> {
        let Function {
            promises,
            takes_mut,
            body,
            constant,
            visibility,
        } = function;
        let started = std::time::Instant::now();
        // A function that may appear in a proposition is a function of the
        // logic: its body is a kernel term, total by construction, and
        // nothing in it may fail to return.
        let mut logical = super::env::first_broken(LOGICAL, promises).is_none() && !takes_mut;
        let (diagnostics, holes) = (self.diagnostics.len(), self.holes.len());
        self.not_a_term = None;
        let mut elaborated =
            self.function_body(name, parameters, result, body, logical, promises, constant);
        // The interim rule of LOC-193: a function that makes every promise
        // of the logic and whose body is not a kernel term, because it has
        // an operator that may panic in it, is checked as an ordinary
        // function with its promises, so that the checker enforces
        // `no_panic` on the operator, and is known by its contract only.
        let mut not_a_term = None;
        if elaborated.is_err()
            && logical
            && let Some((what, span)) = self.not_a_term.take()
        {
            self.diagnostics.truncate(diagnostics);
            self.holes.truncate(holes);
            let (line, _) = self.source.line_column(span.start).unwrap_or((0, 0));
            not_a_term = Some((what, line));
            logical = false;
            elaborated =
                self.function_body(name, parameters, result, body, logical, promises, constant);
        }
        let (params, result_ty, block) = elaborated?;
        let item = FnItem {
            name: name.text.clone(),
            math: logical,
            params: params.clone(),
            result: result_ty.clone(),
            body: block,
        };
        let elaborate_micros = started.elapsed().as_micros();
        let started = std::time::Instant::now();
        // The checker enforces the promises of an ordinary function; a
        // function of the logic keeps them by construction.
        let declared = if constant {
            self.session.declare_constant(&item, promises)
        } else {
            self.session.declare_fn_promising(&item, promises)
        };
        let reference = match declared {
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
            promises,
            takes_mut,
            not_a_term,
            visibility,
        })))
    }

    /// The signature and the body, elaborated in a fresh scope: as a term of
    /// the logic when `logical`, as code otherwise.
    #[allow(clippy::too_many_arguments)]
    fn function_body(
        &mut self,
        name: &ast::Name,
        parameters: &[ast::Parameter],
        result: &ast::Type,
        body: Body<'_>,
        logical: bool,
        promises: Promises,
        constant: bool,
    ) -> Elab<(Vec<Binder>, Type, crate::typed::Block)> {
        self.start_item(&name.text, logical, promises);
        if constant {
            self.formula = Some("the value of a constant");
        }
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
                let checked = self.check(value, &result_ty)?;
                // Rust computes a constant itself, and does not call to.
                if let Some(callee) = self.called_by_constant(&checked.expr) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "L0234",
                            format!("the value of `{}` calls `{callee}`, and Rust computes a constant without calling a function", name.text),
                            value.span,
                        )
                        .note("the value of a constant is a literal, a cast, a comparison, or a tuple, struct, or variant of those, and may name another constant; a value a function computes is a function of no parameters, `fn name() -> T { .. }`"),
                    );
                    return Err(());
                }
                crate::typed::Block {
                    stmts: Vec::new(),
                    tail: Some(Box::new(checked.expr)),
                }
            }
        };
        Ok((params, result_ty, block))
    }

    fn prop(
        &mut self,
        name: &ast::Name,
        parameters: &[ast::Parameter],
        variants: &[ast::PropVariant],
    ) -> Elab<Global> {
        // The whole declaration is a proposition: its parameters, its payloads,
        // and the arguments each variant proves it at.
        self.start_item(&name.text, true, LOGICAL);
        self.formula = Some("a proposition");
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
            self.start_item(&name.text, true, LOGICAL);
            self.formula = Some("a proposition");
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
                        named: variant.shape == ast::VariantShape::Struct,
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
                        named: variant.shape == ast::VariantShape::Struct,
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
            self.values.insert(
                name.to_string(),
                Global::Fn(Rc::new(FnInfo {
                    reference: FnRef::Math(id),
                    name: name.to_string(),
                    params,
                    result,
                    constant: false,
                    // A lemma is a function of the logic and does nothing.
                    promises: Promises {
                        terminates: true,
                        no_panic: true,
                        no_alloc: true,
                        no_io: true,
                    },
                    takes_mut: false,
                    not_a_term: None,
                    visibility: None,
                })),
            );
        }
    }

    /// The first function the value of a constant calls that is not another
    /// constant, by name. Rust's `const` evaluator computes literals, casts,
    /// comparisons, primitive methods, tuples, structs, variants, fields,
    /// and conditionals of those, and does not call a function; the ghost
    /// parts of the value, which are erased, do not count.
    fn called_by_constant(&self, expr: &crate::typed::Expr) -> Option<String> {
        use crate::typed::{Expr, Stmt};
        let in_block = |block: &crate::typed::Block| {
            block
                .stmts
                .iter()
                .find_map(|stmt| match stmt {
                    Stmt::Let { value, .. } | Stmt::Expr(value) | Stmt::Assign { value, .. } => {
                        self.called_by_constant(value)
                    }
                })
                .or_else(|| {
                    block
                        .tail
                        .as_deref()
                        .and_then(|tail| self.called_by_constant(tail))
                })
        };
        let first = |exprs: &[Expr]| exprs.iter().find_map(|expr| self.called_by_constant(expr));
        match expr {
            Expr::CallMath { id, name, ty, .. } => {
                if matches!(ty, Type::Prop | Type::Proof(_)) {
                    return None;
                }
                let constant = self.values.values().any(|global| match global {
                    Global::Fn(info) => info.constant && info.reference == FnRef::Math(*id),
                    _ => false,
                });
                (!constant).then(|| name.clone())
            }
            Expr::CallFn { name, .. } => Some(name.clone()),
            Expr::Tuple { fields, .. }
            | Expr::Variant {
                payload: fields, ..
            } => first(fields),
            Expr::Struct { fields, .. } => fields
                .iter()
                .find_map(|(_, value)| self.called_by_constant(value)),
            Expr::Field { target, .. } | Expr::Cast { expr: target, .. } => {
                self.called_by_constant(target)
            }
            Expr::Method {
                receiver,
                arguments,
                ..
            } => self
                .called_by_constant(receiver)
                .or_else(|| first(arguments)),
            Expr::Compare { left, right, .. } => self
                .called_by_constant(left)
                .or_else(|| self.called_by_constant(right)),
            Expr::If {
                condition,
                then_block,
                else_block,
                ..
            } => self
                .called_by_constant(condition)
                .or_else(|| in_block(then_block))
                .or_else(|| in_block(else_block)),
            Expr::Match {
                scrutinee, arms, ..
            } => self
                .called_by_constant(scrutinee)
                .or_else(|| arms.iter().find_map(|arm| in_block(&arm.body))),
            Expr::Block(block) => in_block(block),
            _ => None,
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
            named: false,
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
            self.types.insert(
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

#[derive(Clone, Copy)]
enum Body<'a> {
    Block(&'a ast::Block),
    Expr(&'a ast::Expr),
}

/// What a `fn` or a `const` says about itself besides its signature.
struct Function<'a> {
    promises: Promises,
    takes_mut: bool,
    body: Body<'a>,
    /// Declared with `const`: used by name, without a call.
    constant: bool,
    /// `pub` or a restricted form, as written; private without one.
    visibility: Option<ast::Visibility>,
}
