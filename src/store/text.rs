//! The text form of kernel types, terms, and proofs, for the proofs file.
//!
//! The form follows the grammar of the kernel contract, with three changes
//! that make it stable across elaborations and total to read:
//!
//! - A declared struct, enum, proposition, or math function is written by
//!   name, as `struct:Lock`, `enum:Event`, `prop:Small`, or `fn:within_limit`,
//!   never by the number the kernel gave it in this run. `Names` carries the
//!   table both ways.
//! - A variable or hypothesis of the context is written by its position
//!   among the variables, `$2`, or among the hypotheses, `h1`, of the
//!   context it was printed in, and read back as whatever holds that
//!   position in the context it is read in. A variable bound inside a term
//!   is a de Bruijn index, `#0`, and a bound hypothesis is `#h0`.
//! - Every form is delimited, so a term is printed without any knowledge of
//!   precedence: `(a ==[u8] b)`, `(p => q)`, `(a, b) : (u8, u8)`,
//!   `absurd(p, T)`, `for(lo, hi, p, (S), init, body)`, and so on.
//!
//! A proof is written as a block of named steps, one per line, `tN = term`
//! or `sN = proof`, the last line the conclusion: a piece that occurs more
//! than once is a step and is named where it is used, so the block is the
//! proof as a DAG (`steps.rs`). A line may name earlier steps only. A proof
//! also reads as one bare expression, which is how version 1 of the proofs
//! file wrote it.
//!
//! What is read is never trusted: the parser builds any kernel term the
//! text describes, well formed or not, and the caller hands the result to
//! the kernel's `check_proof`. The parser never panics on any input: the
//! text is bounded in length, a literal in digits, nesting by the kernel's
//! own `MAX_DEPTH`, and the tree a block of steps expands to by
//! `MAX_NODES`, all as counts.
//!
//! Two things here talk to the store installed for the elaborator (`super`):
//! `print_key` tells it the claim of the obligation it just keyed, and
//! `parse_proof` tells it the canonical text of the proof it just read. The
//! elaborator's hole solver keys, looks up, reads, and then accepts or
//! records, so the store pairs each with the entry in hand; that is how an
//! entry gets its `claim` and how a migrated or hand-edited entry is
//! rewritten in the writer's own form when it is used.

use std::collections::HashMap;
use std::fmt::{self, Write};
use std::hash::Hash;

use super::steps::{self, Kind, Sharing, Steps};

use crate::kernel::{
    Axiom, Binding, CmpOp, Context, EnumId, FnId, ForLoop, HypId, HypRef, Integer, MAX_DEPTH,
    MachineInt, Op, Prim, Proof, ProofArm, PropId, StructId, Term, TermArm, Type, VarId,
};

/// The most bytes of text one term or proof may be.
pub use crate::limits::MAX_PROOF_TEXT_BYTES as MAX_TEXT;

/// The most digits a literal may have.
pub use crate::limits::MAX_PROOF_DIGITS as MAX_DIGITS;

/// The most nodes a proof may have once its steps are expanded into the
/// tree the kernel checks: a step is copied at each use, so a block of
/// steps can name a tree far larger than its text.
pub use crate::limits::MAX_PROOF_EXPANDED_NODES as MAX_NODES;

// --- Names ---------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Table<Id> {
    by_id: HashMap<Id, String>,
    by_name: HashMap<String, Id>,
}

impl<Id> Default for Table<Id> {
    fn default() -> Self {
        Self {
            by_id: HashMap::new(),
            by_name: HashMap::new(),
        }
    }
}

impl<Id: Copy + Eq + Hash> Table<Id> {
    /// A name names one identity and an identity has one name: an earlier
    /// mapping of either is dropped.
    fn insert(&mut self, name: &str, id: Id) {
        if let Some(old) = self.by_name.insert(name.to_string(), id) {
            self.by_id.remove(&old);
        }
        if let Some(old) = self.by_id.insert(id, name.to_string())
            && old != name
        {
            self.by_name.remove(&old);
        }
    }

    fn name(&self, id: Id) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    fn id(&self, name: &str) -> Option<Id> {
        self.by_name.get(name).copied()
    }
}

/// The names of the declarations a term may mention. A name is an
/// path of identifiers, `[A-Za-z_][A-Za-z0-9_]*(::identifier)*`; anything else is refused and the
/// declaration stays unnamed, so a term that mentions it cannot be printed.
#[derive(Clone, Debug, Default)]
pub struct Names {
    fns: Table<FnId>,
    structs: Table<StructId>,
    enums: Table<EnumId>,
    props: Table<PropId>,
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Names {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn function(&mut self, name: &str, id: FnId) {
        if name.split("::").all(is_identifier) {
            self.fns.insert(name, id);
        }
    }

    pub fn structure(&mut self, name: &str, id: StructId) {
        if name.split("::").all(is_identifier) {
            self.structs.insert(name, id);
        }
    }

    pub fn enumeration(&mut self, name: &str, id: EnumId) {
        if name.split("::").all(is_identifier) {
            self.enums.insert(name, id);
        }
    }

    pub fn proposition(&mut self, name: &str, id: PropId) {
        if name.split("::").all(is_identifier) {
            self.props.insert(name, id);
        }
    }
}

// --- Printing ------------------------------------------------------------------

/// Why a term could not be printed: it mentions something the text has no
/// name or position for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrintError {
    UnknownVariable,
    UnknownHypothesis,
    UnnamedFunction,
    UnnamedStruct,
    UnnamedEnum,
    UnnamedProp,
}

impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownVariable => "a variable that is not in the context",
            Self::UnknownHypothesis => "a hypothesis that is not in the context",
            Self::UnnamedFunction => "a function with no name",
            Self::UnnamedStruct => "a struct with no name",
            Self::UnnamedEnum => "an enum with no name",
            Self::UnnamedProp => "a proposition with no name",
        })
    }
}

/// The positions of the context's entries: each variable's among the
/// variables, each hypothesis's among the hypotheses, oldest first.
struct Positions {
    vars: Vec<VarId>,
    hyps: Vec<HypId>,
}

impl Positions {
    fn of(ctx: &Context) -> Self {
        let mut positions = Self {
            vars: Vec::new(),
            hyps: Vec::new(),
        };
        for binding in ctx.bindings() {
            match binding {
                Binding::Var { id, .. } => positions.vars.push(id),
                Binding::Hyp { id, .. } => positions.hyps.push(id),
            }
        }
        positions
    }
}

struct Printer<'a> {
    positions: Positions,
    names: &'a Names,
    out: String,
    /// In the steps form: every composite term and proof is interned as it
    /// is printed and stands in the text as a placeholder for its id, and
    /// the block is written from the pieces afterwards (`steps.rs`).
    sharing: Option<Sharing>,
}

type Printed = Result<(), PrintError>;

