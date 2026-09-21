//! Tests for `Int` as a kernel type (build task K2; the kernel contract in
//! atlas.html): literals, the ring and order axioms, discreteness, induction
//! over the non-negative integers, and evaluation of closed terms.
//!
//! Each axiom is used once and misused at least once. A few facts are then
//! derived from the axioms alone, to show that the set is usable. Evaluation
//! is compared with `Integer` on random closed terms from a fixed seed; set
//! LOCUS_EXTENDED to run a hundred times as many. The last test reads the
//! kernel contract out of the atlas and fails when an axiom, a proof rule,
//! or a primitive is not named there.
//! Every term here is written by hand; nothing comes from the parser.

use std::rc::Rc;

use locus::kernel::derive::{Chain, symm_at};
use locus::kernel::{
    Axiom, Context, Definitions, HypRef, Integer, KernelError, MAX_DEPTH, MAX_EVAL_DEPTH, Mode,
    Prelude, Prim, Proof, Term, Type, check_proof, infer_proof, infer_term,
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

/// Modus ponens.
fn mp(implication: Proof, premise: Proof) -> Proof {
    Proof::implies_elim(implication, premise)
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

/// Three integer variables, a `Nat`, and a byte.
struct Vars {
    a: Term,
    b: Term,
    c: Term,
    nat: Term,
    byte: Term,
}

fn vars(ctx: &mut Context) -> Vars {
    let mut int = || Term::var(ctx.declare_ghost(Type::Int).unwrap());
    let (a, b, c) = (int(), int(), int());
    Vars {
        a,
        b,
        c,
        nat: Term::var(ctx.declare_ghost(Type::Nat).unwrap()),
        byte: Term::var(ctx.declare(Type::U8).unwrap()),
    }
}

/// The axiom proves exactly `statement`, and with any one argument replaced
/// by a `Nat` or a byte it proves nothing.
fn states(ctx: &mut Context, v: &Vars, build: &dyn Fn(&[Term]) -> Axiom, statement: Term) {
    let arguments = [v.a.clone(), v.b.clone(), v.c.clone()];
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

// --- The type, its literals, and its primitives ----------------------------------

#[test]
fn int_is_a_ghost_type_with_literals_of_any_size() {
    let (mut ctx, _) = setup();
    let huge: Integer = "-123456789012345678901234567890123456789012345678901234567890"
        .parse()
        .unwrap();
    assert_eq!(
        infer_term(&mut ctx, &Term::Int(huge.clone()), Mode::Logical),
        Ok(Type::Int)
    );
    // No runtime form, exactly as Nat has none.
    assert_eq!(
        infer_term(&mut ctx, &lit(3), Mode::Executable),
        Err(KernelError::GhostTypeInExecutable(Type::Int))
    );
    assert!(Type::Int.is_ghost());
    let n = ctx.declare(Type::Int).unwrap();
    assert_eq!(
        infer_term(&mut ctx, &Term::var(n), Mode::Executable),
        Err(KernelError::GhostInExecutable(n))
    );
    let n = Term::var(n);

    for term in [
        add(n.clone(), lit(1)),
        sub(n.clone(), lit(1)),
        mul(n.clone(), n.clone()),
        neg(n.clone()),
    ] {
        assert_eq!(infer_term(&mut ctx, &term, Mode::Logical), Ok(Type::Int));
        assert!(matches!(
            infer_term(&mut ctx, &term, Mode::Executable),
            Err(KernelError::GhostTypeInExecutable(_))
        ));
    }
    // The order is a proposition, and strict order is an abbreviation.
    assert_eq!(
        infer_term(&mut ctx, &le(n.clone(), lit(0)), Mode::Logical),
        Ok(Type::Prop)
    );
    assert_eq!(lt(n.clone(), lit(0)), le(add(n.clone(), lit(1)), lit(0)));

    // Int is not Nat and not u8: nothing converts between them yet.
    ill_typed(infer_term(
        &mut ctx,
        &add(n.clone(), Term::nat(1)),
        Mode::Logical,
    ));
    ill_typed(infer_term(
        &mut ctx,
        &le(n.clone(), Term::U8(1)),
        Mode::Logical,
    ));
    ill_typed(infer_term(
        &mut ctx,
        &Term::nat_add(Term::nat(1), lit(1)),
        Mode::Logical,
    ));
    ill_typed(infer_term(&mut ctx, &Term::of_nat(lit(1)), Mode::Logical));
    ill_typed(infer_term(
        &mut ctx,
        &Term::eq(Type::Nat, lit(1), lit(1)),
        Mode::Logical,
    ));
    assert!(matches!(
        infer_term(
            &mut ctx,
            &Term::prim(Prim::IntNeg, vec![n.clone(), n.clone()]),
            Mode::Logical
        ),
        Err(KernelError::WrongArity { .. })
    ));

    // Comparison of terms is comparison of numbers, with no computation.
    assert_eq!(
        check_proof(&mut ctx, &Proof::Refl(lit(5)), &eq(lit(5), lit(5))),
        Ok(())
    );
    mismatch(check_proof(
        &mut ctx,
        &Proof::Refl(lit(5)),
        &eq(lit(5), lit(6)),
    ));
    mismatch(check_proof(
        &mut ctx,
        &Proof::Refl(lit(5)),
        &eq(lit(5), lit(-5)),
    ));
    mismatch(check_proof(
        &mut ctx,
        &Proof::Refl(add(lit(2), lit(3))),
        &eq(add(lit(2), lit(3)), lit(5)),
    ));
    assert_eq!(
        Term::Int(huge).to_string(),
        format!(
            "{}i",
            "-123456789012345678901234567890123456789012345678901234567890"
        )
    );
}

#[test]
fn the_literal_axiom_computes_one_primitive_on_literals() {
    let (mut ctx, _) = setup();
    for (term, value) in [
        (add(lit(2), lit(-5)), -3),
        (sub(lit(2), lit(5)), -3),
        (mul(lit(-4), lit(-5)), 20),
        (neg(lit(7)), -7),
        (neg(lit(0)), 0),
    ] {
        assert_eq!(
            infer_proof(&mut ctx, &Proof::Literal(term.clone())),
            Ok(eq(term, lit(value)))
        );
    }
    // Only on literals, and the order has no value to compute.
    for stuck in [add(add(lit(1), lit(1)), lit(1)), le(lit(1), lit(2))] {
        assert!(matches!(
            infer_proof(&mut ctx, &Proof::Literal(stuck)),
            Err(KernelError::NoComputationStep(_))
        ));
    }
}

// --- The ring axioms ------------------------------------------------------------

#[test]
fn addition_is_associative_and_commutative_with_zero_and_negation() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let (a, b, c) = (v.a.clone(), v.b.clone(), v.c.clone());

    let assoc = eq(
        add(add(a.clone(), b.clone()), c.clone()),
        add(a.clone(), add(b.clone(), c.clone())),
    );
    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntAddAssoc(t[0].clone(), t[1].clone(), t[2].clone()),
        assoc,
    );
    let assoc = ax(Axiom::IntAddAssoc(a.clone(), b.clone(), c.clone()));
    // The other orientation, and the arguments in another order.
    mismatch(check_proof(
        &mut ctx,
        &assoc,
        &eq(
            add(a.clone(), add(b.clone(), c.clone())),
            add(add(a.clone(), b.clone()), c.clone()),
        ),
    ));
    mismatch(check_proof(
        &mut ctx,
        &assoc,
        &eq(
            add(add(a.clone(), c.clone()), b.clone()),
            add(a.clone(), add(b.clone(), c.clone())),
        ),
    ));
    // The same shape with another operation.
    mismatch(check_proof(
        &mut ctx,
        &assoc,
        &eq(
            sub(sub(a.clone(), b.clone()), c.clone()),
            sub(a.clone(), sub(b.clone(), c.clone())),
        ),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntAddComm(t[0].clone(), t[1].clone()),
        eq(add(a.clone(), b.clone()), add(b.clone(), a.clone())),
    );
    let comm = ax(Axiom::IntAddComm(a.clone(), b.clone()));
    mismatch(check_proof(
        &mut ctx,
        &comm,
        &eq(sub(a.clone(), b.clone()), sub(b.clone(), a.clone())),
    ));
    // The axiom at the wrong type is not even a proposition.
    ill_typed(check_proof(
        &mut ctx,
        &comm,
        &Term::eq(
            Type::Nat,
            add(a.clone(), b.clone()),
            add(b.clone(), a.clone()),
        ),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntAddZero(t[0].clone()),
        eq(add(a.clone(), lit(0)), a.clone()),
    );
    let zero = ax(Axiom::IntAddZero(a.clone()));
    mismatch(check_proof(
        &mut ctx,
        &zero,
        &eq(add(a.clone(), lit(1)), a.clone()),
    ));
    mismatch(check_proof(
        &mut ctx,
        &zero,
        &eq(add(lit(0), a.clone()), a.clone()),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntAddNeg(t[0].clone()),
        eq(add(a.clone(), neg(a.clone())), lit(0)),
    );
    let inverse = ax(Axiom::IntAddNeg(a.clone()));
    mismatch(check_proof(
        &mut ctx,
        &inverse,
        &eq(add(a.clone(), a.clone()), lit(0)),
    ));
    mismatch(check_proof(
        &mut ctx,
        &inverse,
        &eq(add(a.clone(), neg(a.clone())), lit(1)),
    ));
    mismatch(check_proof(
        &mut ctx,
        &inverse,
        &eq(add(a.clone(), neg(b.clone())), lit(0)),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntSubDef(t[0].clone(), t[1].clone()),
        eq(sub(a.clone(), b.clone()), add(a.clone(), neg(b.clone()))),
    );
    // Swapped, subtraction would be its own opposite.
    mismatch(check_proof(
        &mut ctx,
        &ax(Axiom::IntSubDef(a.clone(), b.clone())),
        &eq(sub(a.clone(), b.clone()), add(b.clone(), neg(a.clone()))),
    ));
    mismatch(check_proof(
        &mut ctx,
        &ax(Axiom::IntSubDef(a.clone(), b.clone())),
        &eq(sub(a.clone(), b.clone()), add(a, b)),
    ));
}

#[test]
fn multiplication_is_associative_and_commutative_with_one_and_distributes() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let (a, b, c) = (v.a.clone(), v.b.clone(), v.c.clone());

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntMulAssoc(t[0].clone(), t[1].clone(), t[2].clone()),
        eq(
            mul(mul(a.clone(), b.clone()), c.clone()),
            mul(a.clone(), mul(b.clone(), c.clone())),
        ),
    );
    mismatch(check_proof(
        &mut ctx,
        &ax(Axiom::IntMulAssoc(a.clone(), b.clone(), c.clone())),
        &eq(
            mul(mul(a.clone(), b.clone()), c.clone()),
            mul(b.clone(), mul(a.clone(), c.clone())),
        ),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntMulComm(t[0].clone(), t[1].clone()),
        eq(mul(a.clone(), b.clone()), mul(b.clone(), a.clone())),
    );
    mismatch(check_proof(
        &mut ctx,
        &ax(Axiom::IntMulComm(a.clone(), b.clone())),
        &eq(mul(a.clone(), b.clone()), mul(a.clone(), b.clone())),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntMulOne(t[0].clone()),
        eq(mul(a.clone(), lit(1)), a.clone()),
    );
    let one = ax(Axiom::IntMulOne(a.clone()));
    mismatch(check_proof(
        &mut ctx,
        &one,
        &eq(mul(a.clone(), lit(0)), a.clone()),
    ));
    mismatch(check_proof(
        &mut ctx,
        &one,
        &eq(mul(a.clone(), lit(-1)), a.clone()),
    ));
    mismatch(check_proof(
        &mut ctx,
        &one,
        &eq(mul(a.clone(), lit(1)), lit(1)),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntMulAdd(t[0].clone(), t[1].clone(), t[2].clone()),
        eq(
            mul(a.clone(), add(b.clone(), c.clone())),
            add(mul(a.clone(), b.clone()), mul(a.clone(), c.clone())),
        ),
    );
    let distributes = ax(Axiom::IntMulAdd(a.clone(), b.clone(), c.clone()));
    mismatch(check_proof(
        &mut ctx,
        &distributes,
        &eq(
            mul(a.clone(), add(b.clone(), c.clone())),
            add(mul(a.clone(), b.clone()), c.clone()),
        ),
    ));
    // Addition does not distribute over multiplication.
    mismatch(check_proof(
        &mut ctx,
        &distributes,
        &eq(
            add(a.clone(), mul(b.clone(), c.clone())),
            mul(add(a.clone(), b.clone()), add(a, c)),
        ),
    ));
}

// --- The order axioms -----------------------------------------------------------

#[test]
fn the_order_is_reflexive_transitive_and_antisymmetric() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let (a, b, c) = (v.a.clone(), v.b.clone(), v.c.clone());

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntLeRefl(t[0].clone()),
        le(a.clone(), a.clone()),
    );
    let refl = ax(Axiom::IntLeRefl(a.clone()));
    // Not the strict order, and not at two different terms.
    mismatch(check_proof(&mut ctx, &refl, &lt(a.clone(), a.clone())));
    mismatch(check_proof(&mut ctx, &refl, &le(a.clone(), b.clone())));
    mismatch(check_proof(
        &mut ctx,
        &ax(Axiom::IntLeRefl(add(lit(1), lit(1)))),
        &le(add(lit(1), lit(1)), lit(2)),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntLeTrans(t[0].clone(), t[1].clone(), t[2].clone()),
        implies(
            le(a.clone(), b.clone()),
            implies(le(b.clone(), c.clone()), le(a.clone(), c.clone())),
        ),
    );
    let trans = ax(Axiom::IntLeTrans(a.clone(), b.clone(), c.clone()));
    mismatch(check_proof(
        &mut ctx,
        &trans,
        &implies(
            le(a.clone(), b.clone()),
            implies(le(b.clone(), c.clone()), le(c.clone(), a.clone())),
        ),
    ));
    // The middle term must be the same on both sides.
    mismatch(check_proof(
        &mut ctx,
        &trans,
        &implies(
            le(a.clone(), b.clone()),
            implies(le(c.clone(), b.clone()), le(a.clone(), c.clone())),
        ),
    ));
    // The premises are owed: the conclusion alone is not what it proves.
    mismatch(check_proof(&mut ctx, &trans, &le(a.clone(), c.clone())));
    let ab = ctx.assume(le(a.clone(), b.clone())).unwrap();
    let cb = ctx.assume(le(c.clone(), b.clone())).unwrap();
    mismatch(infer_proof(
        &mut ctx,
        &mp(mp(trans, Proof::hyp(ab)), Proof::hyp(cb)),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntLeAntisymm(t[0].clone(), t[1].clone()),
        implies(
            le(a.clone(), b.clone()),
            implies(le(b.clone(), a.clone()), eq(a.clone(), b.clone())),
        ),
    );
    let antisymm = ax(Axiom::IntLeAntisymm(a.clone(), b.clone()));
    // One direction is not enough.
    mismatch(check_proof(
        &mut ctx,
        &antisymm,
        &implies(le(a.clone(), b.clone()), eq(a.clone(), b.clone())),
    ));
    mismatch(check_proof(
        &mut ctx,
        &antisymm,
        &implies(
            le(a.clone(), b.clone()),
            implies(le(a.clone(), b.clone()), eq(a.clone(), b.clone())),
        ),
    ));
    mismatch(infer_proof(
        &mut ctx,
        &mp(mp(antisymm, Proof::hyp(ab)), Proof::hyp(ab)),
    ));
}

#[test]
fn the_order_is_compatible_with_addition_and_multiplication() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let (a, b, c) = (v.a.clone(), v.b.clone(), v.c.clone());

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntLeAdd(t[0].clone(), t[1].clone(), t[2].clone()),
        implies(
            le(a.clone(), b.clone()),
            le(add(a.clone(), c.clone()), add(b.clone(), c.clone())),
        ),
    );
    let monotone = ax(Axiom::IntLeAdd(a.clone(), b.clone(), c.clone()));
    // The strict order does not follow from the weak one.
    mismatch(check_proof(
        &mut ctx,
        &monotone,
        &implies(
            le(a.clone(), b.clone()),
            lt(add(a.clone(), c.clone()), add(b.clone(), c.clone())),
        ),
    ));
    // Reversed, and with a different term added on each side.
    mismatch(check_proof(
        &mut ctx,
        &monotone,
        &implies(
            le(a.clone(), b.clone()),
            le(add(b.clone(), c.clone()), add(a.clone(), c.clone())),
        ),
    ));
    mismatch(check_proof(
        &mut ctx,
        &monotone,
        &implies(
            le(a.clone(), b.clone()),
            le(add(a.clone(), c.clone()), add(b.clone(), a.clone())),
        ),
    ));
    // Multiplication is not monotone: the sign of c matters.
    mismatch(check_proof(
        &mut ctx,
        &monotone,
        &implies(
            le(a.clone(), b.clone()),
            le(mul(a.clone(), c.clone()), mul(b.clone(), c.clone())),
        ),
    ));

    let zero = || lit(0);
    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntLeMul(t[0].clone(), t[1].clone()),
        implies(
            le(zero(), a.clone()),
            implies(le(zero(), b.clone()), le(zero(), mul(a.clone(), b.clone()))),
        ),
    );
    let product = ax(Axiom::IntLeMul(a.clone(), b.clone()));
    // Each hypothesis is needed.
    mismatch(check_proof(
        &mut ctx,
        &product,
        &implies(le(zero(), a.clone()), le(zero(), mul(a.clone(), b.clone()))),
    ));
    mismatch(check_proof(
        &mut ctx,
        &product,
        &implies(le(zero(), b.clone()), le(zero(), mul(a.clone(), b.clone()))),
    ));
    mismatch(check_proof(
        &mut ctx,
        &product,
        &le(zero(), mul(a.clone(), b.clone())),
    ));
    let a_only = ctx.assume(le(zero(), a.clone())).unwrap();
    mismatch(infer_proof(
        &mut ctx,
        &mp(mp(product.clone(), Proof::hyp(a_only)), Proof::hyp(a_only)),
    ));
    // A product of non-negatives may be zero.
    mismatch(check_proof(
        &mut ctx,
        &product,
        &implies(
            le(zero(), a.clone()),
            implies(le(zero(), b.clone()), lt(zero(), mul(a, b))),
        ),
    ));
}

#[test]
fn the_order_is_total_and_discrete_and_strict_order_is_irreflexive() {
    let (mut ctx, prelude) = setup();
    let v = vars(&mut ctx);
    let (a, b) = (v.a.clone(), v.b.clone());

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntLeTotal(t[0].clone(), t[1].clone()),
        prelude.or_prop(
            le(a.clone(), b.clone()),
            le(add(b.clone(), lit(1)), a.clone()),
        ),
    );
    let total = ax(Axiom::IntLeTotal(a.clone(), b.clone()));
    // True of the integers, and still not what the axiom says: nothing is
    // computed or rearranged when a claim is compared with a conclusion.
    for variant in [
        prelude.or_prop(
            le(a.clone(), b.clone()),
            le(b.clone(), sub(a.clone(), lit(1))),
        ),
        prelude.or_prop(
            le(a.clone(), b.clone()),
            le(add(lit(1), b.clone()), a.clone()),
        ),
        prelude.or_prop(le(a.clone(), b.clone()), le(b.clone(), a.clone())),
        prelude.or_prop(
            le(add(b.clone(), lit(1)), a.clone()),
            le(a.clone(), b.clone()),
        ),
    ] {
        mismatch(check_proof(&mut ctx, &total, &variant));
    }
    // False of the integers: both cases strict, and one case alone.
    mismatch(check_proof(
        &mut ctx,
        &total,
        &prelude.or_prop(lt(a.clone(), b.clone()), lt(b.clone(), a.clone())),
    ));
    mismatch(check_proof(&mut ctx, &total, &le(a.clone(), b.clone())));
    mismatch(check_proof(
        &mut ctx,
        &total,
        &prelude.and_prop(le(a.clone(), b.clone()), lt(b.clone(), a.clone())),
    ));

    states(
        &mut ctx,
        &v,
        &|t| Axiom::IntLtIrrefl(t[0].clone()),
        prelude.not_prop(le(add(a.clone(), lit(1)), a.clone())),
    );
    let irreflexive = ax(Axiom::IntLtIrrefl(a.clone()));
    // Without the negation it would be a contradiction.
    mismatch(check_proof(
        &mut ctx,
        &irreflexive,
        &le(add(a.clone(), lit(1)), a.clone()),
    ));
    // The weak order is reflexive, so its negation must not be provable.
    mismatch(check_proof(
        &mut ctx,
        &irreflexive,
        &prelude.not_prop(le(a.clone(), a.clone())),
    ));
    mismatch(check_proof(
        &mut ctx,
        &irreflexive,
        &prelude.not_prop(le(a.clone(), add(a.clone(), lit(1)))),
    ));

    // The axioms that mention False or Or need the prelude, as the others do.
    let mut bare = Context::new();
    let n = Term::var(bare.declare_ghost(Type::Int).unwrap());
    assert_eq!(
        infer_proof(&mut bare, &ax(Axiom::IntLtIrrefl(n))),
        Err(KernelError::NoPrelude)
    );
}

// --- Facts derived from the axioms alone -----------------------------------------

/// `n + 1 <= m => m + 1 <= n + 1 => False`: three axioms in a proof of
/// eleven nodes, counting every rule applied, hypotheses included.
fn nothing_between(n: &Term, m: &Term) -> Proof {
    let succ = |t: &Term| add(t.clone(), lit(1));
    Proof::implies_intro(le(succ(n), m.clone()), |above| {
        Proof::implies_intro(le(succ(m), succ(n)), |below| {
            // (n + 1) + 1 <= m + 1 <= n + 1
            let lifted = mp(ax(Axiom::IntLeAdd(succ(n), m.clone(), lit(1))), above);
            let chained = mp(
                mp(
                    ax(Axiom::IntLeTrans(succ(&succ(n)), succ(m), succ(n))),
                    lifted,
                ),
                below,
            );
            mp(ax(Axiom::IntLtIrrefl(succ(n))), chained)
        })
    })
}

/// `a <= a + 1`: from `0 <= 1`, which evaluation decides, by adding `a` on
/// both sides and tidying each side. Four axioms, eleven nodes.
fn le_succ(a: &Term) -> Proof {
    let zero_le_one = Proof::Evaluate(le(lit(0), lit(1)));
    // 0 + a <= 1 + a
    let shifted = mp(ax(Axiom::IntLeAdd(lit(0), lit(1), a.clone())), zero_le_one);
    let left = Chain::new(Type::Int, add(lit(0), a.clone()))
        .step(ax(Axiom::IntAddComm(lit(0), a.clone())))
        .step(ax(Axiom::IntAddZero(a.clone())))
        .finish();
    let right = ax(Axiom::IntAddComm(lit(1), a.clone()));
    let tidy_left = Proof::transport(left, |hole| le(hole, add(lit(1), a.clone())), shifted);
    Proof::transport(right, |hole| le(a.clone(), hole), tidy_left)
}

/// `(x + c) + (-c) == x`: three axioms, seven nodes.
fn add_then_subtract(x: &Term, c: &Term) -> Proof {
    Chain::new(Type::Int, add(add(x.clone(), c.clone()), neg(c.clone())))
        .step(ax(Axiom::IntAddAssoc(x.clone(), c.clone(), neg(c.clone()))))
        .rewrite(|hole| add(x.clone(), hole), ax(Axiom::IntAddNeg(c.clone())))
        .step(ax(Axiom::IntAddZero(x.clone())))
        .finish()
}

/// `a + c <= b + c => a <= b`: seven axioms, twenty nodes, of which eight are
/// transports: one for each rearrangement of a sum.
fn le_cancel(a: &Term, b: &Term, c: &Term) -> Proof {
    let (a_c, b_c) = (add(a.clone(), c.clone()), add(b.clone(), c.clone()));
    Proof::implies_intro(le(a_c.clone(), b_c.clone()), |h| {
        let shifted = mp(
            ax(Axiom::IntLeAdd(a_c.clone(), b_c.clone(), neg(c.clone()))),
            h,
        );
        let right = add(b_c.clone(), neg(c.clone()));
        let tidy_left = Proof::transport(add_then_subtract(a, c), |hole| le(hole, right), shifted);
        Proof::transport(
            add_then_subtract(b, c),
            |hole| le(a.clone(), hole),
            tidy_left,
        )
    })
}

#[test]
fn facts_about_the_order_follow_from_the_axioms_alone() {
    let (mut ctx, prelude) = setup();
    let v = vars(&mut ctx);
    let (a, b, c) = (v.a.clone(), v.b.clone(), v.c.clone());
    let succ = |t: &Term| add(t.clone(), lit(1));

    // x < y gives x + 1 <= y, because that is what x < y says.
    let unfold = Proof::implies_intro(lt(a.clone(), b.clone()), |h| h);
    assert_eq!(
        check_proof(
            &mut ctx,
            &unfold,
            &implies(lt(a.clone(), b.clone()), le(succ(&a), b.clone()))
        ),
        Ok(())
    );

    // Nothing lies strictly between n and n + 1.
    let falsehood = prelude.falsehood_prop();
    assert_eq!(
        check_proof(
            &mut ctx,
            &nothing_between(&a, &b),
            &implies(
                lt(a.clone(), b.clone()),
                implies(lt(b.clone(), succ(&a)), falsehood.clone())
            )
        ),
        Ok(())
    );
    // With the weak order on one side there is something: n + 1 itself.
    mismatch(check_proof(
        &mut ctx,
        &nothing_between(&a, &b),
        &implies(
            lt(a.clone(), b.clone()),
            implies(le(b.clone(), succ(&a)), falsehood),
        ),
    ));

    // a <= a + 1.
    assert_eq!(
        check_proof(&mut ctx, &le_succ(&a), &le(a.clone(), succ(&a))),
        Ok(())
    );
    mismatch(check_proof(
        &mut ctx,
        &le_succ(&a),
        &le(succ(&a), a.clone()),
    ));

    // a <= b gives a < b + 1: one instance of compatibility with addition.
    let weaken = Proof::implies_intro(le(a.clone(), b.clone()), |h| {
        mp(ax(Axiom::IntLeAdd(a.clone(), b.clone(), lit(1))), h)
    });
    assert_eq!(
        check_proof(
            &mut ctx,
            &weaken,
            &implies(le(a.clone(), b.clone()), lt(a.clone(), succ(&b)))
        ),
        Ok(())
    );
    mismatch(check_proof(
        &mut ctx,
        &weaken,
        &implies(le(a.clone(), b.clone()), lt(a.clone(), b.clone())),
    ));

    // a <= b and b < c give a < c.
    let mixed = Proof::implies_intro(le(a.clone(), b.clone()), |ab| {
        Proof::implies_intro(lt(b.clone(), c.clone()), |bc| {
            let lifted = mp(ax(Axiom::IntLeAdd(a.clone(), b.clone(), lit(1))), ab);
            mp(
                mp(ax(Axiom::IntLeTrans(succ(&a), succ(&b), c.clone())), lifted),
                bc,
            )
        })
    });
    assert_eq!(
        check_proof(
            &mut ctx,
            &mixed,
            &implies(
                le(a.clone(), b.clone()),
                implies(lt(b.clone(), c.clone()), lt(a.clone(), c.clone()))
            )
        ),
        Ok(())
    );

    // Addition cancels in the order, which needs the ring laws.
    assert_eq!(
        check_proof(
            &mut ctx,
            &le_cancel(&a, &b, &c),
            &implies(
                le(add(a.clone(), c.clone()), add(b.clone(), c.clone())),
                le(a.clone(), b.clone())
            )
        ),
        Ok(())
    );

    // Two literals that differ are different numbers: evaluation refutes
    // the equation, and the refutation is an ordinary fact.
    assert_eq!(
        check_proof(
            &mut ctx,
            &Proof::Evaluate(eq(lit(0), lit(1))),
            &prelude.not_prop(eq(lit(0), lit(1)))
        ),
        Ok(())
    );
}

// --- Induction -------------------------------------------------------------------

#[test]
fn induction_over_the_non_negative_integers_checks_its_base_and_its_step() {
    let (mut ctx, _) = setup();
    let v = vars(&mut ctx);
    let (a, n) = (v.a.clone(), v.b.clone());
    let nonneg = |t: &Term| le(lit(0), t.clone());

    // 0 <= n => a <= a + n.
    let claim = |k: Term| le(a.clone(), add(a.clone(), k));
    let base = || {
        Proof::transport(
            symm_at(
                &Type::Int,
                &add(a.clone(), lit(0)),
                ax(Axiom::IntAddZero(a.clone())),
            ),
            |hole| le(a.clone(), hole),
            ax(Axiom::IntLeRefl(a.clone())),
        )
    };
    let step = |k: Term, _: Proof, ih: Proof| {
        // a <= a + k <= (a + k) + 1 == a + (k + 1)
        let sum = add(a.clone(), k.clone());
        let longer = mp(
            mp(
                ax(Axiom::IntLeTrans(
                    a.clone(),
                    sum.clone(),
                    add(sum.clone(), lit(1)),
                )),
                ih,
            ),
            le_succ(&sum),
        );
        Proof::transport(
            ax(Axiom::IntAddAssoc(a.clone(), k, lit(1))),
            |hole| le(a.clone(), hole),
            longer,
        )
    };
    let induction = Proof::int_induction(claim, base(), step, n.clone());
    assert_eq!(
        infer_proof(&mut ctx, &induction),
        Ok(implies(nonneg(&n), claim(n.clone())))
    );
    // The conclusion is about the non-negative integers only: the claim
    // itself is false at n == -1, and is not what the rule proves.
    mismatch(check_proof(&mut ctx, &induction, &claim(n.clone())));
    // Generalized, it is the statement for every n.
    let for_all = Proof::forall_intro(Type::Int, |m| Proof::int_induction(claim, base(), step, m));
    assert_eq!(
        check_proof(
            &mut ctx,
            &for_all,
            &Term::forall(Type::Int, |m| implies(nonneg(&m), claim(m)))
        ),
        Ok(())
    );

    // A base case at 1 and not at 0.
    let base_at_one = Proof::int_induction(claim, le_succ(&a), step, n.clone());
    mismatch(infer_proof(&mut ctx, &base_at_one));
    // A step that only restates its hypothesis proves the claim about k,
    // not about k + 1.
    let lazy_step = Proof::int_induction(claim, base(), |_, _, ih| ih, n.clone());
    mismatch(infer_proof(&mut ctx, &lazy_step));
    // The step's first hypothesis is 0 <= k, not the claim about k.
    let crossed = Proof::int_induction(
        claim,
        base(),
        |k, nonneg, ih| step(k, ih, nonneg),
        n.clone(),
    );
    mismatch(infer_proof(&mut ctx, &crossed));
    // The step must reach k + 1 written exactly so, not 1 + k.
    let one_plus = Proof::int_induction(
        claim,
        base(),
        |k, _, ih| {
            let sum = add(a.clone(), k.clone());
            let longer = mp(
                mp(
                    ax(Axiom::IntLeTrans(
                        a.clone(),
                        sum.clone(),
                        add(sum.clone(), lit(1)),
                    )),
                    ih,
                ),
                le_succ(&sum),
            );
            let regrouped = Proof::transport(
                ax(Axiom::IntAddAssoc(a.clone(), k.clone(), lit(1))),
                |hole| le(a.clone(), hole),
                longer,
            );
            Proof::transport(
                ax(Axiom::IntAddComm(k.clone(), lit(1))),
                |hole| le(a.clone(), add(a.clone(), hole)),
                regrouped,
            )
        },
        n.clone(),
    );
    mismatch(infer_proof(&mut ctx, &one_plus));
    // The step binds one variable and two hypotheses, unlike Nat's.
    let Proof::NatInduction { step: nat_arm, .. } =
        Proof::nat_induction(|k| k, Proof::Omitted, |_, ih| ih, Term::nat(0))
    else {
        unreachable!()
    };
    let Proof::IntInduction { motive, .. } = induction.clone() else {
        unreachable!()
    };
    let short_arm = Proof::IntInduction {
        motive,
        base: Box::new(base()),
        step: nat_arm,
        target: n.clone(),
    };
    assert!(matches!(
        infer_proof(&mut ctx, &short_arm),
        Err(KernelError::ArmBinders { .. })
    ));
    // The motive must be a proposition about an Int, and the target an Int.
    let data_motive = Proof::int_induction(|k| k, base(), |_, _, ih| ih, n.clone());
    ill_typed(infer_proof(&mut ctx, &data_motive));
    let nat_motive = Proof::int_induction(
        |k| Term::eq(Type::Nat, k.clone(), k),
        Proof::Refl(Term::nat(0)),
        |_, _, ih| ih,
        n.clone(),
    );
    ill_typed(infer_proof(&mut ctx, &nat_motive));
    for target in [v.nat.clone(), v.byte.clone(), Term::nat(3)] {
        ill_typed(infer_proof(
            &mut ctx,
            &Proof::int_induction(claim, base(), step, target),
        ));
    }
    // The induction variable does not outlive the step.
    let leaked = std::cell::RefCell::new(None);
    let _ = Proof::int_induction(
        claim,
        base(),
        |k, nonneg, ih| {
            *leaked.borrow_mut() = Some((k, nonneg));
            ih
        },
        n.clone(),
    );
    let (k, k_nonneg) = leaked.into_inner().unwrap();
    assert!(matches!(
        infer_term(&mut ctx, &k, Mode::Logical),
        Err(KernelError::UnknownVariable(_))
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &k_nonneg),
        Err(KernelError::UnknownHypothesis(_))
    ));
}

// --- Evaluation ------------------------------------------------------------------

#[test]
fn evaluation_computes_closed_integer_terms_and_decides_comparisons() {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let double = definitions
        .declare_fn(&Type::function(1, |_| Type::Int), |params| {
            add(params[0].clone(), params[0].clone())
        })
        .unwrap();
    assert!(!definitions.is_executable(double));
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let n = Term::var(ctx.declare_ghost(Type::Int).unwrap());

    // Through a call, a product, and a case, as for any other data.
    let call = Term::call(Term::Fn(double), vec![sub(lit(1), lit(22))]);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(call.clone())),
        Ok(eq(call.clone(), lit(-42)))
    );
    let pair_type = Type::Tuple(vec![Type::Int, Type::Bool]);
    let pair = Term::tuple(&pair_type, vec![mul(lit(-6), lit(7)), Term::Bool(true)]);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(pair.clone())),
        Ok(Term::eq(
            pair_type.clone(),
            pair.clone(),
            Term::tuple(&pair_type, vec![lit(-42), Term::Bool(true)])
        ))
    );
    let chosen = Term::case(
        Term::proj(pair, 1),
        Type::Int,
        vec![
            (0, Box::new(|_, _| lit(0))),
            (0, Box::new(|_, _| neg(neg(lit(9))))),
        ],
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(chosen.clone())),
        Ok(eq(chosen, lit(9)))
    );
    // A false claim is simply not what evaluation proves.
    mismatch(check_proof(
        &mut ctx,
        &Proof::Evaluate(call.clone()),
        &eq(call.clone(), lit(-41)),
    ));

    // A comparison is decided: proved when it holds, refuted when not.
    let cases = [
        (le(lit(-3), lit(2)), true),
        (le(lit(2), lit(2)), true),
        (le(lit(2), lit(-3)), false),
        (le(lit(-2), lit(-3)), false),
        (lt(lit(2), lit(3)), true),
        (lt(lit(2), lit(2)), false),
        (le(call.clone(), mul(lit(-6), lit(7))), true),
        (lt(call.clone(), mul(lit(-6), lit(7))), false),
        (eq(call.clone(), mul(lit(-6), lit(7))), true),
        (eq(call.clone(), lit(42)), false),
    ];
    for (claim, holds) in cases {
        let decided = Proof::Evaluate(claim.clone());
        let negation = prelude.not_prop(claim.clone());
        let (proved, other) = if holds {
            (claim, negation)
        } else {
            (negation, claim)
        };
        assert_eq!(infer_proof(&mut ctx, &decided), Ok(proved));
        mismatch(check_proof(&mut ctx, &decided, &other));
    }

    // Only closed terms, only comparisons of integers.
    for open in [
        add(n.clone(), lit(1)),
        le(n.clone(), n.clone()),
        eq(n.clone(), n.clone()),
    ] {
        assert!(matches!(
            infer_proof(&mut ctx, &Proof::Evaluate(open)),
            Err(KernelError::NotClosed(_))
        ));
    }
    for not_offered in [
        Term::eq(Type::U8, Term::U8(1), Term::U8(1)),
        Term::eq(Type::Nat, Term::nat(1), Term::nat(1)),
        implies(le(lit(0), lit(1)), le(lit(0), lit(1))),
    ] {
        assert_eq!(
            infer_proof(&mut ctx, &Proof::Evaluate(not_offered)),
            Err(KernelError::NotPlainData(Type::Prop))
        );
    }
    ill_typed(infer_proof(
        &mut ctx,
        &Proof::Evaluate(le(lit(0), Term::nat(1))),
    ));
    // A proposition stored in data is opaque, the order included.
    let stored_type = Type::Tuple(vec![Type::Prop, Type::Int]);
    let stored = Term::proj(
        Term::tuple(&stored_type, vec![le(lit(5), lit(1)), add(lit(1), lit(1))]),
        1,
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(stored.clone())),
        Ok(eq(stored, lit(2)))
    );

    // Exhaustion is over a byte and never over Int.
    ill_typed(infer_proof(
        &mut ctx,
        &Proof::evaluate_all(|x| le(x, lit(255))),
    ));

    // A refutation is stated with False, which the prelude declares.
    let mut bare = Context::new();
    assert_eq!(
        infer_proof(&mut bare, &Proof::Evaluate(le(lit(1), lit(2)))),
        Ok(le(lit(1), lit(2)))
    );
    assert_eq!(
        infer_proof(&mut bare, &Proof::Evaluate(le(lit(2), lit(1)))),
        Err(KernelError::NoPrelude)
    );
}

