//! The typed tree and `lower` (Architecture in atlas.html): the specification's
//! programs written in source shape, lowered, and checked.

mod common;

use common::*;
use locus::exec::ExecError;
use locus::kernel::{HypId, KernelError, Proof, Term, Type, VarId};
use locus::typed::{Binder, Expr, FnItem, FnRef, LowerError};

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
    let (mut session, _, theory) = setup();
    assert!(matches!(
        session.declare_fn(&preserve(theory, false, true)),
        Ok(FnRef::Exec(_))
    ));
    assert!(matches!(
        session.declare_fn(&preserve(theory, true, true)),
        Ok(FnRef::Math(_))
    ));
    // Either way the branch must use its fact.
    assert!(session.declare_fn(&preserve(theory, false, false)).is_err());
    assert!(session.declare_fn(&preserve(theory, true, false)).is_err());
    // The math version is a kernel function the logic can compute with.
    let FnRef::Math(id) = session.declare_fn(&preserve(theory, true, true)).unwrap() else {
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
            Expr::u8(0),
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
    let (mut session, prelude, _) = setup();
    let classified = session.declare_enum(&classified_enum(prelude)).unwrap();
    let classify_id = exec_id(session.declare_fn(&classify(classified)).unwrap());
    assert!(
        session
            .declare_fn(&zero_or_self(prelude, classified, classify_id))
            .is_ok()
    );
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

// --- Mutation (M2): versions, the assigned set, and the join --------------------------

use locus::erased::{Interpreter, Outcome, Value};
use locus::exec::{CheckInterpreter, Stmt as ExecStmt, Tail};
use locus::kernel::MachineInt;
use locus::typed::{Joined, Pattern, Session, Step, Stmt, StructItem};

/// Declares a function and runs it on a byte in both interpreters, which
/// must agree on a value.
fn agreed(session: &mut Session, item: &FnItem, byte: u8) -> Value {
    let reference = session
        .declare_fn(item)
        .unwrap_or_else(|error| panic!("{}: {error}", item.name));
    let arguments = vec![Value::u8(byte)];
    let checked =
        CheckInterpreter::new(session.program(), 100_000).call(reference, arguments.clone());
    let erased = Interpreter::new(session.erased(), 100_000).call(reference, arguments);
    assert_eq!(checked, erased, "{} at {byte}", item.name);
    match checked {
        Ok(Outcome::Value(value)) => value,
        other => panic!("{} at {byte}: {other:?}", item.name),
    }
}

/// The match statement of a function's body: the arity of its result when
/// that is a tuple, and what each arm ends with. A branch with assignments
/// lowers to a match whose arms end in tuples.
fn joining_match(session: &Session, reference: FnRef) -> (usize, Vec<Vec<Term>>) {
    let function = session.program().function(exec_id(reference)).unwrap();
    for stmt in &function.body.stmts {
        if let ExecStmt::Match { ty, arms, .. } = stmt {
            let arity = match ty {
                Type::Tuple(fields) => fields.len(),
                _ => 0,
            };
            let ends = arms
                .iter()
                .map(|arm| match &arm.body.tail {
                    Tail::Value(Term::Tuple(_, values)) => values.clone(),
                    Tail::Value(other) => vec![other.clone()],
                    other => panic!("an arm ending in {other:?}"),
                })
                .collect();
            return (arity, ends);
        }
    }
    panic!("no match statement")
}

fn bytes(pair: &Value) -> (u8, u8) {
    match pair {
        Value::Tuple(fields) => match fields.as_slice() {
            [Value::Int(_, a), Value::Int(_, b)] => (*a as u8, *b as u8),
            _ => panic!("not a byte pair"),
        },
        _ => panic!("not a tuple"),
    }
}

#[test]
fn an_assignment_is_a_let_of_a_new_version() {
    let (mut session, _, _) = setup();
    for byte in [0, 7, 254, 255] {
        let value = agreed(&mut session.clone(), &straight_line_mutation(), byte);
        assert_eq!(value, Value::u8(byte.wrapping_add(2)));
    }
    let reference = session.declare_fn(&straight_line_mutation()).unwrap();
    let function = session.program().function(exec_id(reference)).unwrap();
    // let x = n; let x1 = x + 1; let x2 = x1 + 1: three lets, no assignment.
    assert_eq!(function.body.stmts.len(), 3);
    assert!(
        function
            .body
            .stmts
            .iter()
            .all(|stmt| matches!(stmt, ExecStmt::Let { .. }))
    );
}

#[test]
fn a_branch_that_assigns_joins_the_new_versions_in_declaration_order() {
    let (mut session, _, _) = setup();
    let item = branching_mutation();
    for byte in [0, 1, 250, 255] {
        let value = agreed(&mut session.clone(), &item, byte);
        let expected = if byte == 0 {
            (2, 1)
        } else {
            (byte.wrapping_add(3), 0)
        };
        assert_eq!(bytes(&value), expected);
    }
    let reference = session.declare_fn(&item).unwrap();
    let (arity, ends) = joining_match(&session, reference);
    // (a, b, ()) from each arm; the false arm comes first.
    assert_eq!(arity, 3);
    assert_eq!(ends.len(), 2);
    assert!(ends.iter().all(|end| end.len() == 3));
    let Stmt::Let {
        pattern: Pattern::Bind { binder: b, .. },
        ..
    } = &item.body.stmts[1]
    else {
        panic!("let mut b")
    };
    // The else arm (false, first) does not assign b: it passes the entry
    // version through. The then arm gives b its own version, and both
    // arms give a one.
    assert_eq!(ends[0][1], Term::var(b.id));
    assert_ne!(ends[1][1], Term::var(b.id));
    assert_ne!(ends[0][0], ends[1][0]);
}

#[test]
fn the_join_must_name_exactly_the_bindings_the_arms_assign() {
    let (mut session, _, _) = setup();
    let n = Binder::new("n", Type::U8);
    let (a, b) = (Binder::new("a", Type::U8), Binder::new("b", Type::U8));
    let (a_then, a_join, b_join) = (
        Binder::new("a", Type::U8),
        Binder::new("a", Type::U8),
        Binder::new("b", Type::U8),
    );
    let program = |joined: Option<Joined>, tail: Expr| FnItem {
        name: "joins".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![
                let_mut(&a, Expr::var(&n)),
                let_mut(&b, Expr::var(&n)),
                Stmt::Expr(if_(
                    is_zero(&n),
                    unit_block(vec![assign(&a, vec![], Expr::u8(2), &a_then)]),
                    unit_block(vec![]),
                    Type::Tuple(vec![]),
                    joined,
                )),
            ],
            tail,
        ),
    };
    // No join recorded, although the then arm assigns `a`.
    assert_eq!(
        session.declare_fn(&program(None, Expr::var(&a))),
        Err(LowerError::JoinMismatch)
    );
    // A join that names `b` too, which no arm assigns.
    assert_eq!(
        session.declare_fn(&program(
            Some(joined(vec![(&a, &a_join), (&b, &b_join)])),
            Expr::var(&a_join)
        )),
        Err(LowerError::JoinMismatch)
    );
    // The right join.
    assert!(
        session
            .declare_fn(&program(
                Some(joined(vec![(&a, &a_join)])),
                Expr::var(&a_join)
            ))
            .is_ok()
    );
    // After the join, the entry version is stale: the tree may not read it.
    assert_eq!(
        session.declare_fn(&program(Some(joined(vec![(&a, &a_join)])), Expr::var(&a))),
        Err(LowerError::StaleMention("a".into()))
    );
}