impl Printer<'_> {
    // As in the parser below, each function on a recursion path is small:
    // the dispatch on a term or a proof only calls, and each form has a
    // function of its own, so that a level of nesting costs a few small
    // frames in an unoptimized build.

    fn push(&mut self, text: &str) {
        self.out.push_str(text);
    }

    fn list<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Self, &T) -> Printed) -> Printed {
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                self.push(", ");
            }
            each(self, item)?;
        }
        Ok(())
    }

    fn terms(&mut self, terms: &[Term]) -> Printed {
        self.push("(");
        self.list(terms, Self::term)?;
        self.push(")");
        Ok(())
    }

    fn types(&mut self, types: &[Type]) -> Printed {
        self.push("(");
        self.list(types, Self::ty)?;
        self.push(")");
        Ok(())
    }

    fn struct_name(&mut self, id: StructId) -> Printed {
        let name = self
            .names
            .structs
            .name(id)
            .ok_or(PrintError::UnnamedStruct)?;
        let _ = write!(self.out, "struct:{name}");
        Ok(())
    }

    fn enum_name(&mut self, id: EnumId) -> Printed {
        let name = self.names.enums.name(id).ok_or(PrintError::UnnamedEnum)?;
        let _ = write!(self.out, "enum:{name}");
        Ok(())
    }

    fn prop_name(&mut self, id: PropId) -> Printed {
        let name = self.names.props.name(id).ok_or(PrintError::UnnamedProp)?;
        let _ = write!(self.out, "prop:{name}");
        Ok(())
    }

    fn fn_name(&mut self, id: FnId) -> Printed {
        let name = self.names.fns.name(id).ok_or(PrintError::UnnamedFunction)?;
        let _ = write!(self.out, "fn:{name}");
        Ok(())
    }

    // --- Types ---

    fn ty(&mut self, ty: &Type) -> Printed {
        match ty {
            Type::Instance(base, args) => {
                self.push("instance_type(");
                self.ty(base)?;
                self.push(", ");
                self.terms(args)?;
                self.push(")");
            }
            Type::Boxed(element) => {
                self.push("box_type(");
                self.ty(element)?;
                self.push(")");
            }
            Type::Buffer(element) => {
                self.push("buffer(");
                self.ty(element)?;
                self.push(")");
            }
            Type::Bool => self.push("bool"),
            Type::U8 => self.push("u8"),
            Type::Int => self.push("Int"),
            Type::Machine(ty) => self.push(ty.name()),
            Type::Prop => self.push("Prop"),
            Type::Proof(prop) => {
                self.push("@");
                self.term(prop)?;
            }
            Type::Tuple(fields) => self.types(fields)?,
            Type::Struct(id) => self.struct_name(*id)?,
            Type::Enum(id) => self.enum_name(*id)?,
            Type::Fn(params, result) => self.fn_type(params, result)?,
        }
        Ok(())
    }

    fn fn_type(&mut self, params: &[Type], result: &Type) -> Printed {
        self.push("fn");
        self.types(params)?;
        self.push(" -> ");
        self.ty(result)
    }

    // --- Terms ---

    fn term(&mut self, term: &Term) -> Printed {
        if self.sharing.is_some() && !steps::leaf_term(term) {
            return self.interned(Kind::Term, |printer| printer.term_inner(term));
        }
        self.term_inner(term)
    }

    /// Prints a composite piece into a buffer of its own, interns it, and
    /// leaves its placeholder in the text.
    fn interned(&mut self, kind: Kind, print: impl FnOnce(&mut Self) -> Printed) -> Printed {
        let outer = std::mem::take(&mut self.out);
        let printed = print(self);
        let body = std::mem::replace(&mut self.out, outer);
        printed?;
        if let Some(sharing) = &mut self.sharing {
            let id = sharing.intern(kind, body);
            self.out.push_str(&steps::placeholder(id));
        }
        Ok(())
    }

    fn instance_term(&mut self, value: &Term, args: &[Term]) -> Printed {
        self.push("instance(");
        self.term(value)?;
        self.push(", ");
        self.terms(args)?;
        self.push(")");
        Ok(())
    }

    fn term_inner(&mut self, term: &Term) -> Printed {
        match term {
            Term::Instance(value, args) => self.instance_term(value, args),
            Term::Boxed(value) => self.boxed_term(value),
            Term::Buffer {
                op,
                element,
                arguments,
            } => self.buffer_term(*op, element, arguments),
            Term::Free(id) => self.free_var(*id),
            Term::Bound(index) => self.number("#", index),
            Term::Bool(value) => self.number("", value),
            Term::U8(value) => self.number("", value),
            Term::Int(value) => self.literal(value, "i"),
            Term::Machine(ty, value) => self.literal(value, ty.name()),
            Term::Prim(prim, arguments) => self.prim(*prim, arguments),
            Term::Eq(ty, left, right) => self.equation(ty, left, right),
            Term::Implies(premise, conclusion) => self.implication(premise, conclusion),
            Term::Forall(ty, body) => self.quantifier("forall", ty, body),
            Term::Exists(ty, body) => self.quantifier("exists", ty, body),
            Term::Tuple(fields, values) => self.tuple(fields, values),
            Term::Struct(id, values) => self.struct_value(*id, values),
            Term::Proj(target, index) => self.projection(target, *index),
            Term::Proof(proof) => self.proof_term(proof),
            Term::Fn(id) => self.fn_name(*id),
            Term::Lambda {
                params,
                result,
                body,
            } => self.lambda(params, result, body),
            Term::Call(callee, arguments) => self.call(callee, arguments),
            Term::Variant(id, index, payload) => self.variant(*id, *index, payload),
            Term::Case {
                scrutinee,
                result,
                arms,
            } => self.case(scrutinee, result, arms),
            Term::PropApp(id, arguments) => self.prop_app(*id, arguments),
            Term::Absurd(proof, ty) => self.absurd(proof, ty),
            Term::For(looped) => self.for_loop(looped),
        }
    }

    fn lambda(&mut self, params: &[Type], result: &Type, body: &Term) -> Printed {
        self.push("lambda(");
        self.types(params)?;
        self.push(", ");
        self.ty(result)?;
        self.push(", ");
        self.term(body)?;
        self.push(")");
        Ok(())
    }

    fn boxed_term(&mut self, value: &Term) -> Printed {
        self.push("boxed(");
        self.term(value)?;
        self.push(")");
        Ok(())
    }

    fn buffer_term(
        &mut self,
        op: crate::kernel::BufferOp,
        element: &Type,
        arguments: &[Term],
    ) -> Printed {
        let name = match op {
            crate::kernel::BufferOp::Literal => "buffer_literal",
            crate::kernel::BufferOp::Length => "buffer_length",
            crate::kernel::BufferOp::Get => "buffer_get",
            crate::kernel::BufferOp::Set => "buffer_set",
            crate::kernel::BufferOp::Push => "buffer_push",
        };
        self.push(name);
        self.push("(");
        self.ty(element)?;
        self.push(", ");
        self.terms(arguments)?;
        self.push(")");
        Ok(())
    }

    fn free_var(&mut self, id: VarId) -> Printed {
        let position = self
            .positions
            .vars
            .iter()
            .position(|known| *known == id)
            .ok_or(PrintError::UnknownVariable)?;
        let _ = write!(self.out, "${position}");
        Ok(())
    }

    fn number(&mut self, prefix: &str, value: &impl fmt::Display) -> Printed {
        let _ = write!(self.out, "{prefix}{value}");
        Ok(())
    }

    fn literal(&mut self, value: &impl fmt::Display, suffix: &str) -> Printed {
        let _ = write!(self.out, "{value}{suffix}");
        Ok(())
    }

    fn prim(&mut self, prim: Prim, arguments: &[Term]) -> Printed {
        let _ = write!(self.out, "{prim}");
        self.terms(arguments)
    }

    fn equation(&mut self, ty: &Type, left: &Term, right: &Term) -> Printed {
        self.push("(");
        self.term(left)?;
        self.push(" ==[");
        self.ty(ty)?;
        self.push("] ");
        self.term(right)?;
        self.push(")");
        Ok(())
    }

    fn implication(&mut self, premise: &Term, conclusion: &Term) -> Printed {
        self.push("(");
        self.term(premise)?;
        self.push(" => ");
        self.term(conclusion)?;
        self.push(")");
        Ok(())
    }

    fn quantifier(&mut self, word: &str, ty: &Type, body: &Term) -> Printed {
        self.push(word);
        self.push(" (#: ");
        self.ty(ty)?;
        self.push(") { ");
        self.term(body)?;
        self.push(" }");
        Ok(())
    }

    fn tuple(&mut self, fields: &[Type], values: &[Term]) -> Printed {
        self.terms(values)?;
        self.push(" : ");
        self.types(fields)
    }

    fn struct_value(&mut self, id: StructId, values: &[Term]) -> Printed {
        self.struct_name(id)?;
        self.push(" { ");
        self.list(values, Self::term)?;
        self.push(" }");
        Ok(())
    }

    fn projection(&mut self, target: &Term, index: usize) -> Printed {
        self.term(target)?;
        let _ = write!(self.out, ".{index}");
        Ok(())
    }

    fn proof_term(&mut self, proof: &Proof) -> Printed {
        self.push("proof(");
        self.proof(proof)?;
        self.push(")");
        Ok(())
    }

    fn call(&mut self, callee: &Term, arguments: &[Term]) -> Printed {
        self.term(callee)?;
        self.terms(arguments)
    }

    fn variant(&mut self, id: EnumId, index: usize, payload: &[Term]) -> Printed {
        self.enum_name(id)?;
        let _ = write!(self.out, "::{index}");
        self.terms(payload)
    }

    fn case(&mut self, scrutinee: &Term, result: &Type, arms: &[TermArm]) -> Printed {
        self.push("case ");
        self.term(scrutinee)?;
        self.push(" : ");
        self.ty(result)?;
        self.push(" { ");
        self.list(arms, Self::term_arm)?;
        self.push(" }");
        Ok(())
    }

    fn term_arm(&mut self, arm: &TermArm) -> Printed {
        let _ = write!(self.out, "|{}| ", arm.binders);
        self.term(&arm.body)
    }

    fn prop_app(&mut self, id: PropId, arguments: &[Term]) -> Printed {
        self.prop_name(id)?;
        self.terms(arguments)
    }

    fn absurd(&mut self, proof: &Proof, ty: &Type) -> Printed {
        self.push("absurd(");
        self.proof(proof)?;
        self.push(", ");
        self.ty(ty)?;
        self.push(")");
        Ok(())
    }

    fn for_loop(&mut self, looped: &ForLoop) -> Printed {
        self.push("for(");
        self.term(&looped.lo)?;
        self.push(", ");
        self.term(&looped.hi)?;
        self.push(", ");
        self.proof(&looped.ordered)?;
        self.push(", ");
        self.types(&looped.state)?;
        self.push(", ");
        self.term(&looped.init)?;
        self.push(", ");
        self.term(&looped.body)?;
        self.push(")");
        Ok(())
    }

    // --- Proofs ---

    fn arm(&mut self, arm: &ProofArm) -> Printed {
        let _ = write!(self.out, "|{}, {}| ", arm.vars, arm.hyps);
        self.proof(&arm.body)
    }

    fn arms(&mut self, arms: &[ProofArm]) -> Printed {
        self.push("[");
        self.list(arms, Self::arm)?;
        self.push("]");
        Ok(())
    }

    fn case_known(&mut self, term: &Term, equation: &Proof) -> Printed {
        self.push("case_known(");
        self.term(term)?;
        self.push(", ");
        self.proof(equation)?;
        self.push(")");
        Ok(())
    }

    fn proof(&mut self, proof: &Proof) -> Printed {
        if self.sharing.is_some() && !steps::leaf_proof(proof) {
            return self.interned(Kind::Proof, |printer| printer.proof_inner(proof));
        }
        self.proof_inner(proof)
    }

    fn proof_inner(&mut self, proof: &Proof) -> Printed {
        let rule = proof.rule_name();
        match proof {
            Proof::CaseKnown { term, equation } => self.case_known(term, equation),
            Proof::BufferStep(term) | Proof::BufferBound { value: term, .. } => {
                self.on_term(rule, term)
            }
            Proof::Hyp(HypRef::Free(id)) => self.free_hyp(*id),
            Proof::Hyp(HypRef::Bound(index)) => self.number("#h", index),
            Proof::OfTerm(term)
            | Proof::Refl(term)
            | Proof::Projection(term)
            | Proof::Literal(term)
            | Proof::Definition(term)
            | Proof::CaseStep(term)
            | Proof::ExcludedMiddle(term)
            | Proof::ForEmpty(term)
            | Proof::Evaluate(term) => self.on_term(rule, term),
            Proof::Transport {
                eq,
                template,
                proof,
            } => self.transport(eq, template, proof),
            Proof::ImpliesIntro { hyp, body } => self.implies_intro(hyp, body),
            Proof::ImpliesElim(left, right) => self.implies_elim(left, right),
            Proof::ForallIntro { ty, body } => self.forall_intro(ty, body),
            Proof::ForallElim(universal, argument) => self.forall_elim(universal, argument),
            Proof::Construct {
                prop,
                variant,
                params,
                payload,
            } => self.construct(*prop, *variant, params, payload),
            Proof::CaseProof {
                scrutinee,
                goal,
                arms,
            } => self.case_proof(scrutinee, goal, arms),
            Proof::CaseData {
                scrutinee,
                goal,
                arms,
            } => self.case_data(scrutinee, goal, arms),
            Proof::ExistsIntro {
                prop,
                witness,
                proof,
            } => self.exists_intro(prop, witness, proof),
            Proof::ExistsElim { exists, goal, arm } => self.exists_elim(exists, goal, arm),
            Proof::ForStep {
                looped,
                lower,
                upper,
            } => self.for_step(looped, lower, upper),
            Proof::Omitted => {
                self.push("omitted");
                Ok(())
            }
            Proof::Axiom(axiom) => self.axiom(axiom),
            Proof::PropInduction {
                scrutinee,
                motive,
                arms,
            } => self.prop_induction(scrutinee, motive, arms),
            Proof::DataInduction {
                target,
                motives,
                arms,
            } => self.data_induction(target, motives, arms),
            Proof::IntInduction {
                motive,
                base,
                step,
                target,
            } => self.induction(rule, motive, base, step, target),
            Proof::Linear {
                goal,
                goal_coefficient,
                pairs,
            } => self.linear(goal, goal_coefficient, pairs),
        }
    }

    fn free_hyp(&mut self, id: HypId) -> Printed {
        let position = self
            .positions
            .hyps
            .iter()
            .position(|known| *known == id)
            .ok_or(PrintError::UnknownHypothesis)?;
        let _ = write!(self.out, "h{position}");
        Ok(())
    }

    fn on_term(&mut self, rule: &str, term: &Term) -> Printed {
        self.push(rule);
        self.push("(");
        self.term(term)?;
        self.push(")");
        Ok(())
    }

    fn transport(&mut self, eq: &Proof, template: &Term, proof: &Proof) -> Printed {
        self.push("transport(");
        self.proof(eq)?;
        self.push(", ");
        self.term(template)?;
        self.push(", ");
        self.proof(proof)?;
        self.push(")");
        Ok(())
    }

    fn implies_intro(&mut self, hyp: &Term, body: &Proof) -> Printed {
        self.push("implies_intro(");
        self.term(hyp)?;
        self.push(", ");
        self.proof(body)?;
        self.push(")");
        Ok(())
    }

    fn implies_elim(&mut self, left: &Proof, right: &Proof) -> Printed {
        self.push("implies_elim(");
        self.proof(left)?;
        self.push(", ");
        self.proof(right)?;
        self.push(")");
        Ok(())
    }

    fn forall_intro(&mut self, ty: &Type, body: &Proof) -> Printed {
        self.push("forall_intro(");
        self.ty(ty)?;
        self.push(", ");
        self.proof(body)?;
        self.push(")");
        Ok(())
    }

    fn forall_elim(&mut self, universal: &Proof, argument: &Term) -> Printed {
        self.push("forall_elim(");
        self.proof(universal)?;
        self.push(", ");
        self.term(argument)?;
        self.push(")");
        Ok(())
    }

    fn construct(
        &mut self,
        prop: PropId,
        variant: usize,
        params: &[Term],
        payload: &[Term],
    ) -> Printed {
        self.push("construct(");
        self.prop_name(prop)?;
        let _ = write!(self.out, ", {variant}, ");
        self.terms(params)?;
        self.push(", ");
        self.terms(payload)?;
        self.push(")");
        Ok(())
    }

    fn case_proof(&mut self, scrutinee: &Proof, goal: &Term, arms: &[ProofArm]) -> Printed {
        self.push("case_proof(");
        self.proof(scrutinee)?;
        self.push(", ");
        self.term(goal)?;
        self.push(", ");
        self.arms(arms)?;
        self.push(")");
        Ok(())
    }

    fn case_data(&mut self, scrutinee: &Term, goal: &Term, arms: &[ProofArm]) -> Printed {
        self.push("case_data(");
        self.term(scrutinee)?;
        self.push(", ");
        self.term(goal)?;
        self.push(", ");
        self.arms(arms)?;
        self.push(")");
        Ok(())
    }

    fn exists_intro(&mut self, prop: &Term, witness: &Term, proof: &Proof) -> Printed {
        self.push("exists_intro(");
        self.term(prop)?;
        self.push(", ");
        self.term(witness)?;
        self.push(", ");
        self.proof(proof)?;
        self.push(")");
        Ok(())
    }

    fn exists_elim(&mut self, exists: &Proof, goal: &Term, arm: &ProofArm) -> Printed {
        self.push("exists_elim(");
        self.proof(exists)?;
        self.push(", ");
        self.term(goal)?;
        self.push(", ");
        self.arm(arm)?;
        self.push(")");
        Ok(())
    }

    fn for_step(&mut self, looped: &Term, lower: &Proof, upper: &Proof) -> Printed {
        self.push("for_step(");
        self.term(looped)?;
        self.push(", ");
        self.proof(lower)?;
        self.push(", ");
        self.proof(upper)?;
        self.push(")");
        Ok(())
    }

    fn prop_induction(
        &mut self,
        scrutinee: &Proof,
        motive: &TermArm,
        arms: &[ProofArm],
    ) -> Printed {
        self.push("prop_induction(");
        self.proof(scrutinee)?;
        let _ = write!(self.out, ", |{}| ", motive.binders);
        self.term(&motive.body)?;
        self.push(", ");
        self.arms(arms)?;
        self.push(")");
        Ok(())
    }

    fn data_induction(
        &mut self,
        target: &Term,
        motives: &[(EnumId, Term)],
        arms: &[ProofArm],
    ) -> Printed {
        self.push("data_induction(");
        self.term(target)?;
        self.push(", [");
        self.list(motives, |this, (id, motive)| {
            this.push("(");
            this.enum_name(*id)?;
            this.push(", ");
            this.term(motive)?;
            this.push(")");
            Ok(())
        })?;
        self.push("], ");
        self.arms(arms)?;
        self.push(")");
        Ok(())
    }

    fn axiom(&mut self, axiom: &Axiom) -> Printed {
        self.push("axiom(");
        self.push(axiom.name());
        match axiom {
            Axiom::ViewLower(ty, _)
            | Axiom::ViewUpper(ty, _)
            | Axiom::WrapView(ty, _)
            | Axiom::ViewWrap(ty, _)
            | Axiom::WrapPeriod(ty, _) => {
                let _ = write!(self.out, "[{}]", ty.name());
            }
            Axiom::CastDef(from, to, _) => {
                let _ = write!(self.out, "[{}, {}]", from.name(), to.name());
            }
            Axiom::OpModel(op, ty, _) | Axiom::OpExact(op, ty, _) => {
                let _ = write!(self.out, "[{}, {}]", op.name(), ty.name());
            }
            Axiom::CmpReflect(_, flag) | Axiom::CmpReify(_, flag) => {
                let _ = write!(self.out, "[{flag}]");
            }
            _ => {}
        }
        for term in axiom.terms() {
            self.push(", ");
            self.term(term)?;
        }
        self.push(")");
        Ok(())
    }

    fn induction(
        &mut self,
        rule: &str,
        motive: &Term,
        base: &Proof,
        step: &ProofArm,
        target: &Term,
    ) -> Printed {
        self.push(rule);
        self.push("(");
        self.term(motive)?;
        self.push(", ");
        self.proof(base)?;
        self.push(", ");
        self.arm(step)?;
        self.push(", ");
        self.term(target)?;
        self.push(")");
        Ok(())
    }

    fn linear(
        &mut self,
        goal: &Term,
        coefficient: &Integer,
        pairs: &[(Proof, Integer)],
    ) -> Printed {
        self.push("linear(");
        self.term(goal)?;
        let _ = write!(self.out, ", {coefficient}, [");
        self.list(pairs, Self::pair)?;
        self.push("])");
        Ok(())
    }

    fn pair(&mut self, pair: &(Proof, Integer)) -> Printed {
        self.push("(");
        self.proof(&pair.0)?;
        let _ = write!(self.out, ", {})", pair.1);
        Ok(())
    }
}