// --- Evaluation against Integer, on random closed terms -----------------------------

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

/// A closed term and its value, computed beside it with `Integer`.
fn random_term(rng: &mut Rng, depth: usize) -> (Term, Integer) {
    if depth == 0 || rng.chance(1, 5) {
        let value = random_integer(rng);
        return (Term::Int(value.clone()), value);
    }
    let (left, l) = random_term(rng, depth - 1);
    if rng.chance(1, 6) {
        return (neg(left), l.neg());
    }
    let (right, r) = random_term(rng, depth - 1);
    match rng.below(3) {
        0 => (add(left, right), l.add(&r)),
        1 => (sub(left, right), l.sub(&r)),
        _ => (mul(left, right), l.mul(&r)),
    }
}

#[test]
fn evaluation_agrees_with_integer_on_random_closed_terms() {
    let (mut ctx, prelude) = setup();
    let one = Integer::from(1i64);
    let mut larger_than_a_word = 0;
    for index in 0..300 * scale() {
        let seed = case_seed(0x4B32_494E, index);
        let mut rng = Rng::new(seed);
        let depth = rng.range(0..7);
        let (term, value) = random_term(&mut rng, depth);
        let (other, other_value) = random_term(&mut rng, depth.min(3));
        if value.magnitude().bit_length() > 128 {
            larger_than_a_word += 1;
        }

        // The value, and no other.
        let evaluated = Proof::Evaluate(term.clone());
        assert_eq!(
            infer_proof(&mut ctx, &evaluated),
            Ok(eq(term.clone(), Term::Int(value.clone()))),
            "seed {seed:#x}"
        );
        let next = Term::Int(value.add(&one));
        mismatch(check_proof(
            &mut ctx,
            &evaluated,
            &eq(term.clone(), next.clone()),
        ));

        // Every comparison, against what the difference of the values says.
        // The neighbors of the value make the boundary cases certain.
        let mut decide = |claim: Term, holds: bool| {
            let negation = prelude.not_prop(claim.clone());
            let (proved, refused) = if holds {
                (claim.clone(), negation)
            } else {
                (negation, claim.clone())
            };
            let decided = Proof::Evaluate(claim);
            assert_eq!(
                infer_proof(&mut ctx, &decided),
                Ok(proved),
                "seed {seed:#x}"
            );
            mismatch(check_proof(&mut ctx, &decided, &refused));
        };
        let same = Term::Int(value.clone());
        let previous = Term::Int(value.sub(&one));
        decide(eq(term.clone(), same.clone()), true);
        decide(eq(term.clone(), next.clone()), false);
        decide(le(term.clone(), same.clone()), true);
        decide(le(same.clone(), term.clone()), true);
        decide(le(term.clone(), previous.clone()), false);
        decide(le(next.clone(), term.clone()), false);
        decide(lt(term.clone(), next), true);
        decide(lt(previous, term.clone()), true);
        decide(lt(term.clone(), same), false);
        let difference = other_value.sub(&value);
        decide(le(term.clone(), other.clone()), !difference.is_negative());
        decide(
            le(other.clone(), term.clone()),
            difference.is_negative() || difference.is_zero(),
        );
        decide(
            lt(term.clone(), other.clone()),
            !difference.is_negative() && !difference.is_zero(),
        );
        decide(eq(other, term), difference.is_zero());
    }
    // The generator does reach past the machine sizes.
    assert!(larger_than_a_word > 50, "{larger_than_a_word}");
}

