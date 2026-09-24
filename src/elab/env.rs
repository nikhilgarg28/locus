//! What is in scope: declared items, local names, and known facts.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::ast;
use crate::diagnostic::Diagnostic;
use crate::exec::{Promise, Promises};
use crate::kernel::theory::Theory;
use crate::kernel::{
    Context, EnumId, FnId, HypId, KernelError, Mode, Prelude, Proof, PropId, StructId, Term, Type,
    VarId, infer_term, telescope_entry,
};
use crate::source::{SourceFile, Span};
use crate::typed::{Binder, Derive, FnRef, Passing, Session};

use super::items::{HoleReport, ItemReport};
use super::moves::{LoopMoves, Moved, Moves};
use super::mutation::{ArmEnd, Entry, Tracked};

pub(super) type Elab<T> = Result<T, ()>;

#[derive(Debug)]
pub(super) struct StructInfo {
    pub origin: Option<Span>,
    pub id: StructId,
    pub name: String,
    /// A field's type may mention the binders of the fields before it.
    pub fields: Vec<Binder>,
    /// `#[derive(...)]`, checked: `Copy` in it makes the type reusable.
    pub derives: Vec<Derive>,
    /// `pub` or a restricted form, as written; private without one.
    pub visibility: Option<ast::Visibility>,
    /// Each field's, in the order of `fields`.
    pub field_visibility: Vec<Option<ast::Visibility>>,
}

#[derive(Debug)]
pub(super) struct EnumInfo {
    pub id: EnumId,
    pub name: String,
    pub variants: Vec<VariantInfo>,
    /// `#[derive(...)]`, checked: `Copy` in it makes the type reusable.
    pub derives: Vec<Derive>,
    /// `pub` or a restricted form, as written; private without one. A
    /// variant is as visible as its enum.
    pub visibility: Option<ast::Visibility>,
}

#[derive(Debug)]
pub(super) struct VariantInfo {
    pub name: String,
    /// A field's type may mention the binders of the fields before it; the
    /// binders' names are the field names when `named`.
    pub payload: Vec<Binder>,
    /// Declared with braces, `V { a: T }`: written and matched by field
    /// name, and printed so in Rust.
    pub named: bool,
}

#[derive(Debug)]
pub(super) struct PropInfo {
    pub id: PropId,
    pub name: String,
    pub params: Vec<Binder>,
    pub variants: Vec<PropVariantInfo>,
}

#[derive(Debug)]
pub(super) struct PropVariantInfo {
    pub name: String,
    /// Computed body of a named arm, over header parameters and witnesses.
    /// Its final payload binder holds evidence of this body.
    pub body: Option<Term>,
    /// Over the proposition's parameters when there is no conclusion, and
    /// over nothing but the earlier payload when there is one.
    pub payload: Vec<Binder>,
    /// Declared with braces: written and matched by field name.
    pub named: bool,
    /// The arguments at which this variant proves the proposition, over the
    /// payload. Absent when it proves it at the parameters themselves.
    pub conclusion: Option<Vec<Term>>,
}

#[derive(Clone, Debug)]
pub(super) struct FnInfo {
    pub origin: Option<Span>,
    /// Checked logical declaration; independent of runtime promises.
    pub logical: bool,
    /// Surface result classification (kernel Bool alone cannot retain it).
    pub result_logical: bool,
    pub reference: FnRef,
    pub name: String,
    pub params: Vec<Binder>,
    /// Over the parameters' identities.
    pub result: Type,
    /// Declared with `const`: used by name, without a call.
    pub constant: bool,
    /// What the function promises: its attributes and the file's defaults.
    pub promises: Promises,
    /// How each parameter is passed, in the order of `params`; a shorter
    /// list means the rest are by value. For a function with `&mut`
    /// parameters `result` is the tuple of their exit values followed by
    /// the declared result (`FnItem::exec_result`).
    pub passing: Vec<Passing>,
    /// `pub` or a restricted form, as written; private without one.
    pub visibility: Option<ast::Visibility>,
    /// The first parameter is `self`: a method of an `impl` block, whose
    /// `name` is `Type::name`, called as `x.name(..)`, which lends or moves
    /// `x` as `passing[0]` says.
    pub receiver: bool,
}

/// The promises that let a function appear in a proposition: it always
/// returns, so `f(x)` denotes one value, and it does nothing observable, so
/// a proposition, which is never run, changes nothing by mentioning it.
pub(super) const LOGICAL: Promises = Promises {
    terminates: true,
    no_panic: true,
    no_alloc: false,
    no_io: true,
};

