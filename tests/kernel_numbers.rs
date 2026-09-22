//! Acceptance tests for the numbers of the kernel (the kernel contract in
//! atlas.html): the model of `u8` over `Int`, reflection of runtime
//! comparisons, and agreement between native evaluation and the model.
//! Every term here is written by hand; nothing comes from the parser.

use std::rc::Rc;

use locus::kernel::derive::{Chain, symm_at};
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, CmpOp, Context, Definitions, KernelError, MachineInt, Mode, Op, Prelude, Proof, Term,
    Type, check_proof, infer_proof, infer_term, proof_is_classical,
};

fn setup() -> (Rc<Definitions>, Prelude, Theory) {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    (Rc::new(definitions), prelude, theory)
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
    // the kernel, by reasoning from the axioms of `Int` and of the model.
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(Rc::clone(&definitions));

    let a = Term::var(ctx.declare(Type::U8).unwrap());
    let b = Term::var(ctx.declare(Type::U8).unwrap());
    let c = Term::var(ctx.declare(Type::U8).unwrap());
    let ab = ctx
        .assume(Term::int_le(
            Term::view(MachineInt::U8, a.clone()),
            Term::view(MachineInt::U8, b.clone()),
        ))
        .unwrap();
    let bc = ctx
        .assume(Term::int_le(
            Term::view(MachineInt::U8, b.clone()),
            Term::view(MachineInt::U8, c.clone()),
        ))
        .unwrap();

    let ac = lemma(
        theory.machine(MachineInt::U8).le_trans,
        vec![
            a.clone(),
            b.clone(),
            c.clone(),
            Term::proof(Proof::hyp(ab)),
            Term::proof(Proof::hyp(bc)),
        ],
    );
    assert_eq!(
        check_proof(
            &mut ctx,
            &ac,
            &Term::int_le(
                Term::view(MachineInt::U8, a.clone()),
                Term::view(MachineInt::U8, c.clone())
            )
        ),
        Ok(())
    );
    // The premises are checked against the instantiated parameters.
    let crossed = lemma(
        theory.machine(MachineInt::U8).le_trans,
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
        theory.int_le_of_lt,
        theory.int_le_add_left,
        theory.machine(MachineInt::U8).le_trans,
        theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
    ] {
        assert!(!definitions.is_classical(id));
    }
    assert_eq!(
        check_proof(
            &mut ctx,
            &lemma(
                theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
                vec![a.clone()]
            ),
            &Term::int_le(
                Term::view(MachineInt::U8, Term::U8(0)),
                Term::view(MachineInt::U8, a)
            )
        ),
        Ok(())
    );
    assert!(!proof_is_classical(&definitions, &ac));
}

/// What the kernel's native evaluation answers for a comparison of literals.
fn native_comparison(ctx: &mut Context, op: CmpOp, a: u8, b: u8) -> bool {
    let comparison = Term::cmp(op, MachineInt::U8, Term::U8(a), Term::U8(b));
    match infer_proof(ctx, &Proof::Literal(comparison)) {
        Ok(Term::Eq(_, _, value)) => *value == Term::Bool(true),
        other => panic!("no literal step: {other:?}"),
    }
}

#[test]
fn native_comparisons_agree_with_the_model_for_every_pair_of_bytes() {
    // Whatever a native comparison answers, the matching fact about the
    // views is provable: reflected from the evaluated comparison, and
    // decided again by evaluating the views themselves.
    let (definitions, prelude, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let view = |byte: u8| Term::view(MachineInt::U8, Term::U8(byte));

    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let native_lt = native_comparison(&mut ctx, CmpOp::Lt, a, b);
            let native_le = native_comparison(&mut ctx, CmpOp::Le, a, b);
            let native_eq = native_comparison(&mut ctx, CmpOp::Eq, a, b);
            assert_eq!(native_lt, a < b);
            assert_eq!(native_le, a <= b);
            assert_eq!(native_eq, a == b);
            let comparison = Term::cmp(CmpOp::Lt, MachineInt::U8, Term::U8(a), Term::U8(b));
            let reflected = Proof::implies_elim(
                Proof::Axiom(Axiom::CmpReflect(comparison.clone(), native_lt)),
                Proof::Literal(comparison),
            );
            let below = Term::int_lt(view(a), view(b));
            let claim = if native_lt {
                below.clone()
            } else {
                prelude.not_prop(below.clone())
            };
            assert_eq!(
                check_proof(&mut ctx, &reflected, &claim),
                Ok(()),
                "{a} < {b}"
            );
            if a % 17 == 0 || b % 17 == 0 {
                assert_eq!(
                    check_proof(&mut ctx, &Proof::Evaluate(below), &claim),
                    Ok(())
                );
            }
        }
    }
}