fn printer<'a>(ctx: &Context, names: &'a Names) -> Printer<'a> {
    Printer {
        positions: Positions::of(ctx),
        names,
        out: String::new(),
        sharing: None,
    }
}

/// The text of a type, over the context's positions and the names.
pub fn print_type(ty: &Type, ctx: &Context, names: &Names) -> Result<String, PrintError> {
    let mut printer = printer(ctx, names);
    printer.ty(ty)?;
    Ok(printer.out)
}

/// The text of a term.
pub fn print_term(term: &Term, ctx: &Context, names: &Names) -> Result<String, PrintError> {
    let mut printer = printer(ctx, names);
    printer.term(term)?;
    Ok(printer.out)
}

/// The text of a proof: a block of named steps, `t1 = ...` and `s1 = ...`
/// one per line, the last line the conclusion, with every piece that is
/// used more than once written once (`steps.rs`).
pub fn print_proof(proof: &Proof, ctx: &Context, names: &Names) -> Result<String, PrintError> {
    let mut printer = printer(ctx, names);
    printer.sharing = Some(Sharing::default());
    printer.proof(proof)?;
    let sharing = printer.sharing.take().unwrap_or_default();
    Ok(sharing.block(&printer.out))
}

/// The text an obligation is keyed by: every entry of the context, in
/// order, each as its position, and the claim, one per line. Two
/// obligations with the same text are the same obligation: a proof of one
/// is a proof of the other, whatever the names in the source.
pub fn print_key(claim: &Term, ctx: &Context, names: &Names) -> Result<String, PrintError> {
    let mut printer = printer(ctx, names);
    let (mut vars, mut hyps) = (0, 0);
    for binding in ctx.bindings() {
        match binding {
            Binding::Var { ty, ghost, .. } => {
                let _ = write!(
                    printer.out,
                    "{} ${vars} : ",
                    if ghost { "ghost" } else { "var" }
                );
                vars += 1;
                printer.ty(ty)?;
            }
            Binding::Hyp { prop, .. } => {
                let _ = write!(printer.out, "hyp h{hyps} : ");
                hyps += 1;
                printer.term(prop)?;
            }
        }
        printer.push("\n");
    }
    printer.push("claim ");
    let start = printer.out.len();
    printer.term(claim)?;
    // The store keeps the claim, to write beside the proof of this
    // obligation and to report a stale entry against.
    let text = printer.out[start..].to_string();
    super::with_current(|store| store.expect_claim(text));
    printer.push("\n");
    Ok(printer.out)
}

