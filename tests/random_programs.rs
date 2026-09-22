//! Random well-typed programs, compared three ways (R1 in atlas.html).
//!
//! The two trusted translations from the typed tree, `lower` and `erase`,
//! are kept honest by running what they produce on the same programs: the
//! check IR in its interpreter, the erased tree in the reference
//! interpreter, and the printed Rust compiled by rustc, with overflow checks
//! on and off. The other tests do that on programs written by hand. This one
//! generates them.
//!
//! The generator is type directed and works on typed trees, not on source
//! text, because the translations start from the tree and the surface syntax
//! moves faster than the tree does. Given a type and the locals in scope it
//! produces an expression of that type, from a small set of productions with
//! weights (`PRODUCTIONS`); adding a construct to the fragment is a line in
//! that table and an arm in `fits` and `produce`. The fragment is the runtime
//! one: the eight machine integer types, `bool`, unit, tuples, a few structs
//! and enums per program, literals, locals, `let` with binding, tuple, and
//! wildcard patterns, field access, the wrapping methods, `as` between
//! machine types, comparisons at each type, `if` (which is also how `!`,
//! `&&`, and `||` appear in the tree), `match` with payload bindings, calls
//! to functions generated earlier (so the call graph is acyclic), `math fn`s
//! over the pure part of all this, the two loops, and since M2 `let mut`
//! with assignment, whole or by a field path, in straight-line code and in
//! the arms of branches. The generator plays the elaborator's part for
//! mutation: a mutable local's identity is updated in place to each new
//! version, an arm's versions are put back after it, and a branch some arm
//! of which assigned an outer binding records the join lowering will
//! compute, so the three-way comparison covers versions and joins. Where the tree needs
//! evidence it gets trivial evidence: the ordered bounds of a `for` are
//! literals, proved by evaluating the comparison, and the one proof type in
//! the fragment is `@[0 == 0]`, proved by reflexivity.
//!
//! Every loop terminates by construction, because interpreter fuel does not
//! bound the compiled program: a `loop` carries a counter in its state that
//! starts at zero, is never assigned in the body, and goes up by one on every
//! `continue`, with a `break` when it reaches a literal limit; a `for` runs
//! between two literals. Calls and loops are also charged against a budget
//! (`BUDGET`) so that no program does much work, which keeps the interpreters
//! far from running out of fuel. The process timeout on the compiled program
//! is a safeguard, and its firing is inconclusive, not a failure.
//!
//! Every generated function is declared through the real `Session`, so it is
//! lowered, checked by the exec checker and the kernel, and erased. A program
//! the checker rejects is a bug in the generator, not a finding about the
//! translations: rejections are counted, the first few are printed, and the
//! test fails if more than a small fraction are rejected, so the generator
//! cannot rot in silence.
//!
//! For each accepted program the functions whose parameters are all machine
//! integers are called on a few inputs, boundary values and random ones. The two
//! interpreters are compared as `tests/differential.rs` compares them: out of
//! fuel on either side is inconclusive. Then both are compared with the
//! compiled Rust, in both builds, through the harness of `tests/corpus.rs`,
//! copied into `tests/common/compiled.rs`: two hundred programs go into one
//! Rust source, one `mod` each, one rustc call per build; the program answers
//! one line per call, flushed, under `catch_unwind`, and is restarted past a
//! call that hangs. Inconclusive cases are counted and printed.
//!
//! A disagreement is printed with the program's seed, its Rust, its typed
//! tree when short, the input, and the outcomes, and then shrunk: statements
//! are deleted, expressions replaced by literals of their type or by one of
//! their own subexpressions, `let`s of literals inlined, one branch of a
//! conditional taken, keeping a change when the program still checks and
//! still disagrees, until nothing helps. The shrinker and the report are
//! exercised below on a planted disagreement, so they are known to work.
//!
//! The fast form runs `FAST_COUNT` programs from a fixed seed. With
//! `LOCUS_EXTENDED` set it runs `EXTENDED_COUNT` from a seed taken from the
//! clock, printed at the start, and fails if more than half a percent of the
//! cases are inconclusive. `LOCUS_SEED` fixes the run's seed in either mode;
//! `LOCUS_PROGRAM` replays one program by the seed a report printed.

mod common;
#[path = "common/compiled.rs"]
mod compiled;
#[path = "common/rng.rs"]
mod rng;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant, SystemTime};

use common::setup;
use compiled::{
    Answered, Overflow, Unit, compile, harness, observe, one_line, remove_binaries, rust_value,
};
use locus::erased::{Interpreter, Module, Outcome, RunError, Value, check_module, print_module};
use locus::exec::CheckInterpreter;
use locus::kernel::{
    Axiom, CmpOp, EnumId, HypId, MachineInt, Op, Prim, Proof, StructId, Term, Type, VarId,
    same_type,
};
use locus::typed::{
    Binder, Block, CompareOp, EnumItem, Expr, FnItem, FnRef, Join, Joined, MatchArm, Pattern,
    Place, Session, Step as PathStep, Stmt, StructItem, VariantItem,
};
use rng::{Rng, case_seed};

/// Steps an interpreter may take on one call. Far above what `BUDGET`
/// allows a program, so out of fuel means the budget was wrong.
const FUEL: u64 = 10_000_000;

/// How long the compiled program may go without answering before it is
/// killed and the call it was in is inconclusive.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Programs per rustc call.
const BATCH: usize = 200;

const FAST_COUNT: u64 = 300;
const FAST_SEED: u64 = 0x5EED_0000_0000_0001;
const EXTENDED_COUNT: u64 = 10_000;

/// The share of generated programs the checker may reject before the
/// generator counts as broken.
const MAX_REJECTED: f64 = 0.02;

/// The share of cases that may be inconclusive in the extended run.
const MAX_INCONCLUSIVE: f64 = 0.005;

/// How much work one function may do, in steps of the interpreters, counting
/// every branch as taken and every loop as running to its limit: what a call
/// costs its caller.
const BUDGET: u64 = 4_000;

/// How deeply expressions nest.
const MAX_DEPTH: usize = 3;

/// Inputs per entry function: boundary values mixed with random ones.
const INPUTS: usize = 6;

/// The boundary values of a machine type: the ends of its range and their
/// neighbours, zero and its neighbours, and the middle of an unsigned range,
/// each within the range.
fn boundary(ty: MachineInt) -> Vec<i128> {
    let (min, max) = (number(ty.min()), number(ty.max()));
    let half = (max + 1) / 2;
    let mut values: Vec<i128> = vec![
        min,
        min + 1,
        -1,
        0,
        1,
        2,
        3,
        7,
        half - 1,
        half,
        max - 1,
        max,
    ];
    values.retain(|value| (min..=max).contains(value));
    values.dedup();
    values
}

fn number(value: locus::kernel::Integer) -> i128 {
    value
        .to_i128()
        .expect("a bound of a machine type fits an i128")
}

/// A random value of the type: a boundary value half the time, else one
/// drawn from the whole range.
fn random_value(rng: &mut Rng, ty: MachineInt) -> i128 {
    if rng.chance(1, 2) {
        return *rng.choose(&boundary(ty));
    }
    let (min, max) = (number(ty.min()), number(ty.max()));
    let width = (max - min + 1) as u128;
    let offset = if width > u128::from(u64::MAX) {
        u128::from(rng.next_u64())
    } else {
        u128::from(rng.below(width as u64))
    };
    min + offset as i128
}

/// How many disagreements are shrunk and reported in full.
const SHRUNK: usize = 3;

/// How many candidates the shrinker may try, when each costs a rustc call.
const SHRINK_STEPS_WITH_RUSTC: usize = 150;

// --- The program ------------------------------------------------------------------

/// A generated program: what was declared, with the identities the session
/// gave each item, which the trees of later items refer to. Declaring the
/// same items in the same order into a copy of the base session gives the
/// same identities, and `declare` checks that it did.
#[derive(Clone, Debug)]
struct Program {
    seed: u64,
    structs: Vec<(StructId, StructItem)>,
    enums: Vec<(EnumId, EnumItem)>,
    fns: Vec<Function>,
}

#[derive(Clone, Debug)]
struct Function {
    item: FnItem,
    reference: FnRef,
    /// What a call costs, against `BUDGET`.
    cost: u64,
}

impl Program {
    fn struct_item(&self, id: StructId) -> &StructItem {
        &self
            .structs
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .expect("a struct of this program")
            .1
    }

    fn enum_item(&self, id: EnumId) -> &EnumItem {
        &self
            .enums
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .expect("an enum of this program")
            .1
    }

    /// The functions that can be called on bytes alone, among those with a
    /// runtime form: a `math fn` that returns a proof is not emitted, so
    /// there is nothing to run.
    fn entries(&self, module: &Module) -> Vec<usize> {
        (0..self.fns.len())
            .filter(|&index| {
                let function = &self.fns[index];
                function
                    .item
                    .params
                    .iter()
                    .all(|param| param.ty.as_machine().is_some())
                    && module
                        .fns
                        .iter()
                        .any(|emitted| emitted.reference == function.reference)
            })
            .collect()
    }

    /// Declares the program again into a copy of `base`, checking that every
    /// item receives the identity it had.
    fn declare(&self, base: &Session) -> Result<Session, String> {
        let mut session = base.clone();
        for (id, item) in &self.structs {
            let given = session
                .declare_struct(item)
                .map_err(|error| format!("struct {}: {error}", item.name))?;
            assert_eq!(
                given, *id,
                "the session numbered struct {} differently",
                item.name
            );
        }
        for (id, item) in &self.enums {
            let given = session
                .declare_enum(item)
                .map_err(|error| format!("enum {}: {error}", item.name))?;
            assert_eq!(
                given, *id,
                "the session numbered enum {} differently",
                item.name
            );
        }
        for function in &self.fns {
            let given = session
                .declare_fn(&function.item)
                .map_err(|error| format!("fn {}: {error}", function.item.name))?;
            assert_eq!(
                given, function.reference,
                "the session numbered fn {} differently",
                function.item.name
            );
        }
        check_module(session.erased()).map_err(|error| format!("erased tree: {error}"))?;
        Ok(session)
    }

    /// The number of expressions and statements, which shrinking reduces.
    fn size(&self) -> usize {
        let mut count = 0;
        let mut counting = self.clone();
        counting.visit_exprs(&mut |_, _, _| {
            count += 1;
            false
        });
        counting.visit_blocks(&mut |block| {
            count += block.stmts.len();
            false
        });
        count
    }
}

/// The unit type.
fn unit() -> Type {
    Type::Tuple(Vec::new())
}

fn is_unit(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(fields) if fields.is_empty())
}

/// The one proof type of the fragment, `@[0 == 0]`, and its proof.
fn evidence_type() -> Type {
    Type::proof(Term::eq(Type::U8, Term::U8(0), Term::U8(0)))
}

fn evidence() -> Expr {
    Expr::Proof(Proof::Refl(Term::U8(0)))
}

/// `lo <= hi` for two literals, by evaluating the comparison and reflecting
/// the result: the trivial evidence a `for` over literal bounds needs.
fn ordered(ty: MachineInt, lo: i128, hi: i128) -> Proof {
    let comparison = Term::cmp(
        CmpOp::Le,
        ty,
        Term::machine_int(ty, lo),
        Term::machine_int(ty, hi),
    );
    Proof::implies_elim(
        Proof::Axiom(Axiom::CmpReflect(comparison.clone(), true)),
        Proof::Evaluate(comparison),
    )
}

fn if_(
    condition: Expr,
    then_block: Block,
    else_block: Block,
    ty: &Type,
    joined: Option<Joined>,
) -> Expr {
    Expr::If {
        condition: Box::new(condition),
        then_fact: HypId::fresh(),
        else_fact: HypId::fresh(),
        then_block,
        else_block,
        ty: ty.clone(),
        result: VarId::fresh(),
        joined,
    }
}

fn tail_block(tail: Expr) -> Block {
    Block {
        stmts: Vec::new(),
        tail: Some(Box::new(tail)),
    }
}

