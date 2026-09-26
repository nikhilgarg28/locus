//! Tests for the model of each machine integer type over `Int` (build task
//! K4; the kernel contract in atlas.html): the type table `MachineInt`, the
//! literals, the primitives `view`, `wrap`, and `cast`, and the six axioms
//! of the schema, instantiated at each of the eight types.
//!
//! Each axiom is used at every type and misused several times: at the wrong
//! type of argument, at a neighbouring type, in the other orientation, with
//! the strict order for the weak one, without its premises, with a wrong
//! period, and with the target of a cast exchanged. `cast(T, T)(x) == x` is
//! derived inside the kernel. Evaluation is compared with Rust exhaustively
//! at 8 bits, for every value of `u8` and `i8` and every target of `as`, at
//! a boundary set for the wider types, and at random: `wrap` of integers of
//! up to two hundred bits against a reduction that uses only Rust's `as` on
//! the low limb, and casts between random pairs of types against Rust's
//! `as`. Set LOCUS_EXTENDED to run a hundred times as many random cases.
//! Every term here is written by hand.

use std::rc::Rc;

use locus::kernel::derive::Chain;
use locus::kernel::{
    Axiom, Context, Definitions, Integer, KernelError, MAX_DEPTH, MAX_EVAL_DEPTH, MachineInt, Mode,
    Prelude, Prim, Proof, Term, Type, check_proof, check_type, infer_proof, infer_term, same,
    same_type,
};

#[path = "common/rng.rs"]
mod rng;
use rng::{Rng, case_seed};

use MachineInt::{I8, I16, I32, I64, Isize32, Isize64, U8, U16, U32, U64, Usize32, Usize64};

const ALL: [MachineInt; 8] = MachineInt::FIXED;

fn setup() -> (Context, Prelude) {
    let (definitions, prelude) = Definitions::with_prelude();
    (Context::with_definitions(Rc::new(definitions)), prelude)
}

/// An `Int` literal.
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

fn cast(from: MachineInt, to: MachineInt, x: Term) -> Term {
    Term::cast(from, to, x)
}

fn add(left: Term, right: Term) -> Term {
    Term::int_add(left, right)
}

fn le(left: Term, right: Term) -> Term {
    Term::int_le(left, right)
}

fn lt(left: Term, right: Term) -> Term {
    Term::int_lt(left, right)
}

fn int_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Int, left, right)
}

/// Equality at a machine type.
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