// --- Limits ----------------------------------------------------------------------

fn nested_sum(depth: usize) -> Term {
    (0..depth).fold(lit(0), |term, _| add(term, lit(1)))
}

#[test]
fn integer_terms_are_held_to_the_depth_and_step_limits() {
    let (mut definitions, _) = Definitions::with_prelude();
    let square = definitions
        .declare_fn(&Type::function(1, |_| Type::Int), |params| {
            mul(params[0].clone(), params[0].clone())
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    // Input depth, as for every other term.
    assert_eq!(
        infer_term(&mut ctx, &nested_sum(MAX_DEPTH - 8), Mode::Logical),
        Ok(Type::Int)
    );
    assert_eq!(
        infer_term(&mut ctx, &nested_sum(MAX_DEPTH + 1), Mode::Logical),
        Err(KernelError::TooDeep)
    );
    assert_eq!(
        infer_proof(&mut ctx, &ax(Axiom::IntLeRefl(nested_sum(MAX_DEPTH + 1)))),
        Err(KernelError::TooDeep)
    );
    let deep_induction = Proof::int_induction(
        |k| le(k.clone(), k),
        ax(Axiom::IntLeRefl(lit(0))),
        |_, _, ih| ih,
        nested_sum(MAX_DEPTH + 1),
    );
    assert_eq!(
        infer_proof(&mut ctx, &deep_induction),
        Err(KernelError::TooDeep)
    );

    // Evaluation depth.
    let fits = nested_sum(MAX_EVAL_DEPTH - 8);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(fits.clone())),
        Ok(eq(fits, lit(MAX_EVAL_DEPTH as i64 - 8)))
    );
    let too_deep = nested_sum(MAX_EVAL_DEPTH + 8);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(too_deep.clone())),
        Err(KernelError::EvaluationTooDeep)
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(le(lit(0), too_deep))),
        Err(KernelError::EvaluationTooDeep)
    );

    // Repeated squaring doubles the size of a number at every call, so a
    // term of a hundred nodes names a number of 2^100 bits. Multiplication
    // is charged by the size of its operands, so the step budget ends this
    // long before memory does. 2^(2^12) is within the budget.
    let squared =
        |times: usize| (0..times).fold(lit(2), |term, _| Term::call(Term::Fn(square), vec![term]));
    let Ok(Term::Eq(_, _, value)) = infer_proof(&mut ctx, &Proof::Evaluate(squared(12))) else {
        panic!("2^(2^12) is within the step budget")
    };
    let Term::Int(value) = *value else {
        panic!("an Int evaluates to a literal")
    };
    assert_eq!(value.magnitude().bit_length(), (1 << 12) + 1);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(squared(100))),
        Err(KernelError::StepLimit)
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(le(squared(100), lit(0)))),
        Err(KernelError::StepLimit)
    );
}