fn compare(op: CompareOp, ty: MachineInt, left: Expr, right: Expr) -> Expr {
    Expr::Compare {
        op,
        ty: Type::machine(ty),
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn plus_one(value: Expr) -> Expr {
    Expr::Method {
        prim: Prim::Op(Op::WrappingAdd, MachineInt::U8),
        receiver: Box::new(value),
        arguments: vec![Expr::u8(1)],
    }
}

fn let_(binder: &Binder, value: Expr) -> Stmt {
    Stmt::Let {
        pattern: Pattern::Bind {
            binder: binder.clone(),
            equation: HypId::fresh(),
            mutable: false,
        },
        value,
    }
}

// --- Rust's inference and lints, which the printed program must satisfy ---------------

/// Whether Rust can tell the expression's type from the expression alone.
/// Since E5 the printer suffixes every literal, so every expression of the
/// fragment is determined, and this is kept only as the place the rule
/// lives should a form without a type of its own return. It was: a bare
/// literal is `{integer}` until something fixes it, and calling
/// `wrapping_add` on one is an error, so the generator never lets a `let` of
/// an undetermined value be a receiver.
fn determined(expr: &Expr, locals: &HashMap<VarId, bool>) -> bool {
    let block = |block: &Block| {
        block
            .tail
            .as_deref()
            .is_none_or(|tail| determined(tail, locals))
    };
    match expr {
        Expr::Literal(..) | Expr::Int(_) => true,
        Expr::Var { id, .. } => locals.get(id).copied().unwrap_or(true),
        Expr::Tuple { fields, .. } => fields.iter().all(|field| determined(field, locals)),
        Expr::Field { target, .. } => determined(target, locals),
        Expr::If {
            then_block,
            else_block,
            ..
        } => block(then_block) && block(else_block),
        Expr::Match { arms, .. } => arms.iter().all(|arm| block(&arm.body)),
        Expr::Block(inner) => block(inner),
        Expr::Loop { body, .. } => breaks_determined(body, locals),
        Expr::Bool(_)
        | Expr::Struct { .. }
        | Expr::Variant { .. }
        | Expr::Method { .. }
        | Expr::Compare { .. }
        | Expr::Cast { .. }
        | Expr::CallMath { .. }
        | Expr::CallFn { .. }
        | Expr::For { .. }
        | Expr::Break(_)
        | Expr::Continue(_)
        | Expr::Proof(_)
        | Expr::Prop(_)
        | Expr::Absurd { .. } => true,
    }
}

/// A loop's value is what its breaks give it. A break ends a block on the
/// tail chain of the body; a nested loop's breaks are its own.
fn breaks_determined(block: &Block, locals: &HashMap<VarId, bool>) -> bool {
    match block.tail.as_deref() {
        Some(Expr::Break(value)) => determined(value, locals),
        Some(Expr::If {
            then_block,
            else_block,
            ..
        }) => breaks_determined(then_block, locals) && breaks_determined(else_block, locals),
        Some(Expr::Match { arms, .. }) => {
            arms.iter().all(|arm| breaks_determined(&arm.body, locals))
        }
        Some(Expr::Block(inner)) => breaks_determined(inner, locals),
        _ => true,
    }
}

/// Whether the printed expression begins with a struct literal, which Rust
/// does not allow at the head of a condition or a scrutinee.
fn starts_with_struct_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Struct { .. } => true,
        Expr::Field { target, .. } => starts_with_struct_literal(target),
        Expr::Method { receiver, .. } => starts_with_struct_literal(receiver),
        Expr::Compare { left, .. } => starts_with_struct_literal(left),
        Expr::Cast { expr, .. } => starts_with_struct_literal(expr),
        _ => false,
    }
}

/// The expression in a position that Rust parses restrictively: braced when
/// it has to be.
fn guarded(expr: Expr) -> Expr {
    if starts_with_struct_literal(&expr) {
        Expr::Block(tail_block(expr))
    } else {
        expr
    }
}

/// Whether the expression is a closed literal: something a `let` can be
/// inlined as, and something replacing it by a literal cannot shrink.
fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Bool(_) | Expr::Literal(..) | Expr::Proof(_) => true,
        Expr::Tuple { fields, .. }
        | Expr::Variant {
            payload: fields, ..
        } => fields.iter().all(is_literal),
        Expr::Struct { fields, .. } => fields.iter().all(|(_, field)| is_literal(field)),
        _ => false,
    }
}

// --- The generator -----------------------------------------------------------------

/// The productions of an expression. Each has a weight, a condition under
/// which it fits a type at a depth (`fits`), and a way of being produced
/// (`produce`). A production that finds nothing to work with, such as a call
/// with no function of the type, falls back to a leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Production {
    /// A literal of the type: a machine integer, a bool, the evidence, or a
    /// tuple, struct, or variant of generated fields.
    Literal,
    /// A local of the type.
    Var,
    /// A field of a local, or of a generated tuple, that has the type.
    Field,
    /// A wrapping method at the type: `wrapping_add`, `wrapping_sub`,
    /// `wrapping_mul`, or, at a signed type, `wrapping_neg`.
    Method,
    /// `as` into the type from a random machine type.
    Cast,
    /// A comparison of two values of one machine type.
    Compare,
    If,
    /// `a && b` or `a || b`, which the tree spells as an `if`.
    ShortCircuit,
    /// `!a`, likewise.
    Not,
    /// A match on an enum of the program, binding each variant's payload.
    Match,
    /// A call to a function generated earlier.
    Call,
    /// A state-passing `loop`, bounded by a counter in its state.
    Loop,
    /// A `for` between two literals, whose value is its final state.
    For,
    /// A block with statements of its own.
    Block,
}

/// The productions and their weights.
const PRODUCTIONS: &[(Production, u32)] = &[
    (Production::Literal, 4),
    (Production::Var, 10),
    (Production::Field, 3),
    (Production::Method, 5),
    (Production::Cast, 3),
    (Production::Compare, 5),
    (Production::If, 4),
    (Production::ShortCircuit, 2),
    (Production::Not, 1),
    (Production::Match, 3),
    (Production::Call, 5),
    (Production::Loop, 2),
    (Production::For, 2),
    (Production::Block, 2),
];

/// What may stand as a statement of its own: the forms Rust does not lint as
/// an unused value or a path with no effect.
const STATEMENTS: &[(Production, u32)] = &[
    (Production::If, 3),
    (Production::Match, 2),
    (Production::Call, 4),
    (Production::Loop, 2),
    (Production::For, 2),
];

#[derive(Clone, Copy, Debug)]
enum StmtKind {
    /// `let v = value;`
    Bind,
    /// `let (v, w) = value;`
    Destructure,
    /// `let _ = value;`
    Discard,
    /// A call, a conditional, or a loop for its own sake.
    Effect,
    /// `let mut v = value;`
    LetMut,
    /// `v = value;` or `v.f.0 = value;`, to a mutable local in scope.
    Assign,
}

const STMTS: &[(StmtKind, u32)] = &[
    (StmtKind::Bind, 6),
    (StmtKind::Destructure, 1),
    (StmtKind::Discard, 1),
    (StmtKind::Effect, 2),
    (StmtKind::LetMut, 3),
    (StmtKind::Assign, 5),
];

#[derive(Clone, Copy, Debug)]
enum TypeKind {
    Machine,
    Bool,
    Unit,
    Evidence,
    Tuple,
    Struct,
    Enum,
}

/// Why a generated program was not accepted: a function the checker rejected,
/// which is a bug in the generator.
#[derive(Clone, Debug)]
struct Rejection {
    seed: u64,
    function: String,
    error: String,
}

struct Generator {
    rng: Rng,
    session: Session,
    program: Program,
    // The function being generated.
    locals: Vec<Binder>,
    /// Beside each local: for a mutable one, the identity of its binding and
    /// the loop depth it was declared at. The local's own `id` is its
    /// current version.
    bindings: Vec<Option<(VarId, usize)>>,
    /// How many loop bodies enclose the current position: a mutable binding
    /// declared outside a loop body is not assigned inside it (M3).
    loop_depth: usize,
    /// Whether each local's Rust type is determined; see `determined`.
    known: HashMap<VarId, bool>,
    /// How many times the current position runs per call, from the loops
    /// around it.
    multiplier: u64,
    /// The function's cost so far.
    cost: u64,
    names: usize,
    /// Whether the function is a `math fn`: only the pure productions.
    math: bool,
}

impl Generator {
    /// Generates a program from a seed, declaring each item as it is made.
    fn generate(seed: u64, base: &Session) -> Result<(Program, Session), Rejection> {
        let mut generator = Self {
            rng: Rng::new(seed),
            session: base.clone(),
            program: Program {
                seed,
                structs: Vec::new(),
                enums: Vec::new(),
                fns: Vec::new(),
            },
            locals: Vec::new(),
            bindings: Vec::new(),
            loop_depth: 0,
            known: HashMap::new(),
            multiplier: 1,
            cost: 0,
            names: 0,
            math: false,
        };
        let structs = generator.rng.range(0..3);
        for index in 0..structs {
            generator.struct_item(index);
        }
        let enums = generator.rng.range(0..3);
        for index in 0..enums {
            generator.enum_item(index);
        }
        let count = generator.rng.range(2..6);
        for index in 0..count {
            let last = index + 1 == count;
            generator.function(index, last)?;
        }
        Ok((generator.program, generator.session))
    }

    fn weighted<T: Copy>(&mut self, choices: &[(T, u32)]) -> Option<T> {
        let total: u32 = choices.iter().map(|(_, weight)| weight).sum();
        if total == 0 {
            return None;
        }
        let mut at = self.rng.below(u64::from(total)) as u32;
        for (item, weight) in choices {
            if at < *weight {
                return Some(*item);
            }
            at -= weight;
        }
        None
    }

    /// A random machine type, `u8` a third of the time so that the byte
    /// programs of the fragment's first form stay common.
    fn machine_type(&mut self) -> MachineInt {
        if self.rng.chance(1, 3) {
            MachineInt::U8
        } else {
            *self.rng.choose(&MachineInt::ALL)
        }
    }

    fn value_of(&mut self, ty: MachineInt) -> i128 {
        random_value(&mut self.rng, ty)
    }

    // --- Types and declarations ---

    fn random_type(&mut self, depth: usize) -> Type {
        let mut choices = vec![
            (TypeKind::Machine, 8),
            (TypeKind::Bool, 4),
            (TypeKind::Unit, 1),
            (TypeKind::Evidence, 1),
        ];
        if depth > 0 {
            choices.push((TypeKind::Tuple, 3));
        }
        if !self.program.structs.is_empty() {
            choices.push((TypeKind::Struct, 2));
        }
        if !self.program.enums.is_empty() {
            choices.push((TypeKind::Enum, 2));
        }
        match self.weighted(&choices).expect("the choices have weight") {
            TypeKind::Machine => Type::machine(self.machine_type()),
            TypeKind::Bool => Type::Bool,
            TypeKind::Unit => unit(),
            TypeKind::Evidence => evidence_type(),
            TypeKind::Tuple => {
                let count = self.rng.range(1..4);
                Type::Tuple((0..count).map(|_| self.random_type(depth - 1)).collect())
            }
            TypeKind::Struct => Type::Struct(self.rng.choose(&self.program.structs).0),
            TypeKind::Enum => Type::Enum(self.rng.choose(&self.program.enums).0),
        }
    }

    /// A struct with a field at least: `Value::debug` prints an empty struct
    /// with braces and Rust's `{:?}` does not.
    fn struct_item(&mut self, index: usize) {
        let count = self.rng.range(1..4);
        let fields = (0..count)
            .map(|field| Binder::new(&format!("f{field}"), self.random_type(1)))
            .collect();
        let item = StructItem {
            name: format!("S{index}"),
            fields,
        };
        let id = self
            .session
            .declare_struct(&item)
            .expect("a struct over the fragment's types is accepted");
        self.program.structs.push((id, item));
    }

    fn enum_item(&mut self, index: usize) {
        let count = self.rng.range(1..4);
        let variants = (0..count)
            .map(|variant| {
                let fields = self.rng.range(0..3);
                VariantItem {
                    name: format!("V{variant}"),
                    payload: (0..fields)
                        .map(|field| Binder::new(&format!("p{field}"), self.random_type(1)))
                        .collect(),
                }
            })
            .collect();
        let item = EnumItem {
            name: format!("E{index}"),
            variants,
        };
        let id = self
            .session
            .declare_enum(&item)
            .expect("an enum over the fragment's types is accepted");
        self.program.enums.push((id, item));
    }

    /// Generates and declares one function. The last one takes bytes only,
    /// so that every program has an entry, and is an ordinary function, so
    /// that it can call everything before it.
    fn function(&mut self, index: usize, last: bool) -> Result<(), Rejection> {
        self.locals.clear();
        self.bindings.clear();
        self.loop_depth = 0;
        self.known.clear();
        self.multiplier = 1;
        self.cost = 0;
        self.names = 0;
        self.math = !last && self.rng.chance(1, 4);
        let arity = if last {
            self.rng.range(1..4)
        } else {
            self.rng.range(0..4)
        };
        let params: Vec<Binder> = (0..arity)
            .map(|_| {
                let ty = if last {
                    Type::machine(self.machine_type())
                } else {
                    self.random_type(1)
                };
                self.fresh(ty)
            })
            .collect();
        for param in &params {
            self.bind(param.clone(), true);
        }
        let result = self.random_type(1);
        let body = self.block(&result, MAX_DEPTH, false);
        let item = FnItem {
            name: format!("f{index}"),
            math: self.math,
            params,
            result,
            body,
        };
        match self.session.declare_fn(&item) {
            Ok(reference) => {
                self.program.fns.push(Function {
                    item,
                    reference,
                    cost: self.cost.max(1),
                });
                Ok(())
            }
            Err(error) => Err(Rejection {
                seed: self.program.seed,
                function: format!("{}: {item:?}", item.name),
                error: error.to_string(),
            }),
        }
    }