#[test]
fn the_assigned_set_is_over_binding_identities() {
    let (session, _, _) = setup();
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let x_join = Binder::new("x", Type::U8);
    let x_then = Binder::new("x", Type::U8);
    // let mut x = n; if n == 0 { <then> } else {}; <tail>
    let program = |name: &str, then: Vec<Stmt>, joined: Option<Joined>, tail: Expr| FnItem {
        name: name.into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![
                let_mut(&x, Expr::var(&n)),
                Stmt::Expr(if_(
                    is_zero(&n),
                    unit_block(then),
                    unit_block(vec![]),
                    Type::Tuple(vec![]),
                    joined,
                )),
            ],
            tail,
        ),
    };
    let arity = |item: &FnItem| {
        let mut session = session.clone();
        let reference = session
            .declare_fn(item)
            .unwrap_or_else(|error| panic!("{}: {error}", item.name));
        joining_match(&session, reference).0
    };

    // A `let mut` local to the arm is never in the set: no tuple, an
    // ordinary match of unit type.
    let y = Binder::new("y", Type::U8);
    let y1 = Binder::new("y", Type::U8);
    let local = program(
        "local",
        vec![
            let_mut(&y, Expr::u8(0)),
            assign(&y, vec![], Expr::u8(1), &y1),
        ],
        None,
        Expr::var(&x),
    );
    assert_eq!(arity(&local), 0);

    // An outer binding assigned inside a nested block of the arm is in it.
    let nested = program(
        "nested",
        vec![Stmt::Expr(Expr::Block(unit_block(vec![assign(
            &x,
            vec![],
            Expr::u8(1),
            &x_then,
        )])))],
        Some(joined(vec![(&x, &x_join)])),
        Expr::var(&x_join),
    );
    assert_eq!(arity(&nested), 2);

    // A shadowing binding assigned while the outer one is not: the outer
    // stays out of the set, although both are named `x`.
    let shadow = Binder::new("x", Type::U8);
    let shadow1 = Binder::new("x", Type::U8);
    let inner_only = program(
        "inner_only",
        vec![
            let_mut(&shadow, Expr::u8(0)),
            assign(&shadow, vec![], Expr::u8(1), &shadow1),
        ],
        None,
        Expr::var(&x),
    );
    assert_eq!(arity(&inner_only), 0);

    // And the reverse: the outer assigned before it is shadowed.
    let outer_first = program(
        "outer_first",
        vec![
            assign(&x, vec![], Expr::u8(5), &x_then),
            let_mut(&shadow, Expr::u8(0)),
            assign(&shadow, vec![], Expr::u8(1), &shadow1),
        ],
        Some(joined(vec![(&x, &x_join)])),
        Expr::var(&x_join),
    );
    assert_eq!(arity(&outer_first), 2);
}

