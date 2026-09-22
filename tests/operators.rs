//! The operators `+`, `-`, `*`, `/`, `%`, and unary minus at every machine
//! type, through the typed tree (build task E6; the Vision's Integers, in
//! code and in propositions, and the Architecture in atlas.html).
//!
//! For every row of the table a function `fn f(a: T, b: T) -> T { a op b }`
//! is built as a typed tree, lowered, checked, and erased, and both
//! interpreters run it at a boundary set of the type crossed with itself,
//! in both of their modes. In the default mode, overflow checks on, the
//! outcome must be what Rust's checked operation gives: its value, or a
//! panic with Rust's message where it gives `None`. In wrapping mode the
//! outcome must be Rust's wrapping operation, except at `/` and `%`, which
//! panic in every build on a zero divisor and on `min / -1`. Rust's side is
//! a macro over the concrete types, as in `tests/kernel_ops.rs`; the
//! compiled comparison with rustc's two builds is the corpus's.
//!
//! The printer's side is here too: each operator prints as written, with
//! Rust's precedence, and a cast on the left of `<` stays parenthesized.

mod common;

use common::setup;
use locus::erased::{Interpreter, Outcome, Overflow, Value, check_module, print_module};
use locus::exec::CheckInterpreter;
use locus::kernel::{HypId, MachineInt, Op, Type, VarId};
use locus::typed::{Binder, Block, CompareOp, Expr, FnItem, FnRef, Session};

use MachineInt::{I8, I16, I32, I64, U8, U16, U32, U64};

const FUEL: u64 = 10_000;

/// What Rust computes: the checked operation, `None` where a build with
/// overflow checks panics, and the wrapping one, `None` only at a zero
/// divisor.
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
            (op, xs) => panic!("{} at {} operands", op.name(), xs.len()),
        };
        Rust {
            checked: checked.map(i128::from),
            wrapping: wrapping.map(i128::from),
        }
    }};
}

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

/// Rust's message for the panic of a row at the operands.
fn message(op: Op, operands: &[i128]) -> &'static str {
    let by_zero = operands.len() == 2 && operands[1] == 0;
    match (op, by_zero) {
        (Op::Add, _) => "attempt to add with overflow",
        (Op::Sub, _) => "attempt to subtract with overflow",
        (Op::Mul, _) => "attempt to multiply with overflow",
        (Op::Neg, _) => "attempt to negate with overflow",
        (Op::Div, true) => "attempt to divide by zero",
        (Op::Div, false) => "attempt to divide with overflow",
        (Op::Rem, true) => "attempt to calculate the remainder with a divisor of zero",
        (Op::Rem, false) => "attempt to calculate the remainder with overflow",
        _ => unreachable!(),
    }
}

fn i128_of(value: &locus::kernel::Integer) -> i128 {
    value.to_i128().expect("fits in i128")
}

/// The boundary set of a type: the ends of the range and their neighbours,
/// zero and its neighbours, and a few powers of two with theirs.
fn boundary(ty: MachineInt) -> Vec<i128> {
    let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
    let mut values = vec![0, 1, 2, -1, -2, lo, lo + 1, hi, hi - 1];
    let bits = ty.bits();
    for k in [1, bits / 2, bits - 2, bits - 1] {
        let power = 1i128 << k;
        values.extend([power - 1, power, power + 1, -power, -power + 1]);
    }
    values.retain(|value| lo <= *value && *value <= hi);
    values.sort_unstable();
    values.dedup();
    values
}

/// `fn f(a: T, b: T) -> T { a op b }`, or `fn f(a: T) -> T { -a }`, with no
/// promise, so the statement carries no evidence.
fn operator_fn(op: Op, ty: MachineInt) -> FnItem {
    let a = Binder::new("a", Type::machine(ty));
    let b = Binder::new("b", Type::machine(ty));
    let params = if op.arity() == 1 {
        vec![a.clone()]
    } else {
        vec![a.clone(), b.clone()]
    };
    let operands = params.iter().map(Expr::var).collect();
    let learned = if matches!(op, Op::Div | Op::Rem) {
        (0..if ty.signed() { 2 } else { 1 })
            .map(|_| HypId::fresh())
            .collect()
    } else {
        Vec::new()
    };
    FnItem {
        passing: Vec::new(),
        exits: Vec::new(),
        name: format!("{}_{}", op.name(), ty.name()),
        math: false,
        params,
        result: Type::machine(ty),
        body: Block {
            stmts: Vec::new(),
            tail: Some(Box::new(Expr::Operate {
                op,
                ty,
                operands,
                result: VarId::fresh(),
                equation: HypId::fresh(),
                fits: None,
                learned,
            })),
        },
    }
}