    // --- Scope ---

    /// A binder with a name no other binder of the function has, so that
    /// nothing shadows anything.
    fn fresh(&mut self, ty: Type) -> Binder {
        let name = format!("v{}", self.names);
        self.names += 1;
        Binder::new(&name, ty)
    }

    fn bind(&mut self, binder: Binder, determined: bool) {
        self.known.insert(binder.id, determined);
        self.locals.push(binder);
        self.bindings.push(None);
    }

    fn bind_mutable(&mut self, binder: Binder, determined: bool) {
        self.known.insert(binder.id, determined);
        self.bindings.push(Some((binder.id, self.loop_depth)));
        self.locals.push(binder);
    }

    /// Ends a scope: the locals declared in it go away.
    fn truncate(&mut self, scope: usize) {
        self.locals.truncate(scope);
        self.bindings.truncate(scope);
    }

    // --- Mutation: versions and joins, as the elaborator keeps them ---

    /// The mutable locals in scope: each one's slot, binding, and current
    /// version. Taken at the entry of a branch.
    fn entry(&self) -> Vec<(usize, VarId, VarId)> {
        self.bindings
            .iter()
            .enumerate()
            .filter_map(|(slot, binding)| {
                binding.map(|(binding, _)| (slot, binding, self.locals[slot].id))
            })
            .collect()
    }

    /// The versions the entry bindings have now: at the end of an arm.
    fn versions(&self, entry: &[(usize, VarId, VarId)]) -> Vec<VarId> {
        entry
            .iter()
            .map(|(slot, _, _)| self.locals[*slot].id)
            .collect()
    }

    /// Puts the entry versions back after an arm.
    fn restore(&mut self, entry: &[(usize, VarId, VarId)]) {
        for (slot, _, version) in entry {
            self.locals[*slot].id = *version;
        }
    }

    /// The join of a branch whose arms ended with the given versions: every
    /// binding some arm assigned gets a version for after the branch, in
    /// declaration order, which is the set and order lowering computes.
    fn join(&mut self, entry: &[(usize, VarId, VarId)], ends: &[Vec<VarId>]) -> Option<Joined> {
        let assigned: Vec<usize> = (0..entry.len())
            .filter(|&k| ends.iter().any(|end| end[k] != entry[k].2))
            .collect();
        if assigned.is_empty() {
            return None;
        }
        let joins = assigned
            .iter()
            .map(|&k| {
                let (slot, binding, _) = entry[k];
                let version = Binder {
                    id: VarId::fresh(),
                    name: self.locals[slot].name.clone(),
                    ty: self.locals[slot].ty.clone(),
                };
                let determined = self.known[&self.locals[slot].id];
                self.known.insert(version.id, determined);
                self.locals[slot].id = version.id;
                Join {
                    binding,
                    version,
                    equation: HypId::fresh(),
                }
            })
            .collect();
        Some(Joined {
            tuple: VarId::fresh(),
            joins,
            equation: HypId::fresh(),
        })
    }

    /// `if condition { then } else { otherwise }`, each arm generated by its
    /// closure on the entry versions, with the join of what they assigned.
    fn branch(
        &mut self,
        condition: Expr,
        ty: &Type,
        then: impl FnOnce(&mut Self) -> Block,
        otherwise: impl FnOnce(&mut Self) -> Block,
    ) -> Expr {
        let entry = self.entry();
        let then_block = then(self);
        let then_end = self.versions(&entry);
        self.restore(&entry);
        let else_block = otherwise(self);
        let else_end = self.versions(&entry);
        self.restore(&entry);
        let joined = self.join(&entry, &[then_end, else_end]);
        if_(condition, then_block, else_block, ty, joined)
    }

    /// `let mut v = value;`, of a type with a runtime form: tracked evidence
    /// is M4's.
    fn let_mut(&mut self, depth: usize) -> Stmt {
        let ty = self.random_type(1);
        if ty.is_ghost() {
            return self.binding(depth);
        }
        let value = self.expr(&ty, depth, false);
        let determined = determined(&value, &self.known);
        let binder = self.fresh(ty);
        self.bind_mutable(binder.clone(), determined);
        Stmt::Let {
            pattern: Pattern::Bind {
                binder,
                equation: HypId::fresh(),
                mutable: true,
            },
            value,
        }
    }

    /// An assignment to a mutable local declared at this loop depth, whole
    /// or by a path into its products, or `None` when there is none. Not in
    /// a `math fn`, whose body is pure.
    fn assign(&mut self, depth: usize) -> Option<Stmt> {
        if self.math {
            return None;
        }
        let candidates: Vec<usize> = (0..self.locals.len())
            .filter(|&slot| matches!(self.bindings[slot], Some((_, at)) if at == self.loop_depth))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let slot = *self.rng.choose(&candidates);
        let (binding, _) = self.bindings[slot].expect("a mutable local");
        let root = self.locals[slot].clone();
        let mut path = Vec::new();
        let mut ty = root.ty.clone();
        while self.rng.chance(1, 2) {
            let step = match &ty {
                Type::Tuple(fields) if !fields.is_empty() => {
                    let index = self.rng.range(0..fields.len());
                    let step = PathStep {
                        index,
                        name: None,
                        ty: ty.clone(),
                        proof_fields: fields
                            .iter()
                            .map(|field| matches!(field, Type::Proof(_)))
                            .collect(),
                    };
                    ty = fields[index].clone();
                    step
                }
                Type::Struct(id) => {
                    let item = self.program.struct_item(*id).clone();
                    let index = self.rng.range(0..item.fields.len());
                    let step = PathStep {
                        index,
                        name: Some(item.fields[index].name.clone()),
                        ty: ty.clone(),
                        proof_fields: item
                            .fields
                            .iter()
                            .map(|field| matches!(field.ty, Type::Proof(_)))
                            .collect(),
                    };
                    ty = item.fields[index].ty.clone();
                    step
                }
                _ => break,
            };
            path.push(step);
        }
        // The right side may assign the binding itself; the version it
        // leaves is what the place is rebuilt from, and this assignment
        // makes the next one.
        let value = self.expr(&ty, depth, false);
        let version = Binder {
            id: VarId::fresh(),
            name: root.name.clone(),
            ty: root.ty.clone(),
        };
        let determined = self.known[&self.locals[slot].id];
        self.known.insert(version.id, determined);
        self.locals[slot].id = version.id;
        Some(Stmt::Assign {
            place: Place {
                binding,
                name: root.name,
                path,
            },
            value,
            version,
            equation: HypId::fresh(),
        })
    }

    // --- Blocks and statements ---

    fn block(&mut self, ty: &Type, depth: usize, determined: bool) -> Block {
        let scope = self.locals.len();
        let stmts = self.stmts(depth);
        // A block of unit type sometimes has no tail at all.
        let tail = if is_unit(ty) && self.rng.chance(1, 3) {
            None
        } else {
            Some(Box::new(self.expr(ty, depth, determined)))
        };
        self.truncate(scope);
        Block { stmts, tail }
    }

    /// Statements, which bind locals the caller takes out of scope.
    fn stmts(&mut self, depth: usize) -> Vec<Stmt> {
        if depth == 0 {
            return Vec::new();
        }
        let count = *self.rng.choose(&[0, 0, 1, 1, 2, 3]);
        (0..count).map(|_| self.stmt(depth - 1)).collect()
    }

    fn stmt(&mut self, depth: usize) -> Stmt {
        match self.weighted(STMTS).expect("the kinds have weight") {
            StmtKind::Destructure => {
                let count = self.rng.range(1..4);
                let fields: Vec<Type> = (0..count).map(|_| self.random_type(0)).collect();
                let ty = Type::Tuple(fields.clone());
                let value = self.expr(&ty, depth, false);
                let determined = determined(&value, &self.known);
                let patterns = fields
                    .into_iter()
                    .map(|field| {
                        let binder = self.fresh(field);
                        self.bind(binder.clone(), determined);
                        Pattern::Bind {
                            binder,
                            equation: HypId::fresh(),
                            mutable: false,
                        }
                    })
                    .collect();
                Stmt::Let {
                    pattern: Pattern::Tuple(patterns),
                    value,
                }
            }
            StmtKind::Discard => {
                let ty = self.random_type(1);
                Stmt::Let {
                    pattern: Pattern::Wildcard,
                    value: self.expr(&ty, depth, false),
                }
            }
            StmtKind::Effect => {
                // Not of a ghost type: a call to a `math fn` that returns a
                // proof erases to `Proved;`, a path statement Rust lints.
                let ty = self.random_type(1);
                let effect = if ty.is_ghost() {
                    None
                } else {
                    self.expr_from(STATEMENTS, &ty, depth, false)
                };
                match effect {
                    Some(expr) => Stmt::Expr(expr),
                    None => self.binding(depth),
                }
            }
            StmtKind::Bind => self.binding(depth),
            StmtKind::LetMut => self.let_mut(depth),
            StmtKind::Assign => self.assign(depth).unwrap_or_else(|| self.binding(depth)),
        }
    }

    fn binding(&mut self, depth: usize) -> Stmt {
        let ty = self.random_type(1);
        let value = self.expr(&ty, depth, false);
        let determined = determined(&value, &self.known);
        let binder = self.fresh(ty);
        self.bind(binder.clone(), determined);
        let_(&binder, value)
    }

    // --- Expressions ---

    /// An expression of the type. `determined` asks for one whose Rust type
    /// is determined, for a receiver.
    fn expr(&mut self, ty: &Type, depth: usize, determined: bool) -> Expr {
        self.expr_from(PRODUCTIONS, ty, depth, determined)
            .unwrap_or_else(|| self.leaf(ty, determined))
    }

    /// An expression by one of the given productions, or `None` when none
    /// of them fits.
    fn expr_from(
        &mut self,
        allowed: &[(Production, u32)],
        ty: &Type,
        depth: usize,
        determined: bool,
    ) -> Option<Expr> {
        self.cost += self.multiplier;
        // Past the budget, only leaves.
        let depth = if self.cost > BUDGET { 0 } else { depth };
        let choices: Vec<(Production, u32)> = allowed
            .iter()
            .filter(|(production, _)| self.fits(*production, ty, depth, determined))
            .copied()
            .collect();
        let production = self.weighted(&choices)?;
        self.produce(production, ty, depth, determined)
    }

    fn fits(&self, production: Production, ty: &Type, depth: usize, determined: bool) -> bool {
        let is_machine = ty.as_machine().is_some();
        let is_bool = same_type(ty, &Type::Bool);
        match production {
            Production::Literal => !(determined && is_machine),
            Production::Var => self
                .locals
                .iter()
                .any(|local| same_type(&local.ty, ty) && (!determined || self.known[&local.id])),
            Production::Field | Production::Block => depth > 0,
            // The kernel has no case with a ghost result.
            Production::If => depth > 0 && !ty.is_ghost(),
            Production::Method | Production::Cast => depth > 0 && is_machine,
            Production::Compare | Production::ShortCircuit | Production::Not => {
                depth > 0 && is_bool
            }
            Production::Match => depth > 0 && !ty.is_ghost() && !self.program.enums.is_empty(),
            Production::Call => depth > 0 && !self.callable(ty).is_empty(),
            Production::Loop => depth > 0 && !self.math,
            Production::For => depth > 0 && matches!(ty, Type::Tuple(_)),
        }
    }

    /// The functions a call of the type may go to: those with the result
    /// type, pure ones from a `math fn`, and affordable ones.
    fn callable(&self, ty: &Type) -> Vec<usize> {
        (0..self.program.fns.len())
            .filter(|&index| {
                let function = &self.program.fns[index];
                same_type(&function.item.result, ty)
                    && (!self.math || function.item.math)
                    && self.cost + self.multiplier * function.cost <= BUDGET
            })
            .collect()
    }

