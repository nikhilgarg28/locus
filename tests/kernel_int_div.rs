//! Tests for quotient and remainder on `Int` (build task K3; the kernel
//! contract in atlas.html): the primitives `int_div` and `int_rem`, which
//! truncate toward zero and are total with `a / 0 == 0` and `a % 0 == a`,
//! and the eight axioms that fix them.
//!
//! Each axiom is used once and misused several times: at the wrong type, in
//! the other orientation, with the factors or the summands in another order,
//! without its condition, and with the weak order where the strict one is
//! given. Each sign combination and a zero divisor are named tests, on closed
//! instances that evaluation decides, with the flooring and Euclidean
//! alternatives refuted. `a % 0 == a` is derived inside the kernel from the
//! decomposition, `a / 0 == 0`, and the ring axioms. Evaluation is compared
//! with `Integer` on random closed terms over every primitive, and with
//! Rust's `/` and `%` on random `i128` pairs; set LOCUS_EXTENDED to run a
//! hundred times as many. Every term here is written by hand.

use std::rc::Rc;

use locus::kernel::derive::{Chain, symm_at};
use locus::kernel::{
    Axiom, Context, Definitions, Integer, KernelError, Mode, Prelude, Prim, Proof, Term, Type,
    check_proof, infer_proof, infer_term,
};

#[path = "common/rng.rs"]
mod rng;
use rng::{Rng, case_seed};

fn setup() -> (Context, Prelude) {
    let (definitions, prelude) = Definitions::with_prelude();
    (Context::with_definitions(Rc::new(definitions)), prelude)
}

fn lit(value: i64) -> Term {
    Term::int(value)
}

fn add(left: Term, right: Term) -> Term {
    Term::int_add(left, right)
}

fn sub(left: Term, right: Term) -> Term {
    Term::int_sub(left, right)
}

fn mul(left: Term, right: Term) -> Term {
    Term::int_mul(left, right)
}

fn div(left: Term, right: Term) -> Term {
    Term::int_div(left, right)
}

fn rem(left: Term, right: Term) -> Term {
    Term::int_rem(left, right)
}

fn neg(number: Term) -> Term {
    Term::int_neg(number)
}

fn le(left: Term, right: Term) -> Term {
    Term::int_le(left, right)
}

fn lt(left: Term, right: Term) -> Term {
    Term::int_lt(left, right)
}

fn eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Int, left, right)
}

fn implies(premise: Term, conclusion: Term) -> Term {
    Term::implies(premise, conclusion)
}

