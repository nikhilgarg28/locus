//! Tests for the table of primitive operations on the machine integer types
//! (build task K5; the kernel contract in atlas.html): `src/kernel/ops.rs`,
//! the primitive `op[T]` for each row, and the two axiom schemas `op_model`
//! and `op_exact`.
//!
//! The table is tested against Rust row by row. At 8 bits, for `u8` and
//! `i8`, every pair of operands of every row goes through the table: the
//! condition `fits_at` holds exactly when Rust's checked operation returns
//! `Some`, and the meaning `compute` equals Rust's wrapping operation, and
//! Rust's plain result where that does not panic. The kernel's `evaluate`
//! and the premises `fits` builds are checked at a sample of pairs crossed
//! with itself, and at every pair under `LOCUS_EXTENDED`. At the wider
//! types the same is done at a boundary set crossed with itself and at
//! random pairs. `min / -1`, `min % -1`, negation of `min`, and division by
//! zero have named tests. Each axiom schema is used at two types, derived
//! from the other and from the model of K4, and misused: wrong operand
//! types, wrong arity, a row that does not exist, a neighbouring type, the
//! sibling operation, the exact equation claimed without its premises, and
//! `op_exact` at a row that cannot overflow. Every term here is written by
//! hand, and Rust's side is a macro over the concrete types.

use std::rc::Rc;

use locus::kernel::derive::Chain;
use locus::kernel::{
    Axiom, Context, Definitions, Integer, KernelError, MAX_DEPTH, MAX_EVAL_DEPTH, MachineInt, Mode,
    Op, Panic, Prelude, Prim, Proof, Row, Term, Type, check_proof, infer_proof, infer_term, same,
    same_type,
};

#[path = "common/rng.rs"]
mod rng;
use rng::{Rng, case_seed};

use MachineInt::{I8, I16, I32, I64, U8, U16, U32, U64};

const ALL: [MachineInt; 8] = MachineInt::ALL;
const SIGNED: [MachineInt; 4] = [I8, I16, I32, I64];
const UNSIGNED: [MachineInt; 4] = [U8, U16, U32, U64];
const WIDER: [MachineInt; 6] = [U16, U32, U64, I16, I32, I64];

fn setup() -> (Context, Prelude) {
    let (definitions, prelude) = Definitions::with_prelude();
    (Context::with_definitions(Rc::new(definitions)), prelude)
}

fn int(value: i128) -> Term {
    Term::Int(Integer::from(value))
}

/// A literal of a machine type, which must be in range.
fn lit(ty: MachineInt, value: i128) -> Term {
    Term::machine(ty, Integer::from(value))
}

fn view(ty: MachineInt, x: Term) -> Term {
    Term::view(ty, x)
}

fn wrap(ty: MachineInt, n: Term) -> Term {
    Term::wrap(ty, n)
}

/// `op[T](operands)`.
fn app(op: Op, ty: MachineInt, operands: &[Term]) -> Term {
    Term::op(op, ty, operands.to_vec())
}

fn le(left: Term, right: Term) -> Term {
    Term::int_le(left, right)
}

fn int_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Int, left, right)
}

fn eq_at(ty: MachineInt, left: Term, right: Term) -> Term {
    Term::eq(Type::machine(ty), left, right)
}

fn implies(premise: Term, conclusion: Term) -> Term {
    Term::implies(premise, conclusion)
}

fn ax(axiom: Axiom) -> Proof {
    Proof::Axiom(axiom)
}

fn min(ty: MachineInt) -> Term {
    Term::Int(ty.min())
}

fn max(ty: MachineInt) -> Term {
    Term::Int(ty.max())
}

fn i128_of(value: &Integer) -> i128 {
    value.to_i128().expect("fits in i128")
}

fn integers(values: &[i128]) -> Vec<Integer> {
    values.iter().map(|value| Integer::from(*value)).collect()
}

fn mismatch<T: std::fmt::Debug>(result: Result<T, KernelError>) {
    assert!(
        matches!(result, Err(KernelError::ProofMismatch { .. })),
        "expected a proof mismatch, found {result:?}"
    );
}

fn ill_typed<T: std::fmt::Debug>(result: Result<T, KernelError>) {
    assert!(
        matches!(result, Err(KernelError::TypeMismatch { .. })),
        "expected a type mismatch, found {result:?}"
    );
}

/// The value `evaluate` gives a closed term of type `T`, as a number.
fn evaluated(ctx: &mut Context, term: &Term, ty: MachineInt) -> i128 {
    match infer_proof(ctx, &Proof::Evaluate(term.clone())) {
        Ok(Term::Eq(found, left, right)) => {
            assert!(same_type(&found, &Type::machine(ty)), "{term} at {found}");
            assert!(same(&left, term));
            let (found, value) = right.machine_value().unwrap_or_else(|| panic!("{right}"));
            assert_eq!(found, ty, "{term}");
            i128_of(&value)
        }
        other => panic!("evaluating {term}: {other:?}"),
    }
}

/// Whether `evaluate` proves the closed comparison, checking that it proves
/// exactly the claim or exactly its negation.
fn decided(ctx: &mut Context, prelude: &Prelude, claim: &Term) -> bool {
    match infer_proof(ctx, &Proof::Evaluate(claim.clone())) {
        Ok(proved) if same(&proved, claim) => true,
        Ok(proved) if same(&proved, &prelude.not_prop(claim.clone())) => false,
        other => panic!("deciding {claim}: {other:?}"),
    }
}

/// Whether the premises `fits` builds hold, each decided by the kernel.
/// Also checks their shapes: a comparison for a row that can overflow, and
/// for a division `view(b) == 0 => False` and, at a signed type,
/// `view(a) == min => (view(b) == -1 => False)`.
fn kernel_fits(ctx: &mut Context, prelude: &Prelude, row: Row, operands: &[Term]) -> bool {
    let premises = row.fits(prelude, operands);
    let falsehood = prelude.falsehood_prop();
    match row.panic() {
        Panic::Never => {
            assert!(premises.is_empty());
            true
        }
        Panic::Overflow => {
            assert_eq!(premises.len(), 2, "{row:?}");
            premises.iter().all(|premise| {
                assert!(matches!(premise, Term::Prim(Prim::IntLe, _)), "{premise}");
                decided(ctx, prelude, premise)
            })
        }
        Panic::Division => {
            assert_eq!(premises.len(), if row.ty.signed() { 2 } else { 1 });
            let Term::Implies(nonzero, conclusion) = &premises[0] else {
                panic!("{}", premises[0]);
            };
            assert!(same(conclusion, &falsehood));
            assert!(same(
                nonzero,
                &int_eq(view(row.ty, operands[1].clone()), int(0))
            ));
            let mut fits = !decided(ctx, prelude, nonzero);
            if row.ty.signed() {
                let Term::Implies(is_min, rest) = &premises[1] else {
                    panic!("{}", premises[1]);
                };
                let Term::Implies(is_minus_one, conclusion) = &**rest else {
                    panic!("{}", premises[1]);
                };
                assert!(same(conclusion, &falsehood));
                assert!(same(
                    is_min,
                    &int_eq(view(row.ty, operands[0].clone()), min(row.ty))
                ));
                assert!(same(
                    is_minus_one,
                    &int_eq(view(row.ty, operands[1].clone()), int(-1))
                ));
                fits =
                    fits && !(decided(ctx, prelude, is_min) && decided(ctx, prelude, is_minus_one));
            }
            fits
        }
    }
}

