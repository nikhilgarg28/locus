//! Tests for the closed-term evaluator, proof by evaluation, and the range
//! successor axiom (the kernel contract in atlas.html).
//! Every term here is written by hand; nothing comes from the parser.

use std::rc::Rc;

use locus::kernel::derive::trans;
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, Context, Definitions, KernelError, Prelude, Prim, Proof, Term, Type, check_proof,
    infer_proof,
};

fn setup() -> (Definitions, Prelude, Theory) {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    (definitions, prelude, theory)
}

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn add_one(term: Term) -> Term {
    Term::wrapping_add(term, Term::U8(1))
}

fn lemma(id: locus::kernel::FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

fn sum_state(_: Term) -> Type {
    Type::Tuple(vec![Type::U8])
}

/// `for i in 0..n (acc: u8 = 0) { continue(acc.wrapping_add(i)) }`
fn sum_below(theory: &Theory, n: Term) -> Term {
    Term::for_range(
        Term::U8(0),
        n.clone(),
        lemma(theory.u8_zero_le, vec![n]),
        sum_state,
        Term::tuple(&sum_state(Term::U8(0)), vec![Term::U8(0)]),
        |i, s, _, _| {
            Term::tuple(
                &sum_state(Term::U8(0)),
                vec![Term::wrapping_add(Term::proj(s, 0), i)],
            )
        },
    )
}

fn counting_state(i: Term) -> Type {
    Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [acc] => Some(Type::proof(u8_eq(acc.clone(), i.clone()))),
        _ => None,
    })
}

/// The index-dependent loop of gate K6.
fn count_up(theory: &Theory, n: Term) -> Term {
    let zero = Term::U8(0);
    Term::for_range(
        zero.clone(),
        n.clone(),
        lemma(theory.u8_zero_le, vec![n]),
        counting_state,
        Term::tuple(
            &counting_state(zero.clone()),
            vec![zero.clone(), Term::proof(Proof::Refl(zero))],
        ),
        |i, s, _, _| {
            let acc = Term::proj(s.clone(), 0);
            let left = add_one(acc.clone());
            Term::tuple(
                &counting_state(add_one(i)),
                vec![
                    add_one(acc.clone()),
                    Term::proof(Proof::transport(
                        Proof::OfTerm(Term::proj(s, 1)),
                        |hole| u8_eq(left.clone(), add_one(hole)),
                        Proof::Refl(add_one(acc)),
                    )),
                ],
            )
        },
    )
}

#[test]
fn a_closed_term_evaluates_in_one_step() {
    let (mut definitions, _, theory) = setup();
    let successor = definitions
        .declare_fn(&Type::function(1, |_| Type::U8), |params| {
            add_one(params[0].clone())
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(definitions));

    // A call, unfolded and computed.
    let call = Term::call(Term::Fn(successor), vec![Term::U8(41)]);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(call.clone())),
        Ok(u8_eq(call, Term::U8(42)))
    );
    // A loop: 0 + 1 + ... + 9 = 45.
    let total = Term::proj(sum_below(&theory, Term::U8(10)), 0);
    assert_eq!(
        check_proof(
            &mut ctx,
            &Proof::Evaluate(total.clone()),
            &u8_eq(total, Term::U8(45))
        ),
        Ok(())
    );
    // A loop whose state carries proofs: the data can be evaluated, because
    // proofs are never looked at.
    let count = Term::proj(count_up(&theory, Term::U8(200)), 0);
    assert_eq!(
        check_proof(
            &mut ctx,
            &Proof::Evaluate(count.clone()),
            &u8_eq(count, Term::U8(200))
        ),
        Ok(())
    );
    // A case and a product.
    let pair_type = Type::Tuple(vec![Type::U8, Type::Bool]);
    let chosen = Term::case(
        Term::prim(Prim::U8Lt, vec![Term::U8(3), Term::U8(4)]),
        pair_type.clone(),
        vec![
            (
                0,
                Box::new(|_, _| Term::tuple(&pair_type, vec![Term::U8(0), Term::Bool(false)])),
            ),
            (
                0,
                Box::new(|_, _| {
                    Term::tuple(&pair_type, vec![add_one(Term::U8(6)), Term::Bool(true)])
                }),
            ),
        ],
    );
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(chosen.clone())),
        Ok(Term::eq(
            pair_type.clone(),
            chosen,
            Term::tuple(&pair_type, vec![Term::U8(7), Term::Bool(true)])
        ))
    );
    // A false claim is simply not what evaluation proves.
    let four = add_one(Term::U8(3));
    assert!(matches!(
        check_proof(
            &mut ctx,
            &Proof::Evaluate(four.clone()),
            &u8_eq(four, Term::U8(5))
        ),
        Err(KernelError::ProofMismatch { .. })
    ));
}