fn ax(axiom: Axiom) -> Proof {
    Proof::Axiom(axiom)
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

/// Two integer variables, a `Nat`, and a byte.
struct Vars {
    a: Term,
    b: Term,
    nat: Term,
    byte: Term,
}

fn vars(ctx: &mut Context) -> Vars {
    let mut int = || Term::var(ctx.declare_ghost(Type::Int).unwrap());
    let (a, b) = (int(), int());
    Vars {
        a,
        b,
        nat: Term::var(ctx.declare_ghost(Type::Nat).unwrap()),
        byte: Term::var(ctx.declare(Type::U8).unwrap()),
    }
}

/// The axiom proves exactly `statement`, and with any one argument replaced
/// by a `Nat` or a byte it proves nothing.
fn states(ctx: &mut Context, v: &Vars, build: &dyn Fn(&[Term]) -> Axiom, statement: Term) {
    let arguments = [v.a.clone(), v.b.clone()];
    let axiom = build(&arguments);
    let arity = axiom.terms().len();
    assert_eq!(
        infer_proof(ctx, &ax(axiom.clone())),
        Ok(statement.clone()),
        "{}",
        axiom.name()
    );
    assert_eq!(check_proof(ctx, &ax(axiom.clone()), &statement), Ok(()));
    for position in 0..arity {
        for wrong in [&v.nat, &v.byte, &Term::nat(0), &Term::U8(0)] {
            let mut arguments = arguments.clone();
            arguments[position] = wrong.clone();
            ill_typed(infer_proof(ctx, &ax(build(&arguments))));
        }
    }
    // An axiom is an instance at terms of the context, not a schema.
    let mut arguments = arguments.clone();
    arguments[0] = Term::Bound(0);
    assert_eq!(
        infer_proof(ctx, &ax(build(&arguments))),
        Err(KernelError::DanglingBound)
    );
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

// --- The primitives -----------------------------------------------------------------

#[test]
fn quotient_and_remainder_are_ghost_primitives_computed_on_literals() {
    let (mut ctx, _) = setup();
    let n = Term::var(ctx.declare_ghost(Type::Int).unwrap());
    let nat = Term::var(ctx.declare_ghost(Type::Nat).unwrap());

    for term in [div(n.clone(), lit(2)), rem(lit(2), n.clone())] {
        assert_eq!(infer_term(&mut ctx, &term, Mode::Logical), Ok(Type::Int));
        assert!(matches!(
            infer_term(&mut ctx, &term, Mode::Executable),
            Err(KernelError::GhostTypeInExecutable(_))
        ));
    }
    for prim in [Prim::IntDiv, Prim::IntRem] {
        for wrong in [
            Term::prim(prim, vec![nat.clone(), lit(1)]),
            Term::prim(prim, vec![lit(1), nat.clone()]),
            Term::prim(prim, vec![Term::U8(1), lit(1)]),
        ] {
            ill_typed(infer_term(&mut ctx, &wrong, Mode::Logical));
        }
        assert!(matches!(
            infer_term(&mut ctx, &Term::prim(prim, vec![n.clone()]), Mode::Logical),
            Err(KernelError::WrongArity { .. })
        ));
    }
    assert_eq!(Prim::IntDiv.name(), "int_div");
    assert_eq!(Prim::IntRem.name(), "int_rem");

    // The literal axiom computes one primitive on literals, Rust's way.
    for (term, value) in [
        (div(lit(7), lit(2)), 3),
        (div(lit(-7), lit(2)), -3),
        (rem(lit(-7), lit(2)), -1),
        (rem(lit(7), lit(-2)), 1),
        (div(lit(7), lit(0)), 0),
        (rem(lit(7), lit(0)), 7),
        (rem(lit(-7), lit(0)), -7),
        (div(lit(0), lit(0)), 0),
    ] {
        assert_eq!(
            infer_proof(&mut ctx, &Proof::Literal(term.clone())),
            Ok(eq(term, lit(value)))
        );
    }
    for stuck in [div(add(lit(1), lit(1)), lit(1)), rem(lit(1), n)] {
        assert!(matches!(
            infer_proof(&mut ctx, &Proof::Literal(stuck)),
            Err(KernelError::NoComputationStep(_))
        ));
    }
}

// --- The axioms ----------------------------------------------------------------------

#[test]
fn the_decomposition_holds_with_no_condition() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let (a, b) = (v.a.clone(), v.b.clone());
    let quotient_part = mul(div(a.clone(), b.clone()), b.clone());
    let remainder = rem(a.clone(), b.clone());
    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntDivRem(t[0].clone(), t[1].clone()),
        eq(a.clone(), add(quotient_part.clone(), remainder.clone())),
    );
    let axiom = ax(Axiom::IntDivRem(a.clone(), b.clone()));
    // The other orientation.
    mismatch(check_proof(
        &mut ctx,
        &axiom,
        &eq(add(quotient_part.clone(), remainder.clone()), a.clone()),
    ));
    // The factors of the product in the other order, and the summands.
    mismatch(check_proof(
        &mut ctx,
        &axiom,
        &eq(
            a.clone(),
            add(mul(b.clone(), div(a.clone(), b.clone())), remainder.clone()),
        ),
    ));
    mismatch(check_proof(
        &mut ctx,
        &axiom,
        &eq(a.clone(), add(remainder.clone(), quotient_part.clone())),
    ));
    // Dividend and divisor exchanged in one place, or everywhere.
    mismatch(check_proof(
        &mut ctx,
        &axiom,
        &eq(
            a.clone(),
            add(mul(div(b.clone(), a.clone()), b.clone()), remainder.clone()),
        ),
    ));
    mismatch(check_proof(
        &mut ctx,
        &axiom,
        &eq(
            b.clone(),
            add(
                mul(div(b.clone(), a.clone()), a.clone()),
                rem(b.clone(), a.clone()),
            ),
        ),
    ));
    // The same shape with subtraction in place of the remainder.
    mismatch(check_proof(
        &mut ctx,
        &axiom,
        &eq(a.clone(), add(quotient_part, sub(a.clone(), b.clone()))),
    ));
    // The instance at a zero divisor is a statement about a alone, and
    // evaluation agrees with it on closed terms.
    assert_eq!(
        infer_proof(&mut ctx, &ax(Axiom::IntDivRem(a.clone(), lit(0)))),
        Ok(eq(
            a.clone(),
            add(mul(div(a.clone(), lit(0)), lit(0)), rem(a.clone(), lit(0)))
        ))
    );
    let closed = eq(
        lit(-9),
        add(mul(div(lit(-9), lit(0)), lit(0)), rem(lit(-9), lit(0))),
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(closed.clone())),
        Ok(closed)
    );
}

