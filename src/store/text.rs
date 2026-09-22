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
//! What is read is never trusted: the parser builds any kernel term the
//! text describes, well formed or not, and the caller hands the result to
//! the kernel's `check_proof`. The parser never panics on any input: the
//! text is bounded in length, a literal in digits, and nesting by the
//! kernel's own `MAX_DEPTH`.

use std::collections::HashMap;
use std::fmt::{self, Write};
use std::hash::Hash;

use crate::kernel::{
    Axiom, Binding, CmpOp, Context, EnumId, FnId, ForLoop, HypId, HypRef, Integer, MAX_DEPTH,
    MachineInt, Natural, Op, Prim, Proof, ProofArm, PropId, StructId, Term, TermArm, Type, VarId,
};

/// The most bytes of text one term or proof may be.
pub const MAX_TEXT: usize = 4 << 20;

/// The most digits a literal may have.
pub const MAX_DIGITS: usize = 4096;

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
/// identifier, `[A-Za-z_][A-Za-z0-9_]*`; anything else is refused and the
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
        if is_identifier(name) {
            self.fns.insert(name, id);
        }
    }

    pub fn structure(&mut self, name: &str, id: StructId) {
        if is_identifier(name) {
            self.structs.insert(name, id);
        }
    }

    pub fn enumeration(&mut self, name: &str, id: EnumId) {
        if is_identifier(name) {
            self.enums.insert(name, id);
        }
    }

    pub fn proposition(&mut self, name: &str, id: PropId) {
        if is_identifier(name) {
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
            Type::Bool => self.push("bool"),
            Type::U8 => self.push("u8"),
            Type::Nat => self.push("Nat"),
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
        match term {
            Term::Free(id) => self.free_var(*id),
            Term::Bound(index) => self.number("#", index),
            Term::Bool(value) => self.number("", value),
            Term::U8(value) => self.number("", value),
            Term::Nat(value) => self.literal(value, "n"),
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

    fn proof(&mut self, proof: &Proof) -> Printed {
        let rule = proof.rule_name();
        match proof {
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
            | Proof::Evaluate(term)
            | Proof::EvaluateAll(term) => self.on_term(rule, term),
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
            Proof::NatInduction {
                motive,
                base,
                step,
                target,
            }
            | Proof::IntInduction {
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
            Axiom::CmpReflect(_, flag) => {
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

/// The text of a proof.
pub fn print_proof(proof: &Proof, ctx: &Context, names: &Names) -> Result<String, PrintError> {
    let mut printer = printer(ctx, names);
    printer.proof(proof)?;
    Ok(printer.out)
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
    printer.term(claim)?;
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
            let end = digits_from(at + usize::from(byte == b'-'));
            if end - at > MAX_DIGITS + 1 {
                return Err(error(at, "the literal has too many digits"));
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

struct Parser<'a> {
    tokens: Vec<(usize, Tok<'a>)>,
    at: usize,
    positions: Positions,
    names: &'a Names,
    depth: usize,
}

type Parsed<T> = Result<T, ParseError>;

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

    /// One more level of nesting, bounded by the kernel's depth limit.
    fn nested<T>(&mut self, parse: impl FnOnce(&mut Self) -> Parsed<T>) -> Parsed<T> {
        if self.depth >= MAX_DEPTH {
            return self.error("nested too deeply");
        }
        self.depth += 1;
        let result = parse(self);
        self.depth -= 1;
        result
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

    fn reference(&mut self) -> Parsed<&'a str> {
        self.expect(":")?;
        self.identifier()
    }

    fn struct_id(&mut self) -> Parsed<StructId> {
        let at = self.offset();
        let name = self.reference()?;
        self.names.structs.id(name).ok_or_else(|| ParseError {
            at,
            message: format!("no struct is named `{name}`"),
        })
    }

    fn enum_id(&mut self) -> Parsed<EnumId> {
        let at = self.offset();
        let name = self.reference()?;
        self.names.enums.id(name).ok_or_else(|| ParseError {
            at,
            message: format!("no enum is named `{name}`"),
        })
    }

    fn prop_id(&mut self) -> Parsed<PropId> {
        let at = self.offset();
        let name = self.reference()?;
        self.names.props.id(name).ok_or_else(|| ParseError {
            at,
            message: format!("no proposition is named `{name}`"),
        })
    }

    fn fn_id(&mut self) -> Parsed<FnId> {
        let at = self.offset();
        let name = self.reference()?;
        self.names.fns.id(name).ok_or_else(|| ParseError {
            at,
            message: format!("no function is named `{name}`"),
        })
    }

    // --- Types ---

    fn ty(&mut self) -> Parsed<Type> {
        self.nested(Self::ty_inner)
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
                    "bool" => Ok(Type::Bool),
                    "Nat" => Ok(Type::Nat),
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

    fn term(&mut self) -> Parsed<Term> {
        self.nested(|parser| {
            let mut term = parser.atom()?;
            loop {
                if parser.eat(".") {
                    let index = parser.index()?;
                    term = Term::Proj(Box::new(term), index);
                } else if parser.peek() == Tok::Punct("(") {
                    let arguments = parser.terms()?;
                    term = Term::Call(Box::new(term), arguments);
                } else {
                    return Ok(term);
                }
            }
        })
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
            "n" => text
                .parse::<Natural>()
                .map(Term::Nat)
                .map_err(|_| bad("a Nat literal has no sign")),
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
        let first = self.term()?;
        if self.eat("==[") {
            return self.equation(first);
        }
        if self.eat("=>") {
            let conclusion = self.term()?;
            self.expect(")")?;
            return Ok(Term::Implies(Box::new(first), Box::new(conclusion)));
        }
        let mut values = vec![first];
        while self.eat(",") {
            values.push(self.term()?);
        }
        self.expect(")")?;
        self.tuple_type(values)
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

    fn keyword_term(&mut self, word: &str) -> Parsed<Term> {
        match word {
            "true" => Ok(Term::Bool(true)),
            "false" => Ok(Term::Bool(false)),
            "forall" => self.quantifier(true),
            "exists" => self.quantifier(false),
            "struct" => self.struct_value(),
            "enum" => self.variant(),
            "prop" => self.prop_app(),
            "fn" => Ok(Term::Fn(self.fn_id()?)),
            "proof" => self.proof_term(),
            "case" => self.case(),
            "absurd" => self.absurd(),
            "for" => self.for_loop(),
            other => self.prim_term(other),
        }
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
            "succ" => Some(Prim::Succ),
            "nat_add" => Some(Prim::NatAdd),
            "int_add" => Some(Prim::IntAdd),
            "int_sub" => Some(Prim::IntSub),
            "int_mul" => Some(Prim::IntMul),
            "int_div" => Some(Prim::IntDiv),
            "int_rem" => Some(Prim::IntRem),
            "int_neg" => Some(Prim::IntNeg),
            "int_le" => Some(Prim::IntLe),
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

    fn proof(&mut self) -> Parsed<Proof> {
        self.nested(Self::proof_inner)
    }

    /// A rule that has arguments, by name: what reads them, after the rule's
    /// opening parenthesis. `omitted` and the hypotheses are leaves.
    const RULES: [(&'static str, Rule<'a>); 25] = [
        ("of_term", Self::of_term),
        ("refl", Self::refl),
        ("projection", Self::projection),
        ("literal", Self::literal_proof),
        ("definition", Self::definition),
        ("case_step", Self::case_step),
        ("excluded_middle", Self::excluded_middle),
        ("for_empty", Self::for_empty),
        ("evaluate", Self::evaluate),
        ("evaluate_all", Self::evaluate_all),
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
        ("nat_induction", Self::nat_induction),
        ("int_induction", Self::int_induction),
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

    fn evaluate_all(&mut self) -> Parsed<Proof> {
        Ok(Proof::EvaluateAll(self.term()?))
    }

    fn axiom_proof(&mut self) -> Parsed<Proof> {
        Ok(Proof::Axiom(self.axiom()?))
    }

    fn nat_induction(&mut self) -> Parsed<Proof> {
        self.induction(true)
    }

    fn int_induction(&mut self) -> Parsed<Proof> {
        self.induction(false)
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

    fn induction(&mut self, nat: bool) -> Parsed<Proof> {
        let motive = self.term()?;
        self.expect(",")?;
        let base = Box::new(self.proof()?);
        self.expect(",")?;
        let step = self.arm()?;
        self.expect(",")?;
        let target = self.term()?;
        Ok(if nat {
            Proof::NatInduction {
                motive,
                base,
                step,
                target,
            }
        } else {
            Proof::IntInduction {
                motive,
                base,
                step,
                target,
            }
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
const AXIOMS: [(&str, Params); 37] = [
    ("nat_add_zero", Params::None),
    ("nat_add_succ", Params::None),
    ("nat_succ_injective", Params::None),
    ("nat_succ_not_zero", Params::None),
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
                "nat_add_zero" | "nat_succ_not_zero" | "int_add_zero" | "int_add_neg"
                | "int_mul_one" | "int_le_refl" | "int_lt_irrefl" | "int_div_zero",
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
            ("nat_add_zero", _) => Axiom::NatAddZero(next()?),
            ("nat_add_succ", _) => Axiom::NatAddSucc(next()?, next()?),
            ("nat_succ_injective", _) => Axiom::NatSuccInjective(next()?, next()?),
            ("nat_succ_not_zero", _) => Axiom::NatSuccNotZero(next()?),
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
            _ => return None,
        };
        Some(axiom)
    }
}

fn parser<'a>(text: &'a str, ctx: &Context, names: &'a Names) -> Parsed<Parser<'a>> {
    if text.len() > MAX_TEXT {
        return Err(ParseError {
            at: MAX_TEXT,
            message: "the text is too long".into(),
        });
    }
    Ok(Parser {
        tokens: lex(text)?,
        at: 0,
        positions: Positions::of(ctx),
        names,
        depth: 0,
    })
}

fn whole<'a, T>(
    mut parser: Parser<'a>,
    parse: impl FnOnce(&mut Parser<'a>) -> Parsed<T>,
) -> Parsed<T> {
    let value = parse(&mut parser)?;
    if parser.peek() != Tok::End {
        return parser.error("expected the end of the text");
    }
    Ok(value)
}

/// Reads a type, over the context's positions and the names.
pub fn parse_type(text: &str, ctx: &Context, names: &Names) -> Parsed<Type> {
    whole(parser(text, ctx, names)?, |parser| parser.ty())
}

/// Reads a term. What comes back is not checked: it is whatever the text
/// says, for the kernel to judge.
pub fn parse_term(text: &str, ctx: &Context, names: &Names) -> Parsed<Term> {
    whole(parser(text, ctx, names)?, |parser| parser.term())
}

/// Reads a proof, with the same caveat.
pub fn parse_proof(text: &str, ctx: &Context, names: &Names) -> Parsed<Proof> {
    whole(parser(text, ctx, names)?, |parser| parser.proof())
}
