//! Acceptance tests for kernel gate K5 (docs/kernel-contract.md): the internal
//! `Nat` with induction, the `u8` model, reflection of runtime comparisons,
//! and agreement between native evaluation and the model.
//! Every term here is written by hand; nothing comes from the parser.

use std::rc::Rc;

use locus::kernel::derive::{Chain, fold_claim, symm_at};
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, Context, Definitions, KernelError, Mode, Prelude, Prim, Proof, Term, Type, check_proof,
    infer_proof, infer_term, proof_is_classical,
};

fn setup() -> (Rc<Definitions>, Prelude, Theory) {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    (Rc::new(definitions), prelude, theory)
}

fn nat_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::Nat, left, right)
}

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn lemma(id: locus::kernel::FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

// --- The stated gate conditions ---------------------------------------------

#[test]
fn a_u8_ordering_lemma_over_three_variables_is_proved_from_the_model() {
    // Declaring the theory is the proof: every lemma in it was checked by
    // the kernel, by reasoning and induction over Nat.
    let (definitions, prelude, theory) = setup();
    let mut ctx = Context::with_definitions(Rc::clone(&definitions));

    let a = Term::var(ctx.declare(Type::U8).unwrap());
    let b = Term::var(ctx.declare(Type::U8).unwrap());
    let c = Term::var(ctx.declare(Type::U8).unwrap());
    let ab = ctx
        .assume(prelude.u8_le_prop(a.clone(), b.clone()))
        .unwrap();
    let bc = ctx
        .assume(prelude.u8_le_prop(b.clone(), c.clone()))
        .unwrap();

    let ac = lemma(
        theory.u8_le_trans,
        vec![
            a.clone(),
            b.clone(),
            c.clone(),
            Term::proof(Proof::hyp(ab)),
            Term::proof(Proof::hyp(bc)),
        ],
    );
    assert_eq!(
        check_proof(&mut ctx, &ac, &prelude.u8_le_prop(a.clone(), c.clone())),
        Ok(())
    );
    // The premises are checked against the instantiated parameters.
    let crossed = lemma(
        theory.u8_le_trans,
        vec![
            a.clone(),
            b,
            c,
            Term::proof(Proof::hyp(bc)),
            Term::proof(Proof::hyp(ab)),
        ],
    );
    assert!(matches!(
        infer_proof(&mut ctx, &crossed),
        Err(KernelError::ProofMismatch { .. })
    ));
    // None of this is classical, and the lemma bounded_walk needs exists.
    for id in [
        theory.nat_add_assoc,
        theory.nat_le_trans,
        theory.u8_le_trans,
        theory.u8_zero_le,
    ] {
        assert!(!definitions.is_classical(id));
    }
    assert_eq!(
        check_proof(
            &mut ctx,
            &lemma(theory.u8_zero_le, vec![a.clone()]),
            &prelude.u8_le_prop(Term::U8(0), a)
        ),
        Ok(())
    );
    assert!(!proof_is_classical(&definitions, &ac));
}

/// A kernel proof that `nat_le(x, y)` for literals with `x <= y`.
fn literal_le(prelude: &Prelude, x: u64, y: u64) -> Proof {
    let (left, right) = (Term::nat(x), Term::nat(y));
    let body = Term::exists(Type::Nat, |k| {
        nat_eq(Term::nat_add(left.clone(), k), right.clone())
    });
    let witness = Term::nat(y - x);
    let sum = Proof::Literal(Term::nat_add(left.clone(), witness.clone()));
    fold_claim(
        &prelude.nat_le_prop(left.clone(), right.clone()),
        Proof::ExistsIntro {
            prop: body,
            witness,
            proof: Box::new(sum),
        },
    )
}

/// A kernel proof that `nat_lt(x, y)` for literals with `x < y`.
fn literal_lt(prelude: &Prelude, x: u64, y: u64) -> Proof {
    // nat_lt(x, y) unfolds to nat_le(succ(x), y); succ(x) evaluates to x + 1.
    let (left, right) = (Term::nat(x), Term::nat(y));
    let evaluated = Proof::Literal(Term::succ(left.clone()));
    let at_successor = Proof::transport(
        symm_at(&Type::Nat, &Term::succ(left.clone()), evaluated),
        |hole| prelude.nat_le_prop(hole, right.clone()),
        literal_le(prelude, x + 1, y),
    );
    fold_claim(&prelude.nat_lt_prop(left, right.clone()), at_successor)
}

/// Moves a fact about the models `a` and `b` onto `to_nat` of the bytes.
fn onto_bytes(
    a: u8,
    b: u8,
    claim: impl Fn(Term, Term) -> Term,
    about_models: Proof,
    swap: bool,
) -> Proof {
    let (first, second) = if swap { (b, a) } else { (a, b) };
    let model = |byte: u8| Term::to_nat(Term::U8(byte));
    let second_literal = Term::nat(u64::from(second));
    let first_done = Proof::transport(
        symm_at(&Type::Nat, &model(first), Proof::Literal(model(first))),
        |hole| claim(hole, second_literal.clone()),
        about_models,
    );
    let first_model = model(first);
    Proof::transport(
        symm_at(&Type::Nat, &model(second), Proof::Literal(model(second))),
        |hole| claim(first_model.clone(), hole),
        first_done,
    )
}

/// What the kernel's native evaluation answers for a comparison of literals.
fn native_comparison(ctx: &mut Context, prim: Prim, a: u8, b: u8) -> bool {
    let comparison = Term::prim(prim, vec![Term::U8(a), Term::U8(b)]);
    match infer_proof(ctx, &Proof::Literal(comparison)) {
        Ok(Term::Eq(_, _, value)) => *value == Term::Bool(true),
        other => panic!("no literal step: {other:?}"),
    }
}

#[test]
fn native_comparisons_agree_with_the_model_for_every_pair_of_bytes() {
    let (definitions, prelude, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let le = |x, y| prelude.nat_le_prop(x, y);
    let lt = |x, y| prelude.nat_lt_prop(x, y);

    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let (x, y) = (u64::from(a), u64::from(b));
            let native_lt = native_comparison(&mut ctx, Prim::U8Lt, a, b);
            let native_le = native_comparison(&mut ctx, Prim::U8Le, a, b);
            let native_eq = native_comparison(&mut ctx, Prim::U8Eq, a, b);
            // Whatever the native comparison answers, the model must be able
            // to prove the matching fact about to_nat(a) and to_nat(b).
            let (proof, claim) = if native_lt {
                let claim = prelude.u8_lt_prop(Term::U8(a), Term::U8(b));
                let fact = onto_bytes(a, b, lt, literal_lt(&prelude, x, y), false);
                (fold_claim(&claim, fact), claim)
            } else {
                let claim = prelude.u8_le_prop(Term::U8(b), Term::U8(a));
                let fact = onto_bytes(a, b, le, literal_le(&prelude, y, x), true);
                (fold_claim(&claim, fact), claim)
            };
            assert_eq!(check_proof(&mut ctx, &proof, &claim), Ok(()), "{a} < {b}");
            assert_eq!(native_le, native_lt || native_eq);
            assert_eq!(native_eq, a == b);
        }
    }
}

