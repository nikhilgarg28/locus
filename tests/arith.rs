//! Tests for the arithmetic procedure (build task K7): certificates for the
//! rule `linear` are found, not written by hand.
//!
//! The seven arithmetic obligations of the target examples are set up as
//! `tests/kernel_linear.rs` sets them up by hand, and found here. Random
//! problems on the box `[-8, 8]^n` are decided by brute force: a false goal
//! is never proved, and the share of true goals found is printed and must
//! be at least 95 percent. A counterexample reported for a false goal
//! satisfies every collected constraint and violates the goal. A problem
//! built to blow up hits the budget of derived constraints, names it, lists
//! the constraints, and says the same thing twice. Machine ranges at 32 and
//! 64 bits and division by a literal are found. Set LOCUS_EXTENDED to run a
//! hundred times as many random cases.

use std::rc::Rc;

use locus::arith::{Budget, Counterexample, GaveUp, Reason, prove};
use locus::kernel::{
    Axiom, Context, Definitions, Integer, MachineInt, Prelude, Proof, Term, Type, check_proof,
};

#[path = "common/rng.rs"]
mod rng;
use rng::{Rng, case_seed};

use MachineInt::{U32, U64};

fn setup() -> (Context, Prelude) {
    let (definitions, prelude) = Definitions::with_prelude();
    (Context::with_definitions(Rc::new(definitions)), prelude)
}

fn extended() -> bool {
    std::env::var_os("LOCUS_EXTENDED").is_some()
}

fn lit(value: i128) -> Term {
    Term::Int(Integer::from(value))
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

fn le(left: Term, right: Term) -> Term {
    Term::int_le(left, right)
}

fn lt(left: Term, right: Term) -> Term {
    Term::int_lt(left, right)
}

fn eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Int, left, right)
}

fn view(ty: MachineInt, x: &Term) -> Term {
    Term::view(ty, x.clone())
}

const MAX_U32: i128 = 4294967295;

/// Proves the goal with the default budget and checks the proof again
/// here, so that the test does not rely on the procedure's own check.
fn found(ctx: &Context, prelude: Prelude, goal: &Term) -> Proof {
    let proof = prove(ctx, Some(prelude), goal, &Budget::default())
        .unwrap_or_else(|gave_up| panic!("{goal}:\n{gave_up}"));
    let mut scratch = ctx.clone();
    check_proof(&mut scratch, &proof, goal).unwrap_or_else(|error| panic!("{goal}: {error}"));
    proof
}

/// The number of pairs and the largest coefficient, over every
/// certificate inside a proof the procedure builds: a certificate, one
/// under a case with no arms, or two joined by antisymmetry.
fn size(proof: &Proof) -> (usize, Integer) {
    fn walk(proof: &Proof, pairs: &mut usize, largest: &mut Integer) {
        match proof {
            Proof::Linear {
                goal_coefficient,
                pairs: found,
                ..
            } => {
                *pairs += found.len();
                for value in found
                    .iter()
                    .map(|(_, c)| c)
                    .chain(std::iter::once(goal_coefficient))
                {
                    let magnitude = Integer::from(value.magnitude().clone());
                    if magnitude > *largest {
                        *largest = magnitude;
                    }
                }
            }
            Proof::CaseProof { scrutinee, .. } => walk(scrutinee, pairs, largest),
            Proof::ImpliesElim(left, right) => {
                walk(left, pairs, largest);
                walk(right, pairs, largest);
            }
            Proof::Axiom(_) => {}
            other => panic!("unexpected proof shape {}", other.rule_name()),
        }
    }
    let (mut pairs, mut largest) = (0, Integer::zero());
    walk(proof, &mut pairs, &mut largest);
    (pairs, largest)
}

// --- The seven obligations of the target examples ----------------------------------------

/// The context of `midpoint` as `tests/kernel_linear.rs` builds it: `lo`,
/// `hi`, `hi - lo`, `half`, and `mid` are variables of `u32`, the exact
/// results of the three operations are hypotheses about their views, and
/// the bridge from `lo <= hi` to `L <= H` is a hypothesis. Nothing else is
/// assumed: the range and division facts are the procedure's to find. The
/// context is built up to a `stage`, so that each obligation is posed with
/// the facts the program has at that point: 0 for `hi - lo`, where only
/// `ordered` is known, 1 for `lo + half`, where the results of `hi - lo`
/// and `/ 2` are known, and 2 for the claim, where `lo + half` is known
/// too.
struct Midpoint {
    ctx: Context,
    prelude: Prelude,
    l: Term,
    h: Term,
    f: Term,
    m: Term,
    q_of_sum: Term,
}