fn modulus(ty: MachineInt) -> Term {
    Term::Int(ty.modulus())
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

/// The value `evaluate` gives a closed term, as the literal it proves the
/// term equal to; the type of the equation is checked too.
fn evaluated(ctx: &mut Context, term: &Term, ty: &Type) -> Term {
    match infer_proof(ctx, &Proof::Evaluate(term.clone())) {
        Ok(Term::Eq(found, left, right)) => {
            assert!(same_type(&found, ty), "{term} evaluated at {found}");
            assert!(same(&left, term));
            *right
        }
        other => panic!("evaluating {term}: {other:?}"),
    }
}

/// Evaluation proves the claim when `holds`, and its negation when not, and
/// never the other.
fn decided(ctx: &mut Context, prelude: &Prelude, claim: Term, holds: bool) {
    let negation = prelude.not_prop(claim.clone());
    let (proved, refused) = if holds {
        (claim.clone(), negation)
    } else {
        (negation, claim.clone())
    };
    let proof = Proof::Evaluate(claim);
    assert_eq!(infer_proof(ctx, &proof), Ok(proved));
    mismatch(check_proof(ctx, &proof, &refused));
}

/// A variable of each machine type, executable, a ghost `Int`, and a bool.
struct Vars {
    machine: Vec<Term>,
    int: Term,
    flag: Term,
}

impl Vars {
    fn of(&self, ty: MachineInt) -> Term {
        self.machine[ALL.iter().position(|t| *t == ty).unwrap()].clone()
    }

    /// A term of every type an axiom's argument could be given, with its type.
    fn candidates(&self) -> Vec<(Term, Type)> {
        let mut terms: Vec<(Term, Type)> = ALL
            .iter()
            .flat_map(|ty| {
                [
                    (self.of(*ty), Type::machine(*ty)),
                    (lit(*ty, 0), Type::machine(*ty)),
                ]
            })
            .collect();
        terms.extend([
            (self.int.clone(), Type::Int),
            (int(0), Type::Int),
            (self.flag.clone(), Type::Bool),
            (Term::Bool(true), Type::Bool),
        ]);
        terms
    }
}

fn vars(ctx: &mut Context) -> Vars {
    let machine = ALL
        .iter()
        .map(|ty| Term::var(ctx.declare(Type::machine(*ty)).unwrap()))
        .collect();
    Vars {
        machine,
        int: Term::var(ctx.declare_ghost(Type::Int).unwrap()),
        flag: Term::var(ctx.declare(Type::Bool).unwrap()),
    }
}

/// The axiom at `argument`, of type `argument_type`, proves exactly
/// `statement`; at an argument of any other type it is ill typed, and at
/// another argument of the same type it proves something else.
fn states(
    ctx: &mut Context,
    v: &Vars,
    build: &dyn Fn(Term) -> Axiom,
    argument: &Term,
    argument_type: &Type,
    statement: Term,
) {
    let axiom = build(argument.clone());
    assert_eq!(axiom.terms(), vec![argument], "{}", axiom.name());
    assert_eq!(
        infer_proof(ctx, &ax(axiom.clone())),
        Ok(statement.clone()),
        "{}",
        axiom.name()
    );
    assert_eq!(check_proof(ctx, &ax(axiom), &statement), Ok(()));
    for (term, ty) in v.candidates() {
        let result = infer_proof(ctx, &ax(build(term.clone())));
        if !same_type(&ty, argument_type) {
            ill_typed(result);
        } else if !same(&term, argument) {
            assert!(
                result.is_ok() && result != Ok(statement.clone()),
                "at {term}: {result:?}"
            );
        }
    }
    // An axiom is an instance at terms of the context, not a schema.
    assert_eq!(
        infer_proof(ctx, &ax(build(Term::Bound(0)))),
        Err(KernelError::DanglingBound)
    );
}

// --- Rust's side ------------------------------------------------------------------

/// `value as T as i128`, for `value` an `i128`: Rust's truncation into `T`.
macro_rules! rust_wrap {
    ($ty:expr, $value:expr) => {
        match $ty {
            U8 => $value as u8 as i128,
            U16 => $value as u16 as i128,
            U32 | Usize32 => $value as u32 as i128,
            U64 | Usize64 => $value as u64 as i128,
            I8 => $value as i8 as i128,
            I16 => $value as i16 as i128,
            I32 | Isize32 => $value as i32 as i128,
            I64 | Isize64 => $value as i64 as i128,
        }
    };
}

/// What Rust computes for `x as T`, where `x : S` has the value `value`.
/// The value is first put into the concrete source type, so that the cast
/// is the one Rust performs between the two machine types.
fn rust_cast(from: MachineInt, to: MachineInt, value: i128) -> i128 {
    assert!(from.contains(&Integer::from(value)));
    match from {
        U8 => rust_wrap!(to, value as u8),
        U16 => rust_wrap!(to, value as u16),
        U32 | Usize32 => rust_wrap!(to, value as u32),
        U64 | Usize64 => rust_wrap!(to, value as u64),
        I8 => rust_wrap!(to, value as i8),
        I16 => rust_wrap!(to, value as i16),
        I32 | Isize32 => rust_wrap!(to, value as i32),
        I64 | Isize64 => rust_wrap!(to, value as i64),
    }
}

/// Rust's truncation of an `i128` into `T`.
fn rust_wrap(to: MachineInt, value: i128) -> i128 {
    rust_wrap!(to, value)
}

fn i128_of(value: &Integer) -> i128 {
    value.to_i128().expect("fits in i128")
}

// --- The type table and the literals ----------------------------------------------

#[test]
#[doc = "spec: 2.12:1, 2.12:2"]
fn the_type_table_matches_rust() {
    for (ty, bits, signed, name, lo, hi) in [
        (U8, 8, false, "u8", 0, i128::from(u8::MAX)),
        (U16, 16, false, "u16", 0, i128::from(u16::MAX)),
        (U32, 32, false, "u32", 0, i128::from(u32::MAX)),
        (U64, 64, false, "u64", 0, i128::from(u64::MAX)),
        (I8, 8, true, "i8", i128::from(i8::MIN), i128::from(i8::MAX)),
        (
            I16,
            16,
            true,
            "i16",
            i128::from(i16::MIN),
            i128::from(i16::MAX),
        ),
        (
            I32,
            32,
            true,
            "i32",
            i128::from(i32::MIN),
            i128::from(i32::MAX),
        ),
        (
            I64,
            64,
            true,
            "i64",
            i128::from(i64::MIN),
            i128::from(i64::MAX),
        ),
    ] {
        assert_eq!(ty.bits(), bits);
        assert_eq!(ty.signed(), signed);
        assert_eq!(ty.name(), name);
        assert_eq!(ty.min(), Integer::from(lo));
        assert_eq!(ty.max(), Integer::from(hi));
        assert_eq!(ty.modulus(), Integer::from(hi - lo + 1));
        assert_eq!(Type::machine(ty).to_string(), name);
        assert_eq!(Type::machine(ty).as_machine(), Some(ty));
        assert!(!Type::machine(ty).is_ghost());
    }
    assert_eq!(ALL.len(), 8);
    assert_eq!(Type::machine(U8), Type::U8);
    assert_eq!(Type::machine(U16), Type::Machine(U16));
    assert_eq!(Type::Int.as_machine(), None);
    assert_eq!(Term::machine(U8, Integer::from(200i128)), Term::U8(200));
    assert_eq!(
        Term::machine(I8, Integer::from(-100i128)),
        Term::Machine(I8, Integer::from(-100i128))
    );
    assert_eq!(
        Term::U8(7).machine_value(),
        Some((U8, Integer::from(7i128)))
    );
    assert_eq!(
        lit(I64, -7).machine_value(),
        Some((I64, Integer::from(-7i128)))
    );
    assert_eq!(int(7).machine_value(), None);
    assert_eq!(lit(U16, 300).to_string(), "300u16");
    assert_eq!(lit(I32, -5).to_string(), "-5i32");
    assert_eq!(view(U16, lit(U16, 3)).to_string(), "view[u16](3u16)");
    assert_eq!(cast(U8, I64, Term::U8(3)).to_string(), "cast[u8, i64](3)");
}

#[test]
#[doc = "spec: 2.3:1, 2.4:1, 2.12:3"]
fn literals_are_runtime_data_within_the_range_of_their_type() {
    let (mut ctx, _) = setup();
    for ty in ALL {
        for value in [ty.min(), ty.max(), Integer::zero()] {
            let literal = Term::machine(ty, value);
            for mode in [Mode::Logical, Mode::Executable] {
                assert_eq!(infer_term(&mut ctx, &literal, mode), Ok(Type::machine(ty)));
            }
        }
        // A literal built out of range, past either end, is not a term.
        let one = Integer::from(1i128);
        for value in [ty.max().add(&one), ty.min().sub(&one), ty.modulus()] {
            let outside = Term::Machine(ty, value);
            let error = if ty == U8 {
                KernelError::MachineFormOfU8
            } else {
                KernelError::OutOfRange(outside.clone())
            };
            assert_eq!(
                infer_term(&mut ctx, &outside, Mode::Logical),
                Err(error.clone())
            );
            // Inside a primitive, a comparison, and an axiom as well.
            assert_eq!(
                infer_term(&mut ctx, &view(ty, outside.clone()), Mode::Logical),
                Err(error.clone())
            );
            assert_eq!(
                infer_term(
                    &mut ctx,
                    &eq_at(ty, outside.clone(), outside.clone()),
                    Mode::Logical
                ),
                Err(error.clone())
            );
            assert_eq!(
                infer_proof(&mut ctx, &ax(Axiom::WrapView(ty, outside.clone()))),
                Err(error.clone())
            );
            // Native evaluation does not compute on it either: the literal
            // rule has no step, before typing rejects the term.
            assert!(matches!(
                infer_proof(&mut ctx, &Proof::Literal(view(ty, outside.clone()))),
                Err(KernelError::NoComputationStep(_))
            ));
            assert_eq!(outside.machine_value(), None);
            assert_eq!(
                infer_proof(&mut ctx, &Proof::Evaluate(view(ty, outside))),
                Err(error)
            );
        }
        // Literals of two types are never the same term; equal values of
        // one type are.
        for other in ALL {
            assert_eq!(same(&lit(ty, 1), &lit(other, 1)), ty == other);
            assert_eq!(
                same_type(&Type::machine(ty), &Type::machine(other)),
                ty == other
            );
        }
        assert!(!same(&lit(ty, 1), &lit(ty, 0)));
    }
    // u8 has one spelling. Its machine forms are rejected everywhere a type
    // or a term is checked.
    assert_eq!(
        check_type(&mut ctx, &Type::Machine(U8)),
        Err(KernelError::MachineFormOfU8)
    );
    assert_eq!(
        ctx.declare(Type::Machine(U8)),
        Err(KernelError::MachineFormOfU8)
    );
    assert_eq!(
        check_type(&mut ctx, &Type::Tuple(vec![Type::Machine(U8)])),
        Err(KernelError::MachineFormOfU8)
    );
    assert_eq!(
        infer_term(&mut ctx, &Term::Machine(U8, Integer::zero()), Mode::Logical),
        Err(KernelError::MachineFormOfU8)
    );
    assert_eq!(
        infer_term(
            &mut ctx,
            &Term::Machine(U8, Integer::from(300i128)),
            Mode::Logical
        ),
        Err(KernelError::MachineFormOfU8)
    );
    for other in ALL {
        assert_eq!(
            check_type(&mut ctx, &Type::Machine(other)).is_ok(),
            other != U8
        );
    }
    // A machine type is plain data: a value of a product of them evaluates.
    let pair = Type::Tuple(vec![Type::Machine(U64), Type::Machine(I16)]);
    let value = Term::tuple(&pair, vec![lit(U64, 5), lit(I16, -5)]);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(Term::proj(value.clone(), 1))),
        Ok(eq_at(I16, Term::proj(value, 1), lit(I16, -5)))
    );
}

