//! D4: checked finite data, structural recursion, and induction.
use locus::kernel::derive::symm_at;
use locus::kernel::{
    Context, Definitions, EnumId, FnId, KernelError, Proof, Term, Type, check_proof, infer_proof,
};
use std::rc::Rc;

fn list(defs: &mut Definitions) -> EnumId {
    defs.declare_logical_enum_group(1, |ids| {
        vec![vec![
            Type::Tuple(vec![]),
            Type::Tuple(vec![Type::Int, Type::Enum(ids[0])]),
        ]]
    })
    .unwrap()[0]
}
fn nil(id: EnumId) -> Term {
    Term::Variant(id, 0, vec![])
}
fn cons(id: EnumId, head: Term, tail: Term) -> Term {
    Term::Variant(id, 1, vec![head, tail])
}
fn call(id: FnId, value: Term) -> Term {
    Term::call(Term::Fn(id), vec![value])
}
fn length_body(id: FnId, xs: Term) -> Term {
    Term::case(
        xs,
        Type::Int,
        vec![
            (0, Box::new(|_, _| Term::int(0))),
            (
                2,
                Box::new(move |fields, _| Term::int_add(Term::int(1), call(id, fields[1].clone()))),
            ),
        ],
    )
}
fn length(defs: &mut Definitions, xs: EnumId) -> FnId {
    defs.declare_structural_fn(
        &Type::Fn(vec![Type::Enum(xs)], Box::new(Type::Int)),
        0,
        |id, p| length_body(id, p[0].clone()),
    )
    .unwrap()
}

#[test]
#[doc = "spec: 2.29:2, 2.29:4"]
fn finite_lists_evaluate_structural_length_and_reject_non_descent() {
    let (mut defs, _) = Definitions::with_prelude();
    let seq = list(&mut defs);
    let len = length(&mut defs, seq);
    let values = cons(seq, Term::int(2), cons(seq, Term::int(3), nil(seq)));
    let mut ctx = Context::with_definitions(Rc::new(defs.clone()));
    assert_eq!(
        check_proof(
            &mut ctx,
            &Proof::Evaluate(call(len, values.clone())),
            &Term::eq(Type::Int, call(len, values.clone()), Term::int(2))
        ),
        Ok(())
    );
    // A recursive data value itself is plain data; walking its declaration
    // must terminate even though the declaration graph has a cycle.
    assert!(infer_proof(&mut ctx, &Proof::Evaluate(values)).is_ok());
    let signature = Type::Fn(vec![Type::Enum(seq)], Box::new(Type::Int));
    assert!(matches!(
        defs.declare_structural_fn(&signature, 0, |id, p| call(id, p[0].clone())),
        Err(KernelError::InvalidRecursion(_))
    ));
    assert!(matches!(
        defs.declare_structural_fn(&signature, 0, |id, p| Term::case(
            p[0].clone(),
            Type::Int,
            vec![
                (0, Box::new(|_, _| Term::int(0))),
                (
                    2,
                    Box::new(move |fields, _| call(
                        id,
                        cons(seq, fields[0].clone(), fields[1].clone())
                    ))
                )
            ]
        )),
        Err(KernelError::InvalidRecursion(_))
    ));
    // Failure leaves no partially accepted declaration behind.
    let another = length(&mut defs, seq);
    assert_ne!(len, another);
}

#[test]
#[doc = "spec: 2.29:1, 2.39:2"]
fn logical_enum_groups_enforce_positive_fields_and_support_mutual_data() {
    let mut defs = Definitions::new();
    assert!(matches!(
        defs.declare_logical_enum_group(1, |ids| vec![vec![Type::Tuple(vec![Type::Fn(
            vec![Type::Enum(ids[0])],
            Box::new(Type::Int)
        )])]]),
        Err(KernelError::InvalidRecursion(_))
    ));
    assert_eq!(
        defs.declare_logical_enum_group(1, |_| vec![vec![Type::Tuple(vec![Type::U8])]]),
        Err(KernelError::NotLogicalType(Type::U8))
    );
    let ids = defs
        .declare_logical_enum_group(2, |ids| {
            vec![
                vec![Type::Tuple(vec![]), Type::Tuple(vec![Type::Enum(ids[1])])],
                vec![Type::Tuple(vec![Type::Enum(ids[0])])],
            ]
        })
        .unwrap();
    assert_eq!(defs.logical_enum_group(ids[0]), Some(ids.clone()));
    assert_eq!(defs.logical_enum_group(ids[1]), Some(ids));
}