    fn produce(
        &mut self,
        production: Production,
        ty: &Type,
        depth: usize,
        determined: bool,
    ) -> Option<Expr> {
        let inner = depth.saturating_sub(1);
        Some(match production {
            Production::Literal => self.literal(ty, inner, determined),
            Production::Var => {
                let choices: Vec<Binder> = self
                    .locals
                    .iter()
                    .filter(|local| {
                        same_type(&local.ty, ty) && (!determined || self.known[&local.id])
                    })
                    .cloned()
                    .collect();
                Expr::var(self.rng.choose(&choices))
            }
            Production::Field => self.field(ty, inner, determined),
            Production::Method => {
                let machine = ty.as_machine().expect("fits");
                let mut ops = vec![Op::WrappingAdd, Op::WrappingSub, Op::WrappingMul];
                if machine.signed() {
                    ops.push(Op::WrappingNeg);
                }
                let op = *self.rng.choose(&ops);
                // A literal receiver is printed with its suffix, so it is
                // determined on its own.
                let receiver = if self.rng.chance(1, 5) {
                    Expr::Literal(machine, self.value_of(machine))
                } else {
                    self.expr(ty, inner, true)
                };
                let arguments = (1..op.arity())
                    .map(|_| self.expr(ty, inner, false))
                    .collect();
                Expr::Method {
                    prim: Prim::Op(op, machine),
                    receiver: Box::new(receiver),
                    arguments,
                }
            }
            Production::Cast => {
                let from = self.machine_type();
                let value = self.expr(&Type::machine(from), inner, false);
                Expr::Cast {
                    expr: Box::new(value),
                    from: Type::machine(from),
                    to: ty.clone(),
                }
            }
            Production::Compare => {
                let op = *self.rng.choose(&[
                    CompareOp::Eq,
                    CompareOp::Ne,
                    CompareOp::Lt,
                    CompareOp::Le,
                    CompareOp::Gt,
                    CompareOp::Ge,
                ]);
                let machine = self.machine_type();
                let left = self.expr(&Type::machine(machine), inner, false);
                let right = self.expr(&Type::machine(machine), inner, false);
                compare(op, machine, left, right)
            }
            Production::If => {
                let condition = self.condition(inner);
                self.branch(
                    condition,
                    ty,
                    |g| g.block(ty, inner, determined),
                    |g| g.block(ty, inner, determined),
                )
            }
            Production::ShortCircuit => {
                let left = self.condition(inner);
                if self.rng.chance(1, 2) {
                    // a && b
                    self.branch(
                        left,
                        ty,
                        |g| g.block(&Type::Bool, inner, false),
                        |_| tail_block(Expr::Bool(false)),
                    )
                } else {
                    // a || b
                    self.branch(
                        left,
                        ty,
                        |_| tail_block(Expr::Bool(true)),
                        |g| g.block(&Type::Bool, inner, false),
                    )
                }
            }
            Production::Not => {
                let condition = self.condition(inner);
                if_(
                    condition,
                    tail_block(Expr::Bool(false)),
                    tail_block(Expr::Bool(true)),
                    ty,
                    None,
                )
            }
            Production::Match => self.match_(ty, inner, determined),
            Production::Call => self.call(ty, inner)?,
            Production::Loop => self.loop_(ty, inner, determined),
            Production::For => self.for_(ty, inner),
            Production::Block => Expr::Block(self.block(ty, inner, determined)),
        })
    }

    /// A leaf of the type, when nothing else fits: a literal, or for a
    /// machine integer that must be determined, a method on a suffixed
    /// literal.
    fn leaf(&mut self, ty: &Type, determined: bool) -> Expr {
        match ty {
            Type::U8 | Type::Machine(_) => {
                let machine = ty.as_machine().expect("a machine type");
                let literal = Expr::Literal(machine, self.value_of(machine));
                if determined {
                    Expr::Method {
                        prim: Prim::Op(Op::WrappingAdd, machine),
                        receiver: Box::new(literal),
                        arguments: vec![Expr::Literal(machine, 0)],
                    }
                } else {
                    literal
                }
            }
            Type::Bool => Expr::Bool(self.rng.chance(1, 2)),
            Type::Proof(_) => evidence(),
            Type::Tuple(fields) => Expr::Tuple {
                ty: ty.clone(),
                fields: fields
                    .clone()
                    .iter()
                    .map(|field| self.leaf(field, determined))
                    .collect(),
            },
            Type::Struct(id) => {
                let item = self.program.struct_item(*id).clone();
                Expr::Struct {
                    id: *id,
                    name: item.name,
                    fields: item
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), self.leaf(&field.ty, determined)))
                        .collect(),
                }
            }
            Type::Enum(id) => {
                let item = self.program.enum_item(*id).clone();
                let index = self.rng.range(0..item.variants.len());
                let variant = &item.variants[index];
                Expr::Variant {
                    id: *id,
                    enum_name: item.name.clone(),
                    index,
                    variant_name: variant.name.clone(),
                    payload: variant
                        .payload
                        .iter()
                        .map(|field| self.leaf(&field.ty, determined))
                        .collect(),
                }
            }
            other => unreachable!("{other:?} is not a type of the fragment"),
        }
    }

    /// A literal of the type. A composite one has generated fields.
    fn literal(&mut self, ty: &Type, depth: usize, determined: bool) -> Expr {
        match ty {
            Type::Tuple(fields) => Expr::Tuple {
                ty: ty.clone(),
                fields: fields
                    .clone()
                    .iter()
                    .map(|field| self.expr(field, depth, determined))
                    .collect(),
            },
            Type::Struct(id) => {
                let item = self.program.struct_item(*id).clone();
                Expr::Struct {
                    id: *id,
                    name: item.name,
                    fields: item
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), self.expr(&field.ty, depth, determined)))
                        .collect(),
                }
            }
            Type::Enum(id) => {
                let item = self.program.enum_item(*id).clone();
                let index = self.rng.range(0..item.variants.len());
                let variant = &item.variants[index];
                Expr::Variant {
                    id: *id,
                    enum_name: item.name.clone(),
                    index,
                    variant_name: variant.name.clone(),
                    payload: variant
                        .payload
                        .iter()
                        .map(|field| self.expr(&field.ty, depth, determined))
                        .collect(),
                }
            }
            other => self.leaf(other, determined),
        }
    }

    /// A condition: a bool, braced if Rust would misparse it.
    fn condition(&mut self, depth: usize) -> Expr {
        let condition = self.expr(&Type::Bool, depth, false);
        guarded(condition)
    }

    /// A field of a local whose product type has one of the type, or of a
    /// generated tuple made to contain the type.
    fn field(&mut self, ty: &Type, depth: usize, determined: bool) -> Expr {
        let mut candidates: Vec<(Binder, usize, Option<String>)> = Vec::new();
        for local in &self.locals {
            if determined && !self.known[&local.id] {
                continue;
            }
            match &local.ty {
                Type::Tuple(fields) => {
                    for (index, field) in fields.iter().enumerate() {
                        if same_type(field, ty) {
                            candidates.push((local.clone(), index, None));
                        }
                    }
                }
                Type::Struct(id) => {
                    for (index, field) in self.program.struct_item(*id).fields.iter().enumerate() {
                        if same_type(&field.ty, ty) {
                            candidates.push((local.clone(), index, Some(field.name.clone())));
                        }
                    }
                }
                _ => {}
            }
        }
        if !candidates.is_empty() && self.rng.chance(2, 3) {
            let (binder, index, name) = self.rng.choose(&candidates).clone();
            return Expr::Field {
                target: Box::new(Expr::var(&binder)),
                index,
                name,
                ty: ty.clone(),
            };
        }
        let other = self.random_type(0);
        let (fields, index) = match self.rng.below(3) {
            0 => (vec![ty.clone()], 0),
            1 => (vec![other, ty.clone()], 1),
            _ => (vec![ty.clone(), other], 0),
        };
        let target = self.expr(&Type::Tuple(fields), depth, determined);
        Expr::Field {
            target: Box::new(target),
            index,
            name: None,
            ty: ty.clone(),
        }
    }

    fn match_(&mut self, ty: &Type, depth: usize, determined: bool) -> Expr {
        let (id, item) = self.rng.choose(&self.program.enums).clone();
        let scrutinee = guarded(self.expr(&Type::Enum(id), depth, false));
        let entry = self.entry();
        let mut ends = Vec::new();
        let arms = item
            .variants
            .iter()
            .map(|variant| {
                let scope = self.locals.len();
                let payload: Vec<Binder> = variant
                    .payload
                    .iter()
                    .map(|field| {
                        let binder = self.fresh(field.ty.clone());
                        self.bind(binder.clone(), true);
                        binder
                    })
                    .collect();
                let body = self.block(ty, depth, determined);
                self.truncate(scope);
                ends.push(self.versions(&entry));
                self.restore(&entry);
                MatchArm {
                    variant_name: variant.name.clone(),
                    payload,
                    fact: HypId::fresh(),
                    body,
                }
            })
            .collect();
        let joined = self.join(&entry, &ends);
        Expr::Match {
            scrutinee: Box::new(scrutinee),
            enum_name: item.name,
            arms,
            ty: ty.clone(),
            result: VarId::fresh(),
            joined,
        }
    }

    fn call(&mut self, ty: &Type, depth: usize) -> Option<Expr> {
        let candidates = self.callable(ty);
        if candidates.is_empty() {
            return None;
        }
        let index = *self.rng.choose(&candidates);
        let function = self.program.fns[index].clone();
        self.cost += self.multiplier * function.cost;
        let arguments = function
            .item
            .params
            .iter()
            .map(|param| self.expr(&param.ty, depth, false))
            .collect();
        Some(match function.reference {
            FnRef::Exec(id) => Expr::CallFn {
                id,
                name: function.item.name,
                arguments,
                result: VarId::fresh(),
                ty: ty.clone(),
            },
            FnRef::Math(id) => Expr::CallMath {
                id,
                name: function.item.name,
                arguments,
                ty: ty.clone(),
            },
        })
    }

    /// loop (i: u8 = 0, s: S = init, ...) -> ty {
    ///     stmts
    ///     if i == LIMIT { stmts; break value } else { stmts; continue(i + 1, next...) }
    /// }
    /// The test is spelled in one of several ways, with the branches to
    /// match; the counter is never assigned, so the loop ends.
    fn loop_(&mut self, ty: &Type, depth: usize, determined: bool) -> Expr {
        let limit = self.rng.below(6) as u8;
        let scope = self.locals.len();
        let counter = self.fresh(Type::U8);
        let mut state = vec![(counter.clone(), Expr::u8(0))];
        for _ in 0..self.rng.below(3) {
            let state_ty = self.random_type(1);
            // Evaluated before the state is in scope.
            let init = self.expr(&state_ty, depth, false);
            state.push((self.fresh(state_ty), init));
        }
        for (binder, _) in &state {
            self.bind(binder.clone(), true);
        }
        let outer = self.multiplier;
        self.multiplier = outer * (u64::from(limit) + 1);
        self.loop_depth += 1;
        let head = self.stmts(depth);
        let i = Expr::var(&counter);
        let at_limit = Expr::u8(limit);
        let spelling = self.rng.below(4);
        let condition = match spelling {
            0 => compare(CompareOp::Eq, MachineInt::U8, i, at_limit),
            1 => compare(CompareOp::Ne, MachineInt::U8, i, at_limit),
            2 => compare(CompareOp::Ge, MachineInt::U8, i, at_limit),
            _ => compare(CompareOp::Lt, MachineInt::U8, i, at_limit),
        };
        // The arms may assign what `head` declared; the join is recorded
        // although every arm leaves the loop or continues it.
        let stop = |g: &mut Self| {
            let scope = g.locals.len();
            let stmts = g.stmts(depth);
            let value = g.expr(ty, depth, determined);
            g.truncate(scope);
            Block {
                stmts,
                tail: Some(Box::new(Expr::Break(Box::new(value)))),
            }
        };
        let go = |g: &mut Self| g.advance(Some(&counter), &state[1..], depth);
        let tail = if matches!(spelling, 0 | 2) {
            self.branch(condition, &unit(), stop, go)
        } else {
            self.branch(condition, &unit(), go, stop)
        };
        self.loop_depth -= 1;
        self.multiplier = outer;
        self.truncate(scope);
        let body = Block {
            stmts: head,
            tail: Some(Box::new(tail)),
        };
        Expr::Loop {
            state,
            result_ty: ty.clone(),
            body,
            result: VarId::fresh(),
        }
    }

    /// for i in LO..HI (s: S = init, ...) { stmts; continue(next...) }
    /// The value is the final state, a tuple of the type.
    fn for_(&mut self, ty: &Type, depth: usize) -> Expr {
        let Type::Tuple(fields) = ty else {
            unreachable!("a for is produced for a tuple type")
        };
        // The index has a random machine type; the bounds are literals
        // near zero, so that a signed range may start below it.
        let machine = self.machine_type();
        let base = if machine.signed() { -2 } else { 0 };
        let lo = base + self.rng.below(4) as i128;
        let hi = lo + self.rng.below(6) as i128;
        let scope = self.locals.len();
        let state: Vec<(Binder, Expr)> = fields
            .clone()
            .into_iter()
            .map(|field| {
                let init = self.expr(&field, depth, false);
                (self.fresh(field), init)
            })
            .collect();
        let index = self.fresh(Type::machine(machine));
        // The bounds are suffixed literals, so the index has a type.
        self.bind(index.clone(), true);
        for (binder, _) in &state {
            self.bind(binder.clone(), true);
        }
        let outer = self.multiplier;
        self.multiplier = outer * ((hi - lo) as u64 + 1);
        self.loop_depth += 1;
        let body = self.advance(None, &state, depth);
        self.loop_depth -= 1;
        self.multiplier = outer;
        self.truncate(scope);
        Expr::For {
            index,
            lower: HypId::fresh(),
            upper: HypId::fresh(),
            lo: Box::new(Expr::Literal(machine, lo)),
            hi: Box::new(Expr::Literal(machine, hi)),
            ordered: ordered(machine, lo, hi),
            state,
            body,
            result: VarId::fresh(),
        }
    }

    /// A block that ends the iteration with `continue`: the counter stepped,
    /// the rest of the state generated. Outside a `math fn` the `continue`
    /// is sometimes under an `if`, so that a match in tail position is
    /// exercised in a loop.
    fn advance(
        &mut self,
        counter: Option<&Binder>,
        state: &[(Binder, Expr)],
        depth: usize,
    ) -> Block {
        let scope = self.locals.len();
        let stmts = self.stmts(depth);
        let tail = if !self.math && self.rng.chance(1, 4) {
            let condition = self.condition(depth);
            self.branch(
                condition,
                &unit(),
                |g| g.advance_plainly(counter, state, depth),
                |g| g.advance_plainly(counter, state, depth),
            )
        } else {
            self.next(counter, state, depth)
        };
        self.truncate(scope);
        Block {
            stmts,
            tail: Some(Box::new(tail)),
        }
    }

    fn advance_plainly(
        &mut self,
        counter: Option<&Binder>,
        state: &[(Binder, Expr)],
        depth: usize,
    ) -> Block {
        let scope = self.locals.len();
        let stmts = self.stmts(depth);
        let tail = self.next(counter, state, depth);
        self.truncate(scope);
        Block {
            stmts,
            tail: Some(Box::new(tail)),
        }
    }

    fn next(&mut self, counter: Option<&Binder>, state: &[(Binder, Expr)], depth: usize) -> Expr {
        let mut next: Vec<Expr> = counter
            .map(|counter| plus_one(Expr::var(counter)))
            .into_iter()
            .collect();
        for (binder, _) in state {
            let ty = binder.ty.clone();
            next.push(self.expr(&ty, depth, false));
        }
        Expr::Continue(next)
    }
}