// --- The primitives ---------------------------------------------------------------

#[test]
#[doc = "spec: 2.3:1, 2.4:1, 2.12:4"]
fn view_and_wrap_are_ghost_and_cast_is_executable() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    for ty in ALL {
        let x = v.of(ty);
        // view: T -> Int, a ghost value.
        assert_eq!(
            infer_term(&mut ctx, &view(ty, x.clone()), Mode::Logical),
            Ok(Type::Int)
        );
        assert!(matches!(
            infer_term(&mut ctx, &view(ty, x.clone()), Mode::Executable),
            Err(KernelError::GhostTypeInExecutable(Type::Int))
        ));
        // wrap: Int -> T, whose argument no executable term can supply.
        assert_eq!(
            infer_term(&mut ctx, &wrap(ty, v.int.clone()), Mode::Logical),
            Ok(Type::machine(ty))
        );
        assert!(matches!(
            infer_term(&mut ctx, &wrap(ty, v.int.clone()), Mode::Executable),
            Err(KernelError::GhostInExecutable(_))
        ));
        assert!(matches!(
            infer_term(&mut ctx, &wrap(ty, int(1)), Mode::Executable),
            Err(KernelError::GhostTypeInExecutable(Type::Int))
        ));
        // cast: S -> T, in both modes, at every pair of types.
        for to in ALL {
            for mode in [Mode::Logical, Mode::Executable] {
                assert_eq!(
                    infer_term(&mut ctx, &cast(ty, to, x.clone()), mode),
                    Ok(Type::machine(to))
                );
            }
            // The wrong source: a variable of another type, or an Int.
            for from in ALL {
                if from != ty {
                    ill_typed(infer_term(
                        &mut ctx,
                        &cast(from, to, x.clone()),
                        Mode::Logical,
                    ));
                }
            }
            ill_typed(infer_term(
                &mut ctx,
                &cast(ty, to, v.int.clone()),
                Mode::Logical,
            ));
        }
        // view of another type's value, of an Int, or of a bool; wrap of a
        // machine value or a bool.
        for other in ALL {
            if other != ty {
                ill_typed(infer_term(&mut ctx, &view(ty, v.of(other)), Mode::Logical));
                ill_typed(infer_term(&mut ctx, &view(other, x.clone()), Mode::Logical));
            }
            ill_typed(infer_term(&mut ctx, &wrap(ty, v.of(other)), Mode::Logical));
        }
        ill_typed(infer_term(
            &mut ctx,
            &view(ty, v.int.clone()),
            Mode::Logical,
        ));
        ill_typed(infer_term(
            &mut ctx,
            &view(ty, v.flag.clone()),
            Mode::Logical,
        ));
        ill_typed(infer_term(
            &mut ctx,
            &wrap(ty, v.flag.clone()),
            Mode::Logical,
        ));
        ill_typed(infer_term(
            &mut ctx,
            &wrap(ty, Term::Bool(true)),
            Mode::Logical,
        ));
        // The arity.
        for prim in [Prim::View(ty), Prim::Wrap(ty), Prim::Cast(ty, ty)] {
            for arguments in [vec![], vec![x.clone(), x.clone()]] {
                assert!(matches!(
                    infer_term(&mut ctx, &Term::prim(prim, arguments), Mode::Logical),
                    Err(KernelError::WrongArity { expected: 1, .. })
                ));
            }
        }
        assert_eq!(Prim::View(ty).name(), "view");
        assert_eq!(Prim::Wrap(ty).name(), "wrap");
        assert_eq!(Prim::Cast(ty, ty).name(), "cast");
        // The type is part of the primitive: the same operation at two
        // types is two terms.
        assert!(same(&view(ty, x.clone()), &view(ty, x.clone())));
        for other in ALL {
            assert_eq!(
                same(&wrap(ty, v.int.clone()), &wrap(other, v.int.clone())),
                ty == other
            );
            assert_eq!(
                same(&cast(ty, ty, x.clone()), &cast(ty, other, x.clone())),
                ty == other
            );
        }
    }
}