#[test]
fn division_by_zero_gives_zero() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let a = v.a.clone();
    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntDivZero(t[0].clone()),
        eq(div(a.clone(), lit(0)), lit(0)),
    );
    let axiom = ax(Axiom::IntDivZero(a.clone()));
    for wrong in [
        eq(lit(0), div(a.clone(), lit(0))),
        eq(div(lit(0), a.clone()), lit(0)),
        eq(div(a.clone(), lit(0)), a.clone()),
        eq(rem(a.clone(), lit(0)), a.clone()),
        eq(rem(a.clone(), lit(0)), lit(0)),
        eq(div(a.clone(), lit(1)), lit(0)),
    ] {
        mismatch(check_proof(&mut ctx, &axiom, &wrong));
    }
}

/// Each bound on the remainder, with its condition, and each near miss:
/// the bound without the condition, with `<=` where `<` is given, with the
/// condition weakened to `<=`, and with the roles of `a` and `b` exchanged.
fn bounded(
    ctx: &mut Context,
    v: &Vars,
    build: &dyn Fn(&[Term]) -> Axiom,
    condition: (Term, Term),
    bound: (Term, Term),
) {
    let (a, b) = (v.a.clone(), v.b.clone());
    let (below, above) = bound;
    let (cond_lo, cond_hi) = condition;
    states(
        ctx,
        v,
        build,
        implies(
            lt(cond_lo.clone(), cond_hi.clone()),
            lt(below.clone(), above.clone()),
        ),
    );
    let axiom = ax(build(&[a.clone(), b.clone()]));
    // Without its condition, which would be false at b == 0.
    mismatch(check_proof(ctx, &axiom, &lt(below.clone(), above.clone())));
    // The weak order in the conclusion, though the strict one is given.
    mismatch(check_proof(
        ctx,
        &axiom,
        &implies(
            lt(cond_lo.clone(), cond_hi.clone()),
            le(below.clone(), above.clone()),
        ),
    ));
    // The weak order in the condition, which would admit b == 0.
    mismatch(check_proof(
        ctx,
        &axiom,
        &implies(
            le(cond_lo.clone(), cond_hi.clone()),
            lt(below.clone(), above.clone()),
        ),
    ));
    // The condition on the dividend instead of the divisor.
    let swap = |t: &Term| if *t == b { a.clone() } else { t.clone() };
    mismatch(check_proof(
        ctx,
        &axiom,
        &implies(
            lt(swap(&cond_lo), swap(&cond_hi)),
            lt(below.clone(), above.clone()),
        ),
    ));
    // The bound the other way round.
    mismatch(check_proof(
        ctx,
        &axiom,
        &implies(lt(cond_lo, cond_hi), lt(above, below)),
    ));
}

#[test]
fn the_remainder_is_bounded_by_a_positive_divisor() {
    let (mut ctx, prelude) = setup();
    let v = vars(&mut ctx);
    let (a, b) = (v.a.clone(), v.b.clone());
    let r = rem(a.clone(), b.clone());
    bounded(
        &mut ctx,
        &v,
        &|t| Axiom::IntRemLowerPos(t[0].clone(), t[1].clone()),
        (lit(0), b.clone()),
        (neg(b.clone()), r.clone()),
    );
    bounded(
        &mut ctx,
        &v,
        &|t| Axiom::IntRemUpperPos(t[0].clone(), t[1].clone()),
        (lit(0), b.clone()),
        (r.clone(), b.clone()),
    );
    // The two lower bounds are different axioms.
    mismatch(check_proof(
        &mut ctx,
        &ax(Axiom::IntRemLowerPos(a.clone(), b.clone())),
        &implies(lt(b.clone(), lit(0)), lt(b.clone(), r.clone())),
    ));
    // At b == 0 the remainder is a itself, so both bounds fail: the lower
    // one at a == 0 and the upper one at every a >= 0.
    decided(
        &mut ctx,
        &prelude,
        lt(neg(lit(0)), rem(lit(0), lit(0))),
        false,
    );
    decided(&mut ctx, &prelude, lt(rem(lit(0), lit(0)), lit(0)), false);
    decided(&mut ctx, &prelude, lt(rem(lit(5), lit(0)), lit(0)), false);
    // At b == 0 the lower bound reads 0 < a: false for a <= 0, true above.
    decided(
        &mut ctx,
        &prelude,
        lt(neg(lit(0)), rem(lit(-1), lit(0))),
        false,
    );
    decided(
        &mut ctx,
        &prelude,
        lt(neg(lit(0)), rem(lit(1), lit(0))),
        true,
    );
}