/// The first promise `made` makes that `callee` does not, in the order the
/// promises are listed in.
pub(super) fn first_broken(made: Promises, callee: Promises) -> Option<Promise> {
    Promise::ALL
        .into_iter()
        .find(|&promise| made.makes(promise) && !callee.makes(promise))
}

#[derive(Clone, Debug)]
pub(super) enum Global {
    Struct(Rc<StructInfo>),
    Enum(Rc<EnumInfo>),
    Prop(Rc<PropInfo>),
    Fn(Rc<FnInfo>),
}

#[derive(Clone, Debug)]
pub(super) struct Local {
    pub name: String,
    pub id: VarId,
    pub ty: Type,
    /// The binding failed to elaborate. A use of it is a consequence of an
    /// error already reported, and is not reported again.
    pub poisoned: bool,
    /// The source type is Logical even when the shared kernel representation
    /// also serves runtime data (notably Bool versus bool).
    pub ghost: bool,
    /// For a binding declared `let mut`: its identity, which every version
    /// of it refers to. `id` is then the current version (`mutation.rs`).
    pub binding: Option<VarId>,
    /// What has been moved out of it, and where (`moves.rs`).
    pub moved: Vec<Moved>,
    /// For a mutable binding whose declared type mentions other mutable
    /// bindings, tracked evidence: what it mentions, and whether it is
    /// valid here (`mutation.rs`).
    pub tracked: Option<Tracked>,
}

/// Something known at this point, with the proof that it holds.
#[derive(Clone, Debug)]
pub(super) struct Fact {
    pub proof: Proof,
    pub claim: Term,
    /// The equation `name == value` of a `let`: what computing replaces the
    /// name by. Any other fact is used exactly, and never to rewrite.
    pub definition: bool,
}

impl Fact {
    pub fn new(proof: Proof, claim: Term) -> Self {
        Self {
            proof,
            claim,
            definition: false,
        }
    }

    pub fn definition(proof: Proof, claim: Term) -> Self {
        Self {
            proof,
            claim,
            definition: true,
        }
    }
}

/// The loop a `break` or `continue` belongs to (`loops.rs`).
#[derive(Clone)]
pub(super) struct LoopTarget {
    /// The type a `break` supplies, once it is known: the type expected of
    /// a `loop`, or the type of its first `break value`.
    pub result: Option<Type>,
    pub layout: Option<crate::typed::ErasureLayout>,
    /// Whether a `break` may carry a value: in a `loop`, and not in a
    /// `while` or a `for`, which produce none.
    pub valued: bool,
    /// The mutable bindings in scope at the loop, at their versions before
    /// it; each `break` records the versions current where it stands, for
    /// the join after the loop.
    pub entry: Entry,
    /// What each `break`, and the exit of a `while`, ended with.
    pub exits: Vec<ArmEnd>,
    /// What was moved at entry and at each exit (`moves.rs`).
    pub moves: LoopMoves,
    /// The slots of the bindings the loop carries: tracked evidence among
    /// them must be valid wherever the loop's state is supplied.
    pub carried: Vec<usize>,
    /// The loop's keyword, for messages.
    pub head: Span,
}

/// The function a `return` leaves (`control.rs`).
#[derive(Clone)]
pub(super) struct ReturnTarget {
    pub layout: crate::typed::ErasureLayout,
    pub logical: bool,
    /// The declared result type, over the parameters' identities: what a
    /// `return` supplies, checked where the `return` stands.
    pub result: Type,
    /// Declared `-> !`: the function never returns, so a `return` cannot
    /// stand in it, and its body must end in a never-typed expression.
    pub never: bool,
}

/// A point to return to at the end of a lexical scope.
pub(super) struct Mark {
    ctx: crate::kernel::Checkpoint,
    names: usize,
    facts: usize,
}

impl Mark {
    /// How many facts were known at the mark.
    pub fn facts(&self) -> usize {
        self.facts
    }
}