// --- Inputs and outcomes -----------------------------------------------------------

type Answer = Result<Outcome, RunError>;

/// Inputs for an entry with the given parameter types: boundary values
/// mixed with random ones.
fn inputs(rng: &mut Rng, params: &[MachineInt]) -> Vec<Vec<Value>> {
    if params.is_empty() {
        return vec![Vec::new()];
    }
    (0..INPUTS)
        .map(|index| {
            params
                .iter()
                .map(|&ty| {
                    let edge = boundary(ty);
                    let value = if index < edge.len() && rng.chance(2, 3) {
                        edge[index]
                    } else {
                        random_value(rng, ty)
                    };
                    Value::Int(ty, value)
                })
                .collect()
        })
        .collect()
}

/// Both interpreters' answers.
fn interpret(session: &Session, callee: FnRef, input: &[Value]) -> [Answer; 2] {
    let checked = CheckInterpreter::new(session.program(), FUEL).call(callee, input.to_vec());
    let erased = Interpreter::new(session.erased(), FUEL).call(callee, input.to_vec());
    [checked, erased]
}

/// Everything observed of one call: the two interpreters, and the compiled
/// program in each build when it was run.
#[derive(Clone, Debug)]
struct Observed {
    checked: Answer,
    erased: Answer,
    rust: [Option<Answered>; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Verdict {
    Agree,
    Disagree(String),
    Inconclusive(String),
}

/// How the three agree, or do not. Out of fuel is inconclusive, as in
/// `tests/differential.rs`; a compiled program killed at the timeout is too.
fn judge(observed: &Observed, module: &Module) -> Verdict {
    let show = |answer: &Answer| match answer {
        Ok(outcome) => outcome.debug(module),
        Err(error) => format!("error: {error}"),
    };
    let (checked, erased) = match (&observed.checked, &observed.erased) {
        (Err(error), _) => {
            return Verdict::Disagree(format!(
                "the check IR interpreter could not run it: {error}"
            ));
        }
        (_, Err(error)) => {
            return Verdict::Disagree(format!("the erased interpreter could not run it: {error}"));
        }
        (Ok(Outcome::OutOfFuel), _) | (_, Ok(Outcome::OutOfFuel)) => {
            return Verdict::Inconclusive("an interpreter ran out of fuel".into());
        }
        (Ok(checked), Ok(erased)) => (checked, erased),
    };
    if checked != erased {
        return Verdict::Disagree(format!(
            "the interpreters differ: check IR {}, erased {}",
            show(&observed.checked),
            show(&observed.erased)
        ));
    }
    let expected = match erased {
        Outcome::Value(value) => Answered::Value(value.debug(module)),
        Outcome::Panic(message) => Answered::Panic(one_line(message)),
        Outcome::OutOfFuel => unreachable!("handled above"),
    };
    for (build, answered) in Overflow::ALL.iter().zip(&observed.rust) {
        match answered {
            None => {}
            Some(Answered::NoAnswer(why)) => {
                return Verdict::Inconclusive(format!("compiled Rust, {}: {why}", build.name()));
            }
            Some(Answered::Failed(why)) => {
                return Verdict::Disagree(format!("compiled Rust, {}: {why}", build.name()));
            }
            Some(answered) if *answered != expected => {
                return Verdict::Disagree(format!(
                    "compiled Rust, {}: {answered}, where the interpreters gave {expected}",
                    build.name()
                ));
            }
            Some(_) => {}
        }
    }
    Verdict::Agree
}

type Judge<'a> = dyn Fn(&Observed, &Module) -> Verdict + 'a;

// --- Running a batch ----------------------------------------------------------------

/// One call to make: an entry function, an input, and what the interpreters
/// answered.
struct Case {
    entry: usize,
    input: Vec<Value>,
    answers: [Answer; 2],
}

/// A program that was generated and checked, with the calls to make.
struct Prepared {
    program: Program,
    session: Session,
    cases: Vec<Case>,
}

fn prepare(program: Program, session: Session, rng: &mut Rng) -> Prepared {
    let mut cases = Vec::new();
    for entry in program.entries(session.erased()) {
        let function = &program.fns[entry];
        let params: Vec<MachineInt> = function
            .item
            .params
            .iter()
            .map(|param| {
                param
                    .ty
                    .as_machine()
                    .expect("an entry takes machine integers")
            })
            .collect();
        for input in inputs(rng, &params) {
            let answers = interpret(&session, function.reference, &input);
            cases.push(Case {
                entry,
                input,
                answers,
            });
        }
    }
    Prepared {
        program,
        session,
        cases,
    }
}

/// A case that did not agree, with everything needed to report and shrink it.
#[derive(Clone, Debug)]
struct Disagreement {
    program: Program,
    entry: usize,
    input: Vec<Value>,
    observed: Observed,
    why: String,
}

#[derive(Default)]
struct Summary {
    generated: usize,
    /// Programs with an assignment, and with one inside an arm of a branch:
    /// what the weights of `STMTS` are set for.
    assigning: usize,
    branching_assignment: usize,
    rejected: Vec<Rejection>,
    /// Generation or checking panicked: the seed and the message.
    crashed: Vec<(u64, String)>,
    cases: usize,
    agreed: usize,
    inconclusive: Vec<String>,
    disagreements: Vec<Disagreement>,
    /// rustc rejected a batch: the message, and the seeds of the programs
    /// that do not compile on their own.
    uncompilable: Vec<String>,
}

fn module_name(index: usize) -> String {
    format!("p{index}")
}

/// The call of a case as Rust, from outside the program's module.
fn call_text(prepared: &Prepared, module: &str, entry: usize, input: &[Value]) -> String {
    let arguments: Vec<String> = input
        .iter()
        .map(|value| rust_value(value, prepared.session.erased(), module))
        .collect();
    format!(
        "{module}::{}({})",
        prepared.program.fns[entry].item.name,
        arguments.join(", ")
    )
}

/// Compiles the prepared programs together, runs them in both builds, and
/// judges every case. `name` keeps this batch's files apart from others'.
fn examine(prepared: &[Prepared], name: &str, judge: &Judge, summary: &mut Summary) {
    let units: Vec<Unit> = prepared
        .iter()
        .enumerate()
        .map(|(index, prepared)| {
            let module = module_name(index);
            let calls = prepared
                .cases
                .iter()
                .map(|case| call_text(prepared, &module, case.entry, &case.input))
                .collect();
            Unit {
                module,
                rust: print_module(prepared.session.erased()),
                calls,
            }
        })
        .collect();
    let source = harness(&units);
    let total: usize = prepared.iter().map(|prepared| prepared.cases.len()).sum();
    let mut answered: [Vec<Option<Answered>>; 2] = [vec![None; total], vec![None; total]];
    let mut compiled = true;
    for (slot, build) in Overflow::ALL.into_iter().enumerate() {
        match compile(name, &source, build) {
            Ok(binary) => {
                let observed = observe(&binary, total, TIMEOUT);
                answered[slot] = observed.into_iter().map(Some).collect();
            }
            Err(stderr) => {
                compiled = false;
                let mut message = format!(
                    "rustc rejected batch {name} with {}:\n{stderr}",
                    build.name()
                );
                // Which programs do not compile on their own.
                let mut found = Vec::new();
                let indices: Vec<usize> = (0..units.len()).collect();
                culprits(&units, &indices, build, name, &mut found);
                for (index, stderr) in found {
                    let _ = write!(
                        message,
                        "\nprogram {} (seed {}) does not compile on its own:\n{stderr}\n{}",
                        units[index].module,
                        prepared[index].program.seed,
                        rendering(&prepared[index].session)
                    );
                }
                summary.uncompilable.push(message);
                // The other build is of the same source.
                break;
            }
        }
    }
    if compiled {
        remove_binaries(name);
    }
    let mut at = 0;
    for prepared in prepared {
        for Case {
            entry,
            input,
            answers: [checked, erased],
        } in &prepared.cases
        {
            let observed = Observed {
                checked: checked.clone(),
                erased: erased.clone(),
                rust: [answered[0][at].take(), answered[1][at].take()],
            };
            at += 1;
            summary.cases += 1;
            let describe = |why: &str| {
                format!(
                    "program seed {}, {}({}): {why}",
                    prepared.program.seed,
                    prepared.program.fns[*entry].item.name,
                    show_input(input, prepared.session.erased())
                )
            };
            match judge(&observed, prepared.session.erased()) {
                Verdict::Agree => summary.agreed += 1,
                Verdict::Inconclusive(why) => summary.inconclusive.push(describe(&why)),
                Verdict::Disagree(why) => summary.disagreements.push(Disagreement {
                    program: prepared.program.clone(),
                    entry: *entry,
                    input: input.clone(),
                    observed,
                    why: describe(&why),
                }),
            }
        }
    }
}

/// Finds up to `SHRUNK` units that do not compile on their own, by
/// bisection: a set that compiles has no culprit in it.
fn culprits(
    units: &[Unit],
    indices: &[usize],
    build: Overflow,
    name: &str,
    found: &mut Vec<(usize, String)>,
) {
    if indices.is_empty() || found.len() >= SHRUNK {
        return;
    }
    let subset: Vec<Unit> = indices
        .iter()
        .map(|&index| Unit {
            module: units[index].module.clone(),
            rust: units[index].rust.clone(),
            calls: units[index].calls.clone(),
        })
        .collect();
    let batch = format!("{name}_culprit");
    let compiled = compile(&batch, &harness(&subset), build);
    remove_binaries(&batch);
    match compiled {
        Ok(_) => {}
        Err(stderr) if indices.len() == 1 => found.push((indices[0], stderr)),
        Err(_) => {
            let (left, right) = indices.split_at(indices.len() / 2);
            culprits(units, left, build, name, found);
            culprits(units, right, build, name, found);
        }
    }
}

fn show_input(input: &[Value], module: &Module) -> String {
    let shown: Vec<String> = input.iter().map(|value| value.debug(module)).collect();
    shown.join(", ")
}

/// Generates and checks the program of a seed, catching a panic in the
/// generator, the checker, or an interpreter.
fn generate_one(seed: u64, base: &Session, summary: &mut Summary) -> Option<Prepared> {
    summary.generated += 1;
    let attempt = catch_unwind(AssertUnwindSafe(|| {
        Generator::generate(seed, base).map(|(program, session)| {
            let mut rng = Rng::new(seed ^ 0xA5A5);
            prepare(program, session, &mut rng)
        })
    }));
    match attempt {
        Ok(Ok(prepared)) => {
            let (assigns, in_branches) = assignments_of(&prepared.program);
            summary.assigning += usize::from(assigns);
            summary.branching_assignment += usize::from(in_branches);
            Some(prepared)
        }
        Ok(Err(rejection)) => {
            summary.rejected.push(rejection);
            None
        }
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "a panic with no message".into());
            summary.crashed.push((seed, message));
            None
        }
    }
}