#[test]
#[doc = "spec: 2.12:10"]
fn the_literal_axiom_computes_one_primitive_on_literals() {
    let (mut ctx, _) = setup();
    let n = Term::var(ctx.declare_ghost(Type::Int).unwrap());
    for (term, value) in [
        (
            view(U16, lit(U16, 300)),
            int_eq(view(U16, lit(U16, 300)), int(300)),
        ),
        (
            view(I8, lit(I8, -1)),
            int_eq(view(I8, lit(I8, -1)), int(-1)),
        ),
        (
            view(U8, Term::U8(255)),
            int_eq(view(U8, Term::U8(255)), int(255)),
        ),
        (
            wrap(I8, int(200)),
            eq_at(I8, wrap(I8, int(200)), lit(I8, -56)),
        ),
        (
            wrap(U8, int(-1)),
            eq_at(U8, wrap(U8, int(-1)), Term::U8(255)),
        ),
        (
            wrap(U64, int(-1)),
            eq_at(U64, wrap(U64, int(-1)), lit(U64, u64::MAX.into())),
        ),
        (
            wrap(I64, Term::Int(Integer::from(1u128 << 63))),
            eq_at(
                I64,
                wrap(I64, Term::Int(Integer::from(1u128 << 63))),
                lit(I64, i64::MIN.into()),
            ),
        ),
        (
            cast(U16, I8, lit(U16, 300)),
            eq_at(I8, cast(U16, I8, lit(U16, 300)), lit(I8, 44)),
        ),
        (
            cast(I8, U64, lit(I8, -1)),
            eq_at(U64, cast(I8, U64, lit(I8, -1)), lit(U64, u64::MAX.into())),
        ),
        (
            cast(I8, I64, lit(I8, -1)),
            eq_at(I64, cast(I8, I64, lit(I8, -1)), lit(I64, -1)),
        ),
        (
            cast(U8, U8, Term::U8(9)),
            eq_at(U8, cast(U8, U8, Term::U8(9)), Term::U8(9)),
        ),
    ] {
        assert_eq!(infer_proof(&mut ctx, &Proof::Literal(term)), Ok(value));
    }
    // No step: a non-literal argument, or a literal of the wrong type.
    for stuck in [
        view(U16, wrap(U16, int(3))),
        wrap(U16, add(int(1), int(1))),
        wrap(U16, n.clone()),
        cast(U16, U8, cast(U8, U16, Term::U8(1))),
        view(U16, Term::U8(1)),
        view(U8, lit(U16, 1)),
        cast(U16, I8, lit(I16, 1)),
        wrap(U16, Term::Bool(true)),
    ] {
        assert!(
            matches!(
                infer_proof(&mut ctx, &Proof::Literal(stuck.clone())),
                Err(KernelError::NoComputationStep(_))
            ),
            "{stuck}"
        );
    }
}

// --- The axioms -------------------------------------------------------------------

#[test]
#[doc = "spec: 2.12:5, 2.12:6, 2.12:7"]
fn view_lies_in_the_range_of_its_type() {
    let (mut ctx, prelude) = setup();
    let v = vars(&mut ctx);
    for ty in ALL {
        let x = v.of(ty);
        states(
            &mut ctx,
            &v,
            &|t| Axiom::ViewLower(ty, t),
            &x,
            &Type::machine(ty),
            le(min(ty), view(ty, x.clone())),
        );
        states(
            &mut ctx,
            &v,
            &|t| Axiom::ViewUpper(ty, t),
            &x,
            &Type::machine(ty),
            le(view(ty, x.clone()), max(ty)),
        );
        let lower = ax(Axiom::ViewLower(ty, x.clone()));
        let upper = ax(Axiom::ViewUpper(ty, x.clone()));
        // The strict order, the other side, the bound of a neighbouring
        // type, and the bound off by one.
        let one = Integer::from(1i128);
        for wrong in [
            lt(min(ty), view(ty, x.clone())),
            le(view(ty, x.clone()), min(ty)),
            le(Term::Int(ty.min().sub(&one)), view(ty, x.clone())),
            le(Term::Int(ty.min().add(&one)), view(ty, x.clone())),
            le(view(ty, x.clone()), max(ty)),
        ] {
            mismatch(check_proof(&mut ctx, &lower, &wrong));
        }
        for wrong in [
            lt(view(ty, x.clone()), max(ty)),
            le(max(ty), view(ty, x.clone())),
            le(view(ty, x.clone()), Term::Int(ty.max().add(&one))),
            le(view(ty, x.clone()), Term::Int(ty.max().sub(&one))),
            le(min(ty), view(ty, x.clone())),
        ] {
            mismatch(check_proof(&mut ctx, &upper, &wrong));
        }
        for other in ALL {
            if other.min() != ty.min() {
                mismatch(check_proof(
                    &mut ctx,
                    &lower,
                    &le(min(other), view(ty, x.clone())),
                ));
            }
            if other.max() != ty.max() {
                mismatch(check_proof(
                    &mut ctx,
                    &upper,
                    &le(view(ty, x.clone()), max(other)),
                ));
            }
        }
        // At the ends of the range, evaluation agrees, and it refutes the
        // strict bounds there.
        for end in [ty.min(), ty.max()] {
            let literal = Term::machine(ty, end.clone());
            decided(
                &mut ctx,
                &prelude,
                le(min(ty), view(ty, literal.clone())),
                true,
            );
            decided(
                &mut ctx,
                &prelude,
                le(view(ty, literal.clone()), max(ty)),
                true,
            );
            decided(
                &mut ctx,
                &prelude,
                lt(min(ty), view(ty, literal.clone())),
                end != ty.min(),
            );
            decided(
                &mut ctx,
                &prelude,
                lt(view(ty, literal), max(ty)),
                end != ty.max(),
            );
        }
    }
}