// --- The contract names everything -------------------------------------------------

/// One of every axiom. `axiom_index` has no wildcard arm, so a new axiom
/// does not compile until it is given an index, and the test below fails
/// until it is given a sample here.
fn every_axiom() -> Vec<Axiom> {
    let t = || lit(0);
    vec![
        Axiom::NatAddZero(t()),
        Axiom::NatAddSucc(t(), t()),
        Axiom::NatSuccInjective(t(), t()),
        Axiom::NatSuccNotZero(t()),
        Axiom::ToNatBound(t()),
        Axiom::OfToNat(t()),
        Axiom::ToOfNat(t()),
        Axiom::OfNatWrap(t()),
        Axiom::WrappingAddModel(t(), t()),
        Axiom::WrappingSubModel(t(), t()),
        Axiom::Reflect(t(), true),
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
    ]
}

const AXIOMS: usize = 35;

fn axiom_index(axiom: &Axiom) -> usize {
    match axiom {
        Axiom::NatAddZero(_) => 0,
        Axiom::NatAddSucc(..) => 1,
        Axiom::NatSuccInjective(..) => 2,
        Axiom::NatSuccNotZero(_) => 3,
        Axiom::ToNatBound(_) => 4,
        Axiom::OfToNat(_) => 5,
        Axiom::ToOfNat(_) => 6,
        Axiom::OfNatWrap(_) => 7,
        Axiom::WrappingAddModel(..) => 8,
        Axiom::WrappingSubModel(..) => 9,
        Axiom::Reflect(..) => 10,
        Axiom::IntAddAssoc(..) => 11,
        Axiom::IntAddComm(..) => 12,
        Axiom::IntAddZero(_) => 13,
        Axiom::IntAddNeg(_) => 14,
        Axiom::IntSubDef(..) => 15,
        Axiom::IntMulAssoc(..) => 16,
        Axiom::IntMulComm(..) => 17,
        Axiom::IntMulOne(_) => 18,
        Axiom::IntMulAdd(..) => 19,
        Axiom::IntLeRefl(_) => 20,
        Axiom::IntLeTrans(..) => 21,
        Axiom::IntLeAntisymm(..) => 22,
        Axiom::IntLeAdd(..) => 23,
        Axiom::IntLeMul(..) => 24,
        Axiom::IntLeTotal(..) => 25,
        Axiom::IntLtIrrefl(_) => 26,
        Axiom::IntDivRem(..) => 27,
        Axiom::IntDivZero(_) => 28,
        Axiom::IntRemLowerPos(..) => 29,
        Axiom::IntRemUpperPos(..) => 30,
        Axiom::IntRemLowerNeg(..) => 31,
        Axiom::IntRemUpperNeg(..) => 32,
        Axiom::IntRemNonneg(..) => 33,
        Axiom::IntRemNonpos(..) => 34,
    }
}