fn midpoint(stage: usize) -> Midpoint {
    let (mut ctx, prelude) = setup();
    let u32 = Type::machine(U32);
    let var = |ctx: &mut Context| Term::var(ctx.declare(u32.clone()).unwrap());
    let (lo, hi, d, half, mid) = (
        var(&mut ctx),
        var(&mut ctx),
        var(&mut ctx),
        var(&mut ctx),
        var(&mut ctx),
    );
    let (l, h, dv, f, m) = (
        view(U32, &lo),
        view(U32, &hi),
        view(U32, &d),
        view(U32, &half),
        view(U32, &mid),
    );
    let q = div(dv.clone(), lit(2));
    let s = add(l.clone(), h.clone());
    let big_q = div(s, lit(2));
    ctx.assume(le(l.clone(), h.clone())).unwrap();
    if stage >= 1 {
        ctx.assume(eq(dv.clone(), sub(h.clone(), l.clone())))
            .unwrap();
        ctx.assume(eq(f.clone(), q)).unwrap();
    }
    if stage >= 2 {
        ctx.assume(eq(m.clone(), add(l.clone(), f.clone())))
            .unwrap();
    }
    Midpoint {
        ctx,
        prelude,
        l,
        h,
        f,
        m,
        q_of_sum: big_q,
    }
}

#[test]
fn the_seven_arithmetic_obligations_of_the_target_examples_are_found() {
    let mut sizes = Vec::new();
    let mut record = |name: &str, proof: &Proof| {
        let (pairs, largest) = size(proof);
        println!("{name}: {pairs} pairs, largest coefficient {largest}");
        sizes.push((name.to_string(), pairs, largest));
    };

    // The lock: from lock.failures < 3, its view plus one fits in u32.
    let (mut ctx, prelude) = setup();
    let u32 = Type::machine(U32);
    let failures = Term::var(ctx.declare(u32.clone()).unwrap());
    let fv = view(U32, &failures);
    ctx.assume(lt(fv.clone(), lit(3))).unwrap();
    let fits = le(add(fv.clone(), lit(1)), lit(MAX_U32));
    record("fits", &found(&ctx, prelude, &fits));
    // next.failures <= 3, where next.failures is lock.failures + 1 exactly.
    let next = Term::var(ctx.declare(u32.clone()).unwrap());
    let nv = view(U32, &next);
    ctx.assume(eq(nv.clone(), add(fv.clone(), lit(1)))).unwrap();
    record("next.failures <= 3", &found(&ctx, prelude, &le(nv, lit(3))));

    // remaining: 3 - failures does not go below zero, from failures <= 3,
    // and left <= 3, where left is 3 - failures exactly.
    let (mut ctx, prelude) = setup();
    let failures = Term::var(ctx.declare(u32.clone()).unwrap());
    let fv = view(U32, &failures);
    ctx.assume(le(fv.clone(), lit(3))).unwrap();
    let nonneg = le(lit(0), sub(lit(3), fv.clone()));
    record("3 - failures", &found(&ctx, prelude, &nonneg));
    let left = Term::var(ctx.declare(u32).unwrap());
    let lv = view(U32, &left);
    ctx.assume(eq(lv.clone(), sub(lit(3), fv))).unwrap();
    record("left <= 3", &found(&ctx, prelude, &le(lv, lit(3))));

    // midpoint: the obligations of hi - lo and lo + half, and the claim,
    // each with the facts the program has at that point.
    let example = midpoint(0);
    let goal = le(lit(0), sub(example.h.clone(), example.l.clone()));
    record("hi - lo", &found(&example.ctx, example.prelude, &goal));
    let example = midpoint(1);
    let goal = le(add(example.l.clone(), example.f.clone()), lit(MAX_U32));
    let proof = found(&example.ctx, example.prelude, &goal);
    record("lo + half", &proof);
    assert!(size(&proof).0 >= 5, "the sum needs the division facts");
    let example = midpoint(2);
    let claim = eq(example.m.clone(), example.q_of_sum.clone());
    let proof = found(&example.ctx, example.prelude, &claim);
    record("M == Q", &proof);
    assert!(
        matches!(proof, Proof::ImpliesElim(..)),
        "an equation is proved by antisymmetry"
    );

    assert_eq!(sizes.len(), 7);
    for (name, pairs, largest) in &sizes {
        assert!(*pairs <= 20, "{name}: {pairs} pairs");
        assert!(*largest <= Integer::from(8i64), "{name}: {largest}");
    }
}