#[test]
fn native_arithmetic_agrees_with_the_model_for_every_pair_of_bytes() {
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let literal = |ctx: &mut Context, term: Term| match infer_proof(ctx, &Proof::Literal(term)) {
        Ok(Term::Eq(_, _, value)) => *value,
        other => panic!("no literal step: {other:?}"),
    };

    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let (x, y) = (Term::U8(a), Term::U8(b));
            // wrapping_add: the native answer must be what the model axiom
            // says, of_nat(to_nat(a) + to_nat(b)), evaluated step by step.
            let native = literal(&mut ctx, Term::wrapping_add(x.clone(), y.clone()));
            let (na, nb) = (Term::nat(u64::from(a)), Term::nat(u64::from(b)));
            let sum = Term::nat_add(na.clone(), nb.clone());
            let by_model = Chain::new(Type::U8, Term::wrapping_add(x.clone(), y.clone()))
                .step(Proof::Axiom(Axiom::WrappingAddModel(x.clone(), y.clone())))
                .rewrite(
                    |hole| Term::of_nat(Term::nat_add(hole, Term::to_nat(y.clone()))),
                    Proof::Literal(Term::to_nat(x.clone())),
                )
                .rewrite(
                    |hole| Term::of_nat(Term::nat_add(na.clone(), hole)),
                    Proof::Literal(Term::to_nat(y.clone())),
                )
                .rewrite(Term::of_nat, Proof::Literal(sum.clone()))
                .step(Proof::Literal(Term::of_nat(literal(&mut ctx, sum))))
                .finish();
            let goal = u8_eq(Term::wrapping_add(x.clone(), y.clone()), native);
            assert_eq!(check_proof(&mut ctx, &by_model, &goal), Ok(()), "{a} + {b}");

            // wrapping_sub: the model axiom says adding b back gives a.
            let difference = literal(&mut ctx, Term::wrapping_sub(x.clone(), y.clone()));
            let restored = literal(&mut ctx, Term::wrapping_add(difference, y));
            assert_eq!(restored, x, "{a} - {b}");
        }
    }
}