/// One of every proof rule, under the same discipline.
fn every_rule() -> Vec<Proof> {
    let t = || lit(0);
    let p = || Box::new(Proof::Omitted);
    let arm = || Proof::arm(0, 0, |_, _| Proof::Omitted);
    vec![
        Proof::Hyp(HypRef::Bound(0)),
        Proof::OfTerm(t()),
        Proof::Refl(t()),
        Proof::transport(Proof::Omitted, |hole| hole, Proof::Omitted),
        Proof::implies_intro(t(), |h| h),
        Proof::ImpliesElim(p(), p()),
        Proof::forall_intro(Type::Int, |_| Proof::Omitted),
        Proof::ForallElim(p(), t()),
        Proof::Projection(t()),
        Proof::Literal(t()),
        Proof::Definition(t()),
        Proof::CaseStep(t()),
        Proof::CaseProof {
            scrutinee: p(),
            goal: t(),
            arms: vec![],
        },
        Proof::CaseData {
            scrutinee: t(),
            goal: t(),
            arms: vec![],
        },
        Proof::ExistsIntro {
            prop: t(),
            witness: t(),
            proof: p(),
        },
        Proof::ExistsElim {
            exists: p(),
            goal: t(),
            arm: arm(),
        },
        Proof::ExcludedMiddle(t()),
        Proof::ForEmpty(t()),
        Proof::ForStep {
            looped: t(),
            lower: p(),
            upper: p(),
        },
        Proof::Omitted,
        Proof::Evaluate(t()),
        Proof::EvaluateAll(t()),
        Proof::Axiom(Axiom::IntLeRefl(t())),
        Proof::nat_induction(|k| k, Proof::Omitted, |_, ih| ih, t()),
        Proof::int_induction(|k| k, Proof::Omitted, |_, _, ih| ih, t()),
    ]
}

