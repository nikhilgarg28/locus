//! The typed tree and `lower` (docs/ir-architecture.md): the specification's
//! programs written in source shape, lowered, and checked.

use locus::exec::ExecError;
use locus::kernel::derive::symm_at;
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, Definitions, FnId, HypId, KernelError, Prelude, Prim, Proof, Term, Type, VarId,
};
use locus::typed::{
    Binder, Block, CompareOp, EnumItem, Expr, FnItem, FnRef, LowerError, MatchArm, Pattern,
    Session, Stmt, VariantItem,
};

fn setup() -> (Session, Prelude, Theory) {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    (Session::new(definitions), prelude, theory)
}

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn add_one(term: Term) -> Term {
    Term::wrapping_add(term, Term::U8(1))
}

fn lemma(id: FnId, arguments: Vec<Term>) -> Proof {
    Proof::OfTerm(Term::call(Term::Fn(id), arguments))
}

/// `value.wrapping_add(1)`
fn plus_one(value: Expr) -> Expr {
    Expr::Method {
        prim: Prim::WrappingAdd,
        receiver: Box::new(value),
        arguments: vec![Expr::U8(1)],
    }
}

fn block(stmts: Vec<Stmt>, tail: Expr) -> Block {
    Block {
        stmts,
        tail: Some(Box::new(tail)),
    }
}

fn let_(binder: &Binder, equation: HypId, value: Expr) -> Stmt {
    Stmt::Let {
        pattern: Pattern::Bind {
            binder: binder.clone(),
            equation,
        },
        value,
    }
}

/// `target.index`, a byte.
fn field(target: Expr, index: usize) -> Expr {
    Expr::Field {
        target: Box::new(target),
        index,
        name: None,
        ty: Type::U8,
    }
}

/// `(out: u8, @[claim(out)])`
fn data_with_evidence(claim: impl Fn(Term) -> Term + 'static) -> Type {
    Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [out] => Some(Type::proof(claim(out.clone()))),
        _ => None,
    })
}

fn exec_id(reference: FnRef) -> locus::exec::ExecFnId {
    match reference {
        FnRef::Exec(id) => id,
        FnRef::Math(_) => panic!("expected an ordinary function"),
    }
}

/// fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
///     let out = n.wrapping_add(1);
///     (out, _)
/// }
fn increment(math: bool) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let out = Binder::new("out", Type::U8);
    let out_is = HypId::fresh();
    let n_term = n.term();
    let result = data_with_evidence(move |out| u8_eq(out, add_one(n_term.clone())));
    FnItem {
        name: "increment".into(),
        math,
        params: vec![n.clone()],
        result: result.clone(),
        body: block(
            vec![let_(&out, out_is, plus_one(Expr::var(&n)))],
            Expr::Tuple {
                ty: result,
                fields: vec![Expr::var(&out), Expr::Proof(Proof::hyp(out_is))],
            },
        ),
    }
}

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

/// fn preserve(n: u8) -> (out: u8, @[out == n]) {
///     if n == 0 { (0, _) } else { (n, _) }
/// }
fn preserve(math: bool, use_the_fact: bool) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let n_term = n.term();
    let result = data_with_evidence({
        let n_term = n_term.clone();
        move |out| u8_eq(out, n_term.clone())
    });
    let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
    // The fact of the then branch is about the comparison the condition
    // performs; reflection turns it into n == 0.
    let comparison = Term::prim(Prim::U8Eq, vec![n_term.clone(), Term::U8(0)]);
    let n_is_zero = Proof::implies_elim(
        Proof::Axiom(Axiom::Reflect(comparison, true)),
        Proof::hyp(then_fact),
    );
    let target = n_term.clone();
    let zero_is_n = Proof::transport(
        n_is_zero,
        |hole| u8_eq(hole, target.clone()),
        Proof::Refl(n_term.clone()),
    );
    let evidence = if use_the_fact {
        zero_is_n
    } else {
        Proof::Refl(Term::U8(0))
    };
    let pair = |value: Expr, proof: Proof| Expr::Tuple {
        ty: result.clone(),
        fields: vec![value, Expr::Proof(proof)],
    };
    FnItem {
        name: "preserve".into(),
        math,
        params: vec![n.clone()],
        result: result.clone(),
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::If {
                condition: Box::new(Expr::Compare {
                    op: CompareOp::Eq,
                    left: Box::new(Expr::var(&n)),
                    right: Box::new(Expr::U8(0)),
                }),
                then_fact,
                else_fact,
                then_block: block(vec![], pair(Expr::U8(0), evidence)),
                else_block: block(vec![], pair(Expr::var(&n), Proof::Refl(n_term))),
                ty: result.clone(),
                result: VarId::fresh(),
            })),
        },
    }
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