// --- Rust's side ------------------------------------------------------------------

/// What Rust computes for a row: the plain operation where it does not
/// panic, and the wrapping method where it exists. `checked` is `None`
/// exactly where a build with overflow checks panics; `wrapping` is `None`
/// only at a zero divisor, where every build panics and there is no
/// wrapping method to ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rust {
    checked: Option<i128>,
    wrapping: Option<i128>,
}

macro_rules! rust_row {
    ($t:ty, $op:expr, $operands:expr) => {{
        let xs: Vec<$t> = $operands
            .iter()
            .map(|value| <$t>::try_from(*value).expect("an operand of the type"))
            .collect();
        let (checked, wrapping): (Option<$t>, Option<$t>) = match ($op, xs.as_slice()) {
            (Op::Add, [a, b]) => (a.checked_add(*b), Some(a.wrapping_add(*b))),
            (Op::Sub, [a, b]) => (a.checked_sub(*b), Some(a.wrapping_sub(*b))),
            (Op::Mul, [a, b]) => (a.checked_mul(*b), Some(a.wrapping_mul(*b))),
            (Op::Div, [a, b]) => (a.checked_div(*b), (*b != 0).then(|| a.wrapping_div(*b))),
            (Op::Rem, [a, b]) => (a.checked_rem(*b), (*b != 0).then(|| a.wrapping_rem(*b))),
            (Op::Neg, [a]) => (a.checked_neg(), Some(a.wrapping_neg())),
            (Op::WrappingAdd, [a, b]) => (Some(a.wrapping_add(*b)), Some(a.wrapping_add(*b))),
            (Op::WrappingSub, [a, b]) => (Some(a.wrapping_sub(*b)), Some(a.wrapping_sub(*b))),
            (Op::WrappingMul, [a, b]) => (Some(a.wrapping_mul(*b)), Some(a.wrapping_mul(*b))),
            (Op::WrappingNeg, [a]) => (Some(a.wrapping_neg()), Some(a.wrapping_neg())),
            (op, xs) => panic!("{} at {} operands", op.name(), xs.len()),
        };
        Rust {
            checked: checked.map(i128::from),
            wrapping: wrapping.map(i128::from),
        }
    }};
}

/// Rust's results for the row at the operands, computed in the concrete
/// type.
fn rust(op: Op, ty: MachineInt, operands: &[i128]) -> Rust {
    match ty {
        U8 => rust_row!(u8, op, operands),
        U16 => rust_row!(u16, op, operands),
        U32 => rust_row!(u32, op, operands),
        U64 => rust_row!(u64, op, operands),
        I8 => rust_row!(i8, op, operands),
        I16 => rust_row!(i16, op, operands),
        I32 => rust_row!(i32, op, operands),
        I64 => rust_row!(i64, op, operands),
    }
}

/// The table's answer at the operands must be Rust's: `fits_at` is
/// `checked.is_some()`, and `compute` is the wrapping result, which is the
/// plain result wherever Rust has one; at a zero divisor, where Rust has
/// neither, it is the total value of the kernel's `Int` division, `0` for
/// `/` and the dividend for `%`. Returns the expected value.
fn table_agrees(row: Row, operands: &[i128]) -> i128 {
    let values = integers(operands);
    let expected = rust(row.op, row.ty, operands);
    // Formatted only on failure: this runs for every pair of bytes.
    let name = || format!("{}[{}]{operands:?}", row.op.name(), row.ty.name());
    assert_eq!(
        row.fits_at(&values),
        expected.checked.is_some(),
        "{}",
        name()
    );
    let found = i128_of(&row.compute(&values));
    match expected.wrapping {
        Some(wrapping) => assert_eq!(found, wrapping, "{}", name()),
        None => {
            assert_eq!(operands[1], 0, "{}", name());
            let total = if row.op == Op::Div { 0 } else { operands[0] };
            assert_eq!(found, total, "{}", name());
        }
    }
    if let Some(plain) = expected.checked {
        assert_eq!(found, plain, "{}", name());
    }
    found
}

/// The kernel's answer must be the table's: `evaluate` of the applied row
/// gives the meaning, and the premises `fits` builds are decided as
/// `fits_at` says.
fn kernel_agrees(ctx: &mut Context, prelude: &Prelude, row: Row, operands: &[i128]) {
    let expected = table_agrees(row, operands);
    let terms: Vec<Term> = operands.iter().map(|value| lit(row.ty, *value)).collect();
    let term = app(row.op, row.ty, &terms);
    assert_eq!(evaluated(ctx, &term, row.ty), expected, "{term}");
    assert_eq!(
        kernel_fits(ctx, prelude, row, &terms),
        row.fits_at(&integers(operands)),
        "{term}"
    );
}

fn scale() -> u64 {
    if std::env::var_os("LOCUS_EXTENDED").is_some_and(|value| !value.is_empty()) {
        100
    } else {
        1
    }
}

fn every_8_bit(ty: MachineInt) -> Vec<i128> {
    (i128_of(&ty.min())..=i128_of(&ty.max())).collect()
}

/// A sample of a type's values: the ends of the range and their
/// neighbours, zero and its neighbours, and a few powers of two and their
/// neighbours, in range.
fn sample(ty: MachineInt) -> Vec<i128> {
    let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
    let mut values = vec![
        0,
        1,
        2,
        3,
        -1,
        -2,
        -3,
        lo,
        lo + 1,
        lo + 2,
        hi,
        hi - 1,
        hi - 2,
    ];
    let bits = ty.bits();
    for k in [
        1,
        2,
        bits / 2 - 1,
        bits / 2,
        bits / 2 + 1,
        bits - 2,
        bits - 1,
    ] {
        let power = 1i128 << k;
        values.extend([power - 1, power, power + 1, -power - 1, -power, -power + 1]);
    }
    values.retain(|value| lo <= *value && *value <= hi);
    values.sort_unstable();
    values.dedup();
    values
}

/// The operand lists of a row over a set of values: pairs for a binary
/// row, single values for a negation.
fn operand_lists(row: Row, values: &[i128]) -> Vec<Vec<i128>> {
    if row.arity() == 1 {
        values.iter().map(|a| vec![*a]).collect()
    } else {
        values
            .iter()
            .flat_map(|a| values.iter().map(move |b| vec![*a, *b]))
            .collect()
    }
}

fn rows_at(ty: MachineInt) -> Vec<Row> {
    Op::ALL.iter().filter_map(|op| op.row(ty)).collect()
}

// --- The table ----------------------------------------------------------------------