fn nonnegative_length(seq: EnumId, len: FnId, target: Term) -> Proof {
    let claim = |value: Term| Term::int_le(Term::int(0), call(len, value));
    Proof::DataInduction {
        target,
        motives: vec![(seq, claim(Term::Bound(0)))],
        arms: vec![
            Proof::arm(0, 0, |_, _| Proof::Evaluate(claim(nil(seq)))),
            Proof::arm(2, 1, |fields, hypotheses| {
                let value = cons(seq, fields[0].clone(), fields[1].clone());
                let invocation = call(len, value.clone());
                let unfolded = length_body(len, value);
                let rhs = Term::int_add(Term::int(1), call(len, fields[1].clone()));
                let equation = Proof::transport(
                    Proof::CaseStep(unfolded),
                    |hole| Term::eq(Type::Int, invocation.clone(), hole),
                    Proof::Definition(invocation.clone()),
                );
                let lower = Proof::linear(
                    Term::int_le(Term::int(0), rhs),
                    1,
                    vec![(hypotheses[0].clone(), 1)],
                );
                Proof::transport(
                    symm_at(&Type::Int, &invocation, equation),
                    |hole| Term::int_le(Term::int(0), hole),
                    lower,
                )
            }),
        ],
    }
}

#[test]
#[doc = "spec: 2.29:3"]
fn induction_proves_a_length_lemma_and_checks_each_hypothesis_and_motive() {
    let (mut defs, _) = Definitions::with_prelude();
    let seq = list(&mut defs);
    let len = length(&mut defs, seq);
    let mut ctx = Context::with_definitions(Rc::new(defs));
    let xs = Term::var(ctx.declare_ghost(Type::Enum(seq)).unwrap());
    let proof = nonnegative_length(seq, len, xs.clone());
    let claim = Term::int_le(Term::int(0), call(len, xs));
    assert_eq!(check_proof(&mut ctx, &proof, &claim), Ok(()));
    let Proof::DataInduction {
        target,
        motives,
        arms,
    } = proof
    else {
        unreachable!()
    };
    let mut short = arms.clone();
    short[1].hyps = 0;
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::DataInduction {
                target: target.clone(),
                motives: motives.clone(),
                arms: short
            }
        )
        .is_err()
    );
    let mut swapped = arms.clone();
    swapped.reverse();
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::DataInduction {
                target: target.clone(),
                motives: motives.clone(),
                arms: swapped
            }
        )
        .is_err()
    );
    let false_motive = Term::int_le(Term::int(1), call(len, Term::Bound(0)));
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::DataInduction {
                target: target.clone(),
                motives: vec![(seq, false_motive)],
                arms: arms.clone()
            }
        )
        .is_err()
    );
    assert!(
        infer_proof(
            &mut ctx,
            &Proof::DataInduction {
                target,
                motives: vec![],
                arms
            }
        )
        .is_err()
    );
}

#[test]
fn a_recursive_field_cannot_be_eliminated_using_an_empty_placeholder_schema() {
    let mut defs = Definitions::new();
    let result = defs.declare_logical_enum_group(1, |ids| {
        let field = locus::kernel::VarId::fresh();
        let impossible = Term::case(Term::Free(field), Type::Prop, vec![]);
        vec![vec![
            Type::Tuple(vec![]),
            Type::tuple_over(&[
                (field, Type::Enum(ids[0])),
                (
                    locus::kernel::VarId::fresh(),
                    Type::Proof(Box::new(impossible)),
                ),
            ]),
        ]]
    });
    assert!(
        result.is_err(),
        "a real two-variant enum must not admit an empty case"
    );
    // The rejected candidate has not reserved a name or an enum identity.
    assert!(
        defs.declare_logical_enum_group(1, |_| vec![vec![Type::Tuple(vec![])]])
            .is_ok()
    );
}