/// Whether the program assigns anywhere, and inside an arm of a branch.
fn assignments_of(program: &Program) -> (bool, bool) {
    let mut counting = program.clone();
    let (mut assigns, mut in_branches) = (false, false);
    let assigns_in = |block: &Block| {
        block
            .stmts
            .iter()
            .any(|stmt| matches!(stmt, Stmt::Assign { .. }))
    };
    counting.visit_blocks(&mut |block| {
        assigns |= assigns_in(block);
        false
    });
    counting.visit_exprs(&mut |expr, _, _| {
        match expr {
            Expr::If {
                then_block,
                else_block,
                ..
            } => in_branches |= assigns_in(then_block) || assigns_in(else_block),
            Expr::Match { arms, .. } => in_branches |= arms.iter().any(|arm| assigns_in(&arm.body)),
            _ => {}
        }
        false
    });
    (assigns, in_branches)
}

/// Runs `count` programs from the seed, a batch at a time.
fn run(seed: u64, count: u64, judge: &Judge) -> Summary {
    let (base, _, _) = setup();
    let mut summary = Summary::default();
    let seeds: Vec<u64> = match std::env::var("LOCUS_PROGRAM") {
        Ok(program) => vec![program.parse().expect("LOCUS_PROGRAM is a program's seed")],
        Err(_) => (0..count).map(|index| case_seed(seed, index)).collect(),
    };
    for (number, chunk) in seeds.chunks(BATCH).enumerate() {
        let started = Instant::now();
        let prepared: Vec<Prepared> = chunk
            .iter()
            .filter_map(|&seed| generate_one(seed, &base, &mut summary))
            .collect();
        let generated = started.elapsed();
        examine(&prepared, &format!("random_{number}"), judge, &mut summary);
        let _ = writeln!(
            std::io::stderr(),
            "random programs: batch {number}: {} programs, {} cases; generated and interpreted in {generated:.1?}, compiled and compared in {:.1?}",
            prepared.len(),
            prepared
                .iter()
                .map(|prepared| prepared.cases.len())
                .sum::<usize>(),
            started.elapsed() - generated
        );
    }
    summary
}

// --- Reporting and shrinking ----------------------------------------------------------

/// The program as Rust, without the printer's fixed header.
fn rendering(session: &Session) -> String {
    let printed = print_module(session.erased());
    match printed.find("pub struct Ghost;\n") {
        Some(at) => printed[at + "pub struct Ghost;\n".len()..]
            .trim()
            .to_string(),
        None => printed,
    }
}

fn report(disagreement: &Disagreement, base: &Session) -> String {
    let Disagreement {
        program,
        entry,
        input,
        observed,
        why,
    } = disagreement;
    let session = program
        .declare(base)
        .expect("the program was accepted before");
    let module = session.erased();
    let show = |answer: &Answer| match answer {
        Ok(outcome) => outcome.debug(module),
        Err(error) => format!("error: {error}"),
    };
    let mut out = format!(
        "random program with seed {} disagrees at {}({}):\n  {why}\n",
        program.seed,
        program.fns[*entry].item.name,
        show_input(input, module)
    );
    let _ = writeln!(out, "  check IR interpreter: {}", show(&observed.checked));
    let _ = writeln!(out, "  erased interpreter:   {}", show(&observed.erased));
    for (build, answered) in Overflow::ALL.iter().zip(&observed.rust) {
        let _ = writeln!(
            out,
            "  compiled Rust, {}: {}",
            build.name(),
            answered
                .as_ref()
                .map_or_else(|| "not run".to_string(), ToString::to_string)
        );
    }
    let _ = writeln!(
        out,
        "the program, as generated Rust:\n{}",
        rendering(&session)
    );
    let tree = format!("{:?}", program.fns);
    if tree.len() <= 3_000 {
        let _ = writeln!(out, "the typed tree:\n{tree}");
    } else {
        let _ = writeln!(
            out,
            "the typed tree is {} characters long and is not shown",
            tree.len()
        );
    }
    out
}

/// Whether the program, at the disagreement's entry and input, still checks
/// and still disagrees. The interpreters are asked first; the compiled
/// program only when they agree and the original disagreement was with it.
fn still_disagrees(
    program: &Program,
    entry: usize,
    input: &[Value],
    base: &Session,
    with_rustc: bool,
    judge: &Judge,
) -> bool {
    let Ok(session) = program.declare(base) else {
        return false;
    };
    let [checked, erased] = interpret(&session, program.fns[entry].reference, input);
    let mut observed = Observed {
        checked,
        erased,
        rust: [None, None],
    };
    match judge(&observed, session.erased()) {
        Verdict::Disagree(_) => return true,
        Verdict::Inconclusive(_) => return false,
        Verdict::Agree => {}
    }
    if !with_rustc {
        return false;
    }
    let module = module_name(0);
    let prepared = Prepared {
        program: program.clone(),
        session,
        cases: Vec::new(),
    };
    let unit = Unit {
        rust: print_module(prepared.session.erased()),
        calls: vec![call_text(&prepared, &module, entry, input)],
        module,
    };
    let source = harness(std::slice::from_ref(&unit));
    for (slot, build) in Overflow::ALL.into_iter().enumerate() {
        let Ok(binary) = compile("random_shrink", &source, build) else {
            return false;
        };
        observed.rust[slot] = observe(&binary, 1, TIMEOUT).pop();
    }
    remove_binaries("random_shrink");
    matches!(
        judge(&observed, prepared.session.erased()),
        Verdict::Disagree(_)
    )
}

/// One way of making a program smaller. The first index says where, across
/// the whole program in a fixed order; the second which of the alternatives
/// there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// Delete the n-th statement.
    DeleteStmt(usize),
    /// Inline the n-th `let` of a literal or a variable and delete it.
    InlineLet(usize),
    /// Replace the n-th expression by one of its subexpressions: a branch of
    /// a conditional, a field of a tuple literal, the tail of a bare block,
    /// an operand, or an argument of the same type.
    Hoist(usize, usize),
    /// Replace the n-th expression by a literal of its type.
    Literal(usize, usize),
}

/// A variant of an enum: its name and the types of its payload.
type Variant = (String, Vec<Type>);

/// What the walk over a program needs to know of its declarations: the
/// names and types of struct fields and variant payloads, and the parameter
/// types of functions.
struct Tables {
    structs: HashMap<StructId, (String, Vec<(String, Type)>)>,
    enums: HashMap<EnumId, (String, Vec<Variant>)>,
    enum_names: HashMap<String, EnumId>,
    params: HashMap<FnRef, Vec<Type>>,
}

impl Tables {
    fn of(program: &Program) -> Self {
        Self {
            structs: program
                .structs
                .iter()
                .map(|(id, item)| {
                    let fields = item
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), field.ty.clone()))
                        .collect();
                    (*id, (item.name.clone(), fields))
                })
                .collect(),
            enums: program
                .enums
                .iter()
                .map(|(id, item)| {
                    let variants = item
                        .variants
                        .iter()
                        .map(|variant| {
                            let payload = variant.payload.iter().map(|f| f.ty.clone()).collect();
                            (variant.name.clone(), payload)
                        })
                        .collect();
                    (*id, (item.name.clone(), variants))
                })
                .collect(),
            enum_names: program
                .enums
                .iter()
                .map(|(id, item)| (item.name.clone(), *id))
                .collect(),
            params: program
                .fns
                .iter()
                .map(|function| {
                    let params = function.item.params.iter().map(|p| p.ty.clone()).collect();
                    (function.reference, params)
                })
                .collect(),
        }
    }

    /// The type of a place: the binding's type, then each field's.
    fn place_type(&self, place: &Place) -> Option<Type> {
        let mut ty = place.path.first().map(|step| step.ty.clone())?;
        for step in &place.path {
            let fields = match &step.ty {
                Type::Tuple(fields) => fields.clone(),
                Type::Struct(id) => self.struct_fields(*id),
                _ => return None,
            };
            ty = fields.get(step.index)?.clone();
        }
        Some(ty)
    }

    fn struct_fields(&self, id: StructId) -> Vec<Type> {
        self.structs
            .get(&id)
            .map(|(_, fields)| fields.iter().map(|(_, ty)| ty.clone()).collect())
            .unwrap_or_default()
    }

    fn variant_payload(&self, id: EnumId, index: usize) -> Vec<Type> {
        self.enums
            .get(&id)
            .and_then(|(_, variants)| variants.get(index))
            .map(|(_, payload)| payload.clone())
            .unwrap_or_default()
    }
}

/// What the walk knows where it stands: the loops around the position, each
/// with the type its `break` needs and the types its `continue` needs.
struct Scope<'t> {
    tables: &'t Tables,
    loops: Vec<(Option<Type>, Vec<Type>)>,
}

/// Visits an expression in place. Returns true to stop the walk, which is
/// how a mutation says it happened; false to go on into the children.
type Visit<'a> = dyn FnMut(&mut Expr, Option<&Type>, &Scope) -> bool + 'a;

fn pattern_type(pattern: &Pattern) -> Option<Type> {
    match pattern {
        Pattern::Bind { binder, .. } => Some(binder.ty.clone()),
        Pattern::Wildcard => None,
        Pattern::Tuple(patterns) => patterns
            .iter()
            .map(pattern_type)
            .collect::<Option<Vec<Type>>>()
            .map(Type::Tuple),
    }
}

fn walk_block(
    block: &mut Block,
    expected: Option<&Type>,
    scope: &mut Scope,
    visit: &mut Visit,
) -> bool {
    for stmt in &mut block.stmts {
        let stopped = match stmt {
            Stmt::Let { pattern, value } => {
                let ty = pattern_type(pattern);
                walk_expr(value, ty.as_ref(), scope, visit)
            }
            Stmt::Assign { place, value, .. } => {
                let ty = scope.tables.place_type(place);
                walk_expr(value, ty.as_ref(), scope, visit)
            }
            Stmt::Expr(expr) => walk_expr(expr, None, scope, visit),
        };
        if stopped {
            return true;
        }
    }
    match block.tail.as_deref_mut() {
        Some(tail) => walk_expr(tail, expected, scope, visit),
        None => false,
    }
}

fn walk_all(exprs: &mut [Expr], types: &[Type], scope: &mut Scope, visit: &mut Visit) -> bool {
    exprs
        .iter_mut()
        .enumerate()
        .any(|(index, expr)| walk_expr(expr, types.get(index), scope, visit))
}

fn walk_expr(
    expr: &mut Expr,
    expected: Option<&Type>,
    scope: &mut Scope,
    visit: &mut Visit,
) -> bool {
    if visit(expr, expected, scope) {
        return true;
    }
    match expr {
        Expr::Tuple { ty, fields } => {
            let types = match ty {
                Type::Tuple(types) => types.clone(),
                _ => Vec::new(),
            };
            walk_all(fields, &types, scope, visit)
        }
        Expr::Struct { id, fields, .. } => {
            let types = scope.tables.struct_fields(*id);
            fields
                .iter_mut()
                .enumerate()
                .any(|(index, (_, field))| walk_expr(field, types.get(index), scope, visit))
        }
        Expr::Variant {
            id, index, payload, ..
        } => {
            let types = scope.tables.variant_payload(*id, *index);
            walk_all(payload, &types, scope, visit)
        }
        Expr::Field { target, .. } => walk_expr(target, None, scope, visit),
        Expr::Method {
            prim,
            receiver,
            arguments,
        } => {
            let ty = match prim {
                Prim::Op(_, ty) => Type::machine(*ty),
                _ => Type::U8,
            };
            let types = vec![ty.clone(); arguments.len()];
            walk_expr(receiver, Some(&ty), scope, visit)
                || walk_all(arguments, &types, scope, visit)
        }
        Expr::Compare {
            ty, left, right, ..
        } => walk_expr(left, Some(ty), scope, visit) || walk_expr(right, Some(ty), scope, visit),
        Expr::Cast { expr, from, .. } => walk_expr(expr, Some(from), scope, visit),
        Expr::CallMath { id, arguments, .. } => {
            let types = scope.tables.params.get(&FnRef::Math(*id)).cloned();
            walk_all(arguments, &types.unwrap_or_default(), scope, visit)
        }
        Expr::CallFn { id, arguments, .. } => {
            let types = scope.tables.params.get(&FnRef::Exec(*id)).cloned();
            walk_all(arguments, &types.unwrap_or_default(), scope, visit)
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ty,
            ..
        } => {
            walk_expr(condition, Some(&Type::Bool), scope, visit)
                || walk_block(then_block, Some(ty), scope, visit)
                || walk_block(else_block, Some(ty), scope, visit)
        }
        Expr::Match {
            scrutinee,
            enum_name,
            arms,
            ty,
            ..
        } => {
            let scrutinee_ty = scope
                .tables
                .enum_names
                .get(enum_name)
                .map(|id| Type::Enum(*id));
            walk_expr(scrutinee, scrutinee_ty.as_ref(), scope, visit)
                || arms
                    .iter_mut()
                    .any(|arm| walk_block(&mut arm.body, Some(ty), scope, visit))
        }
        Expr::Block(block) => walk_block(block, expected, scope, visit),
        Expr::Loop {
            state,
            result_ty,
            body,
            ..
        } => {
            for (binder, init) in state.iter_mut() {
                if walk_expr(init, Some(&binder.ty), scope, visit) {
                    return true;
                }
            }
            let types = state.iter().map(|(binder, _)| binder.ty.clone()).collect();
            scope.loops.push((Some(result_ty.clone()), types));
            let stopped = walk_block(body, None, scope, visit);
            scope.loops.pop();
            stopped
        }
        Expr::For {
            index,
            lo,
            hi,
            state,
            body,
            ..
        } => {
            if walk_expr(lo, Some(&index.ty), scope, visit)
                || walk_expr(hi, Some(&index.ty), scope, visit)
            {
                return true;
            }
            for (binder, init) in state.iter_mut() {
                if walk_expr(init, Some(&binder.ty), scope, visit) {
                    return true;
                }
            }
            let types = state.iter().map(|(binder, _)| binder.ty.clone()).collect();
            scope.loops.push((None, types));
            let stopped = walk_block(body, None, scope, visit);
            scope.loops.pop();
            stopped
        }
        Expr::Break(value) => {
            let ty = scope.loops.last().and_then(|(result, _)| result.clone());
            walk_expr(value, ty.as_ref(), scope, visit)
        }
        Expr::Continue(next) => {
            let types = scope
                .loops
                .last()
                .map(|(_, state)| state.clone())
                .unwrap_or_default();
            walk_all(next, &types, scope, visit)
        }
        Expr::Var { .. }
        | Expr::Bool(_)
        | Expr::Literal(..)
        | Expr::Int(_)
        | Expr::Proof(_)
        | Expr::Prop(_)
        | Expr::Absurd { .. } => false,
    }
}