#[test]
#[doc = "spec: 2.17:1, 2.17:2, 2.17:3"]
fn the_table_has_the_rows_it_says_and_no_others() {
    // Ten operations at each signed type, eight at each unsigned one, and
    // `Row::all` lists exactly those.
    let all = Row::all();
    assert_eq!(all.len(), 4 * 10 + 4 * 8);
    for ty in ALL {
        for op in Op::ALL {
            let exists = ty.signed() || !matches!(op, Op::Neg | Op::WrappingNeg);
            assert_eq!(op.exists_at(ty), exists, "{}[{}]", op.name(), ty.name());
            assert_eq!(op.row(ty).is_some(), exists);
            assert_eq!(all.contains(&Row { op, ty }), exists);
        }
    }
    // Row by row: what panics, and whether a build without overflow checks
    // wraps instead. Only the overflow of + - * and unary minus wraps; the
    // wrapping methods never panic; / and % panic in every build.
    for row in all {
        let (panic, wraps, arity) = match row.op {
            Op::Add | Op::Sub | Op::Mul => (Panic::Overflow, true, 2),
            Op::Neg => (Panic::Overflow, true, 1),
            Op::Div | Op::Rem => (Panic::Division, false, 2),
            Op::WrappingAdd | Op::WrappingSub | Op::WrappingMul => (Panic::Never, false, 2),
            Op::WrappingNeg => (Panic::Never, false, 1),
        };
        assert_eq!(row.panic(), panic, "{row:?}");
        assert_eq!(row.wraps_instead(), wraps, "{row:?}");
        assert_eq!(row.arity(), arity, "{row:?}");
        assert_eq!(row.op.arity(), arity);
    }
    assert_eq!(Op::ALL.len(), 10);
    let names: Vec<&str> = Op::ALL.iter().map(|op| op.name()).collect();
    assert_eq!(
        names,
        [
            "add",
            "sub",
            "mul",
            "div",
            "rem",
            "neg",
            "wrapping_add",
            "wrapping_sub",
            "wrapping_mul",
            "wrapping_neg"
        ]
    );
    let symbols: Vec<&str> = Op::ALL.iter().map(|op| op.symbol()).collect();
    assert_eq!(&symbols[..6], ["+", "-", "*", "/", "%", "-"]);
}

#[test]
#[doc = "spec: 2.4:1"]
fn a_row_is_typed_at_its_type_in_both_modes_and_a_missing_row_is_rejected() {
    let (mut ctx, _) = setup();
    let vars: Vec<(MachineInt, Term, Term)> = ALL
        .iter()
        .map(|ty| {
            (
                *ty,
                Term::var(ctx.declare(Type::machine(*ty)).unwrap()),
                Term::var(ctx.declare(Type::machine(*ty)).unwrap()),
            )
        })
        .collect();
    let ghost = Term::var(ctx.declare_ghost(Type::Int).unwrap());
    for (ty, a, b) in &vars {
        for op in Op::ALL {
            let operands: Vec<Term> = if op.arity() == 1 {
                vec![a.clone()]
            } else {
                vec![a.clone(), b.clone()]
            };
            let term = app(op, *ty, &operands);
            let Some(row) = op.row(*ty) else {
                // No row: rejected in either mode, whatever the operands.
                for mode in [Mode::Logical, Mode::Executable] {
                    assert_eq!(
                        infer_term(&mut ctx, &term, mode),
                        Err(KernelError::NoRow(op, *ty)),
                        "{term}"
                    );
                }
                assert_eq!(
                    infer_term(&mut ctx, &app(op, *ty, &[]), Mode::Logical),
                    Err(KernelError::NoRow(op, *ty))
                );
                continue;
            };
            // Runtime data in, runtime data out: typed in both modes.
            for mode in [Mode::Logical, Mode::Executable] {
                assert_eq!(
                    infer_term(&mut ctx, &term, mode),
                    Ok(Type::machine(*ty)),
                    "{term}"
                );
            }
            assert_eq!(row.arity(), operands.len());
            // Wrong arity.
            let mut extra = operands.clone();
            extra.push(a.clone());
            assert_eq!(
                infer_term(&mut ctx, &app(op, *ty, &extra), Mode::Logical),
                Err(KernelError::WrongArity {
                    expected: row.arity(),
                    found: row.arity() + 1
                })
            );
            assert_eq!(
                infer_term(
                    &mut ctx,
                    &app(op, *ty, &operands[..row.arity() - 1]),
                    Mode::Logical
                ),
                Err(KernelError::WrongArity {
                    expected: row.arity(),
                    found: row.arity() - 1
                })
            );
            // An operand of another type, of every other machine type and
            // of `Int`; and the row at another type applied to these.
            for (other_ty, other_a, _) in &vars {
                if other_ty == ty {
                    continue;
                }
                let mut wrong = operands.clone();
                wrong[row.arity() - 1] = other_a.clone();
                ill_typed(infer_term(&mut ctx, &app(op, *ty, &wrong), Mode::Logical));
                if op.exists_at(*other_ty) {
                    ill_typed(infer_term(
                        &mut ctx,
                        &app(op, *other_ty, &operands),
                        Mode::Logical,
                    ));
                }
            }
            let mut wrong = operands.clone();
            wrong[0] = ghost.clone();
            ill_typed(infer_term(&mut ctx, &app(op, *ty, &wrong), Mode::Logical));
            let mut wrong = operands.clone();
            wrong[0] = view(*ty, a.clone());
            ill_typed(infer_term(&mut ctx, &app(op, *ty, &wrong), Mode::Logical));
            // The result is data: it can be viewed, and compared at its type.
            assert_eq!(
                infer_term(&mut ctx, &view(*ty, term.clone()), Mode::Logical),
                Ok(Type::Int)
            );
            assert_eq!(
                infer_term(
                    &mut ctx,
                    &eq_at(*ty, term.clone(), a.clone()),
                    Mode::Logical
                ),
                Ok(Type::Prop)
            );
        }
    }
    // Names and display.
    assert_eq!(Prim::Op(Op::Add, U16).name(), "add");
    assert_eq!(Prim::Op(Op::WrappingNeg, I8).name(), "wrapping_neg");
    assert_eq!(
        app(Op::Add, U16, &[lit(U16, 1), lit(U16, 2)]).to_string(),
        "add[u16](1u16, 2u16)"
    );
    assert_eq!(
        app(Op::Neg, I8, &[lit(I8, -1)]).to_string(),
        "neg[i8](-1i8)"
    );
    assert_eq!(
        app(Op::WrappingAdd, U8, &[Term::U8(1), Term::U8(2)]).to_string(),
        "wrapping_add[u8](1, 2)"
    );
    // A literal out of range inside a row is rejected where it is typed.
    let bad = Term::Machine(U16, Integer::from(70000i64));
    assert_eq!(
        infer_term(
            &mut ctx,
            &app(Op::Add, U16, &[bad.clone(), lit(U16, 1)]),
            Mode::Logical
        ),
        Err(KernelError::OutOfRange(bad))
    );
}