#[test]
fn evaluation_is_for_closed_terms_of_plain_data() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let n = Term::var(ctx.declare(Type::U8).unwrap());

    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Evaluate(add_one(n))),
        Err(KernelError::NotClosed(_))
    ));
    // The whole counting state has a proof field, so it is not offered; its
    // data field is.
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Evaluate(count_up(&theory, Term::U8(3)))),
        Err(KernelError::NotPlainData(_))
    ));
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Evaluate(u8_eq(Term::U8(1), Term::U8(1)))),
        Err(KernelError::NotPlainData(Type::Prop))
    ));
    // Ill-typed terms never reach the evaluator.
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Evaluate(add_one(Term::Bool(true)))),
        Err(KernelError::TypeMismatch { .. })
    ));
}

#[test]
fn the_evaluator_has_a_step_budget_not_a_time_limit() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    // Three nested loops of 200 iterations: eight million bodies.
    let nest = |inner: Term| {
        Term::for_range(
            Term::U8(0),
            Term::U8(200),
            lemma(theory.u8_zero_le, vec![Term::U8(200)]),
            sum_state,
            Term::tuple(&sum_state(Term::U8(0)), vec![Term::U8(0)]),
            move |_, _, _, _| inner.clone(),
        )
    };
    let unit = Term::tuple(&sum_state(Term::U8(0)), vec![Term::U8(1)]);
    let deep = Term::proj(nest(nest(nest(unit))), 0);
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(deep)),
        Err(KernelError::StepLimit)
    );
}

#[test]
fn a_claim_about_every_byte_is_proved_by_256_evaluations() {
    let (definitions, prelude, _) = setup();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let top = |x: Term| Term::prim(Prim::U8Le, vec![x, Term::U8(255)]);

    let all = Proof::evaluate_all(top);
    let statement = Term::forall(Type::U8, |x| Term::eq(Type::Bool, top(x), Term::Bool(true)));
    assert_eq!(check_proof(&mut ctx, &all, &statement), Ok(()));

    // With reflection this is a fact about the ordering of any byte.
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let at_n = Proof::implies_elim(
        Proof::Axiom(Axiom::Reflect(top(n.clone()), true)),
        Proof::forall_elim(all, n.clone()),
    );
    assert_eq!(
        check_proof(&mut ctx, &at_n, &prelude.u8_le_prop(n, Term::U8(255))),
        Ok(())
    );

    // A claim with a counterexample is refuted, and says where.
    let strict = Proof::evaluate_all(|x| Term::prim(Prim::U8Lt, vec![x, Term::U8(255)]));
    assert_eq!(
        infer_proof(&mut ctx, &strict),
        Err(KernelError::Refuted(Term::U8(255)))
    );
    // The body must be a bool.
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::evaluate_all(add_one)),
        Err(KernelError::TypeMismatch { .. })
    ));
}

