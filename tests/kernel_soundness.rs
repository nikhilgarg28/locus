//! R2: kernel soundness under mutation. No proof is accepted for a claim
//! known to be false.
//!
//! The unit is a triple: a kernel context, a claim, and a proof the kernel
//! accepts for it there. Triples come from three places. Some are written by
//! hand below, a few for each family of rule. The lemmas of the kernel
//! theory give one each. The rest come from the corpus: every `.lc` file
//! under `examples/` and `tests/corpus/accept/` is elaborated, and the
//! elaborator records every proof its search finds, with the claim and the
//! context the kernel accepted it in (`HoleReport::found`). Walking the
//! checked program instead would miss proofs: a pure `if` or `for` lowers to
//! a kernel term, the proofs in it sit under the term's binders, and nothing
//! outside the kernel can open a binder.
//!
//! For each triple the test makes claims that are false, checks the original
//! proof against each of them, and then checks random one-node mutants of
//! the proof against them. Any acceptance is a finding. So is a mutant that
//! proves, of its own accord, anything false. A mutant accepted for the
//! original claim is not a finding, and neither is one that proves some
//! other true thing; both are counted, for interest.
//!
//! "False" is decided here, by a small evaluator that shares no code with
//! the kernel: a claim is false when some assignment of values to the
//! context's variables makes every hypothesis true and the claim false. Its
//! integers are `i128` with checked arithmetic, so a claim that computes
//! past `i128` goes undecided, as does a quantifier over `Int` or a
//! machine integer type that no sampled value settles. The machine integer
//! types are `i128` values held in range, with their own table of widths
//! and a reduction written out in modulus arithmetic. A claim the
//! evaluator cannot decide is skipped, and the skips are counted. Four
//! tests at the end check the evaluator itself.
//!
//! `LOCUS_EXTENDED=1` runs a hundred times as many mutants.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use locus::kernel::derive::{Chain, fold_claim, symm_at, unfold_claim};
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, Binding, CmpOp, Context, Definitions, FnId, HypId, HypRef, Integer, MachineInt, Mode,
    Op, Prelude, Prim, Proof, ProofArm, Term, TermArm, Type, VarId, case_variants, check_proof,
    infer_proof, infer_term,
};
use locus::parser::parse;
use locus::source::SourceMap;
use locus::typed::FnRef;

use MachineInt::{I8, I16, I32, I64, Isize32, Isize64, U8, U16, U32, U64, Usize32, Usize64};

const SEED: u64 = 0x5eed_10c5_2024_0002;

/// Random mutants per pairing of a proof with a false claim.
const MUTANTS: usize = 24;

/// At most this many false claims per triple meet mutants. Every false claim
/// meets the original proof.
const FALSE_CLAIMS: usize = 10;

fn mutants_per_pair() -> usize {
    if std::env::var_os("LOCUS_EXTENDED").is_some() {
        MUTANTS * 100
    } else {
        MUTANTS
    }
}

// --- A small generator --------------------------------------------------------

/// xorshift64*. Local on purpose: the test must not change when a shared
/// generator does.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[self.below(items.len())])
        }
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            items.swap(index, self.below(index + 1));
        }
    }
}

// --- Scenes: a kernel context that can be read back -----------------------------

#[derive(Clone, Debug)]
enum Entry {
    Var(VarId, Type),
    Hyp(HypId, Term),
}

/// A kernel context together with what was put in it. The kernel does not
/// say what a context holds, and the oracle needs to know.
#[derive(Clone)]
struct Scene {
    definitions: Rc<Definitions>,
    prelude: Prelude,
    ctx: Context,
    entries: Vec<Entry>,
}

impl Scene {
    fn new(definitions: &Rc<Definitions>) -> Self {
        Self {
            definitions: Rc::clone(definitions),
            prelude: definitions.prelude().expect("the prelude is declared"),
            ctx: Context::with_definitions(Rc::clone(definitions)),
            entries: Vec::new(),
        }
    }

    /// The scene of a context recorded elsewhere. The definitions are the
    /// file's final ones, which extend those the context was made over.
    fn of_context(definitions: &Rc<Definitions>, ctx: &Context) -> Self {
        let entries = ctx
            .bindings()
            .map(|binding| match binding {
                Binding::Var { id, ty, .. } => Entry::Var(id, ty.clone()),
                Binding::Hyp { id, prop } => Entry::Hyp(id, prop.clone()),
            })
            .collect();
        Self {
            definitions: Rc::clone(definitions),
            prelude: definitions.prelude().expect("the prelude is declared"),
            ctx: ctx.clone(),
            entries,
        }
    }

    fn declare(&mut self, ty: Type) -> Term {
        let id = self
            .ctx
            .declare(ty.clone())
            .expect("the scene declares a well-formed variable");
        self.entries.push(Entry::Var(id, ty));
        Term::Free(id)
    }

    fn assume(&mut self, prop: Term) -> HypId {
        let id = self
            .ctx
            .assume(prop.clone())
            .expect("the scene assumes a well-formed proposition");
        self.entries.push(Entry::Hyp(id, prop));
        id
    }

    fn hyps(&self) -> Vec<HypId> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Hyp(id, _) => Some(*id),
                Entry::Var(..) => None,
            })
            .collect()
    }

    fn vars(&self) -> Vec<(VarId, Type)> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Var(id, ty) => Some((*id, ty.clone())),
                Entry::Hyp(..) => None,
            })
            .collect()
    }

    fn describe(&self) -> String {
        let mut text = String::new();
        for entry in &self.entries {
            match entry {
                Entry::Var(id, ty) => text.push_str(&format!("    {id:?}: {ty}\n")),
                Entry::Hyp(id, prop) => text.push_str(&format!("    {id:?}: @ {prop}\n")),
            }
        }
        text
    }
}

struct Triple {
    origin: String,
    scene: Scene,
    claim: Term,
    proof: Proof,
}

// --- The oracle: an evaluator that shares nothing with the kernel ---------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Buffer(Vec<Value>),
    Bool(bool),
    U8(u8),
    /// An integer of the logic, as far as `i128` reaches. The oracle's
    /// arithmetic is checked, and a result outside `i128` is no value: the
    /// claim goes undecided.
    Int(i128),
    /// A value of a machine integer type other than `u8`, which is `U8`.
    /// The value lies in the range of the type: `machine_of` builds one, and
    /// a literal outside its range has no value.
    Machine(MachineInt, i128),
    /// A tuple or a struct.
    Product(Vec<Value>),
    Variant(usize, Vec<Value>),
    Fn(FnId),
    Closure {
        arity: usize,
        body: Term,
        captured: Vec<Value>,
    },
    /// A variable defined to be this proposition, which mentions only the
    /// context's variables.
    Prop(Term),
    /// A proof, or anything else with no value the oracle tracks. Proofs are
    /// irrelevant, so two of these are equal.
    Opaque,
}

impl Value {
    fn is_data(&self) -> bool {
        match self {
            Self::Bool(_) | Self::U8(_) | Self::Int(_) | Self::Machine(..) => true,
            Self::Buffer(values) | Self::Product(values) | Self::Variant(_, values) => values
                .iter()
                .all(|value| *value == Self::Opaque || value.is_data()),
            Self::Fn(_) | Self::Closure { .. } | Self::Prop(_) | Self::Opaque => false,
        }
    }

    /// Two data values of different machine types are not comparable: an
    /// equation between them is ill typed, not false.
    fn comparable(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Machine(left, _), Self::Machine(right, _)) => left == right,
            (Self::U8(_), Self::Machine(..)) | (Self::Machine(..), Self::U8(_)) => false,
            _ => true,
        }
    }
}

type Assignment = HashMap<VarId, Value>;

// --- The oracle's own table of the machine integer types ------------------------

/// The width and signedness of a machine type, from its name. Every other
/// fact about the type below is computed from this pair.
fn shape(ty: MachineInt) -> (u32, bool) {
    match ty {
        U8 => (8, false),
        U16 => (16, false),
        U32 | Usize32 => (32, false),
        U64 | Usize64 => (64, false),
        I8 => (8, true),
        I16 => (16, true),
        I32 | Isize32 => (32, true),
        I64 | Isize64 => (64, true),
    }
}

/// The least and greatest value of a machine type.
fn machine_range(ty: MachineInt) -> (i128, i128) {
    let (bits, signed) = shape(ty);
    if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    }
}

fn in_range(ty: MachineInt, value: i128) -> bool {
    let (lo, hi) = machine_range(ty);
    lo <= value && value <= hi
}

/// The one value of `ty` congruent to `value` modulo `2^bits`: the
/// remainder, made non-negative, and moved down by a period when a signed
/// type has it in its upper half. Every value of `u64` fits in `i128`, so
/// nothing here overflows.
fn machine_wrap(ty: MachineInt, value: i128) -> i128 {
    let (bits, signed) = shape(ty);
    let modulus = 1i128 << bits;
    let mut reduced = value % modulus;
    if reduced < 0 {
        reduced += modulus;
    }
    if signed && reduced >= modulus / 2 {
        reduced -= modulus;
    }
    reduced
}

/// The value of `ty` with the given number, which must be in range.
fn machine_of(ty: MachineInt, value: i128) -> Value {
    assert!(
        in_range(ty, value),
        "{value} is not a value of {}",
        ty.name()
    );
    match ty {
        U8 => Value::U8(value as u8),
        other => Value::Machine(other, value),
    }
}

/// The number of a value of `ty`, when the value is of that type.
fn machine_number(ty: MachineInt, value: &Value) -> Option<i128> {
    match (ty, value) {
        (U8, Value::U8(byte)) => Some(i128::from(*byte)),
        (ty, Value::Machine(found, number)) if *found == ty => Some(*number),
        _ => None,
    }
}

/// The values a quantifier over a machine type is tried at, and the fixed
/// part of what a variable of the type is tried at: the ends of the range
/// and their neighbours, and the small numbers around zero and around a
/// byte, where in range. A sample, like `INT_SAMPLE`.
fn machine_sample(ty: MachineInt) -> Vec<i128> {
    let (lo, hi) = machine_range(ty);
    let mut values = vec![
        lo,
        lo + 1,
        lo + 2,
        hi,
        hi - 1,
        hi - 2,
        -2,
        -1,
        0,
        1,
        2,
        127,
        128,
        255,
        256,
    ];
    values.retain(|value| in_range(ty, *value));
    values.sort_unstable();
    values.dedup();
    values
}

/// The types a machine type is confused with: the other sign at the same
/// width, and the same sign one width up and down.
fn neighbours(ty: MachineInt) -> Vec<MachineInt> {
    match ty {
        U8 => vec![I8, U16],
        U16 => vec![I16, U8, U32],
        U32 | Usize32 => vec![I32, U16, U64],
        U64 | Usize64 => vec![I64, U32],
        I8 => vec![U8, I16],
        I16 => vec![U16, I8, I32],
        I32 | Isize32 => vec![U32, I16, I64],
        I64 | Isize64 => vec![U64, I32],
    }
}

/// The oracle's reading of a row of the table of primitive operations: the
/// exact result over `i128` with checked arithmetic, reduced into the type.
/// Division and remainder are total, `a / 0` being `0` and `a % 0` being
/// `a`, as the kernel's `Int` has them; the checked operations cannot
/// fail on values of a machine type, since every product of two `u64` or
/// two `i64` values fits in `i128`. The negations exist at the signed
/// types only; at an unsigned type there is no row and no value.
fn machine_op(op: Op, ty: MachineInt, operands: &[i128]) -> Option<i128> {
    if matches!(op, Op::Neg | Op::WrappingNeg) && !shape(ty).1 {
        return None;
    }
    let exact = match (op, operands) {
        (Op::Add | Op::WrappingAdd, [a, b]) => a.checked_add(*b)?,
        (Op::Sub | Op::WrappingSub, [a, b]) => a.checked_sub(*b)?,
        (Op::Mul | Op::WrappingMul, [a, b]) => a.checked_mul(*b)?,
        (Op::Div, [a, b]) => {
            if *b == 0 {
                0
            } else {
                a.checked_div(*b)?
            }
        }
        (Op::Rem, [a, b]) => {
            if *b == 0 {
                *a
            } else {
                a.checked_rem(*b)?
            }
        }
        (Op::Neg | Op::WrappingNeg, [a]) => a.checked_neg()?,
        _ => return None,
    };
    Some(machine_wrap(ty, exact))
}

/// The operation a row is confused with: the other operation of its kind,
/// or its wrapping counterpart, or the plain one.
fn op_sibling(op: Op) -> Op {
    match op {
        Op::Add => Op::Sub,
        Op::Sub => Op::Add,
        Op::Mul => Op::WrappingMul,
        Op::Div => Op::Rem,
        Op::Rem => Op::Div,
        Op::Neg => Op::WrappingNeg,
        Op::WrappingAdd => Op::WrappingSub,
        Op::WrappingSub => Op::WrappingAdd,
        Op::WrappingMul => Op::Mul,
        Op::WrappingNeg => Op::Neg,
    }
}

/// A literal of a machine type, `u8` included.
fn lit(ty: MachineInt, value: i128) -> Term {
    Term::machine(ty, Integer::from(value))
}

fn int128(value: i128) -> Term {
    Term::Int(Integer::from(value))
}

/// How much one question to the oracle may compute.
const ORACLE_STEPS: usize = 400_000;

/// The same for a claim nobody chose: whatever a mutant happens to prove.
/// These are asked about far more often, of fewer witnesses.
const MUTANT_CLAIM_STEPS: usize = 20_000;
const MUTANT_CLAIM_WITNESSES: usize = 12;

/// The integers a quantifier over `Int` is tried at, and the ones every
/// variable of `Int` is tried at. A sample: it refutes a `forall` and proves
/// an `exists`, never the reverse.
const INT_SAMPLE: [i128; 11] = [-257, -256, -3, -2, -1, 0, 1, 2, 3, 255, 256];

struct Oracle<'a> {
    definitions: &'a Definitions,
    prelude: Prelude,
    free: Assignment,
    /// Values of the enclosing binders: `Bound(0)` is the last.
    stack: Vec<Value>,
    steps: usize,
    limit: usize,
}

impl<'a> Oracle<'a> {
    fn new(scene: &'a Scene) -> Self {
        Self {
            definitions: &scene.definitions,
            prelude: scene.prelude,
            free: Assignment::new(),
            stack: Vec::new(),
            steps: 0,
            limit: ORACLE_STEPS,
        }
    }

    fn tick(&mut self) -> Option<()> {
        self.steps += 1;
        (self.steps <= self.limit).then_some(())
    }