#[test]
#[doc = "spec: 2.17:5"]
fn the_literal_axiom_computes_one_row_on_literals() {
    let (mut ctx, _) = setup();
    let x = Term::var(ctx.declare(Type::machine(U16)).unwrap());
    for (term, value) in [
        (
            app(Op::Add, U16, &[lit(U16, 65535), lit(U16, 1)]),
            lit(U16, 0),
        ),
        (
            app(Op::Add, U8, &[Term::U8(200), Term::U8(100)]),
            Term::U8(44),
        ),
        (
            app(Op::Sub, U32, &[lit(U32, 0), lit(U32, 1)]),
            lit(U32, u32::MAX.into()),
        ),
        (app(Op::Mul, I8, &[lit(I8, 16), lit(I8, 16)]), lit(I8, 0)),
        (
            app(Op::Div, I8, &[lit(I8, -128), lit(I8, -1)]),
            lit(I8, -128),
        ),
        (app(Op::Rem, I8, &[lit(I8, -128), lit(I8, -1)]), lit(I8, 0)),
        (
            app(Op::Div, I64, &[lit(I64, -7), lit(I64, 2)]),
            lit(I64, -3),
        ),
        (
            app(Op::Rem, I64, &[lit(I64, -7), lit(I64, 2)]),
            lit(I64, -1),
        ),
        (app(Op::Div, U8, &[Term::U8(7), Term::U8(0)]), Term::U8(0)),
        (app(Op::Rem, U8, &[Term::U8(7), Term::U8(0)]), Term::U8(7)),
        (
            app(Op::Neg, I32, &[lit(I32, i32::MIN.into())]),
            lit(I32, i32::MIN.into()),
        ),
        (app(Op::Neg, I32, &[lit(I32, 5)]), lit(I32, -5)),
        (
            app(Op::WrappingAdd, I16, &[lit(I16, 32767), lit(I16, 1)]),
            lit(I16, -32768),
        ),
        (
            app(Op::WrappingSub, U64, &[lit(U64, 0), lit(U64, 1)]),
            lit(U64, u64::MAX.into()),
        ),
        (
            app(Op::WrappingMul, U16, &[lit(U16, 256), lit(U16, 256)]),
            lit(U16, 0),
        ),
        (app(Op::WrappingNeg, I8, &[lit(I8, -128)]), lit(I8, -128)),
    ] {
        let ty = value.machine_value().unwrap().0;
        assert_eq!(
            infer_proof(&mut ctx, &Proof::Literal(term.clone())),
            Ok(eq_at(ty, term, value))
        );
    }
    // No step: a non-literal operand, a literal of another type, a missing
    // row, or the wrong number of literals.
    for stuck in [
        app(Op::Add, U16, &[x.clone(), lit(U16, 1)]),
        app(
            Op::Add,
            U16,
            &[lit(U16, 1), app(Op::Add, U16, &[lit(U16, 1), lit(U16, 1)])],
        ),
        app(Op::Add, U16, &[lit(U16, 1), lit(I16, 1)]),
        app(Op::Add, U16, &[lit(U16, 1), int(1)]),
        app(Op::Neg, U16, &[lit(U16, 1)]),
        app(Op::Neg, I16, &[lit(I16, 1), lit(I16, 1)]),
        app(Op::Add, I16, &[lit(I16, 1)]),
    ] {
        let result = infer_proof(&mut ctx, &Proof::Literal(stuck.clone()));
        assert!(
            matches!(
                result,
                Err(KernelError::NoComputationStep(_))
                    | Err(KernelError::TypeMismatch { .. })
                    | Err(KernelError::NoRow(..))
                    | Err(KernelError::WrongArity { .. })
            ),
            "{stuck}: {result:?}"
        );
    }
}

// --- Against Rust, at 8 bits ---------------------------------------------------------

#[test]
#[doc = "spec: 2.17:11, 2.17:4, 2.17:9"]
fn every_pair_at_8_bits_agrees_with_rust_row_by_row() {
    let (mut ctx, prelude) = setup();
    let mut through_table = 0u64;
    let mut through_kernel = 0u64;
    for ty in [U8, I8] {
        let all = every_8_bit(ty);
        let sampled = sample(ty);
        for row in rows_at(ty) {
            // Every pair through the table.
            for operands in operand_lists(row, &all) {
                table_agrees(row, &operands);
                through_table += 1;
            }
            // Through the kernel: the sample crossed with itself, or every
            // pair under LOCUS_EXTENDED.
            let values = if scale() > 1 { &all } else { &sampled };
            for operands in operand_lists(row, values) {
                kernel_agrees(&mut ctx, &prelude, row, &operands);
                through_kernel += 1;
            }
        }
    }
    // 8 rows at u8 and 10 at i8, with one negation per type.
    assert_eq!(through_table, (7 + 9) * 256 * 256 + 2 * 256);
    assert!(through_kernel >= 16 * 20 * 20, "{through_kernel}");
}

#[test]
fn the_wrapping_rows_at_u8_agree_with_the_primitives_of_the_u8_model() {
    let (mut ctx, _) = setup();
    let add = Op::WrappingAdd.row(U8).unwrap();
    let sub = Op::WrappingSub.row(U8).unwrap();
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let operands = integers(&[a.into(), b.into()]);
            assert_eq!(i128_of(&add.compute(&operands)), a.wrapping_add(b).into());
            assert_eq!(i128_of(&sub.compute(&operands)), a.wrapping_sub(b).into());
        }
    }
    // And in the kernel, the two primitives evaluate alike, at every pair
    // under LOCUS_EXTENDED and at a sample otherwise.
    let values: Vec<u8> = if scale() > 1 {
        (0..=255).collect()
    } else {
        vec![0, 1, 2, 7, 100, 127, 128, 200, 254, 255]
    };
    for a in &values {
        for b in &values {
            let (ta, tb) = (Term::U8(*a), Term::U8(*b));
            let row = app(Op::WrappingAdd, U8, &[ta.clone(), tb.clone()]);
            let model = Term::op(
                Op::WrappingAdd,
                MachineInt::U8,
                vec![ta.clone(), tb.clone()],
            );
            assert_eq!(evaluated(&mut ctx, &row, U8), a.wrapping_add(*b).into());
            assert_eq!(evaluated(&mut ctx, &model, U8), a.wrapping_add(*b).into());
            let row = app(Op::WrappingSub, U8, &[ta.clone(), tb.clone()]);
            let model = Term::op(Op::WrappingSub, MachineInt::U8, vec![ta, tb]);
            assert_eq!(evaluated(&mut ctx, &row, U8), a.wrapping_sub(*b).into());
            assert_eq!(evaluated(&mut ctx, &model, U8), a.wrapping_sub(*b).into());
        }
    }
    // The kernel proves them equal on closed values, by evaluating each.
    let (ta, tb) = (Term::U8(200), Term::U8(100));
    let row = app(Op::WrappingAdd, U8, &[ta.clone(), tb.clone()]);
    let model = Term::op(Op::WrappingAdd, MachineInt::U8, vec![ta, tb]);
    let proof = Chain::new(Type::U8, row.clone())
        .step(Proof::Evaluate(row.clone()))
        .step_rev(&model, Proof::Evaluate(model.clone()))
        .finish();
    assert_eq!(
        check_proof(&mut ctx, &proof, &eq_at(U8, row, model)),
        Ok(())
    );
}

// --- Against Rust, at the wider types --------------------------------------------------

#[test]
fn the_boundaries_of_every_wider_type_crossed_with_themselves_agree_with_rust() {
    let (mut ctx, prelude) = setup();
    let mut checked = 0u64;
    for ty in WIDER {
        let values = sample(ty);
        assert!(values.len() >= 20, "{}: {}", ty.name(), values.len());
        for row in rows_at(ty) {
            for operands in operand_lists(row, &values) {
                table_agrees(row, &operands);
                checked += 1;
            }
        }
    }
    assert!(checked > 6 * 8 * 20 * 20, "{checked}");
    // Through the kernel: the ends of the range and their neighbours, zero
    // and its neighbours, crossed, for every row of every wider type.
    let mut through_kernel = 0u64;
    for ty in WIDER {
        let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
        let mut values = vec![0, 1, 2, -1, -2, lo, lo + 1, hi, hi - 1];
        values.retain(|value| lo <= *value && *value <= hi);
        values.dedup();
        for row in rows_at(ty) {
            for operands in operand_lists(row, &values) {
                kernel_agrees(&mut ctx, &prelude, row, &operands);
                through_kernel += 1;
            }
        }
    }
    assert!(through_kernel > 6 * 8 * 7 * 7, "{through_kernel}");
}