/// `Construct` names a declared proposition, whose identity only a
/// declaration gives out.
fn construct_rule() -> Proof {
    let (_, prelude) = Definitions::with_prelude();
    Proof::Construct {
        prop: prelude.truth,
        variant: 0,
        params: vec![],
        payload: vec![],
    }
}

const RULES: usize = 26;

fn rule_index(proof: &Proof) -> usize {
    match proof {
        Proof::Hyp(_) => 0,
        Proof::OfTerm(_) => 1,
        Proof::Refl(_) => 2,
        Proof::Transport { .. } => 3,
        Proof::ImpliesIntro { .. } => 4,
        Proof::ImpliesElim(..) => 5,
        Proof::ForallIntro { .. } => 6,
        Proof::ForallElim(..) => 7,
        Proof::Projection(_) => 8,
        Proof::Literal(_) => 9,
        Proof::Definition(_) => 10,
        Proof::CaseStep(_) => 11,
        Proof::Construct { .. } => 12,
        Proof::CaseProof { .. } => 13,
        Proof::CaseData { .. } => 14,
        Proof::ExistsIntro { .. } => 15,
        Proof::ExistsElim { .. } => 16,
        Proof::ExcludedMiddle(_) => 17,
        Proof::ForEmpty(_) => 18,
        Proof::ForStep { .. } => 19,
        Proof::Omitted => 20,
        Proof::Evaluate(_) => 21,
        Proof::EvaluateAll(_) => 22,
        Proof::Axiom(_) => 23,
        Proof::NatInduction { .. } => 24,
        Proof::IntInduction { .. } => 25,
    }
}