    /// The truth of a proposition, when it can be decided.
    fn prop(&mut self, term: &Term) -> Option<bool> {
        self.tick()?;
        match term {
            Term::Instance(..) => None,
            Term::Eq(ty, left, right) => {
                if *ty == Type::Prop {
                    // Two propositions of different truth are not equal. Of
                    // the same truth they may or may not be.
                    let (left, right) = (self.prop(left)?, self.prop(right)?);
                    return (left != right).then_some(false);
                }
                let (left, right) = (self.value(left)?, self.value(right)?);
                (left.is_data() && right.is_data() && left.comparable(&right))
                    .then_some(left == right)
            }
            Term::Implies(premise, conclusion) => {
                match (self.prop(premise), self.prop(conclusion)) {
                    (Some(false), _) | (_, Some(true)) => Some(true),
                    (Some(true), Some(false)) => Some(false),
                    _ => None,
                }
            }
            Term::Forall(ty, body) => self.quantifier(ty, body, true),
            Term::Exists(ty, body) => self.quantifier(ty, body, false),
            // The order of the integers is a proposition, not a value.
            Term::Prim(Prim::IntLe, arguments) => {
                let [left, right] = arguments.as_slice() else {
                    return None;
                };
                let (Value::Int(left), Value::Int(right)) = (self.value(left)?, self.value(right)?)
                else {
                    return None;
                };
                Some(left <= right)
            }
            Term::PropApp(id, arguments) => {
                let prelude = self.prelude;
                if *id == prelude.truth {
                    Some(true)
                } else if *id == prelude.falsehood {
                    Some(false)
                } else if *id == prelude.and {
                    let [left, right] = arguments.as_slice() else {
                        return None;
                    };
                    if self.contradicts(left, right) || self.contradicts(right, left) {
                        return Some(false);
                    }
                    match (self.prop(left), self.prop(right)) {
                        (Some(false), _) | (_, Some(false)) => Some(false),
                        (Some(true), Some(true)) => Some(true),
                        _ => None,
                    }
                } else if *id == prelude.or {
                    let [left, right] = arguments.as_slice() else {
                        return None;
                    };
                    match (self.prop(left), self.prop(right)) {
                        (Some(true), _) | (_, Some(true)) => Some(true),
                        (Some(false), Some(false)) => Some(false),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            Term::Call(callee, arguments) => {
                let (arity, body, mut captured) = match self.value(callee)? {
                    Value::Fn(id) => {
                        let (arity, body) = self.definitions.function_body(id)?;
                        (arity, body.clone(), Vec::new())
                    }
                    Value::Closure {
                        arity,
                        body,
                        captured,
                    } => (arity, body, captured),
                    _ => return None,
                };
                let arguments = self.values(arguments)?;
                if arity != arguments.len() {
                    return None;
                }
                captured.extend(arguments);
                let saved = std::mem::replace(&mut self.stack, captured);
                let result = self.prop(&body);
                self.stack = saved;
                result
            }
            Term::Case {
                scrutinee, arms, ..
            } => {
                let pushed = self.enter_arm(scrutinee, arms)?;
                let result = self.prop(&arms[pushed.0].body);
                self.stack.truncate(self.stack.len() - pushed.1);
                result
            }
            Term::Free(id) => {
                let Value::Prop(definition) = self.free.get(id)?.clone() else {
                    return None;
                };
                let saved = std::mem::take(&mut self.stack);
                let result = self.prop(&definition);
                self.stack = saved;
                result
            }
            _ => None,
        }
    }

    /// `right` is literally `left => False`.
    fn contradicts(&self, left: &Term, right: &Term) -> bool {
        match right {
            Term::Implies(premise, conclusion) => {
                **premise == *left && **conclusion == self.prelude.falsehood_prop()
            }
            _ => false,
        }
    }

    fn quantifier(&mut self, ty: &Type, body: &Term, universal: bool) -> Option<bool> {
        let (domain, complete): (Vec<Value>, bool) = match ty {
            Type::Bool => (vec![Value::Bool(false), Value::Bool(true)], true),
            Type::U8 => ((0..=255).map(Value::U8).collect(), true),
            Type::Int => (INT_SAMPLE.map(Value::Int).to_vec(), false),
            Type::Machine(ty) => (
                machine_sample(*ty)
                    .into_iter()
                    .map(|value| machine_of(*ty, value))
                    .collect(),
                false,
            ),
            // A proof has no content: quantifying over proofs of `P` is
            // assuming `P`.
            Type::Proof(prop) => {
                let premise = self.prop(prop);
                self.stack.push(Value::Opaque);
                let conclusion = self.prop(body);
                self.stack.pop();
                return match (universal, premise, conclusion) {
                    (true, Some(false), _) | (true, _, Some(true)) => Some(true),
                    (true, Some(true), Some(false)) => Some(false),
                    (false, Some(false), _) | (false, _, Some(false)) => Some(false),
                    (false, Some(true), Some(true)) => Some(true),
                    _ => None,
                };
            }
            _ => return None,
        };
        // The answer that one instance settles: a false instance refutes a
        // `forall`, a true one proves an `exists`.
        let settles = !universal;
        let mut all_known = true;
        for value in domain {
            self.stack.push(value);
            let instance = self.prop(body);
            self.stack.pop();
            match instance {
                Some(found) if found == settles => return Some(settles),
                Some(_) => {}
                None => all_known = false,
            }
        }
        (all_known && complete).then_some(!settles)
    }

    fn values(&mut self, terms: &[Term]) -> Option<Vec<Value>> {
        terms.iter().map(|term| self.value(term)).collect()
    }

    /// Chooses the arm of a case and pushes its payload. Returns the arm and
    /// how many values were pushed.
    fn enter_arm(&mut self, scrutinee: &Term, arms: &[TermArm]) -> Option<(usize, usize)> {
        let (index, payload) = match self.value(scrutinee)? {
            Value::Bool(value) => (usize::from(value), Vec::new()),
            Value::Variant(index, payload) => (index, payload),
            _ => return None,
        };
        let arm = arms.get(index)?;
        if arm.binders as usize != payload.len() {
            return None;
        }
        let pushed = payload.len();
        self.stack.extend(payload);
        Some((index, pushed))
    }

    /// The value of a data term, when it has one.
    fn value(&mut self, term: &Term) -> Option<Value> {
        self.tick()?;
        match term {
            Term::Instance(value, _) => self.value(value),
            Term::Boxed(value) => Some(Value::Product(vec![self.value(value)?])),
            Term::Buffer { op, arguments, .. } => {
                use locus::kernel::BufferOp;
                if *op == BufferOp::Literal {
                    return Some(Value::Buffer(self.values(arguments)?));
                }
                let Value::Buffer(mut items) = self.value(arguments.first()?)? else {
                    return None;
                };
                match op {
                    BufferOp::Length => Some(Value::Int(items.len() as i128)),
                    BufferOp::Get | BufferOp::Set => {
                        let Value::Int(index) = self.value(arguments.get(1)?)? else {
                            return None;
                        };
                        let index = usize::try_from(index).ok()?;
                        if *op == BufferOp::Get {
                            items.get(index).cloned()
                        } else {
                            *items.get_mut(index)? = self.value(arguments.get(4)?)?;
                            Some(Value::Buffer(items))
                        }
                    }
                    BufferOp::Push => {
                        items.push(self.value(arguments.get(1)?)?);
                        Some(Value::Buffer(items))
                    }
                    BufferOp::Literal => unreachable!(),
                }
            }
            Term::Free(id) => self.free.get(id).cloned(),
            Term::Bound(index) => {
                let position = self.stack.len().checked_sub(*index as usize + 1)?;
                Some(self.stack[position].clone())
            }
            Term::Bool(value) => Some(Value::Bool(*value)),
            Term::U8(value) => Some(Value::U8(*value)),
            Term::Int(value) => value.to_i128().map(Value::Int),
            // A literal at `u8` in this form, or outside its range, is not a
            // term and has no value.
            Term::Machine(ty, value) => {
                let value = value.to_i128()?;
                (*ty != U8 && in_range(*ty, value)).then_some(Value::Machine(*ty, value))
            }
            Term::Prim(prim, arguments) => {
                let arguments = self.values(arguments)?;
                primitive(*prim, &arguments)
            }
            Term::Tuple(_, values) | Term::Struct(_, values) => {
                Some(Value::Product(self.values(values)?))
            }
            Term::Proj(target, index) => match self.value(target)? {
                Value::Product(values) => values.get(*index).cloned(),
                _ => None,
            },
            Term::Proof(_) => Some(Value::Opaque),
            Term::Fn(id) => Some(Value::Fn(*id)),
            Term::Lambda { params, body, .. } => Some(Value::Closure {
                arity: params.len(),
                body: (**body).clone(),
                captured: self.stack.clone(),
            }),
            Term::Call(callee, arguments) => {
                let (arity, body, mut captured) = match self.value(callee)? {
                    Value::Fn(id) => {
                        let (arity, body) = self.definitions.function_body(id)?;
                        (arity, body.clone(), Vec::new())
                    }
                    Value::Closure {
                        arity,
                        body,
                        captured,
                    } => (arity, body, captured),
                    _ => return None,
                };
                let arguments = self.values(arguments)?;
                if arity != arguments.len() {
                    return None;
                }
                captured.extend(arguments);
                let saved = std::mem::replace(&mut self.stack, captured);
                let result = self.value(&body);
                self.stack = saved;
                result
            }
            Term::Variant(_, index, payload) => Some(Value::Variant(*index, self.values(payload)?)),
            Term::Case {
                scrutinee, arms, ..
            } => {
                let pushed = self.enter_arm(scrutinee, arms)?;
                let result = self.value(&arms[pushed.0].body);
                self.stack.truncate(self.stack.len() - pushed.1);
                result
            }
            Term::For(looped) => {
                let (Value::U8(lo), Value::U8(hi)) =
                    (self.value(&looped.lo)?, self.value(&looped.hi)?)
                else {
                    return None;
                };
                if lo > hi {
                    return None;
                }
                let mut state = self.value(&looped.init)?;
                for index in lo..hi {
                    self.stack.push(Value::U8(index));
                    self.stack.push(state);
                    let next = self.value(&looped.body);
                    self.stack.truncate(self.stack.len() - 2);
                    state = next?;
                }
                Some(state)
            }
            Term::Eq(..)
            | Term::Implies(..)
            | Term::Forall(..)
            | Term::Exists(..)
            | Term::PropApp(..)
            | Term::Absurd(..) => None,
        }
    }
}

/// The primitives, written from their documentation in `Prim`.
fn primitive(prim: Prim, arguments: &[Value]) -> Option<Value> {
    Some(match (prim, arguments) {
        (Prim::IntCmp(op), [Value::Int(a), Value::Int(b)]) => Value::Bool(match op {
            CmpOp::Eq => a == b,
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
        }),
        (Prim::IntAdd, [Value::Int(a), Value::Int(b)]) => Value::Int(a.checked_add(*b)?),
        (Prim::IntSub, [Value::Int(a), Value::Int(b)]) => Value::Int(a.checked_sub(*b)?),
        (Prim::IntMul, [Value::Int(a), Value::Int(b)]) => Value::Int(a.checked_mul(*b)?),
        (Prim::IntNeg, [Value::Int(a)]) => Value::Int(a.checked_neg()?),
        // Truncating, as Rust's `/` and `%`, and total: `a / 0` is `0` and
        // `a % 0` is `a`.
        (Prim::IntDiv, [Value::Int(a), Value::Int(b)]) => {
            Value::Int(if *b == 0 { 0 } else { a.checked_div(*b)? })
        }
        (Prim::IntRem, [Value::Int(a), Value::Int(b)]) => {
            Value::Int(if *b == 0 { *a } else { a.checked_rem(*b)? })
        }
        // The model of a machine type over Int: view is the inclusion, wrap
        // is reduction into the range, and cast is wrap of view.
        (Prim::View(ty), [x]) => Value::Int(machine_number(ty, x)?),
        (Prim::Wrap(ty), [Value::Int(n)]) => machine_of(ty, machine_wrap(ty, *n)),
        (Prim::Cast(from, to), [x]) => machine_of(to, machine_wrap(to, machine_number(from, x)?)),
        // A row of the table of primitive operations: the meaning in every
        // build, the exact result of the operands reduced into the type.
        (Prim::Op(op, ty), operands) => {
            let numbers: Vec<i128> = operands
                .iter()
                .map(|operand| machine_number(ty, operand))
                .collect::<Option<_>>()?;
            machine_of(ty, machine_op(op, ty, &numbers)?)
        }
        // A comparison at a machine type: the comparison of the numbers,
        // which are the views.
        (Prim::Cmp(op, ty), [a, b]) => {
            let (a, b) = (machine_number(ty, a)?, machine_number(ty, b)?);
            Value::Bool(match op {
                CmpOp::Eq => a == b,
                CmpOp::Lt => a < b,
                CmpOp::Le => a <= b,
            })
        }
        _ => return None,
    })
}

/// The comparison a comparison is confused with, as for the `u8` model's.
fn cmp_sibling(op: CmpOp) -> CmpOp {
    match op {
        CmpOp::Le => CmpOp::Lt,
        CmpOp::Lt => CmpOp::Le,
        CmpOp::Eq => CmpOp::Lt,
    }
}

// --- Witnesses: assignments under which every hypothesis holds ------------------

const SEARCH_NODES: usize = 30_000;
const WITNESSES: usize = 300;
const PRODUCT_CANDIDATES: usize = 48;

struct Search<'a> {
    scene: &'a Scene,
    oracle: Oracle<'a>,
    bytes: Vec<u8>,
    ints: Vec<i128>,
    /// A few random numbers of the full 64-bit width, either sign, for the
    /// variables of machine types; the ones in range of the type are used.
    randoms: Vec<i128>,
    nodes: usize,
    found: Vec<Assignment>,
}

/// Every byte, integer, and machine literal in a term, as a place where a
/// comparison changes. A machine literal's number is an integer worth
/// trying: it is what the literal views to.
fn literals(term: &Term, bytes: &mut Vec<u8>, ints: &mut Vec<i128>) {
    match term {
        Term::U8(byte) => bytes.push(*byte),
        Term::Int(value) | Term::Machine(_, value) => ints.extend(value.to_i128()),
        _ => {}
    }
    for child in term_children(term) {
        literals(child, bytes, ints);
    }
}

impl<'a> Search<'a> {
    fn new(scene: &'a Scene, claim: &Term, rng: &mut Rng) -> Self {
        let (mut seen_bytes, mut seen_ints) = (Vec::new(), Vec::new());
        literals(claim, &mut seen_bytes, &mut seen_ints);
        for entry in &scene.entries {
            if let Entry::Hyp(_, prop) = entry {
                literals(prop, &mut seen_bytes, &mut seen_ints);
            }
        }
        let mut bytes = vec![0, 1, 2, 3, 127, 128, 254, 255];
        for byte in seen_bytes {
            bytes.extend([byte.wrapping_sub(1), byte, byte.wrapping_add(1)]);
        }
        bytes.sort_unstable();
        bytes.dedup();
        rng.shuffle(&mut bytes);
        let mut ints = INT_SAMPLE.to_vec();
        for value in seen_ints {
            ints.extend([value.saturating_sub(1), value, value.saturating_add(1)]);
        }
        ints.sort_unstable();
        ints.dedup();
        rng.shuffle(&mut ints);
        let randoms = (0..3)
            .flat_map(|_| {
                let wide = i128::from(rng.next());
                let narrow = i128::from(rng.next() % 1000);
                [wide, -wide, narrow, -narrow]
            })
            .collect();
        Self {
            scene,
            oracle: Oracle::new(scene),
            bytes,
            ints,
            randoms,
            nodes: 0,
            found: Vec::new(),
        }
    }

    /// The values of a machine type a variable is tried at: the sample of
    /// the type, every integer literal around that lies in its range, and
    /// the random numbers that do.
    fn machine_candidates(&self, ty: MachineInt) -> Vec<Value> {
        let mut values = machine_sample(ty);
        values.extend(
            self.ints
                .iter()
                .chain(&self.randoms)
                .copied()
                .filter(|value| in_range(ty, *value)),
        );
        values.sort_unstable();
        values.dedup();
        values
            .into_iter()
            .map(|value| machine_of(ty, value))
            .collect()
    }

    /// Some values of a type. Fewer than all of them is fine: the search
    /// only ever needs one witness. A value of a product with a proof field
    /// exists only when the field's proposition holds of it.
    fn candidates(&mut self, ty: &Type, depth: usize) -> Vec<Value> {
        match ty {
            Type::Boxed(element) => self
                .candidates(element, depth + 1)
                .into_iter()
                .map(|v| Value::Product(vec![v]))
                .collect(),
            Type::Buffer(element) => {
                let mut values = vec![Value::Buffer(vec![])];
                if depth < 3 {
                    values.extend(
                        self.candidates(element, depth + 1)
                            .into_iter()
                            .take(4)
                            .map(|v| Value::Buffer(vec![v])),
                    );
                }
                values
            }
            Type::Bool => vec![Value::Bool(false), Value::Bool(true)],
            Type::U8 => self.bytes.iter().copied().map(Value::U8).collect(),
            Type::Int => self.ints.iter().copied().map(Value::Int).collect(),
            Type::Machine(machine) => self.machine_candidates(*machine),
            Type::Tuple(_) | Type::Struct(_) if depth < 3 => self.products(ty, depth),
            Type::Enum(_) if depth < 3 => {
                let Some(variants) = case_variants(&self.scene.ctx, ty) else {
                    return Vec::new();
                };
                let mut values = Vec::new();
                for (index, payload) in variants.iter().enumerate() {
                    // A payload that depends on itself is left out: there
                    // are other variants, or no witness.
                    let fields: Vec<Vec<Value>> = payload
                        .iter()
                        .map(|field| match field {
                            Type::Bool
                            | Type::U8
                            | Type::Machine(_)
                            | Type::Enum(_)
                            | Type::Struct(_) => self.candidates(field, depth + 1),
                            _ => Vec::new(),
                        })
                        .collect();
                    values.extend(
                        combinations(&fields)
                            .into_iter()
                            .map(|payload| Value::Variant(index, payload)),
                    );
                }
                values
            }
            _ => Vec::new(),
        }
    }

    fn products(&mut self, ty: &Type, depth: usize) -> Vec<Value> {
        // The kernel's typing, not its proof checking, says what the fields
        // are: project from a variable of the type until there is no field.
        let mut ctx = self.scene.ctx.clone();
        let Ok(whole) = ctx.declare_ghost(ty.clone()) else {
            return Vec::new();
        };
        let mut fields = Vec::new();
        while let Ok(field) = infer_term(
            &mut ctx,
            &Term::proj(Term::Free(whole), fields.len()),
            Mode::Logical,
        ) {
            fields.push(field);
        }
        let choices: Vec<Vec<Value>> = fields
            .iter()
            .map(|field| match field {
                Type::Proof(_) => vec![Value::Opaque],
                _ => self.candidates(field, depth + 1),
            })
            .collect();
        let mut values = Vec::new();
        for product in combinations(&choices) {
            let value = Value::Product(product);
            self.oracle.free.insert(whole, value.clone());
            let inhabited = fields.iter().all(|field| match field {
                Type::Proof(prop) => self.oracle.prop(prop) == Some(true),
                _ => true,
            });
            self.oracle.free.remove(&whole);
            if inhabited {
                values.push(value);
            }
        }
        values
    }

    fn run(&mut self, position: usize) {
        self.nodes += 1;
        if self.nodes > SEARCH_NODES || self.found.len() >= WITNESSES * 8 {
            return;
        }
        self.oracle.steps = 0;
        let scene = self.scene;
        let Some(entry) = scene.entries.get(position) else {
            self.found.push(self.oracle.free.clone());
            return;
        };
        match entry {
            Entry::Hyp(_, prop) => {
                if self.oracle.prop(prop) == Some(true) {
                    self.run(position + 1);
                }
            }
            // Evidence in scope: its proposition holds.
            Entry::Var(_, Type::Proof(prop)) => {
                if self.oracle.prop(prop) == Some(true) {
                    self.run(position + 1);
                }
            }
            Entry::Var(id, ty) => {
                // A defined variable has the value of its definition. When
                // that has no value here the variable stays unassigned,
                // which is sound: the definition is what it equals, and
                // anything that mentions it goes undecided.
                let definition = match scene.entries.get(position + 1) {
                    Some(Entry::Hyp(_, Term::Eq(_, left, value))) if **left == Term::Free(*id) => {
                        Some(value)
                    }
                    _ => None,
                };
                if let Some(value) = definition {
                    let known = match self.oracle.value(value) {
                        Some(value) => Some(value),
                        None if *ty == Type::Prop => Some(Value::Prop((**value).clone())),
                        None => None,
                    };
                    match known {
                        Some(value) => {
                            self.oracle.free.insert(*id, value);
                            self.run(position + 2);
                            self.oracle.free.remove(id);
                        }
                        None => self.run(position + 2),
                    }
                    return;
                }
                // Every proposition exists; nothing needs its value.
                if *ty == Type::Prop {
                    self.run(position + 1);
                    return;
                }
                for value in self.candidates(ty, 0) {
                    self.oracle.free.insert(*id, value);
                    self.run(position + 1);
                    self.oracle.free.remove(id);
                }
            }
        }
    }
}

/// The product of the choices, thinned to a fixed number when it is large.
fn combinations(choices: &[Vec<Value>]) -> Vec<Vec<Value>> {
    let total: usize = choices.iter().map(Vec::len).product();
    if total == 0 {
        return Vec::new();
    }
    let count = total.min(PRODUCT_CANDIDATES);
    (0..count)
        .map(|k| {
            let mut index = k * total / count;
            choices
                .iter()
                .map(|choice| {
                    let value = choice[index % choice.len()].clone();
                    index /= choice.len();
                    value
                })
                .collect()
        })
        .collect()
}

/// Assignments that satisfy the scene. Empty when none was found, and then
/// nothing is known to be false there.
fn witnesses(scene: &Scene, claim: &Term, rng: &mut Rng) -> Vec<Assignment> {
    let mut search = Search::new(scene, claim, rng);
    search.run(0);
    let mut found = search.found;
    if found.len() > WITNESSES {
        let step = found.len() / WITNESSES;
        found = found.into_iter().step_by(step).collect();
    }
    found
}

/// The first witness at which the claim is false.
fn refute<'w>(scene: &Scene, claim: &Term, witnesses: &'w [Assignment]) -> Option<&'w Assignment> {
    refute_within(scene, claim, witnesses, ORACLE_STEPS)
}

fn refute_within<'w>(
    scene: &Scene,
    claim: &Term,
    witnesses: &'w [Assignment],
    limit: usize,
) -> Option<&'w Assignment> {
    let mut oracle = Oracle::new(scene);
    oracle.limit = limit;
    witnesses.iter().find(|witness| {
        oracle.free = (*witness).clone();
        oracle.steps = 0;
        oracle.stack.clear();
        oracle.prop(claim) == Some(false)
    })
}

fn describe_witness(witness: &Assignment) -> String {
    let mut pairs: Vec<String> = witness
        .iter()
        .map(|(id, value)| format!("{id:?} = {value:?}"))
        .collect();
    pairs.sort();
    format!("{{{}}}", pairs.join(", "))
}

// --- Walking terms ----------------------------------------------------------------

fn term_children(term: &Term) -> Vec<&Term> {
    match term {
        Term::Instance(value, args) => std::iter::once(&**value).chain(args).collect(),
        Term::Free(_)
        | Term::Bound(_)
        | Term::Bool(_)
        | Term::U8(_)
        | Term::Int(_)
        | Term::Machine(..)
        | Term::Proof(_)
        | Term::Fn(_)
        | Term::Absurd(..) => Vec::new(),
        Term::Boxed(value) => vec![value],
        Term::Buffer {
            arguments: terms, ..
        }
        | Term::Prim(_, terms)
        | Term::Tuple(_, terms)
        | Term::Struct(_, terms)
        | Term::Variant(_, _, terms)
        | Term::PropApp(_, terms) => terms.iter().collect(),
        Term::Eq(_, left, right) | Term::Implies(left, right) => vec![left, right],
        Term::Forall(_, body) | Term::Exists(_, body) | Term::Lambda { body, .. } => vec![body],
        Term::Proj(target, _) => vec![target],
        Term::Call(callee, arguments) => std::iter::once(&**callee).chain(arguments).collect(),
        Term::Case {
            scrutinee, arms, ..
        } => std::iter::once(&**scrutinee)
            .chain(arms.iter().map(|arm| &arm.body))
            .collect(),
        Term::For(looped) => vec![&looped.lo, &looped.hi, &looped.init, &looped.body],
    }
}

/// The term with its children replaced, in the order `term_children` gives.
fn term_with_children(term: &Term, children: Vec<Term>) -> Term {
    let mut next = children.into_iter();
    let mut take = || next.next().expect("one replacement per child");
    match term {
        Term::Instance(_, args) => {
            Term::Instance(Box::new(take()), args.iter().map(|_| take()).collect())
        }
        Term::Free(_)
        | Term::Bound(_)
        | Term::Bool(_)
        | Term::U8(_)
        | Term::Int(_)
        | Term::Machine(..)
        | Term::Proof(_)
        | Term::Fn(_)
        | Term::Absurd(..) => term.clone(),
        Term::Boxed(_) => Term::Boxed(Box::new(take())),
        Term::Buffer {
            op,
            element,
            arguments,
        } => Term::Buffer {
            op: *op,
            element: element.clone(),
            arguments: arguments.iter().map(|_| take()).collect(),
        },
        Term::Lambda { params, result, .. } => Term::Lambda {
            params: params.clone(),
            result: result.clone(),
            body: Box::new(take()),
        },
        Term::Prim(prim, terms) => Term::Prim(*prim, terms.iter().map(|_| take()).collect()),
        Term::Tuple(fields, terms) => {
            Term::Tuple(fields.clone(), terms.iter().map(|_| take()).collect())
        }
        Term::Struct(id, terms) => Term::Struct(*id, terms.iter().map(|_| take()).collect()),
        Term::Variant(id, index, terms) => {
            Term::Variant(*id, *index, terms.iter().map(|_| take()).collect())
        }
        Term::PropApp(id, terms) => Term::PropApp(*id, terms.iter().map(|_| take()).collect()),
        Term::Eq(ty, _, _) => Term::eq(ty.clone(), take(), take()),
        Term::Implies(_, _) => Term::implies(take(), take()),
        Term::Forall(ty, _) => Term::Forall(ty.clone(), Box::new(take())),
        Term::Exists(ty, _) => Term::Exists(ty.clone(), Box::new(take())),
        Term::Proj(_, index) => Term::proj(take(), *index),
        Term::Call(_, arguments) => {
            let callee = take();
            Term::call(callee, arguments.iter().map(|_| take()).collect())
        }
        Term::Case { result, arms, .. } => Term::Case {
            scrutinee: Box::new(take()),
            result: result.clone(),
            arms: arms
                .iter()
                .map(|arm| TermArm {
                    binders: arm.binders,
                    body: take(),
                })
                .collect(),
        },
        Term::For(looped) => {
            let mut looped = (**looped).clone();
            looped.lo = take();
            looped.hi = take();
            looped.init = take();
            looped.body = take();
            Term::For(Box::new(looped))
        }
    }
}

/// Every term that differs from `term` at exactly one node, where `local`
/// says what a node may become.
fn rewrites(term: &Term, local: &dyn Fn(&Term) -> Vec<Term>) -> Vec<Term> {
    let mut out = local(term);
    let children = term_children(term);
    for (position, child) in children.iter().enumerate() {
        for replacement in rewrites(child, local) {
            let mut changed: Vec<Term> = children.iter().map(|child| (*child).clone()).collect();
            changed[position] = replacement;
            out.push(term_with_children(term, changed));
        }
    }
    out
}

