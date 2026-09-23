//! Explicit checked guards justify recursion over the nonnegative integers.
use locus::kernel::{
    Axiom, CmpOp, Context, Definitions, KernelError, Proof, Term, Type, check_proof,
};
use std::rc::Rc;

fn claim(next: Term, current: Term, defs: &Definitions) -> Term {
    defs.decrease_claim(next, current).unwrap()
}

#[test]
#[doc = "spec: 2.32:1, 2.32:2"]
fn guard_checks_the_actual_measure_and_cannot_justify_non_decrease() {
    let (mut defs, prelude) = Definitions::with_prelude();
    let signature = Type::Fn(vec![Type::Int], Box::new(Type::Int));
    let copy = defs.clone();
    let count = defs
        .declare_measured_fn(&signature, 0, |id, p| {
            let n = p[0].clone();
            let positive = Term::int_cmp(CmpOp::Lt, Term::int(0), n.clone());
            Term::case(
                positive.clone(),
                Type::Int,
                vec![
                    (0, Box::new(|_, _| Term::int(0))),
                    (
                        0,
                        Box::new(move |_, h| {
                            let next = Term::int_add(n.clone(), Term::int(-1));
                            let nonnegative = Term::int_le(Term::int(0), next.clone());
                            let smaller = Term::int_lt(next.clone(), n.clone());
                            let positive_fact = Proof::implies_elim(
                                Proof::Axiom(Axiom::CmpReflect(positive.clone(), true)),
                                h.clone(),
                            );
                            let left_test = Term::int_cmp(CmpOp::Le, Term::int(0), next.clone());
                            let right_test = Term::int_cmp(CmpOp::Lt, next.clone(), n.clone());
                            let left = Proof::implies_elim(
                                Proof::Axiom(Axiom::CmpReify(left_test.clone(), true)),
                                Proof::linear(nonnegative, 1, vec![(positive_fact, 1)]),
                            );
                            let right = Proof::implies_elim(
                                Proof::Axiom(Axiom::CmpReify(right_test.clone(), true)),
                                Proof::linear(smaller, 1, vec![]),
                            );
                            let evidence = Proof::Construct {
                                prop: prelude.and,
                                variant: 0,
                                params: vec![Term::holds(left_test), Term::holds(right_test)],
                                payload: vec![Term::proof(left), Term::proof(right)],
                            };
                            let guard = claim(next.clone(), n.clone(), &copy);
                            let called = Term::call(Term::Fn(id), vec![next]);
                            let guarded = Term::proj(
                                Term::Tuple(
                                    vec![Type::proof(guard), Type::Int],
                                    vec![Term::proof(evidence), called],
                                ),
                                1,
                            );
                            Term::int_add(Term::int(1), guarded)
                        }),
                    ),
                ],
            )
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs.clone()));
    for n in [-2, 0, 1, 7] {
        let call = Term::call(Term::Fn(count), vec![Term::int(n)]);
        assert_eq!(
            check_proof(
                &mut ctx,
                &Proof::Evaluate(call.clone()),
                &Term::eq(Type::Int, call, Term::int(n.max(0)))
            ),
            Ok(())
        );
    }
    assert!(matches!(
        defs.declare_measured_fn(&signature, 0, |id, p| Term::call(Term::Fn(id), p.to_vec())),
        Err(KernelError::InvalidRecursion(_))
    ));
    // The evidence is true, but it does not establish strict descent.
    assert!(matches!(
        defs.declare_measured_fn(&signature, 0, |id, p| {
            let same = Term::eq(Type::Int, p[0].clone(), p[0].clone());
            Term::proj(
                Term::Tuple(
                    vec![Type::proof(same), Type::Int],
                    vec![
                        Term::proof(Proof::Refl(p[0].clone())),
                        Term::call(Term::Fn(id), p.to_vec()),
                    ],
                ),
                1,
            )
        }),
        Err(KernelError::InvalidRecursion(_))
    ));
}

#[test]
#[doc = "spec: 2.33:1"]
fn known_case_reduction_checks_scrutinee_constructor_payload_and_result() {
    let (mut defs, _) = Definitions::with_prelude();
    let choice = defs.declare_enum(&[Type::Tuple(vec![Type::Int])]).unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let b = Term::Free(ctx.declare(Type::Bool).unwrap());
    let other = Term::Free(ctx.declare(Type::Bool).unwrap());
    let fact = ctx
        .assume(Term::eq(Type::Bool, b.clone(), Term::Bool(true)))
        .unwrap();
    let wrong = ctx
        .assume(Term::eq(Type::Bool, other, Term::Bool(true)))
        .unwrap();
    let case = Term::case(
        b.clone(),
        Type::Int,
        vec![
            (0, Box::new(|_, _| Term::int(0))),
            (0, Box::new(|_, _| Term::int(1))),
        ],
    );
    let step = Proof::CaseKnown {
        term: case.clone(),
        equation: Box::new(Proof::hyp(fact)),
    };
    assert_eq!(
        check_proof(
            &mut ctx,
            &step,
            &Term::eq(Type::Int, case.clone(), Term::int(1))
        ),
        Ok(())
    );
    assert!(
        check_proof(
            &mut ctx,
            &step,
            &Term::eq(Type::Int, case.clone(), Term::int(0))
        )
        .is_err()
    );
    let bad = Proof::CaseKnown {
        term: case.clone(),
        equation: Box::new(Proof::hyp(wrong)),
    };
    assert!(
        check_proof(
            &mut ctx,
            &bad,
            &Term::eq(Type::Int, case.clone(), Term::int(1))
        )
        .is_err()
    );
    let bad = Proof::CaseKnown {
        term: case.clone(),
        equation: Box::new(Proof::Refl(Term::int(1))),
    };
    assert!(check_proof(&mut ctx, &bad, &Term::eq(Type::Int, case, Term::int(1))).is_err());
    let object = Term::Free(ctx.declare(Type::Enum(choice)).unwrap());
    let constructor = Term::Variant(choice, 0, vec![Term::int(7)]);
    let equation = ctx
        .assume(Term::eq(Type::Enum(choice), object.clone(), constructor))
        .unwrap();
    let projected = Term::case(
        object.clone(),
        Type::Int,
        vec![(1, Box::new(|v, _| v[0].clone()))],
    );
    let step = Proof::CaseKnown {
        term: projected.clone(),
        equation: Box::new(Proof::hyp(equation)),
    };
    assert_eq!(
        check_proof(
            &mut ctx,
            &step,
            &Term::eq(Type::Int, projected.clone(), Term::int(7))
        ),
        Ok(())
    );
    assert!(
        check_proof(
            &mut ctx,
            &step,
            &Term::eq(Type::Int, projected, Term::int(8))
        )
        .is_err()
    );
    assert!(
        ctx.assume(Term::eq(
            Type::Enum(choice),
            object.clone(),
            Term::Variant(choice, 0, vec![Term::Bool(true)])
        ))
        .is_err()
    );
    assert!(
        ctx.assume(Term::eq(
            Type::Enum(choice),
            object.clone(),
            Term::Variant(choice, 1, vec![])
        ))
        .is_err()
    );
    let ill_typed = Term::case(
        object,
        Type::Int,
        vec![(1, Box::new(|_, _| Term::Bool(true)))],
    );
    let bad = Proof::CaseKnown {
        term: ill_typed.clone(),
        equation: Box::new(Proof::hyp(equation)),
    };
    assert!(
        check_proof(
            &mut ctx,
            &bad,
            &Term::eq(Type::Int, ill_typed, Term::int(7))
        )
        .is_err()
    );
}