const PRIMS: [Prim; 16] = [
    Prim::WrappingAdd,
    Prim::WrappingSub,
    Prim::U8Eq,
    Prim::U8Lt,
    Prim::U8Le,
    Prim::ToNat,
    Prim::OfNat,
    Prim::Succ,
    Prim::NatAdd,
    Prim::IntAdd,
    Prim::IntSub,
    Prim::IntMul,
    Prim::IntNeg,
    Prim::IntLe,
    Prim::IntDiv,
    Prim::IntRem,
];

fn prim_index(prim: Prim) -> usize {
    match prim {
        Prim::WrappingAdd => 0,
        Prim::WrappingSub => 1,
        Prim::U8Eq => 2,
        Prim::U8Lt => 3,
        Prim::U8Le => 4,
        Prim::ToNat => 5,
        Prim::OfNat => 6,
        Prim::Succ => 7,
        Prim::NatAdd => 8,
        Prim::IntAdd => 9,
        Prim::IntSub => 10,
        Prim::IntMul => 11,
        Prim::IntNeg => 12,
        Prim::IntLe => 13,
        Prim::IntDiv => 14,
        Prim::IntRem => 15,
    }
}

/// The names the kernel gives its axioms, rules, and primitives, after
/// checking that the samples above leave none out.
fn kernel_names() -> Vec<&'static str> {
    let axioms = every_axiom();
    let mut seen: Vec<usize> = axioms.iter().map(axiom_index).collect();
    seen.sort_unstable();
    assert_eq!(
        seen,
        (0..AXIOMS).collect::<Vec<_>>(),
        "a sample of every axiom"
    );

    let mut rules = every_rule();
    rules.push(construct_rule());
    let mut seen: Vec<usize> = rules.iter().map(rule_index).collect();
    seen.sort_unstable();
    assert_eq!(
        seen,
        (0..RULES).collect::<Vec<_>>(),
        "a sample of every rule"
    );

    let mut seen: Vec<usize> = PRIMS.iter().map(|prim| prim_index(*prim)).collect();
    seen.sort_unstable();
    assert_eq!(
        seen,
        (0..PRIMS.len()).collect::<Vec<_>>(),
        "every primitive"
    );

    let mut names: Vec<&'static str> = axioms.iter().map(Axiom::name).collect();
    names.extend(rules.iter().map(Proof::rule_name));
    names.extend(PRIMS.iter().map(|prim| prim.name()));
    let mut distinct = names.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), names.len(), "names are not shared");
    names
}