#[test]
fn native_arithmetic_agrees_with_the_model_for_every_pair_of_bytes() {
    // wrapping_add: the native answer is what op_model says, wrap of the
    // sum of the views, evaluated step by step through the literal axiom.
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let literal = |ctx: &mut Context, term: Term| match infer_proof(ctx, &Proof::Literal(term)) {
        Ok(Term::Eq(_, _, value)) => *value,
        other => panic!("no literal step: {other:?}"),
    };
    let view = |x: &Term| Term::view(MachineInt::U8, x.clone());

    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let (x, y) = (Term::U8(a), Term::U8(b));
            let sum = Term::op(Op::WrappingAdd, MachineInt::U8, vec![x.clone(), y.clone()]);
            let native = literal(&mut ctx, sum.clone());
            assert_eq!(native, Term::U8(a.wrapping_add(b)));
            let (ia, ib) = (Term::int(i64::from(a)), Term::int(i64::from(b)));
            let exact = Term::int_add(ia.clone(), ib.clone());
            let by_model = Chain::new(Type::U8, sum.clone())
                .step(Proof::Axiom(Axiom::OpModel(
                    Op::WrappingAdd,
                    MachineInt::U8,
                    vec![x.clone(), y.clone()],
                )))
                .rewrite(
                    |hole| Term::wrap(MachineInt::U8, Term::int_add(hole, view(&y))),
                    Proof::Literal(view(&x)),
                )
                .rewrite(
                    |hole| Term::wrap(MachineInt::U8, Term::int_add(ia.clone(), hole)),
                    Proof::Literal(view(&y)),
                )
                .rewrite(
                    |hole| Term::wrap(MachineInt::U8, hole),
                    Proof::Literal(exact.clone()),
                )
                .step(Proof::Literal(Term::wrap(
                    MachineInt::U8,
                    literal(&mut ctx, exact),
                )))
                .finish();
            let goal = u8_eq(sum, native);
            assert_eq!(check_proof(&mut ctx, &by_model, &goal), Ok(()), "{a} + {b}");

            // wrapping_sub: adding b back gives a.
            let difference = literal(
                &mut ctx,
                Term::op(Op::WrappingSub, MachineInt::U8, vec![x.clone(), y.clone()]),
            );
            let restored = literal(
                &mut ctx,
                Term::op(Op::WrappingAdd, MachineInt::U8, vec![difference, y]),
            );
            assert_eq!(restored, x, "{a} - {b}");
        }
    }
}

#[test]
fn native_conversions_agree_with_the_model_axioms() {
    // view and wrap on literals, against the axioms of the model of u8
    // over Int: the round trips close, and wrap has period 256.
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let mut literal = |term: Term| match infer_proof(&mut ctx, &Proof::Literal(term)) {
        Ok(Term::Eq(_, _, value)) => *value,
        other => panic!("no literal step: {other:?}"),
    };
    for n in -300i64..1024 {
        let byte = literal(Term::wrap(MachineInt::U8, Term::int(n)));
        assert_eq!(byte, Term::U8(n.rem_euclid(256) as u8));
        // 0 <= n <= 255 => view(wrap(n)) == n
        if (0..256).contains(&n) {
            assert_eq!(
                literal(Term::view(MachineInt::U8, byte.clone())),
                Term::int(n)
            );
        }
        // wrap(n + 256) == wrap(n)
        let shifted = literal(Term::int_add(Term::int(n), Term::int(256)));
        assert_eq!(literal(Term::wrap(MachineInt::U8, shifted)), byte);
        // 0 <= view(x) <= 255, and wrap(view(x)) == x
        let Term::Int(model) = literal(Term::view(MachineInt::U8, byte.clone())) else {
            panic!()
        };
        assert!(
            model
                .to_i128()
                .is_some_and(|model| (0..256).contains(&model))
        );
        assert_eq!(literal(Term::wrap(MachineInt::U8, Term::Int(model))), byte);
    }
}

// --- Induction and the Peano axioms --------------------------------------------

// --- The model in use ------------------------------------------------------------