#[test]
fn the_midpoint_halves_are_found_with_certificates_the_size_of_the_contracts() {
    let example = midpoint(2);
    let (m, q) = (&example.m, &example.q_of_sum);
    for (name, goal) in [
        ("M <= Q", le(m.clone(), q.clone())),
        ("Q <= M", le(q.clone(), m.clone())),
    ] {
        let proof = found(&example.ctx, example.prelude, &goal);
        let (pairs, largest) = size(&proof);
        println!("{name}: {pairs} pairs, largest coefficient {largest}");
        // The contract's certificates have seven pairs and coefficients up
        // to 2; the found ones may differ, but not by much.
        assert!(pairs <= 10, "{name}: {pairs} pairs");
        assert!(largest <= Integer::from(4i64), "{name}: {largest}");
    }
}

// --- Goal shapes and hypothesis shapes -----------------------------------------------

#[test]
fn every_goal_shape_and_hypothesis_shape_is_handled() {
    let (mut ctx, prelude) = setup();
    let x = Term::var(ctx.declare(Type::Int).unwrap());
    let y = Term::var(ctx.declare(Type::Int).unwrap());
    // A conjunction is split; a negated comparison is a constraint.
    ctx.assume(prelude.and_prop(le(x.clone(), lit(5)), le(y.clone(), lit(5))))
        .unwrap();
    ctx.assume(prelude.not_prop(le(y.clone(), lit(2)))).unwrap();
    found(&ctx, prelude, &le(add(x.clone(), y.clone()), lit(10)));
    found(&ctx, prelude, &le(lit(3), y.clone()));
    found(&ctx, prelude, &lt(lit(2), y.clone()));
    // An equation goal, and one that is not provable.
    ctx.assume(le(lit(5), y.clone())).unwrap();
    found(&ctx, prelude, &eq(y.clone(), lit(5)));
    let gave_up = prove(
        &ctx,
        Some(prelude),
        &eq(x.clone(), lit(5)),
        &Budget::default(),
    )
    .expect_err("x == 5 does not follow");
    assert!(matches!(gave_up.reason, Reason::Consistent), "{gave_up}");
    // False from contradictory facts, and anything from them by a case
    // with no arms.
    ctx.assume(le(lit(6), x.clone())).unwrap();
    let proof = found(&ctx, prelude, &prelude.falsehood_prop());
    assert!(matches!(proof, Proof::Linear { .. }));
    let proof = found(&ctx, prelude, &le(lit(100), y.clone()));
    assert!(matches!(
        proof,
        Proof::CaseProof { ref arms, .. } if arms.is_empty()
    ));
    // A goal of another shape is refused with its own reason.
    let gave_up = prove(
        &ctx,
        Some(prelude),
        &prelude.truth_prop(),
        &Budget::default(),
    )
    .expect_err("True is not a linear goal");
    assert!(matches!(gave_up.reason, Reason::NotLinear(_)), "{gave_up}");
    assert!(
        gave_up
            .to_string()
            .starts_with("the arithmetic procedure gave up: the goal "),
        "{gave_up}"
    );
}

#[test]
fn evidence_carried_by_variables_is_read() {
    // A parameter `ordered: @(lo <= hi)` is a variable of proof type, and
    // a value `(left, @(left <= 3))` carries its proof as a field.
    let (mut ctx, prelude) = setup();
    let lo = Term::var(ctx.declare(Type::Int).unwrap());
    let hi = Term::var(ctx.declare(Type::Int).unwrap());
    ctx.declare(Type::proof(le(lo.clone(), hi.clone())))
        .unwrap();
    found(&ctx, prelude, &le(lit(0), sub(hi.clone(), lo.clone())));
    let pair = Type::tuple(|earlier| match earlier {
        [] => Some(Type::Int),
        [left] => Some(Type::proof(le(left.clone(), lit(3)))),
        _ => None,
    });
    let value = Term::var(ctx.declare(pair).unwrap());
    let proof = found(&ctx, prelude, &lt(Term::proj(value.clone(), 0), lit(4)));
    assert!(matches!(
        proof,
        Proof::Linear { ref pairs, .. }
            if pairs.len() == 1 && matches!(pairs[0].0, Proof::OfTerm(Term::Proj(..)))
    ));
}