#[test]
fn a_field_write_counts_for_the_root_of_its_path() {
    let (mut session, _, _) = setup();
    let n = Binder::new("n", Type::U8);
    let p = Binder::new("p", pair_type());
    let (p_then, p_join) = (Binder::new("p", pair_type()), Binder::new("p", pair_type()));
    let item = FnItem {
        name: "field".into(),
        math: false,
        params: vec![n.clone()],
        result: pair_type(),
        body: block(
            vec![
                let_mut(&p, pair(Expr::var(&n), Expr::u8(0))),
                Stmt::Expr(if_(
                    is_zero(&n),
                    unit_block(vec![assign(
                        &p,
                        vec![byte_tuple_step(1, 2)],
                        Expr::u8(9),
                        &p_then,
                    )]),
                    unit_block(vec![]),
                    Type::Tuple(vec![]),
                    Some(joined(vec![(&p, &p_join)])),
                )),
            ],
            Expr::var(&p_join),
        ),
    };
    for byte in [0, 1, 255] {
        let value = agreed(&mut session.clone(), &item, byte);
        assert_eq!(bytes(&value), (byte, if byte == 0 { 9 } else { 0 }));
    }
    let reference = session.declare_fn(&item).unwrap();
    let (arity, ends) = joining_match(&session, reference);
    assert_eq!(arity, 2);
    // The then arm's version of p is the rebuilt pair (p.0, 9), bound by a
    // let and passed as a variable; the else arm passes p itself.
    assert_ne!(ends[0][0], ends[1][0]);
    assert!(matches!(&ends[1][0], Term::Free(_)));
}

#[test]
fn the_right_side_of_an_assignment_runs_before_the_place_is_rebuilt() {
    let (session, _, _) = setup();
    for byte in [0, 3, 255] {
        let value = agreed(&mut session.clone(), &right_side_changes_the_place(), byte);
        // lo == 3 and hi == 7: the place was rebuilt from the version the
        // right side made, not from the entry version.
        assert_eq!(bytes(&value), (3, 7));
    }
}

#[test]
fn an_assignment_to_a_binding_lowering_does_not_know_is_rejected() {
    let (mut session, _, _) = setup();
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let x1 = Binder::new("x", Type::U8);
    let program = |first: Stmt| FnItem {
        name: "unknown".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![first, assign(&x, vec![], Expr::u8(1), &x1)],
            Expr::var(&x1),
        ),
    };
    // A binding declared without `mut`.
    assert_eq!(
        session.declare_fn(&program(let_(&x, HypId::fresh(), Expr::var(&n)))),
        Err(LowerError::AssignToUnknown("x".into()))
    );
    // A binding never declared at all.
    let other = Binder::new("other", Type::U8);
    assert_eq!(
        session.declare_fn(&program(let_mut(&other, Expr::var(&n)))),
        Err(LowerError::AssignToUnknown("x".into()))
    );
}