/// What one node of a claim may become: a neighbouring literal, the other
/// end of a machine range, a sibling comparison or operation, the same
/// primitive at a neighbouring machine type, swapped sides, `<=` made
/// strict or a strict `<=` relaxed, another variable, a negation.
fn perturb_node(term: &Term, prelude: &Prelude, vars: &[(VarId, Type)]) -> Vec<Term> {
    let mut out = Vec::new();
    match term {
        Term::U8(byte) => {
            out.push(Term::U8(byte.wrapping_add(1)));
            out.push(Term::U8(byte.wrapping_sub(1)));
        }
        Term::Int(value) => {
            out.push(Term::Int(value.add(&Integer::from(1i64))));
            out.push(Term::Int(value.sub(&Integer::from(1i64))));
            // The bound of a range for the other bound: a claim about the
            // range of a machine type then names the wrong end.
            if let Some(value) = value.to_i128() {
                for ty in MachineInt::FIXED {
                    let (lo, hi) = machine_range(ty);
                    if value == lo {
                        out.push(int128(hi));
                    }
                    if value == hi {
                        out.push(int128(lo));
                    }
                }
            }
        }
        // A machine literal moves to a neighbour within its range, and from
        // one end of the range to the other.
        Term::Machine(ty, value) => {
            if let Some(value) = value.to_i128() {
                let (lo, hi) = machine_range(*ty);
                if value < hi {
                    out.push(lit(*ty, value + 1));
                }
                if value > lo {
                    out.push(lit(*ty, value - 1));
                }
                if value == lo {
                    out.push(lit(*ty, hi));
                }
                if value == hi {
                    out.push(lit(*ty, lo));
                }
            }
        }
        Term::Bool(value) => out.push(Term::Bool(!value)),
        Term::Bound(index) => {
            out.push(Term::Bound(index + 1));
            if *index > 0 {
                out.push(Term::Bound(index - 1));
            }
        }
        Term::Free(id) => {
            let ty = vars.iter().find(|(var, _)| var == id).map(|(_, ty)| ty);
            for (other, other_ty) in vars {
                if other != id && Some(other_ty) == ty && !matches!(other_ty, Type::Proof(_)) {
                    out.push(Term::Free(*other));
                }
            }
        }
        Term::Prim(prim, arguments) => {
            let sibling = match prim {
                Prim::IntAdd => Some(Prim::IntSub),
                Prim::IntSub => Some(Prim::IntAdd),
                Prim::IntMul => Some(Prim::IntAdd),
                Prim::IntDiv => Some(Prim::IntRem),
                Prim::IntRem => Some(Prim::IntDiv),
                // `<` is not a primitive of its own; it is handled below.
                Prim::IntNeg => None,
                Prim::IntLe => None,
                Prim::IntCmp(op) => Some(Prim::IntCmp(cmp_sibling(*op))),
                // The machine primitives have several siblings each, below.
                Prim::View(_) | Prim::Wrap(_) | Prim::Cast(..) => None,
                // A row of the table: the sibling operation at the same type.
                Prim::Op(op, ty) => Some(Prim::Op(op_sibling(*op), *ty)),
                Prim::Cmp(op, ty) => Some(Prim::Cmp(cmp_sibling(*op), *ty)),
            };
            if let Some(sibling) = sibling {
                out.push(Term::Prim(sibling, arguments.clone()));
            }
            // The same primitive at a neighbouring type, and a cast turned
            // around. A view or a cast so changed expects an argument of
            // another type, and a wrap or a cast so changed has another
            // result type: in a proof the kernel must notice, and in a
            // claim the oracle finds the equation ill typed and leaves it.
            let retyped: Vec<Prim> = match prim {
                Prim::View(ty) => neighbours(*ty).into_iter().map(Prim::View).collect(),
                Prim::Wrap(ty) => neighbours(*ty).into_iter().map(Prim::Wrap).collect(),
                Prim::Op(op, ty) => neighbours(*ty)
                    .into_iter()
                    .map(|other| Prim::Op(*op, other))
                    .collect(),
                Prim::Cmp(op, ty) => neighbours(*ty)
                    .into_iter()
                    .map(|other| Prim::Cmp(*op, other))
                    .collect(),
                Prim::Cast(from, to) => {
                    let mut casts: Vec<Prim> = neighbours(*to)
                        .into_iter()
                        .map(|other| Prim::Cast(*from, other))
                        .collect();
                    if from != to {
                        casts.push(Prim::Cast(*to, *from));
                    }
                    casts
                }
                _ => Vec::new(),
            };
            for prim in retyped {
                out.push(Term::Prim(prim, arguments.clone()));
            }
            if let [left, right] = arguments.as_slice() {
                out.push(Term::Prim(*prim, vec![right.clone(), left.clone()]));
            }
            if let (Prim::IntNeg, Some(first)) = (prim, arguments.first()) {
                out.push(first.clone());
            }
            // `a <= b` to `a < b`, which is `a + 1 <= b`, and back.
            if let (Prim::IntLe, [left, right]) = (prim, arguments.as_slice()) {
                out.push(Term::int_lt(left.clone(), right.clone()));
                if let Term::Prim(Prim::IntAdd, sum) = left
                    && let [inner, Term::Int(one)] = sum.as_slice()
                    && *one == Integer::from(1i64)
                {
                    out.push(Term::int_le(inner.clone(), right.clone()));
                }
            }
        }
        Term::Call(callee, arguments) => {
            if let [left, right] = arguments.as_slice() {
                out.push(Term::call(
                    (**callee).clone(),
                    vec![right.clone(), left.clone()],
                ));
            }
        }
        Term::Eq(..) => out.push(prelude.not_prop(term.clone())),
        Term::Implies(premise, conclusion) => {
            out.push(Term::implies((**conclusion).clone(), (**premise).clone()));
            out.push((**conclusion).clone());
        }
        Term::PropApp(id, arguments) if *id == prelude.or => {
            out.push(Term::PropApp(prelude.and, arguments.clone()));
        }
        Term::Exists(ty, body) => out.push(Term::Forall(ty.clone(), body.clone())),
        _ => {}
    }
    out
}

fn perturbations(term: &Term, scene: &Scene) -> Vec<Term> {
    let vars = scene.vars();
    rewrites(term, &|node| perturb_node(node, &scene.prelude, &vars))
}

/// Candidates for a false claim. The oracle decides which of them are.
fn false_candidates(triple: &Triple) -> Vec<Term> {
    let prelude = &triple.scene.prelude;
    let claim = &triple.claim;
    let negation = prelude.not_prop(claim.clone());
    let mut out = vec![
        prelude.falsehood_prop(),
        negation.clone(),
        prelude.and_prop(claim.clone(), negation),
    ];
    out.extend(perturbations(claim, &triple.scene));
    out
}

// --- Walking proofs ---------------------------------------------------------------

fn embedded_in(terms: &[Term]) -> Vec<(&Proof, u32, u32)> {
    terms
        .iter()
        .filter_map(|term| match term {
            Term::Proof(proof) => Some((&**proof, 0, 0)),
            _ => None,
        })
        .collect()
}

fn arm_bodies(arms: &[ProofArm]) -> Vec<(&Proof, u32, u32)> {
    arms.iter()
        .map(|arm| (&*arm.body, arm.vars, arm.hyps))
        .collect()
}

/// A proof's immediate subproofs, each with the term and hypothesis binders
/// it is under. Proofs passed as arguments of a lemma call or as the payload
/// of a constructor count: that is where an elaborated proof keeps most of
/// its structure.
fn proof_children(proof: &Proof) -> Vec<(&Proof, u32, u32)> {
    match proof {
        Proof::CaseKnown { equation, .. } => vec![(equation, 0, 0)],
        Proof::BufferStep(_)
        | Proof::BufferBound { .. }
        | Proof::Hyp(_)
        | Proof::Refl(_)
        | Proof::Projection(_)
        | Proof::Literal(_)
        | Proof::Definition(_)
        | Proof::CaseStep(_)
        | Proof::ExcludedMiddle(_)
        | Proof::ForEmpty(_)
        | Proof::Omitted
        | Proof::Evaluate(_)
        | Proof::Axiom(_) => Vec::new(),
        Proof::OfTerm(term) => match term {
            Term::Call(_, arguments) => embedded_in(arguments),
            _ => Vec::new(),
        },
        Proof::Transport { eq, proof, .. } => vec![(eq, 0, 0), (proof, 0, 0)],
        Proof::ImpliesIntro { body, .. } => vec![(body, 0, 1)],
        Proof::ImpliesElim(implication, premise) => vec![(implication, 0, 0), (premise, 0, 0)],
        Proof::ForallIntro { body, .. } => vec![(body, 1, 0)],
        Proof::ForallElim(universal, _) => vec![(universal, 0, 0)],
        Proof::Construct { payload, .. } => embedded_in(payload),
        Proof::CaseProof {
            scrutinee,
            arms: list,
            ..
        } => {
            let mut out = vec![(&**scrutinee, 0, 0)];
            out.extend(arm_bodies(list));
            out
        }
        Proof::PropInduction {
            scrutinee, arms, ..
        } => {
            let mut out = vec![(&**scrutinee, 0, 0)];
            out.extend(arm_bodies(arms));
            out
        }
        Proof::CaseData { arms: list, .. } | Proof::DataInduction { arms: list, .. } => {
            arm_bodies(list)
        }
        Proof::ExistsIntro { proof, .. } => vec![(proof, 0, 0)],
        Proof::ExistsElim { exists, arm, .. } => {
            vec![(exists, 0, 0), (&*arm.body, arm.vars, arm.hyps)]
        }
        Proof::ForStep { lower, upper, .. } => vec![(lower, 0, 0), (upper, 0, 0)],
        Proof::IntInduction { base, step, .. } => {
            vec![(base, 0, 0), (&*step.body, step.vars, step.hyps)]
        }
        Proof::Linear { pairs, .. } => pairs.iter().map(|(proof, _)| (proof, 0, 0)).collect(),
    }
}

/// The proof with its subproofs replaced, in the order `proof_children`
/// gives.
fn proof_with_children(proof: &Proof, children: Vec<Proof>) -> Proof {
    let mut next = children.into_iter();
    let mut take = || next.next().expect("one replacement per subproof");
    let embedded = |terms: &[Term], take: &mut dyn FnMut() -> Proof| -> Vec<Term> {
        terms
            .iter()
            .map(|term| match term {
                Term::Proof(_) => Term::proof(take()),
                other => other.clone(),
            })
            .collect()
    };
    let rearm = |arm: &ProofArm, body: Proof| ProofArm {
        vars: arm.vars,
        hyps: arm.hyps,
        body: Box::new(body),
    };
    match proof {
        Proof::CaseKnown { term, .. } => Proof::CaseKnown {
            term: term.clone(),
            equation: Box::new(take()),
        },
        Proof::OfTerm(Term::Call(callee, arguments)) => Proof::OfTerm(Term::call(
            (**callee).clone(),
            embedded(arguments, &mut take),
        )),
        Proof::Transport { template, .. } => Proof::Transport {
            eq: Box::new(take()),
            template: template.clone(),
            proof: Box::new(take()),
        },
        Proof::ImpliesIntro { hyp, .. } => Proof::ImpliesIntro {
            hyp: hyp.clone(),
            body: Box::new(take()),
        },
        Proof::ImpliesElim(..) => {
            let implication = take();
            Proof::implies_elim(implication, take())
        }
        Proof::ForallIntro { ty, .. } => Proof::ForallIntro {
            ty: ty.clone(),
            body: Box::new(take()),
        },
        Proof::ForallElim(_, argument) => Proof::forall_elim(take(), argument.clone()),
        Proof::Construct {
            prop,
            variant,
            params,
            payload,
        } => Proof::Construct {
            prop: *prop,
            variant: *variant,
            params: params.clone(),
            payload: embedded(payload, &mut take),
        },
        Proof::CaseProof { goal, arms, .. } => Proof::CaseProof {
            scrutinee: Box::new(take()),
            goal: goal.clone(),
            arms: arms.iter().map(|arm| rearm(arm, take())).collect(),
        },
        Proof::CaseData {
            scrutinee,
            goal,
            arms,
        } => Proof::CaseData {
            scrutinee: scrutinee.clone(),
            goal: goal.clone(),
            arms: arms.iter().map(|arm| rearm(arm, take())).collect(),
        },
        Proof::ExistsIntro { prop, witness, .. } => Proof::ExistsIntro {
            prop: prop.clone(),
            witness: witness.clone(),
            proof: Box::new(take()),
        },
        Proof::ExistsElim { goal, arm, .. } => Proof::ExistsElim {
            exists: Box::new(take()),
            goal: goal.clone(),
            arm: rearm(arm, take()),
        },
        Proof::ForStep { looped, .. } => Proof::ForStep {
            looped: looped.clone(),
            lower: Box::new(take()),
            upper: Box::new(take()),
        },
        Proof::PropInduction { motive, arms, .. } => Proof::PropInduction {
            scrutinee: Box::new(take()),
            motive: motive.clone(),
            arms: arms.iter().map(|arm| rearm(arm, take())).collect(),
        },
        Proof::DataInduction {
            target,
            motives,
            arms,
        } => Proof::DataInduction {
            target: target.clone(),
            motives: motives.clone(),
            arms: arms.iter().map(|arm| rearm(arm, take())).collect(),
        },
        Proof::IntInduction {
            motive,
            step,
            target,
            ..
        } => Proof::IntInduction {
            motive: motive.clone(),
            base: Box::new(take()),
            step: rearm(step, take()),
            target: target.clone(),
        },
        Proof::Linear {
            goal,
            goal_coefficient,
            pairs,
        } => Proof::Linear {
            goal: goal.clone(),
            goal_coefficient: goal_coefficient.clone(),
            pairs: pairs
                .iter()
                .map(|(_, coefficient)| (take(), coefficient.clone()))
                .collect(),
        },
        leaf => leaf.clone(),
    }
}

/// The same axiom, instantiated at `terms` instead: the inverse of
/// `Axiom::terms`. A flag the axiom carries is kept. Everything else here
/// that handles axioms goes through this and `Axiom::terms`, so a new axiom
/// needs an arm here and a line in `every_axiom_at`, and nothing more.
fn axiom_with_terms(axiom: &Axiom, terms: Vec<Term>) -> Axiom {
    let mut next = terms.into_iter();
    let mut take = || next.next().expect("one term per place");
    match axiom {
        Axiom::CmpReflect(_, flag) => Axiom::CmpReflect(take(), *flag),
        Axiom::CmpReify(_, flag) => Axiom::CmpReify(take(), *flag),
        Axiom::IntAddAssoc(..) => Axiom::IntAddAssoc(take(), take(), take()),
        Axiom::IntAddComm(..) => Axiom::IntAddComm(take(), take()),
        Axiom::IntAddZero(_) => Axiom::IntAddZero(take()),
        Axiom::IntAddNeg(_) => Axiom::IntAddNeg(take()),
        Axiom::IntSubDef(..) => Axiom::IntSubDef(take(), take()),
        Axiom::IntMulAssoc(..) => Axiom::IntMulAssoc(take(), take(), take()),
        Axiom::IntMulComm(..) => Axiom::IntMulComm(take(), take()),
        Axiom::IntMulOne(_) => Axiom::IntMulOne(take()),
        Axiom::IntMulAdd(..) => Axiom::IntMulAdd(take(), take(), take()),
        Axiom::IntLeRefl(_) => Axiom::IntLeRefl(take()),
        Axiom::IntLeTrans(..) => Axiom::IntLeTrans(take(), take(), take()),
        Axiom::IntLeAntisymm(..) => Axiom::IntLeAntisymm(take(), take()),
        Axiom::IntLeAdd(..) => Axiom::IntLeAdd(take(), take(), take()),
        Axiom::IntLeMul(..) => Axiom::IntLeMul(take(), take()),
        Axiom::IntLeTotal(..) => Axiom::IntLeTotal(take(), take()),
        Axiom::IntLtIrrefl(_) => Axiom::IntLtIrrefl(take()),
        Axiom::IntDivRem(..) => Axiom::IntDivRem(take(), take()),
        Axiom::IntDivZero(_) => Axiom::IntDivZero(take()),
        Axiom::IntRemLowerPos(..) => Axiom::IntRemLowerPos(take(), take()),
        Axiom::IntRemUpperPos(..) => Axiom::IntRemUpperPos(take(), take()),
        Axiom::IntRemLowerNeg(..) => Axiom::IntRemLowerNeg(take(), take()),
        Axiom::IntRemUpperNeg(..) => Axiom::IntRemUpperNeg(take(), take()),
        Axiom::IntRemNonneg(..) => Axiom::IntRemNonneg(take(), take()),
        Axiom::IntRemNonpos(..) => Axiom::IntRemNonpos(take(), take()),
        Axiom::ViewLower(ty, _) => Axiom::ViewLower(*ty, take()),
        Axiom::ViewUpper(ty, _) => Axiom::ViewUpper(*ty, take()),
        Axiom::WrapView(ty, _) => Axiom::WrapView(*ty, take()),
        Axiom::ViewWrap(ty, _) => Axiom::ViewWrap(*ty, take()),
        Axiom::WrapPeriod(ty, _) => Axiom::WrapPeriod(*ty, take()),
        Axiom::CastDef(from, to, _) => Axiom::CastDef(*from, *to, take()),
        Axiom::OpModel(op, ty, xs) => Axiom::OpModel(*op, *ty, xs.iter().map(|_| take()).collect()),
        Axiom::OpExact(op, ty, xs) => Axiom::OpExact(*op, *ty, xs.iter().map(|_| take()).collect()),
    }
}

/// The machine types an axiom is instantiated at, in the order the axiom
/// carries them; empty for the axioms that carry none. The inverse is
/// `axiom_with_machine_types`, and a new axiom with a type parameter needs
/// an arm in each.
fn machine_types(axiom: &Axiom) -> Vec<MachineInt> {
    match axiom {
        Axiom::CmpReflect(..)
        | Axiom::CmpReify(..)
        | Axiom::IntAddAssoc(..)
        | Axiom::IntAddComm(..)
        | Axiom::IntAddZero(_)
        | Axiom::IntAddNeg(_)
        | Axiom::IntSubDef(..)
        | Axiom::IntMulAssoc(..)
        | Axiom::IntMulComm(..)
        | Axiom::IntMulOne(_)
        | Axiom::IntMulAdd(..)
        | Axiom::IntLeRefl(_)
        | Axiom::IntLeTrans(..)
        | Axiom::IntLeAntisymm(..)
        | Axiom::IntLeAdd(..)
        | Axiom::IntLeMul(..)
        | Axiom::IntLeTotal(..)
        | Axiom::IntLtIrrefl(_)
        | Axiom::IntDivRem(..)
        | Axiom::IntDivZero(_)
        | Axiom::IntRemLowerPos(..)
        | Axiom::IntRemUpperPos(..)
        | Axiom::IntRemLowerNeg(..)
        | Axiom::IntRemUpperNeg(..)
        | Axiom::IntRemNonneg(..)
        | Axiom::IntRemNonpos(..) => Vec::new(),
        Axiom::ViewLower(ty, _)
        | Axiom::ViewUpper(ty, _)
        | Axiom::WrapView(ty, _)
        | Axiom::ViewWrap(ty, _)
        | Axiom::WrapPeriod(ty, _) => vec![*ty],
        Axiom::CastDef(from, to, _) => vec![*from, *to],
        Axiom::OpModel(_, ty, _) | Axiom::OpExact(_, ty, _) => vec![*ty],
    }
}

/// The same axiom at the same terms, instantiated at other machine types:
/// the inverse of `machine_types`. An axiom that carries no type is
/// returned as it is.
fn axiom_with_machine_types(axiom: &Axiom, types: &[MachineInt]) -> Axiom {
    match axiom {
        Axiom::ViewLower(_, x) => Axiom::ViewLower(types[0], x.clone()),
        Axiom::ViewUpper(_, x) => Axiom::ViewUpper(types[0], x.clone()),
        Axiom::WrapView(_, x) => Axiom::WrapView(types[0], x.clone()),
        Axiom::ViewWrap(_, n) => Axiom::ViewWrap(types[0], n.clone()),
        Axiom::WrapPeriod(_, n) => Axiom::WrapPeriod(types[0], n.clone()),
        Axiom::CastDef(_, _, x) => Axiom::CastDef(types[0], types[1], x.clone()),
        Axiom::OpModel(op, _, xs) => Axiom::OpModel(*op, types[0], xs.clone()),
        Axiom::OpExact(op, _, xs) => Axiom::OpExact(*op, types[0], xs.clone()),
        Axiom::CmpReflect(..)
        | Axiom::CmpReify(..)
        | Axiom::IntAddAssoc(..)
        | Axiom::IntAddComm(..)
        | Axiom::IntAddZero(_)
        | Axiom::IntAddNeg(_)
        | Axiom::IntSubDef(..)
        | Axiom::IntMulAssoc(..)
        | Axiom::IntMulComm(..)
        | Axiom::IntMulOne(_)
        | Axiom::IntMulAdd(..)
        | Axiom::IntLeRefl(_)
        | Axiom::IntLeTrans(..)
        | Axiom::IntLeAntisymm(..)
        | Axiom::IntLeAdd(..)
        | Axiom::IntLeMul(..)
        | Axiom::IntLeTotal(..)
        | Axiom::IntLtIrrefl(_)
        | Axiom::IntDivRem(..)
        | Axiom::IntDivZero(_)
        | Axiom::IntRemLowerPos(..)
        | Axiom::IntRemUpperPos(..)
        | Axiom::IntRemLowerNeg(..)
        | Axiom::IntRemUpperNeg(..)
        | Axiom::IntRemNonneg(..)
        | Axiom::IntRemNonpos(..) => axiom.clone(),
    }
}

/// Every axiom, instantiated at copies of one term: the stock a sibling
/// axiom is chosen from. One line per axiom, matching `axiom_with_terms`.
/// The machine schemas stand at one type each; a sibling drawn from here
/// takes the types of the axiom it replaces when that one carries any.
fn every_axiom_at(t: &Term) -> Vec<Axiom> {
    let t = || t.clone();
    vec![
        Axiom::CmpReflect(t(), true),
        Axiom::CmpReflect(t(), false),
        Axiom::CmpReify(t(), true),
        Axiom::CmpReify(t(), false),
        Axiom::IntAddAssoc(t(), t(), t()),
        Axiom::IntAddComm(t(), t()),
        Axiom::IntAddZero(t()),
        Axiom::IntAddNeg(t()),
        Axiom::IntSubDef(t(), t()),
        Axiom::IntMulAssoc(t(), t(), t()),
        Axiom::IntMulComm(t(), t()),
        Axiom::IntMulOne(t()),
        Axiom::IntMulAdd(t(), t(), t()),
        Axiom::IntLeRefl(t()),
        Axiom::IntLeTrans(t(), t(), t()),
        Axiom::IntLeAntisymm(t(), t()),
        Axiom::IntLeAdd(t(), t(), t()),
        Axiom::IntLeMul(t(), t()),
        Axiom::IntLeTotal(t(), t()),
        Axiom::IntLtIrrefl(t()),
        Axiom::IntDivRem(t(), t()),
        Axiom::IntDivZero(t()),
        Axiom::IntRemLowerPos(t(), t()),
        Axiom::IntRemUpperPos(t(), t()),
        Axiom::IntRemLowerNeg(t(), t()),
        Axiom::IntRemUpperNeg(t(), t()),
        Axiom::IntRemNonneg(t(), t()),
        Axiom::IntRemNonpos(t(), t()),
        Axiom::ViewLower(U16, t()),
        Axiom::ViewUpper(I8, t()),
        Axiom::WrapView(U64, t()),
        Axiom::ViewWrap(I32, t()),
        Axiom::WrapPeriod(U32, t()),
        Axiom::CastDef(I16, U16, t()),
        Axiom::OpModel(Op::Add, U16, vec![t(), t()]),
        Axiom::OpModel(Op::Div, I8, vec![t(), t()]),
        Axiom::OpModel(Op::Neg, I64, vec![t()]),
        Axiom::OpModel(Op::WrappingMul, U32, vec![t(), t()]),
        Axiom::OpExact(Op::Sub, U8, vec![t(), t()]),
        Axiom::OpExact(Op::Mul, I32, vec![t(), t()]),
        Axiom::OpExact(Op::Neg, I16, vec![t()]),
    ]
}

/// The same axiom about the table, at a sibling operation: the inverse of
/// nothing, since the operation is not a term. An axiom that carries no
/// operation is returned as it is.
fn axiom_with_op_sibling(axiom: &Axiom) -> Axiom {
    match axiom {
        Axiom::OpModel(op, ty, xs) => Axiom::OpModel(op_sibling(*op), *ty, xs.clone()),
        Axiom::OpExact(op, ty, xs) => Axiom::OpExact(op_sibling(*op), *ty, xs.clone()),
        other => other.clone(),
    }
}