// --- Reading -------------------------------------------------------------------

/// Why a text is not a term or a proof, with the byte offset it went wrong
/// at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub at: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at byte {}: {}", self.at, self.message)
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tok<'a> {
    Ident(&'a str),
    /// A decimal literal with its sign, and the letters glued after it.
    Number {
        text: &'a str,
        suffix: &'a str,
    },
    /// `$n`
    FreeVar(u32),
    /// `#n`
    BoundVar(u32),
    /// `#hn`
    BoundHyp(u32),
    Punct(&'static str),
    End,
}

const PUNCT: [&str; 14] = [
    "==[", "=>", "->", "::", "(", ")", "[", "]", "{", "}", ",", ":", ".", "|",
];

fn lex(text: &str) -> Result<Vec<(usize, Tok<'_>)>, ParseError> {
    let error = |at: usize, message: &str| ParseError {
        at,
        message: message.to_string(),
    };
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    let digits_from = |start: usize| {
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        end
    };
    let small = |start: usize, end: usize| -> Result<u32, ParseError> {
        if start == end {
            return Err(error(start, "expected digits"));
        }
        text[start..end]
            .parse()
            .map_err(|_| error(start, "the index is too large"))
    };
    while at < bytes.len() {
        let byte = bytes[at];
        if byte.is_ascii_whitespace() {
            at += 1;
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let mut end = at;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            tokens.push((at, Tok::Ident(&text[at..end])));
            at = end;
            continue;
        }
        if byte.is_ascii_digit()
            || (byte == b'-' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit))
        {
            let digits_at = at + usize::from(byte == b'-');
            let end = digits_from(digits_at);
            if end - digits_at > MAX_DIGITS {
                return Err(error(
                    at,
                    &format!("MAX_PROOF_DIGITS limit of {MAX_DIGITS} was exceeded"),
                ));
            }
            let mut suffix_end = end;
            while suffix_end < bytes.len() && bytes[suffix_end].is_ascii_alphanumeric() {
                suffix_end += 1;
            }
            tokens.push((
                at,
                Tok::Number {
                    text: &text[at..end],
                    suffix: &text[end..suffix_end],
                },
            ));
            at = suffix_end;
            continue;
        }
        if byte == b'$' {
            let end = digits_from(at + 1);
            tokens.push((at, Tok::FreeVar(small(at + 1, end)?)));
            at = end;
            continue;
        }
        if byte == b'#' {
            let next = bytes.get(at + 1).copied();
            if next == Some(b'h') && bytes.get(at + 2).is_some_and(u8::is_ascii_digit) {
                let end = digits_from(at + 2);
                tokens.push((at, Tok::BoundHyp(small(at + 2, end)?)));
                at = end;
            } else if next.is_some_and(|byte| byte.is_ascii_digit()) {
                let end = digits_from(at + 1);
                tokens.push((at, Tok::BoundVar(small(at + 1, end)?)));
                at = end;
            } else {
                // The binder of a quantifier, `(#: T)`.
                tokens.push((at, Tok::Punct("#")));
                at += 1;
            }
            continue;
        }
        if byte == b'@' {
            tokens.push((at, Tok::Punct("@")));
            at += 1;
            continue;
        }
        match PUNCT.iter().find(|punct| text[at..].starts_with(*punct)) {
            Some(punct) => {
                tokens.push((at, Tok::Punct(punct)));
                at += punct.len();
            }
            None => return Err(error(at, "unexpected character")),
        }
    }
    tokens.push((bytes.len(), Tok::End));
    Ok(tokens)
}

/// What reads the arguments of a rule, after its opening parenthesis.
type Rule<'a> = fn(&mut Parser<'a>) -> Parsed<Proof>;

pub(super) struct Parser<'a> {
    tokens: Vec<(usize, Tok<'a>)>,
    at: usize,
    positions: Positions,
    names: &'a Names,
    /// The steps a line may name: the earlier lines of its block.
    steps: &'a Steps,
    depth: usize,
    /// The nodes built so far, a step's whole tree at each use, and the
    /// deepest nesting reached: the size and depth of what is read.
    nodes: usize,
    node_limit: usize,
    deepest: usize,
}

pub(super) type Parsed<T> = Result<T, ParseError>;