#[test]
fn native_conversions_agree_with_the_model_axioms() {
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let mut literal = |term: Term| match infer_proof(&mut ctx, &Proof::Literal(term)) {
        Ok(Term::Eq(_, _, value)) => *value,
        other => panic!("no literal step: {other:?}"),
    };
    for n in 0..1024u64 {
        let byte = literal(Term::of_nat(Term::nat(n)));
        // n < 256 => to_nat(of_nat(n)) == n
        if n < 256 {
            assert_eq!(literal(Term::to_nat(byte.clone())), Term::nat(n));
        }
        // of_nat(n + 256) == of_nat(n)
        let wrapped = literal(Term::nat_add(Term::nat(n), Term::nat(256)));
        assert_eq!(literal(Term::of_nat(wrapped)), byte);
        // to_nat(x) < 256, and of_nat(to_nat(x)) == x
        let Term::Nat(model) = literal(Term::to_nat(byte.clone())) else {
            panic!()
        };
        assert!(model.to_u64().is_some_and(|model| model < 256));
        assert_eq!(literal(Term::of_nat(Term::Nat(model))), byte);
    }
}

// --- Induction and the Peano axioms --------------------------------------------

#[test]
fn induction_checks_its_base_and_its_step() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let n = Term::var(ctx.declare_ghost(Type::Nat).unwrap());
    let zero = Term::nat(0);
    let claim = |k: Term| nat_eq(Term::nat_add(Term::nat(0), k.clone()), k);

    // The lemma, and the same induction written inline.
    assert_eq!(
        check_proof(
            &mut ctx,
            &lemma(theory.nat_zero_add, vec![n.clone()]),
            &claim(n.clone())
        ),
        Ok(())
    );
    let step = |k: Term, ih: Proof| {
        Chain::new(
            Type::Nat,
            Term::nat_add(Term::nat(0), Term::succ(k.clone())),
        )
        .step(Proof::Axiom(Axiom::NatAddSucc(Term::nat(0), k)))
        .rewrite(Term::succ, ih)
        .finish()
    };
    let inline = Proof::nat_induction(
        claim,
        Proof::Axiom(Axiom::NatAddZero(zero.clone())),
        step,
        n.clone(),
    );
    assert_eq!(check_proof(&mut ctx, &inline, &claim(n.clone())), Ok(()));

    // A wrong base case.
    let bad_base = Proof::nat_induction(claim, Proof::Refl(zero.clone()), step, n.clone());
    assert!(matches!(
        infer_proof(&mut ctx, &bad_base),
        Err(KernelError::ProofMismatch { .. })
    ));
    // A step that only restates its hypothesis proves the claim about k, not
    // about succ(k).
    let lazy_step = Proof::nat_induction(
        claim,
        Proof::Axiom(Axiom::NatAddZero(zero.clone())),
        |_, ih| ih,
        n.clone(),
    );
    assert!(matches!(
        infer_proof(&mut ctx, &lazy_step),
        Err(KernelError::ProofMismatch { .. })
    ));
    // The motive must be a proposition and the target a Nat.
    let data_motive = Proof::nat_induction(|k| k, Proof::Refl(zero.clone()), |_, ih| ih, n);
    assert!(matches!(
        infer_proof(&mut ctx, &data_motive),
        Err(KernelError::TypeMismatch { .. })
    ));
    let byte_target = Proof::nat_induction(
        claim,
        Proof::Axiom(Axiom::NatAddZero(zero)),
        step,
        Term::U8(3),
    );
    assert!(matches!(
        infer_proof(&mut ctx, &byte_target),
        Err(KernelError::TypeMismatch { .. })
    ));
}