#[test]
fn the_remainder_is_bounded_by_a_negative_divisor() {
    let (mut ctx, prelude) = setup();
    let v = vars(&mut ctx);
    let (a, b) = (v.a.clone(), v.b.clone());
    let r = rem(a.clone(), b.clone());
    bounded(
        &mut ctx,
        &v,
        &|t| Axiom::IntRemLowerNeg(t[0].clone(), t[1].clone()),
        (b.clone(), lit(0)),
        (b.clone(), r.clone()),
    );
    bounded(
        &mut ctx,
        &v,
        &|t| Axiom::IntRemUpperNeg(t[0].clone(), t[1].clone()),
        (b.clone(), lit(0)),
        (r.clone(), neg(b.clone())),
    );
    mismatch(check_proof(
        &mut ctx,
        &ax(Axiom::IntRemUpperNeg(a.clone(), b.clone())),
        &implies(lt(lit(0), b.clone()), lt(r.clone(), b.clone())),
    ));
    // Both bounds fail at b == 0, a == 0.
    decided(&mut ctx, &prelude, lt(lit(0), rem(lit(0), lit(0))), false);
    decided(
        &mut ctx,
        &prelude,
        lt(rem(lit(0), lit(0)), neg(lit(0))),
        false,
    );
}

#[test]
fn the_remainder_has_the_sign_of_the_dividend() {
    let (mut ctx, prelude) = setup();
    let v = vars(&mut ctx);
    let (a, b) = (v.a.clone(), v.b.clone());
    let r = rem(a.clone(), b.clone());
    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntRemNonneg(t[0].clone(), t[1].clone()),
        implies(le(lit(0), a.clone()), le(lit(0), r.clone())),
    );
    let nonneg = ax(Axiom::IntRemNonneg(a.clone(), b.clone()));
    for wrong in [
        // Without the condition.
        le(lit(0), r.clone()),
        // Strict where weak is given: 0 % b == 0.
        implies(le(lit(0), a.clone()), lt(lit(0), r.clone())),
        implies(lt(lit(0), a.clone()), lt(lit(0), r.clone())),
        // The condition on the divisor, whose sign the remainder ignores.
        implies(le(lit(0), b.clone()), le(lit(0), r.clone())),
        // The remainder the other way round.
        implies(le(lit(0), a.clone()), le(lit(0), rem(b.clone(), a.clone()))),
        // The other axiom.
        implies(le(a.clone(), lit(0)), le(r.clone(), lit(0))),
    ] {
        mismatch(check_proof(&mut ctx, &nonneg, &wrong));
    }

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntRemNonpos(t[0].clone(), t[1].clone()),
        implies(le(a.clone(), lit(0)), le(r.clone(), lit(0))),
    );
    let nonpos = ax(Axiom::IntRemNonpos(a.clone(), b.clone()));
    for wrong in [
        le(r.clone(), lit(0)),
        implies(le(a.clone(), lit(0)), lt(r.clone(), lit(0))),
        implies(le(b.clone(), lit(0)), le(r.clone(), lit(0))),
        implies(le(lit(0), a.clone()), le(lit(0), r.clone())),
    ] {
        mismatch(check_proof(&mut ctx, &nonpos, &wrong));
    }

    // The sign of the divisor does not matter, and neither does a zero one.
    for (a, b) in [(7, 2), (7, -2), (7, 0), (0, 3), (0, 0)] {
        decided(&mut ctx, &prelude, le(lit(0), rem(lit(a), lit(b))), true);
    }
    for (a, b) in [(-7, 2), (-7, -2), (-7, 0), (0, -3)] {
        decided(&mut ctx, &prelude, le(rem(lit(a), lit(b)), lit(0)), true);
    }
    // The conditions are needed: a negative dividend has a negative
    // remainder whatever the divisor does.
    decided(&mut ctx, &prelude, le(lit(0), rem(lit(-7), lit(2))), false);
    decided(&mut ctx, &prelude, le(rem(lit(7), lit(-2)), lit(0)), false);
}

// --- Each sign combination, on closed instances -----------------------------------------