/// Both interpreters in the given mode.
fn run(session: &Session, callee: FnRef, arguments: Vec<Value>, mode: Overflow) -> [Outcome; 2] {
    let checked = CheckInterpreter::new(session.program(), FUEL)
        .with_overflow(mode)
        .call(callee, arguments.clone())
        .expect("the check IR runs");
    let erased = Interpreter::new(session.erased(), FUEL)
        .with_overflow(mode)
        .call(callee, arguments)
        .expect("the erased tree runs");
    [checked, erased]
}

#[test]
fn every_row_agrees_with_rust_in_both_modes_at_the_boundary_set() {
    let (mut session, _, _) = setup();
    let mut rows = Vec::new();
    for ty in MachineInt::ALL {
        for op in [Op::Add, Op::Sub, Op::Mul, Op::Div, Op::Rem, Op::Neg] {
            let Some(row) = op.row(ty) else {
                continue;
            };
            let reference = session
                .declare_fn(&operator_fn(op, ty))
                .unwrap_or_else(|error| panic!("{}[{}]: {error}", op.name(), ty.name()));
            rows.push((row, reference));
        }
    }
    assert_eq!(check_module(session.erased()), Ok(()));
    let mut cases = 0;
    let mut panics = 0;
    for (row, reference) in rows {
        let values = boundary(row.ty);
        let lists: Vec<Vec<i128>> = if row.arity() == 1 {
            values.iter().map(|a| vec![*a]).collect()
        } else {
            values
                .iter()
                .flat_map(|a| values.iter().map(move |b| vec![*a, *b]))
                .collect()
        };
        for operands in lists {
            let expected = rust(row.op, row.ty, &operands);
            let arguments: Vec<Value> = operands
                .iter()
                .map(|value| Value::Int(row.ty, *value))
                .collect();
            let name = || format!("{}[{}]{operands:?}", row.op.name(), row.ty.name());
            // Checks: Rust's checked operation, or Rust's panic.
            let wanted = match expected.checked {
                Some(value) => Outcome::Value(Value::Int(row.ty, value)),
                None => Outcome::Panic(message(row.op, &operands).into()),
            };
            for outcome in run(&session, reference, arguments.clone(), Overflow::Checks) {
                assert_eq!(outcome, wanted, "{} with overflow checks", name());
            }
            // Wrap: Rust's wrapping operation; a division still panics
            // where every build does.
            let wanted = match expected.wrapping {
                Some(value) if row.wraps_instead() || expected.checked.is_some() => {
                    Outcome::Value(Value::Int(row.ty, value))
                }
                _ => Outcome::Panic(message(row.op, &operands).into()),
            };
            for outcome in run(&session, reference, arguments, Overflow::Wrap) {
                assert_eq!(outcome, wanted, "{} without overflow checks", name());
            }
            cases += 1;
            panics += usize::from(expected.checked.is_none());
        }
    }
    // The set is chosen so that both outcomes occur, many times.
    assert!(cases > 4_000, "{cases} cases");
    assert!(panics > 400, "{panics} panics");
}

#[test]
fn the_named_pairs_panic_where_rust_does() {
    let (mut session, _, _) = setup();
    let byte = |value: i128| Value::Int(I8, value);
    let div = session.declare_fn(&operator_fn(Op::Div, I8)).unwrap();
    let rem = session.declare_fn(&operator_fn(Op::Rem, I8)).unwrap();
    let neg = session.declare_fn(&operator_fn(Op::Neg, I8)).unwrap();
    let add = session.declare_fn(&operator_fn(Op::Add, U8)).unwrap();
    for mode in Overflow::ALL {
        // `min / -1` and `min % -1` and a zero divisor panic in every build.
        for outcome in run(&session, div, vec![byte(-128), byte(-1)], mode) {
            assert_eq!(
                outcome,
                Outcome::Panic("attempt to divide with overflow".into())
            );
        }
        for outcome in run(&session, rem, vec![byte(-128), byte(-1)], mode) {
            assert_eq!(
                outcome,
                Outcome::Panic("attempt to calculate the remainder with overflow".into())
            );
        }
        for outcome in run(&session, div, vec![byte(7), byte(0)], mode) {
            assert_eq!(outcome, Outcome::Panic("attempt to divide by zero".into()));
        }
        for outcome in run(&session, rem, vec![byte(7), byte(0)], mode) {
            assert_eq!(
                outcome,
                Outcome::Panic("attempt to calculate the remainder with a divisor of zero".into())
            );
        }
        // Every other row panics in one build and wraps in the other.
        let expected = |wrapped: Value, message: &str| match mode {
            Overflow::Checks => Outcome::Panic(message.into()),
            Overflow::Wrap => Outcome::Value(wrapped),
        };
        for outcome in run(&session, neg, vec![byte(-128)], mode) {
            assert_eq!(
                outcome,
                expected(byte(-128), "attempt to negate with overflow")
            );
        }
        for outcome in run(
            &session,
            add,
            vec![Value::Int(U8, 255), Value::Int(U8, 1)],
            mode,
        ) {
            assert_eq!(
                outcome,
                expected(Value::Int(U8, 0), "attempt to add with overflow")
            );
        }
    }
}