#[derive(Clone)]
pub(super) struct Env<'a> {
    pub module_access: Option<std::sync::Arc<crate::project::Access>>,
    pub models: Vec<super::models::ModelEntry>,
    pub quantifiers: Vec<crate::kernel::Quantifiers>,
    pub closure_capture_boundary: Option<usize>,
    pub explicit_model_depth: usize,
    pub layout_hints: HashMap<Span, crate::typed::ErasureLayout>,
    pub source: &'a SourceFile,
    pub previews: crate::preview::Previews,
    pub session: Session,
    pub prelude: Prelude,
    pub theory: Theory,
    /// The two namespaces of Rust: a type and a value may share a name.
    /// Structs, enums, and propositions are types; functions and constants
    /// are values.
    pub types: HashMap<String, Global>,
    pub values: HashMap<String, Global>,
    /// Items that were rejected; a mention of one is not reported again.
    pub failed: HashSet<String>,
    /// `#![...]` at the top of the file: promised by every function in it.
    pub file_promises: Promises,
    pub diagnostics: crate::limits::DiagnosticBuffer,
    pub normalization_exhausted: bool,
    pub holes: Vec<HoleReport>,
    pub items: Vec<ItemReport>,

    // The function being elaborated.
    pub ctx: Context,
    pub names: Vec<Local>,
    pub facts: Vec<Fact>,
    pub loops: Vec<LoopTarget>,
    /// Where a `return` goes: the ordinary function whose body this is,
    /// and nothing inside a constant or a function of the logic.
    pub returns: Option<ReturnTarget>,
    /// A local callable replaced by a checked structural self reference.
    pub recursion: Option<(Binder, Vec<usize>)>,
    /// The functions declared `-> !`, which never return: a call to one is
    /// never-typed (`exprs.rs`).
    pub never_fns: HashSet<crate::exec::ExecFnId>,
    /// How to print each identity: a name, or the source text of the
    /// expression whose result it is.
    pub labels: HashMap<VarId, String>,
    /// Inside a function of the logic, a proposition, or a proof type,
    /// where nothing may fail to return.
    pub total: bool,
    pub in_constant: bool,
    pub suppress_models: usize,
    /// The name of the item being elaborated, for messages.
    pub item_name: String,
    /// What the function being elaborated promises. Every call is checked
    /// against it, and a loop is refused under `terminates`.
    pub promises: Promises,
    /// Inside a formula, or the value of a constant: a place where only a
    /// function that may appear in a proposition can be called. The text
    /// names the place for a message.
    pub formula: Option<&'static str>,
    /// Set while the body of a function of the logic is elaborated and
    /// something in it is not a kernel term, an operator that may panic or
    /// a call of such a function: what it was and where. `items` then
    /// elaborates the function again as an ordinary one (LOC-193).
    /// The move analysis (`moves.rs`).
    pub moves: Moves,
    /// The `&mut` parameters of the function being elaborated: for each,
    /// the binder its result type speaks of it by, its value at return,
    /// the parameter's identity, which is also its entry value, and its
    /// name (`references.rs`).
    pub exits: Vec<(VarId, VarId, String)>,
    /// The reference parameters of the function being elaborated, by
    /// identity: a `&T` parameter's own, a `&mut T` parameter's binding.
    /// Nothing is moved out of one (`moves.rs`).
    pub borrowed: Vec<VarId>,
    /// The type of the `impl` block whose function is being elaborated,
    /// which `Self` names (`items.rs`).
    pub owner: Option<String>,
}