/// `a / b == q` and `a % b == r` are proved by evaluation, and every other
/// quotient and remainder offered is refuted.
fn divides(
    ctx: &mut Context,
    prelude: &Prelude,
    (a, b): (i64, i64),
    (q, r): (i64, i64),
    others: &[(i64, i64)],
) {
    decided(ctx, prelude, eq(div(lit(a), lit(b)), lit(q)), true);
    decided(ctx, prelude, eq(rem(lit(a), lit(b)), lit(r)), true);
    for (other_q, other_r) in others {
        assert_ne!(*other_q, q);
        assert_ne!(*other_r, r);
        decided(ctx, prelude, eq(div(lit(a), lit(b)), lit(*other_q)), false);
        decided(ctx, prelude, eq(rem(lit(a), lit(b)), lit(*other_r)), false);
    }
    // Either way, a == q * b + r.
    assert_eq!(a, q * b + r);
    let decomposed = eq(
        lit(a),
        add(mul(div(lit(a), lit(b)), lit(b)), rem(lit(a), lit(b))),
    );
    decided(ctx, prelude, decomposed, true);
}

#[test]
fn positive_over_positive() {
    let (mut ctx, prelude) = setup();
    divides(&mut ctx, &prelude, (7, 2), (3, 1), &[(4, -1), (2, 3)]);
    divides(&mut ctx, &prelude, (6, 3), (2, 0), &[(1, 3), (3, -3)]);
    divides(&mut ctx, &prelude, (1, 9), (0, 1), &[(1, -8)]);
}

#[test]
fn negative_over_positive() {
    let (mut ctx, prelude) = setup();
    // Truncation gives -3 and -1. Flooring, and Euclid, would give -4 and 1.
    divides(&mut ctx, &prelude, (-7, 2), (-3, -1), &[(-4, 1)]);
    divides(&mut ctx, &prelude, (-1, 9), (0, -1), &[(-1, 8)]);
    divides(&mut ctx, &prelude, (-6, 3), (-2, 0), &[(-3, 3)]);
}

#[test]
fn positive_over_negative() {
    let (mut ctx, prelude) = setup();
    // Truncation gives -3 and 1, as Euclid does; flooring gives -4 and -1.
    divides(&mut ctx, &prelude, (7, -2), (-3, 1), &[(-4, -1)]);
    divides(&mut ctx, &prelude, (1, -9), (0, 1), &[(-1, -8)]);
}

#[test]
fn negative_over_negative() {
    let (mut ctx, prelude) = setup();
    // Truncation and flooring give 3 and -1; Euclid gives 4 and 1.
    divides(&mut ctx, &prelude, (-7, -2), (3, -1), &[(4, 1), (2, -3)]);
    divides(&mut ctx, &prelude, (-1, -9), (0, -1), &[(1, 8)]);
}

#[test]
fn zero_divisor_and_zero_dividend() {
    let (mut ctx, prelude) = setup();
    // Neither a / 0 == a nor a % 0 == 0, and no infinity of any kind.
    divides(
        &mut ctx,
        &prelude,
        (7, 0),
        (0, 7),
        &[(7, 0), (1, 6), (-1, -7)],
    );
    divides(&mut ctx, &prelude, (-7, 0), (0, -7), &[(-7, 0), (-1, 7)]);
    divides(&mut ctx, &prelude, (0, 0), (0, 0), &[(1, 1), (-1, -1)]);
    divides(&mut ctx, &prelude, (0, 5), (0, 0), &[(5, 5), (1, -5)]);
    divides(&mut ctx, &prelude, (0, -5), (0, 0), &[(-1, -5), (1, 5)]);
    // Division by zero is not an error and not stuck: a claim about it is
    // decided like any other.
    decided(&mut ctx, &prelude, lt(div(lit(7), lit(0)), lit(1)), true);
    decided(&mut ctx, &prelude, le(rem(lit(7), lit(0)), lit(6)), false);
}

// --- A fact derived from the axioms alone ------------------------------------------