fn map_axiom(axiom: &Axiom, f: &dyn Fn(&Term) -> Term) -> Axiom {
    axiom_with_terms(axiom, axiom.terms().into_iter().map(f).collect())
}

/// The node with `f` applied to every term it holds itself. Its subproofs
/// are left alone, wherever they sit.
fn map_node_terms(node: &Proof, f: &dyn Fn(&Term) -> Term) -> Proof {
    let spare_proofs = |terms: &[Term]| -> Vec<Term> {
        terms
            .iter()
            .map(|term| match term {
                Term::Proof(_) => term.clone(),
                other => f(other),
            })
            .collect()
    };
    match node {
        Proof::Hyp(_) | Proof::Omitted | Proof::ImpliesElim(..) | Proof::ForallIntro { .. } => {
            node.clone()
        }
        Proof::OfTerm(Term::Call(callee, arguments)) => {
            Proof::OfTerm(Term::call((**callee).clone(), spare_proofs(arguments)))
        }
        Proof::OfTerm(term) => Proof::OfTerm(f(term)),
        Proof::Refl(term) => Proof::Refl(f(term)),
        Proof::Projection(term) => Proof::Projection(f(term)),
        Proof::Literal(term) => Proof::Literal(f(term)),
        Proof::Definition(term) => Proof::Definition(f(term)),
        Proof::CaseStep(term) => Proof::CaseStep(f(term)),
        Proof::CaseKnown { term, equation } => Proof::CaseKnown {
            term: f(term),
            equation: equation.clone(),
        },
        Proof::ExcludedMiddle(term) => Proof::ExcludedMiddle(f(term)),
        Proof::ForEmpty(term) => Proof::ForEmpty(f(term)),
        Proof::BufferStep(term) => Proof::BufferStep(f(term)),
        Proof::BufferBound { value, upper } => Proof::BufferBound {
            value: f(value),
            upper: *upper,
        },
        Proof::Evaluate(term) => Proof::Evaluate(f(term)),
        Proof::Axiom(axiom) => Proof::Axiom(map_axiom(axiom, f)),
        Proof::Transport {
            eq,
            template,
            proof,
        } => Proof::Transport {
            eq: eq.clone(),
            template: f(template),
            proof: proof.clone(),
        },
        Proof::ImpliesIntro { hyp, body } => Proof::ImpliesIntro {
            hyp: f(hyp),
            body: body.clone(),
        },
        Proof::ForallElim(universal, argument) => Proof::ForallElim(universal.clone(), f(argument)),
        Proof::Construct {
            prop,
            variant,
            params,
            payload,
        } => Proof::Construct {
            prop: *prop,
            variant: *variant,
            params: spare_proofs(params),
            payload: spare_proofs(payload),
        },
        Proof::CaseProof {
            scrutinee,
            goal,
            arms,
        } => Proof::CaseProof {
            scrutinee: scrutinee.clone(),
            goal: f(goal),
            arms: arms.clone(),
        },
        Proof::CaseData {
            scrutinee,
            goal,
            arms,
        } => Proof::CaseData {
            scrutinee: f(scrutinee),
            goal: f(goal),
            arms: arms.clone(),
        },
        Proof::ExistsIntro {
            prop,
            witness,
            proof,
        } => Proof::ExistsIntro {
            prop: f(prop),
            witness: f(witness),
            proof: proof.clone(),
        },
        Proof::ExistsElim { exists, goal, arm } => Proof::ExistsElim {
            exists: exists.clone(),
            goal: f(goal),
            arm: arm.clone(),
        },
        Proof::ForStep {
            looped,
            lower,
            upper,
        } => Proof::ForStep {
            looped: f(looped),
            lower: lower.clone(),
            upper: upper.clone(),
        },
        Proof::PropInduction {
            scrutinee,
            motive,
            arms,
        } => Proof::PropInduction {
            scrutinee: scrutinee.clone(),
            motive: TermArm {
                binders: motive.binders,
                body: f(&motive.body),
            },
            arms: arms.clone(),
        },
        Proof::DataInduction {
            target,
            motives,
            arms,
        } => Proof::DataInduction {
            target: f(target),
            motives: motives.iter().map(|(id, m)| (*id, f(m))).collect(),
            arms: arms.clone(),
        },
        Proof::IntInduction {
            motive,
            base,
            step,
            target,
        } => Proof::IntInduction {
            motive: f(motive),
            base: base.clone(),
            step: step.clone(),
            target: f(target),
        },
        Proof::Linear {
            goal,
            goal_coefficient,
            pairs,
        } => Proof::Linear {
            goal: f(goal),
            goal_coefficient: goal_coefficient.clone(),
            pairs: pairs.clone(),
        },
    }
}

/// `f` applied to every term of every node of the proof.
fn map_proof_terms(proof: &Proof, f: &dyn Fn(&Term) -> Term) -> Proof {
    let node = map_node_terms(proof, f);
    let children = proof_children(&node)
        .into_iter()
        .map(|(child, _, _)| map_proof_terms(child, f))
        .collect();
    proof_with_children(&node, children)
}

/// Every occurrence of `old` in the term replaced by `new`.
fn replace_in(term: &Term, old: &Term, new: &Term) -> Term {
    if term == old {
        return new.clone();
    }
    let children = term_children(term)
        .into_iter()
        .map(|child| replace_in(child, old, new))
        .collect();
    term_with_children(term, children)
}

/// Where two claims part: the smallest subterm of the first that must be
/// replaced to get the second.
fn difference<'t>(from: &'t Term, to: &'t Term) -> (&'t Term, &'t Term) {
    let (left, right) = (term_children(from), term_children(to));
    if left.len() == right.len() {
        let differing: Vec<usize> = (0..left.len())
            .filter(|index| left[*index] != right[*index])
            .collect();
        if let [index] = differing.as_slice() {
            let same_shape =
                term_with_children(from, right.iter().map(|term| (*term).clone()).collect()) == *to;
            if same_shape {
                return difference(left[*index], right[*index]);
            }
        }
    }
    (from, to)
}

fn proof_size(proof: &Proof) -> usize {
    1 + proof_children(proof)
        .iter()
        .map(|(child, _, _)| proof_size(child))
        .sum::<usize>()
}

fn collect_subproofs(proof: &Proof, out: &mut Vec<Proof>) {
    out.push(proof.clone());
    for (child, _, _) in proof_children(proof) {
        collect_subproofs(child, out);
    }
}

/// Replaces node `target` of the proof, counting in preorder. `change`
/// receives the node and the binders it is under.
fn replace_node(
    proof: &Proof,
    target: &mut usize,
    binders: (u32, u32),
    change: &mut dyn FnMut(&Proof, (u32, u32)) -> Proof,
) -> Proof {
    if *target == 0 {
        *target = usize::MAX;
        return change(proof, binders);
    }
    *target -= 1;
    let children = proof_children(proof)
        .into_iter()
        .map(|(child, vars, hyps)| {
            replace_node(child, target, (binders.0 + vars, binders.1 + hyps), change)
        })
        .collect();
    proof_with_children(proof, children)
}

// --- Mutation -----------------------------------------------------------------------

/// What a mutation may draw on.
struct Material<'a> {
    scene: &'a Scene,
    hyps: Vec<HypId>,
    /// Context variables that are evidence.
    evidence: Vec<VarId>,
    /// Subproofs of this proof and of the other proofs of the same origin.
    proofs: &'a [Proof],
    /// Terms to put where a term is wanted.
    terms: Vec<Term>,
    /// The false claim under attack.
    target: &'a Term,
    /// The part of the true claim that was changed, and what it became.
    change: (Term, Term),
}

impl<'a> Material<'a> {
    fn new(triple: &'a Triple, proofs: &'a [Proof], target: &'a Term) -> Self {
        let scene = &triple.scene;
        let vars = scene.vars();
        let mut terms = vec![
            Term::U8(0),
            Term::U8(1),
            Term::U8(255),
            Term::Bool(true),
            Term::int(0),
            Term::int(1),
            Term::int(-1),
            lit(U16, 0),
            lit(I8, -1),
            lit(I32, i128::from(i32::MAX)),
            lit(U64, i128::from(u64::MAX)),
        ];
        for (id, ty) in &vars {
            if !matches!(ty, Type::Proof(_)) {
                terms.push(Term::Free(*id));
            }
        }
        for claim in [&triple.claim, target] {
            terms.extend(term_children(claim).into_iter().cloned());
            for child in term_children(claim) {
                terms.extend(term_children(child).into_iter().cloned());
            }
        }
        Self {
            scene,
            hyps: scene.hyps(),
            evidence: vars
                .iter()
                .filter(|(_, ty)| matches!(ty, Type::Proof(_)))
                .map(|(id, _)| *id)
                .collect(),
            proofs,
            terms,
            target,
            change: {
                let (old, new) = difference(&triple.claim, target);
                (old.clone(), new.clone())
            },
        }
    }

    fn term(&self, rng: &mut Rng) -> Term {
        rng.pick(&self.terms).cloned().unwrap_or(Term::U8(0))
    }

    /// A perturbed copy of a term that sits inside a proof node, or the
    /// false claim itself.
    fn bend(&self, term: &Term, rng: &mut Rng) -> Term {
        if rng.below(4) == 0 {
            return self.target.clone();
        }
        let options = perturbations(term, self.scene);
        rng.pick(&options)
            .cloned()
            .unwrap_or_else(|| self.term(rng))
    }

    fn some_hyp(&self, hyp_binders: u32, rng: &mut Rng) -> Proof {
        let bound = hyp_binders as usize;
        let choices = self.hyps.len() + self.evidence.len() + bound;
        if choices == 0 {
            return Proof::Refl(self.term(rng));
        }
        let choice = rng.below(choices);
        if choice < self.hyps.len() {
            Proof::hyp(self.hyps[choice])
        } else if choice < self.hyps.len() + self.evidence.len() {
            Proof::OfTerm(Term::Free(self.evidence[choice - self.hyps.len()]))
        } else {
            Proof::Hyp(HypRef::Bound(
                (choice - self.hyps.len() - self.evidence.len()) as u32,
            ))
        }
    }

    /// A computation rule aimed at the false claim.
    fn computed(&self, rng: &mut Rng) -> Proof {
        let side = match self.target {
            // Evaluation decides a comparison of integers as a whole, and an
            // equation of integers or of machine integers.
            Term::Prim(Prim::IntLe, _) => return Proof::Evaluate(self.target.clone()),
            Term::Eq(Type::Int | Type::Machine(_), _, _) if rng.below(2) == 0 => {
                return Proof::Evaluate(self.target.clone());
            }
            Term::Eq(_, left, right) => {
                if rng.below(2) == 0 {
                    (**left).clone()
                } else {
                    (**right).clone()
                }
            }
            _ => self.term(rng),
        };
        match rng.below(5) {
            0 => Proof::Evaluate(side),
            1 => Proof::Refl(side),
            2 => Proof::Literal(side),
            3 => Proof::Definition(side),
            _ => Proof::ExcludedMiddle(self.target.clone()),
        }
    }

    /// The axiom about other terms, or with two of its terms exchanged, or
    /// a sibling axiom of the same arity about the same terms. `Reflect`
    /// also has a flag to flip, and a machine schema a type to move to a
    /// neighbouring one. Nothing here names an axiom: the stock of siblings
    /// is `every_axiom_at`.
    fn change_axiom(&self, axiom: &Axiom, rng: &mut Rng) -> Axiom {
        let mut terms: Vec<Term> = axiom.terms().into_iter().cloned().collect();
        let arity = terms.len();
        let own_types = machine_types(axiom);
        let has_extra =
            matches!(axiom, Axiom::CmpReflect(..) | Axiom::CmpReify(..)) || !own_types.is_empty();
        let choice = rng.below(if has_extra { 4 } else { 3 });
        match choice {
            // One term perturbed, or replaced by any other.
            0 => {
                let place = rng.below(arity);
                terms[place] = if rng.below(2) == 0 {
                    self.bend(&terms[place], rng)
                } else {
                    self.term(rng)
                };
                axiom_with_terms(axiom, terms)
            }
            // Two terms exchanged; of one term, the term replaced.
            1 => {
                if arity > 1 {
                    let first = rng.below(arity);
                    let second = (first + 1 + rng.below(arity - 1)) % arity;
                    terms.swap(first, second);
                } else {
                    terms[0] = self.term(rng);
                }
                axiom_with_terms(axiom, terms)
            }
            // A sibling: another axiom of the same arity at the same terms,
            // and at the same machine types when both carry any.
            2 => {
                let siblings: Vec<Axiom> = every_axiom_at(&Term::int(0))
                    .into_iter()
                    .filter(|other| other.terms().len() == arity && other.name() != axiom.name())
                    .collect();
                match rng.pick(&siblings) {
                    Some(sibling) => {
                        let wanted = machine_types(sibling).len();
                        let sibling = if own_types.is_empty() || wanted == 0 {
                            sibling.clone()
                        } else {
                            let types: Vec<MachineInt> = (0..wanted)
                                .map(|place| own_types[place % own_types.len()])
                                .collect();
                            axiom_with_machine_types(sibling, &types)
                        };
                        axiom_with_terms(&sibling, terms)
                    }
                    None => axiom_with_terms(axiom, terms),
                }
            }
            _ => match axiom {
                Axiom::CmpReflect(comparison, flag) => Axiom::CmpReflect(comparison.clone(), !flag),
                Axiom::CmpReify(comparison, flag) => Axiom::CmpReify(comparison.clone(), !flag),
                // An axiom about the table, half the time at the sibling
                // operation instead: a row that does not exist, a row with
                // no exact statement, or an axiom about another operation,
                // which the oracle can judge.
                Axiom::OpModel(..) | Axiom::OpExact(..) if rng.below(2) == 0 => {
                    axiom_with_op_sibling(axiom)
                }
                // One type parameter moved to a neighbouring type: the
                // instance is then ill typed, or an axiom about another
                // type, which the oracle can judge.
                _ => {
                    let mut types = own_types;
                    let place = rng.below(types.len());
                    types[place] = *rng
                        .pick(&neighbours(types[place]))
                        .expect("every type has a neighbour");
                    axiom_with_machine_types(axiom, &types)
                }
            },
        }
    }

    /// Changes something the node itself holds: a term, an index, a flag, or
    /// the order of its subproofs. `None` when the node holds nothing.
    fn change_in_place(&self, node: &Proof, rng: &mut Rng) -> Option<Proof> {
        Some(match node {
            Proof::BufferStep(term) => Proof::BufferStep(self.bend(term, rng)),
            Proof::BufferBound { value, upper } => Proof::BufferBound {
                value: self.bend(value, rng),
                upper: *upper,
            },
            Proof::Hyp(_) | Proof::Omitted => return None,
            Proof::OfTerm(Term::Call(callee, arguments)) => {
                let mut arguments = arguments.clone();
                if arguments.is_empty() {
                    return None;
                }
                let position = rng.below(arguments.len());
                if matches!(arguments[position], Term::Proof(_)) {
                    let other = rng.below(arguments.len());
                    arguments.swap(position, other);
                } else {
                    arguments[position] = self.term(rng);
                }
                Proof::OfTerm(Term::call((**callee).clone(), arguments))
            }
            Proof::OfTerm(term) => Proof::OfTerm(self.bend(term, rng)),
            Proof::Refl(term) => Proof::Refl(self.bend(term, rng)),
            Proof::Projection(term) => Proof::Projection(self.bend(term, rng)),
            Proof::Literal(term) => Proof::Literal(self.bend(term, rng)),
            Proof::Definition(term) => Proof::Definition(self.bend(term, rng)),
            Proof::CaseStep(term) => Proof::CaseStep(self.bend(term, rng)),
            Proof::CaseKnown { term, equation } => Proof::CaseKnown {
                term: self.bend(term, rng),
                equation: equation.clone(),
            },
            Proof::ExcludedMiddle(term) => Proof::ExcludedMiddle(self.bend(term, rng)),
            Proof::ForEmpty(term) => Proof::ForEmpty(self.bend(term, rng)),
            Proof::Evaluate(term) => Proof::Evaluate(self.bend(term, rng)),
            Proof::Axiom(axiom) => Proof::Axiom(self.change_axiom(axiom, rng)),
            Proof::Transport {
                eq,
                template,
                proof,
            } => {
                if rng.below(2) == 0 {
                    Proof::Transport {
                        eq: proof.clone(),
                        template: template.clone(),
                        proof: eq.clone(),
                    }
                } else {
                    Proof::Transport {
                        eq: eq.clone(),
                        template: self.bend(template, rng),
                        proof: proof.clone(),
                    }
                }
            }
            Proof::ImpliesIntro { hyp, body } => Proof::ImpliesIntro {
                hyp: self.bend(hyp, rng),
                body: body.clone(),
            },
            Proof::ImpliesElim(implication, premise) => {
                Proof::ImpliesElim(premise.clone(), implication.clone())
            }
            Proof::ForallIntro { ty, body } => Proof::ForallIntro {
                ty: match ty {
                    Type::U8 => Type::Bool,
                    Type::Int => Type::Bool,
                    Type::Machine(machine) => Type::machine(neighbours(*machine)[0]),
                    _ => Type::Int,
                },
                body: body.clone(),
            },
            Proof::ForallElim(universal, _) => Proof::ForallElim(universal.clone(), self.term(rng)),
            Proof::Construct {
                prop,
                variant,
                params,
                payload,
            } => {
                let (mut variant, mut params, mut payload) =
                    (*variant, params.clone(), payload.clone());
                match rng.below(3) {
                    // The sibling rule: `Or::Left` for `Or::Right`.
                    0 => variant = if variant == 0 { 1 } else { variant - 1 },
                    1 if params.len() > 1 => params.reverse(),
                    1 if !params.is_empty() => params[0] = self.target.clone(),
                    _ if payload.len() > 1 => payload.reverse(),
                    _ => variant += 1,
                }
                Proof::Construct {
                    prop: *prop,
                    variant,
                    params,
                    payload,
                }
            }
            Proof::CaseProof {
                scrutinee,
                goal,
                arms,
            } => {
                let mut arms = arms.clone();
                let goal = if arms.len() > 1 && rng.below(2) == 0 {
                    arms.reverse();
                    goal.clone()
                } else {
                    self.bend(goal, rng)
                };
                Proof::CaseProof {
                    scrutinee: scrutinee.clone(),
                    goal,
                    arms,
                }
            }
            Proof::CaseData {
                scrutinee,
                goal,
                arms,
            } => {
                let mut arms = arms.clone();
                let (scrutinee, goal) = match rng.below(3) {
                    0 if arms.len() > 1 => {
                        arms.reverse();
                        (scrutinee.clone(), goal.clone())
                    }
                    1 => (self.bend(scrutinee, rng), goal.clone()),
                    _ => (scrutinee.clone(), self.bend(goal, rng)),
                };
                Proof::CaseData {
                    scrutinee,
                    goal,
                    arms,
                }
            }
            Proof::ExistsIntro {
                prop,
                witness,
                proof,
            } => {
                let (prop, witness) = if rng.below(2) == 0 {
                    (self.bend(prop, rng), witness.clone())
                } else {
                    (prop.clone(), self.term(rng))
                };
                Proof::ExistsIntro {
                    prop,
                    witness,
                    proof: proof.clone(),
                }
            }
            Proof::ExistsElim { exists, goal, arm } => Proof::ExistsElim {
                exists: exists.clone(),
                goal: self.bend(goal, rng),
                arm: arm.clone(),
            },
            Proof::ForStep {
                looped,
                lower,
                upper,
            } => Proof::ForStep {
                looped: looped.clone(),
                lower: upper.clone(),
                upper: lower.clone(),
            },
            Proof::PropInduction {
                scrutinee,
                motive,
                arms,
            } => {
                let mut motive = motive.clone();
                if rng.below(2) == 0 {
                    motive.binders += 1;
                } else {
                    motive.body = self.bend(&motive.body, rng);
                }
                Proof::PropInduction {
                    scrutinee: scrutinee.clone(),
                    motive,
                    arms: arms.clone(),
                }
            }
            Proof::DataInduction {
                target,
                motives,
                arms,
            } => {
                let mut arms = arms.clone();
                let mut motives = motives.clone();
                if rng.below(2) == 0 {
                    arms.reverse();
                } else if let Some((_, motive)) = motives.first_mut() {
                    *motive = self.bend(motive, rng);
                }
                Proof::DataInduction {
                    target: target.clone(),
                    motives,
                    arms,
                }
            }
            Proof::IntInduction {
                motive,
                base,
                step,
                target,
            } => {
                let (motive, target) = if rng.below(2) == 0 {
                    (self.bend(motive, rng), target.clone())
                } else {
                    (motive.clone(), self.term(rng))
                };
                Proof::IntInduction {
                    motive,
                    base: base.clone(),
                    step: step.clone(),
                    target,
                }
            }
            Proof::Linear {
                goal,
                goal_coefficient,
                pairs,
            } => {
                // One coefficient perturbed, a pair dropped, two pairs
                // swapped, or the goal changed.
                let (mut goal, mut goal_coefficient, mut pairs) =
                    (goal.clone(), goal_coefficient.clone(), pairs.clone());
                let nudge = |c: &Integer, rng: &mut Rng| match rng.below(3) {
                    0 => c.add(&Integer::from(1i64)),
                    1 => c.sub(&Integer::from(1i64)),
                    _ => c.neg(),
                };
                match rng.below(4) {
                    0 if !pairs.is_empty() => {
                        let at = rng.below(pairs.len());
                        pairs[at].1 = nudge(&pairs[at].1, rng);
                    }
                    0 => goal_coefficient = nudge(&goal_coefficient, rng),
                    1 if !pairs.is_empty() => {
                        pairs.remove(rng.below(pairs.len()));
                    }
                    2 if pairs.len() > 1 => {
                        let (a, b) = (rng.below(pairs.len()), rng.below(pairs.len()));
                        pairs.swap(a, b);
                    }
                    _ => goal = self.bend(&goal, rng),
                }
                Proof::Linear {
                    goal,
                    goal_coefficient,
                    pairs,
                }
            }
        })
    }