impl<'a> Parser<'a> {
    fn peek(&self) -> Tok<'a> {
        self.tokens[self.at.min(self.tokens.len() - 1)].1
    }

    fn offset(&self) -> usize {
        self.tokens[self.at.min(self.tokens.len() - 1)].0
    }

    fn bump(&mut self) -> Tok<'a> {
        let token = self.peek();
        if self.at < self.tokens.len() - 1 {
            self.at += 1;
        }
        token
    }

    fn error<T>(&self, message: impl Into<String>) -> Parsed<T> {
        Err(ParseError {
            at: self.offset(),
            message: message.into(),
        })
    }

    fn expect(&mut self, punct: &'static str) -> Parsed<()> {
        if self.peek() == Tok::Punct(punct) {
            self.bump();
            Ok(())
        } else {
            self.error(format!("expected `{punct}`"))
        }
    }

    fn eat(&mut self, punct: &'static str) -> bool {
        if self.peek() == Tok::Punct(punct) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn keyword(&mut self, word: &str) -> Parsed<()> {
        if self.peek() == Tok::Ident(word) {
            self.bump();
            Ok(())
        } else {
            self.error(format!("expected `{word}`"))
        }
    }

    fn identifier(&mut self) -> Parsed<&'a str> {
        match self.peek() {
            Tok::Ident(name) => {
                self.bump();
                Ok(name)
            }
            _ => self.error("expected a name"),
        }
    }

    /// One more level of nesting, bounded by the kernel's depth limit, and
    /// one more node, bounded by `MAX_NODES`.
    fn enter(&mut self) -> Parsed<()> {
        if self.depth >= MAX_DEPTH {
            return self.error(format!(
                "MAX_KERNEL_DEPTH limit of {MAX_DEPTH} was exceeded"
            ));
        }
        self.charge_nodes(1)?;
        self.depth += 1;
        self.deepest = self.deepest.max(self.depth);
        Ok(())
    }

    fn charge_nodes(&mut self, count: usize) -> Parsed<()> {
        if count > self.node_limit.saturating_sub(self.nodes) {
            return self.error(format!(
                "MAX_PROOF_EXPANDED_NODES limit of {MAX_NODES} was exceeded"
            ));
        }
        self.nodes += count;
        Ok(())
    }

    pub(super) fn with_node_limit(mut self, limit: usize) -> Self {
        self.node_limit = limit.min(MAX_NODES);
        self
    }

    fn nested<T>(&mut self, parse: impl FnOnce(&mut Self) -> Parsed<T>) -> Parsed<T> {
        self.enter()?;
        let result = parse(self);
        self.depth -= 1;
        result
    }

    // The two are functions of their own so that the copy of a step's
    // tree lives in a frame that is not on the recursion path.

    fn term_step(&mut self, name: &str) -> Parsed<Term> {
        let step = self.step(name, Kind::Term)?;
        Ok(step.term().clone())
    }

    fn proof_step(&mut self, name: &str) -> Parsed<Proof> {
        let step = self.step(name, Kind::Proof)?;
        Ok(step.proof().clone())
    }

    /// A use of the step named `name`, of the kind the text asks for: its
    /// tree is copied in, and its size and depth are charged as if it had
    /// been written out.
    fn step(&mut self, name: &str, kind: Kind) -> Parsed<&'a steps::Step> {
        let Some(step) = self.steps.get(name) else {
            return self.error(format!("no earlier step is named `{name}`"));
        };
        if step.kind() != kind {
            return self.error(format!(
                "`{name}` is a {}, and a {} is expected here",
                step.kind().noun(),
                kind.noun()
            ));
        }
        // The step's root is the node being read, already counted and at
        // the current depth; the rest of its tree is charged.
        let (size, depth) = (step.size.saturating_sub(1), step.depth.saturating_sub(1));
        self.charge_nodes(size)?;
        if self.depth.saturating_add(depth) > MAX_DEPTH {
            return self.error(format!(
                "MAX_KERNEL_DEPTH limit of {MAX_DEPTH} was exceeded"
            ));
        }
        self.deepest = self.deepest.max(self.depth + depth);
        self.bump();
        Ok(step)
    }

    /// `open item, item, ... close`, possibly empty.
    fn list<T>(
        &mut self,
        open: &'static str,
        close: &'static str,
        mut item: impl FnMut(&mut Self) -> Parsed<T>,
    ) -> Parsed<Vec<T>> {
        self.expect(open)?;
        let mut items = Vec::new();
        if self.eat(close) {
            return Ok(items);
        }
        loop {
            items.push(item(self)?);
            if self.eat(close) {
                return Ok(items);
            }
            self.expect(",")?;
        }
    }

    /// A non-negative index, an unsuffixed literal.
    fn index(&mut self) -> Parsed<usize> {
        match self.peek() {
            Tok::Number { text, suffix: "" } if !text.starts_with('-') => {
                let value = text.parse();
                self.bump();
                value.map_err(|_| ParseError {
                    at: self.offset(),
                    message: "the index is too large".into(),
                })
            }
            _ => self.error("expected an index"),
        }
    }

    fn count(&mut self) -> Parsed<u32> {
        let index = self.index()?;
        u32::try_from(index).map_err(|_| ParseError {
            at: self.offset(),
            message: "the count is too large".into(),
        })
    }

    /// A plain integer of either sign, as a coefficient is written.
    fn integer(&mut self) -> Parsed<Integer> {
        match self.peek() {
            Tok::Number { text, suffix: "" } => {
                let value = text.parse::<Integer>();
                self.bump();
                value.map_err(|_| ParseError {
                    at: self.offset(),
                    message: "expected an integer".into(),
                })
            }
            _ => self.error("expected an integer"),
        }
    }

    fn machine_type(&mut self) -> Parsed<MachineInt> {
        let at = self.offset();
        let name = self.identifier()?;
        MachineInt::from_name(name).ok_or_else(|| ParseError {
            at,
            message: format!("`{name}` is not a machine integer type"),
        })
    }

    fn reference(&mut self) -> Parsed<String> {
        self.expect(":")?;
        let mut name = self.identifier()?.to_owned();
        while self.peek() == Tok::Punct("::")
            && matches!(self.tokens.get(self.at + 1), Some((_, Tok::Ident(_))))
        {
            self.bump();
            name.push_str("::");
            name.push_str(self.identifier()?);
        }
        Ok(name)
    }

    fn struct_id(&mut self) -> Parsed<StructId> {
        let at = self.offset();
        let name = self.reference()?;
        self.names.structs.id(&name).ok_or_else(|| ParseError {
            at,
            message: format!("no struct is named `{name}`"),
        })
    }

    fn enum_id(&mut self) -> Parsed<EnumId> {
        let at = self.offset();
        let name = self.reference()?;
        self.names.enums.id(&name).ok_or_else(|| ParseError {
            at,
            message: format!("no enum is named `{name}`"),
        })
    }

    fn prop_id(&mut self) -> Parsed<PropId> {
        let at = self.offset();
        let name = self.reference()?;
        self.names.props.id(&name).ok_or_else(|| ParseError {
            at,
            message: format!("no proposition is named `{name}`"),
        })
    }

    fn fn_id(&mut self) -> Parsed<FnId> {
        let at = self.offset();
        let name = self.reference()?;
        self.names.fns.id(&name).ok_or_else(|| ParseError {
            at,
            message: format!("no function is named `{name}`"),
        })
    }

    // --- Types ---

    fn ty(&mut self) -> Parsed<Type> {
        self.nested(Self::ty_inner)
    }

    fn container_type(&mut self, boxed: bool) -> Parsed<Type> {
        self.expect("(")?;
        let element = Box::new(self.ty()?);
        self.expect(")")?;
        Ok(if boxed {
            Type::Boxed(element)
        } else {
            Type::Buffer(element)
        })
    }

    fn instance_type(&mut self) -> Parsed<Type> {
        self.expect("(")?;
        let base = Box::new(self.ty()?);
        self.expect(",")?;
        let args = self.terms()?;
        self.expect(")")?;
        Ok(Type::Instance(base, args.into()))
    }

    fn ty_inner(&mut self) -> Parsed<Type> {
        match self.peek() {
            Tok::Punct("@") => {
                self.bump();
                Ok(Type::proof(self.term()?))
            }
            Tok::Punct("(") => Ok(Type::Tuple(self.list("(", ")", Self::ty)?)),
            Tok::Ident(name) => {
                self.bump();
                match name {
                    "instance_type" => self.instance_type(),
                    "box_type" => self.container_type(true),
                    "buffer" => self.container_type(false),
                    "bool" => Ok(Type::Bool),
                    "Int" => Ok(Type::Int),
                    "Prop" => Ok(Type::Prop),
                    "struct" => Ok(Type::Struct(self.struct_id()?)),
                    "enum" => Ok(Type::Enum(self.enum_id()?)),
                    "fn" => {
                        let params = self.list("(", ")", Self::ty)?;
                        self.expect("->")?;
                        let result = self.ty()?;
                        Ok(Type::Fn(params, Box::new(result)))
                    }
                    other => match MachineInt::from_name(other) {
                        Some(ty) => Ok(Type::machine(ty)),
                        None => self.error(format!("`{other}` is not a type")),
                    },
                }
            }
            _ => self.error("expected a type"),
        }
    }

    // --- Terms ---
    //
    // Every function on a recursion path below is kept small, with one
    // form per function: an unoptimized build gives a function with a
    // large `match` a frame holding every arm's temporaries, and the
    // nesting bound is `MAX_DEPTH` levels, each of several frames, on a
    // stack that may be 2 MiB.

    pub(super) fn term(&mut self) -> Parsed<Term> {
        self.nested(|parser| {
            // Isolate this subtree from earlier siblings when counting
            // postfix wrappers, which do not recurse through the parser.
            let enclosing_deepest = parser.deepest;
            parser.deepest = parser.depth;
            let mut term = parser.atom()?;
            loop {
                let function_depth = parser.deepest;
                if parser.eat(".") {
                    let index = parser.index()?;
                    parser.postfix_node(function_depth)?;
                    term = Term::Proj(Box::new(term), index);
                } else if parser.peek() == Tok::Punct("(") {
                    let arguments = parser.terms()?;
                    parser.postfix_node(function_depth)?;
                    term = Term::Call(Box::new(term), arguments);
                } else {
                    parser.deepest = parser.deepest.max(enclosing_deepest);
                    return Ok(term);
                }
            }
        })
    }

    fn postfix_node(&mut self, previous_depth: usize) -> Parsed<()> {
        let depth = previous_depth.saturating_add(1);
        if depth > MAX_DEPTH {
            return self.error(format!(
                "MAX_KERNEL_DEPTH limit of {MAX_DEPTH} was exceeded"
            ));
        }
        self.charge_nodes(1)?;
        self.deepest = self.deepest.max(depth);
        Ok(())
    }

    /// `(t, ..., t)`
    fn terms(&mut self) -> Parsed<Vec<Term>> {
        self.list("(", ")", Self::term)
    }

    /// `(A, ..., A)`
    fn types(&mut self) -> Parsed<Vec<Type>> {
        self.list("(", ")", Self::ty)
    }

    fn literal(&mut self, text: &str, suffix: &str) -> Parsed<Term> {
        let at = self.offset();
        let bad = |message: &str| ParseError {
            at,
            message: message.to_string(),
        };
        let integer = || {
            text.parse::<Integer>()
                .map_err(|_| bad("expected a literal"))
        };
        match suffix {
            "" => text
                .parse::<u8>()
                .map(Term::U8)
                .map_err(|_| bad("a literal without a suffix is a byte")),
            "i" => integer().map(Term::Int),
            other => match MachineInt::from_name(other) {
                Some(MachineInt::U8) => {
                    let value = integer()?;
                    match value.to_i128().and_then(|value| u8::try_from(value).ok()) {
                        Some(value) => Ok(Term::U8(value)),
                        None => Err(bad("a byte literal is out of range")),
                    }
                }
                Some(ty) => Ok(Term::Machine(ty, integer()?)),
                None => Err(bad("unknown literal suffix")),
            },
        }
    }

    fn atom(&mut self) -> Parsed<Term> {
        match self.peek() {
            Tok::FreeVar(position) => {
                self.bump();
                match self.positions.vars.get(position as usize) {
                    Some(id) => Ok(Term::Free(*id)),
                    None => self.error(format!("the context has no variable ${position}")),
                }
            }
            Tok::BoundVar(index) => {
                self.bump();
                Ok(Term::Bound(index))
            }
            Tok::Number { text, suffix } => {
                let literal = self.literal(text, suffix);
                self.bump();
                literal
            }
            Tok::Punct("(") => {
                self.bump();
                self.parenthesized()
            }
            Tok::Ident(word) if steps::is_step_name(word) => self.term_step(word),
            Tok::Ident(word) => {
                self.bump();
                self.keyword_term(word)
            }
            _ => self.error("expected a term"),
        }
    }

    /// After `(`: an equation, an implication, or a tuple value.
    fn parenthesized(&mut self) -> Parsed<Term> {
        if self.eat(")") {
            return self.tuple_type(Vec::new());
        }
        let first = Box::new(self.term()?);
        if self.eat("==[") {
            return self.equation(*first);
        }
        if self.eat("=>") {
            return self.implication(first);
        }
        self.tuple_tail(first)
    }

    // Boxing the pending term bounds parser frames at MAX_DEPTH on a 1 MiB stack.
    #[allow(clippy::boxed_local)]
    fn tuple_tail(&mut self, first: Box<Term>) -> Parsed<Term> {
        let mut values = vec![*first];
        while self.eat(",") {
            values.push(self.term()?);
        }
        self.expect(")")?;
        self.tuple_type(values)
    }

    fn implication(&mut self, first: Box<Term>) -> Parsed<Term> {
        let conclusion = self.term()?;
        self.expect(")")?;
        Ok(Term::Implies(first, Box::new(conclusion)))
    }

    /// After `(left ==[`: the type, `]`, the right side, and `)`.
    fn equation(&mut self, left: Term) -> Parsed<Term> {
        let ty = self.ty()?;
        self.expect("]")?;
        let right = self.term()?;
        self.expect(")")?;
        Ok(Term::Eq(ty, Box::new(left), Box::new(right)))
    }

    fn tuple_type(&mut self, values: Vec<Term>) -> Parsed<Term> {
        self.expect(":")?;
        let fields = self.types()?;
        Ok(Term::Tuple(fields, values))
    }

    fn quantifier(&mut self, forall: bool) -> Parsed<Term> {
        self.expect("(")?;
        self.expect("#")?;
        self.expect(":")?;
        let ty = self.ty()?;
        self.expect(")")?;
        self.expect("{")?;
        let body = Box::new(self.term()?);
        self.expect("}")?;
        Ok(if forall {
            Term::Forall(ty, body)
        } else {
            Term::Exists(ty, body)
        })
    }

    fn instance_term(&mut self) -> Parsed<Term> {
        self.expect("(")?;
        let value = Box::new(self.term()?);
        self.expect(",")?;
        let args = self.terms()?;
        self.expect(")")?;
        Ok(Term::Instance(value, args))
    }

    fn keyword_term(&mut self, word: &str) -> Parsed<Term> {
        match word {
            "instance" => self.instance_term(),
            "boxed" => self.boxed_term(),
            "buffer_literal" | "buffer_length" | "buffer_get" | "buffer_set" | "buffer_push" => {
                self.buffer_term(word)
            }
            "true" => Ok(Term::Bool(true)),
            "false" => Ok(Term::Bool(false)),
            "forall" => self.quantifier(true),
            "exists" => self.quantifier(false),
            "struct" => self.struct_value(),
            "enum" => self.variant(),
            "prop" => self.prop_app(),
            "fn" => Ok(Term::Fn(self.fn_id()?)),
            "lambda" => self.lambda(),
            "proof" => self.proof_term(),
            "case" => self.case(),
            "absurd" => self.absurd(),
            "for" => self.for_loop(),
            other => self.prim_term(other),
        }
    }

    fn lambda(&mut self) -> Parsed<Term> {
        self.expect("(")?;
        let params = self.types()?;
        self.expect(",")?;
        let result = self.ty()?;
        self.expect(",")?;
        let body = Box::new(self.term()?);
        self.expect(")")?;
        Ok(Term::Lambda {
            params,
            result,
            body,
        })
    }

    fn boxed_term(&mut self) -> Parsed<Term> {
        self.expect("(")?;
        let value = Box::new(self.term()?);
        self.expect(")")?;
        Ok(Term::Boxed(value))
    }

    fn buffer_term(&mut self, word: &str) -> Parsed<Term> {
        let op = match word {
            "buffer_literal" => crate::kernel::BufferOp::Literal,
            "buffer_length" => crate::kernel::BufferOp::Length,
            "buffer_get" => crate::kernel::BufferOp::Get,
            "buffer_set" => crate::kernel::BufferOp::Set,
            _ => crate::kernel::BufferOp::Push,
        };
        self.expect("(")?;
        let element = self.ty()?;
        self.expect(",")?;
        let arguments = self.terms()?;
        self.expect(")")?;
        Ok(Term::Buffer {
            op,
            element,
            arguments,
        })
    }

    fn struct_value(&mut self) -> Parsed<Term> {
        let id = self.struct_id()?;
        let values = self.list("{", "}", Self::term)?;
        Ok(Term::Struct(id, values))
    }

    fn variant(&mut self) -> Parsed<Term> {
        let id = self.enum_id()?;
        self.expect("::")?;
        let index = self.index()?;
        let payload = self.terms()?;
        Ok(Term::Variant(id, index, payload))
    }

    fn prop_app(&mut self) -> Parsed<Term> {
        let id = self.prop_id()?;
        let arguments = self.terms()?;
        Ok(Term::PropApp(id, arguments))
    }

    fn proof_term(&mut self) -> Parsed<Term> {
        self.expect("(")?;
        let proof = self.proof()?;
        self.expect(")")?;
        Ok(Term::Proof(Box::new(proof)))
    }

    fn case(&mut self) -> Parsed<Term> {
        let scrutinee = Box::new(self.term()?);
        self.expect(":")?;
        let result = self.ty()?;
        let arms = self.list("{", "}", Self::term_arm)?;
        Ok(Term::Case {
            scrutinee,
            result,
            arms,
        })
    }

    fn term_arm(&mut self) -> Parsed<TermArm> {
        self.expect("|")?;
        let binders = self.count()?;
        self.expect("|")?;
        let body = self.term()?;
        Ok(TermArm { binders, body })
    }

    fn absurd(&mut self) -> Parsed<Term> {
        self.expect("(")?;
        let proof = Box::new(self.proof()?);
        self.expect(",")?;
        let ty = self.ty()?;
        self.expect(")")?;
        Ok(Term::Absurd(proof, ty))
    }

    fn for_loop(&mut self) -> Parsed<Term> {
        self.expect("(")?;
        let lo = self.term()?;
        self.expect(",")?;
        let hi = self.term()?;
        self.expect(",")?;
        let ordered = self.proof()?;
        self.expect(",")?;
        let state = self.types()?;
        self.expect(",")?;
        let init = self.term()?;
        self.expect(",")?;
        let body = self.term()?;
        self.expect(")")?;
        Ok(Term::For(Box::new(ForLoop {
            lo,
            hi,
            ordered,
            state,
            init,
            body,
        })))
    }

    fn prim_term(&mut self, name: &str) -> Parsed<Term> {
        let prim = self.prim(name)?;
        let arguments = self.terms()?;
        Ok(Term::Prim(prim, arguments))
    }

    /// `[T]`
    fn one_type(&mut self) -> Parsed<MachineInt> {
        self.expect("[")?;
        let ty = self.machine_type()?;
        self.expect("]")?;
        Ok(ty)
    }

    /// `[S, T]`
    fn two_types(&mut self) -> Parsed<(MachineInt, MachineInt)> {
        self.expect("[")?;
        let from = self.machine_type()?;
        self.expect(",")?;
        let to = self.machine_type()?;
        self.expect("]")?;
        Ok((from, to))
    }

    /// The primitive named `name`, with its type parameters if it has any.
    fn prim(&mut self, name: &str) -> Parsed<Prim> {
        let plain = match name {
            "int_add" => Some(Prim::IntAdd),
            "int_sub" => Some(Prim::IntSub),
            "int_mul" => Some(Prim::IntMul),
            "int_div" => Some(Prim::IntDiv),
            "int_rem" => Some(Prim::IntRem),
            "int_neg" => Some(Prim::IntNeg),
            "int_le" => Some(Prim::IntLe),
            "int_eq_b" => Some(Prim::IntCmp(CmpOp::Eq)),
            "int_lt_b" => Some(Prim::IntCmp(CmpOp::Lt)),
            "int_le_b" => Some(Prim::IntCmp(CmpOp::Le)),
            _ => None,
        };
        if let Some(prim) = plain {
            return Ok(prim);
        }
        match name {
            "view" => Ok(Prim::View(self.one_type()?)),
            "wrap" => Ok(Prim::Wrap(self.one_type()?)),
            "cast" => {
                let (from, to) = self.two_types()?;
                Ok(Prim::Cast(from, to))
            }
            _ => {
                if let Some(op) = Op::ALL.into_iter().find(|op| op.name() == name) {
                    return Ok(Prim::Op(op, self.one_type()?));
                }
                if let Some(op) = CmpOp::ALL.into_iter().find(|op| op.name() == name) {
                    return Ok(Prim::Cmp(op, self.one_type()?));
                }
                self.error(format!("`{name}` is not a term"))
            }
        }
    }

    // --- Proofs ---

    fn arm(&mut self) -> Parsed<ProofArm> {
        self.expect("|")?;
        let vars = self.count()?;
        self.expect(",")?;
        let hyps = self.count()?;
        self.expect("|")?;
        let body = Box::new(self.proof()?);
        Ok(ProofArm { vars, hyps, body })
    }

    fn arms(&mut self) -> Parsed<Vec<ProofArm>> {
        self.list("[", "]", Self::arm)
    }

    pub(super) fn proof(&mut self) -> Parsed<Proof> {
        // Keep the small recursion frame needed at MAX_KERNEL_DEPTH, while
        // charging proof nodes just like the generic nested parser does.
        self.enter()?;
        let result = self.proof_inner();
        self.depth -= 1;
        result
    }

    /// A rule that has arguments, by name: what reads them, after the rule's
    /// opening parenthesis. `omitted` and the hypotheses are leaves.
    const RULES: [(&'static str, Rule<'a>); 29] = [
        ("buffer_step", Self::buffer_step),
        ("buffer_lower", Self::buffer_lower),
        ("buffer_upper", Self::buffer_upper),
        ("of_term", Self::of_term),
        ("refl", Self::refl),
        ("projection", Self::projection),
        ("literal", Self::literal_proof),
        ("definition", Self::definition),
        ("case_step", Self::case_step),
        ("case_known", Self::case_known),
        ("excluded_middle", Self::excluded_middle),
        ("for_empty", Self::for_empty),
        ("evaluate", Self::evaluate),
        ("transport", Self::transport),
        ("implies_intro", Self::implies_intro),
        ("implies_elim", Self::implies_elim),
        ("forall_intro", Self::forall_intro),
        ("forall_elim", Self::forall_elim),
        ("construct", Self::construct),
        ("case_proof", Self::case_proof),
        ("case_data", Self::case_data),
        ("exists_intro", Self::exists_intro),
        ("exists_elim", Self::exists_elim),
        ("for_step", Self::for_step),
        ("axiom", Self::axiom_proof),
        ("int_induction", Self::int_induction),
        ("data_induction", Self::data_induction),
        ("prop_induction", Self::prop_induction),
        ("linear", Self::linear),
    ];

    fn proof_inner(&mut self) -> Parsed<Proof> {
        let word = match self.peek() {
            Tok::Ident(word) => word,
            _ => return self.leaf_proof(),
        };
        if word == "omitted"
            || (word.starts_with('h') && word[1..].bytes().all(|byte| byte.is_ascii_digit()))
        {
            return self.leaf_proof();
        }
        if steps::is_step_name(word) {
            return self.proof_step(word);
        }
        let Some((_, rule)) = Self::RULES.iter().find(|(known, _)| *known == word) else {
            return self.error(format!("`{word}` is not a proof rule"));
        };
        self.bump();
        self.expect("(")?;
        let proof = rule(self)?;
        self.expect(")")?;
        Ok(proof)
    }

    /// A proof with nothing inside it: a hypothesis, or `omitted`.
    fn leaf_proof(&mut self) -> Parsed<Proof> {
        match self.peek() {
            Tok::BoundHyp(index) => {
                self.bump();
                Ok(Proof::Hyp(HypRef::Bound(index)))
            }
            Tok::Ident("omitted") => {
                self.bump();
                Ok(Proof::Omitted)
            }
            Tok::Ident(word) if word.starts_with('h') && word.len() > 1 => {
                let Ok(position) = word[1..].parse::<usize>() else {
                    return self.error("the index is too large");
                };
                self.bump();
                match self.positions.hyps.get(position) {
                    Some(id) => Ok(Proof::Hyp(HypRef::Free(*id))),
                    None => self.error(format!("the context has no hypothesis h{position}")),
                }
            }
            _ => self.error("expected a proof"),
        }
    }

    fn buffer_step(&mut self) -> Parsed<Proof> {
        Ok(Proof::BufferStep(self.term()?))
    }
    fn buffer_lower(&mut self) -> Parsed<Proof> {
        Ok(Proof::BufferBound {
            value: self.term()?,
            upper: false,
        })
    }
    fn buffer_upper(&mut self) -> Parsed<Proof> {
        Ok(Proof::BufferBound {
            value: self.term()?,
            upper: true,
        })
    }

    fn of_term(&mut self) -> Parsed<Proof> {
        Ok(Proof::OfTerm(self.term()?))
    }

    fn refl(&mut self) -> Parsed<Proof> {
        Ok(Proof::Refl(self.term()?))
    }

    fn projection(&mut self) -> Parsed<Proof> {
        Ok(Proof::Projection(self.term()?))
    }

    fn literal_proof(&mut self) -> Parsed<Proof> {
        Ok(Proof::Literal(self.term()?))
    }

    fn definition(&mut self) -> Parsed<Proof> {
        Ok(Proof::Definition(self.term()?))
    }

    fn case_known(&mut self) -> Parsed<Proof> {
        let term = self.term()?;
        self.expect(",")?;
        let equation = self.proof()?;
        Ok(Proof::CaseKnown {
            term,
            equation: Box::new(equation),
        })
    }

    fn case_step(&mut self) -> Parsed<Proof> {
        Ok(Proof::CaseStep(self.term()?))
    }

    fn excluded_middle(&mut self) -> Parsed<Proof> {
        Ok(Proof::ExcludedMiddle(self.term()?))
    }

    fn for_empty(&mut self) -> Parsed<Proof> {
        Ok(Proof::ForEmpty(self.term()?))
    }

    fn evaluate(&mut self) -> Parsed<Proof> {
        Ok(Proof::Evaluate(self.term()?))
    }

    fn axiom_proof(&mut self) -> Parsed<Proof> {
        Ok(Proof::Axiom(self.axiom()?))
    }

    fn int_induction(&mut self) -> Parsed<Proof> {
        self.induction()
    }

    fn transport(&mut self) -> Parsed<Proof> {
        let eq = Box::new(self.proof()?);
        self.expect(",")?;
        let template = self.term()?;
        self.expect(",")?;
        let proof = Box::new(self.proof()?);
        Ok(Proof::Transport {
            eq,
            template,
            proof,
        })
    }

    fn implies_intro(&mut self) -> Parsed<Proof> {
        let hyp = self.term()?;
        self.expect(",")?;
        let body = Box::new(self.proof()?);
        Ok(Proof::ImpliesIntro { hyp, body })
    }

    fn implies_elim(&mut self) -> Parsed<Proof> {
        let left = Box::new(self.proof()?);
        self.expect(",")?;
        let right = Box::new(self.proof()?);
        Ok(Proof::ImpliesElim(left, right))
    }

    fn forall_intro(&mut self) -> Parsed<Proof> {
        let ty = self.ty()?;
        self.expect(",")?;
        let body = Box::new(self.proof()?);
        Ok(Proof::ForallIntro { ty, body })
    }

    fn forall_elim(&mut self) -> Parsed<Proof> {
        let universal = Box::new(self.proof()?);
        self.expect(",")?;
        let argument = self.term()?;
        Ok(Proof::ForallElim(universal, argument))
    }

    fn construct(&mut self) -> Parsed<Proof> {
        self.keyword("prop")?;
        let prop = self.prop_id()?;
        self.expect(",")?;
        let variant = self.index()?;
        self.expect(",")?;
        let params = self.terms()?;
        self.expect(",")?;
        let payload = self.terms()?;
        Ok(Proof::Construct {
            prop,
            variant,
            params,
            payload,
        })
    }

    fn case_proof(&mut self) -> Parsed<Proof> {
        let scrutinee = Box::new(self.proof()?);
        self.expect(",")?;
        let goal = self.term()?;
        self.expect(",")?;
        let arms = self.arms()?;
        Ok(Proof::CaseProof {
            scrutinee,
            goal,
            arms,
        })
    }

    fn case_data(&mut self) -> Parsed<Proof> {
        let scrutinee = self.term()?;
        self.expect(",")?;
        let goal = self.term()?;
        self.expect(",")?;
        let arms = self.arms()?;
        Ok(Proof::CaseData {
            scrutinee,
            goal,
            arms,
        })
    }

    fn exists_intro(&mut self) -> Parsed<Proof> {
        let prop = self.term()?;
        self.expect(",")?;
        let witness = self.term()?;
        self.expect(",")?;
        let proof = Box::new(self.proof()?);
        Ok(Proof::ExistsIntro {
            prop,
            witness,
            proof,
        })
    }

    fn exists_elim(&mut self) -> Parsed<Proof> {
        let exists = Box::new(self.proof()?);
        self.expect(",")?;
        let goal = self.term()?;
        self.expect(",")?;
        let arm = self.arm()?;
        Ok(Proof::ExistsElim { exists, goal, arm })
    }

    fn for_step(&mut self) -> Parsed<Proof> {
        let looped = self.term()?;
        self.expect(",")?;
        let lower = Box::new(self.proof()?);
        self.expect(",")?;
        let upper = Box::new(self.proof()?);
        Ok(Proof::ForStep {
            looped,
            lower,
            upper,
        })
    }

    fn prop_induction(&mut self) -> Parsed<Proof> {
        let scrutinee = Box::new(self.proof()?);
        self.expect(",")?;
        self.expect("|")?;
        let binders = self.count()?;
        self.expect("|")?;
        let body = self.term()?;
        self.expect(",")?;
        let arms = self.arms()?;
        Ok(Proof::PropInduction {
            scrutinee,
            motive: TermArm { binders, body },
            arms,
        })
    }

    fn data_induction(&mut self) -> Parsed<Proof> {
        let target = self.term()?;
        self.expect(",")?;
        let motives = self.list("[", "]", |this| {
            this.expect("(")?;
            if this.identifier()? != "enum" {
                return this.error("expected an enum motive");
            }
            let id = this.enum_id()?;
            this.expect(",")?;
            let motive = this.term()?;
            this.expect(")")?;
            Ok((id, motive))
        })?;
        self.expect(",")?;
        let arms = self.arms()?;
        Ok(Proof::DataInduction {
            target,
            motives,
            arms,
        })
    }

    fn induction(&mut self) -> Parsed<Proof> {
        let motive = self.term()?;
        self.expect(",")?;
        let base = Box::new(self.proof()?);
        self.expect(",")?;
        let step = self.arm()?;
        self.expect(",")?;
        let target = self.term()?;
        Ok(Proof::IntInduction {
            motive,
            base,
            step,
            target,
        })
    }

    fn linear(&mut self) -> Parsed<Proof> {
        let goal = self.term()?;
        self.expect(",")?;
        let goal_coefficient = self.integer()?;
        self.expect(",")?;
        let pairs = self.list("[", "]", Self::pair)?;
        Ok(Proof::Linear {
            goal,
            goal_coefficient,
            pairs,
        })
    }

    fn pair(&mut self) -> Parsed<(Proof, Integer)> {
        self.expect("(")?;
        let proof = self.proof()?;
        self.expect(",")?;
        let coefficient = self.integer()?;
        self.expect(")")?;
        Ok((proof, coefficient))
    }

    /// `name[params], terms...`, after `axiom(`: the head says how many
    /// terms follow, which are read, and the axiom is then built.
    fn axiom(&mut self) -> Parsed<Axiom> {
        let at = self.offset();
        let head = self.axiom_head()?;
        let mut terms = Vec::with_capacity(head.arity());
        for _ in 0..head.arity() {
            self.expect(",")?;
            terms.push(self.term()?);
        }
        head.build(terms).ok_or_else(|| ParseError {
            at,
            message: "the axiom's terms".into(),
        })
    }

    fn axiom_head(&mut self) -> Parsed<AxiomHead> {
        let at = self.offset();
        let name = self.identifier()?;
        let Some(shape) = AXIOMS.iter().find(|(known, _)| *known == name) else {
            return Err(ParseError {
                at,
                message: format!("`{name}` is not an axiom"),
            });
        };
        let params = match shape.1 {
            Params::None => AxiomParams::None,
            Params::Type => AxiomParams::Type(self.one_type()?),
            Params::Cast => {
                let (from, to) = self.two_types()?;
                AxiomParams::Cast(from, to)
            }
            Params::Row => {
                self.expect("[")?;
                let op_at = self.offset();
                let op_name = self.identifier()?;
                let Some(op) = Op::ALL.into_iter().find(|op| op.name() == op_name) else {
                    return Err(ParseError {
                        at: op_at,
                        message: format!("`{op_name}` is not an operation of the table"),
                    });
                };
                self.expect(",")?;
                let ty = self.machine_type()?;
                self.expect("]")?;
                AxiomParams::Row(op, ty)
            }
            Params::Flag => {
                self.expect("[")?;
                let flag = match self.identifier()? {
                    "true" => true,
                    "false" => false,
                    _ => return self.error("expected `true` or `false`"),
                };
                self.expect("]")?;
                AxiomParams::Flag(flag)
            }
        };
        Ok(AxiomHead {
            name: shape.0,
            params,
        })
    }
}

