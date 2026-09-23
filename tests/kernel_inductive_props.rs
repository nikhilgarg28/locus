//! D5: positive named predicates and induction over their proof trees.
use locus::kernel::{
    Axiom, Context, Definitions, KernelError, Prelude, Proof, PropId, PropVariant, Term, TermArm,
    Type, check_proof, infer_proof,
};
use std::rc::Rc;

fn and(prelude: Prelude, left: Term, right: Term, a: Proof, b: Proof) -> Proof {
    Proof::Construct {
        prop: prelude.and,
        variant: 0,
        params: vec![left, right],
        payload: vec![Term::proof(a), Term::proof(b)],
    }
}
fn declare_reachable(defs: &mut Definitions, prelude: Prelude) -> PropId {
    defs.declare_inductive_prop(vec![Type::Int, Type::Int], |id| {
        vec![
            PropVariant::arm(Type::Tuple(vec![Type::Int, Type::Int]), |p| {
                Term::eq(Type::Int, p[0].clone(), p[1].clone())
            }),
            PropVariant::arm(Type::Tuple(vec![Type::Int, Type::Int, Type::Int]), |p| {
                prelude.and_prop(
                    Term::int_le(p[0].clone(), p[2].clone()),
                    Term::PropApp(id, vec![p[2].clone(), p[1].clone()]),
                )
            }),
        ]
    })
    .unwrap()
}
fn ordered_steps(from: Term, middle: Term, to: Term, strengthened: Proof) -> Proof {
    let goal = Term::int_le(from.clone(), to.clone());
    Proof::CaseProof {
        scrutinee: Box::new(strengthened),
        goal: goal.clone(),
        arms: vec![Proof::arm(2, 0, |outer, _| Proof::CaseProof {
            scrutinee: Box::new(Proof::OfTerm(outer[1].clone())),
            goal: goal.clone(),
            arms: vec![Proof::arm(2, 0, |inner, _| {
                Proof::implies_elim(
                    Proof::implies_elim(
                        Proof::Axiom(Axiom::IntLeTrans(from.clone(), middle.clone(), to.clone())),
                        Proof::OfTerm(outer[0].clone()),
                    ),
                    Proof::OfTerm(inner[1].clone()),
                )
            })],
        })],
    }
}
fn reachability_induction(proof: Proof) -> Proof {
    Proof::PropInduction {
        scrutinee: Box::new(proof),
        motive: TermArm {
            binders: 2,
            body: Term::int_le(Term::Bound(1), Term::Bound(0)),
        },
        arms: vec![
            Proof::arm(3, 1, |p, h| {
                Proof::transport(
                    h[0].clone(),
                    |value| Term::int_le(p[0].clone(), value),
                    Proof::Axiom(Axiom::IntLeRefl(p[0].clone())),
                )
            }),
            Proof::arm(4, 1, |p, h| {
                ordered_steps(p[0].clone(), p[2].clone(), p[1].clone(), h[0].clone())
            }),
        ],
    }
}