    /// One node of the proof, changed. One mutant in eight is of another
    /// kind: the proof with the claim's own change made wherever the changed
    /// part occurs, which is the proof the false claim would have if it had
    /// one.
    fn mutant(&self, proof: &Proof, rng: &mut Rng) -> Proof {
        let (old, new) = &self.change;
        let push = |term: &Term| replace_in(term, old, new);
        if rng.below(8) == 0 {
            let pushed = map_proof_terms(proof, &push);
            // Unchanged when the proof never mentions the changed part.
            if pushed != *proof {
                return pushed;
            }
        }
        let mut target = rng.below(proof_size(proof));
        replace_node(proof, &mut target, (0, 0), &mut |node, binders| {
            match rng.below(9) {
                // The claim's change, made in this node alone.
                4 if map_node_terms(node, &push) != *node => map_node_terms(node, &push),
                // Another hypothesis.
                0 => self.some_hyp(binders.1, rng),
                // A different proof from the same place.
                1 => rng
                    .pick(self.proofs)
                    .cloned()
                    .unwrap_or_else(|| self.some_hyp(binders.1, rng)),
                // A computation rule where a proof was.
                2 => self.computed(rng),
                // The node applied to something, or something applied to it.
                3 => match rng.below(3) {
                    0 => Proof::implies_elim(node.clone(), self.some_hyp(binders.1, rng)),
                    1 => Proof::implies_elim(self.some_hyp(binders.1, rng), node.clone()),
                    _ => Proof::forall_elim(node.clone(), self.term(rng)),
                },
                // Anything the node itself holds.
                _ => self
                    .change_in_place(node, rng)
                    .unwrap_or_else(|| self.some_hyp(binders.1, rng)),
            }
        })
    }
}

// --- The run -------------------------------------------------------------------------

#[derive(Default)]
struct Tally {
    triples: usize,
    /// Triples with at least one claim known to be false.
    attacked: usize,
    /// Triples whose scene has no witness the oracle can find.
    no_witness: usize,
    /// Candidate claims the oracle found false, and those it could not
    /// decide or found true.
    decided_false: usize,
    skipped: usize,
    pairs: usize,
    mutants: usize,
    /// Mutants that are proofs of something, so that the claim is what
    /// stands between them and acceptance.
    mutants_proving_something: usize,
    mutants_accepted_for_original: usize,
    findings: Vec<String>,
}

impl Tally {
    fn report(&self, label: &str, started: Instant) {
        println!(
            "{label}: {} triples ({} attacked, {} in a context with no witness the oracle can \
             find); candidate claims: {} decided false, {} skipped; {} pairings of a proof with \
             a false claim; {} mutants, of which {} prove something and {} were accepted for \
             the original claim; {:.2}s",
            self.triples,
            self.attacked,
            self.no_witness,
            self.decided_false,
            self.skipped,
            self.pairs,
            self.mutants,
            self.mutants_proving_something,
            self.mutants_accepted_for_original,
            started.elapsed().as_secs_f64(),
        );
    }
}

fn finding(
    triple: &Triple,
    claim: &Term,
    witness: &Assignment,
    proof: &Proof,
    what: &str,
) -> String {
    format!(
        "{what}\n  origin: {}\n  seed: {SEED:#x}\n  context:\n{}  original claim: {}\n  \
         false claim: {claim}\n  false at: {}\n  proof: {proof:?}\n",
        triple.origin,
        triple.scene.describe(),
        triple.claim,
        describe_witness(witness),
    )
}

/// The triples whose proofs may be spliced into one another: those of one
/// source file, or of one kind.
fn group(origin: &str) -> &str {
    let file = origin.split_once('#').or_else(|| origin.split_once("::"));
    match file.or_else(|| origin.split_once('/')) {
        Some((group, _)) => group,
        None => origin,
    }
}

/// Attacks every triple.
fn attack(triples: &[Triple], tally: &mut Tally, rng: &mut Rng) {
    let count = mutants_per_pair();
    let mut pools: HashMap<&str, Vec<Proof>> = HashMap::new();
    for triple in triples {
        collect_subproofs(
            &triple.proof,
            pools.entry(group(&triple.origin)).or_default(),
        );
    }

    for triple in triples {
        tally.triples += 1;
        let mut ctx = triple.scene.ctx.clone();
        assert_eq!(
            check_proof(&mut ctx, &triple.proof, &triple.claim),
            Ok(()),
            "{}: the kernel rejects the triple's own proof of {}",
            triple.origin,
            triple.claim
        );

        let found = witnesses(&triple.scene, &triple.claim, rng);
        if found.is_empty() {
            tally.no_witness += 1;
            continue;
        }
        // The pairing the kernel has already accepted is itself under test.
        if let Some(witness) = refute(&triple.scene, &triple.claim, &found) {
            tally.findings.push(finding(
                triple,
                &triple.claim,
                witness,
                &triple.proof,
                "the oracle refutes a claim the kernel accepted",
            ));
            continue;
        }

        let mut candidates = false_candidates(triple);
        rng.shuffle(&mut candidates[3..]);
        let mut falsehoods: Vec<(Term, &Assignment)> = Vec::new();
        for candidate in candidates {
            match refute(&triple.scene, &candidate, &found) {
                Some(witness) => falsehoods.push((candidate, witness)),
                None => tally.skipped += 1,
            }
        }
        tally.decided_false += falsehoods.len();
        if !falsehoods.is_empty() {
            tally.attacked += 1;
        }
        // Every false claim meets the original proof; the first few also
        // meet its mutants.
        for (claim, witness) in falsehoods.iter().skip(FALSE_CLAIMS) {
            tally.pairs += 1;
            if check_proof(&mut ctx, &triple.proof, claim).is_ok() {
                tally.findings.push(finding(
                    triple,
                    claim,
                    witness,
                    &triple.proof,
                    "the original proof is accepted for a false claim",
                ));
            }
        }
        falsehoods.truncate(FALSE_CLAIMS);

        let pool = &pools[group(&triple.origin)];
        for (claim, witness) in &falsehoods {
            tally.pairs += 1;
            if check_proof(&mut ctx, &triple.proof, claim).is_ok() {
                tally.findings.push(finding(
                    triple,
                    claim,
                    witness,
                    &triple.proof,
                    "the original proof is accepted for a false claim",
                ));
            }
            let material = Material::new(triple, pool, claim);
            for _ in 0..count {
                let mutant = material.mutant(&triple.proof, rng);
                tally.mutants += 1;
                if check_proof(&mut ctx, &mutant, claim).is_ok() {
                    tally.findings.push(finding(
                        triple,
                        claim,
                        witness,
                        &mutant,
                        "a mutant is accepted for a false claim",
                    ));
                }
                // Whatever the mutant proves, it must not be false either.
                if let Ok(proved) = infer_proof(&mut ctx, &mutant) {
                    tally.mutants_proving_something += 1;
                    let few = &found[..found.len().min(MUTANT_CLAIM_WITNESSES)];
                    if let Some(witness) =
                        refute_within(&triple.scene, &proved, few, MUTANT_CLAIM_STEPS)
                    {
                        tally.findings.push(finding(
                            triple,
                            &proved,
                            witness,
                            &mutant,
                            "a mutant proves a claim of its own that is false",
                        ));
                    }
                }
                if check_proof(&mut ctx, &mutant, &triple.claim).is_ok() {
                    tally.mutants_accepted_for_original += 1;
                }
            }
        }
    }
}

fn conclude(tally: &Tally) {
    assert!(
        tally.findings.is_empty(),
        "{} finding(s):\n\n{}",
        tally.findings.len(),
        tally.findings.join("\n")
    );
}

// --- Triples written by hand ----------------------------------------------------------

struct World {
    definitions: Rc<Definitions>,
    prelude: Prelude,
    theory: Theory,
    /// `math fn double(n: u8) -> u8 { n.wrapping_add(n) }`
    double: FnId,
    /// `math fn within(n: u8) -> Prop { n <= 3 }`
    within: FnId,
    /// `math fn int_double(n: Int) -> Int { n + n }`
    int_double: FnId,
    witness_prop: locus::kernel::PropId,
    induction_data: locus::kernel::EnumId,
    induction_prop: locus::kernel::PropId,
    generic_refl: FnId,
}

fn world() -> World {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    let double = definitions
        .declare_fn(
            &Type::function(1, |params| match params {
                [] => Type::U8,
                _ => Type::U8,
            }),
            |params| {
                Term::op(
                    Op::WrappingAdd,
                    MachineInt::U8,
                    vec![params[0].clone(), params[0].clone()],
                )
            },
        )
        .expect("double is declared");
    let within = definitions
        .declare_fn(
            &Type::function(1, |params| match params {
                [] => Type::U8,
                _ => Type::Prop,
            }),
            |params| {
                Term::int_le(
                    Term::view(MachineInt::U8, params[0].clone()),
                    Term::view(MachineInt::U8, Term::U8(3)),
                )
            },
        )
        .expect("within is declared");
    let int_double = definitions
        .declare_fn(&Type::function(1, |_| Type::Int), |params| {
            Term::int_add(params[0].clone(), params[0].clone())
        })
        .expect("int_double is declared");
    let witness_prop = definitions
        .declare_prop(
            vec![Type::Int],
            vec![locus::kernel::PropVariant::arm(
                Type::Tuple(vec![Type::Int, Type::Int]),
                |p| Term::int_le(p[1].clone(), p[0].clone()),
            )],
        )
        .unwrap();
    let induction_data = definitions
        .declare_logical_enum_group(1, |ids| {
            vec![vec![
                Type::Tuple(vec![]),
                Type::Tuple(vec![Type::Enum(ids[0])]),
            ]]
        })
        .unwrap()[0];
    let induction_prop = definitions
        .declare_inductive_prop(vec![Type::Int], |id| {
            vec![
                locus::kernel::PropVariant::arm(Type::Tuple(vec![Type::Int]), |p| {
                    int_eq(p[0].clone(), Term::int(0))
                }),
                locus::kernel::PropVariant::arm(Type::Tuple(vec![Type::Int]), |p| {
                    Term::PropApp(id, p.to_vec())
                }),
            ]
        })
        .unwrap();
    let template = definitions
        .declare_generic(vec![locus::kernel::TypeBound::Logical], |t| {
            locus::kernel::GenericDeclaration::function(
                Type::Fn(
                    vec![t[0].clone()],
                    Box::new(Type::proof(Term::eq(
                        t[0].clone(),
                        Term::Bound(0),
                        Term::Bound(0),
                    ))),
                ),
                |p| Term::proof(Proof::Refl(p[0].clone())),
            )
        })
        .unwrap();
    let locus::kernel::GenericInstance::Function(generic_refl) = definitions
        .instantiate_generic(template, &[Type::Int])
        .unwrap()
    else {
        unreachable!()
    };
    World {
        definitions: Rc::new(definitions),
        prelude,
        theory,
        double,
        within,
        int_double,
        witness_prop,
        induction_data,
        induction_prop,
        generic_refl,
    }
}

fn int_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Int, left, right)
}

fn ax(axiom: Axiom) -> Proof {
    Proof::Axiom(axiom)
}

/// `a <= a + 1`: from `0 <= 1`, which evaluation decides, by adding `a` on
/// both sides and tidying each side. Four axioms, eleven nodes.
fn int_le_succ(a: &Term) -> Proof {
    let (add, le, lit) = (Term::int_add, Term::int_le, Term::int);
    let zero_le_one = Proof::Evaluate(le(lit(0), lit(1)));
    // 0 + a <= 1 + a
    let shifted = Proof::implies_elim(ax(Axiom::IntLeAdd(lit(0), lit(1), a.clone())), zero_le_one);
    let left = Chain::new(Type::Int, add(lit(0), a.clone()))
        .step(ax(Axiom::IntAddComm(lit(0), a.clone())))
        .step(ax(Axiom::IntAddZero(a.clone())))
        .finish();
    let right = ax(Axiom::IntAddComm(lit(1), a.clone()));
    let tidy_left = Proof::transport(left, |hole| le(hole, add(lit(1), a.clone())), shifted);
    Proof::transport(right, |hole| le(a.clone(), hole), tidy_left)
}