#[test]
fn the_model_of_u8_is_ghost_and_axioms_are_typed() {
    let (definitions, _, _) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let x = Term::var(ctx.declare(Type::U8).unwrap());

    // The model of an executable byte is not executable.
    assert!(matches!(
        infer_term(
            &mut ctx,
            &Term::view(MachineInt::U8, x.clone()),
            Mode::Executable
        ),
        Err(KernelError::GhostTypeInExecutable(Type::Int))
    ));
    assert_eq!(
        infer_term(
            &mut ctx,
            &Term::view(MachineInt::U8, x.clone()),
            Mode::Logical
        ),
        Ok(Type::Int)
    );
    // The runtime comparison is executable and has type bool.
    let comparison = Term::cmp(CmpOp::Lt, MachineInt::U8, x.clone(), Term::U8(9));
    assert_eq!(
        infer_term(&mut ctx, &comparison, Mode::Executable),
        Ok(Type::Bool)
    );
    // An axiom about bytes does not accept an integer, and conversely.
    assert!(matches!(
        infer_proof(
            &mut ctx,
            &Proof::Axiom(Axiom::ViewLower(MachineInt::U8, Term::int(3)))
        ),
        Err(KernelError::TypeMismatch { .. })
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Axiom(Axiom::IntAddZero(x.clone()))),
        Err(KernelError::TypeMismatch { .. })
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Axiom(Axiom::CmpReflect(x, true))),
        Err(KernelError::TypeMismatch { .. })
    ));
    // The axioms are stated with the prelude's propositions.
    let mut bare = Context::new();
    assert_eq!(
        infer_proof(&mut bare, &Proof::Axiom(Axiom::IntAddZero(Term::int(1)))),
        Err(KernelError::NoPrelude)
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
    let comparison = Term::cmp(CmpOp::Lt, MachineInt::U8, a.clone(), b.clone());
    let claim = Term::int_lt(Term::view(MachineInt::U8, a), Term::view(MachineInt::U8, b));
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
                let reflect = Proof::Axiom(Axiom::CmpReflect(comparison.clone(), false));
                side(1, Proof::implies_elim(reflect, facts[0].clone()))
            }),
            // comparison == true
            Proof::arm(0, 1, |_, facts| {
                let reflect = Proof::Axiom(Axiom::CmpReflect(comparison.clone(), true));
                side(0, Proof::implies_elim(reflect, facts[0].clone()))
            }),
        ],
    };
    assert_eq!(check_proof(&mut ctx, &decided, &goal), Ok(()));
    assert!(!proof_is_classical(&shared, &decided));
}

#[test]
fn nonzero_is_now_expressible() {
    // struct NonZero { value: u8, evidence: @[value != 0] }, the claim over
    // the views, as reflecting the test gives it.
    let (mut definitions, prelude) = Definitions::with_prelude();
    let view = |x: Term| Term::view(MachineInt::U8, x);
    let fields = Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [value] => Some(Type::proof(prelude.not_prop(Term::eq(
            Type::Int,
            view(value.clone()),
            view(Term::U8(0)),
        )))),
        _ => None,
    });
    let nonzero = definitions.declare_struct(&fields).unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    // For a literal, the evidence is the runtime comparison, evaluated and
    // reflected: u8_eq(5, 0) == false, hence 5 == 0 => False.
    let evidence_for = |byte: u8| {
        let comparison = Term::cmp(CmpOp::Eq, MachineInt::U8, Term::U8(byte), Term::U8(0));
        Proof::implies_elim(
            Proof::Axiom(Axiom::CmpReflect(comparison.clone(), false)),
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
        .assume(Term::int_le(
            Term::view(MachineInt::U8, i.clone()),
            Term::view(MachineInt::U8, limit.clone()),
        ))
        .unwrap();
    let differs = ctx
        .assume(prelude.not_prop(Term::eq(
            Type::Int,
            Term::view(MachineInt::U8, i.clone()),
            Term::view(MachineInt::U8, limit.clone()),
        )))
        .unwrap();

    let below = lemma(
        theory.machine(MachineInt::U8).lt_of_le_of_ne,
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
            &Term::int_lt(
                Term::view(MachineInt::U8, i.clone()),
                Term::view(MachineInt::U8, limit.clone())
            )
        ),
        Ok(())
    );

    let (next, next_is) = ctx
        .define(&Term::op(
            Op::WrappingAdd,
            MachineInt::U8,
            vec![i.clone(), Term::U8(1)],
        ))
        .unwrap();
    let at_successor = lemma(
        theory.machine(MachineInt::U8).succ_le_of_lt,
        vec![i.clone(), limit.clone(), Term::proof(below)],
    );
    // The lemma speaks of i.wrapping_add(1); the let equation carries it to
    // `next`, which is the step the elaborator inserts silently.
    let next_bound = Proof::transport(
        symm_at(&Type::U8, &Term::var(next), Proof::hyp(next_is)),
        |hole| {
            Term::int_le(
                Term::view(MachineInt::U8, hole),
                Term::view(MachineInt::U8, limit.clone()),
            )
        },
        at_successor,
    );
    assert_eq!(
        check_proof(
            &mut ctx,
            &next_bound,
            &Term::int_le(
                Term::view(MachineInt::U8, Term::var(next)),
                Term::view(MachineInt::U8, limit.clone())
            )
        ),
        Ok(())
    );

    // The initial invariant, 0 <= limit, and nothing here is classical.
    assert!(
        check_proof(
            &mut ctx,
            &lemma(
                theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
                vec![limit.clone()]
            ),
            &Term::int_le(
                Term::view(MachineInt::U8, Term::U8(0)),
                Term::view(MachineInt::U8, limit)
            )
        )
        .is_ok()
    );
    for id in [
        theory.int_lt_of_le_of_ne,
        theory.int_le_add_right,
        theory.int_le_sub,
        theory.machine(MachineInt::U8).lt_of_le_of_ne,
        theory.machine(MachineInt::U8).succ_le_of_lt,
    ] {
        assert!(!definitions.is_classical(id));
    }
}