#[test]
#[doc = "spec: 2.30:2, 2.30:3"]
fn reachable_is_constructed_and_its_ordering_proved_by_induction() {
    let (mut defs, prelude) = Definitions::with_prelude();
    let reachable = declare_reachable(&mut defs, prelude);
    let same = Proof::Construct {
        prop: reachable,
        variant: 0,
        params: vec![Term::int(1), Term::int(1)],
        payload: vec![Term::proof(Proof::Refl(Term::int(1)))],
    };
    let next = Proof::Construct {
        prop: reachable,
        variant: 1,
        params: vec![Term::int(0), Term::int(1)],
        payload: vec![
            Term::int(1),
            Term::proof(and(
                prelude,
                Term::int_le(Term::int(0), Term::int(1)),
                Term::PropApp(reachable, vec![Term::int(1), Term::int(1)]),
                Proof::Evaluate(Term::int_le(Term::int(0), Term::int(1))),
                same,
            )),
        ],
    };
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let proof = reachability_induction(next);
    assert_eq!(
        check_proof(&mut ctx, &proof, &Term::int_le(Term::int(0), Term::int(1))),
        Ok(())
    );
    let Proof::PropInduction {
        scrutinee,
        motive,
        arms,
    } = proof
    else {
        unreachable!()
    };
    let mut wrong_motive = motive.clone();
    wrong_motive.binders = 1;
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::PropInduction {
                scrutinee: scrutinee.clone(),
                motive: wrong_motive,
                arms: arms.clone()
            }
        )
        .is_err()
    );
    let mut wrong_arms = arms.clone();
    wrong_arms[1].hyps = 0;
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::PropInduction {
                scrutinee: scrutinee.clone(),
                motive: motive.clone(),
                arms: wrong_arms
            }
        )
        .is_err()
    );
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::PropInduction {
                scrutinee,
                motive: TermArm {
                    binders: 2,
                    body: Term::int_le(Term::Bound(0), Term::Bound(1))
                },
                arms
            }
        )
        .is_err()
    );
}

#[test]
#[doc = "spec: 2.27:4, 2.30:1"]
fn recursion_under_negation_or_an_unknown_helper_is_rejected() {
    let (mut defs, prelude) = Definitions::with_prelude();
    let bad = defs.declare_inductive_prop(vec![Type::Int], |id| {
        vec![PropVariant::arm(Type::Tuple(vec![Type::Int]), |p| {
            prelude.not_prop(Term::PropApp(id, p.to_vec()))
        })]
    });
    assert!(matches!(bad, Err(KernelError::InvalidRecursion(_))));
    let helper = defs
        .declare_fn(&Type::Fn(vec![Type::Prop], Box::new(Type::Prop)), |p| {
            p[0].clone()
        })
        .unwrap();
    let hidden = defs.declare_inductive_prop(vec![Type::Int], |id| {
        vec![PropVariant::arm(Type::Tuple(vec![Type::Int]), |p| {
            Term::call(Term::Fn(helper), vec![Term::PropApp(id, p.to_vec())])
        })]
    });
    assert!(matches!(hidden, Err(KernelError::InvalidRecursion(_))));
    let witness = defs.declare_inductive_prop(vec![], |id| {
        vec![PropVariant::arm(
            Type::Tuple(vec![Type::proof(Term::PropApp(id, vec![]))]),
            |_| prelude.truth_prop(),
        )]
    });
    assert!(matches!(witness, Err(KernelError::InvalidRecursion(_))));
    // Refused declarations never publish their temporary self-name.
    assert!(
        defs.declare_inductive_prop(vec![], |_| vec![PropVariant::arm(
            Type::Tuple(vec![]),
            |_| prelude.truth_prop()
        )])
        .is_ok()
    );
}