/// What an axiom's name is followed by, inside brackets.
#[derive(Clone, Copy)]
enum Params {
    None,
    /// `[T]`, a machine type.
    Type,
    /// `[S, T]`, two machine types.
    Cast,
    /// `[op, T]`, a row of the table.
    Row,
    /// `[true]` or `[false]`.
    Flag,
}

/// Every axiom by name, with the shape of its parameters. Its terms are
/// counted by `AxiomHead::arity`.
const AXIOMS: [(&str, Params); 34] = [
    ("int_add_assoc", Params::None),
    ("int_add_comm", Params::None),
    ("int_add_zero", Params::None),
    ("int_add_neg", Params::None),
    ("int_sub_def", Params::None),
    ("int_mul_assoc", Params::None),
    ("int_mul_comm", Params::None),
    ("int_mul_one", Params::None),
    ("int_mul_add", Params::None),
    ("int_le_refl", Params::None),
    ("int_le_trans", Params::None),
    ("int_le_antisymm", Params::None),
    ("int_le_add", Params::None),
    ("int_le_mul", Params::None),
    ("int_le_total", Params::None),
    ("int_lt_irrefl", Params::None),
    ("int_div_rem", Params::None),
    ("int_div_zero", Params::None),
    ("int_rem_lower_pos", Params::None),
    ("int_rem_upper_pos", Params::None),
    ("int_rem_lower_neg", Params::None),
    ("int_rem_upper_neg", Params::None),
    ("int_rem_nonneg", Params::None),
    ("int_rem_nonpos", Params::None),
    ("view_lower", Params::Type),
    ("view_upper", Params::Type),
    ("wrap_view", Params::Type),
    ("view_wrap", Params::Type),
    ("wrap_period", Params::Type),
    ("cast_def", Params::Cast),
    ("op_model", Params::Row),
    ("op_exact", Params::Row),
    ("cmp_reflect", Params::Flag),
    ("cmp_reify", Params::Flag),
];