#[test]
#[doc = "spec: 2.12:6, 2.12:9"]
fn wrap_of_view_is_the_identity() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    for ty in ALL {
        let x = v.of(ty);
        let round = wrap(ty, view(ty, x.clone()));
        states(
            &mut ctx,
            &v,
            &|t| Axiom::WrapView(ty, t),
            &x,
            &Type::machine(ty),
            eq_at(ty, round.clone(), x.clone()),
        );
        let axiom = ax(Axiom::WrapView(ty, x.clone()));
        // The other orientation; the equation at another type is not even
        // a proposition, because its sides have type T.
        mismatch(check_proof(
            &mut ctx,
            &axiom,
            &eq_at(ty, x.clone(), round.clone()),
        ));
        for other in ALL {
            if other != ty {
                ill_typed(check_proof(
                    &mut ctx,
                    &axiom,
                    &eq_at(other, round.clone(), x.clone()),
                ));
            }
        }
        ill_typed(check_proof(
            &mut ctx,
            &axiom,
            &Term::eq(Type::Int, round.clone(), x.clone()),
        ));
        // Not the composition the other way, which is an Int equation with
        // a premise (view_wrap), nor a cast.
        mismatch(check_proof(
            &mut ctx,
            &axiom,
            &int_eq(view(ty, wrap(ty, view(ty, x.clone()))), view(ty, x.clone())),
        ));
        mismatch(check_proof(
            &mut ctx,
            &axiom,
            &eq_at(ty, cast(ty, ty, x.clone()), x.clone()),
        ));
    }
}

#[test]
#[doc = "spec: 2.12:6, 2.12:9"]
fn view_of_wrap_is_the_identity_within_the_range() {
    let (mut ctx, prelude) = setup();
    let v = vars(&mut ctx);
    let n = v.int.clone();
    for ty in ALL {
        let round = int_eq(view(ty, wrap(ty, n.clone())), n.clone());
        let in_range = |body: Term| {
            implies(
                le(min(ty), n.clone()),
                implies(le(n.clone(), max(ty)), body),
            )
        };
        states(
            &mut ctx,
            &v,
            &|t| Axiom::ViewWrap(ty, t),
            &n,
            &Type::Int,
            in_range(round.clone()),
        );
        let axiom = ax(Axiom::ViewWrap(ty, n.clone()));
        for wrong in [
            // Without its premises, or with only one of them.
            round.clone(),
            implies(le(min(ty), n.clone()), round.clone()),
            implies(le(n.clone(), max(ty)), round.clone()),
            // The premises in the other order, or strict.
            implies(
                le(n.clone(), max(ty)),
                implies(le(min(ty), n.clone()), round.clone()),
            ),
            implies(
                lt(min(ty), n.clone()),
                implies(le(n.clone(), max(ty)), round.clone()),
            ),
            implies(
                le(min(ty), n.clone()),
                implies(lt(n.clone(), max(ty)), round.clone()),
            ),
            // The other orientation, and the wrong conclusion.
            in_range(int_eq(n.clone(), view(ty, wrap(ty, n.clone())))),
            in_range(int_eq(n.clone(), n.clone())),
        ] {
            mismatch(check_proof(&mut ctx, &axiom, &wrong));
        }
        // The premises at a neighbouring type's range.
        for other in ALL {
            if other != ty {
                mismatch(check_proof(
                    &mut ctx,
                    &axiom,
                    &implies(
                        le(min(other), n.clone()),
                        implies(le(n.clone(), max(other)), round.clone()),
                    ),
                ));
            }
        }
        // The premises are needed: just outside the range, the round trip
        // is false, and evaluation says so.
        let one = Integer::from(1i128);
        for outside in [
            ty.max().add(&one),
            ty.min().sub(&one),
            ty.modulus(),
            ty.modulus().neg(),
        ] {
            let literal = Term::Int(outside);
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, wrap(ty, literal.clone())), literal),
                false,
            );
        }
        for inside in [ty.min(), ty.max(), Integer::zero()] {
            let literal = Term::Int(inside);
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, wrap(ty, literal.clone())), literal),
                true,
            );
        }
    }
}

#[test]
#[doc = "spec: 2.12:6"]
fn wrap_is_periodic_with_period_two_to_the_bits() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let n = v.int.clone();
    for ty in ALL {
        let period = |shift: Term| eq_at(ty, wrap(ty, add(n.clone(), shift)), wrap(ty, n.clone()));
        states(
            &mut ctx,
            &v,
            &|t| Axiom::WrapPeriod(ty, t),
            &n,
            &Type::Int,
            period(modulus(ty)),
        );
        let axiom = ax(Axiom::WrapPeriod(ty, n.clone()));
        let one = Integer::from(1i128);
        let two = Integer::from(2i128);
        for wrong in [
            // 2^bits - 1, 2^bits + 1, 2^(bits + 1), and 2^(bits - 1), the
            // last two true or false but not the axiom.
            period(Term::Int(ty.modulus().sub(&one))),
            period(Term::Int(ty.modulus().add(&one))),
            period(Term::Int(ty.modulus().mul(&two))),
            period(Term::Int(ty.modulus().div(&two))),
            // The shift on the other side, or subtracted, or the other
            // orientation.
            eq_at(
                ty,
                wrap(ty, add(modulus(ty), n.clone())),
                wrap(ty, n.clone()),
            ),
            eq_at(
                ty,
                wrap(ty, Term::int_sub(n.clone(), modulus(ty))),
                wrap(ty, n.clone()),
            ),
            eq_at(
                ty,
                wrap(ty, n.clone()),
                wrap(ty, add(n.clone(), modulus(ty))),
            ),
            // As an equation of the views.
            int_eq(
                view(ty, wrap(ty, add(n.clone(), modulus(ty)))),
                view(ty, wrap(ty, n.clone())),
            ),
        ] {
            mismatch(check_proof(&mut ctx, &axiom, &wrong));
        }
        // The period of another type: the same term only when the widths
        // agree, and then it is the same axiom at another type, which is a
        // different equation because the wraps differ.
        for other in ALL {
            if other != ty {
                mismatch(check_proof(
                    &mut ctx,
                    &axiom,
                    &eq_at(
                        other,
                        wrap(other, add(n.clone(), modulus(other))),
                        wrap(other, n.clone()),
                    ),
                ));
            }
        }
        // Evaluation: the period holds at every literal tried; a shift by
        // 2^bits - 1 does not, and by 2^(bits + 1) it does, though that is
        // not the axiom.
        for start in [
            Integer::zero(),
            one.clone(),
            ty.max(),
            ty.min(),
            ty.modulus().neg(),
        ] {
            let mut at = |shift: &Integer| {
                let term = wrap(ty, add(Term::Int(start.clone()), Term::Int(shift.clone())));
                evaluated(&mut ctx, &term, &Type::machine(ty))
            };
            let plain = at(&Integer::zero());
            assert!(same(&at(&ty.modulus()), &plain));
            assert!(same(&at(&ty.modulus().mul(&two)), &plain));
            assert!(same(&at(&ty.modulus().neg()), &plain));
            assert!(!same(&at(&ty.modulus().sub(&one)), &plain));
            assert!(!same(&at(&ty.modulus().add(&one)), &plain));
        }
    }
}