#[test]
#[doc = "spec: 2.30:4"]
fn quantified_positive_recursion_keeps_induction_witnesses_scoped() {
    let (mut defs, prelude) = Definitions::with_prelude();
    let reachable = defs
        .declare_inductive_prop(vec![Type::Int, Type::Int], |id| {
            vec![
                PropVariant::arm(Type::Tuple(vec![Type::Int, Type::Int]), |p| {
                    Term::eq(Type::Int, p[0].clone(), p[1].clone())
                }),
                PropVariant::arm(Type::Tuple(vec![Type::Int, Type::Int]), |p| {
                    Term::exists(Type::Int, |middle| {
                        prelude.and_prop(
                            Term::int_le(p[0].clone(), middle.clone()),
                            Term::PropApp(id, vec![middle, p[1].clone()]),
                        )
                    })
                }),
            ]
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let from = Term::var(ctx.declare(Type::Int).unwrap());
    let to = Term::var(ctx.declare(Type::Int).unwrap());
    let h = ctx
        .assume(Term::PropApp(reachable, vec![from.clone(), to.clone()]))
        .unwrap();
    let proof = Proof::PropInduction {
        scrutinee: Box::new(Proof::hyp(h)),
        motive: TermArm {
            binders: 2,
            body: Term::int_le(Term::Bound(1), Term::Bound(0)),
        },
        arms: vec![
            Proof::arm(3, 1, |p, h| {
                Proof::transport(
                    h[0].clone(),
                    |value| Term::int_le(p[0].clone(), value),
                    Proof::Axiom(Axiom::IntLeRefl(p[0].clone())),
                )
            }),
            Proof::arm(3, 1, |p, h| Proof::ExistsElim {
                exists: Box::new(h[0].clone()),
                goal: Term::int_le(p[0].clone(), p[1].clone()),
                arm: Proof::arm(1, 1, |w, facts| {
                    ordered_steps(p[0].clone(), w[0].clone(), p[1].clone(), facts[0].clone())
                }),
            }),
        ],
    };
    assert_eq!(
        check_proof(&mut ctx, &proof, &Term::int_le(from, to)),
        Ok(())
    );
}

#[test]
#[doc = "spec: 2.30:5"]
fn structural_proof_recursion_requires_an_actual_matched_subproof() {
    let (mut defs, prelude) = Definitions::with_prelude();
    let reachable = declare_reachable(&mut defs, prelude);
    let signature = Type::Fn(
        vec![
            Type::Int,
            Type::Int,
            Type::proof(Term::PropApp(
                reachable,
                vec![Term::Bound(1), Term::Bound(0)],
            )),
        ],
        Box::new(Type::proof(Term::int_le(Term::Bound(2), Term::Bound(1)))),
    );
    assert!(matches!(
        defs.declare_structural_fn(&signature, 2, |id, args| {
            Term::proof(Proof::OfTerm(Term::call(
                Term::Fn(id),
                vec![
                    args[0].clone(),
                    args[1].clone(),
                    Term::proof(Proof::OfTerm(args[2].clone())),
                ],
            )))
        }),
        Err(KernelError::InvalidRecursion(_))
    ));
    let ordered = defs
        .declare_structural_fn(&signature, 2, |id, p| {
            let goal = Term::int_le(p[0].clone(), p[1].clone());
            Term::proof(Proof::CaseProof {
                scrutinee: Box::new(Proof::OfTerm(p[2].clone())),
                goal: goal.clone(),
                arms: vec![
                    Proof::arm(1, 0, |q, _| {
                        Proof::linear(goal.clone(), 1, vec![(Proof::OfTerm(q[0].clone()), 1)])
                    }),
                    Proof::arm(2, 0, |q, _| Proof::CaseProof {
                        scrutinee: Box::new(Proof::OfTerm(q[1].clone())),
                        goal: goal.clone(),
                        arms: vec![Proof::arm(2, 0, |r, _| {
                            Proof::linear(
                                goal.clone(),
                                1,
                                vec![
                                    (Proof::OfTerm(r[0].clone()), 1),
                                    (
                                        Proof::OfTerm(Term::call(
                                            Term::Fn(id),
                                            vec![
                                                q[0].clone(),
                                                p[1].clone(),
                                                Term::proof(Proof::OfTerm(r[1].clone())),
                                            ],
                                        )),
                                        1,
                                    ),
                                ],
                            )
                        })],
                    }),
                ],
            })
        })
        .unwrap();
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let start = Proof::Construct {
        prop: reachable,
        variant: 0,
        params: vec![Term::int(3), Term::int(3)],
        payload: vec![Term::proof(Proof::Refl(Term::int(3)))],
    };
    assert!(
        check_proof(
            &mut ctx,
            &Proof::OfTerm(Term::call(
                Term::Fn(ordered),
                vec![Term::int(3), Term::int(3), Term::proof(start)]
            )),
            &Term::int_le(Term::int(3), Term::int(3))
        )
        .is_ok()
    );
}
