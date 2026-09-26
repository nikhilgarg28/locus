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
use crate::typed::{
    Binder, Derive, EnumItem, FnItem, FnRef, Passing, Session, StructItem, VariantItem,
};

use super::env::{
    Elab, EnumInfo, Env, FnInfo, Global, LOGICAL, PropInfo, PropVariantInfo, StructInfo,
    VariantInfo,
};
use super::order::{Unit, declared_name, dependency_order, unit_name, units};
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
    /// Opt-in raw search-stage and final kernel-check timings.
    pub measurements: Vec<crate::measurement::Sample>,
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
    /// No error was reported; warnings do not count.
    pub fn is_success(&self) -> bool {
        !self.diagnostics.iter().any(Diagnostic::is_error)
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
    elaborate_with_options(
        source,
        program,
        &super::Options {
            check_moves,
            ..super::Options::default()
        },
    )
}

pub fn elaborate_with_options(
    source: &SourceFile,
    program: &ast::Program,
    options: &super::Options,
) -> Elaborated {
    if crate::project::specs::present(program) {
        let (resolved, graph, diagnostics) =
            crate::project::resolve_inline(program.clone(), source);
        if !diagnostics.is_empty() {
            let mut result = elaborate_with_options(source, &ast::Program::default(), options);
            result.diagnostics.extend(diagnostics);
            return result;
        }
        let options = super::Options {
            module_access: Some(std::sync::Arc::new(graph.access.clone())),
            ..options.clone()
        };
        let mut result = elaborate_with_options(source, &resolved, &options);
        // The flat API's entry lookup continues to use source-level root names.
        for (name, _) in &mut result.functions {
            if let Some(item) = graph
                .items
                .iter()
                .find(|i| i.canonical == *name && i.module == graph.package_roots[0])
            {
                *name = item.original.clone();
            }
        }
        return result;
    }
    let (mut definitions, prelude) = Definitions::with_prelude_for(options.pointer_width);
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory is checked");
    let mut session = Session::with_pointer_width(definitions, options.pointer_width);
    let natural = super::naturals::declare(&mut session);
    let mut env = Env {
        pointer_width: options.pointer_width,
        module_access: options.module_access.clone(),
        models: super::models::primitive_models(Type::Struct(natural.id)),
        quantifiers: Vec::new(),
        closure_capture_boundary: None,
        explicit_model_depth: 0,
        layout_hints: HashMap::new(),
        source,
        previews: options.previews.clone(),
        session,
        prelude,
        theory,
        types: HashMap::from([("Nat".into(), Global::Struct(natural))]),
        values: HashMap::new(),
        failed: HashSet::new(),
        file_promises: Promises::default(),
        diagnostics: crate::limits::DiagnosticBuffer::default(),
        normalization_exhausted: false,
        holes: Vec::new(),
        items: Vec::new(),
        ctx: Context::new(),
        names: Vec::new(),
        facts: Vec::new(),
        loops: Vec::new(),
        returns: None,
        recursion: None,
        never_fns: HashSet::new(),
        labels: HashMap::new(),
        total: false,
        in_constant: false,
        suppress_models: 0,
        item_name: String::new(),
        promises: Promises::default(),
        formula: None,
        moves: super::moves::Moves::new(options.check_moves),
        exits: Vec::new(),
        borrowed: Vec::new(),
        owner: None,
    };

    env.declare_builtin_props();
    env.declare_builtin_lemmas();
    let (specialized, diagnostics, quantifiers, access) =
        super::generics::specialize(program, &options.previews, options.module_access.as_deref());
    env.module_access = access.map(std::sync::Arc::new);
    env.diagnostics.extend(diagnostics);
    let program = &specialized;
    env.report_unchecked_syntax(program);
    env.file_promises = env.promises_of(&program.attributes, Promises::default());

    let units = units(program);
    let names: Vec<Option<String>> = units.iter().map(unit_name).collect();
    let mut duplicates = env.refuse_bad_impls(program, &units);

    // A name is declared once per namespace: a type and a value may share
    // it. A function of an `impl` block is `Type::name`, which a variant
    // of the same enum is too, in Rust's value namespace.
    let mut seen: HashMap<(bool, &str), Span> = HashMap::new();
    for (index, (unit, qualified)) in units.iter().zip(&names).enumerate() {
        let (Some(name), Some(qualified)) = (declared_name(unit.declaration), qualified) else {
            continue;
        };
        let is_type = matches!(
            unit.declaration.kind,
            DeclarationKind::Struct { .. }
                | DeclarationKind::Enum { .. }
                | DeclarationKind::Prop { .. }
        );
        if is_type && qualified == "Nat" {
            env.diagnostics.push(Diagnostic::error(
                "L0202",
                "`Nat` is a built-in logical type and cannot be redeclared",
                name.span,
            ));
            duplicates.insert(index);
            continue;
        }
        let constructor = matches!(
            unit.declaration.kind,
            DeclarationKind::Struct {
                shape: ast::VariantShape::Tuple | ast::VariantShape::Unit,
                ..
            }
        );
        for is_type in if constructor {
            vec![true, false]
        } else {
            vec![is_type]
        } {
            let earlier = seen
                .get(&(is_type, qualified.as_str()))
                .copied()
                .or_else(|| {
                    unit.owner
                        .and_then(|owner| variant_named(program, owner, name))
                });
            if let Some(first) = earlier {
                env.diagnostics.push(
                    Diagnostic::error(
                        "L0202",
                        format!("`{qualified}` is declared twice"),
                        name.span,
                    )
                    .label(first, "first declared here"),
                );
                duplicates.insert(index);
            } else {
                seen.insert((is_type, qualified), name.span);
            }
        }
    }

    let (order, cyclic) = dependency_order(&units, &quantifiers, env.module_access.as_deref());
    for index in cyclic {
        let name = declared_name(units[index].declaration).expect("a unit has a name");
        let qualified = names[index].clone().expect("a unit has a name");
        env.diagnostics.push(
            Diagnostic::error(
                "L0203",
                format!("`{qualified}` is defined in terms of itself"),
                name.span,
            )
            .note("recursion is not part of the core language; a bounded `for` repeats a step a known number of times"),
        );
        env.failed.insert(qualified);
    }
    let mut accepted: Vec<(usize, String, FnRef)> = Vec::new();
    for group in order {
        if group.len() > 1 {
            if group.iter().any(|index| duplicates.contains(index)) {
                for index in group {
                    if let Some(name) = &names[index] {
                        env.failed.insert(name.clone());
                    }
                }
                continue;
            }
            let declarations: Vec<_> = group
                .iter()
                .map(|index| units[*index].declaration)
                .collect();
            for declaration in &declarations {
                let name = declared_name(declaration).expect("logical enum group");
                env.refuse_promises(&declaration.attributes, name, "an enum");
            }
            match env.logical_enum_group(&declarations) {
                Ok(globals) => {
                    for (name, global) in globals {
                        env.insert_global(name, global);
                    }
                }
                Err(()) => {
                    for index in group {
                        if let Some(name) = &names[index] {
                            env.failed.insert(name.clone());
                        }
                    }
                }
            }
            continue;
        }
        let index = group[0];
        if duplicates.contains(&index) {
            continue;
        }
        let unit = &units[index];
        let Some(name) = names[index].clone() else {
            continue;
        };
        match env.unit(unit) {
            Ok(global) => {
                if let Global::Fn(info) = &global {
                    if let Some(model) = unit.model
                        && env.register_model(model, Rc::clone(info)).is_err()
                    {
                        env.failed.insert(name);
                        continue;
                    }
                    accepted.push((index, name.clone(), info.reference));
                }
                env.insert_global(name, global);
                env.register_quantifiers(&quantifiers);
            }
            // Either reported, or a consequence of a failure that was.
            Err(()) => {
                env.failed.insert(name);
            }
        }
    }
    accepted.sort_by_key(|(index, _, _)| *index);
    let visibilities = env.export_boundary(&units);
    env.diagnostics
        .sort_by_key(|diagnostic| diagnostic.labels[0].span.start);
    Elaborated {
        session: env.session,
        functions: accepted
            .into_iter()
            .map(|(_, name, reference)| (name, reference))
            .collect(),
        diagnostics: env.diagnostics.into_vec(),
        holes: env.holes,
        items: env.items,
        visibilities,
    }
}