#[test]
fn successor_is_injective_and_never_zero() {
    let (definitions, prelude, _) = setup();
    let mut ctx = Context::with_definitions(definitions);

    // 1 == 0 => False: 1 is succ(0), and no successor is zero.
    let one_is_zero = nat_eq(Term::nat(1), Term::nat(0));
    let refuted = Proof::implies_intro(one_is_zero.clone(), |h| {
        let one = Proof::Literal(Term::succ(Term::nat(0)));
        let succ_is_zero = Proof::transport(
            symm_at(&Type::Nat, &Term::succ(Term::nat(0)), one),
            |hole| nat_eq(hole, Term::nat(0)),
            h,
        );
        Proof::implies_elim(
            Proof::Axiom(Axiom::NatSuccNotZero(Term::nat(0))),
            succ_is_zero,
        )
    });
    assert_eq!(
        check_proof(&mut ctx, &refuted, &prelude.not_prop(one_is_zero)),
        Ok(())
    );

    let a = Term::var(ctx.declare_ghost(Type::Nat).unwrap());
    let b = Term::var(ctx.declare_ghost(Type::Nat).unwrap());
    let h = ctx
        .assume(nat_eq(Term::succ(a.clone()), Term::succ(b.clone())))
        .unwrap();
    let injective = Proof::implies_elim(
        Proof::Axiom(Axiom::NatSuccInjective(a.clone(), b.clone())),
        Proof::hyp(h),
    );
    assert_eq!(check_proof(&mut ctx, &injective, &nat_eq(a, b)), Ok(()));
}

// --- The model in use ------------------------------------------------------------

#[test]
fn nat_is_ghost_and_axioms_are_typed() {
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let x = Term::var(ctx.declare(Type::U8).unwrap());

    // The model of an executable byte is not executable.
    assert!(matches!(
        infer_term(&mut ctx, &Term::to_nat(x.clone()), Mode::Executable),
        Err(KernelError::GhostTypeInExecutable(Type::Nat))
    ));
    assert_eq!(
        infer_term(&mut ctx, &Term::to_nat(x.clone()), Mode::Logical),
        Ok(Type::Nat)
    );
    // The runtime comparison is executable and has type bool.
    let comparison = Term::prim(Prim::U8Lt, vec![x.clone(), Term::U8(9)]);
    assert_eq!(
        infer_term(&mut ctx, &comparison, Mode::Executable),
        Ok(Type::Bool)
    );
    // An axiom about bytes does not accept a Nat, and conversely.
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Axiom(Axiom::ToNatBound(Term::nat(3)))),
        Err(KernelError::TypeMismatch { .. })
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Axiom(Axiom::NatAddZero(x.clone()))),
        Err(KernelError::TypeMismatch { .. })
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Axiom(Axiom::Reflect(x, true))),
        Err(KernelError::TypeMismatch { .. })
    ));
    // The axioms are stated with the prelude's orderings.
    let mut bare = Context::new();
    assert_eq!(
        infer_proof(&mut bare, &Proof::Axiom(Axiom::NatAddZero(Term::nat(1)))),
        Err(KernelError::NoPrelude)
    );
    // Nat literals are not machine integers: arithmetic continues past u64.
    let past = Term::succ(Term::nat(u64::MAX));
    let Ok(Term::Eq(_, _, value)) = infer_proof(&mut ctx, &Proof::Literal(past)) else {
        panic!("succ has a literal step at any size")
    };
    assert_eq!(value.to_string(), "18446744073709551616n");
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Literal(Term::of_nat(*value))),
        Ok(u8_eq(
            Term::of_nat(Term::Nat(locus::kernel::Natural::from(u64::MAX).succ())),
            Term::U8(0)
        ))
    );
}

#[test]
fn a_runtime_comparison_reflects_into_its_proposition() {
    // The branches of "if a < b": each learns the proposition, or its
    // negation, from the value of the runtime comparison. Together they
    // decide a < b with no classical reasoning.
    let (definitions, prelude, _) = setup();
    let shared = Rc::clone(&definitions);
    let mut ctx = Context::with_definitions(definitions);
    let a = Term::var(ctx.declare(Type::U8).unwrap());
    let b = Term::var(ctx.declare(Type::U8).unwrap());
    let comparison = Term::prim(Prim::U8Lt, vec![a.clone(), b.clone()]);
    let claim = prelude.u8_lt_prop(a, b);
    let refutation = prelude.not_prop(claim.clone());
    let goal = prelude.or_prop(claim.clone(), refutation.clone());

    let side = |variant: usize, proof: Proof| Proof::Construct {
        prop: prelude.or,
        variant,
        params: vec![claim.clone(), refutation.clone()],
        payload: vec![Term::proof(proof)],
    };
    let decided = Proof::CaseData {
        scrutinee: comparison.clone(),
        goal: goal.clone(),
        arms: vec![
            // comparison == false
            Proof::arm(0, 1, |_, facts| {
                let reflect = Proof::Axiom(Axiom::Reflect(comparison.clone(), false));
                side(1, Proof::implies_elim(reflect, facts[0].clone()))
            }),
            // comparison == true
            Proof::arm(0, 1, |_, facts| {
                let reflect = Proof::Axiom(Axiom::Reflect(comparison.clone(), true));
                side(0, Proof::implies_elim(reflect, facts[0].clone()))
            }),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &decided, &goal), Ok(()));
    assert!(!proof_is_classical(&shared, &decided));
}