impl Env<'_> {
    pub fn error(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }

    pub fn fail<T>(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        span: Span,
    ) -> Elab<T> {
        self.error(code, message, span);
        Err(())
    }

    /// A kernel rejection of something the elaborator built. The user did
    /// nothing wrong that an earlier, better message should not have caught.
    pub fn internal<T>(&mut self, error: impl std::fmt::Display, span: Span) -> Elab<T> {
        self.diagnostics.push(
            Diagnostic::error("L0300", format!("the checker rejected this: {error}"), span)
                .note("this is a gap in the elaborator's own checks; the program was not accepted"),
        );
        Err(())
    }

    pub fn kernel<T>(&mut self, result: Result<T, KernelError>, span: Span) -> Elab<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => self.internal(error, span),
        }
    }

    pub fn text(&self, span: Span) -> &str {
        self.source.slice(span).unwrap_or("")
    }

    /// A type's name as written, with `Self` read as the type of the
    /// `impl` block being elaborated.
    pub fn type_text(&self, name: &ast::Name) -> String {
        match (&self.owner, name.text.as_str()) {
            (Some(owner), "Self") => owner.clone(),
            _ => name.text.clone(),
        }
    }

    /// The function `Prefix::name` names, with `Self` read as the type of
    /// the `impl` block: a function of an `impl` block, or nothing.
    pub fn path_function(&self, path: &ast::Path) -> Option<Rc<FnInfo>> {
        let (prefix, name) = path.pair()?;
        let qualified = format!("{}::{}", self.type_text(prefix), name.text);
        match self.values.get(&qualified) {
            Some(Global::Fn(info)) => Some(Rc::clone(info)),
            _ => None,
        }
    }

    /// The function `name` of the `impl` block of the type, if it has one.
    pub fn method_of(&self, ty: &Type, name: &str) -> Option<Rc<FnInfo>> {
        let owner = self.type_name(ty)?;
        match self.values.get(&format!("{owner}::{name}")) {
            Some(Global::Fn(info)) => Some(Rc::clone(info)),
            _ => None,
        }
    }

    /// The name of a struct or enum type, which an `impl` block can be for.
    pub fn type_name(&self, ty: &Type) -> Option<String> {
        match ty {
            Type::Struct(id) => self.struct_by_id(*id).map(|info| info.name.clone()),
            Type::Enum(id) => self.enum_by_id(*id).map(|info| info.name.clone()),
            _ => None,
        }
    }

    pub fn mark(&self) -> Mark {
        Mark {
            ctx: self.ctx.checkpoint(),
            names: self.names.len(),
            facts: self.facts.len(),
        }
    }

    /// Ends a scope whose bindings and facts end with it: a branch, an arm,
    /// a loop body, a quantifier.
    pub fn close(&mut self, mark: Mark) {
        self.ctx.rollback(mark.ctx);
        self.names.truncate(mark.names);
        self.facts.truncate(mark.facts);
    }

    /// Ends a block whose statements the checker splices into the enclosing
    /// sequence: the names and facts go out of scope, and the identities they
    /// spoke of remain declared.
    pub fn close_names(&mut self, mark: Mark) {
        self.names.truncate(mark.names);
        self.facts.truncate(mark.facts);
    }

    pub fn lookup(&self, name: &str) -> Option<&Local> {
        self.names.iter().rev().find(|local| local.name == name)
    }

    pub fn type_of(&mut self, term: &Term, span: Span) -> Elab<Type> {
        let result = infer_term(&mut self.ctx, term, Mode::Logical);
        self.kernel(result, span)
    }

    /// Makes `name` refer to an identity the context already holds, and
    /// records what its type lets one conclude. `ghost` retains the source
    /// type's logical classification.
    pub fn bind(&mut self, name: &str, id: VarId, ty: &Type, ghost: bool) {
        self.labels.insert(id, name.to_string());
        self.names.push(Local {
            name: name.to_string(),
            id,
            ty: ty.clone(),
            poisoned: false,
            ghost,
            binding: None,
            moved: Vec::new(),
            tracked: None,
        });
        self.learn_from(&Term::var(id), ty);
    }

    /// Elaborates a logical context. Ordinary calls are rejected (L0209),
    /// physical observations use immutable models, and modelled arithmetic
    /// operates on logical Int. Naming physical data observes without moving.
    /// Unsupported loops are rejected (L0215); recursive logical functions
    /// use a separately checked structural or explicit measure.
    pub fn logical<T>(&mut self, place: &'static str, inside: impl FnOnce(&mut Self) -> T) -> T {
        let was_total = std::mem::replace(&mut self.total, true);
        let was_formula = self.formula.replace(place);
        let result = self.ghost(inside);
        self.total = was_total;
        self.formula = was_formula;
        result
    }

    /// Brings the names of a pattern into scope after its `let` failed.
    pub fn poison(&mut self, pattern: &crate::ast::Pattern) {
        use crate::ast::PatternKind;
        match &pattern.kind {
            PatternKind::Name { name, .. } => self.names.push(Local {
                name: name.text.clone(),
                id: VarId::fresh(),
                ty: Type::Tuple(Vec::new()),
                poisoned: true,
                ghost: false,
                binding: None,
                moved: Vec::new(),
                tracked: None,
            }),
            PatternKind::Group(inner) => self.poison(inner),
            PatternKind::Tuple(parts) => parts.iter().for_each(|part| self.poison(part)),
            PatternKind::Struct { fields, .. } => {
                fields.iter().for_each(|field| self.poison(&field.pattern));
            }
            PatternKind::Variant { arguments, .. } => {
                arguments
                    .iter()
                    .flatten()
                    .for_each(|part| self.poison(part));
            }
            _ => {}
        }
    }

    /// Declares a variable in the mirrored context and brings it into scope.
    /// It is logical in the kernel when its source type or use requires it.
    pub fn declare(&mut self, binder: &Binder, ghost: bool, span: Span) -> Elab<()> {
        let result = self
            .ctx
            .declare_with(binder.id, binder.ty.clone(), ghost || binder.ghost);
        self.kernel(result, span)?;
        self.bind(&binder.name, binder.id, &binder.ty, binder.ghost);
        Ok(())
    }

    /// Declares the result of an expression that the checker will name.
    pub fn declare_result(&mut self, id: VarId, ty: &Type, span: Span) -> Elab<()> {
        let result = self.ctx.declare_with(id, ty.clone(), false);
        self.kernel(result, span)?;
        let label = self
            .text(span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.labels.insert(id, label);
        self.learn_from(&Term::var(id), ty);
        Ok(())
    }

    pub fn assume(&mut self, id: HypId, claim: Term, span: Span) -> Elab<()> {
        let result = self.ctx.assume_with(id, claim.clone());
        self.kernel(result, span)?;
        self.facts.push(Fact::new(Proof::hyp(id), claim));
        Ok(())
    }

    /// The facts a value carries: itself when it is a proof, and the proof
    /// fields of a tuple or struct, each about the value's own fields.
    pub fn learn_from(&mut self, value: &Term, ty: &Type) {
        match ty {
            Type::Proof(claim) => self
                .facts
                .push(Fact::new(Proof::OfTerm(value.clone()), (**claim).clone())),
            Type::Tuple(fields) => {
                let earlier: Vec<Term> = (0..fields.len())
                    .map(|index| Term::proj(value.clone(), index))
                    .collect();
                for index in 0..fields.len() {
                    if let Some(field) = telescope_entry(ty, index, &earlier[..index]) {
                        self.learn_from(&earlier[index], &field);
                    }
                }
            }
            Type::Struct(id) => {
                let Some(info) = self.struct_by_id(*id) else {
                    return;
                };
                for (index, field) in info.fields.iter().enumerate() {
                    let mut field_ty = field.ty.clone();
                    for (earlier, binder) in info.fields[..index].iter().enumerate() {
                        field_ty =
                            field_ty.replace_var(binder.id, &Term::proj(value.clone(), earlier));
                    }
                    self.learn_from(&Term::proj(value.clone(), index), &field_ty);
                }
            }
            _ => {}
        }
    }

    /// Files a declared item under its namespace.
    pub fn insert_global(&mut self, name: String, global: Global) {
        match global {
            Global::Struct(_) | Global::Enum(_) | Global::Prop(_) => {
                self.types.insert(name, global)
            }
            Global::Fn(_) => self.values.insert(name, global),
        };
    }

    pub fn struct_by_id(&self, id: StructId) -> Option<Rc<StructInfo>> {
        self.types.values().find_map(|global| match global {
            Global::Struct(info) if info.id == id => Some(Rc::clone(info)),
            _ => None,
        })
    }

    pub fn enum_by_id(&self, id: EnumId) -> Option<Rc<EnumInfo>> {
        self.types.values().find_map(|global| match global {
            Global::Enum(info) if info.id == id => Some(Rc::clone(info)),
            _ => None,
        })
    }

    pub fn prop_by_id(&self, id: PropId) -> Option<Rc<PropInfo>> {
        self.types.values().find_map(|global| match global {
            Global::Prop(info) if info.id == id => Some(Rc::clone(info)),
            _ => None,
        })
    }

    pub fn fn_by_id(&self, id: FnId) -> Option<Rc<FnInfo>> {
        self.values.values().find_map(|global| match global {
            Global::Fn(info) if info.reference == FnRef::Math(id) => Some(Rc::clone(info)),
            _ => None,
        })
    }
}