/// The name a declaration declares, by its kind.
fn declared_name_of(kind: &DeclarationKind) -> Option<&str> {
    match kind {
        DeclarationKind::Foreign { name, .. }
        | DeclarationKind::Function { name, .. }
        | DeclarationKind::Struct { name, .. }
        | DeclarationKind::Enum { name, .. }
        | DeclarationKind::Prop { name, .. }
        | DeclarationKind::Constant { name, .. } => Some(&name.text),
        DeclarationKind::Impl { .. }
        | DeclarationKind::SpecImpl { .. }
        | DeclarationKind::AssociatedType { .. }
        | DeclarationKind::Trait { .. }
        | DeclarationKind::Spec { .. }
        | DeclarationKind::ModuleImpl { .. }
        | DeclarationKind::Module { .. }
        | DeclarationKind::Use { .. }
        | DeclarationKind::ImportedModule { .. }
        | DeclarationKind::RustImport { .. } => None,
    }
}

/// The span of the variant `name` of the enum `owner`, when the file
/// declares one: a function of `impl Enum` may not take a variant's name,
/// since Rust files both under `Enum::name`.
fn variant_named(program: &ast::Program, owner: &ast::Path, name: &ast::Name) -> Option<Span> {
    program
        .declarations
        .iter()
        .find_map(|declaration| match &declaration.kind {
            DeclarationKind::Enum {
                name: enum_name,
                variants,
                ..
            } if enum_name.text == owner.text() => variants
                .iter()
                .find(|variant| variant.name.text == name.text)
                .map(|variant| variant.name.span),
            _ => None,
        })
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
    fn export_boundary(&mut self, units: &[Unit<'_>]) -> Visibilities {
        let mut visibilities = Visibilities::default();
        for unit in units {
            let declaration = unit.declaration;
            let Some(qualified) = unit_name(unit) else {
                continue;
            };
            if self.failed.contains(&qualified) {
                continue;
            }
            let name = ast::Name {
                text: qualified,
                span: declared_name(declaration).expect("a unit has a name").span,
            };
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
                    if self.module_access.is_none() && is_public(info.visibility.as_ref()) {
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
                    if self.module_access.is_none() && is_public(info.visibility.as_ref()) {
                        self.check_exported_fn(&info, parameters);
                    }
                }
                DeclarationKind::Constant { .. } => {
                    if let Some(Global::Fn(info)) = self.values.get(&name.text) {
                        visibilities.set_value(&name.text, &spelled(info.visibility.as_ref()));
                    }
                }
                DeclarationKind::Prop { .. }
                | DeclarationKind::Impl { .. }
                | DeclarationKind::SpecImpl { .. }
                | DeclarationKind::AssociatedType { .. }
                | DeclarationKind::Trait { .. }
                | DeclarationKind::Spec { .. }
                | DeclarationKind::ModuleImpl { .. }
                | DeclarationKind::Module { .. }
                | DeclarationKind::Use { .. }
                | DeclarationKind::ImportedModule { .. }
                | DeclarationKind::RustImport { .. }
                | DeclarationKind::Foreign { .. } => {}
            }
        }
        visibilities
    }

    /// Rule one: a `pub` function takes no evidence a Rust caller could
    /// supply.
    fn check_exported_fn(&mut self, info: &FnInfo, parameters: &[ast::Parameter]) {
        // Explicit logic definitions have no callable Rust export.
        if info.logical {
            return;
        }
        let Some(visibility) = &info.visibility else {
            return;
        };
        // The receiver of a method is the type itself, and is not written
        // among the parameters.
        let params = info.params.iter().skip(usize::from(info.receiver));
        for (parameter, binder) in parameters.iter().zip(params) {
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
        self.carries_evidence_seen(ty, &mut Vec::new())
    }

    fn carries_evidence_seen(&self, ty: &Type, seen: &mut Vec<Type>) -> bool {
        if matches!(ty, Type::Struct(_) | Type::Enum(_)) {
            if seen.contains(ty) {
                return false;
            }
            seen.push(ty.clone());
        }
        match ty {
            Type::Instance(base, _) => self.carries_evidence_seen(base, seen),
            Type::Boxed(element) | Type::Buffer(element) => {
                self.carries_evidence_seen(element, seen)
            }
            Type::Proof(_) => true,
            Type::Tuple(fields) => fields
                .iter()
                .any(|field| self.carries_evidence_seen(field, seen)),
            Type::Struct(id) => self.struct_by_id(*id).is_some_and(|info| {
                info.fields
                    .iter()
                    .any(|field| self.carries_evidence_seen(&field.ty, seen))
            }),
            Type::Enum(id) => self.enum_by_id(*id).is_some_and(|info| {
                info.variants.iter().any(|variant| {
                    variant
                        .payload
                        .iter()
                        .any(|field| self.carries_evidence_seen(&field.ty, seen))
                })
            }),
            Type::Fn(_, result) => self.carries_evidence_seen(result, seen),
            Type::Bool | Type::U8 | Type::Machine(_) | Type::Prop | Type::Int => false,
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
            Type::Instance(base, _) => self.carries_evidence(base),
            Type::Boxed(element) | Type::Buffer(element) => self.speaks_of_siblings(element),
            Type::Proof(_) => true,
            Type::Tuple(fields) => fields.iter().any(|field| self.speaks_of_siblings(field)),
            Type::Fn(_, result) => self.carries_evidence(result),
            Type::Struct(_)
            | Type::Enum(_)
            | Type::Bool
            | Type::U8
            | Type::Machine(_)
            | Type::Prop
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
        self.forgery_seen(ty, &mut Vec::new())
    }

    fn forgery_seen(&self, ty: &Type, seen: &mut Vec<Type>) -> Option<Forgery> {
        if matches!(ty, Type::Struct(_) | Type::Enum(_)) {
            if seen.contains(ty) {
                return None;
            }
            seen.push(ty.clone());
        }
        match ty {
            Type::Instance(_, _) => Some(Forgery {
                path: String::new(),
                reason: "is indexed by erased logical arguments that Rust cannot distinguish"
                    .into(),
            }),
            Type::Boxed(element) | Type::Buffer(element) => self
                .forgery_seen(element, seen)
                .map(|forgery| forgery.under("[]")),
            Type::Proof(_) => Some(Forgery {
                path: String::new(),
                reason: "is evidence".into(),
            }),
            Type::Tuple(fields) => fields.iter().enumerate().find_map(|(index, field)| {
                self.forgery_seen(field, seen)
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
                    Some(
                        self.forgery_seen(&field.ty, seen)?
                            .under(&format!(".{}", field.name)),
                    )
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
                                Some(
                                    self.forgery_seen(&field.ty, seen)?
                                        .under(&format!(".{position}")),
                                )
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
            Type::Fn(_, result) => self.forgery_seen(result, seen).map(|forgery| Forgery {
                path: String::new(),
                reason: format!(
                    "is a function whose result{} {}",
                    forgery.located(),
                    forgery.reason
                ),
            }),
            Type::Bool | Type::U8 | Type::Machine(_) | Type::Prop | Type::Int => None,
        }
    }
}

impl Env<'_> {
    /// What is parsed and has no meaning: a `derive` at the top of the
    /// file or on a method is out of place. Doc comments need no report,
    /// since ignoring them changes nothing a program says.
    fn report_unchecked_syntax(&mut self, program: &ast::Program) {
        self.refuse_derive(&program.attributes, "the file");
        for declaration in &program.declarations {
            for attribute in &declaration.attributes {
                if let ast::AttributeKind::Derive(paths) = &attribute.kind {
                    for path in paths.iter().filter(|path| path.text() == "Model") {
                        if let DeclarationKind::Struct { name, .. } = &declaration.kind {
                            let generated = format!("{}Model", name.text);
                            if program.declarations.iter().any(|other| {
                                declared_name_of(&other.kind).is_some_and(|name| name == generated)
                            }) {
                                self.diagnostics.push(Diagnostic::error(
                                    "L0282",
                                    format!("derived model name `{generated}` is already declared"),
                                    path.span,
                                ));
                            }
                        } else {
                            self.diagnostics.push(Diagnostic::error(
                                "L0282",
                                "Model can currently be derived only for a physical struct",
                                path.span,
                            ));
                        }
                    }
                }
            }
            if let DeclarationKind::Impl { methods, .. } = &declaration.kind {
                for method in methods {
                    self.refuse_derive(&method.attributes, "a method");
                }
            }
        }
    }

    /// An `impl` block is for a struct or an enum the file declares, by
    /// its plain name (O4); the parser sees to it that the block holds
    /// functions. The units of a block that is not are reported once, at
    /// the block, and skipped: their indices are returned, and their names
    /// are filed as failed so that a mention of one is not reported again.
    fn refuse_bad_impls(&mut self, program: &ast::Program, units: &[Unit<'_>]) -> HashSet<usize> {
        let mut skipped = HashSet::new();
        for declaration in &program.declarations {
            let DeclarationKind::Impl {
                target,
                model,
                methods,
                ..
            } = &declaration.kind
            else {
                continue;
            };
            if let Some(model) = model {
                if self.model_shape(model, methods).is_err() {
                    for (index, unit) in units.iter().enumerate() {
                        if unit.owner.is_some_and(|owner| std::ptr::eq(owner, target)) {
                            skipped.insert(index);
                        }
                    }
                }
                continue;
            }
            // The type, in the type namespace; failing that, whatever else
            // the file declares under the name, for the message.
            let of_name = |name: &ast::Name, wanted: fn(&DeclarationKind) -> bool| {
                program
                    .declarations
                    .iter()
                    .map(|declaration| &declaration.kind)
                    .find(|kind| {
                        wanted(kind)
                            && declared_name_of(kind).is_some_and(|declared| declared == name.text)
                    })
            };
            let reason = match target.single() {
                None => Some(
                    "paths through modules are not in Locus yet; modules are a later project"
                        .to_string(),
                ),
                Some(name) => {
                    let is_type = |kind: &DeclarationKind| {
                        matches!(
                            kind,
                            DeclarationKind::Struct { .. } | DeclarationKind::Enum { .. }
                        )
                    };
                    match (of_name(name, is_type), of_name(name, |_| true)) {
                        (Some(_), _) => None,
                        (None, Some(DeclarationKind::Prop { .. })) => Some(format!(
                            "`{}` is a proposition, and an `impl` block is for a struct or an enum",
                            name.text
                        )),
                        (None, Some(_)) => Some(format!(
                            "`{}` is a function, and an `impl` block is for a struct or an enum",
                            name.text
                        )),
                        (None, None) => Some(format!("unknown type `{}`", name.text)),
                    }
                }
            };
            if let Some(reason) = reason {
                self.diagnostics.push(
                    Diagnostic::error("L0200", reason, target.span)
                        .note("an `impl` block is written for a struct or an enum declared in the same file, by its name"),
                );
                for (index, unit) in units.iter().enumerate() {
                    if unit.owner.is_some_and(|owner| std::ptr::eq(owner, target)) {
                        skipped.insert(index);
                        if let Some(name) = unit_name(unit) {
                            self.failed.insert(name);
                        }
                    }
                }
            }
        }
        skipped
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
                AttributeKind::Derive(_) | AttributeKind::Trusted { .. } => {}
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
    pub(super) fn derives(
        &mut self,
        attributes: &[ast::Attribute],
        type_name: &str,
        fields: &[(String, &[Binder])],
    ) -> Elab<Vec<Derive>> {
        self.check_logical_derive(attributes, fields)?;
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
                if name == "Logical" || name == "Model" {
                    continue;
                }
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
                    } else if !(self.derives_trait(&field.ty, *derive)
                        || matches!(derive, Derive::Copy | Derive::Clone)
                            && matches!(
                                self.session.binding_layout(field.id),
                                crate::typed::ErasureLayout::Shared { .. }
                            ))
                    {
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
    pub(super) fn start_item(&mut self, name: &str, total: bool, promises: Promises) {
        let definitions = self.session.program().definitions().clone();
        self.ctx = Context::with_definitions(Rc::new(definitions));
        self.names.clear();
        self.layout_hints.clear();
        self.facts.clear();
        self.loops.clear();
        self.returns = None;
        self.recursion = None;
        self.total = total;
        self.in_constant = false;
        self.suppress_models = 0;
        self.item_name = name.to_string();
        self.promises = promises;
        self.formula = None;
        self.moves.start_item();
        self.exits.clear();
        self.borrowed.clear();
    }

    /// A unit: a declaration of the file, or a function of an `impl` block
    /// with `Self` and `self` standing for its type.
    fn unit(&mut self, unit: &Unit<'_>) -> Elab<Global> {
        let Some(owner) = unit.owner else {
            return self.declaration(unit.declaration, None);
        };
        let owner_name = owner.text();
        let owner_ty = match self.types.get(&owner_name) {
            Some(Global::Struct(info)) => Type::Struct(info.id),
            Some(Global::Enum(info)) => Type::Enum(info.id),
            _ if unit.model.is_some() => self.ty(&unit.model.unwrap().target)?,
            // Reported at the block, or the type itself failed.
            _ => return Err(()),
        };
        self.owner = Some(owner_name.clone());
        let result = self.declaration(unit.declaration, Some((owner_name, owner_ty)));
        self.owner = None;
        result
    }

    fn capture_binders(&mut self, names: &[ast::Name]) -> Elab<Vec<Binder>> {
        let mut result = Vec::new();
        for name in names {
            let binder = Binder {
                id: crate::kernel::VarId::fresh(),
                name: name.text.clone(),
                ty: Type::Prop,
                ghost: true,
            };
            self.declare(&binder, true, name.span)?;
            result.push(binder);
        }
        Ok(result)
    }

    fn declaration(
        &mut self,
        declaration: &ast::Declaration,
        owner: Option<(String, Type)>,
    ) -> Elab<Global> {
        let attributes = &declaration.attributes;
        match &declaration.kind {
            DeclarationKind::Foreign { name, foreign } => {
                self.foreign_function(name, foreign, declaration.visibility.clone())
            }
            DeclarationKind::ImportedModule { .. } | DeclarationKind::RustImport { .. } => self
                .fail(
                    "L0512",
                    "Rust imports require the Cargo project loader",
                    declaration.span,
                ),
            DeclarationKind::Struct {
                shape,
                name,
                fields,
                generics,
            } => {
                self.refuse_promises(attributes, name, "a struct");
                self.start_item(&name.text, true, LOGICAL);
                let captures = self.capture_binders(&declaration.captures)?;
                let field_visibility: Vec<Option<ast::Visibility>> = fields
                    .iter()
                    .map(|field| field.visibility.clone())
                    .collect();
                let fields = self.telescope(
                    fields
                        .iter()
                        .map(|field| (Some(&field.name), &field.ty, field.span)),
                    true,
                )?;
                let derives =
                    self.derives(attributes, &name.text, &[(name.text.clone(), &fields)])?;
                let item = StructItem {
                    shape: *shape,
                    name: name.text.clone(),
                    fields: fields.clone(),
                    derives: derives.clone(),
                };
                let id = match self.session.declare_struct_family(&item, &captures) {
                    Ok(id) => id,
                    Err(error) => return self.internal(error, name.span),
                };
                self.session.register_type_lifetimes(
                    &Type::Struct(id),
                    generics
                        .iter()
                        .filter(|p| p.lifetime)
                        .map(|p| p.name.text.clone())
                        .collect(),
                );
                if super::logical_data::has_logical_derive(attributes)
                    && let Err(error) = self.session.mark_logical_type(&Type::Struct(id))
                {
                    return self.internal(error, name.span);
                }
                let info = Rc::new(StructInfo {
                    shape: *shape,
                    captures,
                    origin: Some(name.span),
                    id,
                    name: name.text.clone(),
                    fields,
                    derives,
                    visibility: declaration.visibility.clone(),
                    field_visibility: field_visibility.clone(),
                });
                let count: usize = attributes
                    .iter()
                    .filter_map(|attribute| match &attribute.kind {
                        AttributeKind::Derive(paths) => {
                            Some(paths.iter().filter(|path| path.text() == "Model").count())
                        }
                        _ => None,
                    })
                    .sum();
                if count > 1 {
                    return self.fail("L0242", "`Model` is derived twice", name.span);
                }
                if count == 1 {
                    self.derive_model(&info, name.span)?;
                }
                Ok(Global::Struct(info))
            }
            DeclarationKind::Enum {
                name,
                variants,
                generics,
            } => {
                self.refuse_promises(attributes, name, "an enum");
                if super::logical_data::has_logical_derive(attributes)
                    && declaration.captures.is_empty()
                {
                    return self.logical_enum(declaration, name, variants);
                }
                if variants.iter().any(|v| {
                    v.fields
                        .iter()
                        .any(|f| super::logical_data::contains_self(&f.ty, &name.text))
                }) {
                    return self.runtime_recursive_enum(declaration, name, variants);
                }
                self.start_item(&name.text, true, LOGICAL);
                let captures = self.capture_binders(&declaration.captures)?;
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
                    for capture in &captures {
                        self.declare(capture, true, name.span)?;
                    }
                    let payload = self.telescope(
                        variant
                            .fields
                            .iter()
                            .map(|field| (field.name.as_ref(), &field.ty, field.span)),
                        true,
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
                let id = match self.session.declare_enum_family(&item, &captures) {
                    Ok(id) => id,
                    Err(error) => return self.internal(error, name.span),
                };
                self.session.register_type_lifetimes(
                    &Type::Enum(id),
                    generics
                        .iter()
                        .filter(|p| p.lifetime)
                        .map(|p| p.name.text.clone())
                        .collect(),
                );
                if super::logical_data::has_logical_derive(attributes)
                    && let Err(error) = self.session.mark_logical_type(&Type::Enum(id))
                {
                    return self.internal(error, name.span);
                }
                Ok(Global::Enum(Rc::new(EnumInfo {
                    captures,
                    id,
                    name: name.text.clone(),
                    variants: items,
                    derives,
                    visibility: declaration.visibility.clone(),
                })))
            }
            DeclarationKind::Function {
                name,
                logical,
                self_param,
                parameters,
                result,
                body,
                ..
            } => {
                self.refuse_derive(attributes, "a function");
                if *logical {
                    self.require_preview(
                        crate::preview::Feature::LogicalSplit,
                        "logic fn",
                        name.span,
                    )?;
                }
                let declared_promises = self.promises_of(attributes, self.file_promises);
                let promises = if *logical { LOGICAL } else { declared_promises };
                if let Some((reason, implementation)) =
                    attributes
                        .iter()
                        .find_map(|attribute| match &attribute.kind {
                            AttributeKind::Trusted {
                                reason,
                                implementation,
                            } => Some((reason, implementation)),
                            _ => None,
                        })
                {
                    if owner.is_some() || *logical {
                        return self.fail(
                            "L0287",
                            "trusted declarations are runtime free functions",
                            name.span,
                        );
                    }
                    return self.trusted_function(
                        name,
                        parameters,
                        result,
                        promises,
                        declaration.visibility.clone(),
                        reason,
                        implementation,
                    );
                }
                let receiver = match (self_param, &owner) {
                    (Some(param), Some((_, ty))) => Some(Receiver {
                        passing: match param.kind {
                            ast::SelfKind::Value => Passing::Value,
                            ast::SelfKind::MutValue => Passing::MutValue,
                            ast::SelfKind::Ref => Passing::Ref,
                            ast::SelfKind::RefMut => Passing::RefMut,
                        },
                        ty: ty.clone(),
                        span: param.span,
                    }),
                    _ => None,
                };
                let takes_mut = receiver
                    .as_ref()
                    .is_some_and(|receiver| receiver.passing == Passing::RefMut)
                    || parameters.iter().any(|parameter| {
                        matches!(parameter.ty.kind, ast::TypeKind::Ref { mutable: true, .. })
                    });
                let function = Function {
                    logical: *logical,
                    promises,
                    takes_mut,
                    body: Body::Block(body),
                    constant: false,
                    visibility: declaration.visibility.clone(),
                    owner: owner.map(|(name, _)| name),
                    receiver,
                };
                self.function(name, parameters, result, function)
            }
            DeclarationKind::Constant { name, ty, value } => {
                self.refuse_promises(attributes, name, "a constant");
                self.refuse_derive(attributes, "a constant");
                let function = Function {
                    logical: false,
                    promises: LOGICAL,
                    takes_mut: false,
                    body: Body::Expr(value),
                    constant: true,
                    visibility: declaration.visibility.clone(),
                    owner: owner.map(|(name, _)| name),
                    receiver: None,
                };
                self.function(name, &[], ty, function)
            }
            DeclarationKind::Prop {
                name,
                parameters,
                variants,
                ..
            } => {
                self.refuse_promises(attributes, name, "a proposition");
                self.refuse_derive(attributes, "a proposition");
                self.prop(name, parameters, variants)
            }
            DeclarationKind::SpecImpl { .. }
            | DeclarationKind::AssociatedType { .. }
            | DeclarationKind::Trait { .. }
            | DeclarationKind::Spec { .. }
            | DeclarationKind::ModuleImpl { .. }
            | DeclarationKind::Module { .. }
            | DeclarationKind::Use { .. } => self.fail(
                "L0500",
                "resolve modules with the project loader before elaboration",
                declaration.span,
            ),
            DeclarationKind::Impl { .. } => {
                unreachable!("the functions of an impl block are units of their own")
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
            logical: explicit_logic,
            promises,
            takes_mut,
            body,
            constant,
            visibility,
            owner,
            receiver,
        } = function;
        // A function of an `impl` block is `Type::name` everywhere it is
        // filed, named, or printed by the logic.
        let qualified = match &owner {
            Some(owner) => format!("{owner}::{}", name.text),
            None => name.text.clone(),
        };
        let name = &ast::Name {
            text: qualified,
            span: name.span,
        };
        let started = std::time::Instant::now();
        // A function that may appear in a proposition is a function of the
        // logic: its body is a kernel term, total by construction, and
        // nothing in it may fail to return.
        let logical = explicit_logic || constant;
        if explicit_logic && takes_mut {
            return self.fail(
                "L0270",
                "a logic fn cannot take a mutable reference",
                name.span,
            );
        }
        let result_logical = self.logical_spelling(result);
        if explicit_logic && !result_logical {
            return self.fail(
                "L0270",
                "a logic fn must return a Logical type",
                result.span,
            );
        }
        let signature = Signature {
            receiver: receiver.as_ref(),
            parameters,
            result,
        };
        let elaborated = self.function_body(name, &signature, body, logical, promises, constant);
        let (header, block) = elaborated?;
        let Header {
            params,
            passing,
            exits,
            result: result_ty,
        } = header;
        let item = FnItem {
            name: name.text.clone(),
            math: logical,
            params: params.clone(),
            result: result_ty,
            body: block,
            passing: passing.clone(),
            exits,
        };
        // What a call sees: for a function with `&mut` parameters, the
        // tuple of their exit values and the result.
        let result_ty = item.exec_result();
        let elaborate_micros = started.elapsed().as_micros();
        let started = std::time::Instant::now();
        // The checker enforces the promises of an ordinary function; a
        // function of the logic keeps them by construction.
        let recursion = self
            .recursion
            .as_ref()
            .map(|(binder, candidates)| (binder.id, candidates.clone()));
        let declared = if let Some((recursive, candidates)) = &recursion {
            self.session
                .declare_structural_fn(&item, *recursive, candidates)
        } else {
            match &owner {
                _ if constant => self.session.declare_constant_with_layout(
                    &item,
                    promises,
                    self.written_layout(result),
                ),
                Some(owner) => self.session.declare_method_with_layout(
                    &item,
                    promises,
                    owner,
                    receiver.is_some(),
                    self.written_layout(result),
                ),
                None => self.session.declare_fn_with_layout(
                    &item,
                    promises,
                    self.written_layout(result),
                ),
            }
        };
        let reference = match declared {
            Ok(reference) => reference,
            Err(crate::typed::LowerError::ReferencePermission(message)) => {
                return self.fail("L0286", message, name.span);
            }
            Err(error) if recursion.is_some() => {
                return self.fail(
                    "L0203",
                    format!(
                        "recursive logic function `{}` is not structurally decreasing: {error}",
                        name.text
                    ),
                    name.span,
                );
            }
            Err(error) => return self.internal(error, name.span),
        };
        if constant {
            self.session.set_constant_owner(reference, owner.as_deref());
        }
        {
            self.session
                .classify_function(reference, explicit_logic, result_logical)
                .map_err(|error| self.error("L0300", error.to_string(), name.span))?;
        }
        if let (FnRef::Exec(id), ast::TypeKind::Never) = (reference, &result.kind)
            && !constant
        {
            self.never_fns.insert(id);
        }
        self.items.push(ItemReport {
            name: name.text.clone(),
            elaborate_micros,
            check_micros: started.elapsed().as_micros(),
        });
        Ok(Global::Fn(Rc::new(FnInfo {
            origin: Some(name.span),
            logical,
            result_logical,
            reference,
            name: name.text.clone(),
            params,
            result: result_ty,
            constant,
            promises,
            passing,
            visibility,
            receiver: receiver.is_some(),
        })))
    }

    /// The signature and the body, elaborated in a fresh scope: as a term of
    /// the logic when `logical`, as code otherwise. The receiver of a
    /// method is a parameter named `self`, first, of the `impl` block's
    /// type, passed as written: `self`, `mut self`, `&self`, or `&mut self`.
    fn function_body(
        &mut self,
        name: &ast::Name,
        signature: &Signature<'_>,
        body: Body<'_>,
        logical: bool,
        promises: Promises,
        constant: bool,
    ) -> Elab<(Header, crate::typed::Block)> {
        let Signature {
            receiver,
            parameters,
            result,
        } = *signature;
        self.start_item(&name.text, logical, promises);
        self.in_constant = constant;
        if logical && !constant {
            self.formula = Some("a logic fn");
        }
        if constant {
            self.formula = Some("the value of a constant");
        }
        let mut params: Vec<Binder> = Vec::new();
        let mut passing = Vec::new();
        if let Some(receiver) = receiver {
            let binder = Binder::new("self", receiver.ty.clone());
            self.declare(&binder, false, receiver.span)?;
            self.declare_passing(binder.id, receiver.passing);
            params.push(binder);
            passing.push(if logical {
                Passing::Value
            } else {
                receiver.passing
            });
        }
        for parameter in parameters {
            if params
                .iter()
                .any(|earlier| earlier.name == parameter.name.text)
            {
                let message = format!("parameter `{}` is declared twice", parameter.name.text);
                return self.fail("L0202", message, parameter.name.span);
            }
            let (written, mode) = if logical {
                (self.observation_parameter(parameter)?, Passing::Value)
            } else {
                self.parameter_type(parameter)?
            };
            let mut binder = Binder::new(&parameter.name.text, written.ty);
            binder.ghost = written.ghost;
            let layout = if logical {
                self.written_layout(parameter.ty.observed())
            } else {
                self.written_parameter_layout(&parameter.ty)
            };
            self.session.register_binding_layout(binder.id, layout);
            self.declare(&binder, false, parameter.span)?;
            self.declare_passing(binder.id, mode);
            // Preserve explicit dereference syntax in existing logical bodies;
            // the callable signature itself has no reference passing mode.
            if logical && !std::ptr::eq(parameter.ty.observed(), &parameter.ty) {
                self.borrowed.push(binder.id);
            }
            params.push(binder);
            passing.push(mode);
        }
        // `-> !` is a function that never returns. In the logic its result
        // is evidence of `False`, the empty type: a call to it is
        // never-typed, and where a value of another type is wanted the
        // evidence gives it by `match {}` (`exprs.rs`). Its body must end
        // in a never-typed expression, and no `return` can stand in it.
        // Any other result type speaks of a `&mut` parameter's value at
        // return (`references.rs`).
        let never = !constant && matches!(result.kind, ast::TypeKind::Never);
        let (exits, result_ty) = if never {
            let exits = self.exit_binders(&params, &passing, result.span)?;
            (exits, Type::proof(self.prelude.falsehood_prop()))
        } else {
            self.result_over_exits(result, &params, &passing)?
        };
        if !logical && !constant {
            self.returns = Some(super::env::ReturnTarget {
                layout: self.written_layout(result),
                logical: self.logical_spelling(result),
                result: result_ty.clone(),
                never,
            });
        }
        if logical
            && !constant
            && receiver.is_none()
            && self.previews.contains(crate::preview::Feature::LogicalData)
        {
            let candidates: Vec<_> = params.iter().enumerate().filter_map(|(index, param)| match &param.ty {
                Type::Int => Some(index),
                Type::Enum(id) if self.session.program().definitions().finite_enum_group(*id).is_some() => Some(index),
                Type::Proof(claim) if matches!(&**claim, Term::PropApp(id, _) if self.session.program().definitions().is_inductive_prop(*id)) => Some(index),
                _ => None,
            }).collect();
            if !candidates.is_empty() {
                let ty = Type::function_over(&super::types::pairs(&params), &result_ty);
                let mut binder = Binder::new(&name.text, ty);
                binder.ghost = true;
                self.declare(&binder, false, name.span)?;
                self.recursion = Some((binder, candidates));
            }
        }
        let block = match body {
            Body::Block(block) => {
                self.expect_block_layout(block, &self.written_layout(result));
                let end = block_end(block.span);
                let before = self.diagnostics.len();
                let elaborated = self.block(block, Some(&result_ty));
                if never {
                    // Evidence of `False` is how the logic spells `!`; a
                    // path of the body that produces a value is told so.
                    let shown = self.show_type(&result_ty);
                    for diagnostic in &mut self.diagnostics[before..] {
                        if diagnostic.code == "L0220" && diagnostic.message.contains(&shown) {
                            diagnostic.message = format!(
                                "`{}` is declared `-> !`, and its body can reach its end here",
                                name.text
                            );
                        }
                    }
                }
                let (block, _, ends_never) = elaborated?;
                if never && !ends_never {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "L0220",
                            format!("`{}` is declared `-> !`, and its body can reach its end", name.text),
                            result.span,
                        )
                        .label(end, "the body ends here, and a function that never returns has no end to reach")
                        .note("a function that never returns ends in a panic, a `loop` without `break`, or a call to a function declared `-> !`"),
                    );
                    return Err(());
                }
                block
            }
            Body::Expr(value) => {
                self.expect_layout(value, &self.written_layout(result));
                let checked = self.check(value, &result_ty)?;
                // Ordinary calls were rejected on entry to this logical initializer.
                crate::typed::Block {
                    stmts: Vec::new(),
                    tail: Some(Box::new(checked.expr)),
                }
            }
        };
        if !self.logical_spelling(result)
            && matches!(result_ty, Type::Bool)
            && block
                .tail
                .as_deref()
                .is_some_and(super::reconcile::is_logical_expr)
        {
            return self.fail(
                "L0272",
                "a function returning runtime bool cannot return logical Bool",
                result.span,
            );
        }
        Ok((
            Header {
                params,
                passing,
                exits,
                result: result_ty,
            },
            block,
        ))
    }

    fn prop(
        &mut self,
        name: &ast::Name,
        parameters: &[ast::Parameter],
        variants: &[ast::PropVariant],
    ) -> Elab<Global> {
        let named = variants.iter().any(|variant| variant.body.is_some());
        if named {
            self.require_preview(
                crate::preview::Feature::NamedProps,
                "named proposition arms",
                name.span,
            )?;
        }
        if self.previews.contains(crate::preview::Feature::NamedProps)
            && let Some(legacy) = variants.iter().find(|variant| variant.body.is_none())
        {
            self.diagnostics
                .push(crate::parser::legacy_prop_migration(legacy, self.source));
            return Err(());
        }
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
        let mut predicate = Binder::new(
            &name.text,
            Type::function_over(&super::types::pairs(&params), &Type::Prop),
        );
        predicate.ghost = true;
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
            if let Some(body) = &variant.body {
                // A local callable permits checking the formula without an
                // unchecked global predicate declaration or elimination rule.
                self.declare(&predicate, false, variant.span)?;
                for param in &params {
                    self.declare(param, true, variant.span)?;
                }
                let mut payload = self.telescope(fields, true)?;
                if let Some(field) = payload
                    .iter()
                    .find(|field| matches!(field.ty, Type::Proof(_)))
                {
                    return self.fail(
                        "L0275",
                        format!(
                            "witness `{}` is evidence; put its proposition in the arm body",
                            field.name
                        ),
                        variant.span,
                    );
                }
                let (block, _, _) = self.logical("a proposition arm body", |env| {
                    env.block(body, Some(&Type::Prop))
                })?;
                let value = super::exprs::Value::new(crate::typed::Expr::Block(block), Type::Prop);
                let computed = self.term(&value, body.span)?;
                let mut telescope = params.clone();
                telescope.extend(payload.iter().cloned());
                let ids: Vec<_> = telescope.iter().map(|binder| binder.id).collect();
                let arm_body = computed.clone();
                kernel_variants.push(PropVariant::arm(tuple_over(&telescope), |given| {
                    ids.iter()
                        .zip(given)
                        .fold(arm_body, |term, (id, argument)| {
                            term.replace_var(*id, argument)
                        })
                }));
                payload.push(Binder::new("evidence", Type::proof(computed.clone())));
                infos.push(PropVariantInfo {
                    name: variant.name.text.clone(),
                    body: Some(computed),
                    payload,
                    named: variant.shape == ast::VariantShape::Struct,
                    conclusion: None,
                });
                continue;
            }
            match &variant.target {
                None => {
                    // The parameters are in scope in the payload.
                    for param in &params {
                        self.declare(param, true, variant.span)?;
                    }
                    let payload = self.telescope(fields, true)?;
                    let mut telescope = params.clone();
                    telescope.extend(payload.iter().cloned());
                    kernel_variants.push(PropVariant::Params(tuple_over(&telescope)));
                    infos.push(PropVariantInfo {
                        name: variant.name.text.clone(),
                        body: None,
                        payload,
                        named: variant.shape == ast::VariantShape::Struct,
                        conclusion: None,
                    });
                }
                Some(target) => {
                    let payload = self.telescope(fields, true)?;
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
                        body: None,
                        payload,
                        named: variant.shape == ast::VariantShape::Struct,
                        conclusion: Some(conclusion),
                    });
                }
            }
        }
        let param_types = params.iter().map(|param| param.ty.clone()).collect();
        let recursive = infos
            .iter()
            .filter_map(|info| info.body.as_ref())
            .any(|body| {
                body.find(&|term| matches!(term, Term::Free(id) if *id == predicate.id))
                    .is_some()
            });
        let declared = if recursive {
            self.require_preview(
                crate::preview::Feature::LogicalData,
                "recursive proposition",
                name.span,
            )?;
            self.session
                .declare_inductive_prop(param_types, kernel_variants, predicate.id)
        } else {
            self.session.declare_prop(param_types, kernel_variants)
        };
        let id = match declared {
            Ok(id) => id,
            Err(error) if recursive => {
                return self.fail(
                    "L0203",
                    format!(
                        "recursive proposition `{}` is not strictly positive: {error}",
                        name.text
                    ),
                    name.span,
                );
            }
            Err(error) => return self.internal(error, name.span),
        };
        if recursive {
            for info in &mut infos {
                if let Some(body) = &mut info.body {
                    *body = body.replace_predicate(predicate.id, id);
                }
                for field in &mut info.payload {
                    field.ty = field.ty.replace_predicate(predicate.id, id);
                }
            }
        }
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
                    origin: None,
                    logical: true,
                    result_logical: true,
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
                    passing: Vec::new(),
                    visibility: None,
                    receiver: false,
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
            body: None,
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

/// A function's signature as written: the receiver of a method, the
/// parameters, and the result type.
#[derive(Clone, Copy)]
struct Signature<'a> {
    receiver: Option<&'a Receiver>,
    parameters: &'a [ast::Parameter],
    result: &'a ast::Type,
}

/// The `self` parameter of a method: how it is passed, the type of the
/// `impl` block, and where it is written.
struct Receiver {
    passing: Passing,
    ty: Type,
    span: Span,
}

/// A function's signature as elaborated: its parameters, how each is
/// passed, the exit binders of its `&mut` parameters, and its result type
/// over them (`references.rs`).
struct Header {
    params: Vec<Binder>,
    passing: Vec<crate::typed::Passing>,
    exits: Vec<Binder>,
    result: Type,
}

#[derive(Clone, Copy)]
enum Body<'a> {
    Block(&'a ast::Block),
    Expr(&'a ast::Expr),
}

/// The closing brace of a block, for a label at its end.
fn block_end(span: Span) -> Span {
    Span::new(span.file, span.end.saturating_sub(1), span.end)
}

/// What a `fn` or a `const` says about itself besides its signature.
struct Function<'a> {
    logical: bool,
    promises: Promises,
    takes_mut: bool,
    body: Body<'a>,
    /// Declared with `const`: used by name, without a call.
    constant: bool,
    /// `pub` or a restricted form, as written; private without one.
    visibility: Option<ast::Visibility>,
    /// Declared in an `impl` block: the type's name.
    owner: Option<String>,
    /// The `self` parameter of a method.
    receiver: Option<Receiver>,
}