#[test]
fn nonzero_is_now_expressible() {
    // struct NonZero { value: u8, evidence: @[value != 0] }
    let (mut definitions, prelude) = Definitions::with_prelude();
    let fields = Type::tuple(|earlier| match earlier {
        [] => Some(Type::U8),
        [value] => Some(Type::proof(
            prelude.not_prop(u8_eq(value.clone(), Term::U8(0))),
        )),
        _ => None,
    });
    let nonzero = definitions.declare_struct(&fields).unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    // For a literal, the evidence is the runtime comparison, evaluated and
    // reflected: u8_eq(5, 0) == false, hence 5 == 0 => False.
    let evidence_for = |byte: u8| {
        let comparison = Term::prim(Prim::U8Eq, vec![Term::U8(byte), Term::U8(0)]);
        Proof::implies_elim(
            Proof::Axiom(Axiom::Reflect(comparison.clone(), false)),
            Proof::Literal(comparison),
        )
    };
    let five = Term::Struct(nonzero, vec![Term::U8(5), Term::proof(evidence_for(5))]);
    assert_eq!(
        infer_term(&mut ctx, &five, Mode::Executable),
        Ok(Type::Struct(nonzero))
    );
    // Zero cannot be packaged: the comparison evaluates to true.
    let zero = Term::Struct(nonzero, vec![Term::U8(0), Term::proof(evidence_for(0))]);
    assert!(matches!(
        infer_term(&mut ctx, &zero, Mode::Executable),
        Err(KernelError::ProofMismatch { .. })
    ));
}

#[test]
fn the_facts_bounded_walk_needs_are_lemmas_over_the_model() {
    // The else branch of the specification's bounded_walk (section 10.4):
    //     let differs: @[i != limit] = _;
    //     let below: @[i < limit] = lt_of_le_of_ne(i, limit, bound, differs);
    //     let next = i.wrapping_add(1);
    //     let next_bound: @[next <= limit] = step_stays_below(i, limit, below);
    let (definitions, prelude, theory) = setup();
    let mut ctx = Context::with_definitions(Rc::clone(&definitions));
    let i = Term::var(ctx.declare(Type::U8).unwrap());
    let limit = Term::var(ctx.declare(Type::U8).unwrap());
    let bound = ctx
        .assume(prelude.u8_le_prop(i.clone(), limit.clone()))
        .unwrap();
    let differs = ctx
        .assume(prelude.not_prop(u8_eq(i.clone(), limit.clone())))
        .unwrap();

    let below = lemma(
        theory.u8_lt_of_le_of_ne,
        vec![
            i.clone(),
            limit.clone(),
            Term::proof(Proof::hyp(bound)),
            Term::proof(Proof::hyp(differs)),
        ],
    );
    assert_eq!(
        check_proof(
            &mut ctx,
            &below,
            &prelude.u8_lt_prop(i.clone(), limit.clone())
        ),
        Ok(())
    );

    let (next, next_is) = ctx
        .define(&Term::wrapping_add(i.clone(), Term::U8(1)))
        .unwrap();
    let at_successor = lemma(
        theory.u8_succ_le_of_lt,
        vec![i.clone(), limit.clone(), Term::proof(below)],
    );
    // The lemma speaks of i.wrapping_add(1); the let equation carries it to
    // `next`, which is the step the elaborator inserts silently.
    let next_bound = Proof::transport(
        symm_at(&Type::U8, &Term::var(next), Proof::hyp(next_is)),
        |hole| prelude.u8_le_prop(hole, limit.clone()),
        at_successor,
    );
    assert_eq!(
        check_proof(
            &mut ctx,
            &next_bound,
            &prelude.u8_le_prop(Term::var(next), limit.clone())
        ),
        Ok(())
    );

    // The initial invariant, 0 <= limit, and nothing here is classical.
    assert!(
        check_proof(
            &mut ctx,
            &lemma(theory.u8_zero_le, vec![limit.clone()]),
            &prelude.u8_le_prop(Term::U8(0), limit)
        )
        .is_ok()
    );
    for id in [
        theory.nat_succ_add,
        theory.nat_zero_or_succ,
        theory.nat_le_succ_succ,
        theory.u8_lt_of_le_of_ne,
        theory.u8_succ_le_of_lt,
    ] {
        assert!(!definitions.is_classical(id));
    }
}