#[test]
fn a_loop_unrolls_one_step_at_a_successor_bound() {
    let (definitions, prelude, theory) = setup();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let h = Term::var(ctx.declare(Type::U8).unwrap());
    let next = add_one(h.clone());
    // h + 1 does not wrap.
    let no_wrap = ctx
        .assume(prelude.u8_lt_prop(h.clone(), next.clone()))
        .unwrap();
    let lower = lemma(theory.u8_zero_le, vec![h.clone()]);

    // sum_below(h + 1) == (sum_below(h).0 + h,)
    let looped = sum_below(&theory, next.clone());
    let step = Proof::ForStep {
        looped: looped.clone(),
        lower: Box::new(lower.clone()),
        upper: Box::new(Proof::hyp(no_wrap)),
    };
    let previous = sum_below(&theory, h.clone());
    let expected = Term::tuple(
        &sum_state(Term::U8(0)),
        vec![Term::wrapping_add(Term::proj(previous, 0), h.clone())],
    );
    assert_eq!(
        check_proof(
            &mut ctx,
            &step,
            &Term::eq(sum_state(Term::U8(0)), looped, expected)
        ),
        Ok(())
    );

    // The same for index-dependent state: both sides have the state type at
    // h + 1, so no transport between types is needed.
    let counted = count_up(&theory, next.clone());
    let counted_step = Proof::ForStep {
        looped: counted.clone(),
        lower: Box::new(lower.clone()),
        upper: Box::new(Proof::hyp(no_wrap)),
    };
    let Ok(Term::Eq(ty, left, _)) = infer_proof(&mut ctx, &counted_step) else {
        panic!("the dependent loop unrolls")
    };
    assert_eq!(ty, counting_state(next));
    assert_eq!(*left, counted);

    // The bound must be a successor, and the premises must be the right ones.
    let not_successor = Proof::ForStep {
        looped: sum_below(&theory, h.clone()),
        lower: Box::new(lower.clone()),
        upper: Box::new(Proof::hyp(no_wrap)),
    };
    assert!(matches!(
        infer_proof(&mut ctx, &not_successor),
        Err(KernelError::NoComputationStep(_))
    ));
    let wrong_upper = Proof::ForStep {
        looped: sum_below(&theory, add_one(h.clone())),
        lower: Box::new(lower.clone()),
        upper: Box::new(lower),
    };
    assert!(matches!(
        infer_proof(&mut ctx, &wrong_upper),
        Err(KernelError::ProofMismatch { .. })
    ));
}

#[test]
fn unrolling_and_the_empty_range_compute_a_loop_symbolically() {
    // sum_below(0 + 1) == (0 + 0,): one successor step, then the empty range.
    let (definitions, prelude, theory) = setup();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let zero = Term::U8(0);
    let one = add_one(zero.clone());
    let no_wrap = ctx
        .assume(prelude.u8_lt_prop(zero.clone(), one.clone()))
        .unwrap();
    let lower = lemma(theory.u8_zero_le, vec![zero.clone()]);

    let looped = sum_below(&theory, one);
    let unroll = Proof::ForStep {
        looped: looped.clone(),
        lower: Box::new(lower.clone()),
        upper: Box::new(Proof::hyp(no_wrap)),
    };
    // Inside the unrolled body sits the loop over 0..0, with `lower` as its
    // ordering proof; rewrite it to its initial state.
    let empty = sum_below(&theory, zero.clone());
    let init = Term::tuple(&sum_state(zero.clone()), vec![zero.clone()]);
    let state = sum_state(zero.clone());
    let collapse = Proof::transport(
        Proof::ForEmpty(empty.clone()),
        |hole| {
            Term::eq(
                state.clone(),
                Term::tuple(
                    &state,
                    vec![Term::wrapping_add(
                        Term::proj(empty.clone(), 0),
                        Term::U8(0),
                    )],
                ),
                Term::tuple(
                    &state,
                    vec![Term::wrapping_add(Term::proj(hole, 0), Term::U8(0))],
                ),
            )
        },
        Proof::Refl(Term::tuple(
            &state,
            vec![Term::wrapping_add(
                Term::proj(empty.clone(), 0),
                Term::U8(0),
            )],
        )),
    );
    let both = trans(&mut ctx, &unroll, &collapse).unwrap();
    let goal = Term::eq(
        state.clone(),
        looped,
        Term::tuple(
            &state,
            vec![Term::wrapping_add(Term::proj(init, 0), Term::U8(0))],
        ),
    );
    assert_eq!(check_proof(&mut ctx, &both, &goal), Ok(()));
}