impl Program {
    /// Visits every expression of every function, in a fixed order, until
    /// the visit says to stop. Returns whether it did.
    fn visit_exprs(&mut self, visit: &mut Visit) -> bool {
        let tables = Tables::of(self);
        let mut scope = Scope {
            tables: &tables,
            loops: Vec::new(),
        };
        for function in &mut self.fns {
            let result = function.item.result.clone();
            if walk_block(&mut function.item.body, Some(&result), &mut scope, visit) {
                return true;
            }
        }
        false
    }

    /// Visits every block of every function: function bodies, then the
    /// blocks of the expressions that own them, in the order of `visit_exprs`.
    fn visit_blocks(&mut self, visit: &mut dyn FnMut(&mut Block) -> bool) -> bool {
        for function in &mut self.fns {
            if visit(&mut function.item.body) {
                return true;
            }
        }
        self.visit_exprs(&mut |expr, _, _| match expr {
            Expr::If {
                then_block,
                else_block,
                ..
            } => visit(then_block) || visit(else_block),
            Expr::Match { arms, .. } => arms.iter_mut().any(|arm| visit(&mut arm.body)),
            Expr::Block(block) | Expr::Loop { body: block, .. } | Expr::For { body: block, .. } => {
                visit(block)
            }
            _ => false,
        })
    }

    /// Replaces every use of a variable in every function by the expression.
    fn substitute(&mut self, id: VarId, value: &Expr) {
        self.visit_exprs(&mut |expr, _, _| {
            if matches!(expr, Expr::Var { id: used, .. } if *used == id) {
                *expr = value.clone();
            }
            false
        });
    }
}

