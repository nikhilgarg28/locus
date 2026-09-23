//! Acceptance tests for kernel gate K6 (the kernel contract in atlas.html): the range
//! iteration rule, with state whose type depends on the index.
//! Every term here is written by hand; nothing comes from the parser.

use std::rc::Rc;

use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Context, Definitions, KernelError, MachineInt, Mode, Op, Prelude, Proof, Term, Type,
    check_proof, infer_proof, infer_term, same,
};

fn setup() -> (Rc<Definitions>, Prelude, Theory) {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    (Rc::new(definitions), prelude, theory)
}

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn add_one(term: Term) -> Term {
    Term::op(Op::WrappingAdd, MachineInt::U8, vec![term, Term::U8(1)])
}

fn lemma(id: locus::kernel::FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

/// The state `(acc: u8, same: @[acc == i])` at index `i`.
fn counting_state(i: Term) -> Type {
    Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [acc] => Some(Type::proof(u8_eq(acc.clone(), i.clone()))),
        _ => None,
    })
}

/// One step of counting: `(acc + 1, proof that acc + 1 == i + 1)`.
fn counting_step(i: Term, s: Term) -> Term {
    let acc = Term::proj(s.clone(), 0);
    let tracked = Proof::OfTerm(Term::proj(s, 1));
    let left = add_one(acc.clone());
    Term::tuple(
        &counting_state(add_one(i)),
        vec![
            add_one(acc.clone()),
            Term::proof(Proof::transport(
                tracked,
                |hole| u8_eq(left.clone(), add_one(hole)),
                Proof::Refl(add_one(acc)),
            )),
        ],
    )
}

/// `for i in 0..n (acc: u8 = 0, same: @[acc == i] = _) { continue(acc + 1, _) }`
fn count_up(theory: &Theory, n: Term) -> Term {
    let zero = Term::U8(0);
    Term::for_range(
        zero.clone(),
        n.clone(),
        lemma(
            theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
            vec![n],
        ),
        counting_state,
        Term::tuple(
            &counting_state(zero.clone()),
            vec![zero.clone(), Term::proof(Proof::Refl(zero))],
        ),
        |i, s, _, _| counting_step(i, s),
    )
}

// --- The stated gate conditions ---------------------------------------------

#[test]
#[doc = "spec: 2.9:1, 2.9:10, 2.9:2, 2.9:4"]
fn the_bounded_count_example_checks_with_index_dependent_state() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(Rc::clone(&definitions));
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let looped = count_up(&theory, n.clone());

    // The result type is the state at the final index: (acc, @[acc == n]).
    assert_eq!(
        infer_term(&mut ctx, &looped, Mode::Executable),
        Ok(counting_state(n.clone()))
    );
    // A caller consumes the dependent result: the count equals n.
    let evidence = Proof::OfTerm(Term::proj(looped.clone(), 1));
    assert_eq!(
        check_proof(&mut ctx, &evidence, &u8_eq(Term::proj(looped, 0), n)),
        Ok(())
    );

    // As a declared math function with a dependent result:
    // math fn count_up(n: u8) -> (total: u8, same: @[total == n])
    let mut with_function = (*definitions).clone();
    let signature = Type::function(1, |params| match params {
        [] => Type::U8,
        [n] => counting_state(n.clone()),
        _ => unreachable!(),
    });
    assert!(
        with_function
            .declare_fn(&signature, |params| count_up(&theory, params[0].clone()))
            .is_ok()
    );
}

#[test]
fn the_invariant_must_be_re_established_at_the_next_index() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let zero = Term::U8(0);
    let init = Term::tuple(
        &counting_state(zero.clone()),
        vec![zero.clone(), Term::proof(Proof::Refl(zero.clone()))],
    );
    let ordered = lemma(
        theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
        vec![n.clone()],
    );

    // Returning the state unchanged proves acc == i, not acc == i + 1.
    let stuck = Term::for_range(
        zero.clone(),
        n.clone(),
        ordered.clone(),
        counting_state,
        init.clone(),
        |_, s, _, _| s,
    );
    assert!(matches!(
        infer_term(&mut ctx, &stuck, Mode::Logical),
        Err(KernelError::TypeMismatch { .. })
    ));
    // Claiming the next invariant without proving it fails in the field.
    let unproved = Term::for_range(
        zero.clone(),
        n.clone(),
        ordered.clone(),
        counting_state,
        init,
        |i, s, _, _| {
            let acc = Term::proj(s, 0);
            Term::tuple(
                &counting_state(add_one(i)),
                vec![add_one(acc.clone()), Term::proof(Proof::Refl(add_one(acc)))],
            )
        },
    );
    assert!(matches!(
        infer_term(&mut ctx, &unproved, Mode::Logical),
        Err(KernelError::ProofMismatch { .. })
    ));
    // The initial state is checked at lo: starting the count at 1 is wrong.
    let one = Term::U8(1);
    let bad_init = Term::for_range(
        zero,
        n,
        ordered,
        counting_state,
        Term::tuple(
            &counting_state(one.clone()),
            vec![one.clone(), Term::proof(Proof::Refl(one))],
        ),
        |i, s, _, _| counting_step(i, s),
    );
    assert!(matches!(
        infer_term(&mut ctx, &bad_init, Mode::Logical),
        Err(KernelError::TypeMismatch { .. })
    ));
}