enum AxiomParams {
    None,
    Type(MachineInt),
    Cast(MachineInt, MachineInt),
    Row(Op, MachineInt),
    Flag(bool),
}

struct AxiomHead {
    name: &'static str,
    params: AxiomParams,
}

impl AxiomHead {
    /// How many terms the axiom takes.
    fn arity(&self) -> usize {
        match (self.name, &self.params) {
            (_, AxiomParams::Row(op, _)) => op.arity(),
            (
                "int_add_zero" | "int_add_neg" | "int_mul_one" | "int_le_refl" | "int_lt_irrefl"
                | "int_div_zero",
                _,
            ) => 1,
            (_, AxiomParams::Type(_) | AxiomParams::Cast(..) | AxiomParams::Flag(_)) => 1,
            (
                "int_add_assoc" | "int_mul_assoc" | "int_mul_add" | "int_le_trans" | "int_le_add",
                _,
            ) => 3,
            _ => 2,
        }
    }

    /// The axiom over its terms, which are exactly `arity` many.
    fn build(self, terms: Vec<Term>) -> Option<Axiom> {
        if let AxiomParams::Row(op, ty) = self.params {
            return match self.name {
                "op_model" => Some(Axiom::OpModel(op, ty, terms)),
                "op_exact" => Some(Axiom::OpExact(op, ty, terms)),
                _ => None,
            };
        }
        let mut terms = terms.into_iter();
        let mut next = || terms.next();
        let axiom = match (self.name, self.params) {
            ("int_add_assoc", _) => Axiom::IntAddAssoc(next()?, next()?, next()?),
            ("int_add_comm", _) => Axiom::IntAddComm(next()?, next()?),
            ("int_add_zero", _) => Axiom::IntAddZero(next()?),
            ("int_add_neg", _) => Axiom::IntAddNeg(next()?),
            ("int_sub_def", _) => Axiom::IntSubDef(next()?, next()?),
            ("int_mul_assoc", _) => Axiom::IntMulAssoc(next()?, next()?, next()?),
            ("int_mul_comm", _) => Axiom::IntMulComm(next()?, next()?),
            ("int_mul_one", _) => Axiom::IntMulOne(next()?),
            ("int_mul_add", _) => Axiom::IntMulAdd(next()?, next()?, next()?),
            ("int_le_refl", _) => Axiom::IntLeRefl(next()?),
            ("int_le_trans", _) => Axiom::IntLeTrans(next()?, next()?, next()?),
            ("int_le_antisymm", _) => Axiom::IntLeAntisymm(next()?, next()?),
            ("int_le_add", _) => Axiom::IntLeAdd(next()?, next()?, next()?),
            ("int_le_mul", _) => Axiom::IntLeMul(next()?, next()?),
            ("int_le_total", _) => Axiom::IntLeTotal(next()?, next()?),
            ("int_lt_irrefl", _) => Axiom::IntLtIrrefl(next()?),
            ("int_div_rem", _) => Axiom::IntDivRem(next()?, next()?),
            ("int_div_zero", _) => Axiom::IntDivZero(next()?),
            ("int_rem_lower_pos", _) => Axiom::IntRemLowerPos(next()?, next()?),
            ("int_rem_upper_pos", _) => Axiom::IntRemUpperPos(next()?, next()?),
            ("int_rem_lower_neg", _) => Axiom::IntRemLowerNeg(next()?, next()?),
            ("int_rem_upper_neg", _) => Axiom::IntRemUpperNeg(next()?, next()?),
            ("int_rem_nonneg", _) => Axiom::IntRemNonneg(next()?, next()?),
            ("int_rem_nonpos", _) => Axiom::IntRemNonpos(next()?, next()?),
            ("view_lower", AxiomParams::Type(ty)) => Axiom::ViewLower(ty, next()?),
            ("view_upper", AxiomParams::Type(ty)) => Axiom::ViewUpper(ty, next()?),
            ("wrap_view", AxiomParams::Type(ty)) => Axiom::WrapView(ty, next()?),
            ("view_wrap", AxiomParams::Type(ty)) => Axiom::ViewWrap(ty, next()?),
            ("wrap_period", AxiomParams::Type(ty)) => Axiom::WrapPeriod(ty, next()?),
            ("cast_def", AxiomParams::Cast(from, to)) => Axiom::CastDef(from, to, next()?),
            ("cmp_reflect", AxiomParams::Flag(flag)) => Axiom::CmpReflect(next()?, flag),
            ("cmp_reify", AxiomParams::Flag(flag)) => Axiom::CmpReify(next()?, flag),
            _ => return None,
        };
        Some(axiom)
    }
}