#[test]
fn a_goal_true_over_the_integers_only_is_proved_by_cases() {
    // 4x <= 3 and 4y <= 3 give x <= 0 and y <= 0 over the integers, but
    // over the rationals x + y can reach 3/2. No single certificate proves
    // x + y <= 0: the proof splits on an atom by int_le_total, and each
    // case has one.
    let (mut ctx, prelude) = setup();
    let x = Term::var(ctx.declare(Type::Int).unwrap());
    let y = Term::var(ctx.declare(Type::Int).unwrap());
    ctx.assume(le(mul(lit(4), x.clone()), lit(3))).unwrap();
    ctx.assume(le(mul(lit(4), y.clone()), lit(3))).unwrap();
    let goal = le(add(x.clone(), y.clone()), lit(0));
    let proof = found(&ctx, prelude, &goal);
    let Proof::CaseProof {
        scrutinee, arms, ..
    } = &proof
    else {
        panic!("a case split, found {}", proof.rule_name());
    };
    assert!(matches!(**scrutinee, Proof::Axiom(Axiom::IntLeTotal(..))));
    assert_eq!(arms.len(), 2);
    // With no splits allowed, the procedure says which count ran out.
    let none = Budget {
        branches: 0,
        ..Budget::default()
    };
    let gave_up = prove(&ctx, Some(prelude), &goal, &none).expect_err("no splits");
    assert!(
        matches!(
            gave_up.reason,
            Reason::Budget {
                name: "branches",
                limit: 0
            }
        ),
        "{gave_up}"
    );
    assert!(
        matches!(gave_up.counterexample, Counterexample::NoneInBox),
        "{gave_up}"
    );
    // The same over the views of two i8 variables, with the range axioms
    // as the only bounds.
    let (mut ctx, prelude) = setup();
    let x = Term::var(ctx.declare(Type::machine(MachineInt::I8)).unwrap());
    let y = Term::var(ctx.declare(Type::machine(MachineInt::I8)).unwrap());
    let (xv, yv) = (view(MachineInt::I8, &x), view(MachineInt::I8, &y));
    ctx.assume(le(mul(lit(4), xv.clone()), lit(3))).unwrap();
    ctx.assume(le(mul(lit(4), yv.clone()), lit(3))).unwrap();
    found(&ctx, prelude, &le(add(xv, yv), lit(0)));
}

#[test]
fn a_tautology_needs_no_facts_and_no_prelude() {
    let ctx = Context::new();
    let proof = prove(&ctx, None, &le(lit(3), lit(4)), &Budget::default()).unwrap();
    assert!(matches!(proof, Proof::Linear { ref pairs, .. } if pairs.is_empty()));
    let gave_up =
        prove(&ctx, None, &le(lit(4), lit(3)), &Budget::default()).expect_err("4 <= 3 is false");
    assert!(matches!(gave_up.reason, Reason::Consistent), "{gave_up}");
    assert!(
        gave_up.to_string().ends_with(
            "it had no constraints\na counterexample: the goal is false as it stands, with no atoms"
        ),
        "{gave_up}"
    );
}

// --- Machine ranges at 32 and 64 bits, LOC-72 -----------------------------------------