#[test]
fn an_empty_range_is_its_initial_state() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let k = Term::var(ctx.declare(Type::U8).unwrap());
    let init = Term::tuple(
        &counting_state(k.clone()),
        vec![k.clone(), Term::proof(Proof::Refl(k.clone()))],
    );
    let empty = Term::for_range(
        k.clone(),
        k.clone(),
        lemma(theory.machine(MachineInt::U8).le_refl, vec![k.clone()]),
        counting_state,
        init.clone(),
        |i, s, _, _| counting_step(i, s),
    );
    assert_eq!(
        infer_term(&mut ctx, &empty, Mode::Executable),
        Ok(counting_state(k.clone()))
    );
    let step = Proof::ForEmpty(empty.clone());
    assert_eq!(
        check_proof(&mut ctx, &step, &Term::eq(counting_state(k), empty, init)),
        Ok(())
    );
    // A range that is not visibly empty has no such step.
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::ForEmpty(count_up(&theory, n))),
        Err(KernelError::NoComputationStep(_))
    ));
}

#[test]
#[doc = "spec: 2.9:5"]
fn reversed_bounds_are_rejected_for_want_of_evidence() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let (five, three) = (Term::U8(5), Term::U8(3));
    let init = Term::tuple(
        &counting_state(five.clone()),
        vec![five.clone(), Term::proof(Proof::Refl(five.clone()))],
    );
    // There is no proof of u8_le(5, 3) to give. Every fact that is at hand
    // proves some other ordering, and the rule checks which.
    for ordered in [
        lemma(theory.machine(MachineInt::U8).le_refl, vec![five.clone()]),
        lemma(theory.machine(MachineInt::U8).le_refl, vec![three.clone()]),
        lemma(
            theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
            vec![three.clone()],
        ),
    ] {
        let reversed = Term::for_range(
            five.clone(),
            three.clone(),
            ordered,
            counting_state,
            init.clone(),
            |i, s, _, _| counting_step(i, s),
        );
        assert!(matches!(
            infer_term(&mut ctx, &reversed, Mode::Logical),
            Err(KernelError::ProofMismatch { .. })
        ));
    }
}

#[test]
fn a_range_may_end_at_255_and_the_body_knows_its_bounds() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let (zero, top) = (Term::U8(0), Term::U8(255));
    let init = Term::tuple(
        &counting_state(zero.clone()),
        vec![zero.clone(), Term::proof(Proof::Refl(zero.clone()))],
    );
    let looped = Term::for_range(
        zero.clone(),
        top.clone(),
        lemma(
            theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
            vec![top.clone()],
        ),
        counting_state,
        init,
        |i, s, lower, upper| {
            // Route the state's proof through both hypotheses at their stated
            // propositions, so the kernel checks what the body was given.
            let Term::Tuple(fields, mut values) = counting_step(i.clone(), s) else {
                unreachable!()
            };
            let Some(Term::Proof(proof)) = values.pop() else {
                unreachable!()
            };
            let at_least_lo = Term::int_le(
                Term::view(MachineInt::U8, Term::U8(0)),
                Term::view(MachineInt::U8, i.clone()),
            );
            let below_hi = Term::int_lt(
                Term::view(MachineInt::U8, i),
                Term::view(MachineInt::U8, Term::U8(255)),
            );
            let guarded = Proof::implies_elim(
                Proof::implies_intro(below_hi, |_| {
                    Proof::implies_elim(Proof::implies_intro(at_least_lo, |_| *proof), lower)
                }),
                upper,
            );
            values.push(Term::proof(guarded));
            Term::Tuple(fields, values)
        },
    );
    assert_eq!(
        infer_term(&mut ctx, &looped, Mode::Executable),
        Ok(counting_state(top))
    );
}