/// The literals of a type, smallest first; a program is shrunk toward the
/// first that keeps it disagreeing.
fn literals_of(ty: &Type, tables: &Tables) -> Vec<Expr> {
    let first = |ty: &Type| literals_of(ty, tables).into_iter().next();
    let all = |types: &[Type]| types.iter().map(first).collect::<Option<Vec<Expr>>>();
    match ty {
        Type::U8 | Type::Machine(_) => {
            let machine = ty.as_machine().expect("a machine type");
            vec![Expr::Literal(machine, 0), Expr::Literal(machine, 1)]
        }
        Type::Bool => vec![Expr::Bool(false), Expr::Bool(true)],
        Type::Proof(_) => vec![evidence()],
        Type::Tuple(fields) => all(fields)
            .map(|fields| Expr::Tuple {
                ty: ty.clone(),
                fields,
            })
            .into_iter()
            .collect(),
        Type::Struct(id) => {
            let Some((name, fields)) = tables.structs.get(id) else {
                return Vec::new();
            };
            fields
                .iter()
                .map(|(field, ty)| first(ty).map(|value| (field.clone(), value)))
                .collect::<Option<Vec<(String, Expr)>>>()
                .map(|fields| Expr::Struct {
                    id: *id,
                    name: name.clone(),
                    fields,
                })
                .into_iter()
                .collect()
        }
        Type::Enum(id) => {
            let Some((name, variants)) = tables.enums.get(id) else {
                return Vec::new();
            };
            variants
                .iter()
                .enumerate()
                .filter_map(|(index, (variant, payload))| {
                    Some(Expr::Variant {
                        id: *id,
                        enum_name: name.clone(),
                        index,
                        variant_name: variant.clone(),
                        payload: all(payload)?,
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// The subexpressions of the type that may stand in for the expression.
fn hoistable(expr: &Expr, expected: Option<&Type>, tables: &Tables) -> Vec<Expr> {
    // The arguments of a call that have the call's type.
    let arguments_like = |reference: FnRef, arguments: &[Expr]| -> Vec<Expr> {
        let (Some(expected), Some(params)) = (expected, tables.params.get(&reference)) else {
            return Vec::new();
        };
        arguments
            .iter()
            .zip(params)
            .filter(|(_, param)| same_type(param, expected))
            .map(|(argument, _)| argument.clone())
            .collect()
    };
    match expr {
        Expr::If {
            then_block,
            else_block,
            ..
        } => vec![
            Expr::Block(then_block.clone()),
            Expr::Block(else_block.clone()),
        ],
        Expr::Match { arms, .. } => arms
            .iter()
            .map(|arm| Expr::Block(arm.body.clone()))
            .collect(),
        Expr::Field { target, index, .. } => match &**target {
            Expr::Tuple { fields, .. } => fields.get(*index).cloned().into_iter().collect(),
            Expr::Struct { fields, .. } => fields
                .get(*index)
                .map(|(_, field)| field.clone())
                .into_iter()
                .collect(),
            _ => Vec::new(),
        },
        Expr::Block(block) if block.stmts.is_empty() => {
            block.tail.as_deref().cloned().into_iter().collect()
        }
        Expr::Method {
            receiver,
            arguments,
            ..
        } => std::iter::once((**receiver).clone())
            .chain(arguments.iter().cloned())
            .collect(),
        Expr::CallFn { id, arguments, .. } => arguments_like(FnRef::Exec(*id), arguments),
        Expr::CallMath { id, arguments, .. } => arguments_like(FnRef::Math(*id), arguments),
        _ => Vec::new(),
    }
}

/// Applies a step to the program. `None` means there is no such position,
/// `Some(false)` that there is but no such alternative, `Some(true)` that
/// the program changed.
fn apply(program: &mut Program, step: Step) -> Option<bool> {
    match step {
        Step::DeleteStmt(target) => {
            let mut counter = 0;
            let mut found = false;
            program.visit_blocks(&mut |block| {
                if target < counter + block.stmts.len() {
                    block.stmts.remove(target - counter);
                    found = true;
                    return true;
                }
                counter += block.stmts.len();
                false
            });
            found.then_some(true)
        }
        Step::InlineLet(target) => {
            let mut counter = 0;
            let mut inlined: Option<(VarId, Expr)> = None;
            program.visit_blocks(&mut |block| {
                for (index, stmt) in block.stmts.iter().enumerate() {
                    let Stmt::Let {
                        pattern:
                            Pattern::Bind {
                                binder,
                                mutable: false,
                                ..
                            },
                        value,
                    } = stmt
                    else {
                        continue;
                    };
                    if !(is_literal(value) || matches!(value, Expr::Var { .. })) {
                        continue;
                    }
                    if counter == target {
                        inlined = Some((binder.id, value.clone()));
                        block.stmts.remove(index);
                        return true;
                    }
                    counter += 1;
                }
                false
            });
            let (id, value) = inlined?;
            program.substitute(id, &value);
            Some(true)
        }
        Step::Hoist(target, alternative) => {
            let mut counter = 0;
            let mut outcome = None;
            program.visit_exprs(&mut |expr, expected, scope| {
                if counter < target {
                    counter += 1;
                    return false;
                }
                let candidates = hoistable(expr, expected, scope.tables);
                outcome = Some(match candidates.into_iter().nth(alternative) {
                    Some(candidate) => {
                        *expr = candidate;
                        true
                    }
                    None => false,
                });
                true
            });
            outcome
        }
        Step::Literal(target, alternative) => {
            let mut counter = 0;
            let mut outcome = None;
            program.visit_exprs(&mut |expr, expected, scope| {
                if counter < target {
                    counter += 1;
                    return false;
                }
                let candidates = match expected {
                    Some(ty) if !is_literal(expr) => literals_of(ty, scope.tables),
                    _ => Vec::new(),
                };
                outcome = Some(match candidates.into_iter().nth(alternative) {
                    Some(candidate) => {
                        *expr = candidate;
                        true
                    }
                    None => false,
                });
                true
            });
            outcome
        }
    }
}

/// Shrinks a disagreeing program until no step keeps it disagreeing. Every
/// kept step but inlining makes the program smaller, and inlining removes a
/// `let` nothing puts back, so this ends.
fn shrink(disagreement: &Disagreement, base: &Session, judge: &Judge) -> (Program, usize) {
    let Disagreement {
        program,
        entry,
        input,
        observed,
        ..
    } = disagreement;
    // Whether the compiled program is needed to see the disagreement.
    let with_rustc = matches!(
        judge(
            &Observed {
                checked: observed.checked.clone(),
                erased: observed.erased.clone(),
                rust: [None, None],
            },
            base.erased()
        ),
        Verdict::Agree
    );
    let mut current = program.clone();
    let mut kept = 0;
    let mut tried = 0;
    let limit = if with_rustc {
        SHRINK_STEPS_WITH_RUSTC
    } else {
        usize::MAX
    };
    let kinds: [fn(usize, usize) -> Step; 4] = [
        |n, _| Step::DeleteStmt(n),
        |n, _| Step::InlineLet(n),
        Step::Hoist,
        Step::Literal,
    ];
    loop {
        let mut improved = false;
        for kind in kinds {
            let mut position = 0;
            'positions: loop {
                let mut alternative = 0;
                loop {
                    if tried >= limit {
                        return (current, kept);
                    }
                    let step = kind(position, alternative);
                    let mut candidate = current.clone();
                    match apply(&mut candidate, step) {
                        None => break 'positions,
                        Some(false) => break,
                        Some(true) => {}
                    }
                    // A kept step makes the program smaller, except that
                    // inlining a `let` may not (and removes a `let` nothing
                    // puts back), and a literal may replace a variable of the
                    // same size (and is never made a variable again).
                    let allowed = match step {
                        Step::InlineLet(_) => true,
                        Step::Literal(..) => candidate.size() <= current.size(),
                        Step::DeleteStmt(_) | Step::Hoist(..) => candidate.size() < current.size(),
                    };
                    tried += 1;
                    if allowed
                        && still_disagrees(&candidate, *entry, input, base, with_rustc, judge)
                    {
                        current = candidate;
                        kept += 1;
                        improved = true;
                        // The positions moved; look at this one again.
                        continue 'positions;
                    }
                    alternative += 1;
                    if matches!(step, Step::DeleteStmt(_) | Step::InlineLet(_)) {
                        break;
                    }
                }
                position += 1;
            }
        }
        if !improved {
            return (current, kept);
        }
    }
}

/// The report of a disagreement, with the program shrunk.
fn full_report(disagreement: &Disagreement, base: &Session, judge: &Judge) -> String {
    let mut out = report(disagreement, base);
    let (shrunk, kept) = shrink(disagreement, base, judge);
    match shrunk.declare(base) {
        Ok(session) => {
            let _ = writeln!(out, "shrunk by {kept} step(s) to:\n{}", rendering(&session));
            let tree = format!("{:?}", shrunk.fns);
            if tree.len() <= 3_000 {
                let _ = writeln!(out, "the shrunk typed tree:\n{tree}");
            }
        }
        Err(error) => {
            let _ = writeln!(out, "the shrunk program no longer checks ({error})");
        }
    }
    out
}

/// The first part of a long text.
fn shortened(text: &str) -> String {
    const LIMIT: usize = 3_000;
    if text.len() <= LIMIT {
        return text.to_string();
    }
    let cut = (0..=LIMIT)
        .rev()
        .find(|&at| text.is_char_boundary(at))
        .unwrap_or(0);
    format!("{}... ({} characters in all)", &text[..cut], text.len())
}

fn print_summary(summary: &Summary, mode: &str) {
    let mut out = format!(
        "random programs ({mode}): {} generated ({} assign, {} in a branch), {} rejected by the checker, {} crashed, {} cases: {} agreed, {} inconclusive, {} disagreed\n",
        summary.generated,
        summary.assigning,
        summary.branching_assignment,
        summary.rejected.len(),
        summary.crashed.len(),
        summary.cases,
        summary.agreed,
        summary.inconclusive.len(),
        summary.disagreements.len()
    );
    for rejection in summary.rejected.iter().take(3) {
        let _ = writeln!(
            out,
            "rejected: program with seed {}: {}\n  {}",
            rejection.seed,
            rejection.error,
            shortened(&rejection.function)
        );
    }
    for (seed, message) in summary.crashed.iter().take(3) {
        let _ = writeln!(out, "crashed: program with seed {seed}: {message}");
    }
    for why in summary.inconclusive.iter().take(20) {
        let _ = writeln!(out, "inconclusive: {why}");
    }
    let _ = write!(std::io::stderr(), "{out}");
}

/// The run's seed: `LOCUS_SEED`, or in the extended run the clock.
fn run_seed(extended: bool) -> u64 {
    if let Ok(seed) = std::env::var("LOCUS_SEED") {
        return seed.parse().expect("LOCUS_SEED is a number");
    }
    if extended {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(FAST_SEED, |since| since.as_nanos() as u64)
    } else {
        FAST_SEED
    }
}

// --- The tests ----------------------------------------------------------------------

#[test]
fn generated_programs_agree_three_ways() {
    let extended = std::env::var_os("LOCUS_EXTENDED").is_some();
    let (seed, count, mode) = if extended {
        (run_seed(true), EXTENDED_COUNT, "extended")
    } else {
        (run_seed(false), FAST_COUNT, "fast")
    };
    let _ = writeln!(
        std::io::stderr(),
        "random programs ({mode}): seed {seed}, {count} programs"
    );
    let started = Instant::now();
    let summary = run(seed, count, &judge);
    print_summary(&summary, mode);
    let _ = writeln!(
        std::io::stderr(),
        "random programs ({mode}): {:.1?} in all",
        started.elapsed()
    );

    let (base, _, _) = setup();
    let mut failures = Vec::new();
    for disagreement in summary.disagreements.iter().take(SHRUNK) {
        failures.push(full_report(disagreement, &base, &judge));
    }
    for disagreement in summary.disagreements.iter().skip(SHRUNK) {
        failures.push(disagreement.why.clone());
    }
    failures.extend(summary.uncompilable.iter().cloned());
    for (seed, message) in &summary.crashed {
        failures.push(format!("program with seed {seed} crashed: {message}"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));

    let rejected = summary.rejected.len() as f64 / summary.generated.max(1) as f64;
    assert!(
        rejected <= MAX_REJECTED,
        "the checker rejected {} of {} generated programs; the first: {:?}",
        summary.rejected.len(),
        summary.generated,
        summary.rejected.first()
    );
    assert!(summary.cases > 0, "nothing was compared");
    if extended {
        let inconclusive = summary.inconclusive.len() as f64 / summary.cases as f64;
        assert!(
            inconclusive <= MAX_INCONCLUSIVE,
            "{} of {} cases were inconclusive",
            summary.inconclusive.len(),
            summary.cases
        );
    }
}

/// fn f0(v0: u8) -> u8 {
///     let v1 = v0.wrapping_add(1);
///     let v2 = if v0 == 3 { 7 } else { v1 };
///     let v3 = (v2, v1);
///     v3.0
/// }
fn planted(base: &Session) -> (Program, Session) {
    let v0 = Binder::new("v0", Type::U8);
    let v1 = Binder::new("v1", Type::U8);
    let v2 = Binder::new("v2", Type::U8);
    let pair = Type::Tuple(vec![Type::U8, Type::U8]);
    let v3 = Binder::new("v3", pair.clone());
    let item = FnItem {
        name: "f0".into(),
        math: false,
        params: vec![v0.clone()],
        result: Type::U8,
        body: Block {
            stmts: vec![
                let_(&v1, plus_one(Expr::var(&v0))),
                let_(
                    &v2,
                    if_(
                        compare(CompareOp::Eq, MachineInt::U8, Expr::var(&v0), Expr::u8(3)),
                        tail_block(Expr::u8(7)),
                        tail_block(Expr::var(&v1)),
                        &Type::U8,
                        None,
                    ),
                ),
                let_(
                    &v3,
                    Expr::Tuple {
                        ty: pair,
                        fields: vec![Expr::var(&v2), Expr::var(&v1)],
                    },
                ),
            ],
            tail: Some(Box::new(Expr::Field {
                target: Box::new(Expr::var(&v3)),
                index: 0,
                name: None,
                ty: Type::U8,
            })),
        },
    };
    let mut session = base.clone();
    let reference = session
        .declare_fn(&item)
        .expect("the planted program checks");
    let program = Program {
        seed: 7_777,
        structs: Vec::new(),
        enums: Vec::new(),
        fns: vec![Function {
            item,
            reference,
            cost: 1,
        }],
    };
    (program, session)
}

/// A comparator that calls the value 7 a disagreement, as if one side had
/// returned something else.
fn planted_judge(observed: &Observed, module: &Module) -> Verdict {
    if let Ok(Outcome::Value(Value::Int(MachineInt::U8, 7))) = observed.erased {
        return Verdict::Disagree("the planted comparator calls 7 a disagreement".into());
    }
    judge(observed, module)
}

#[test]
fn a_planted_disagreement_is_reported_with_its_seed_and_shrunk() {
    let (base, _, _) = setup();
    let (program, session) = planted(&base);
    let prepared = Prepared {
        cases: [3, 4]
            .into_iter()
            .map(|byte| Case {
                entry: 0,
                input: vec![Value::u8(byte)],
                answers: interpret(&session, program.fns[0].reference, &[Value::u8(byte)]),
            })
            .collect(),
        program,
        session,
    };
    let mut summary = Summary::default();
    examine(&[prepared], "random_planted", &planted_judge, &mut summary);
    assert_eq!(summary.cases, 2);
    assert_eq!(summary.agreed, 1);
    assert!(
        summary.uncompilable.is_empty(),
        "{:?}",
        summary.uncompilable
    );
    let [disagreement] = summary.disagreements.as_slice() else {
        panic!("one disagreement, not {:?}", summary.disagreements);
    };
    assert_eq!(disagreement.input, vec![Value::u8(3)]);
    assert!(
        disagreement.why.contains("seed 7777"),
        "{}",
        disagreement.why
    );

    let report = full_report(disagreement, &base, &planted_judge);
    for expected in [
        "random program with seed 7777 disagrees at f0(3)",
        "the planted comparator calls 7 a disagreement",
        "check IR interpreter: 7",
        "erased interpreter:   7",
        "compiled Rust, overflow checks on: 7",
        "compiled Rust, overflow checks off: 7",
        "let v3 = (v2, v1);",
        "the typed tree:",
        "shrunk by",
    ] {
        assert!(
            report.contains(expected),
            "missing {expected:?} in:\n{report}"
        );
    }

    // The shrunk program is `fn f0(v0: u8) -> u8 { 7 }`: the conditional is
    // replaced by its taken branch, the lets inlined, the projection taken.
    let (shrunk, kept) = shrink(disagreement, &base, &planted_judge);
    assert!(kept >= 4, "{kept} steps");
    let body = &shrunk.fns[0].item.body;
    assert!(body.stmts.is_empty(), "{body:?}");
    assert!(
        matches!(body.tail.as_deref(), Some(Expr::Literal(MachineInt::U8, 7))),
        "{body:?}"
    );
    assert!(shrunk.declare(&base).is_ok());
    assert!(report.contains("shrunk by"), "{report}");
    let shrunk_rust = rendering(&shrunk.declare(&base).unwrap());
    assert!(
        shrunk_rust.contains("pub fn f0(v0: u8) -> u8 {\n    7_u8\n}"),
        "{shrunk_rust}"
    );
}

#[test]
fn a_shrink_step_that_breaks_the_program_is_not_kept() {
    // Deleting the `let` of a variable in use leaves a program the checker
    // rejects, which is never kept, and the report says so instead of
    // failing.
    let (base, _, _) = setup();
    let (program, _) = planted(&base);
    let mut broken = program.clone();
    assert_eq!(apply(&mut broken, Step::DeleteStmt(0)), Some(true));
    assert!(broken.declare(&base).is_err());
    assert!(!still_disagrees(
        &broken,
        0,
        &[Value::u8(3)],
        &base,
        false,
        &planted_judge
    ));
    // Past the last statement and the last expression there is nothing.
    assert_eq!(apply(&mut program.clone(), Step::DeleteStmt(3)), None);
    assert_eq!(apply(&mut program.clone(), Step::Literal(1_000, 0)), None);
    assert_eq!(apply(&mut program.clone(), Step::Hoist(1_000, 0)), None);
    // A literal is not replaced by a literal.
    let mut literal = program.clone();
    assert_eq!(apply(&mut literal, Step::Literal(0, 0)), Some(true));
    assert_eq!(apply(&mut literal, Step::Literal(0, 0)), Some(false));
}

#[test]
fn a_generated_program_is_declared_to_the_same_identities_again() {
    let (base, _, _) = setup();
    let mut seen_loop = false;
    let mut seen_for = false;
    let mut seen_math = false;
    for index in 0..20 {
        let seed = case_seed(FAST_SEED, index);
        let Ok((program, session)) = Generator::generate(seed, &base) else {
            continue;
        };
        let again = program
            .declare(&base)
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(print_module(again.erased()), print_module(session.erased()));
        let rust = print_module(session.erased());
        seen_loop |= rust.contains("loop {");
        seen_for |= rust.contains("for v");
        seen_math |= program.fns.iter().any(|function| function.item.math);
    }
    assert!(
        seen_loop && seen_for && seen_math,
        "the fragment is exercised"
    );
}

#[test]
fn the_programs_of_a_seed_are_the_same_every_time() {
    let (base, _, _) = setup();
    let seed = case_seed(FAST_SEED, 3);
    let first = Generator::generate(seed, &base).map(|(_, session)| print_module(session.erased()));
    let second =
        Generator::generate(seed, &base).map(|(_, session)| print_module(session.erased()));
    assert_eq!(first.map_err(|r| r.error), second.map_err(|r| r.error));
}

/// A value whose type only a literal gave it, a `let` of a bare literal or
/// the index of a `for` between two literals, used as a receiver of
/// `wrapping_add`. Rust cannot call a method on an `{integer}`, so until E5
/// the printed program did not compile and the generator avoided the shapes
/// (see `determined`). The printer now writes every literal with its type
/// as a suffix, and both programs compile and agree.
#[test]
fn a_byte_typed_by_a_literal_alone_can_be_a_receiver() {
    let (base, _, _) = setup();
    // fn by_let(n: u8) -> u8 { let k = 200; k.wrapping_add(n) }
    let n = Binder::new("n", Type::U8);
    let k = Binder::new("k", Type::U8);
    let by_let = FnItem {
        name: "by_let".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: Block {
            stmts: vec![let_(&k, Expr::u8(200))],
            tail: Some(Box::new(Expr::Method {
                prim: Prim::Op(Op::WrappingAdd, MachineInt::U8),
                receiver: Box::new(Expr::var(&k)),
                arguments: vec![Expr::var(&n)],
            })),
        },
    };
    // fn by_for(n: u8) -> (u8,) { for i in 0..3 (acc: u8 = n) { continue(i.wrapping_add(acc)) } }
    let n = Binder::new("n", Type::U8);
    let i = Binder::new("i", Type::U8);
    let acc = Binder::new("acc", Type::U8);
    let by_for = FnItem {
        name: "by_for".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::Tuple(vec![Type::U8]),
        body: tail_block(Expr::For {
            index: i.clone(),
            lower: HypId::fresh(),
            upper: HypId::fresh(),
            lo: Box::new(Expr::u8(0)),
            hi: Box::new(Expr::u8(3)),
            ordered: ordered(MachineInt::U8, 0, 3),
            state: vec![(acc.clone(), Expr::var(&n))],
            body: tail_block(Expr::Continue(vec![Expr::Method {
                prim: Prim::Op(Op::WrappingAdd, MachineInt::U8),
                receiver: Box::new(Expr::var(&i)),
                arguments: vec![Expr::var(&acc)],
            }])),
            result: VarId::fresh(),
        }),
    };
    let mut session = base.clone();
    session.declare_fn(&by_let).unwrap();
    session.declare_fn(&by_for).unwrap();
    let unit = Unit {
        module: "p0".into(),
        rust: print_module(session.erased()),
        calls: vec!["p0::by_let(1)".into(), "p0::by_for(1)".into()],
    };
    let source = harness(std::slice::from_ref(&unit));
    let compiled = compile("random_bare_literal", &source, Overflow::Checked);
    remove_binaries("random_bare_literal");
    assert!(compiled.is_ok(), "{}", compiled.unwrap_err());
}
