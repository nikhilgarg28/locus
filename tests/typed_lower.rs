//! The typed tree and `lower` (docs/ir-architecture.md): the specification's
//! programs written in source shape, lowered, and checked.

mod common;

use common::*;
use locus::exec::ExecError;
use locus::kernel::{Axiom, HypId, KernelError, Prelude, Prim, Proof, Term, Type, VarId};
use locus::typed::{
    Binder, Block, CompareOp, EnumItem, Expr, FnItem, FnRef, LowerError, MatchArm, VariantItem,
};

#[test]
fn a_nested_call_is_named_by_the_tree_and_sequenced_by_lowering() {
    let (mut session, _, _) = setup();
    let increment_id = exec_id(session.declare_fn(&increment(false)).unwrap());

    // fn twice(n: u8) -> (out: u8, @[out == n + 1 + 1]) {
    //     let second = increment(increment(n).0);
    //     (second.0, _)
    // }
    let twice = |honest: bool| {
        let n = Binder::new("n", Type::U8);
        let (first, second_call) = (VarId::fresh(), VarId::fresh());
        let n_term = n.term();
        let result = data_with_evidence(move |out| u8_eq(out, add_one(add_one(n_term.clone()))));
        let inner =
            |n_term: Term| data_with_evidence(move |out| u8_eq(out, add_one(n_term.clone())));
        let first_term = Term::var(first);
        let second = Binder::new("second", inner(Term::proj(first_term.clone(), 0)));
        // second.1 : second.0 == first.0 + 1, and first.1 : first.0 == n + 1.
        let left = Term::proj(second.term(), 0);
        let evidence = if honest {
            Proof::transport(
                Proof::OfTerm(Term::proj(first_term, 1)),
                |hole| u8_eq(left.clone(), add_one(hole)),
                Proof::OfTerm(Term::proj(second.term(), 1)),
            )
        } else {
            Proof::Refl(left.clone())
        };
        FnItem {
            name: "twice".into(),
            math: false,
            params: vec![n.clone()],
            result: result.clone(),
            body: block(
                vec![let_(
                    &second,
                    HypId::fresh(),
                    Expr::CallFn {
                        id: increment_id,
                        name: "increment".into(),
                        arguments: vec![field(
                            Expr::CallFn {
                                id: increment_id,
                                name: "increment".into(),
                                arguments: vec![Expr::var(&n)],
                                result: first,
                                ty: inner(n.term()),
                            },
                            0,
                        )],
                        result: second_call,
                        ty: second.ty.clone(),
                    },
                )],
                Expr::Tuple {
                    ty: result,
                    fields: vec![field(Expr::var(&second), 0), Expr::Proof(evidence)],
                },
            ),
        }
    };
    assert!(session.declare_fn(&twice(true)).is_ok());
    assert!(matches!(
        session.declare_fn(&twice(false)),
        Err(LowerError::Exec(ExecError::Kernel(
            KernelError::ProofMismatch { .. }
        )))
    ));
}

#[test]
fn one_tree_lowers_as_an_ordinary_function_and_as_a_math_function() {
    let (mut session, _, _) = setup();
    assert!(matches!(
        session.declare_fn(&preserve(false, true)),
        Ok(FnRef::Exec(_))
    ));
    assert!(matches!(
        session.declare_fn(&preserve(true, true)),
        Ok(FnRef::Math(_))
    ));
    // Either way the branch must use its fact.
    assert!(session.declare_fn(&preserve(false, false)).is_err());
    assert!(session.declare_fn(&preserve(true, false)).is_err());
    // The math version is a kernel function the logic can compute with.
    let FnRef::Math(id) = session.declare_fn(&preserve(true, true)).unwrap() else {
        panic!()
    };
    assert!(session.program().definitions().is_executable(id));
}