/// `x * 0 == 0`, from the ring axioms: `x * 0 == x * 0 + (x * 0 + -(x * 0))
/// == (x * 0 + x * 0) + -(x * 0) == x * (0 + 0) + -(x * 0) == x * 0 + -(x * 0)
/// == 0`. Six axioms, twenty-one nodes.
fn mul_zero(x: &Term) -> Proof {
    let x0 = mul(x.clone(), lit(0));
    let minus = neg(x0.clone());
    Chain::new(Type::Int, x0.clone())
        .step_rev(&add(x0.clone(), lit(0)), ax(Axiom::IntAddZero(x0.clone())))
        .rewrite_rev(
            &Type::Int,
            |hole| add(x0.clone(), hole),
            &add(x0.clone(), minus.clone()),
            ax(Axiom::IntAddNeg(x0.clone())),
        )
        .step_rev(
            &add(add(x0.clone(), x0.clone()), minus.clone()),
            ax(Axiom::IntAddAssoc(x0.clone(), x0.clone(), minus.clone())),
        )
        .rewrite_rev(
            &Type::Int,
            |hole| add(hole, minus.clone()),
            &mul(x.clone(), add(lit(0), lit(0))),
            ax(Axiom::IntMulAdd(x.clone(), lit(0), lit(0))),
        )
        .rewrite(
            |hole| add(mul(x.clone(), hole), minus.clone()),
            ax(Axiom::IntAddZero(lit(0))),
        )
        .step(ax(Axiom::IntAddNeg(x0)))
        .finish()
}

/// `a % 0 == a`: the decomposition at `b == 0` reads `a == (a / 0) * 0 +
/// a % 0`; `a / 0` is `0`, `0 * 0` is `0` by `mul_zero`, and `0 + a % 0`
/// is `a % 0`. Five axioms of its own and the six of `mul_zero`, in
/// thirty-three nodes.
fn rem_zero(a: &Term) -> Proof {
    let r = rem(a.clone(), lit(0));
    let forward = Chain::new(Type::Int, a.clone())
        .step(ax(Axiom::IntDivRem(a.clone(), lit(0))))
        .rewrite(
            |hole| add(mul(hole, lit(0)), r.clone()),
            ax(Axiom::IntDivZero(a.clone())),
        )
        .rewrite(|hole| add(hole, r.clone()), mul_zero(&lit(0)))
        .step(ax(Axiom::IntAddComm(lit(0), r.clone())))
        .step(ax(Axiom::IntAddZero(r)))
        .finish();
    symm_at(&Type::Int, a, forward)
}

/// The number of rule applications in a proof, hypotheses included.
fn nodes(proof: &Proof) -> usize {
    match proof {
        Proof::Transport { eq, proof, .. } => 1 + nodes(eq) + nodes(proof),
        Proof::ImpliesElim(left, right) => 1 + nodes(left) + nodes(right),
        Proof::ImpliesIntro { body, .. } => 1 + nodes(body),
        Proof::Hyp(_) | Proof::Refl(_) | Proof::Axiom(_) | Proof::Literal(_) => 1,
        other => panic!("not used in these derivations: {other:?}"),
    }
}

#[test]
fn a_rem_zero_is_derived_from_the_decomposition_and_division_by_zero() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let (a, b) = (v.a.clone(), v.b.clone());

    assert_eq!(
        check_proof(&mut ctx, &mul_zero(&b), &eq(mul(b.clone(), lit(0)), lit(0))),
        Ok(())
    );
    assert_eq!(nodes(&mul_zero(&b)), 21);

    let derived = rem_zero(&a);
    assert_eq!(
        check_proof(&mut ctx, &derived, &eq(rem(a.clone(), lit(0)), a.clone())),
        Ok(())
    );
    assert_eq!(nodes(&derived), 33);
    // Not a % 0 == 0, and not about another number.
    mismatch(check_proof(
        &mut ctx,
        &derived,
        &eq(rem(a.clone(), lit(0)), lit(0)),
    ));
    mismatch(check_proof(
        &mut ctx,
        &derived,
        &eq(rem(b.clone(), lit(0)), b.clone()),
    ));
    // The derivation is sound on closed instances, where evaluation is the
    // judge.
    let closed = rem_zero(&lit(-42));
    assert_eq!(
        check_proof(&mut ctx, &closed, &eq(rem(lit(-42), lit(0)), lit(-42))),
        Ok(())
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(rem(lit(-42), lit(0)))),
        Ok(eq(rem(lit(-42), lit(0)), lit(-42)))
    );
}

// --- Evaluation against Integer and against Rust ---------------------------------------

fn scale() -> u64 {
    if std::env::var_os("LOCUS_EXTENDED").is_some_and(|value| !value.is_empty()) {
        100
    } else {
        1
    }
}