/// fn bounded_walk(limit: u8) -> (value: u8, evidence: @[value <= limit]) of
/// specification section 10.4, in source shape.
fn bounded_walk(prelude: Prelude, theory: Theory, carry_the_invariant: bool) -> FnItem {
    let limit = Binder::new("limit", Type::U8);
    let limit_term = limit.term();
    let result = data_with_evidence({
        let limit_term = limit_term.clone();
        move |value| prelude.u8_le_prop(value, limit_term.clone())
    });
    let i = Binder::new("i", Type::U8);
    let bound = Binder::new(
        "bound",
        Type::proof(prelude.u8_le_prop(i.term(), limit_term.clone())),
    );
    let next = Binder::new("next", Type::U8);
    let next_bound = Binder::new(
        "next_bound",
        Type::proof(prelude.u8_le_prop(next.term(), limit_term.clone())),
    );
    let differs = Binder::new(
        "differs",
        Type::proof(prelude.not_prop(u8_eq(i.term(), limit_term.clone()))),
    );
    let below = Binder::new(
        "below",
        Type::proof(prelude.u8_lt_prop(i.term(), limit_term.clone())),
    );
    let (then_fact, else_fact, next_is) = (HypId::fresh(), HypId::fresh(), HypId::fresh());
    let comparison = Term::prim(Prim::U8Eq, vec![i.term(), limit_term.clone()]);
    let as_proof = |binder: &Binder| Proof::OfTerm(binder.term());
    let limit_in = limit_term.clone();
    let carried = if carry_the_invariant {
        Expr::var(&next_bound)
    } else {
        Expr::var(&bound)
    };
    let keep_walking = block(
        vec![
            let_(
                &differs,
                HypId::fresh(),
                Expr::Proof(Proof::implies_elim(
                    Proof::Axiom(Axiom::Reflect(comparison, false)),
                    Proof::hyp(else_fact),
                )),
            ),
            let_(
                &below,
                HypId::fresh(),
                Expr::Proof(lemma(
                    theory.u8_lt_of_le_of_ne,
                    vec![
                        i.term(),
                        limit_term.clone(),
                        Term::proof(as_proof(&bound)),
                        Term::proof(as_proof(&differs)),
                    ],
                )),
            ),
            let_(&next, next_is, plus_one(Expr::var(&i))),
            let_(
                &next_bound,
                HypId::fresh(),
                Expr::Proof(Proof::transport(
                    symm_at(&Type::U8, &next.term(), Proof::hyp(next_is)),
                    |hole| prelude.u8_le_prop(hole, limit_in.clone()),
                    lemma(
                        theory.u8_succ_le_of_lt,
                        vec![i.term(), limit_term.clone(), Term::proof(as_proof(&below))],
                    ),
                )),
            ),
        ],
        Expr::Continue(vec![Expr::var(&next), carried]),
    );
    let stop = block(
        vec![],
        Expr::Break(Box::new(Expr::Tuple {
            ty: result.clone(),
            fields: vec![Expr::var(&i), Expr::var(&bound)],
        })),
    );
    FnItem {
        name: "bounded_walk".into(),
        math: false,
        params: vec![limit.clone()],
        result: result.clone(),
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::Loop {
                state: vec![
                    (i.clone(), Expr::U8(0)),
                    (
                        bound.clone(),
                        Expr::Proof(lemma(theory.u8_zero_le, vec![limit_term.clone()])),
                    ),
                ],
                result_ty: result,
                body: Block {
                    stmts: vec![],
                    tail: Some(Box::new(Expr::If {
                        condition: Box::new(Expr::Compare {
                            op: CompareOp::Eq,
                            left: Box::new(Expr::var(&i)),
                            right: Box::new(Expr::var(&limit)),
                        }),
                        then_fact,
                        else_fact,
                        then_block: stop,
                        else_block: keep_walking,
                        ty: Type::Tuple(vec![]),
                        result: VarId::fresh(),
                    })),
                },
                result: VarId::fresh(),
            })),
        },
    }
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

/// for i in 0..n (acc: u8 = 0, same: @[acc == i] = _) { continue(step(acc), _) }
/// where `step` is either a pure increment or a call to `increment`.
fn counting_loop(theory: Theory, math: bool, step: Option<locus::exec::ExecFnId>) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let i = Binder::new("i", Type::U8);
    let acc = Binder::new("acc", Type::U8);
    let same = Binder::new("same", Type::proof(u8_eq(acc.term(), i.term())));
    let n_term = n.term();
    let result = data_with_evidence(move |total| u8_eq(total, n_term.clone()));

    let (stmts, stepped, stepped_term, because) = match step {
        None => {
            let value = add_one(acc.term());
            (
                vec![],
                plus_one(Expr::var(&acc)),
                value.clone(),
                Proof::Refl(value),
            )
        }
        Some(increment_id) => {
            // let r = increment(acc);   r.1 : r.0 == acc + 1
            let call = VarId::fresh();
            let r_term = Term::var(call);
            let acc_term = acc.term();
            let r_type = data_with_evidence(move |out| u8_eq(out, add_one(acc_term.clone())));
            let stmts = vec![Stmt::Let {
                pattern: Pattern::Wildcard,
                value: Expr::CallFn {
                    id: increment_id,
                    name: "increment".into(),
                    arguments: vec![Expr::var(&acc)],
                    result: call,
                    ty: r_type.clone(),
                },
            }];
            (
                stmts,
                field(
                    Expr::Var {
                        id: call,
                        name: "r".into(),
                        ty: r_type,
                    },
                    0,
                ),
                Term::proj(r_term.clone(), 0),
                Proof::OfTerm(Term::proj(r_term, 1)),
            )
        }
    };
    // stepped == acc + 1 and acc == i give stepped == i + 1.
    let left = stepped_term.clone();
    let advanced = Proof::transport(
        Proof::OfTerm(same.term()),
        |hole| u8_eq(left.clone(), add_one(hole)),
        because,
    );
    FnItem {
        name: "count".into(),
        math,
        params: vec![n.clone()],
        result,
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::For {
                index: i.clone(),
                lower: HypId::fresh(),
                upper: HypId::fresh(),
                lo: Box::new(Expr::U8(0)),
                hi: Box::new(Expr::var(&n)),
                ordered: lemma(theory.u8_zero_le, vec![n.term()]),
                state: vec![
                    (acc.clone(), Expr::U8(0)),
                    (same.clone(), Expr::Proof(Proof::Refl(Term::U8(0)))),
                ],
                body: block(stmts, Expr::Continue(vec![stepped, Expr::Proof(advanced)])),
                result: VarId::fresh(),
            })),
        },
    }
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