#[test]
fn the_lets_of_a_math_function_become_substitution() {
    // math fn add_two(n: u8) -> (out: u8, @[out == n + 1 + 1]) {
    //     let a = n.wrapping_add(1);
    //     let b = a.wrapping_add(1);
    //     (b, _)        // b == a + 1 and a == n + 1
    // }
    let (mut session, _, _) = setup();
    let n = Binder::new("n", Type::U8);
    let a = Binder::new("a", Type::U8);
    let b = Binder::new("b", Type::U8);
    let (a_is, b_is) = (HypId::fresh(), HypId::fresh());
    let n_term = n.term();
    let result = data_with_evidence(move |out| u8_eq(out, add_one(add_one(n_term.clone()))));
    let b_term = b.term();
    let evidence = Proof::transport(
        Proof::hyp(a_is),
        |hole| u8_eq(b_term.clone(), add_one(hole)),
        Proof::hyp(b_is),
    );
    let item = FnItem {
        name: "add_two".into(),
        math: true,
        params: vec![n.clone()],
        result: result.clone(),
        body: block(
            vec![
                let_(&a, a_is, plus_one(Expr::var(&n))),
                let_(&b, b_is, plus_one(Expr::var(&a))),
            ],
            Expr::Tuple {
                ty: result,
                fields: vec![Expr::var(&b), Expr::Proof(evidence)],
            },
        ),
    };
    assert!(matches!(session.declare_fn(&item), Ok(FnRef::Math(_))));
}

#[test]
fn a_math_function_must_be_pure() {
    let (mut session, _, _) = setup();
    let increment_id = exec_id(session.declare_fn(&increment(false)).unwrap());
    let n = Binder::new("n", Type::U8);
    let calls_out = FnItem {
        name: "calls_out".into(),
        math: true,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![],
            field(
                Expr::CallFn {
                    id: increment_id,
                    name: "increment".into(),
                    arguments: vec![Expr::var(&n)],
                    result: VarId::fresh(),
                    ty: increment(false).result,
                },
                0,
            ),
        ),
    };
    assert_eq!(
        session.declare_fn(&calls_out),
        Err(LowerError::ImpureInMath("calls_out".into()))
    );
    // A let whose annotation is not what the value proves is caught.
    let h = Binder::new("h", Type::proof(u8_eq(Term::U8(1), Term::U8(2))));
    let mislabelled = FnItem {
        name: "mislabelled".into(),
        math: false,
        params: vec![],
        result: Type::U8,
        body: block(
            vec![let_(
                &h,
                HypId::fresh(),
                Expr::Proof(Proof::Refl(Term::U8(1))),
            )],
            Expr::U8(0),
        ),
    };
    assert!(matches!(
        session.declare_fn(&mislabelled),
        Err(LowerError::Exec(ExecError::Kernel(
            KernelError::TypeMismatch { .. }
        )))
    ));
}