fn n_id(term: &Term) -> VarId {
    match term {
        Term::Free(id) => *id,
        _ => panic!("a variable"),
    }
}

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn lemma(id: FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

fn and_intro(prelude: &Prelude, p: &Term, q: &Term, left: Proof, right: Proof) -> Proof {
    Proof::Construct {
        prop: prelude.and,
        variant: 0,
        params: vec![p.clone(), q.clone()],
        payload: vec![Term::proof(left), Term::proof(right)],
    }
}

fn or_intro(prelude: &Prelude, p: &Term, q: &Term, side: usize, proof: Proof) -> Proof {
    Proof::Construct {
        prop: prelude.or,
        variant: side,
        params: vec![p.clone(), q.clone()],
        payload: vec![Term::proof(proof)],
    }
}

fn hand_built(world: &World) -> Vec<Triple> {
    let prelude = &world.prelude;
    let mut out = Vec::new();
    let mut add = |name: &str, scene: Scene, claim: Term, proof: Proof| {
        out.push(Triple {
            origin: format!("hand/{name}"),
            scene,
            claim,
            proof,
        });
    };
    let le = |left: &Term, right: u8| {
        Term::int_le(
            Term::view(MachineInt::U8, left.clone()),
            Term::view(MachineInt::U8, Term::U8(right)),
        )
    };

    {
        let scene = Scene::new(&world.definitions);
        let parameter = VarId::fresh();
        let lambda = Term::lambda_over(
            &[(parameter, Type::Int)],
            &Type::Int,
            Term::int_add(Term::Free(parameter), Term::int(1)),
        );
        let call = Term::call(lambda, vec![Term::int(3)]);
        add(
            "lambda_beta",
            scene.clone(),
            int_eq(call.clone(), Term::int_add(Term::int(3), Term::int(1))),
            Proof::Definition(call.clone()),
        );
        add(
            "lambda_evaluation",
            scene,
            int_eq(call.clone(), Term::int(4)),
            Proof::Evaluate(call),
        );
    }

    {
        let scene = Scene::new(&world.definitions);
        let buffer = Term::Buffer {
            op: locus::kernel::BufferOp::Literal,
            element: Type::U8,
            arguments: vec![Term::U8(4), Term::U8(8)],
        };
        let len = locus::kernel::buffer::length(Type::U8, buffer.clone());
        add(
            "buffer_length",
            scene.clone(),
            int_eq(len.clone(), Term::int(2)),
            Proof::BufferStep(len.clone()),
        );
        add(
            "buffer_lower",
            scene.clone(),
            Term::int_le(Term::int(0), len.clone()),
            Proof::BufferBound {
                value: buffer.clone(),
                upper: false,
            },
        );
        add(
            "buffer_upper",
            scene,
            Term::int_le(len, Term::Int(MachineInt::U64.max())),
            Proof::BufferBound {
                value: buffer,
                upper: true,
            },
        );
    }

    // D1/D4/D5 produce ordinary claims that the independent oracle can
    // decide; induction arms, motive binders, and recursive evidence are
    // included in the same proof mutation machinery as every existing rule.
    {
        let mut scene = Scene::new(&world.definitions);
        let n = scene.declare(Type::Int);
        add(
            "generic_refl_instance",
            scene,
            int_eq(n.clone(), n.clone()),
            Proof::OfTerm(Term::call(Term::Fn(world.generic_refl), vec![n])),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let id = world.induction_data;
        let zero = Term::Variant(id, 0, vec![]);
        let value = Term::Variant(id, 1, vec![zero.clone()]);
        let proof = Proof::DataInduction {
            target: value.clone(),
            motives: vec![(id, Term::eq(Type::Enum(id), Term::Bound(0), Term::Bound(0)))],
            arms: vec![
                Proof::arm(0, 0, |_, _| Proof::Refl(zero)),
                Proof::arm(1, 1, |p, _| Proof::Refl(Term::Variant(id, 1, p.to_vec()))),
            ],
        };
        add(
            "data_induction_finite",
            scene,
            Term::eq(Type::Enum(id), value.clone(), value),
            proof,
        );
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let n = scene.declare(Type::Int);
        let claim = int_eq(n.clone(), Term::int(0));
        let h = scene.assume(claim.clone());
        let proof = Proof::PropInduction {
            scrutinee: Box::new(Proof::Construct {
                prop: world.induction_prop,
                variant: 0,
                params: vec![n],
                payload: vec![Term::proof(Proof::hyp(h))],
            }),
            motive: TermArm {
                binders: 1,
                body: int_eq(Term::Bound(0), Term::int(0)),
            },
            arms: vec![
                Proof::arm(2, 1, |_, facts| facts[0].clone()),
                Proof::arm(2, 1, |p, facts| Proof::CaseProof {
                    scrutinee: Box::new(facts[0].clone()),
                    goal: int_eq(p[0].clone(), Term::int(0)),
                    arms: vec![Proof::arm(2, 0, |parts, _| Proof::OfTerm(parts[1].clone()))],
                }),
            ],
        };
        add("prop_induction_positive", scene, claim, proof);
    }

    // A named arm is constructed, opened, and its body evidence reused.
    // The oracle judges the final arithmetic claim independently of the
    // declared proposition, whose semantics are not built into the oracle.
    {
        let mut scene = Scene::new(&world.definitions);
        let n = scene.declare(Type::Int);
        let claim = Term::int_le(Term::int(0), n.clone());
        let h = scene.assume(claim.clone());
        let constructed = Proof::Construct {
            prop: world.witness_prop,
            variant: 0,
            params: vec![n.clone()],
            payload: vec![Term::int(0), Term::proof(Proof::hyp(h))],
        };
        // Elimination does not remember the chosen witness, so transport
        // an existential fact rather than asserting the witness stayed zero.
        let goal = Term::exists(Type::Int, |w| Term::int_le(w, n.clone()));
        let opened = Proof::CaseProof {
            scrutinee: Box::new(constructed),
            goal: goal.clone(),
            arms: vec![Proof::arm(2, 0, |fields, _| Proof::ExistsIntro {
                prop: goal.clone(),
                witness: fields[0].clone(),
                proof: Box::new(Proof::OfTerm(fields[1].clone())),
            })],
        };
        add("named_arm_witness_construct_eliminate", scene, goal, opened);
    }

    // Propositional structure.
    {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let h = scene.assume(le(&x, 3));
        add("hypothesis", scene, le(&x, 3), Proof::hyp(h));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::U8), scene.declare(Type::U8));
        let (p, q) = (le(&x, 3), u8_eq(y, Term::U8(1)));
        let (hp, hq) = (scene.assume(p.clone()), scene.assume(q.clone()));
        let proof = and_intro(prelude, &p, &q, Proof::hyp(hp), Proof::hyp(hq));
        add("and_intro", scene, prelude.and_prop(p, q), proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::U8), scene.declare(Type::U8));
        let (p, q) = (le(&x, 3), u8_eq(y, Term::U8(2)));
        let h = scene.assume(prelude.and_prop(p.clone(), q.clone()));
        let proof = Proof::CaseProof {
            scrutinee: Box::new(Proof::hyp(h)),
            goal: q.clone(),
            arms: vec![Proof::arm(2, 0, |payload, _| {
                Proof::OfTerm(payload[1].clone())
            })],
        };
        add("and_elim", scene, q, proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::U8), scene.declare(Type::U8));
        let (p, q) = (le(&x, 3), u8_eq(y, Term::U8(2)));
        let h = scene.assume(p.clone());
        let proof = or_intro(prelude, &p, &q, 0, Proof::hyp(h));
        add("or_left", scene, prelude.or_prop(p, q), proof);
    }
    {
        // p || q gives q || p.
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::U8), scene.declare(Type::U8));
        let (p, q) = (le(&x, 3), u8_eq(y, Term::U8(2)));
        let h = scene.assume(prelude.or_prop(p.clone(), q.clone()));
        let goal = prelude.or_prop(q.clone(), p.clone());
        let proof = Proof::CaseProof {
            scrutinee: Box::new(Proof::hyp(h)),
            goal: goal.clone(),
            arms: vec![
                Proof::arm(1, 0, |payload, _| {
                    or_intro(prelude, &q, &p, 1, Proof::OfTerm(payload[0].clone()))
                }),
                Proof::arm(1, 0, |payload, _| {
                    or_intro(prelude, &q, &p, 0, Proof::OfTerm(payload[0].clone()))
                }),
            ],
        };
        add("or_elim", scene, goal, proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::U8), scene.declare(Type::U8));
        let (p, q) = (u8_eq(x, Term::U8(0)), u8_eq(y, Term::U8(0)));
        let rule = scene.assume(Term::implies(p.clone(), q.clone()));
        let h = scene.assume(p);
        let proof = Proof::implies_elim(Proof::hyp(rule), Proof::hyp(h));
        add("modus_ponens", scene, q, proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let p = u8_eq(x, Term::U8(1));
        let proof = Proof::implies_intro(p.clone(), |h| h);
        add("implies_intro", scene, Term::implies(p.clone(), p), proof);
    }
    {
        let scene = Scene::new(&world.definitions);
        let p = u8_eq(Term::U8(1), Term::U8(2));
        let claim = prelude.or_prop(p.clone(), prelude.not_prop(p.clone()));
        add("excluded_middle", scene, claim, Proof::ExcludedMiddle(p));
    }

    // Quantifiers.
    {
        let scene = Scene::new(&world.definitions);
        let claim = Term::forall(Type::U8, |n| u8_eq(n.clone(), n));
        let proof = Proof::forall_intro(Type::U8, Proof::Refl);
        add("forall_intro", scene, claim, proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let h = scene.assume(Term::forall(Type::U8, |n| {
            u8_eq(
                Term::op(
                    Op::WrappingAdd,
                    MachineInt::U8,
                    vec![n.clone(), Term::U8(0)],
                ),
                n,
            )
        }));
        let claim = u8_eq(
            Term::op(
                Op::WrappingAdd,
                MachineInt::U8,
                vec![x.clone(), Term::U8(0)],
            ),
            x.clone(),
        );
        add(
            "forall_elim",
            scene,
            claim,
            Proof::forall_elim(Proof::hyp(h), x),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let claim = Term::exists(Type::U8, |n| u8_eq(n, Term::U8(0)));
        let proof = Proof::ExistsIntro {
            prop: claim.clone(),
            witness: Term::U8(0),
            proof: Box::new(Proof::Refl(Term::U8(0))),
        };
        add("exists_intro", scene, claim, proof);
    }
    {
        // exists n { n == x && n <= 3 } gives x <= 3.
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let h = scene.assume(Term::exists(Type::U8, |n| {
            prelude.and_prop(u8_eq(n.clone(), x.clone()), le(&n, 3))
        }));
        let goal = le(&x, 3);
        let proof = Proof::ExistsElim {
            exists: Box::new(Proof::hyp(h)),
            goal: goal.clone(),
            arm: Proof::arm(1, 1, |_, hyps| Proof::CaseProof {
                scrutinee: Box::new(hyps[0].clone()),
                goal: le(&x, 3),
                arms: vec![Proof::arm(2, 0, |payload, _| {
                    Proof::transport(
                        Proof::OfTerm(payload[0].clone()),
                        |hole| {
                            Term::int_le(
                                Term::view(MachineInt::U8, hole),
                                Term::view(MachineInt::U8, Term::U8(3)),
                            )
                        },
                        Proof::OfTerm(payload[1].clone()),
                    )
                })],
            }),
        };
        add("exists_elim", scene, goal, proof);
    }

    // Equality and rewriting.
    {
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::U8), scene.declare(Type::U8));
        let eq = scene.assume(u8_eq(x.clone(), y.clone()));
        let bound = scene.assume(le(&x, 3));
        let proof = Proof::transport(
            Proof::hyp(eq),
            |hole| {
                Term::int_le(
                    Term::view(MachineInt::U8, hole),
                    Term::view(MachineInt::U8, Term::U8(3)),
                )
            },
            Proof::hyp(bound),
        );
        add("transport", scene, le(&y, 3), proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::U8), scene.declare(Type::U8));
        let eq = scene.assume(u8_eq(x.clone(), y.clone()));
        let proof = symm_at(&Type::U8, &x, Proof::hyp(eq));
        add("symmetry", scene, u8_eq(y, x), proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let claim = u8_eq(x.clone(), x.clone());
        add("reflexivity", scene, claim, Proof::Refl(x));
    }

    // Projections and cases.
    {
        let mut scene = Scene::new(&world.definitions);
        let b = scene.declare(Type::Bool);
        let h = scene.assume(Term::eq(Type::Bool, b.clone(), Term::Bool(true)));
        let term = Term::case(
            b,
            Type::U8,
            vec![
                (0, Box::new(|_, _| Term::U8(2))),
                (0, Box::new(|_, _| Term::U8(7))),
            ],
        );
        let proof = Proof::CaseKnown {
            term: term.clone(),
            equation: Box::new(Proof::hyp(h)),
        };
        add(
            "case_known",
            scene,
            Term::eq(Type::U8, term, Term::U8(7)),
            proof,
        );
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let pair = Term::tuple(
            &Type::Tuple(vec![Type::U8, Type::Bool]),
            vec![x.clone(), Term::Bool(true)],
        );
        let projected = Term::proj(pair, 0);
        let claim = u8_eq(projected.clone(), x);
        add("projection", scene, claim, Proof::Projection(projected));
    }
    {
        let scene = Scene::new(&world.definitions);
        let case = Term::case(
            Term::Bool(true),
            Type::U8,
            vec![
                (0, Box::new(|_, _| Term::U8(1))),
                (0, Box::new(|_, _| Term::U8(2))),
            ],
        );
        let claim = u8_eq(case.clone(), Term::U8(2));
        add("case_step", scene, claim, Proof::CaseStep(case));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let b = scene.declare(Type::Bool);
        let is = |value: bool| Term::eq(Type::Bool, b.clone(), Term::Bool(value));
        let goal = prelude.or_prop(is(true), is(false));
        let proof = Proof::CaseData {
            scrutinee: b.clone(),
            goal: goal.clone(),
            arms: vec![
                Proof::arm(0, 1, |_, facts| {
                    or_intro(prelude, &is(true), &is(false), 1, facts[0].clone())
                }),
                Proof::arm(0, 1, |_, facts| {
                    or_intro(prelude, &is(true), &is(false), 0, facts[0].clone())
                }),
            ],
        };
        add("case_data", scene, goal, proof);
    }

    // Reflection of runtime comparisons at a machine type: each comparison
    // at two types, in both directions, with the claim written from the
    // test's own reading of the order of the views.
    for (ty, op) in [
        (U16, CmpOp::Le),
        (U16, CmpOp::Lt),
        (U16, CmpOp::Eq),
        (I32, CmpOp::Le),
        (I32, CmpOp::Lt),
        (I32, CmpOp::Eq),
    ] {
        let name = format!("cmp_reflect_{}_{}", op.name(), ty.name());
        let claim_of = |a: &Term, b: &Term| match op {
            CmpOp::Eq => int_eq(Term::view(ty, a.clone()), Term::view(ty, b.clone())),
            CmpOp::Lt => Term::int_lt(Term::view(ty, a.clone()), Term::view(ty, b.clone())),
            CmpOp::Le => Term::int_le(Term::view(ty, a.clone()), Term::view(ty, b.clone())),
        };
        {
            let mut scene = Scene::new(&world.definitions);
            let (a, b) = (
                scene.declare(Type::machine(ty)),
                scene.declare(Type::machine(ty)),
            );
            let comparison = Term::cmp(op, ty, a.clone(), b.clone());
            let h = scene.assume(Term::eq(Type::Bool, comparison.clone(), Term::Bool(true)));
            let proof = Proof::implies_elim(
                Proof::Axiom(Axiom::CmpReflect(comparison, true)),
                Proof::hyp(h),
            );
            add(&format!("{name}_true"), scene, claim_of(&a, &b), proof);
        }
        {
            let mut scene = Scene::new(&world.definitions);
            let a = scene.declare(Type::machine(ty));
            let b = lit(ty, 3);
            let comparison = Term::cmp(op, ty, a.clone(), b.clone());
            let h = scene.assume(Term::eq(Type::Bool, comparison.clone(), Term::Bool(false)));
            let proof = Proof::implies_elim(
                Proof::Axiom(Axiom::CmpReflect(comparison, false)),
                Proof::hyp(h),
            );
            add(
                &format!("{name}_false"),
                scene,
                prelude.not_prop(claim_of(&a, &b)),
                proof,
            );
        }
    }
    {
        // A comparison at a literal pair, evaluated, then reflected.
        let scene = Scene::new(&world.definitions);
        let comparison = Term::cmp(CmpOp::Lt, I8, lit(I8, -128), lit(I8, 127));
        let proof = Proof::implies_elim(
            Proof::Axiom(Axiom::CmpReflect(comparison.clone(), true)),
            Proof::Evaluate(comparison),
        );
        let claim = Term::int_lt(Term::view(I8, lit(I8, -128)), Term::view(I8, lit(I8, 127)));
        add("cmp_reflect_evaluated_i8", scene, claim, proof);
    }

    // Logical comparisons: independently checked Int oracle values and
    // mutants of both directions, flags, and comparison operators.
    for op in CmpOp::ALL {
        for flag in [false, true] {
            for reify in [false, true] {
                let mut scene = Scene::new(&world.definitions);
                let a = scene.declare(Type::Int);
                let b = Term::int(3);
                let test = Term::int_cmp(op, a.clone(), b.clone());
                let prop = match op {
                    CmpOp::Eq => int_eq(a, b),
                    CmpOp::Lt => Term::int_lt(a, b),
                    CmpOp::Le => Term::int_le(a, b),
                };
                let claim = if flag { prop } else { prelude.not_prop(prop) };
                let observed = Term::eq(Type::Bool, test.clone(), Term::Bool(flag));
                let (premise, goal) = if reify {
                    (claim, observed)
                } else {
                    (observed, claim)
                };
                let h = scene.assume(premise);
                let axiom = if reify {
                    Axiom::CmpReify(test, flag)
                } else {
                    Axiom::CmpReflect(test, flag)
                };
                add(
                    &format!("int_cmp_{}_{}_{}", op.name(), flag, reify),
                    scene,
                    goal,
                    Proof::implies_elim(ax(axiom), Proof::hyp(h)),
                );
            }
        }
    }

    // Evaluation.
    {
        let scene = Scene::new(&world.definitions);
        let sum = Term::op(
            Op::WrappingAdd,
            MachineInt::U8,
            vec![Term::U8(250), Term::U8(10)],
        );
        let claim = u8_eq(sum.clone(), Term::U8(4));
        add("literal", scene, claim, Proof::Literal(sum));
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = Term::call(
            Term::Fn(world.double),
            vec![Term::op(
                Op::WrappingAdd,
                MachineInt::U8,
                vec![Term::U8(100), Term::U8(50)],
            )],
        );
        let claim = u8_eq(term.clone(), Term::U8(44));
        add("evaluate", scene, claim, Proof::Evaluate(term));
    }

    // Definitions.
    {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let call = Term::call(Term::Fn(world.double), vec![x.clone()]);
        let claim = u8_eq(
            call.clone(),
            Term::op(Op::WrappingAdd, MachineInt::U8, vec![x.clone(), x]),
        );
        add("definition", scene, claim, Proof::Definition(call));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let call = Term::call(Term::Fn(world.within), vec![x.clone()]);
        let h = scene.assume(call.clone());
        let proof = unfold_claim(&call, Proof::hyp(h));
        add("unfold", scene, le(&x, 3), proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let call = Term::call(Term::Fn(world.within), vec![x.clone()]);
        let h = scene.assume(le(&x, 3));
        let proof = fold_claim(&call, Proof::hyp(h));
        add("fold", scene, call, proof);
    }

    // Lemmas of the theory, used as a caller uses them.
    {
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::U8);
        let b = scene.declare(Type::U8);
        let c = scene.declare(Type::U8);
        let ab = scene.assume(Term::int_le(
            Term::view(MachineInt::U8, a.clone()),
            Term::view(MachineInt::U8, b.clone()),
        ));
        let bc = scene.assume(Term::int_le(
            Term::view(MachineInt::U8, b.clone()),
            Term::view(MachineInt::U8, c.clone()),
        ));
        let proof = lemma(
            world.theory.machine(MachineInt::U8).le_trans,
            vec![
                a.clone(),
                b,
                c.clone(),
                Term::proof(Proof::hyp(ab)),
                Term::proof(Proof::hyp(bc)),
            ],
        );
        add(
            "lemma_call",
            scene,
            Term::int_le(Term::view(MachineInt::U8, a), Term::view(MachineInt::U8, c)),
            proof,
        );
    }
    {
        // A `for` over an empty range is its initial state.
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let state = Type::Tuple(vec![Type::U8]);
        let init = Term::tuple(&state, vec![Term::U8(5)]);
        let looped = Term::for_range(
            x.clone(),
            x.clone(),
            lemma(world.theory.machine(MachineInt::U8).le_refl, vec![x]),
            |_| state.clone(),
            init.clone(),
            |_, s, _, _| s,
        );
        let claim = Term::eq(state.clone(), looped.clone(), init);
        add("for_empty", scene, claim, Proof::ForEmpty(looped));
    }

    // Int: the ring axioms.
    let (iadd, imul, ile, ilit) = (Term::int_add, Term::int_mul, Term::int_le, Term::int);
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let claim = int_eq(iadd(a.clone(), b.clone()), iadd(b.clone(), a.clone()));
        add("int_add_comm", scene, claim, ax(Axiom::IntAddComm(a, b)));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b, c) = (
            scene.declare(Type::Int),
            scene.declare(Type::Int),
            scene.declare(Type::Int),
        );
        let claim = int_eq(
            imul(a.clone(), iadd(b.clone(), c.clone())),
            iadd(imul(a.clone(), b.clone()), imul(a.clone(), c.clone())),
        );
        add("int_mul_add", scene, claim, ax(Axiom::IntMulAdd(a, b, c)));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::Int);
        let claim = int_eq(iadd(a.clone(), Term::int_neg(a.clone())), ilit(0));
        add("int_add_neg", scene, claim, ax(Axiom::IntAddNeg(a)));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let claim = int_eq(
            Term::int_sub(a.clone(), b.clone()),
            iadd(a.clone(), Term::int_neg(b.clone())),
        );
        add("int_sub_def", scene, claim, ax(Axiom::IntSubDef(a, b)));
    }
    {
        // A ring equation derived in steps: (x + c) + (-c) == x.
        let mut scene = Scene::new(&world.definitions);
        let (x, c) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let neg_c = Term::int_neg(c.clone());
        let proof = Chain::new(Type::Int, iadd(iadd(x.clone(), c.clone()), neg_c.clone()))
            .step(ax(Axiom::IntAddAssoc(x.clone(), c.clone(), neg_c.clone())))
            .rewrite(
                |hole| iadd(x.clone(), hole),
                ax(Axiom::IntAddNeg(c.clone())),
            )
            .step(ax(Axiom::IntAddZero(x.clone())))
            .finish();
        let claim = int_eq(iadd(iadd(x.clone(), c), neg_c), x);
        add("int_add_then_subtract", scene, claim, proof);
    }

    // Int: the order axioms, used as implications.
    {
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::Int);
        add(
            "int_le_refl",
            scene,
            ile(a.clone(), a.clone()),
            ax(Axiom::IntLeRefl(a)),
        );
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b, c) = (
            scene.declare(Type::Int),
            scene.declare(Type::Int),
            scene.declare(Type::Int),
        );
        let h = scene.assume(ile(a.clone(), b.clone()));
        let proof = Proof::implies_elim(
            ax(Axiom::IntLeAdd(a.clone(), b.clone(), c.clone())),
            Proof::hyp(h),
        );
        add(
            "int_le_add",
            scene,
            ile(iadd(a, c.clone()), iadd(b, c)),
            proof,
        );
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b, c) = (
            scene.declare(Type::Int),
            scene.declare(Type::Int),
            scene.declare(Type::Int),
        );
        let ab = scene.assume(ile(a.clone(), b.clone()));
        let bc = scene.assume(ile(b.clone(), c.clone()));
        let proof = Proof::implies_elim(
            Proof::implies_elim(
                ax(Axiom::IntLeTrans(a.clone(), b, c.clone())),
                Proof::hyp(ab),
            ),
            Proof::hyp(bc),
        );
        add("int_le_trans", scene, ile(a, c), proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let ab = scene.assume(ile(a.clone(), b.clone()));
        let ba = scene.assume(ile(b.clone(), a.clone()));
        let proof = Proof::implies_elim(
            Proof::implies_elim(
                ax(Axiom::IntLeAntisymm(a.clone(), b.clone())),
                Proof::hyp(ab),
            ),
            Proof::hyp(ba),
        );
        add("int_le_antisymm", scene, int_eq(a, b), proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let pa = scene.assume(ile(ilit(0), a.clone()));
        let pb = scene.assume(ile(ilit(0), b.clone()));
        let proof = Proof::implies_elim(
            Proof::implies_elim(ax(Axiom::IntLeMul(a.clone(), b.clone())), Proof::hyp(pa)),
            Proof::hyp(pb),
        );
        add("int_le_mul", scene, ile(ilit(0), imul(a, b)), proof);
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let claim = prelude.or_prop(
            ile(a.clone(), b.clone()),
            Term::int_lt(b.clone(), a.clone()),
        );
        add("int_le_total", scene, claim, ax(Axiom::IntLeTotal(a, b)));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::Int);
        let claim = prelude.not_prop(Term::int_lt(a.clone(), a.clone()));
        add("int_lt_irrefl", scene, claim, ax(Axiom::IntLtIrrefl(a)));
    }
    {
        // a <= a + 1, derived: evaluation, two order axioms, and transport.
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::Int);
        let claim = ile(a.clone(), iadd(a.clone(), ilit(1)));
        add("int_le_succ", scene, claim, int_le_succ(&a));
    }

    // Int: evaluation, of closed computations and of closed comparisons.
    {
        let scene = Scene::new(&world.definitions);
        let term = Term::call(
            Term::Fn(world.int_double),
            vec![Term::int_sub(ilit(1), ilit(22))],
        );
        let claim = int_eq(term.clone(), ilit(-42));
        add("int_evaluate", scene, claim, Proof::Evaluate(term));
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = iadd(imul(ilit(-6), ilit(7)), Term::int_neg(ilit(-2)));
        let claim = int_eq(term.clone(), ilit(-40));
        add("int_evaluate_closed", scene, claim, Proof::Evaluate(term));
    }
    {
        let scene = Scene::new(&world.definitions);
        let claim = ile(ilit(-3), ilit(2));
        add(
            "int_evaluate_le_true",
            scene,
            claim.clone(),
            Proof::Evaluate(claim),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let comparison = ile(ilit(2), ilit(-3));
        let claim = prelude.not_prop(comparison.clone());
        add(
            "int_evaluate_le_false",
            scene,
            claim,
            Proof::Evaluate(comparison),
        );
    }
    {
        // `2 < 2` is `2 + 1 <= 2`, so this is decided by the same rule.
        let scene = Scene::new(&world.definitions);
        let comparison = Term::int_lt(ilit(2), ilit(2));
        let claim = prelude.not_prop(comparison.clone());
        add(
            "int_evaluate_lt_false",
            scene,
            claim,
            Proof::Evaluate(comparison),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let call = Term::call(
            Term::Fn(world.int_double),
            vec![Term::int_sub(ilit(1), ilit(22))],
        );
        let claim = int_eq(call, imul(ilit(-6), ilit(7)));
        add(
            "int_evaluate_eq",
            scene,
            claim.clone(),
            Proof::Evaluate(claim),
        );
    }

    // Int: division and remainder.
    let (idiv, irem) = (Term::int_div, Term::int_rem);
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let claim = int_eq(
            a.clone(),
            iadd(
                imul(idiv(a.clone(), b.clone()), b.clone()),
                irem(a.clone(), b.clone()),
            ),
        );
        add("int_div_rem", scene, claim, ax(Axiom::IntDivRem(a, b)));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::Int);
        let claim = int_eq(idiv(a.clone(), ilit(0)), ilit(0));
        add("int_div_zero", scene, claim, ax(Axiom::IntDivZero(a)));
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let h = scene.assume(Term::int_lt(ilit(0), b.clone()));
        let proof = Proof::implies_elim(
            ax(Axiom::IntRemUpperPos(a.clone(), b.clone())),
            Proof::hyp(h),
        );
        add(
            "int_rem_upper_pos",
            scene,
            Term::int_lt(irem(a, b.clone()), b),
            proof,
        );
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let h = scene.assume(Term::int_lt(b.clone(), ilit(0)));
        let proof = Proof::implies_elim(
            ax(Axiom::IntRemLowerNeg(a.clone(), b.clone())),
            Proof::hyp(h),
        );
        add(
            "int_rem_lower_neg",
            scene,
            Term::int_lt(b.clone(), irem(a, b)),
            proof,
        );
    }
    {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let h = scene.assume(ile(ilit(0), a.clone()));
        let proof =
            Proof::implies_elim(ax(Axiom::IntRemNonneg(a.clone(), b.clone())), Proof::hyp(h));
        add("int_rem_nonneg", scene, ile(ilit(0), irem(a, b)), proof);
    }
    {
        // Truncation toward zero: -7 / 2 == -3 and -7 % 2 == -1, and a / 0 == 0.
        let scene = Scene::new(&world.definitions);
        let term = idiv(ilit(-7), ilit(2));
        add(
            "int_evaluate_div",
            scene,
            int_eq(term.clone(), ilit(-3)),
            Proof::Evaluate(term),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = irem(ilit(-7), ilit(2));
        add(
            "int_evaluate_rem",
            scene,
            int_eq(term.clone(), ilit(-1)),
            Proof::Evaluate(term),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = irem(ilit(7), ilit(0));
        add(
            "int_evaluate_rem_zero",
            scene,
            int_eq(term.clone(), ilit(7)),
            Proof::Evaluate(term),
        );
    }

    // Int: induction over the non-negative integers.
    {
        // 0 <= n => a <= a + n.
        let mut scene = Scene::new(&world.definitions);
        let (a, n) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let motive = |k: Term| ile(a.clone(), iadd(a.clone(), k));
        let base = Proof::transport(
            symm_at(
                &Type::Int,
                &iadd(a.clone(), ilit(0)),
                ax(Axiom::IntAddZero(a.clone())),
            ),
            |hole| ile(a.clone(), hole),
            ax(Axiom::IntLeRefl(a.clone())),
        );
        let step = |k: Term, _: Proof, ih: Proof| {
            // a <= a + k <= (a + k) + 1 == a + (k + 1)
            let sum = iadd(a.clone(), k.clone());
            let longer = Proof::implies_elim(
                Proof::implies_elim(
                    ax(Axiom::IntLeTrans(
                        a.clone(),
                        sum.clone(),
                        iadd(sum.clone(), ilit(1)),
                    )),
                    ih,
                ),
                int_le_succ(&sum),
            );
            Proof::transport(
                ax(Axiom::IntAddAssoc(a.clone(), k, ilit(1))),
                |hole| ile(a.clone(), hole),
                longer,
            )
        };
        let proof = Proof::int_induction(motive, base, step, n.clone());
        let claim = Term::implies(ile(ilit(0), n.clone()), motive(n));
        add("int_induction", scene, claim, proof);
    }

    // The machine integer models: each schema at an unsigned and a signed
    // type. The bounds and periods in the claims are written from the
    // test's own table, not read from the kernel's.
    let (view, wrap, cast) = (Term::view, Term::wrap, Term::cast);
    let eq_at = |ty: MachineInt, left: Term, right: Term| Term::eq(Type::machine(ty), left, right);
    let min_of = |ty: MachineInt| int128(machine_range(ty).0);
    let max_of = |ty: MachineInt| int128(machine_range(ty).1);
    let period_of = |ty: MachineInt| int128(1i128 << shape(ty).0);
    for ty in [U16, I32] {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::machine(ty));
        let claim = ile(min_of(ty), view(ty, x.clone()));
        add(
            &format!("view_lower_{}", ty.name()),
            scene,
            claim,
            ax(Axiom::ViewLower(ty, x)),
        );
    }
    for ty in [U64, I8] {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::machine(ty));
        let claim = ile(view(ty, x.clone()), max_of(ty));
        add(
            &format!("view_upper_{}", ty.name()),
            scene,
            claim,
            ax(Axiom::ViewUpper(ty, x)),
        );
    }
    for ty in [U32, I16] {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::machine(ty));
        let claim = eq_at(ty, wrap(ty, view(ty, x.clone())), x.clone());
        add(
            &format!("wrap_view_{}", ty.name()),
            scene,
            claim,
            ax(Axiom::WrapView(ty, x)),
        );
    }
    for ty in [U16, I64] {
        // The round trip on Int, under its two premises as hypotheses.
        let mut scene = Scene::new(&world.definitions);
        let n = scene.declare(Type::Int);
        let lower = scene.assume(ile(min_of(ty), n.clone()));
        let upper = scene.assume(ile(n.clone(), max_of(ty)));
        let proof = Proof::implies_elim(
            Proof::implies_elim(ax(Axiom::ViewWrap(ty, n.clone())), Proof::hyp(lower)),
            Proof::hyp(upper),
        );
        let claim = int_eq(view(ty, wrap(ty, n.clone())), n);
        add(&format!("view_wrap_{}", ty.name()), scene, claim, proof);
    }
    {
        // The same axiom stated whole, so that its premises are attacked.
        let mut scene = Scene::new(&world.definitions);
        let n = scene.declare(Type::Int);
        let claim = Term::implies(
            ile(min_of(I8), n.clone()),
            Term::implies(
                ile(n.clone(), max_of(I8)),
                int_eq(view(I8, wrap(I8, n.clone())), n.clone()),
            ),
        );
        add(
            "view_wrap_implication_i8",
            scene,
            claim,
            ax(Axiom::ViewWrap(I8, n)),
        );
    }
    for ty in [U8, I32] {
        let mut scene = Scene::new(&world.definitions);
        let n = scene.declare(Type::Int);
        let claim = eq_at(
            ty,
            wrap(ty, iadd(n.clone(), period_of(ty))),
            wrap(ty, n.clone()),
        );
        add(
            &format!("wrap_period_{}", ty.name()),
            scene,
            claim,
            ax(Axiom::WrapPeriod(ty, n)),
        );
    }
    for (from, to) in [(U16, I8), (I8, U64)] {
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::machine(from));
        let claim = eq_at(
            to,
            cast(from, to, x.clone()),
            wrap(to, view(from, x.clone())),
        );
        add(
            &format!("cast_def_{}_{}", from.name(), to.name()),
            scene,
            claim,
            ax(Axiom::CastDef(from, to, x)),
        );
    }
    for ty in [U16, I32] {
        // cast(T, T)(x) == x, derived: cast_def, then wrap_view.
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::machine(ty));
        let proof = Chain::new(Type::machine(ty), cast(ty, ty, x.clone()))
            .step(ax(Axiom::CastDef(ty, ty, x.clone())))
            .step(ax(Axiom::WrapView(ty, x.clone())))
            .finish();
        let claim = eq_at(ty, cast(ty, ty, x.clone()), x);
        add(&format!("cast_identity_{}", ty.name()), scene, claim, proof);
    }

    // The table of primitive operations: a few rows of each schema, with
    // the claims written from the test's own table. `op_model` at an
    // operation that overflows, at a division, and at a negation;
    // `op_exact` under its premises as hypotheses, and stated whole.
    let op = |op: Op, ty: MachineInt, operands: Vec<Term>| Term::op(op, ty, operands);
    for (which, ty) in [(Op::Add, U16), (Op::Mul, I32), (Op::WrappingSub, U8)] {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (
            scene.declare(Type::machine(ty)),
            scene.declare(Type::machine(ty)),
        );
        let exact = match which {
            Op::Add => iadd(view(ty, a.clone()), view(ty, b.clone())),
            Op::Mul => imul(view(ty, a.clone()), view(ty, b.clone())),
            _ => Term::int_sub(view(ty, a.clone()), view(ty, b.clone())),
        };
        let claim = eq_at(
            ty,
            op(which, ty, vec![a.clone(), b.clone()]),
            wrap(ty, exact),
        );
        add(
            &format!("op_model_{}_{}", which.name(), ty.name()),
            scene,
            claim,
            ax(Axiom::OpModel(which, ty, vec![a, b])),
        );
    }
    for (which, ty) in [(Op::Div, I8), (Op::Rem, U64)] {
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (
            scene.declare(Type::machine(ty)),
            scene.declare(Type::machine(ty)),
        );
        let exact = match which {
            Op::Div => Term::int_div(view(ty, a.clone()), view(ty, b.clone())),
            _ => Term::int_rem(view(ty, a.clone()), view(ty, b.clone())),
        };
        let claim = eq_at(
            ty,
            op(which, ty, vec![a.clone(), b.clone()]),
            wrap(ty, exact),
        );
        add(
            &format!("op_model_{}_{}", which.name(), ty.name()),
            scene,
            claim,
            ax(Axiom::OpModel(which, ty, vec![a, b])),
        );
    }
    for ty in [I16, I64] {
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::machine(ty));
        let claim = eq_at(
            ty,
            op(Op::Neg, ty, vec![a.clone()]),
            wrap(ty, Term::int_neg(view(ty, a.clone()))),
        );
        add(
            &format!("op_model_neg_{}", ty.name()),
            scene,
            claim,
            ax(Axiom::OpModel(Op::Neg, ty, vec![a])),
        );
    }
    for ty in [U8, I32] {
        // The exact sum, under the two premises as hypotheses.
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (
            scene.declare(Type::machine(ty)),
            scene.declare(Type::machine(ty)),
        );
        let exact = iadd(view(ty, a.clone()), view(ty, b.clone()));
        let lower = scene.assume(ile(min_of(ty), exact.clone()));
        let upper = scene.assume(ile(exact.clone(), max_of(ty)));
        let proof = Proof::implies_elim(
            Proof::implies_elim(
                ax(Axiom::OpExact(Op::Add, ty, vec![a.clone(), b.clone()])),
                Proof::hyp(lower),
            ),
            Proof::hyp(upper),
        );
        let claim = int_eq(view(ty, op(Op::Add, ty, vec![a, b])), exact);
        add(&format!("op_exact_add_{}", ty.name()), scene, claim, proof);
    }
    {
        // The same schema stated whole at a product, so that its premises
        // are attacked.
        let mut scene = Scene::new(&world.definitions);
        let (a, b) = (
            scene.declare(Type::machine(I8)),
            scene.declare(Type::machine(I8)),
        );
        let exact = imul(view(I8, a.clone()), view(I8, b.clone()));
        let claim = Term::implies(
            ile(min_of(I8), exact.clone()),
            Term::implies(
                ile(exact.clone(), max_of(I8)),
                int_eq(view(I8, op(Op::Mul, I8, vec![a.clone(), b.clone()])), exact),
            ),
        );
        add(
            "op_exact_implication_mul_i8",
            scene,
            claim,
            ax(Axiom::OpExact(Op::Mul, I8, vec![a, b])),
        );
    }
    {
        // Negation stated whole.
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::machine(I16));
        let exact = Term::int_neg(view(I16, a.clone()));
        let claim = Term::implies(
            ile(min_of(I16), exact.clone()),
            Term::implies(
                ile(exact.clone(), max_of(I16)),
                int_eq(view(I16, op(Op::Neg, I16, vec![a.clone()])), exact),
            ),
        );
        add(
            "op_exact_implication_neg_i16",
            scene,
            claim,
            ax(Axiom::OpExact(Op::Neg, I16, vec![a])),
        );
    }

    // The table of primitive operations: evaluation of closed operations,
    // at the pairs where Rust panics included, since the kernel computes the
    // meaning that holds in every build.
    for (name, term, ty, value) in [
        (
            "add_u8",
            op(Op::Add, U8, vec![Term::U8(200), Term::U8(100)]),
            U8,
            44,
        ),
        (
            "sub_u16",
            op(Op::Sub, U16, vec![lit(U16, 0), lit(U16, 1)]),
            U16,
            65535,
        ),
        (
            "mul_i8",
            op(Op::Mul, I8, vec![lit(I8, -128), lit(I8, -1)]),
            I8,
            -128,
        ),
        (
            "div_i8_min",
            op(Op::Div, I8, vec![lit(I8, -128), lit(I8, -1)]),
            I8,
            -128,
        ),
        (
            "rem_i8_min",
            op(Op::Rem, I8, vec![lit(I8, -128), lit(I8, -1)]),
            I8,
            0,
        ),
        (
            "div_u32_zero",
            op(Op::Div, U32, vec![lit(U32, 7), lit(U32, 0)]),
            U32,
            0,
        ),
        (
            "rem_u32_zero",
            op(Op::Rem, U32, vec![lit(U32, 7), lit(U32, 0)]),
            U32,
            7,
        ),
        (
            "div_i64",
            op(Op::Div, I64, vec![lit(I64, -7), lit(I64, 2)]),
            I64,
            -3,
        ),
        (
            "rem_i64",
            op(Op::Rem, I64, vec![lit(I64, -7), lit(I64, 2)]),
            I64,
            -1,
        ),
        (
            "neg_i16_min",
            op(Op::Neg, I16, vec![lit(I16, -32768)]),
            I16,
            -32768,
        ),
        (
            "wrapping_add_u8",
            op(Op::WrappingAdd, U8, vec![Term::U8(255), Term::U8(1)]),
            U8,
            0,
        ),
        (
            "wrapping_mul_i32",
            op(Op::WrappingMul, I32, vec![lit(I32, 65536), lit(I32, 65536)]),
            I32,
            0,
        ),
        (
            "wrapping_neg_i8",
            op(Op::WrappingNeg, I8, vec![lit(I8, -128)]),
            I8,
            -128,
        ),
    ] {
        let scene = Scene::new(&world.definitions);
        let claim = eq_at(ty, term.clone(), lit(ty, value));
        add(
            &format!("evaluate_{name}"),
            scene,
            claim,
            Proof::Evaluate(term),
        );
    }
    {
        // The row `wrapping_add` at `u8` agrees with the primitive of the
        // `u8` model on closed values: both evaluate, and the two results
        // are chained.
        let scene = Scene::new(&world.definitions);
        let (a, b) = (Term::U8(200), Term::U8(100));
        let row = op(Op::WrappingAdd, U8, vec![a.clone(), b.clone()]);
        let model = Term::op(Op::WrappingAdd, MachineInt::U8, vec![a, b]);
        let proof = Chain::new(Type::U8, row.clone())
            .step(Proof::Evaluate(row.clone()))
            .step_rev(&model, Proof::Evaluate(model.clone()))
            .finish();
        let claim = eq_at(U8, row, model);
        add("wrapping_add_rows_agree", scene, claim, proof);
    }

    // Machine integers: evaluation of closed casts, wraps, and views.
    {
        let scene = Scene::new(&world.definitions);
        let term = cast(U16, I8, lit(U16, 300));
        let claim = eq_at(I8, term.clone(), lit(I8, 44));
        add(
            "evaluate_cast_to_signed",
            scene,
            claim,
            Proof::Evaluate(term),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = cast(I8, U64, lit(I8, -1));
        let claim = eq_at(U64, term.clone(), lit(U64, i128::from(u64::MAX)));
        add(
            "evaluate_cast_to_unsigned",
            scene,
            claim,
            Proof::Evaluate(term),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = cast(I16, U8, lit(I16, -1));
        let claim = eq_at(U8, term.clone(), Term::U8(255));
        add("literal_cast", scene, claim, Proof::Literal(term));
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = wrap(U32, ilit(-1));
        let claim = eq_at(U32, term.clone(), lit(U32, i128::from(u32::MAX)));
        add(
            "evaluate_wrap_unsigned",
            scene,
            claim,
            Proof::Evaluate(term),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = wrap(I8, ilit(200));
        let claim = eq_at(I8, term.clone(), lit(I8, -56));
        add("evaluate_wrap_signed", scene, claim, Proof::Evaluate(term));
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = view(I16, lit(I16, -5));
        let claim = int_eq(term.clone(), ilit(-5));
        add("evaluate_view", scene, claim, Proof::Evaluate(term));
    }
    {
        let scene = Scene::new(&world.definitions);
        let term = view(U8, cast(I8, U8, lit(I8, -1)));
        let claim = int_eq(term.clone(), ilit(255));
        add("evaluate_view_of_cast", scene, claim, Proof::Evaluate(term));
    }
    {
        let scene = Scene::new(&world.definitions);
        let claim = ile(view(U64, lit(U64, i128::from(u64::MAX))), max_of(U64));
        add(
            "evaluate_view_le_true",
            scene,
            claim.clone(),
            Proof::Evaluate(claim),
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let comparison = ile(view(I8, lit(I8, -128)), ilit(-129));
        let claim = prelude.not_prop(comparison.clone());
        add(
            "evaluate_view_le_false",
            scene,
            claim,
            Proof::Evaluate(comparison),
        );
    }

    // Machine integers with Int arithmetic: view is bounded, and the bound
    // moves with the arithmetic.
    {
        // view(x) < 256, which is view(x) + 1 <= 256: from view(x) <= 255 by
        // adding one to both sides and evaluating 255 + 1.
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::U8);
        let v = view(U8, x.clone());
        let shifted = Proof::implies_elim(
            ax(Axiom::IntLeAdd(v.clone(), ilit(255), ilit(1))),
            ax(Axiom::ViewUpper(U8, x)),
        );
        let sum = Proof::Evaluate(iadd(ilit(255), ilit(1)));
        let proof = Proof::transport(sum, |hole| ile(iadd(v.clone(), ilit(1)), hole), shifted);
        let claim = Term::int_lt(v, ilit(256));
        add("view_lt_256", scene, claim, proof);
    }
    {
        // 0 <= view(x) + 32768 for x : i16, from -32768 <= view(x).
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::Machine(I16));
        let v = view(I16, x.clone());
        let shifted = Proof::implies_elim(
            ax(Axiom::IntLeAdd(ilit(-32768), v.clone(), ilit(32768))),
            ax(Axiom::ViewLower(I16, x)),
        );
        let sum = Proof::Evaluate(iadd(ilit(-32768), ilit(32768)));
        let proof = Proof::transport(sum, |hole| ile(hole, iadd(v.clone(), ilit(32768))), shifted);
        let claim = ile(ilit(0), iadd(v, ilit(32768)));
        add("view_shifted_nonneg_i16", scene, claim, proof);
    }
    {
        // An equation at a machine type carries a bound across.
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (
            scene.declare(Type::Machine(I32)),
            scene.declare(Type::Machine(I32)),
        );
        let eq = scene.assume(eq_at(I32, x.clone(), y.clone()));
        let proof = Proof::transport(
            Proof::hyp(eq),
            |hole| ile(view(I32, hole), max_of(I32)),
            ax(Axiom::ViewUpper(I32, x)),
        );
        add(
            "transport_machine",
            scene,
            ile(view(I32, y), max_of(I32)),
            proof,
        );
    }
    {
        let scene = Scene::new(&world.definitions);
        let claim = Term::forall(Type::Machine(I16), |x| {
            eq_at(I16, wrap(I16, view(I16, x.clone())), x)
        });
        let proof = Proof::forall_intro(Type::Machine(I16), |x| ax(Axiom::WrapView(I16, x)));
        add("forall_machine", scene, claim, proof);
    }
    {
        let scene = Scene::new(&world.definitions);
        let claim = Term::exists(Type::Machine(U32), |x| int_eq(view(U32, x), ilit(7)));
        let instance = int_eq(view(U32, lit(U32, 7)), ilit(7));
        let proof = Proof::ExistsIntro {
            prop: claim.clone(),
            witness: lit(U32, 7),
            proof: Box::new(Proof::Evaluate(instance)),
        };
        add("exists_machine", scene, claim, proof);
    }

    // Linear certificates: each kind of constraint and goal once.
    {
        // The lock's `fits`: view(x) + 1 <= u32::MAX from view(x) < 3.
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::machine(U32));
        let v = view(U32, x);
        let small = scene.assume(Term::int_lt(v.clone(), ilit(3)));
        let claim = ile(iadd(v, ilit(1)), max_of(U32));
        let proof = Proof::linear(claim.clone(), 1, vec![(Proof::hyp(small), 1)]);
        add("linear_lock_fits", scene, claim, proof);
    }
    {
        // a / 2 <= a for 0 <= a, from the decomposition and the sign of the
        // remainder: 2(q - a - 1) - (2q + r - a) + a + r = -2.
        let mut scene = Scene::new(&world.definitions);
        let a = scene.declare(Type::Int);
        let nonneg = scene.assume(ile(ilit(0), a.clone()));
        let claim = ile(idiv(a.clone(), ilit(2)), a.clone());
        let sign = Proof::implies_elim(
            ax(Axiom::IntRemNonneg(a.clone(), ilit(2))),
            Proof::hyp(nonneg),
        );
        let proof = Proof::linear(
            claim.clone(),
            2,
            vec![
                (ax(Axiom::IntDivRem(a, ilit(2))), -1),
                (Proof::hyp(nonneg), 1),
                (sign, 1),
            ],
        );
        add("linear_half_le", scene, claim, proof);
    }
    {
        // !(3 <= x) from x <= 2: the hypothesis is introduced and the
        // certificate proves False from the two.
        let mut scene = Scene::new(&world.definitions);
        let x = scene.declare(Type::Int);
        let upper = scene.assume(ile(x.clone(), ilit(2)));
        let below = ile(ilit(3), x);
        let claim = prelude.not_prop(below.clone());
        let proof = Proof::implies_intro(below, |lower| {
            Proof::linear(
                prelude.falsehood_prop(),
                1,
                vec![(Proof::hyp(upper), 1), (lower, 1)],
            )
        });
        add("linear_false", scene, claim, proof);
    }
    {
        // y + 1 <= x from x == y + 1, the equation with a negative
        // coefficient, and the negated inequality x < y + 1 as a hypothesis.
        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let successor = iadd(y.clone(), ilit(1));
        let equation = scene.assume(int_eq(x.clone(), successor.clone()));
        let claim = ile(successor.clone(), x.clone());
        let proof = Proof::linear(claim.clone(), 1, vec![(Proof::hyp(equation), -1)]);
        add("linear_equation", scene, claim, proof);

        let mut scene = Scene::new(&world.definitions);
        let (x, y) = (scene.declare(Type::Int), scene.declare(Type::Int));
        let successor = iadd(y.clone(), ilit(1));
        let not_below = scene.assume(prelude.not_prop(ile(successor.clone(), x.clone())));
        let claim = ile(x, successor);
        let proof = Proof::linear(claim.clone(), 1, vec![(Proof::hyp(not_below), 1)]);
        add("linear_negated_hypothesis", scene, claim, proof);
    }
    out
}