/// Replaces identities the elaborator chose with the ones a binder supplies.
pub(super) fn substitute(proof: Proof, vars: &[(VarId, Term)], hyps: &[(HypId, Proof)]) -> Proof {
    let mut wrapped = Term::proof(proof);
    for (id, term) in vars {
        wrapped = wrapped.replace_var(*id, term);
    }
    for (id, proof) in hyps {
        wrapped = wrapped.replace_hyp(*id, proof);
    }
    match wrapped {
        Term::Proof(proof) => *proof,
        _ => unreachable!("substitution keeps the shape of a term"),
    }
}

impl Env<'_> {
    pub(super) fn module_visible(
        &mut self,
        origin: Option<Span>,
        visibility: Option<&ast::Visibility>,
        name: &str,
        at: Span,
    ) -> Elab<()> {
        if let (Some(access), Some(origin)) = (&self.module_access, origin)
            && !access.allowed(visibility, access.module_at(origin), access.module_at(at))
        {
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0503",
                    format!("`{name}` is private to its module"),
                    at,
                )
                .label(origin, "declared here"),
            );
            return Err(());
        }
        Ok(())
    }
    pub(super) fn field_visible(&mut self, info: &StructInfo, index: usize, at: Span) -> Elab<()> {
        self.module_visible(
            info.origin,
            info.field_visibility[index].as_ref(),
            &format!("{}.{}", info.name, info.fields[index].name),
            at,
        )
    }
}