#[test]
#[doc = "spec: 2.12:6"]
fn cast_is_wrap_of_view_at_every_pair_of_types() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    for from in ALL {
        let x = v.of(from);
        for to in ALL {
            let unfolded = wrap(to, view(from, x.clone()));
            states(
                &mut ctx,
                &v,
                &|t| Axiom::CastDef(from, to, t),
                &x,
                &Type::machine(from),
                eq_at(to, cast(from, to, x.clone()), unfolded.clone()),
            );
            let axiom = ax(Axiom::CastDef(from, to, x.clone()));
            // The other orientation; the same equation at another target,
            // which is the axiom at that target; and the equation stated at
            // the source type, which is not a proposition.
            mismatch(check_proof(
                &mut ctx,
                &axiom,
                &eq_at(to, unfolded.clone(), cast(from, to, x.clone())),
            ));
            for other in ALL {
                if other != to {
                    mismatch(check_proof(
                        &mut ctx,
                        &axiom,
                        &eq_at(
                            other,
                            cast(from, other, x.clone()),
                            wrap(other, view(from, x.clone())),
                        ),
                    ));
                }
            }
            if from != to {
                ill_typed(check_proof(
                    &mut ctx,
                    &axiom,
                    &eq_at(from, cast(from, to, x.clone()), unfolded.clone()),
                ));
            }
            // The types exchanged: for x : S that is ill typed, not merely
            // a different statement.
            if from != to {
                ill_typed(infer_proof(
                    &mut ctx,
                    &ax(Axiom::CastDef(to, from, x.clone())),
                ));
                ill_typed(check_proof(
                    &mut ctx,
                    &axiom,
                    &eq_at(
                        from,
                        cast(to, from, x.clone()),
                        wrap(from, view(to, x.clone())),
                    ),
                ));
            }
        }
        // cast(T, T)(x) == x is not an axiom; it is derived from cast_def
        // and wrap_view in two steps.
        let identity = eq_at(from, cast(from, from, x.clone()), x.clone());
        mismatch(check_proof(
            &mut ctx,
            &ax(Axiom::CastDef(from, from, x.clone())),
            &identity,
        ));
        let derived = Chain::new(Type::machine(from), cast(from, from, x.clone()))
            .step(ax(Axiom::CastDef(from, from, x.clone())))
            .step(ax(Axiom::WrapView(from, x.clone())))
            .finish();
        assert_eq!(check_proof(&mut ctx, &derived, &identity), Ok(()));
    }
}

// --- Exhaustive at 8 bits, against Rust ---------------------------------------------

/// Every value of an 8-bit type, as Rust holds it.
fn every_8_bit(ty: MachineInt) -> Vec<i128> {
    match ty {
        U8 => (u8::MIN..=u8::MAX).map(i128::from).collect(),
        I8 => (i8::MIN..=i8::MAX).map(i128::from).collect(),
        _ => panic!("not an 8-bit type"),
    }
}

#[test]
fn view_and_the_round_trips_agree_with_rust_for_every_8_bit_value() {
    let (mut ctx, prelude) = setup();
    for ty in [U8, I8] {
        for value in every_8_bit(ty) {
            let literal = lit(ty, value);
            // view is the value itself, and nothing else.
            assert!(same(
                &evaluated(&mut ctx, &view(ty, literal.clone()), &Type::Int),
                &int(value)
            ));
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, literal.clone()), int(value)),
                true,
            );
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, literal.clone()), int(value + 1)),
                false,
            );
            // wrap of the value is the literal, and the round trips close.
            assert!(same(
                &evaluated(&mut ctx, &wrap(ty, int(value)), &Type::machine(ty)),
                &literal
            ));
            assert!(same(
                &evaluated(
                    &mut ctx,
                    &wrap(ty, view(ty, literal.clone())),
                    &Type::machine(ty)
                ),
                &literal
            ));
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, wrap(ty, int(value))), int(value)),
                true,
            );
            // Rust agrees that the value is its own wrap.
            assert_eq!(rust_wrap(ty, value), value);
        }
    }
    // On bytes, the view is the number of the byte.
    for byte in u8::MIN..=u8::MAX {
        let literal = Term::U8(byte);
        assert!(same(
            &evaluated(&mut ctx, &view(U8, literal.clone()), &Type::Int),
            &int(byte.into())
        ));
    }
}