#[test]
fn an_enum_a_match_and_negated_conditions() {
    // enum Classified { Zero(value: u8, @[value == 0]), NonZero(value: u8, @[value != 0]) }
    let (mut session, prelude, _) = setup();
    let payload = |claim: fn(&Prelude, Term) -> Term| {
        let value = Binder::new("value", Type::U8);
        let evidence = Binder::new("evidence", Type::proof(claim(&prelude, value.term())));
        vec![value, evidence]
    };
    let classified = session
        .declare_enum(&EnumItem {
            name: "Classified".into(),
            variants: vec![
                VariantItem {
                    name: "Zero".into(),
                    payload: payload(|_, value| u8_eq(value, Term::U8(0))),
                },
                VariantItem {
                    name: "NonZero".into(),
                    payload: payload(|prelude, value| prelude.not_prop(u8_eq(value, Term::U8(0)))),
                },
            ],
        })
        .unwrap();

    // fn classify(n: u8) -> Classified {
    //     if n != 0 { Classified::NonZero(n, _) } else { Classified::Zero(n, _) }
    // }
    // The condition is a negation, so the then branch is the one where the
    // comparison n == 0 came out false.
    let n = Binder::new("n", Type::U8);
    let comparison = Term::prim(Prim::U8Eq, vec![n.term(), Term::U8(0)]);
    let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
    let reflect = |flag: bool, fact: HypId| {
        Proof::implies_elim(
            Proof::Axiom(Axiom::Reflect(comparison.clone(), flag)),
            Proof::hyp(fact),
        )
    };
    let variant = |index: usize, name: &str, evidence: Proof| Expr::Variant {
        id: classified,
        enum_name: "Classified".into(),
        index,
        variant_name: name.into(),
        payload: vec![Expr::var(&n), Expr::Proof(evidence)],
    };
    let classify = FnItem {
        name: "classify".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::Enum(classified),
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::If {
                condition: Box::new(Expr::Compare {
                    op: CompareOp::Ne,
                    left: Box::new(Expr::var(&n)),
                    right: Box::new(Expr::U8(0)),
                }),
                then_fact,
                else_fact,
                then_block: block(vec![], variant(1, "NonZero", reflect(false, then_fact))),
                else_block: block(vec![], variant(0, "Zero", reflect(true, else_fact))),
                ty: Type::Enum(classified),
                result: VarId::fresh(),
            })),
        },
    };
    let classify_id = exec_id(session.declare_fn(&classify).unwrap());

    // fn zero_or_self(m: u8) -> u8 {
    //     match classify(m) { Classified::Zero(v, h) => v, Classified::NonZero(v, h) => v }
    // }
    let m = Binder::new("m", Type::U8);
    let arm = |name: &str, claim: fn(&Prelude, Term) -> Term| {
        let v = Binder::new("v", Type::U8);
        let h = Binder::new("h", Type::proof(claim(&prelude, v.term())));
        MatchArm {
            variant_name: name.into(),
            payload: vec![v.clone(), h],
            fact: HypId::fresh(),
            body: block(vec![], Expr::var(&v)),
        }
    };
    let caller = FnItem {
        name: "zero_or_self".into(),
        math: false,
        params: vec![m.clone()],
        result: Type::U8,
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::Match {
                scrutinee: Box::new(Expr::CallFn {
                    id: classify_id,
                    name: "classify".into(),
                    arguments: vec![Expr::var(&m)],
                    result: VarId::fresh(),
                    ty: Type::Enum(classified),
                }),
                arms: vec![
                    arm("Zero", |_, v| u8_eq(v, Term::U8(0))),
                    arm("NonZero", |prelude, v| {
                        prelude.not_prop(u8_eq(v, Term::U8(0)))
                    }),
                ],
                ty: Type::U8,
                result: VarId::fresh(),
            })),
        },
    };
    assert!(session.declare_fn(&caller).is_ok());
}

#[test]
fn a_loop_in_source_shape_carries_its_invariant() {
    let (mut session, prelude, theory) = setup();
    assert_eq!(
        session
            .declare_fn(&bounded_walk(prelude, theory, true))
            .map(|_| ()),
        Ok(())
    );
    assert!(matches!(
        session.declare_fn(&bounded_walk(prelude, theory, false)),
        Err(LowerError::Exec(ExecError::Kernel(
            KernelError::ProofMismatch { .. }
        )))
    ));
}

#[test]
fn a_bounded_for_lowers_to_a_kernel_term_or_to_a_statement() {
    let (mut session, _, theory) = setup();
    // Pure body: a math function, and the for becomes a kernel term.
    assert!(matches!(
        session.declare_fn(&counting_loop(theory, true, None)),
        Ok(FnRef::Math(_))
    ));
    // The same loop in an ordinary function.
    assert!(matches!(
        session.declare_fn(&counting_loop(theory, false, None)),
        Ok(FnRef::Exec(_))
    ));
    // A body that calls an ordinary function: a statement of the check IR.
    let increment_id = exec_id(session.declare_fn(&increment(false)).unwrap());
    assert_eq!(
        session
            .declare_fn(&counting_loop(theory, false, Some(increment_id)))
            .map(|_| ()),
        Ok(())
    );
    // Which a math function cannot contain.
    assert_eq!(
        session.declare_fn(&counting_loop(theory, true, Some(increment_id))),
        Err(LowerError::ImpureInMath("count".into()))
    );
}