pub(super) fn parser<'a>(
    text: &'a str,
    ctx: &Context,
    names: &'a Names,
    steps: &'a Steps,
) -> Parsed<Parser<'a>> {
    if text.len() > MAX_TEXT {
        return Err(ParseError {
            at: MAX_TEXT,
            message: format!("MAX_PROOF_TEXT_BYTES limit of {MAX_TEXT} was exceeded"),
        });
    }
    Ok(Parser {
        tokens: lex(text)?,
        at: 0,
        positions: Positions::of(ctx),
        names,
        steps,
        depth: 0,
        nodes: 0,
        node_limit: MAX_NODES,
        deepest: 0,
    })
}

/// Reads one whole text as `parse` reads it, and reports what the parser
/// counted: the nodes built and the deepest nesting.
pub(super) fn whole<'a, T>(
    mut parser: Parser<'a>,
    parse: impl FnOnce(&mut Parser<'a>) -> Parsed<T>,
) -> Parsed<(T, usize, usize)> {
    let value = parse(&mut parser)?;
    if parser.peek() != Tok::End {
        return parser.error("expected the end of the text");
    }
    Ok((value, parser.nodes, parser.deepest))
}

/// Reads a bare text, one that names no steps.
fn bare<T>(
    text: &str,
    ctx: &Context,
    names: &Names,
    parse: impl for<'b> FnOnce(&mut Parser<'b>) -> Parsed<T>,
) -> Parsed<T> {
    let none = Steps::default();
    let parser = parser(text, ctx, names, &none)?;
    whole(parser, parse).map(|(value, _, _)| value)
}

/// Reads a type, over the context's positions and the names.
pub fn parse_type(text: &str, ctx: &Context, names: &Names) -> Parsed<Type> {
    bare(text, ctx, names, |parser| parser.ty())
}

/// Reads a term. What comes back is not checked: it is whatever the text
/// says, for the kernel to judge.
pub fn parse_term(text: &str, ctx: &Context, names: &Names) -> Parsed<Term> {
    bare(text, ctx, names, |parser| parser.term())
}

/// Reads a proof, with the same caveat: a block of steps, or one bare
/// expression. The tree of the proof read is what the kernel is handed;
/// the block is a way of writing it and nothing more.
pub fn parse_proof(text: &str, ctx: &Context, names: &Names) -> Parsed<Proof> {
    // Bound the whole block, not only each separately parsed line.
    if text.len() > MAX_TEXT {
        return Err(ParseError {
            at: MAX_TEXT,
            message: format!("MAX_PROOF_TEXT_BYTES limit of {MAX_TEXT} was exceeded"),
        });
    }
    let proof = if steps::is_block(text) {
        steps::parse_block(text, ctx, names)?
    } else {
        bare(text, ctx, names, |parser| parser.proof())?
    };
    // The store rewrites the entry it is reading in the writer's own form.
    if super::with_current(|_| ()).is_some() {
        let canonical = print_proof(&proof, ctx, names).ok();
        super::with_current(|store| store.read_as(canonical));
    }
    Ok(proof)
}