// --- Triples from the corpus ------------------------------------------------------------

/// The proofs in the body of a math function, which is where a lemma keeps
/// its proof. The body is under one binder per parameter, and so is the
/// signature, so a proof in it is closed by one `forall` per parameter: the
/// kernel then opens the binders itself, and says what the closed proof
/// proves. Returns how many proofs the kernel gave no claim for.
fn math_triples(
    definitions: &Rc<Definitions>,
    id: FnId,
    origin: &str,
    out: &mut Vec<Triple>,
) -> usize {
    let Some(Type::Fn(params, _)) = definitions.signature(id) else {
        return 0;
    };
    let Some((_, body)) = definitions.function_body(id) else {
        return 0;
    };
    let mut found = Vec::new();
    embedded_proofs(body, &mut found);
    let mut unplaced = 0;
    for proof in found {
        let closed = params
            .iter()
            .rev()
            .fold(proof.clone(), |body, ty| Proof::ForallIntro {
                ty: ty.clone(),
                body: Box::new(body),
            });
        let mut scene = Scene::new(definitions);
        match infer_proof(&mut scene.ctx, &closed) {
            Ok(claim) => out.push(Triple {
                origin: origin.to_string(),
                scene,
                claim,
                proof: closed,
            }),
            Err(_) => unplaced += 1,
        }
    }
    unplaced
}

/// Proofs in a term that are under none of the term's own binders.
fn embedded_proofs<'t>(term: &'t Term, out: &mut Vec<&'t Proof>) {
    match term {
        Term::Proof(proof) => out.push(proof),
        Term::Boxed(term) => embedded_proofs(term, out),
        Term::Tuple(_, terms) | Term::Struct(_, terms) | Term::Variant(_, _, terms) => {
            for term in terms {
                embedded_proofs(term, out);
            }
        }
        _ => {}
    }
}

/// The `.lc` files of a directory of the repository, in order.
fn files_in(directory: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths: Vec<PathBuf> = std::fs::read_dir(root.join(directory))
        .unwrap_or_else(|error| panic!("{directory}: {error}"))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "lc"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            let text = std::fs::read_to_string(path).unwrap();
            (format!("{directory}/{name}"), text)
        })
        .collect()
}

struct Corpus {
    triples: Vec<Triple>,
    /// Proofs the search found, which is every `_` and every conversion.
    found: usize,
    /// Proofs in the bodies of math functions, written out or found.
    in_math_bodies: usize,
    unplaced: usize,
}

/// Every proof of every file of the corpus. The elaborator records each
/// proof its search finds, with the claim and the context the kernel
/// accepted it in, wherever in the program it stands: under a branch of a
/// pure `if`, inside a loop, in a math function. A math function whose body
/// holds proofs gives those as well, closed over its parameters, which
/// covers evidence that was written out and not searched for.
fn corpus_triples() -> Corpus {
    let mut corpus = Corpus {
        triples: Vec::new(),
        found: 0,
        in_math_bodies: 0,
        unplaced: 0,
    };
    for directory in ["examples", "tests/corpus/accept"] {
        for (name, text) in files_in(directory) {
            let mut sources = SourceMap::default();
            let file = sources.add(name.clone(), &text);
            let source = sources.get(file);
            let parsed = parse(source);
            assert!(parsed.is_success(), "{name} does not parse");
            let mut options = locus::elab::Options::default();
            for line in text.lines() {
                if let Some(feature) = line.trim().strip_prefix("//~ preview:") {
                    let feature =
                        locus::preview::Feature::parse(feature.trim()).expect("known preview");
                    if feature.status() == locus::preview::Status::Preview {
                        options.previews.enable(feature.name()).unwrap();
                    }
                }
            }
            let elaborated = locus::elab::elaborate_with_options(source, &parsed.program, &options);
            assert!(
                elaborated.is_success(),
                "{name} is not accepted: {:?}",
                elaborated.diagnostics
            );
            let definitions = Rc::new(elaborated.session.program().definitions().clone());
            for (index, hole) in elaborated.holes.iter().enumerate() {
                let found = hole
                    .found
                    .as_ref()
                    .expect("an accepted file has no open hole");
                corpus.found += 1;
                corpus.triples.push(Triple {
                    origin: format!("{name}#{index}"),
                    scene: Scene::of_context(&definitions, &found.context),
                    claim: found.claim.clone(),
                    proof: found.proof.clone(),
                });
            }
            for (function, reference) in &elaborated.functions {
                if let FnRef::Math(id) = reference {
                    let before = corpus.triples.len();
                    corpus.unplaced += math_triples(
                        &definitions,
                        *id,
                        &format!("{name}::{function}"),
                        &mut corpus.triples,
                    );
                    corpus.in_math_bodies += corpus.triples.len() - before;
                }
            }
        }
    }
    corpus
}