/// `a op b` as an expression of the tree, at `u8`, with no evidence.
fn operate(op: Op, operands: Vec<Expr>) -> Expr {
    let learned = if matches!(op, Op::Div | Op::Rem) {
        vec![HypId::fresh()]
    } else {
        Vec::new()
    };
    Expr::Operate {
        op,
        ty: U8,
        operands,
        result: VarId::fresh(),
        equation: HypId::fresh(),
        fits: None,
        learned,
    }
}

#[test]
fn the_printer_writes_the_operators_with_rusts_precedence() {
    let (mut session, _, _) = setup();
    let a = Binder::new("a", Type::U8);
    let b = Binder::new("b", Type::U8);
    let c = Binder::new("c", Type::U8);
    let var = Expr::var;
    // (a + b) * c - a / (b % c)
    let body = operate(
        Op::Sub,
        vec![
            operate(
                Op::Mul,
                vec![operate(Op::Add, vec![var(&a), var(&b)]), var(&c)],
            ),
            operate(
                Op::Div,
                vec![var(&a), operate(Op::Rem, vec![var(&b), var(&c)])],
            ),
        ],
    );
    session
        .declare_fn(&FnItem {
            passing: Vec::new(),
            exits: Vec::new(),
            name: "mixed".into(),
            math: false,
            params: vec![a.clone(), b.clone(), c.clone()],
            result: Type::U8,
            body: Block {
                stmts: Vec::new(),
                tail: Some(Box::new(body)),
            },
        })
        .unwrap();
    // a - (b - c), and a - b - c, which differ.
    let right_nested = operate(
        Op::Sub,
        vec![var(&a), operate(Op::Sub, vec![var(&b), var(&c)])],
    );
    let left_nested = operate(
        Op::Sub,
        vec![operate(Op::Sub, vec![var(&a), var(&b)]), var(&c)],
    );
    for (name, body) in [("right_nested", right_nested), ("left_nested", left_nested)] {
        session
            .declare_fn(&FnItem {
                passing: Vec::new(),
                exits: Vec::new(),
                name: name.into(),
                math: false,
                params: vec![a.clone(), b.clone(), c.clone()],
                result: Type::U8,
                body: Block {
                    stmts: Vec::new(),
                    tail: Some(Box::new(body)),
                },
            })
            .unwrap();
    }
    // A cast on the left of `<`, which rustc would otherwise read as the
    // start of a generic argument list, and a negation of an operand that
    // is itself an operator.
    let x = Binder::new("x", Type::machine(I16));
    let y = Binder::new("y", Type::machine(I16));
    let cast_then_compare = Expr::Compare {
        op: CompareOp::Lt,
        ty: Type::machine(I16),
        left: Box::new(Expr::Cast {
            expr: Box::new(var(&a)),
            from: Type::U8,
            to: Type::machine(I16),
        }),
        right: Box::new(var(&x)),
    };
    let negated_sum = Expr::Operate {
        op: Op::Neg,
        ty: I16,
        operands: vec![Expr::Operate {
            op: Op::Add,
            ty: I16,
            operands: vec![var(&x), var(&y)],
            result: VarId::fresh(),
            equation: HypId::fresh(),
            fits: None,
            learned: Vec::new(),
        }],
        result: VarId::fresh(),
        equation: HypId::fresh(),
        fits: None,
        learned: Vec::new(),
    };
    session
        .declare_fn(&FnItem {
            passing: Vec::new(),
            exits: Vec::new(),
            name: "cast_then_compare".into(),
            math: false,
            params: vec![a.clone(), x.clone()],
            result: Type::Bool,
            body: Block {
                stmts: Vec::new(),
                tail: Some(Box::new(cast_then_compare)),
            },
        })
        .unwrap();
    session
        .declare_fn(&FnItem {
            passing: Vec::new(),
            exits: Vec::new(),
            name: "negated_sum".into(),
            math: false,
            params: vec![x.clone(), y.clone()],
            result: Type::machine(I16),
            body: Block {
                stmts: Vec::new(),
                tail: Some(Box::new(negated_sum)),
            },
        })
        .unwrap();
    let rust = print_module(session.erased());
    assert!(rust.contains("(a + b) * c - a / (b % c)\n"), "{rust}");
    assert!(rust.contains("a - (b - c)\n"), "{rust}");
    assert!(rust.contains("a - b - c\n"), "{rust}");
    assert!(rust.contains("((a as i16) < x)\n"), "{rust}");
    assert!(rust.contains("-(x + y)\n"), "{rust}");
}