/// The text of one JSON string whose opening quote is at `start`, and the
/// position after its closing quote. The build has no JSON dependency, and
/// strings are the only part of the format with any subtlety.
fn json_string(text: &[char], start: usize) -> (String, usize) {
    assert_eq!(text[start], '"');
    let mut out = String::new();
    let mut at = start + 1;
    let hex = |at: usize| -> u32 {
        let digits: String = text[at..at + 4].iter().collect();
        u32::from_str_radix(&digits, 16).expect("four hex digits after \\u")
    };
    loop {
        let c = text[at];
        at += 1;
        match c {
            '"' => return (out, at),
            '\\' => {
                let escape = text[at];
                at += 1;
                match escape {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'u' => {
                        let mut code = hex(at);
                        at += 4;
                        // A surrogate pair is one character.
                        if (0xD800..0xDC00).contains(&code)
                            && text[at] == '\\'
                            && text[at + 1] == 'u'
                        {
                            let low = hex(at + 2);
                            at += 6;
                            code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                        }
                        out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                    }
                    other => out.push(other),
                }
            }
            other => out.push(other),
        }
    }
}

/// The body of the atlas document with the given id, one line per entry.
///
/// The atlas holds one JSON block. This reads it as a stream of tokens, of
/// which only strings need care, and looks for the key `"id"` with the
/// wanted value, then the next key `"body"`, then the strings up to the
/// bracket that closes the array. A body is an array of strings and nothing
/// else, so no nesting has to be followed, and because a string is always
/// consumed whole, nothing written inside a document can be mistaken for
/// structure.
fn atlas_document(html: &str, id: &str) -> Vec<String> {
    let open = "<script type=\"application/json\" id=\"atlas-data\">";
    let start = html.find(open).expect("the atlas data block") + open.len();
    let end = start + html[start..].find("</script>").expect("the block ends");
    let text: Vec<char> = html[start..end].chars().collect();

    // Tokens: a string, or any other character that is not white space.
    let mut tokens: Vec<Result<String, char>> = Vec::new();
    let mut at = 0;
    while at < text.len() {
        if text[at] == '"' {
            let (string, next) = json_string(&text, at);
            tokens.push(Ok(string));
            at = next;
        } else {
            if !text[at].is_whitespace() {
                tokens.push(Err(text[at]));
            }
            at += 1;
        }
    }
    let is_key = |index: usize, name: &str| {
        tokens[index].as_deref() == Ok(name) && tokens.get(index + 1) == Some(&Err(':'))
    };
    let found = (0..tokens.len())
        .find(|&index| {
            is_key(index, "id") && tokens.get(index + 2).map(|t| t.as_deref()) == Some(Ok(id))
        })
        .unwrap_or_else(|| panic!("no document {id} in the atlas"));
    let body = (found..tokens.len())
        .find(|&index| is_key(index, "body"))
        .expect("the document has a body");
    assert_eq!(tokens[body + 2], Err('['));
    let mut lines = Vec::new();
    for token in &tokens[body + 3..] {
        match token {
            Ok(line) => lines.push(line.clone()),
            Err(',') => {}
            Err(']') => return lines,
            Err(other) => panic!("a body is an array of strings, found {other}"),
        }
    }
    panic!("the body does not end")
}

/// Whether `name` occurs in `text` as a whole identifier.
fn mentions(text: &str, name: &str) -> bool {
    let part = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(name).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(part)
            && !text[at + name.len()..].chars().next().is_some_and(part)
    })
}

#[test]
fn the_contract_names_every_axiom_rule_and_primitive() {
    let path = std::env::var_os("LOCUS_ATLAS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("atlas.html"));
    let html = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    let contract = atlas_document(&html, "kernel-contract").join("\n");
    assert!(contract.starts_with("# Kernel contract"));

    let missing: Vec<&str> = kernel_names()
        .into_iter()
        .filter(|name| !mentions(&contract, name))
        .collect();
    assert!(
        missing.is_empty(),
        "the kernel contract in {} does not name: {}",
        path.display(),
        missing.join(", ")
    );
}

#[test]
fn the_contract_reader_handles_escapes_and_whole_words() {
    let html = concat!(
        "<script type=\"application/json\" id=\"atlas-data\">\n",
        "{\"docs\": [{\"id\": \"other\", \"body\": [\"\\\"id\\\": \\\"wanted\\\"\"]},\n",
        " {\"id\": \"wanted\", \"title\": \"body\", \"body\": [\"a \\u003c b\", \"tab\\there ]\", ",
        "\"\\ud83d\\ude00 \\\\ \\\"q\\\"\"]}]}\n</script>\n",
        "const FILE = '<script type=\"application/json\" id=\"atlas-data\">';"
    );
    assert_eq!(
        atlas_document(html, "wanted"),
        vec!["a < b", "tab\there ]", "\u{1F600} \\ \"q\""]
    );
    assert!(mentions("uses `evaluate_all(t)` here", "evaluate_all"));
    assert!(!mentions("uses `evaluate_all(t)` here", "evaluate"));
    assert!(!mentions("the nat_add_zero axiom", "add_zero"));
    assert!(mentions("int_le", "int_le"));
}