#[test]
fn machine_ranges_at_32_and_64_bits_are_found() {
    let (mut ctx, prelude) = setup();
    let x = Term::var(ctx.declare(Type::machine(U32)).unwrap());
    let y = Term::var(ctx.declare(Type::machine(U32)).unwrap());
    let (xv, yv) = (view(U32, &x), view(U32, &y));
    ctx.assume(le(xv.clone(), lit(5))).unwrap();
    ctx.assume(le(yv.clone(), lit(5))).unwrap();
    let proof = found(
        &ctx,
        prelude,
        &le(add(xv.clone(), yv.clone()), lit(MAX_U32)),
    );
    let (pairs, largest) = size(&proof);
    println!("u32 sum: {pairs} pairs, largest coefficient {largest}");
    // Without the hypotheses the sum can overflow, and the counterexample
    // comes from the elimination: it lies far outside the box.
    let (mut ctx, prelude) = setup();
    let x = Term::var(ctx.declare(Type::machine(U32)).unwrap());
    let y = Term::var(ctx.declare(Type::machine(U32)).unwrap());
    let goal = le(add(view(U32, &x), view(U32, &y)), lit(MAX_U32));
    let gave_up = prove(&ctx, Some(prelude), &goal, &Budget::default()).expect_err("overflows");
    assert!(matches!(gave_up.reason, Reason::Consistent), "{gave_up}");
    assert_eq!(gave_up.constraints.len(), 4, "{gave_up}");
    let Counterexample::Found(assignment) = &gave_up.counterexample else {
        panic!("{gave_up}");
    };
    let total: i128 = assignment.iter().map(|(_, v)| v.to_i128().unwrap()).sum();
    assert!(total > MAX_U32, "{gave_up}");
    assert!(
        assignment
            .iter()
            .all(|(_, v)| (0..=MAX_U32).contains(&v.to_i128().unwrap())),
        "{gave_up}"
    );

    // At 64 bits: view x + 1 <= 2^64 from the range of u64 alone.
    let (mut ctx, prelude) = setup();
    let x = Term::var(ctx.declare(Type::machine(U64)).unwrap());
    let two_64 = Term::Int(Integer::from(1u128 << 64));
    let proof = found(&ctx, prelude, &le(add(view(U64, &x), lit(1)), two_64));
    let (pairs, largest) = size(&proof);
    println!("u64 successor: {pairs} pairs, largest coefficient {largest}");
    assert_eq!(pairs, 1);
    // And the signed range: -2^63 - 1 < view x for x : i64.
    let x = Term::var(ctx.declare(Type::machine(MachineInt::I64)).unwrap());
    let below_min = Term::Int(Integer::from(i128::from(i64::MIN) - 1));
    found(&ctx, prelude, &lt(below_min, view(MachineInt::I64, &x)));
}

// --- Division by a literal -------------------------------------------------------------

#[test]
fn division_by_a_literal_is_found() {
    let (mut ctx, prelude) = setup();
    let d = Term::var(ctx.declare(Type::Int).unwrap());
    ctx.assume(le(lit(0), d.clone())).unwrap();
    for goal in [
        le(div(d.clone(), lit(2)), d.clone()),
        le(rem(d.clone(), lit(2)), lit(1)),
        eq(
            d.clone(),
            add(mul(lit(2), div(d.clone(), lit(2))), rem(d.clone(), lit(2))),
        ),
        le(lit(0), div(d.clone(), lit(2))),
        // A negative divisor: the remainder is still non-negative for a
        // non-negative dividend, and below the divisor's magnitude.
        le(rem(d.clone(), lit(-3)), lit(2)),
        le(lit(0), rem(d.clone(), lit(-3))),
    ] {
        let proof = found(&ctx, prelude, &goal);
        let (pairs, largest) = size(&proof);
        println!("{goal}: {pairs} pairs, largest coefficient {largest}");
    }
    // Without the sign of d, the quotient can be negative.
    let (mut ctx, prelude) = setup();
    let d = Term::var(ctx.declare(Type::Int).unwrap());
    let goal = le(lit(0), div(d.clone(), lit(2)));
    let gave_up =
        prove(&ctx, Some(prelude), &goal, &Budget::default()).expect_err("d may be negative");
    assert!(matches!(gave_up.reason, Reason::Consistent), "{gave_up}");
    let Counterexample::Found(assignment) = &gave_up.counterexample else {
        panic!("{gave_up}");
    };
    assert!(
        assignment
            .iter()
            .any(|(atom, value)| *atom == div(d.clone(), lit(2)) && value.is_negative())
    );
    // But the remainder is bounded on both sides regardless.
    found(&ctx, prelude, &lt(lit(-2), rem(d.clone(), lit(2))));
    found(&ctx, prelude, &lt(rem(d.clone(), lit(2)), lit(2)));
    // A division by zero has no facts, and a division by a variable none.
    let k = Term::var(ctx.declare(Type::Int).unwrap());
    for goal in [
        le(rem(d.clone(), lit(0)), lit(1)),
        le(rem(d.clone(), k.clone()), k.clone()),
    ] {
        let gave_up = prove(&ctx, Some(prelude), &goal, &Budget::default()).expect_err("no facts");
        assert!(gave_up.constraints.is_empty(), "{gave_up}");
    }
}