#[test]
fn a_stale_mention_is_rejected_and_a_loop_body_may_not_assign_outside() {
    let (mut session, _, theory) = setup();
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let x1 = Binder::new("x", Type::U8);
    let stale = FnItem {
        name: "stale".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![
                let_mut(&x, Expr::var(&n)),
                assign(&x, vec![], Expr::u8(1), &x1),
            ],
            // The tree reads the old version after the assignment.
            Expr::var(&x),
        ),
    };
    assert_eq!(
        session.declare_fn(&stale),
        Err(LowerError::StaleMention("x".into()))
    );
    // for i in 0..n (acc = 0) { x = i; continue(acc) }
    let i = Binder::new("i", Type::U8);
    let acc = Binder::new("acc", Type::U8);
    let in_loop = FnItem {
        name: "in_loop".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::Tuple(vec![Type::U8]),
        body: block(
            vec![let_mut(&x, Expr::var(&n))],
            Expr::For {
                index: i.clone(),
                lower: HypId::fresh(),
                upper: HypId::fresh(),
                lo: Box::new(Expr::u8(0)),
                hi: Box::new(Expr::var(&n)),
                ordered: lemma(
                    theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
                    vec![n.term()],
                ),
                state: vec![(acc.clone(), Expr::u8(0))],
                body: block(
                    vec![assign(&x, vec![], Expr::var(&i), &x1)],
                    Expr::Continue(vec![Expr::var(&acc)]),
                ),
                result: VarId::fresh(),
            },
        ),
    };
    assert_eq!(
        session.declare_fn(&in_loop),
        Err(LowerError::AssignInLoop("x".into()))
    );
}

#[test]
fn a_field_that_evidence_depends_on_cannot_be_assigned_alone() {
    // struct Percent { value: u8, in_range: @(value <= 100) }
    let (mut session, _, _) = setup();
    let value = Binder::new("value", Type::U8);
    let in_range = Binder::new("in_range", Type::proof(u8_le(value.term(), Term::U8(100))));
    let percent = session
        .declare_struct(&StructItem {
            name: "Percent".into(),
            fields: vec![value.clone(), in_range],
        })
        .unwrap();
    // fn set(p: Percent) -> Percent { let mut copy = p; copy.value = 0; copy }
    let p = Binder::new("p", Type::Struct(percent));
    let copy = Binder::new("copy", Type::Struct(percent));
    let copy1 = Binder::new("copy", Type::Struct(percent));
    let step = Step {
        index: 0,
        name: Some("value".into()),
        ty: Type::Struct(percent),
        proof_fields: vec![false, true],
    };
    let item = FnItem {
        name: "set".into(),
        math: false,
        params: vec![p.clone()],
        result: Type::Struct(percent),
        body: block(
            vec![
                let_mut(&copy, Expr::var(&p)),
                assign(&copy, vec![step], Expr::u8(0), &copy1),
            ],
            Expr::var(&copy1),
        ),
    };
    // The rebuilt struct carries `in_range` about the old value; the kernel
    // rejects it whatever the elaborator thought.
    assert!(matches!(
        session.declare_fn(&item),
        Err(LowerError::Exec(ExecError::Kernel(_)))
    ));
}

#[test]
fn a_block_that_assigns_keeps_its_version_and_mut_is_printed_only_when_needed() {
    let (mut session, _, _) = setup();
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let x1 = Binder::new("x", Type::U8);
    let item = FnItem {
        name: "blocked".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![
                let_mut(&x, Expr::var(&n)),
                Stmt::Expr(Expr::Block(unit_block(vec![assign(
                    &x,
                    vec![],
                    Expr::u8(3),
                    &x1,
                )]))),
            ],
            Expr::var(&x1),
        ),
    };
    assert_eq!(agreed(&mut session, &item, 9), Value::u8(3));
    let rust = locus::erased::print_module(session.erased());
    assert!(rust.contains("let mut x = n;"), "{rust}");
    assert!(rust.contains("x = 3_u8;"), "{rust}");
    // A `let mut` that is never assigned is printed without `mut`.
    let unassigned = FnItem {
        name: "unassigned".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(vec![let_mut(&x, Expr::var(&n))], Expr::var(&x)),
    };
    session.declare_fn(&unassigned).unwrap();
    let rust = locus::erased::print_module(session.erased());
    assert!(rust.contains("let x = n;"), "{rust}");
}