#[test]
fn iteration_nests_and_an_inner_state_may_mention_the_outer_index() {
    // for i in 0..n (total: u8 = 0) {
    //     let inner = for j in 0..m (x: u8 = i, at: @[x == i] = _) { continue(x, at) };
    //     continue(inner.0)
    // }
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let m = Term::var(ctx.declare(Type::U8).unwrap());
    let outer_state = |_: Term| Type::Tuple(vec![Type::U8]);
    let zero = Term::U8(0);

    let nested = Term::for_range(
        zero.clone(),
        n.clone(),
        lemma(
            theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
            vec![n],
        ),
        outer_state,
        Term::tuple(&outer_state(zero.clone()), vec![zero.clone()]),
        |i, _, _, _| {
            // The inner invariant is about the outer index.
            let pinned = |outer: Term| {
                move |_: Term| {
                    let outer = outer.clone();
                    Type::tuple(move |earlier| match earlier {
                        [] => Some(Type::U8),
                        [x] => Some(Type::proof(u8_eq(x.clone(), outer.clone()))),
                        _ => None,
                    })
                }
            };
            let inner_state = pinned(i.clone());
            let inner = Term::for_range(
                Term::U8(0),
                m.clone(),
                lemma(
                    theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
                    vec![m.clone()],
                ),
                pinned(i.clone()),
                Term::tuple(
                    &inner_state(Term::U8(0)),
                    vec![i.clone(), Term::proof(Proof::Refl(i.clone()))],
                ),
                |j, s, _, _| {
                    Term::tuple(
                        &pinned(i.clone())(add_one(j)),
                        vec![
                            Term::proj(s.clone(), 0),
                            Term::proof(Proof::OfTerm(Term::proj(s, 1))),
                        ],
                    )
                },
            );
            Term::tuple(&outer_state(add_one(i.clone())), vec![Term::proj(inner, 0)])
        },
    );
    assert_eq!(
        infer_term(&mut ctx, &nested, Mode::Executable),
        Ok(Type::Tuple(vec![Type::U8]))
    );
}

// --- Ghost rules and comparison ---------------------------------------------------

#[test]
#[doc = "spec: 2.9:3"]
fn an_executable_loop_cannot_take_its_bounds_or_state_from_ghosts() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let g = Term::var(ctx.declare_ghost(Type::U8).unwrap());
    let ghost_bound = count_up(&theory, g.clone());
    assert!(matches!(
        infer_term(&mut ctx, &ghost_bound, Mode::Executable),
        Err(KernelError::GhostInExecutable(_))
    ));
    // The same loop is an ordinary logical term.
    assert!(infer_term(&mut ctx, &ghost_bound, Mode::Logical).is_ok());

    // In an executable loop the index is executable data: it may be stored.
    let n = Term::var(ctx.declare(Type::U8).unwrap());
    let state = |_: Term| Type::Tuple(vec![Type::U8]);
    let last_index = Term::for_range(
        Term::U8(0),
        n.clone(),
        lemma(
            theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
            vec![n.clone()],
        ),
        state,
        Term::tuple(&state(Term::U8(0)), vec![Term::U8(0)]),
        |i, _, _, _| Term::tuple(&state(Term::U8(0)), vec![i]),
    );
    assert!(infer_term(&mut ctx, &last_index, Mode::Executable).is_ok());
    // But a ghost cannot be stored in the executable state.
    let leaky = Term::for_range(
        Term::U8(0),
        n.clone(),
        lemma(
            theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
            vec![n],
        ),
        state,
        Term::tuple(&state(Term::U8(0)), vec![Term::U8(0)]),
        |_, _, _, _| Term::tuple(&state(Term::U8(0)), vec![g.clone()]),
    );
    assert!(matches!(
        infer_term(&mut ctx, &leaky, Mode::Executable),
        Err(KernelError::GhostInExecutable(_))
    ));
}

#[test]
#[doc = "spec: 2.9:6"]
fn the_ordering_proof_is_irrelevant_to_comparison() {
    let (definitions, _, theory) = setup();
    let mut ctx = Context::with_definitions(definitions);
    let k = Term::var(ctx.declare(Type::U8).unwrap());
    let build = |ordered: Proof| {
        Term::for_range(
            k.clone(),
            k.clone(),
            ordered,
            counting_state,
            Term::tuple(
                &counting_state(k.clone()),
                vec![k.clone(), Term::proof(Proof::Refl(k.clone()))],
            ),
            |i, s, _, _| counting_step(i, s),
        )
    };
    let direct = lemma(theory.machine(MachineInt::U8).le_refl, vec![k.clone()]);
    let via_transitivity = lemma(
        theory.machine(MachineInt::U8).le_trans,
        vec![
            k.clone(),
            k.clone(),
            k.clone(),
            Term::proof(direct.clone()),
            Term::proof(direct.clone()),
        ],
    );
    let (first, second) = (build(direct), build(via_transitivity));
    assert!(infer_term(&mut ctx, &second, Mode::Logical).is_ok());
    assert_ne!(first, second);
    assert!(same(&first, &second));
}