/// The lemmas of the kernel theory: each is a math function whose body is a
/// proof, found by no search and checked when the theory was declared.
fn theory_triples(world: &World) -> Vec<Triple> {
    let theory = &world.theory;
    let mut triples = Vec::new();
    // The lemmas about `Int` and about the machine types come from the
    // table of names that E5 exposes to source, so a lemma added there is
    // attacked here without being listed twice. The family about a machine
    // type is one code path declared at each type, so it is attacked at an
    // unsigned and a signed type, neither of them `u8`, whose every value
    // the oracle would try under each quantifier; `tests/kernel_lemmas.rs`
    // uses every lemma at every type.
    let named: Vec<(&str, FnId)> = theory
        .lemma_names()
        .into_iter()
        .filter(|(name, _)| {
            name.starts_with("int_") || name.starts_with("u16_") || name.starts_with("i32_")
        })
        .collect();
    for (name, id) in named.iter().copied() {
        let origin = format!("theory/{name}");
        let before = triples.len();
        let unplaced = math_triples(&world.definitions, id, &origin, &mut triples);
        assert_eq!((triples.len() - before, unplaced), (1, 0), "{origin}");
    }
    triples
}

// --- The tests --------------------------------------------------------------------------

/// The oracle and the kernel's evaluator were written apart. On closed data
/// they must agree, or one of them is wrong about what a term means.
#[test]
fn the_oracle_agrees_with_the_kernel_on_closed_terms() {
    let world = world();
    let scene = Scene::new(&world.definitions);
    let mut ctx = scene.ctx.clone();
    let mut rng = Rng(SEED ^ 3);
    fn byte(rng: &mut Rng, world: &World, depth: usize) -> Term {
        let edge = [0, 1, 2, 127, 128, 254, 255];
        if depth == 0 {
            return Term::U8(*rng.pick(&edge).unwrap());
        }
        let sub = |rng: &mut Rng| byte(rng, world, depth - 1);
        match rng.below(6) {
            0 => Term::op(Op::WrappingAdd, MachineInt::U8, vec![sub(rng), sub(rng)]),
            1 => Term::op(Op::WrappingSub, MachineInt::U8, vec![sub(rng), sub(rng)]),
            2 => Term::call(Term::Fn(world.double), vec![sub(rng)]),
            3 => Term::wrap(
                MachineInt::U8,
                Term::int_add(
                    Term::view(MachineInt::U8, sub(rng)),
                    Term::int_add(Term::view(MachineInt::U8, sub(rng)), Term::int(1)),
                ),
            ),
            4 => {
                let (no, yes) = (sub(rng), sub(rng));
                let op = *rng.pick(&CmpOp::ALL).unwrap();
                Term::case(
                    Term::cmp(op, MachineInt::U8, sub(rng), sub(rng)),
                    Type::U8,
                    vec![
                        (0, Box::new(move |_, _| no)),
                        (0, Box::new(move |_, _| yes)),
                    ],
                )
            }
            _ => Term::proj(
                Term::tuple(
                    &Type::Tuple(vec![Type::Bool, Type::U8]),
                    vec![Term::Bool(true), sub(rng)],
                ),
                1,
            ),
        }
    }
    for _ in 0..300 {
        let term = byte(&mut rng, &world, 3);
        let Ok(Term::Eq(_, _, value)) = infer_proof(&mut ctx, &Proof::Evaluate(term.clone()))
        else {
            panic!("the kernel does not evaluate {term}");
        };
        let mut oracle = Oracle::new(&scene);
        let found = oracle.value(&term);
        let Term::U8(expected) = *value else {
            panic!("{term} evaluates to {value}");
        };
        assert_eq!(found, Some(Value::U8(expected)), "{term}");
    }

    // The same for the integers, whose oracle arithmetic is `i128`: where
    // the oracle has a value it is the kernel's, and where it has none the
    // kernel's value is outside `i128`.
    fn integer(rng: &mut Rng, world: &World, depth: usize) -> Term {
        let edge: [i64; 9] = [-3, -1, 0, 1, 2, 7, 255, i64::MIN, i64::MAX];
        if depth == 0 {
            return Term::int(*rng.pick(&edge).unwrap());
        }
        let sub = |rng: &mut Rng| integer(rng, world, depth - 1);
        match rng.below(9) {
            0 => Term::int_add(sub(rng), sub(rng)),
            1 => Term::int_sub(sub(rng), sub(rng)),
            2 => Term::int_mul(sub(rng), sub(rng)),
            3 => Term::int_neg(sub(rng)),
            4 => Term::call(Term::Fn(world.int_double), vec![sub(rng)]),
            5 => Term::int_div(sub(rng), sub(rng)),
            6 => Term::int_rem(sub(rng), sub(rng)),
            7 => {
                let (ty, value) = machine(rng, world, depth - 1);
                Term::view(ty, value)
            }
            _ => {
                let (no, yes) = (sub(rng), sub(rng));
                Term::case(
                    Term::cmp(
                        CmpOp::Lt,
                        MachineInt::U8,
                        byte(rng, world, 1),
                        byte(rng, world, 1),
                    ),
                    Type::Int,
                    vec![
                        (0, Box::new(move |_, _| no)),
                        (0, Box::new(move |_, _| yes)),
                    ],
                )
            }
        }
    }
    let (mut valued, mut overflowed) = (0, 0);
    for _ in 0..300 {
        let term = integer(&mut rng, &world, 3);
        let Ok(Term::Eq(_, _, value)) = infer_proof(&mut ctx, &Proof::Evaluate(term.clone()))
        else {
            panic!("the kernel does not evaluate {term}");
        };
        let Term::Int(expected) = *value else {
            panic!("{term} evaluates to {value}");
        };
        let mut oracle = Oracle::new(&scene);
        match oracle.value(&term) {
            Some(found) => {
                valued += 1;
                let expected = expected.to_i128().expect("the oracle's value is in range");
                assert_eq!(found, Value::Int(expected), "{term}");
            }
            None => overflowed += 1,
        }
    }
    assert!(valued > 100, "{valued} terms had a value");
    assert!(overflowed > 0, "{overflowed} terms overflowed i128");

    // And for the machine types: a closed term of a machine type is a
    // literal at an edge of its range, a wrap of an integer, a cast of a
    // value of another type, or a round trip. The kernel's value is a
    // literal of the type; the oracle's is the same number, or nothing when
    // an integer inside went past `i128`.
    fn machine(rng: &mut Rng, world: &World, depth: usize) -> (MachineInt, Term) {
        let ty = *rng.pick(&MachineInt::FIXED).unwrap();
        if depth == 0 {
            let sample = machine_sample(ty);
            return (ty, lit(ty, *rng.pick(&sample).unwrap()));
        }
        match rng.below(4) {
            0 => (ty, Term::wrap(ty, integer(rng, world, depth - 1))),
            1 => {
                let (from, value) = machine(rng, world, depth - 1);
                (ty, Term::cast(from, ty, value))
            }
            2 => {
                let (inner, value) = machine(rng, world, depth - 1);
                (inner, Term::wrap(inner, Term::view(inner, value)))
            }
            // A comparison at a machine type chooses between two literals.
            _ => {
                let (inner, left) = machine(rng, world, depth - 1);
                let right = Term::wrap(inner, integer(rng, world, depth - 1));
                let op = *rng.pick(&CmpOp::ALL).unwrap();
                let sample = machine_sample(ty);
                let (no, yes) = (
                    lit(ty, *rng.pick(&sample).unwrap()),
                    lit(ty, *rng.pick(&sample).unwrap()),
                );
                let chosen = Term::case(
                    Term::cmp(op, inner, left, right),
                    Type::machine(ty),
                    vec![
                        (0, Box::new(move |_, _| no)),
                        (0, Box::new(move |_, _| yes)),
                    ],
                );
                (ty, chosen)
            }
        }
    }
    let (mut valued, mut overflowed) = (0, 0);
    let mut types_seen = Vec::new();
    for _ in 0..300 {
        let (ty, term) = machine(&mut rng, &world, 4);
        let Ok(Term::Eq(found_ty, _, value)) =
            infer_proof(&mut ctx, &Proof::Evaluate(term.clone()))
        else {
            panic!("the kernel does not evaluate {term}");
        };
        assert_eq!(found_ty, Type::machine(ty), "{term}");
        let Some((found, expected)) = value.machine_value() else {
            panic!("{term} evaluates to {value}");
        };
        assert_eq!(found, ty);
        let expected = expected.to_i128().expect("a machine value fits in i128");
        let mut oracle = Oracle::new(&scene);
        match oracle.value(&term) {
            Some(found) => {
                valued += 1;
                types_seen.push(ty);
                assert_eq!(found, machine_of(ty, expected), "{term}");
            }
            None => overflowed += 1,
        }
    }
    assert!(valued > 100, "{valued} machine terms had a value");
    assert!(overflowed > 0, "{overflowed} machine terms overflowed i128");
    for ty in MachineInt::FIXED {
        assert!(
            types_seen.contains(&ty),
            "no term of {} had a value",
            ty.name()
        );
    }
}

/// The oracle's table of the machine types, and its reduction into a range,
/// were written from the names of the types. They must agree with the
/// kernel's `MachineInt` on the ends of every range and on `wrap` at a
/// sample of arguments, or one of the two is wrong. A cross-check only:
/// nothing in the oracle calls the kernel's table.
#[test]
fn the_oracle_reduces_into_a_range_as_the_kernel_table_does() {
    let mut rng = Rng(SEED ^ 4);
    let mut compared = 0;
    for ty in MachineInt::FIXED {
        let (lo, hi) = machine_range(ty);
        assert_eq!(Integer::from(lo), ty.min(), "{}", ty.name());
        assert_eq!(Integer::from(hi), ty.max(), "{}", ty.name());
        assert_eq!(shape(ty), (ty.bits(), ty.signed()));
        let period = 1i128 << shape(ty).0;
        let mut arguments = machine_sample(ty);
        arguments.extend([lo - 2, lo - 1, hi + 1, hi + 2]);
        for shift in [period, -period, 2 * period, -2 * period] {
            arguments.extend([shift - 1, shift, shift + 1]);
        }
        arguments.extend((0..24).map(|_| {
            let wide = i128::from(rng.next()) << rng.below(64);
            if rng.below(2) == 0 { wide } else { -wide }
        }));
        arguments.extend([i128::MIN, i128::MAX]);
        for argument in arguments {
            let reduced = machine_wrap(ty, argument);
            assert!(
                in_range(ty, reduced),
                "wrap[{}]({argument}) = {reduced}",
                ty.name()
            );
            assert_eq!(
                Integer::from(reduced),
                ty.wrap(&Integer::from(argument)),
                "wrap[{}]({argument})",
                ty.name()
            );
            compared += 1;
        }
    }
    assert!(compared > 8 * 40, "{compared}");
}

/// The oracle reads the order of the views by what it means. The kernel
/// relates it to the runtime comparisons by reflection. For every pair of
/// bytes tried, what the oracle says is what the kernel proves, and the
/// kernel does not prove the opposite.
#[test]
fn the_oracle_reads_the_orderings_as_the_kernel_does() {
    let world = world();
    let prelude = &world.prelude;
    let scene = Scene::new(&world.definitions);
    let mut ctx = scene.ctx.clone();
    let edge = [0u8, 1, 2, 3, 127, 128, 254, 255];
    for left in edge {
        for right in edge {
            let (a, b) = (Term::U8(left), Term::U8(right));
            let orderings = [
                (
                    CmpOp::Le,
                    Term::int_le(
                        Term::view(MachineInt::U8, a.clone()),
                        Term::view(MachineInt::U8, b.clone()),
                    ),
                    left <= right,
                ),
                (
                    CmpOp::Lt,
                    Term::int_lt(
                        Term::view(MachineInt::U8, a.clone()),
                        Term::view(MachineInt::U8, b.clone()),
                    ),
                    left < right,
                ),
            ];
            for (op, claim, holds) in orderings {
                let mut oracle = Oracle::new(&scene);
                assert_eq!(oracle.prop(&claim), Some(holds), "{claim}");
                let comparison = Term::cmp(op, MachineInt::U8, a.clone(), b.clone());
                let by_reflection = |flag: bool| {
                    Proof::implies_elim(
                        Proof::Axiom(Axiom::CmpReflect(comparison.clone(), flag)),
                        Proof::Evaluate(comparison.clone()),
                    )
                };
                let (proved, refuted) = (claim.clone(), prelude.not_prop(claim.clone()));
                let (truth, falsehood) = if holds {
                    (proved, refuted)
                } else {
                    (refuted, proved)
                };
                assert_eq!(check_proof(&mut ctx, &by_reflection(holds), &truth), Ok(()));
                assert!(check_proof(&mut ctx, &by_reflection(holds), &falsehood).is_err());
                assert!(check_proof(&mut ctx, &by_reflection(!holds), &falsehood).is_err());
            }
        }
    }
    // The order of the integers is a proposition that evaluation decides.
    // The oracle reads it as `<=` on `i128`; what it says is what the kernel
    // proves, for `<=`, for `<` written as `+ 1 <=`, and for `==`.
    for left in INT_SAMPLE {
        for right in INT_SAMPLE {
            let (a, b) = (
                Term::Int(Integer::from(left)),
                Term::Int(Integer::from(right)),
            );
            let comparisons = [
                (Term::int_le(a.clone(), b.clone()), left <= right),
                (Term::int_lt(a.clone(), b.clone()), left < right),
                (Term::eq(Type::Int, a.clone(), b.clone()), left == right),
            ];
            for (claim, holds) in comparisons {
                let mut oracle = Oracle::new(&scene);
                assert_eq!(oracle.prop(&claim), Some(holds), "{claim}");
                let decided = Proof::Evaluate(claim.clone());
                let (truth, falsehood) = if holds {
                    (claim.clone(), prelude.not_prop(claim.clone()))
                } else {
                    (prelude.not_prop(claim.clone()), claim.clone())
                };
                assert_eq!(check_proof(&mut ctx, &decided, &truth), Ok(()));
                assert!(check_proof(&mut ctx, &decided, &falsehood).is_err());
            }
        }
    }
}

/// The oracle must say "false" only of what is false. These are claims
/// whose truth is known on sight.
#[test]
fn the_oracle_decides_what_it_should_and_no_more() {
    let world = world();
    let prelude = &world.prelude;
    let mut scene = Scene::new(&world.definitions);
    let x = scene.declare(Type::U8);
    scene.assume(Term::int_le(
        Term::view(MachineInt::U8, x.clone()),
        Term::view(MachineInt::U8, Term::U8(3)),
    ));
    let found = witnesses(&scene, &prelude.truth_prop(), &mut Rng(SEED));
    assert!(!found.is_empty());
    let le = |bound: u8| {
        Term::int_le(
            Term::view(MachineInt::U8, x.clone()),
            Term::view(MachineInt::U8, Term::U8(bound)),
        )
    };
    // True of every witness: never refuted.
    for claim in [le(3), le(4), le(255), prelude.truth_prop()] {
        assert!(refute(&scene, &claim, &found).is_none(), "{claim}");
    }
    // False of some witness, and the witness satisfies the hypothesis.
    for claim in [
        le(2),
        le(0),
        prelude.falsehood_prop(),
        prelude.not_prop(le(3)),
    ] {
        let witness = refute(&scene, &claim, &found).unwrap_or_else(|| panic!("{claim}"));
        let Some(Value::U8(value)) = witness.values().next() else {
            panic!("a witness assigns x a byte");
        };
        assert!(*value <= 3);
    }
    // Undecidable here: a quantifier over all of Int that has no small
    // counterexample, a claim about an unknown proposition, and a
    // computation past `i128`, which the oracle does not do.
    let p = scene.declare(Type::Prop);
    let found = witnesses(&scene, &p, &mut Rng(SEED));
    let open = Term::forall(Type::Int, |n| {
        Term::int_le(n.clone(), Term::int_add(n, Term::int(1)))
    });
    let squares = Term::forall(Type::Int, |n| {
        Term::int_le(Term::int(0), Term::int_mul(n.clone(), n))
    });
    let huge = Term::Int(Integer::from(i128::MAX));
    let past = Term::eq(
        Type::Int,
        Term::int_add(huge.clone(), Term::int(1)),
        huge.clone(),
    );
    for claim in [open, squares, past, p] {
        assert!(refute(&scene, &claim, &found).is_none(), "{claim}");
    }
    // Decided from the sample: a universal over Int with a counterexample
    // in it, an existential with a witness in it, and a strict comparison.
    let negatives = Term::forall(Type::Int, |n| Term::int_le(Term::int(0), n));
    let some_negative = Term::exists(Type::Int, |n| Term::int_lt(n, Term::int(0)));
    let strict = Term::int_lt(Term::int(3), Term::int(3));
    assert!(refute(&scene, &negatives, &found).is_some());
    assert!(refute(&scene, &prelude.not_prop(some_negative), &found).is_some());
    assert!(refute(&scene, &strict, &found).is_some());
    // A variable of Int is tried at negative values too.
    let n = scene.declare(Type::Int);
    let found = witnesses(&scene, &n, &mut Rng(SEED));
    let nonneg = Term::int_le(Term::int(0), n.clone());
    let witness = refute(&scene, &nonneg, &found).expect("some witness makes n negative");
    assert!(matches!(witness.get(&n_id(&n)), Some(Value::Int(value)) if *value < 0));
    // A variable of a machine type is tried at both ends of its range and
    // around zero: the bounds of the type hold of every witness, a bound
    // off by one does not, and the round trip holds. An equation between
    // two machine types is ill typed, not false.
    let m = scene.declare(Type::Machine(I16));
    let found = witnesses(&scene, &m, &mut Rng(SEED));
    let v = Term::view(I16, m.clone());
    for claim in [
        Term::int_le(int128(-32768), v.clone()),
        Term::int_le(v.clone(), int128(32767)),
        Term::eq(Type::Machine(I16), Term::wrap(I16, v.clone()), m.clone()),
        Term::eq(
            Type::Machine(U16),
            Term::wrap(U16, Term::int(1)),
            Term::wrap(I16, Term::int(1)),
        ),
    ] {
        assert!(refute(&scene, &claim, &found).is_none(), "{claim}");
    }
    type Refuting = fn(i128) -> bool;
    let refuting: [(Term, Refuting); 4] = [
        (Term::int_le(int128(-32767), v.clone()), |value| {
            value == -32768
        }),
        (Term::int_le(v.clone(), int128(32766)), |value| {
            value == 32767
        }),
        (Term::int_le(Term::int(0), v.clone()), |value| value < 0),
        (Term::int_le(v.clone(), Term::int(0)), |value| value > 0),
    ];
    for (claim, expected) in refuting {
        let witness = refute(&scene, &claim, &found).unwrap_or_else(|| panic!("{claim}"));
        assert!(
            matches!(witness.get(&n_id(&m)), Some(Value::Machine(I16, value)) if expected(*value)),
            "{claim} at {}",
            describe_witness(witness)
        );
    }
    // A quantifier over a machine type is settled by its sample: refuted at
    // an end of the range, proved by a witness in it, and otherwise open.
    let all_nonneg = Term::forall(Type::Machine(I8), |x| {
        Term::int_le(Term::int(0), Term::view(I8, x))
    });
    let some_min = Term::exists(Type::Machine(U32), |x| {
        Term::eq(Type::Int, Term::view(U32, x), int128(u32::MAX.into()))
    });
    let round = Term::forall(Type::Machine(U64), |x| {
        Term::eq(
            Type::Machine(U64),
            Term::wrap(U64, Term::view(U64, x.clone())),
            x,
        )
    });
    assert!(refute(&scene, &all_nonneg, &found).is_some());
    assert!(refute(&scene, &prelude.not_prop(some_min), &found).is_some());
    assert!(refute(&scene, &round, &found).is_none());
    assert!(refute(&scene, &prelude.not_prop(round), &found).is_none());
    // An inconsistent context has no witness, so nothing is false in it.
    scene.assume(Term::int_lt(
        Term::view(MachineInt::U8, Term::U8(9)),
        Term::view(MachineInt::U8, x),
    ));
    assert!(witnesses(&scene, &prelude.falsehood_prop(), &mut Rng(SEED)).is_empty());
}

#[test]
fn no_hand_built_proof_or_mutant_of_one_is_accepted_for_a_false_claim() {
    let started = Instant::now();
    let world = world();
    let triples = hand_built(&world);
    let mut tally = Tally::default();
    attack(&triples, &mut tally, &mut Rng(SEED));
    tally.report("hand-built", started);
    // Every family must actually be attacked, or the test says nothing.
    assert_eq!(tally.no_witness, 0);
    assert_eq!(tally.attacked, tally.triples);
    conclude(&tally);
}

#[test]
fn no_theory_lemma_or_mutant_of_one_is_accepted_for_a_false_claim() {
    let started = Instant::now();
    let world = world();
    let triples = theory_triples(&world);
    let mut tally = Tally::default();
    attack(&triples, &mut tally, &mut Rng(SEED ^ 1));
    tally.report("theory", started);
    assert!(tally.attacked > 0);
    conclude(&tally);
}

#[test]
fn no_corpus_proof_or_mutant_of_one_is_accepted_for_a_false_claim() {
    let started = Instant::now();
    let corpus = corpus_triples();
    let mut tally = Tally::default();
    attack(&corpus.triples, &mut tally, &mut Rng(SEED ^ 2));
    tally.report("corpus", started);
    println!(
        "corpus: {} proofs found by the search, {} in the bodies of math functions, {} of \
         those with no claim the kernel would give",
        corpus.found, corpus.in_math_bodies, corpus.unplaced
    );
    assert!(corpus.found > 0);
    assert!(tally.attacked > 0);
    conclude(&tally);
}