/// A random value of the type: a boundary value one time in four, a
/// uniformly random one otherwise.
fn random_value(rng: &mut Rng, ty: MachineInt) -> i128 {
    if rng.chance(1, 4) {
        return *rng.choose(&sample(ty));
    }
    let raw = rng.next_u64();
    let value = match ty {
        U8 => i128::from(raw as u8),
        U16 => i128::from(raw as u16),
        U32 => i128::from(raw as u32),
        U64 => i128::from(raw),
        I8 => i128::from(raw as i8),
        I16 => i128::from(raw as i16),
        I32 => i128::from(raw as i32),
        I64 => i128::from(raw as i64),
    };
    assert!(ty.contains(&Integer::from(value)));
    value
}

#[test]
fn random_pairs_at_every_type_agree_with_rust() {
    const SEED: u64 = 0x4b35_0000_0000_0005;
    let (mut ctx, prelude) = setup();
    let cases = 40 * scale();
    let mut checked = 0u64;
    for index in 0..cases {
        let mut rng = Rng::new(case_seed(SEED, index));
        for ty in ALL {
            for row in rows_at(ty) {
                let operands: Vec<i128> = (0..row.arity())
                    .map(|_| random_value(&mut rng, ty))
                    .collect();
                // Random pairs rarely overflow at the wider types; a
                // narrowed pair sometimes does.
                let operands: Vec<i128> = if rng.chance(1, 3) {
                    let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
                    operands
                        .iter()
                        .map(|value| (*value).clamp(lo, hi) - rng.below(4) as i128)
                        .map(|value| value.clamp(lo, hi))
                        .collect()
                } else {
                    operands
                };
                kernel_agrees(&mut ctx, &prelude, row, &operands);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, cases * 72);
}

// --- The named pairs ----------------------------------------------------------------

#[test]
fn min_over_minus_one_panics_at_every_signed_type_and_wraps_to_min() {
    let (mut ctx, prelude) = setup();
    for ty in SIGNED {
        let lo = i128_of(&ty.min());
        let (a, b) = (lit(ty, lo), lit(ty, -1));
        let div = Op::Div.row(ty).unwrap();
        let rem = Op::Rem.row(ty).unwrap();
        let operands = integers(&[lo, -1]);
        // Rust panics, in every build; the table says so.
        assert_eq!(rust(Op::Div, ty, &[lo, -1]).checked, None);
        assert_eq!(rust(Op::Rem, ty, &[lo, -1]).checked, None);
        assert!(!div.fits_at(&operands), "{}", ty.name());
        assert!(!rem.fits_at(&operands), "{}", ty.name());
        assert!(!div.wraps_instead() && !rem.wraps_instead());
        assert!(!kernel_fits(
            &mut ctx,
            &prelude,
            div,
            &[a.clone(), b.clone()]
        ));
        assert!(!kernel_fits(
            &mut ctx,
            &prelude,
            rem,
            &[a.clone(), b.clone()]
        ));
        // The meaning in every build is the wrapped quotient, `min`, which
        // is what `wrapping_div` gives, and the remainder `0`.
        assert_eq!(i128_of(&div.compute(&operands)), lo);
        assert_eq!(i128_of(&rem.compute(&operands)), 0);
        assert_eq!(rust(Op::Div, ty, &[lo, -1]).wrapping, Some(lo));
        assert_eq!(rust(Op::Rem, ty, &[lo, -1]).wrapping, Some(0));
        assert_eq!(
            evaluated(&mut ctx, &app(Op::Div, ty, &[a.clone(), b.clone()]), ty),
            lo
        );
        assert_eq!(
            evaluated(&mut ctx, &app(Op::Rem, ty, &[a.clone(), b.clone()]), ty),
            0
        );
        // `op_model` at this pair states `div(min, -1) == wrap(min / -1)`,
        // and `wrap(min / -1) == min` by evaluation: true of the wrapped
        // meaning, though Rust panics here, and harmless because no fact
        // about a panicking operation is ever needed.
        let claim = eq_at(
            ty,
            app(Op::Div, ty, &[a.clone(), b.clone()]),
            wrap(ty, Term::int_div(view(ty, a.clone()), view(ty, b.clone()))),
        );
        assert_eq!(
            check_proof(
                &mut ctx,
                &ax(Axiom::OpModel(Op::Div, ty, vec![a.clone(), b.clone()])),
                &claim
            ),
            Ok(())
        );
        let quotient = Term::int_div(view(ty, a.clone()), view(ty, b.clone()));
        assert_eq!(
            infer_proof(&mut ctx, &Proof::Evaluate(quotient.clone())),
            Ok(int_eq(quotient, Term::Int(ty.min().neg())))
        );
        assert_eq!(
            evaluated(&mut ctx, &wrap(ty, Term::Int(ty.min().neg())), ty),
            lo
        );
        // The neighbouring pairs do not panic: `min / 1`, `(min + 1) / -1`.
        assert!(div.fits_at(&integers(&[lo, 1])));
        assert!(div.fits_at(&integers(&[lo + 1, -1])));
        assert!(rem.fits_at(&integers(&[lo + 1, -1])));
        assert_eq!(i128_of(&div.compute(&integers(&[lo + 1, -1]))), -(lo + 1));
    }
    // At an unsigned type `-1` is not a value, and there is no such pair:
    // the condition on a division is only that the divisor is nonzero.
    for ty in UNSIGNED {
        let hi = i128_of(&ty.max());
        let div = Op::Div.row(ty).unwrap();
        assert!(div.fits_at(&integers(&[0, hi])));
        assert!(div.fits_at(&integers(&[hi, hi])));
        assert_eq!(div.fits(&prelude, &[lit(ty, 0), lit(ty, hi)]).len(), 1);
    }
}

#[test]
fn division_by_zero_panics_in_every_build_at_every_type() {
    let (mut ctx, prelude) = setup();
    for ty in ALL {
        let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
        for a in [lo, 0, 1, hi] {
            for op in [Op::Div, Op::Rem] {
                let row = op.row(ty).unwrap();
                let operands = integers(&[a, 0]);
                assert!(
                    !row.fits_at(&operands),
                    "{}[{}]({a}, 0)",
                    op.name(),
                    ty.name()
                );
                assert!(!row.wraps_instead());
                assert_eq!(
                    rust(op, ty, &[a, 0]),
                    Rust {
                        checked: None,
                        wrapping: None
                    }
                );
                let terms = [lit(ty, a), lit(ty, 0)];
                assert!(!kernel_fits(&mut ctx, &prelude, row, &terms));
                // The total meaning: `0` for `/`, the dividend for `%`.
                let total = if op == Op::Div { 0 } else { a };
                assert_eq!(i128_of(&row.compute(&operands)), total);
                assert_eq!(evaluated(&mut ctx, &app(op, ty, &terms), ty), total);
            }
        }
        // A nonzero divisor fits, whatever the dividend, apart from the
        // one signed pair.
        for b in [1, hi] {
            assert!(Op::Div.row(ty).unwrap().fits_at(&integers(&[lo, b])));
            assert!(Op::Rem.row(ty).unwrap().fits_at(&integers(&[hi, b])));
        }
    }
}

#[test]
fn negating_min_panics_at_every_signed_type_and_wraps_to_min() {
    let (mut ctx, prelude) = setup();
    for ty in SIGNED {
        let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
        let neg = Op::Neg.row(ty).unwrap();
        let wrapping = Op::WrappingNeg.row(ty).unwrap();
        // Unary minus: panics at `min` and nowhere else, and wraps to `min`
        // in a build without overflow checks; the table says the panic can
        // wrap.
        assert!(!neg.fits_at(&integers(&[lo])), "{}", ty.name());
        assert!(neg.wraps_instead());
        assert_eq!(
            rust(Op::Neg, ty, &[lo]),
            Rust {
                checked: None,
                wrapping: Some(lo)
            }
        );
        assert_eq!(i128_of(&neg.compute(&integers(&[lo]))), lo);
        assert_eq!(
            evaluated(&mut ctx, &app(Op::Neg, ty, &[lit(ty, lo)]), ty),
            lo
        );
        assert!(!kernel_fits(&mut ctx, &prelude, neg, &[lit(ty, lo)]));
        for a in [lo + 1, -1, 0, 1, hi] {
            assert!(neg.fits_at(&integers(&[a])), "{}: -{a}", ty.name());
            assert_eq!(i128_of(&neg.compute(&integers(&[a]))), -a);
            assert!(kernel_fits(&mut ctx, &prelude, neg, &[lit(ty, a)]));
        }
        // `wrapping_neg`: never panics, and `min` is its own negation.
        assert_eq!(wrapping.panic(), Panic::Never);
        assert!(!wrapping.wraps_instead());
        for a in [lo, lo + 1, -1, 0, 1, hi] {
            assert!(wrapping.fits_at(&integers(&[a])));
            assert!(wrapping.fits(&prelude, &[lit(ty, a)]).is_empty());
            let expected = if a == lo { lo } else { -a };
            assert_eq!(i128_of(&wrapping.compute(&integers(&[a]))), expected);
            assert_eq!(
                evaluated(&mut ctx, &app(Op::WrappingNeg, ty, &[lit(ty, a)]), ty),
                expected
            );
        }
        // The premises of `op_exact` for the negation are the bounds on
        // `-view(a)`; at `min` the upper one fails.
        let a = lit(ty, lo);
        let premises = neg.fits(&prelude, std::slice::from_ref(&a));
        assert_eq!(
            premises,
            vec![
                le(min(ty), Term::int_neg(view(ty, a.clone()))),
                le(Term::int_neg(view(ty, a.clone())), max(ty)),
            ]
        );
        assert!(decided(&mut ctx, &prelude, &premises[0]));
        assert!(!decided(&mut ctx, &prelude, &premises[1]));
    }
    // No negation at an unsigned type: no row, no meaning, no axiom.
    for ty in UNSIGNED {
        for op in [Op::Neg, Op::WrappingNeg] {
            assert!(op.row(ty).is_none());
            let term = app(op, ty, &[lit(ty, 1)]);
            assert_eq!(
                infer_term(&mut ctx, &term, Mode::Logical),
                Err(KernelError::NoRow(op, ty))
            );
            assert_eq!(
                infer_proof(&mut ctx, &Proof::Evaluate(term)),
                Err(KernelError::NoRow(op, ty))
            );
            assert_eq!(
                infer_proof(&mut ctx, &ax(Axiom::OpModel(op, ty, vec![lit(ty, 1)]))),
                Err(KernelError::NoRow(op, ty))
            );
            assert_eq!(
                infer_proof(&mut ctx, &ax(Axiom::OpExact(op, ty, vec![lit(ty, 1)]))),
                Err(KernelError::NoRow(op, ty))
            );
        }
    }
}

#[test]
fn overflow_of_plus_minus_times_wraps_and_the_table_says_where() {
    // At each type, the pair just past each end of the range for + - *,
    // against Rust: the checked operation is None, the wrapping one is the
    // reduction, and the table agrees. This is what "wrapping mode" of the
    // interpreters will consult.
    let (mut ctx, prelude) = setup();
    for ty in ALL {
        let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
        let pairs: Vec<(Op, i128, i128)> = vec![
            (Op::Add, hi, 1),
            (Op::Add, 1, hi),
            (Op::Sub, lo, 1),
            (Op::Mul, hi, 2),
            (Op::Mul, 2, hi),
            (Op::Add, hi, hi),
            (Op::Mul, hi, hi),
        ];
        for (op, a, b) in pairs {
            let row = op.row(ty).unwrap();
            let expected = rust(op, ty, &[a, b]);
            assert_eq!(
                expected.checked,
                None,
                "{}[{}]({a}, {b})",
                op.name(),
                ty.name()
            );
            assert!(!row.fits_at(&integers(&[a, b])));
            assert!(row.wraps_instead());
            assert_eq!(
                Some(i128_of(&row.compute(&integers(&[a, b])))),
                expected.wrapping
            );
            kernel_agrees(&mut ctx, &prelude, row, &[a, b]);
            // The wrapping method at the same pair has the same value and
            // no condition.
            let wrapping = match op {
                Op::Add => Op::WrappingAdd,
                Op::Sub => Op::WrappingSub,
                _ => Op::WrappingMul,
            }
            .row(ty)
            .unwrap();
            assert!(wrapping.fits_at(&integers(&[a, b])));
            assert_eq!(
                wrapping.compute(&integers(&[a, b])),
                row.compute(&integers(&[a, b]))
            );
        }
        // And just inside: no panic, exact.
        for (op, a, b) in [
            (Op::Add, hi - 1, 1),
            (Op::Sub, lo + 1, 1),
            (Op::Mul, hi / 2, 2),
        ] {
            let row = op.row(ty).unwrap();
            assert!(row.fits_at(&integers(&[a, b])));
            kernel_agrees(&mut ctx, &prelude, row, &[a, b]);
        }
    }
}

// --- The axioms ------------------------------------------------------------------------

/// The exact result of a row on the views, written here from the operation
/// and not read from the table.
fn exact_by_hand(op: Op, ty: MachineInt, operands: &[Term]) -> Term {
    let v = |i: usize| view(ty, operands[i].clone());
    match op {
        Op::Add | Op::WrappingAdd => Term::int_add(v(0), v(1)),
        Op::Sub | Op::WrappingSub => Term::int_sub(v(0), v(1)),
        Op::Mul | Op::WrappingMul => Term::int_mul(v(0), v(1)),
        Op::Div => Term::int_div(v(0), v(1)),
        Op::Rem => Term::int_rem(v(0), v(1)),
        Op::Neg | Op::WrappingNeg => Term::int_neg(v(0)),
    }
}

/// Executable variables of each machine type, two per type, and a ghost
/// integer.
struct Vars {
    machine: Vec<(MachineInt, Term, Term)>,
    int: Term,
}

impl Vars {
    fn new(ctx: &mut Context) -> Self {
        let machine = ALL
            .iter()
            .map(|ty| {
                (
                    *ty,
                    Term::var(ctx.declare(Type::machine(*ty)).unwrap()),
                    Term::var(ctx.declare(Type::machine(*ty)).unwrap()),
                )
            })
            .collect();
        Self {
            machine,
            int: Term::var(ctx.declare_ghost(Type::Int).unwrap()),
        }
    }

    fn at(&self, ty: MachineInt, arity: usize) -> Vec<Term> {
        let (_, a, b) = self.machine.iter().find(|(t, _, _)| *t == ty).unwrap();
        if arity == 1 {
            vec![a.clone()]
        } else {
            vec![a.clone(), b.clone()]
        }
    }
}

#[test]
#[doc = "spec: 2.17:6, 2.17:7, 2.17:8"]
fn op_model_states_the_wrapped_meaning_of_every_row() {
    let (mut ctx, _) = setup();
    let v = Vars::new(&mut ctx);
    for row in Row::all() {
        let (op, ty) = (row.op, row.ty);
        let operands = v.at(ty, row.arity());
        let axiom = Axiom::OpModel(op, ty, operands.clone());
        assert_eq!(axiom.name(), "op_model");
        assert_eq!(axiom.terms(), operands.iter().collect::<Vec<_>>());
        let statement = eq_at(
            ty,
            app(op, ty, &operands),
            wrap(ty, exact_by_hand(op, ty, &operands)),
        );
        assert_eq!(
            infer_proof(&mut ctx, &ax(axiom.clone())),
            Ok(statement.clone())
        );
        assert_eq!(check_proof(&mut ctx, &ax(axiom), &statement), Ok(()));
        // Near misses. The sibling operation at the same operands states
        // something else.
        let other = Op::ALL
            .iter()
            .copied()
            .find(|other| *other != op && other.arity() == op.arity() && other.exists_at(ty))
            .unwrap();
        let sibling = infer_proof(&mut ctx, &ax(Axiom::OpModel(other, ty, operands.clone())));
        assert!(
            sibling.is_ok() && sibling != Ok(statement.clone()),
            "{sibling:?}"
        );
        mismatch(check_proof(
            &mut ctx,
            &ax(Axiom::OpModel(other, ty, operands.clone())),
            &statement,
        ));
        // The operands exchanged state something else, for a binary row.
        if row.arity() == 2 {
            let swapped = vec![operands[1].clone(), operands[0].clone()];
            mismatch(check_proof(
                &mut ctx,
                &ax(Axiom::OpModel(op, ty, swapped)),
                &statement,
            ));
        }
        // Wrong arity.
        let mut extra = operands.clone();
        extra.push(operands[0].clone());
        assert_eq!(
            infer_proof(&mut ctx, &ax(Axiom::OpModel(op, ty, extra))),
            Err(KernelError::WrongArity {
                expected: row.arity(),
                found: row.arity() + 1
            })
        );
        assert_eq!(
            infer_proof(&mut ctx, &ax(Axiom::OpModel(op, ty, Vec::new()))),
            Err(KernelError::WrongArity {
                expected: row.arity(),
                found: 0
            })
        );
        // An operand of another type, or the row at another type.
        for (other_ty, other_a, _) in &v.machine {
            if *other_ty == ty {
                continue;
            }
            let mut wrong = operands.clone();
            wrong[0] = other_a.clone();
            ill_typed(infer_proof(&mut ctx, &ax(Axiom::OpModel(op, ty, wrong))));
            if op.exists_at(*other_ty) {
                ill_typed(infer_proof(
                    &mut ctx,
                    &ax(Axiom::OpModel(op, *other_ty, operands.clone())),
                ));
            }
        }
        let mut wrong = operands.clone();
        wrong[0] = v.int.clone();
        ill_typed(infer_proof(&mut ctx, &ax(Axiom::OpModel(op, ty, wrong))));
        // The unwrapped equation is not what it says: `view(op(a, b)) == e`
        // needs `op_exact` and its premises.
        mismatch(check_proof(
            &mut ctx,
            &ax(Axiom::OpModel(op, ty, operands.clone())),
            &int_eq(
                view(ty, app(op, ty, &operands)),
                exact_by_hand(op, ty, &operands),
            ),
        ));
        // An axiom is an instance at terms of the context, not a schema.
        let mut dangling = operands.clone();
        dangling[0] = Term::Bound(0);
        assert_eq!(
            infer_proof(&mut ctx, &ax(Axiom::OpModel(op, ty, dangling))),
            Err(KernelError::DanglingBound)
        );
    }
}

#[test]
#[doc = "spec: 2.17:7"]
fn op_exact_states_the_exact_result_under_its_premises_for_the_rows_that_overflow() {
    let (mut ctx, _) = setup();
    let v = Vars::new(&mut ctx);
    for row in Row::all() {
        let (op, ty) = (row.op, row.ty);
        let operands = v.at(ty, row.arity());
        let axiom = Axiom::OpExact(op, ty, operands.clone());
        assert_eq!(axiom.name(), "op_exact");
        assert_eq!(axiom.terms(), operands.iter().collect::<Vec<_>>());
        if row.panic() != Panic::Overflow {
            // No exact statement for a wrapping method or a division.
            assert_eq!(
                infer_proof(&mut ctx, &ax(axiom)),
                Err(KernelError::NoOverflow(op, ty)),
                "{}[{}]",
                op.name(),
                ty.name()
            );
            continue;
        }
        let exact = exact_by_hand(op, ty, &operands);
        let equation = int_eq(view(ty, app(op, ty, &operands)), exact.clone());
        let statement = implies(
            le(min(ty), exact.clone()),
            implies(le(exact.clone(), max(ty)), equation.clone()),
        );
        assert_eq!(
            infer_proof(&mut ctx, &ax(axiom.clone())),
            Ok(statement.clone())
        );
        assert_eq!(
            check_proof(&mut ctx, &ax(axiom.clone()), &statement),
            Ok(())
        );
        // Near misses: the equation without its premises, with one premise,
        // with the premises exchanged, with strict bounds, and at the
        // sibling operation.
        mismatch(check_proof(&mut ctx, &ax(axiom.clone()), &equation));
        mismatch(check_proof(
            &mut ctx,
            &ax(axiom.clone()),
            &implies(le(min(ty), exact.clone()), equation.clone()),
        ));
        mismatch(check_proof(
            &mut ctx,
            &ax(axiom.clone()),
            &implies(le(exact.clone(), max(ty)), equation.clone()),
        ));
        mismatch(check_proof(
            &mut ctx,
            &ax(axiom.clone()),
            &implies(
                le(exact.clone(), max(ty)),
                implies(le(min(ty), exact.clone()), equation.clone()),
            ),
        ));
        mismatch(check_proof(
            &mut ctx,
            &ax(axiom.clone()),
            &implies(
                Term::int_lt(min(ty), exact.clone()),
                implies(Term::int_lt(exact.clone(), max(ty)), equation.clone()),
            ),
        ));
        let other = match op {
            Op::Add => Op::Sub,
            Op::Sub => Op::Mul,
            Op::Mul => Op::Add,
            _ => Op::WrappingNeg,
        };
        let sibling = infer_proof(&mut ctx, &ax(Axiom::OpExact(other, ty, operands.clone())));
        assert_ne!(sibling, Ok(statement.clone()));
        if op.arity() == 2 {
            let swapped = vec![operands[1].clone(), operands[0].clone()];
            mismatch(check_proof(
                &mut ctx,
                &ax(Axiom::OpExact(op, ty, swapped)),
                &statement,
            ));
        }
        // Wrong arity, wrong type, neighbouring type, a dangling index.
        let mut extra = operands.clone();
        extra.push(operands[0].clone());
        assert_eq!(
            infer_proof(&mut ctx, &ax(Axiom::OpExact(op, ty, extra))),
            Err(KernelError::WrongArity {
                expected: row.arity(),
                found: row.arity() + 1
            })
        );
        for (other_ty, other_a, _) in &v.machine {
            if *other_ty == ty {
                continue;
            }
            let mut wrong = operands.clone();
            wrong[0] = other_a.clone();
            ill_typed(infer_proof(&mut ctx, &ax(Axiom::OpExact(op, ty, wrong))));
            if op.exists_at(*other_ty) {
                ill_typed(infer_proof(
                    &mut ctx,
                    &ax(Axiom::OpExact(op, *other_ty, operands.clone())),
                ));
            }
        }
        let mut wrong = operands.clone();
        wrong[0] = v.int.clone();
        ill_typed(infer_proof(&mut ctx, &ax(Axiom::OpExact(op, ty, wrong))));
        let mut dangling = operands.clone();
        dangling[0] = Term::Bound(0);
        assert_eq!(
            infer_proof(&mut ctx, &ax(Axiom::OpExact(op, ty, dangling))),
            Err(KernelError::DanglingBound)
        );
    }
}

#[test]
fn op_exact_is_used_under_its_premises_and_follows_from_op_model_and_view_wrap() {
    // At two types: the exact sum from `op_exact` with the premises as
    // hypotheses, and then the same conclusion derived without `op_exact`,
    // from `op_model` and `view_wrap` of the model of K4, which shows the
    // second schema adds nothing the first and the model do not already
    // say.
    let (mut ctx, _) = setup();
    for (op, ty) in [
        (Op::Add, U16),
        (Op::Mul, I32),
        (Op::Sub, U8),
        (Op::Neg, I64),
    ] {
        let arity = op.arity();
        let operands: Vec<Term> = (0..arity)
            .map(|_| Term::var(ctx.declare(Type::machine(ty)).unwrap()))
            .collect();
        let exact = exact_by_hand(op, ty, &operands);
        let lower = ctx.assume(le(min(ty), exact.clone())).unwrap();
        let upper = ctx.assume(le(exact.clone(), max(ty))).unwrap();
        let applied = app(op, ty, &operands);
        let claim = int_eq(view(ty, applied.clone()), exact.clone());
        // By op_exact.
        let direct = Proof::implies_elim(
            Proof::implies_elim(
                ax(Axiom::OpExact(op, ty, operands.clone())),
                Proof::hyp(lower),
            ),
            Proof::hyp(upper),
        );
        assert_eq!(check_proof(&mut ctx, &direct, &claim), Ok(()));
        // Derived: view(op(xs)) == view(wrap(e)) by op_model under view, and
        // view(wrap(e)) == e by view_wrap under the premises.
        let round_trip = Proof::implies_elim(
            Proof::implies_elim(ax(Axiom::ViewWrap(ty, exact.clone())), Proof::hyp(lower)),
            Proof::hyp(upper),
        );
        let derived = Chain::new(Type::Int, view(ty, applied.clone()))
            .rewrite(
                |hole| view(ty, hole),
                ax(Axiom::OpModel(op, ty, operands.clone())),
            )
            .step(round_trip)
            .finish();
        assert_eq!(check_proof(&mut ctx, &derived, &claim), Ok(()));
        // Neither proves the claim with a premise missing.
        let half = Proof::implies_elim(
            ax(Axiom::OpExact(op, ty, operands.clone())),
            Proof::hyp(lower),
        );
        mismatch(check_proof(&mut ctx, &half, &claim));
    }
}

#[test]
#[doc = "spec: 2.17:10"]
fn op_model_proves_a_closed_operation_equal_to_its_wrapped_meaning() {
    // A use at two types on literals: op_model, then evaluation of the
    // right-hand side, gives the value; and the same value by evaluating
    // the left-hand side.
    let (mut ctx, _) = setup();
    for (op, ty, a, b, value) in [
        (Op::Add, U8, 200, 100, 44),
        (Op::Mul, I16, 256, 256, 0),
        (Op::Div, I8, -128, -1, -128),
        (Op::Rem, U32, 7, 0, 7),
    ] {
        let operands = vec![lit(ty, a), lit(ty, b)];
        let applied = app(op, ty, &operands);
        let meaning = wrap(ty, exact_by_hand(op, ty, &operands));
        let proof = Chain::new(Type::machine(ty), applied.clone())
            .step(ax(Axiom::OpModel(op, ty, operands.clone())))
            .step(Proof::Evaluate(meaning))
            .finish();
        let claim = eq_at(ty, applied.clone(), lit(ty, value));
        assert_eq!(check_proof(&mut ctx, &proof, &claim), Ok(()));
        assert_eq!(
            check_proof(&mut ctx, &Proof::Evaluate(applied), &claim),
            Ok(())
        );
        mismatch(check_proof(
            &mut ctx,
            &proof,
            &eq_at(ty, app(op, ty, &operands), lit(ty, value + 1)),
        ));
    }
}

// --- Depth ------------------------------------------------------------------------------

fn nested(op: Op, ty: MachineInt, depth: usize, x: Term) -> Term {
    (0..depth).fold(x, |inner, _| app(op, ty, &[inner, lit(ty, 1)]))
}

#[test]
fn operation_terms_are_held_to_the_depth_limits() {
    let (mut ctx, _) = setup();
    let x = Term::var(ctx.declare(Type::machine(U32)).unwrap());
    let margin = 8;
    assert_eq!(
        infer_term(
            &mut ctx,
            &nested(Op::Add, U32, MAX_DEPTH - margin, x.clone()),
            Mode::Executable
        ),
        Ok(Type::Machine(U32))
    );
    let too_deep = nested(Op::Add, U32, MAX_DEPTH + 1, x.clone());
    assert_eq!(
        infer_term(&mut ctx, &too_deep, Mode::Logical),
        Err(KernelError::TooDeep)
    );
    assert_eq!(
        infer_proof(
            &mut ctx,
            &ax(Axiom::OpModel(
                Op::Add,
                U32,
                vec![too_deep.clone(), x.clone()]
            ))
        ),
        Err(KernelError::TooDeep)
    );
    assert_eq!(
        infer_proof(
            &mut ctx,
            &ax(Axiom::OpExact(Op::Mul, U32, vec![x, too_deep]))
        ),
        Err(KernelError::TooDeep)
    );
    // Evaluation depth: each row nests one level; a chain of additions of
    // one is a count.
    let fits = nested(Op::WrappingAdd, U32, MAX_EVAL_DEPTH - margin, lit(U32, 0));
    assert_eq!(
        evaluated(&mut ctx, &fits, U32),
        (MAX_EVAL_DEPTH - margin) as i128
    );
    let too_deep = nested(Op::WrappingAdd, U32, MAX_EVAL_DEPTH + margin, lit(U32, 0));
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(too_deep)),
        Err(KernelError::EvaluationTooDeep)
    );
}