/// A number whose bit length is drawn first, so small numbers, numbers
/// around the machine sizes, and numbers of a few hundred bits all occur.
fn random_integer(rng: &mut Rng) -> Integer {
    let max_bits = *rng.choose(&[0, 1, 8, 64, 65, 128, 129, 400]);
    let bits = rng.below(max_bits + 1);
    let base = Integer::from(1u128 << 64);
    let mut value = Integer::zero();
    for _ in 0..bits / 64 {
        value = value
            .mul(&base)
            .add(&Integer::from(u128::from(rng.next_u64())));
    }
    let rest = bits % 64;
    if rest > 0 {
        let top = rng.next_u64() >> (64 - rest);
        value = value
            .mul(&Integer::from(1u128 << rest))
            .add(&Integer::from(u128::from(top)));
    }
    if rng.chance(1, 2) { value.neg() } else { value }
}

/// How many divisions a run of `random_term` built, and how many of them by
/// a literal zero.
#[derive(Default)]
struct Divisions {
    all: usize,
    by_zero: usize,
}

/// A closed term over every primitive and its value, computed beside it with
/// `Integer`. A divisor is a literal zero one time in six, so that total
/// division is exercised and not only reached by chance.
fn random_term(rng: &mut Rng, depth: usize, divisions: &mut Divisions) -> (Term, Integer) {
    if depth == 0 || rng.chance(1, 5) {
        let value = random_integer(rng);
        return (Term::Int(value.clone()), value);
    }
    let (left, l) = random_term(rng, depth - 1, divisions);
    if rng.chance(1, 8) {
        return (neg(left), l.neg());
    }
    let zero = rng.chance(1, 6);
    let (right, r) = if zero {
        (lit(0), Integer::zero())
    } else {
        random_term(rng, depth - 1, divisions)
    };
    let operation = rng.below(5);
    if operation >= 3 {
        divisions.all += 1;
        divisions.by_zero += usize::from(zero);
    }
    match operation {
        0 => (add(left, right), l.add(&r)),
        1 => (sub(left, right), l.sub(&r)),
        2 => (mul(left, right), l.mul(&r)),
        3 => (div(left, right), l.div(&r)),
        _ => (rem(left, right), l.rem(&r)),
    }
}

#[test]
fn evaluation_agrees_with_integer_on_random_terms_over_every_primitive() {
    let (mut ctx, prelude) = setup();
    let one = Integer::from(1i64);
    let mut divisions = Divisions::default();
    for index in 0..300 * scale() {
        let seed = case_seed(0x4B33_4449, index);
        let mut rng = Rng::new(seed);
        let depth = rng.range(1..7);
        let (term, value) = random_term(&mut rng, depth, &mut divisions);

        let evaluated = Proof::Evaluate(term.clone());
        assert_eq!(
            infer_proof(&mut ctx, &evaluated),
            Ok(eq(term.clone(), Term::Int(value.clone()))),
            "seed {seed:#x}"
        );
        mismatch(check_proof(
            &mut ctx,
            &evaluated,
            &eq(term.clone(), Term::Int(value.add(&one))),
        ));
        mismatch(check_proof(
            &mut ctx,
            &evaluated,
            &eq(term.clone(), Term::Int(value.sub(&one))),
        ));

        // The decomposition and the sign of the remainder, decided on the
        // term and a fresh divisor, which is zero one time in four.
        let divisor = if rng.chance(1, 4) {
            Integer::zero()
        } else {
            random_integer(&mut rng)
        };
        let d = Term::Int(divisor.clone());
        let decomposed = eq(
            term.clone(),
            add(
                mul(div(term.clone(), d.clone()), d.clone()),
                rem(term.clone(), d.clone()),
            ),
        );
        decided(&mut ctx, &prelude, decomposed, true);
        let remainder = rem(term.clone(), d.clone());
        let r = value.rem(&divisor);
        decided(
            &mut ctx,
            &prelude,
            le(lit(0), remainder.clone()),
            !r.is_negative(),
        );
        decided(
            &mut ctx,
            &prelude,
            le(remainder.clone(), lit(0)),
            r.is_negative() || r.is_zero(),
        );
        if !divisor.is_zero() {
            // |r| < |b|, in the form the bound axioms state it.
            let magnitude = Term::Int(Integer::from(divisor.magnitude().clone()));
            decided(
                &mut ctx,
                &prelude,
                lt(remainder.clone(), magnitude.clone()),
                true,
            );
            decided(&mut ctx, &prelude, lt(neg(magnitude), remainder), true);
        }
    }
    assert!(divisions.all > 200, "{}", divisions.all);
    assert!(divisions.by_zero > 30, "{}", divisions.by_zero);
}