#[test]
fn the_condition_of_a_remainder_sign_is_found_by_a_nested_run() {
    // 0 <= (a + b) % 4 needs 0 <= a + b, which is not a fact but follows
    // from 0 <= a and 0 <= b.
    let (mut ctx, prelude) = setup();
    let a = Term::var(ctx.declare(Type::Int).unwrap());
    let b = Term::var(ctx.declare(Type::Int).unwrap());
    ctx.assume(le(lit(0), a.clone())).unwrap();
    ctx.assume(le(lit(0), b.clone())).unwrap();
    let s = add(a.clone(), b.clone());
    let proof = found(&ctx, prelude, &le(lit(0), rem(s.clone(), lit(4))));
    let Proof::Linear { pairs, .. } = &proof else {
        panic!("a certificate");
    };
    // The pair is int_rem_nonneg with its condition a certificate of its own.
    assert!(pairs.iter().any(|(proof, _)| matches!(
        proof,
        Proof::ImpliesElim(axiom, condition)
            if matches!(**axiom, Proof::Axiom(Axiom::IntRemNonneg(..)))
                && matches!(**condition, Proof::Linear { .. })
    )));
    // With no depth for a nested run, the sign is not known.
    let shallow = Budget {
        depth: 0,
        ..Budget::default()
    };
    let gave_up = prove(&ctx, Some(prelude), &le(lit(0), rem(s, lit(4))), &shallow)
        .expect_err("no nested run");
    assert!(
        matches!(
            gave_up.reason,
            Reason::Budget {
                name: "depth",
                limit: 0
            }
        ),
        "{gave_up}"
    );
}

// --- Random problems on the box ------------------------------------------------------------

const BOUND: i128 = 8;

/// A random problem: `n` variables of `Int`, each with `-8 <= x` and
/// `x <= 8` in the context, some random constraints, and a goal.
struct Problem {
    ctx: Context,
    prelude: Prelude,
    atoms: Vec<Term>,
    /// Each constraint as `constant + sum coefficient * x` and whether it
    /// must be non-negative or zero.
    constraints: Vec<(Vec<i128>, i128, bool)>,
    goal: Term,
    /// The negated goal, non-negative when the goal fails.
    negated: (Vec<i128>, i128),
}

/// A random side: a constant and a coefficient in -3..=3 for each
/// variable, as a term in a random order of operations.
fn random_side(rng: &mut Rng, atoms: &[Term]) -> (Vec<i128>, i128, Term) {
    let coefficients: Vec<i128> = (0..atoms.len())
        .map(|_| rng.range(0..7) as i128 - 3)
        .collect();
    let constant = rng.range(0..21) as i128 - 10;
    let mut term = lit(constant);
    for (atom, c) in atoms.iter().zip(&coefficients) {
        let scaled = match c {
            0 => continue,
            1 => atom.clone(),
            -1 => Term::int_neg(atom.clone()),
            c if rng.chance(1, 2) => mul(lit(*c), atom.clone()),
            c => mul(atom.clone(), lit(*c)),
        };
        term = if rng.chance(1, 2) {
            add(term, scaled)
        } else {
            sub(term, Term::int_neg(scaled))
        };
    }
    (coefficients, constant, term)
}

fn random_problem(rng: &mut Rng) -> Problem {
    let (mut ctx, prelude) = setup();
    let n = rng.range(2..5);
    let atoms: Vec<Term> = (0..n)
        .map(|_| Term::var(ctx.declare(Type::Int).unwrap()))
        .collect();
    let mut constraints = Vec::new();
    for (index, x) in atoms.iter().enumerate() {
        let unit = |c: i128| {
            let mut row = vec![0; n];
            row[index] = c;
            row
        };
        ctx.assume(le(lit(-BOUND), x.clone())).unwrap();
        constraints.push((unit(1), BOUND, false));
        ctx.assume(le(x.clone(), lit(BOUND))).unwrap();
        constraints.push((unit(-1), BOUND, false));
    }
    let difference = |l: (&[i128], i128), r: (&[i128], i128)| -> (Vec<i128>, i128) {
        (l.0.iter().zip(r.0).map(|(a, b)| a - b).collect(), l.1 - r.1)
    };
    for _ in 0..rng.range(1..5) {
        let (lc, lk, left) = random_side(rng, &atoms);
        let (rc, rk, right) = random_side(rng, &atoms);
        // right - left >= 0, or == 0, or left - right - 1 >= 0.
        match rng.range(0..4) {
            0 => {
                ctx.assume(eq(left, right)).unwrap();
                constraints.push((difference((&rc, rk), (&lc, lk)).0, rk - lk, true));
            }
            1 => {
                ctx.assume(prelude.not_prop(le(left, right))).unwrap();
                let (row, k) = difference((&lc, lk), (&rc, rk));
                constraints.push((row, k - 1, false));
            }
            _ => {
                ctx.assume(le(left, right)).unwrap();
                constraints.push((difference((&rc, rk), (&lc, lk)).0, rk - lk, false));
            }
        }
    }
    let (lc, lk, left) = random_side(rng, &atoms);
    let (rc, rk, right) = random_side(rng, &atoms);
    let (row, k) = difference((&lc, lk), (&rc, rk));
    Problem {
        ctx,
        prelude,
        atoms,
        constraints,
        goal: le(left, right),
        negated: (row, k - 1),
    }
}