#[test]
fn cast_agrees_with_rust_for_every_8_bit_value_and_every_target() {
    let (mut ctx, _) = setup();
    let mut compared = 0;
    for from in [U8, I8] {
        for value in every_8_bit(from) {
            let literal = lit(from, value);
            for to in ALL {
                let expected = lit(to, rust_cast(from, to, value));
                let direct = evaluated(
                    &mut ctx,
                    &cast(from, to, literal.clone()),
                    &Type::machine(to),
                );
                assert!(
                    same(&direct, &expected),
                    "{from:?} {value} as {to:?}: {direct}"
                );
                // The right side of cast_def computes to the same literal.
                let unfolded = evaluated(
                    &mut ctx,
                    &wrap(to, view(from, literal.clone())),
                    &Type::machine(to),
                );
                assert!(same(&unfolded, &expected));
                // And the literal axiom, in one step.
                assert_eq!(
                    infer_proof(&mut ctx, &Proof::Literal(cast(from, to, literal.clone()))),
                    Ok(eq_at(to, cast(from, to, literal.clone()), expected))
                );
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 2 * 256 * 8);
}

// --- The wider types at their boundaries -------------------------------------------

/// The values of `ty` at which something changes: the ends of the range and
/// their neighbours, zero and its neighbours, and every power of two in
/// range with its neighbours.
fn boundary(ty: MachineInt) -> Vec<i128> {
    let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
    let mut values = vec![0, 1, 2, -1, -2, lo, lo + 1, lo + 2, hi, hi - 1, hi - 2];
    for k in 0..ty.bits() {
        let power = 1i128 << k;
        values.extend([power - 1, power, power + 1, -power - 1, -power, -power + 1]);
    }
    values.retain(|value| lo <= *value && *value <= hi);
    values.sort_unstable();
    values.dedup();
    values
}

/// Arguments for `wrap` at `ty`: the boundary values, the values just past
/// each end of the range, `2^bits` and `-2^bits` with their neighbours,
/// `2^(bits + 1)` and its neighbours, and the negation of every boundary
/// value, which lies outside an unsigned range.
fn wrap_arguments(ty: MachineInt) -> Vec<i128> {
    let (lo, hi) = (i128_of(&ty.min()), i128_of(&ty.max()));
    let modulus = 1i128 << ty.bits();
    let mut values = boundary(ty);
    values.extend(boundary(ty).into_iter().map(|value| -value));
    values.extend([lo - 1, lo - 2, hi + 1, hi + 2]);
    for period in [modulus, -modulus, 2 * modulus, -2 * modulus] {
        values.extend([period - 1, period, period + 1]);
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn view_wrap_and_cast_agree_with_rust_at_the_boundaries_of_every_type() {
    let (mut ctx, prelude) = setup();
    let mut compared = 0;
    for ty in ALL {
        let values = boundary(ty);
        assert!(
            values.len() >= 2 * ty.bits() as usize,
            "{ty:?}: {}",
            values.len()
        );
        for value in values {
            let literal = lit(ty, value);
            assert!(same(
                &evaluated(&mut ctx, &view(ty, literal.clone()), &Type::Int),
                &int(value)
            ));
            assert!(same(
                &evaluated(&mut ctx, &wrap(ty, int(value)), &Type::machine(ty)),
                &literal
            ));
            assert!(same(
                &evaluated(
                    &mut ctx,
                    &wrap(ty, view(ty, literal.clone())),
                    &Type::machine(ty)
                ),
                &literal
            ));
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, wrap(ty, int(value))), int(value)),
                true,
            );
            decided(
                &mut ctx,
                &prelude,
                le(min(ty), view(ty, literal.clone())),
                true,
            );
            decided(
                &mut ctx,
                &prelude,
                le(view(ty, literal.clone()), max(ty)),
                true,
            );
            for to in ALL {
                let expected = lit(to, rust_cast(ty, to, value));
                let found = evaluated(&mut ctx, &cast(ty, to, literal.clone()), &Type::machine(to));
                assert!(same(&found, &expected), "{ty:?} {value} as {to:?}: {found}");
                compared += 1;
            }
        }
    }
    assert!(compared > 1500, "{compared}");
}

#[test]
#[doc = "spec: 1.27:2"]
fn wrap_agrees_with_rust_at_the_boundaries_of_every_type() {
    let (mut ctx, prelude) = setup();
    for ty in ALL {
        let modulus = 1i128 << ty.bits();
        for value in wrap_arguments(ty) {
            let expected = lit(ty, rust_wrap(ty, value));
            let found = evaluated(&mut ctx, &wrap(ty, int(value)), &Type::machine(ty));
            assert!(
                same(&found, &expected),
                "wrap[{}]({value}): {found}",
                ty.name()
            );
            // The period, both ways, and the round trip exactly in range.
            for shift in [modulus, -modulus] {
                let shifted =
                    evaluated(&mut ctx, &wrap(ty, int(value + shift)), &Type::machine(ty));
                assert!(same(&shifted, &expected));
            }
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, wrap(ty, int(value))), int(value)),
                ty.contains(&Integer::from(value)),
            );
        }
    }
}

// --- Random values ----------------------------------------------------------------

fn scale() -> u64 {
    if std::env::var_os("LOCUS_EXTENDED").is_some_and(|value| !value.is_empty()) {
        100
    } else {
        1
    }
}

/// A random integer of up to two hundred bits, together with the sign and
/// the 64-bit limbs it was assembled from, least significant first.
fn random_integer(rng: &mut Rng) -> (Integer, bool, Vec<u64>) {
    let max_bits = *rng.choose(&[0, 8, 32, 64, 65, 100, 128, 200, 200, 200]);
    let bits = rng.below(max_bits + 1) as u32;
    let mut limbs: Vec<u64> = (0..bits / 64).map(|_| rng.next_u64()).collect();
    if !bits.is_multiple_of(64) {
        limbs.push(rng.next_u64() >> (64 - bits % 64));
    }
    let base = Integer::from(1u128 << 64);
    let magnitude = limbs.iter().rev().fold(Integer::zero(), |value, limb| {
        value.mul(&base).add(&Integer::from(u128::from(*limb)))
    });
    let negative = rng.chance(1, 2) && !magnitude.is_zero();
    let value = if negative { magnitude.neg() } else { magnitude };
    (value, negative, limbs)
}

/// The reduction of the assembled integer into `ty`, without `Integer`: the
/// low 64 bits of the magnitude are its first limb; the low 64 bits of the
/// negation are the two's complement of that limb, since negation modulo
/// `2^64` depends only on the value modulo `2^64`; and the type's `bits`,
/// at most 64, are the low bits of those, which Rust's `as` from `u64`
/// keeps, reading them as signed when the type is.
fn independent_wrap(ty: MachineInt, negative: bool, limbs: &[u64]) -> i128 {
    let low = limbs.first().copied().unwrap_or(0);
    let low = if negative { low.wrapping_neg() } else { low };
    rust_wrap!(ty, low)
}