#[test]
fn evaluation_agrees_with_rust_on_random_i128_pairs() {
    let (mut ctx, prelude) = setup();
    let term = |value: i128| Term::Int(Integer::from(value));
    let one = Integer::from(1i64);
    let random = |rng: &mut Rng| {
        // Small, at a boundary, or of the full width.
        match rng.below(4) {
            0 => (rng.next_u64() % 2001) as i128 - 1000,
            1 => *rng.choose(&[i128::MIN, i128::MAX, -1, 0, 1, 2, i128::from(i64::MIN)]),
            _ => ((u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64())) as i128,
        }
    };
    let mut compared = 0;
    for index in 0..400 * scale() {
        let mut rng = Rng::new(case_seed(0x4B33_5255, index));
        let (a, b) = (random(&mut rng), random(&mut rng));
        if b == 0 || (a == i128::MIN && b == -1) {
            continue;
        }
        compared += 1;
        let (q, r) = (Integer::from(a / b), Integer::from(a % b));
        for (claim, holds) in [
            (eq(div(term(a), term(b)), Term::Int(q.clone())), true),
            (eq(rem(term(a), term(b)), Term::Int(r.clone())), true),
            (eq(div(term(a), term(b)), Term::Int(q.add(&one))), false),
            (eq(rem(term(a), term(b)), Term::Int(r.sub(&one))), false),
        ] {
            decided(&mut ctx, &prelude, claim, holds);
        }
    }
    assert!(compared > 350, "{compared}");

    // The one pair on which Rust overflows: the logic has 2^127.
    let past: Integer = "170141183460469231731687303715884105728".parse().unwrap();
    assert_eq!(past.to_i128(), None);
    decided(
        &mut ctx,
        &prelude,
        eq(div(term(i128::MIN), term(-1)), Term::Int(past)),
        true,
    );
    decided(
        &mut ctx,
        &prelude,
        eq(rem(term(i128::MIN), term(-1)), lit(0)),
        true,
    );
}

// --- The step charge ---------------------------------------------------------------

/// `2^(2^exponent)`, computed on the test side.
fn power_tower(exponent: u32) -> Integer {
    (0..exponent).fold(Integer::from(2i64), |n, _| n.mul(&n))
}

#[test]
fn a_huge_division_is_charged_and_hits_the_step_limit() {
    let (mut definitions, _) = Definitions::with_prelude();
    // `x / x` and `x % x` on one evaluated argument, so that the argument
    // is evaluated once and the charge is the division's own.
    let self_div = definitions
        .declare_fn(&Type::function(1, |_| Type::Int), |params| {
            div(params[0].clone(), params[0].clone())
        })
        .unwrap();
    let self_rem = definitions
        .declare_fn(&Type::function(1, |_| Type::Int), |params| {
            rem(params[0].clone(), params[0].clone())
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    // 2^(2^17) has 131073 bits and 4097 digits. Dividing it by a small
    // number is charged 131073 steps and evaluates; dividing it by itself
    // would cost 131073 * 4097 steps and is refused before any work is done.
    let tower = power_tower(17);
    assert_eq!(tower.magnitude().bit_length(), 131073);
    let huge = Term::Int(tower.clone());
    let small = div(huge.clone(), lit(3));
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(small.clone())),
        Ok(eq(small, Term::Int(tower.div(&Integer::from(3i64)))))
    );
    for divided in [
        Term::call(Term::Fn(self_div), vec![huge.clone()]),
        Term::call(Term::Fn(self_rem), vec![huge.clone()]),
        div(huge.clone(), huge.clone()),
        rem(lit(1), huge.clone()),
    ] {
        let outcome = infer_proof(&mut ctx, &Proof::Evaluate(divided.clone()));
        let expected = if matches!(divided, Term::Prim(Prim::IntRem, _)) {
            // 1 % huge: one bit of dividend, so a small charge.
            Ok(eq(divided, lit(1)))
        } else {
            Err(KernelError::StepLimit)
        };
        assert_eq!(outcome, expected);
    }
    // In a comparison as well.
    assert_eq!(
        infer_proof(
            &mut ctx,
            &Proof::Evaluate(le(div(huge.clone(), huge), lit(1)))
        ),
        Err(KernelError::StepLimit)
    );
    // A division of the same shape but a hundredth of the size is well
    // within the budget: 4097 bits times 129 digits.
    let large = Term::Int(power_tower(12));
    assert_eq!(
        infer_proof(
            &mut ctx,
            &Proof::Evaluate(div(large.clone(), large.clone()))
        ),
        Ok(eq(div(large.clone(), large), lit(1)))
    );
}