fn value(row: &[i128], k: i128, point: &[i128]) -> i128 {
    k + row.iter().zip(point).map(|(c, x)| c * x).sum::<i128>()
}

/// Every point of the box, as an odometer.
fn points(n: usize, mut visit: impl FnMut(&[i128])) {
    let mut point = vec![-BOUND; n];
    loop {
        visit(&point);
        let mut carry = 0;
        while carry < n {
            point[carry] += 1;
            if point[carry] <= BOUND {
                break;
            }
            point[carry] = -BOUND;
            carry += 1;
        }
        if carry == n {
            return;
        }
    }
}

impl Problem {
    fn satisfies(&self, point: &[i128]) -> bool {
        self.constraints.iter().all(|(row, k, is_equation)| {
            let v = value(row, *k, point);
            if *is_equation { v == 0 } else { v >= 0 }
        })
    }

    /// Whether the goal holds at every point of the box that satisfies the
    /// constraints.
    fn is_true(&self) -> bool {
        let mut holds = true;
        points(self.atoms.len(), |point| {
            if holds && self.satisfies(point) && value(&self.negated.0, self.negated.1, point) >= 0
            {
                holds = false;
            }
        });
        holds
    }
}

#[test]
fn random_problems_on_the_box_are_decided_soundly_and_mostly_completely() {
    const SEED: u64 = 0x6b37_2026_0001;
    let cases = if extended() { 30_000 } else { 300 };
    let budget = Budget::default();
    let (mut true_count, mut found_count, mut false_count, mut counterexamples) = (0, 0, 0, 0);
    let mut unfound = Vec::new();
    for case in 0..cases {
        let mut rng = Rng::new(case_seed(SEED, case));
        let problem = random_problem(&mut rng);
        let truth = problem.is_true();
        let result = prove(&problem.ctx, Some(problem.prelude), &problem.goal, &budget);
        match result {
            Ok(proof) => {
                // Checked here as well, against the goal, in the context.
                let mut ctx = problem.ctx.clone();
                check_proof(&mut ctx, &proof, &problem.goal)
                    .unwrap_or_else(|error| panic!("case {case}: {error}"));
                assert!(
                    truth,
                    "case {case}: a false goal {} was proved",
                    problem.goal
                );
                found_count += 1;
                true_count += 1;
            }
            Err(gave_up) => {
                if truth {
                    true_count += 1;
                    unfound.push((case, gave_up.reason.to_string()));
                } else {
                    false_count += 1;
                    if !matches!(gave_up.counterexample, Counterexample::Found(_)) {
                        println!("  false case {case} without a counterexample: {gave_up}");
                    }
                }
                assert!(
                    matches!(gave_up.reason, Reason::Consistent | Reason::Budget { .. }),
                    "case {case}: {gave_up}"
                );
                // A counterexample satisfies every constraint and violates
                // the goal, by evaluation over the test's own reading.
                if let Counterexample::Found(assignment) = &gave_up.counterexample {
                    counterexamples += 1;
                    let point: Vec<i128> = problem
                        .atoms
                        .iter()
                        .map(|atom| {
                            assignment
                                .iter()
                                .find(|(term, _)| term == atom)
                                .and_then(|(_, value)| value.to_i128())
                                .unwrap_or_else(|| panic!("case {case}: {atom} unassigned"))
                        })
                        .collect();
                    assert!(problem.satisfies(&point), "case {case}: {gave_up}");
                    assert!(
                        value(&problem.negated.0, problem.negated.1, &point) >= 0,
                        "case {case}: {gave_up}"
                    );
                    assert!(!truth, "case {case}: a counterexample to a true goal");
                }
            }
        }
    }
    let share = 100.0 * found_count as f64 / true_count.max(1) as f64;
    println!(
        "{cases} random problems: {true_count} true, {found_count} found ({share:.1}%), \
         {false_count} false with {counterexamples} counterexamples"
    );
    for (case, reason) in unfound.iter().take(5) {
        println!("  unfound case {case}: {reason}");
    }
    assert!(true_count > 0 && false_count > 0);
    assert!(
        share >= 95.0,
        "only {share:.1}% of the true goals were found"
    );
}