#[test]
#[doc = "spec: 2.12:8"]
fn wrap_of_random_integers_agrees_with_an_independent_reduction() {
    let (mut ctx, prelude) = setup();
    let mut wide = 0;
    for index in 0..300 * scale() {
        let seed = case_seed(0x4B34_5752, index);
        let mut rng = Rng::new(seed);
        let (value, negative, limbs) = random_integer(&mut rng);
        wide += usize::from(limbs.len() > 1);
        let term = Term::Int(value.clone());
        for ty in ALL {
            let expected = lit(ty, independent_wrap(ty, negative, &limbs));
            let found = evaluated(&mut ctx, &wrap(ty, term.clone()), &Type::machine(ty));
            assert!(
                same(&found, &expected),
                "seed {seed:#x}: wrap[{}]({value}) = {found}",
                ty.name()
            );
            assert_eq!(Term::machine(ty, ty.wrap(&value)), expected);
            // The wrapped value views back to itself, and the argument only
            // when it was in range.
            let (_, wrapped) = expected.machine_value().unwrap();
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, wrap(ty, term.clone())), Term::Int(wrapped)),
                true,
            );
            decided(
                &mut ctx,
                &prelude,
                int_eq(view(ty, wrap(ty, term.clone())), term.clone()),
                ty.contains(&value),
            );
        }
    }
    assert!(wide > 50, "{wide}");
}

/// A random value of `ty`, drawn from the full width and, one time in four,
/// from a boundary.
fn random_value(rng: &mut Rng, ty: MachineInt) -> i128 {
    if rng.chance(1, 4) {
        return *rng.choose(&boundary(ty));
    }
    rust_wrap(ty, i128::from(rng.next_u64()))
}

#[test]
fn casts_and_round_trips_on_random_values_agree_with_rust() {
    let (mut ctx, prelude) = setup();
    // Six values at every pair of types, so 384 casts.
    for index in 0..6 * 64 * scale() {
        let seed = case_seed(0x4B34_4341, index);
        let mut rng = Rng::new(seed);
        let from = ALL[(index % 64 / 8) as usize];
        let to = ALL[(index % 8) as usize];
        let value = random_value(&mut rng, from);
        assert!(from.contains(&Integer::from(value)), "seed {seed:#x}");
        let literal = lit(from, value);
        let expected = lit(to, rust_cast(from, to, value));
        let found = evaluated(
            &mut ctx,
            &cast(from, to, literal.clone()),
            &Type::machine(to),
        );
        assert!(
            same(&found, &expected),
            "seed {seed:#x}: {value} as {}: {found}",
            to.name()
        );
        let unfolded = evaluated(
            &mut ctx,
            &wrap(to, view(from, literal.clone())),
            &Type::machine(to),
        );
        assert!(same(&unfolded, &expected));
        // The round trips at the source.
        assert!(same(
            &evaluated(
                &mut ctx,
                &wrap(from, view(from, literal.clone())),
                &Type::machine(from)
            ),
            &literal
        ));
        decided(
            &mut ctx,
            &prelude,
            int_eq(view(from, wrap(from, int(value))), int(value)),
            true,
        );
        // A widening cast preserves the value, and a cast that does not fit
        // changes it, which Rust's answer already says; here the model says
        // it too: the value views back exactly when it is in the target.
        decided(
            &mut ctx,
            &prelude,
            int_eq(view(to, cast(from, to, literal)), int(value)),
            to.contains(&Integer::from(value)),
        );
    }
}

// --- Depth ----------------------------------------------------------------------

/// `wrap(view(wrap(view(... x ...))))`, `pairs` pairs deep.
fn nested_round_trips(ty: MachineInt, pairs: usize, x: Term) -> Term {
    (0..pairs).fold(x, |term, _| wrap(ty, view(ty, term)))
}

#[test]
fn machine_terms_are_held_to_the_depth_limits() {
    let (mut ctx, _) = setup();
    let x = Term::var(ctx.declare(Type::Machine(I32)).unwrap());
    let margin = 8;
    // Input depth: a literal, a machine type, and the primitives count as
    // every other node does.
    assert_eq!(
        infer_term(
            &mut ctx,
            &nested_round_trips(I32, (MAX_DEPTH - margin) / 2, x.clone()),
            Mode::Logical
        ),
        Ok(Type::Machine(I32))
    );
    let too_deep = nested_round_trips(I32, MAX_DEPTH / 2 + 1, x.clone());
    assert_eq!(
        infer_term(&mut ctx, &too_deep, Mode::Logical),
        Err(KernelError::TooDeep)
    );
    assert_eq!(
        infer_proof(&mut ctx, &ax(Axiom::WrapView(I32, too_deep.clone()))),
        Err(KernelError::TooDeep)
    );
    assert_eq!(
        infer_proof(&mut ctx, &ax(Axiom::ViewWrap(I32, view(I32, too_deep)))),
        Err(KernelError::TooDeep)
    );
    let deep_type = (0..MAX_DEPTH + 1).fold(Type::Machine(U64), |ty, _| Type::Tuple(vec![ty]));
    assert_eq!(check_type(&mut ctx, &deep_type), Err(KernelError::TooDeep));

    // Evaluation depth: each primitive nests one level.
    let fits = nested_round_trips(I32, (MAX_EVAL_DEPTH - margin) / 2, lit(I32, -9));
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(fits.clone())),
        Ok(eq_at(I32, fits, lit(I32, -9)))
    );
    let too_deep = nested_round_trips(I32, MAX_EVAL_DEPTH / 2 + margin, lit(I32, -9));
    assert_eq!(
        infer_term(&mut ctx, &too_deep, Mode::Logical),
        Ok(Type::Machine(I32))
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(too_deep)),
        Err(KernelError::EvaluationTooDeep)
    );
}