// --- The budget ----------------------------------------------------------------------------

/// A problem built to blow up: `n` variables of `Int`, `x_i - x_j <= 1`
/// for every ordered pair, and a false goal about all of them, so that no
/// elimination ends early.
fn blow_up(n: usize) -> (Context, Prelude, Term) {
    let (mut ctx, prelude) = setup();
    let xs: Vec<Term> = (0..n)
        .map(|_| Term::var(ctx.declare(Type::Int).unwrap()))
        .collect();
    for (i, x) in xs.iter().enumerate() {
        for (j, y) in xs.iter().enumerate() {
            if i != j {
                ctx.assume(le(sub(x.clone(), y.clone()), lit(1))).unwrap();
            }
        }
    }
    let sum = xs
        .iter()
        .cloned()
        .reduce(|acc, x| add(acc, mul(lit(2), x)))
        .unwrap();
    (ctx, prelude, le(sum, lit(0)))
}

#[test]
fn a_problem_that_blows_up_hits_the_budget_of_derived_constraints_and_says_so_twice() {
    let (ctx, prelude, goal) = blow_up(10);
    let budget = Budget::default();
    let first = prove(&ctx, Some(prelude), &goal, &budget).expect_err("blows up");
    let GaveUp {
        reason: Reason::Budget { name, limit },
        constraints,
        ..
    } = &first
    else {
        panic!("{first}");
    };
    assert_eq!(*name, "derived");
    assert_eq!(*limit, budget.derived);
    assert_eq!(constraints.len(), 90);
    let text = first.to_string();
    assert!(
        text.starts_with(&format!(
            "the arithmetic procedure gave up: the budget `derived` of {} ran out; it had:\n  ",
            budget.derived
        )),
        "{text}"
    );
    assert_eq!(
        text.lines().filter(|line| line.starts_with("  ")).count(),
        90,
        "one indented line per constraint; budget notices are separate"
    );
    assert!(text.contains("MAX_COUNTEREXAMPLE_ATOMS"));
    let second = prove(&ctx, Some(prelude), &goal, &budget).expect_err("blows up again");
    assert_eq!(second.to_string(), text);
    // A smaller instance of the same problem is decided: the goal is false.
    let (ctx, prelude, goal) = blow_up(4);
    let decided = prove(&ctx, Some(prelude), &goal, &budget).expect_err("false");
    assert!(matches!(decided.reason, Reason::Consistent), "{decided}");
    assert!(
        matches!(decided.counterexample, Counterexample::Found(_)),
        "{decided}"
    );
    // The other counts are named by name.
    let (ctx, prelude, goal) = blow_up(3);
    for (budget, name) in [
        (
            Budget {
                eliminations: 1,
                ..Budget::default()
            },
            "eliminations",
        ),
        (
            Budget {
                bits: 1,
                ..Budget::default()
            },
            "bits",
        ),
    ] {
        let gave_up = prove(&ctx, Some(prelude), &goal, &budget).expect_err(name);
        assert!(
            matches!(gave_up.reason, Reason::Budget { name: hit, .. } if hit == name),
            "{gave_up}"
        );
    }
}

#[test]
fn the_certificate_stays_within_the_pairs_budget() {
    // Each of n hypotheses is needed: 0 <= x_i for each i, and the goal is
    // 0 <= x_1 + ... + x_n.
    let (mut ctx, prelude) = setup();
    let xs: Vec<Term> = (0..6)
        .map(|_| Term::var(ctx.declare(Type::Int).unwrap()))
        .collect();
    for x in &xs {
        ctx.assume(le(lit(0), x.clone())).unwrap();
    }
    let sum = xs.iter().cloned().reduce(add).unwrap();
    let goal = le(lit(0), sum);
    let proof = found(&ctx, prelude, &goal);
    assert_eq!(size(&proof).0, 6);
    let exact = Budget {
        pairs: 6,
        ..Budget::default()
    };
    prove(&ctx, Some(prelude), &goal, &exact).expect("six pairs are allowed");
    let tight = Budget {
        pairs: 5,
        ..Budget::default()
    };
    let gave_up = prove(&ctx, Some(prelude), &goal, &tight).expect_err("six pairs are needed");
    assert!(
        matches!(
            gave_up.reason,
            Reason::Budget {
                name: "pairs",
                limit: 5
            }
        ),
        "{gave_up}"
    );
}
