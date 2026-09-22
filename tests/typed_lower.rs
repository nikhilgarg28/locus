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
fn a_break_supplies_evidence_about_the_version_the_body_sees() {
    let (mut session, _, theory) = setup();
    assert_eq!(
        session.declare_fn(&bounded_walk(theory, true)).map(|_| ()),
        Ok(())
    );
    // Evidence about the value `i` started with, where evidence about the
    // version at the break is wanted: the kernel rejects it.
    assert!(matches!(
        session.declare_fn(&bounded_walk(theory, false)),
        Err(LowerError::Exec(ExecError::Kernel(
            KernelError::ProofMismatch { .. }
        )))
    ));
}

#[test]
fn a_for_is_a_statement_of_the_check_ir_and_never_a_kernel_term() {
    let (mut session, _, _) = setup();
    // No loop is pure, so none stands in a math function, whatever its
    // body does.
    assert_eq!(
        session.declare_fn(&counting_loop(true, None)),
        Err(LowerError::ImpureInMath("count".into()))
    );
    assert!(matches!(
        session.declare_fn(&counting_loop(false, None)),
        Ok(FnRef::Exec(_))
    ));
    let increment_id = exec_id(session.declare_fn(&increment(false)).unwrap());
    assert_eq!(
        session
            .declare_fn(&counting_loop(false, Some(increment_id)))
            .map(|_| ()),
        Ok(())
    );
    assert_eq!(
        session.declare_fn(&counting_loop(true, Some(increment_id))),
        Err(LowerError::ImpureInMath("count".into()))
    );
}

// --- Mutation (M2): versions, the assigned set, and the join --------------------------

use locus::erased::{Interpreter, Outcome, Value};
use locus::exec::{CheckInterpreter, Stmt as ExecStmt, Tail};
use locus::kernel::MachineInt;
use locus::kernel::derive::symm_at;
use locus::kernel::theory::Theory;
use locus::typed::{Block, CompareOp, Place};
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
fn a_stale_mention_is_rejected() {
    let (mut session, _, _) = setup();
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
}

// --- Loops (M3): what a loop carries -----------------------------------------------------

/// The state of the first loop or `for` statement of a function's body: the
/// number of variables it carries.
fn carried_arity(session: &Session, reference: FnRef) -> usize {
    let function = session.program().function(exec_id(reference)).unwrap();
    for stmt in &function.body.stmts {
        match stmt {
            ExecStmt::Loop { vars, .. } => return vars.len(),
            ExecStmt::For(looped) => return looped.vars.len(),
            _ => {}
        }
    }
    panic!("no loop statement")
}

/// `fn f(n: u8) -> u8 { let mut x = n; let mut y = 0; <loop>; x }`, where the
/// loop is built by `build` from the entry bindings and the versions their
/// bodies see, and its body is `body`. The loop runs `n` times at most.
fn looping(
    name: &str,
    build: impl FnOnce(&Binder, &Binder, &Binder) -> (Vec<Binder>, Vec<(Binder, Binder)>, Block),
) -> (FnItem, Binder) {
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let y = Binder::new("y", Type::U8);
    let (state, joins, body) = build(&n, &x, &y);
    let carried = carried(
        joins
            .iter()
            .map(|(binding, after)| (binding, after))
            .collect(),
    );
    let after_x = joins
        .iter()
        .find(|(binding, _)| binding.id == x.id)
        .map_or(x.clone(), |(_, after)| after.clone());
    let item = FnItem {
        name: name.into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![
                let_mut(&x, Expr::var(&n)),
                let_mut(&y, Expr::u8(0)),
                Stmt::Expr(for_(
                    &Binder::new("i", Type::U8),
                    Expr::u8(0),
                    Expr::var(&n),
                    state,
                    carried,
                    body,
                )),
            ],
            Expr::var(&after_x),
        ),
    };
    (item, after_x)
}

#[test]
fn a_let_mut_inside_the_body_is_not_carried() {
    // for i in 0..n { let mut local = x; local = local + 1; x = local; }
    let (mut session, _, _) = setup();
    let (item, _) = looping("local", |_, x, _| {
        let (x_in, x_after) = versions(x);
        let local = Binder::new("local", Type::U8);
        let (local1, x1) = (Binder::new("local", Type::U8), Binder::new("x", Type::U8));
        let body = unit_block(vec![
            let_mut(&local, Expr::var(&x_in)),
            assign(&local, vec![], plus_one(Expr::var(&local)), &local1),
            assign(x, vec![], Expr::var(&local1), &x1),
        ]);
        (vec![x_in], vec![(x.clone(), x_after)], body)
    });
    for byte in [0, 3, 255] {
        assert_eq!(
            agreed(&mut session.clone(), &item, byte),
            Value::u8(byte.wrapping_add(byte))
        );
    }
    let reference = session.declare_fn(&item).unwrap();
    assert_eq!(carried_arity(&session, reference), 1);
}

#[test]
fn an_outer_binding_assigned_in_a_nested_block_is_carried() {
    // for i in 0..n { { x = x + 1; } }
    let (mut session, _, _) = setup();
    let (item, _) = looping("nested", |_, x, _| {
        let (x_in, x_after) = versions(x);
        let x1 = Binder::new("x", Type::U8);
        let inner = unit_block(vec![assign(x, vec![], plus_one(Expr::var(&x_in)), &x1)]);
        let body = unit_block(vec![Stmt::Expr(Expr::Block(inner))]);
        (vec![x_in], vec![(x.clone(), x_after)], body)
    });
    for byte in [0, 3, 255] {
        assert_eq!(
            agreed(&mut session.clone(), &item, byte),
            Value::u8(byte.wrapping_add(byte))
        );
    }
    let reference = session.declare_fn(&item).unwrap();
    assert_eq!(carried_arity(&session, reference), 1);
}

#[test]
fn a_field_write_in_the_body_carries_the_root() {
    // let mut p = (n, n); for i in 0..n { p.1 = p.1 + 1; } p.1
    let (mut session, _, _) = setup();
    let n = Binder::new("n", Type::U8);
    let p = Binder::new("p", pair_type());
    let (p_in, p_after) = versions(&p);
    let p1 = Binder::new("p", pair_type());
    let item = FnItem {
        name: "field".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![
                let_mut(&p, pair(Expr::var(&n), Expr::var(&n))),
                Stmt::Expr(for_(
                    &Binder::new("i", Type::U8),
                    Expr::u8(0),
                    Expr::var(&n),
                    vec![p_in.clone()],
                    carried(vec![(&p, &p_after)]),
                    unit_block(vec![assign(
                        &p,
                        vec![byte_tuple_step(1, 2)],
                        plus_one(field(Expr::var(&p_in), 1)),
                        &p1,
                    )]),
                )),
            ],
            field(Expr::var(&p_after), 1),
        ),
    };
    for byte in [0, 3, 255] {
        assert_eq!(
            agreed(&mut session.clone(), &item, byte),
            Value::u8(byte.wrapping_add(byte))
        );
    }
    let reference = session.declare_fn(&item).unwrap();
    assert_eq!(carried_arity(&session, reference), 1);
}

#[test]
fn a_binding_that_shadows_inside_the_body_is_another_binding() {
    // for i in 0..n { let mut x = 7; x = x + 1; } with the outer x untouched
    let (mut session, _, _) = setup();
    let (item, _) = looping("shadow", |_, _, _| {
        let inner = Binder::new("x", Type::U8);
        let inner1 = Binder::new("x", Type::U8);
        let body = unit_block(vec![
            let_mut(&inner, Expr::u8(7)),
            assign(&inner, vec![], plus_one(Expr::var(&inner)), &inner1),
        ]);
        (vec![], vec![], body)
    });
    for byte in [0, 3, 255] {
        assert_eq!(agreed(&mut session.clone(), &item, byte), Value::u8(byte));
    }
    let reference = session.declare_fn(&item).unwrap();
    assert_eq!(carried_arity(&session, reference), 0);
}

#[test]
fn an_assignment_in_a_while_condition_is_carried() {
    // let mut x = n; let mut y = 0;
    // while { y = y + 1; y < 4 } { x = x + 1; } (x, y)
    let (mut session, _, _) = setup();
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let y = Binder::new("y", Type::U8);
    let (x_in, x_after) = versions(&x);
    let (y_in, y_after) = versions(&y);
    let (x1, y1) = (Binder::new("x", Type::U8), Binder::new("y", Type::U8));
    let condition = Expr::Block(block(
        vec![assign(&y, vec![], plus_one(Expr::var(&y_in)), &y1)],
        compare_u8(CompareOp::Lt, Expr::var(&y1), Expr::u8(4)),
    ));
    let item = FnItem {
        name: "counted".into(),
        math: false,
        params: vec![n.clone()],
        result: pair_type(),
        body: block(
            vec![
                let_mut(&x, Expr::var(&n)),
                let_mut(&y, Expr::u8(0)),
                Stmt::Expr(while_(
                    condition,
                    vec![x_in.clone(), y_in.clone()],
                    carried(vec![(&x, &x_after), (&y, &y_after)]),
                    unit_block(vec![assign(&x, vec![], plus_one(Expr::var(&x_in)), &x1)]),
                )),
            ],
            pair(Expr::var(&x_after), Expr::var(&y_after)),
        ),
    };
    for byte in [0, 3, 255] {
        assert_eq!(
            bytes(&agreed(&mut session.clone(), &item, byte)),
            (byte.wrapping_add(3), 4)
        );
    }
    let reference = session.declare_fn(&item).unwrap();
    assert_eq!(carried_arity(&session, reference), 2);
}

#[test]
fn a_loop_must_carry_exactly_what_it_assigns() {
    let (mut session, _, _) = setup();
    // The body assigns x, and the tree carries nothing.
    let (nothing, _) = looping("nothing", |_, x, _| {
        let x1 = Binder::new("x", Type::U8);
        let body = unit_block(vec![assign(x, vec![], plus_one(Expr::var(x)), &x1)]);
        (vec![], vec![], body)
    });
    assert_eq!(session.declare_fn(&nothing), Err(LowerError::JoinMismatch));
    // The tree carries y too, which the body leaves alone.
    let (too_much, _) = looping("too_much", |_, x, y| {
        let (x_in, x_after) = versions(x);
        let (y_in, y_after) = versions(y);
        let x1 = Binder::new("x", Type::U8);
        let body = unit_block(vec![assign(x, vec![], plus_one(Expr::var(&x_in)), &x1)]);
        (
            vec![x_in, y_in],
            vec![(x.clone(), x_after), (y.clone(), y_after)],
            body,
        )
    });
    assert_eq!(session.declare_fn(&too_much), Err(LowerError::JoinMismatch));
    // After the loop the entry version is stale.
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let (x_in, x_after) = versions(&x);
    let x1 = Binder::new("x", Type::U8);
    let stale = FnItem {
        name: "stale".into(),
        math: false,
        params: vec![n.clone()],
        result: Type::U8,
        body: block(
            vec![
                let_mut(&x, Expr::var(&n)),
                Stmt::Expr(for_(
                    &Binder::new("i", Type::U8),
                    Expr::u8(0),
                    Expr::var(&n),
                    vec![x_in.clone()],
                    carried(vec![(&x, &x_after)]),
                    unit_block(vec![assign(&x, vec![], plus_one(Expr::var(&x_in)), &x1)]),
                )),
            ],
            Expr::var(&x),
        ),
    };
    assert_eq!(
        session.declare_fn(&stale),
        Err(LowerError::StaleMention("x".into()))
    );
}

#[test]
fn a_break_in_a_for_or_a_while_carries_no_value() {
    let (mut session, _, _) = setup();
    let (item, _) = looping("breaks", |_, x, _| {
        let body = block(vec![], break_(Some(Expr::var(x))));
        (vec![], vec![], body)
    });
    assert_eq!(session.declare_fn(&item), Err(LowerError::BreakWithValue));
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
            derives: Vec::new(),
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

// --- Tracked evidence (M4): versions of proof type ---------------------------------------

/// `lemma zero_le(x) : 0 <= x` for bytes.
fn zero_le(theory: Theory, x: Term) -> Proof {
    lemma(
        theory.machine(MachineInt::U8).unsigned.unwrap().zero_le,
        vec![x],
    )
}

/// `let mut binder = value;` under a known equation.
fn let_mut_with(binder: &Binder, equation: HypId, value: Expr) -> Stmt {
    Stmt::Let {
        pattern: Pattern::Bind {
            binder: binder.clone(),
            equation,
            mutable: true,
        },
        value,
    }
}

/// `binding = value;` giving the binding the version `version`, under a
/// known equation.
fn assign_with(binding: &Binder, value: Expr, version: &Binder, equation: HypId) -> Stmt {
    Stmt::Assign {
        place: Place {
            binding: binding.id,
            name: binding.name.clone(),
            path: vec![],
        },
        value,
        version: version.clone(),
        equation,
    }
}

/// `@[x <= 3]`, tracked evidence about a byte.
fn at_most_three(x: &Binder) -> Type {
    Type::proof(u8_le(x.term(), Term::U8(3)))
}

/// Evidence of `version <= 3` after `version = 0` under `equation`: `0 <= 3`
/// carried along the equation.
fn refreshed(theory: Theory, version: &Binder, equation: HypId) -> Proof {
    Proof::transport(
        symm_at(&Type::U8, &version.term(), Proof::hyp(equation)),
        |hole| u8_le(hole, Term::U8(3)),
        zero_le(theory, Term::U8(3)),
    )
}

/// fn f(n: u8, small: @[n <= 3]) -> (out: u8, @[out <= 3]) {
///     let mut x = n;
///     let mut ok: @[x <= 3] = small;
///     x = 0;
///     <refresh>            // `ok = <0 <= 3 along x == 0>;`, or nothing
///     (x, ok)
/// }
/// The tree that skips the refresh passes the old `ok`, evidence about the
/// `x` of entry, where `x <= 3` about the new one is wanted. Lowering
/// accepts it: the old version is the current version of `ok`. The checker
/// rejects it, and that is the whole safety of tracked evidence.
fn stale_or_refreshed(theory: Theory, refresh: bool) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let small = Binder::new("small", Type::proof(u8_le(n.term(), Term::U8(3))));
    let x = Binder::new("x", Type::U8);
    let ok = Binder::new("ok", at_most_three(&x));
    let x1 = Binder::new("x", Type::U8);
    let ok1 = Binder::new("ok", at_most_three(&x1));
    let (x_is_n, x_is_zero) = (HypId::fresh(), HypId::fresh());
    // `small` is about `n`; `ok` is declared about `x`, which is `n`.
    let about_x = Proof::transport(
        symm_at(&Type::U8, &x.term(), Proof::hyp(x_is_n)),
        |hole| u8_le(hole, Term::U8(3)),
        Proof::OfTerm(small.term()),
    );
    let mut stmts = vec![
        let_mut_with(&x, x_is_n, Expr::var(&n)),
        let_mut(&ok, Expr::Proof(about_x)),
        assign_with(&x, Expr::u8(0), &x1, x_is_zero),
    ];
    let current = if refresh {
        stmts.push(assign(
            &ok,
            vec![],
            Expr::Proof(refreshed(theory, &x1, x_is_zero)),
            &ok1,
        ));
        ok1
    } else {
        ok
    };
    FnItem {
        name: "stale_or_refreshed".into(),
        math: false,
        params: vec![n, small],
        result: data_with_evidence(|out| u8_le(out, Term::U8(3))),
        body: block(
            stmts,
            Expr::Tuple {
                ty: data_with_evidence(|out| u8_le(out, Term::U8(3))),
                fields: vec![Expr::var(&x1), Expr::var(&current)],
            },
        ),
    }
}

#[test]
fn a_stale_use_of_tracked_evidence_is_rejected_by_the_checker_and_not_by_lowering() {
    let (mut session, _, theory) = setup();
    assert!(
        session
            .declare_fn(&stale_or_refreshed(theory, true))
            .is_ok()
    );
    // A type mismatch from the checker: the old `ok` speaks of the old `x`.
    assert!(matches!(
        session.declare_fn(&stale_or_refreshed(theory, false)),
        Err(LowerError::Exec(ExecError::Kernel(_)))
    ));
}

/// fn count(limit: u8) -> (out: u8, @[0 <= out]) {
///     let mut i = 0;
///     let mut ok: @[0 <= i] = zero_le(0);
///     loop {
///         if i == limit { break (i, ok) } else { i = i + 1; ok = <refresh>; }
///     }
/// }
/// with the loop's state `(i, ok: @[0 <= i])` typed over the `i` the body
/// sees, or, when `over_entry`, over the `i` of entry, which lowering
/// rejects; and the refresh either `zero_le(i')` about the new `i` or the
/// old `ok`, which the checker rejects at the `continue`.
fn counting_with_evidence(theory: Theory, over_entry: bool, honest: bool) -> FnItem {
    let limit = Binder::new("limit", Type::U8);
    let i = Binder::new("i", Type::U8);
    let at_least_zero = |i: &Binder| Type::proof(u8_le(Term::U8(0), i.term()));
    let ok = Binder::new("ok", at_least_zero(&i));
    let (i_in, i_after) = versions(&i);
    let ok_in = Binder::new("ok", at_least_zero(if over_entry { &i } else { &i_in }));
    let ok_after = Binder::new("ok", at_least_zero(&i_after));
    let i1 = Binder::new("i", Type::U8);
    let ok1 = Binder::new("ok", at_least_zero(&i1));
    let value = data_with_evidence(|out| u8_le(Term::U8(0), out));
    let refresh = if honest {
        Expr::Proof(zero_le(theory, i1.term()))
    } else {
        Expr::var(&ok_in)
    };
    let stop = block(
        vec![],
        break_(Some(Expr::Tuple {
            ty: value.clone(),
            fields: vec![Expr::var(&i_in), Expr::var(&ok_in)],
        })),
    );
    let go = unit_block(vec![
        assign(&i, vec![], plus_one(Expr::var(&i_in)), &i1),
        assign(&ok, vec![], refresh, &ok1),
    ]);
    let body = Block {
        stmts: vec![],
        tail: Some(Box::new(if_(
            compare_u8(CompareOp::Eq, Expr::var(&i_in), Expr::var(&limit)),
            stop,
            go,
            Type::Tuple(vec![]),
            None,
        ))),
    };
    let i_is_zero = HypId::fresh();
    FnItem {
        name: "counting_with_evidence".into(),
        math: false,
        params: vec![limit],
        result: value.clone(),
        body: block(
            vec![
                let_mut_with(&i, i_is_zero, Expr::u8(0)),
                let_mut(
                    &ok,
                    Expr::Proof(Proof::transport(
                        symm_at(&Type::U8, &i.term(), Proof::hyp(i_is_zero)),
                        |hole| u8_le(Term::U8(0), hole),
                        zero_le(theory, Term::U8(0)),
                    )),
                ),
            ],
            loop_(
                vec![i_in, ok_in],
                carried(vec![(&i, &i_after), (&ok, &ok_after)]),
                value,
                body,
            ),
        ),
    }
}

#[test]
fn a_loop_carries_tracked_evidence_typed_over_the_versions_its_body_sees() {
    let (mut session, _, theory) = setup();
    let honest = counting_with_evidence(theory, false, true);
    for limit in [0, 7, 255] {
        let (out, _) = match agreed(&mut session.clone(), &honest, limit) {
            Value::Tuple(fields) => match fields.as_slice() {
                [Value::Int(_, out), proof] => (*out as u8, proof.clone()),
                _ => panic!("not a byte with evidence"),
            },
            other => panic!("{other:?}"),
        };
        assert_eq!(out, limit);
    }
    let reference = session.declare_fn(&honest).unwrap();
    assert_eq!(carried_arity(&session, reference), 2);
    // The state names the evidence over the `i` of entry: not the type
    // lowering gives the version the body sees.
    assert!(matches!(
        session.declare_fn(&counting_with_evidence(theory, true, true)),
        Err(LowerError::Kernel(KernelError::TypeMismatch { .. }))
    ));
    // The refresh is the old `ok`, about the old `i`: lowering passes it
    // on, and the checker rejects the tree.
    assert!(matches!(
        session.declare_fn(&counting_with_evidence(theory, false, false)),
        Err(LowerError::Exec(ExecError::Kernel(_)))
    ));
}

#[test]
fn a_binding_of_proof_type_may_be_left_out_of_what_a_loop_carries() {
    // fn f(n: u8) -> u8 {
    //     let mut x = n;
    //     let mut ok: @[0 <= x] = zero_le(x);
    //     for i in 0..n { x = x + 1; ok = zero_le(x'); }
    //     x
    // }
    // The tree carries `ok`, or leaves it out: both check, since the old
    // `ok` stays a fact about the old `x`, and nothing after reads `ok`.
    let (mut session, _, theory) = setup();
    let build = |carry_ok: bool| {
        let n = Binder::new("n", Type::U8);
        let x = Binder::new("x", Type::U8);
        let at_least_zero = |x: &Binder| Type::proof(u8_le(Term::U8(0), x.term()));
        let ok = Binder::new("ok", at_least_zero(&x));
        let (x_in, x_after) = versions(&x);
        let ok_in = Binder::new("ok", at_least_zero(&x_in));
        let ok_after = Binder::new("ok", at_least_zero(&x_after));
        let x1 = Binder::new("x", Type::U8);
        let ok1 = Binder::new("ok", at_least_zero(&x1));
        let body = unit_block(vec![
            assign(&x, vec![], plus_one(Expr::var(&x_in)), &x1),
            assign(&ok, vec![], Expr::Proof(zero_le(theory, x1.term())), &ok1),
        ]);
        let (state, joins) = if carry_ok {
            (vec![x_in, ok_in], vec![(&x, &x_after), (&ok, &ok_after)])
        } else {
            (vec![x_in], vec![(&x, &x_after)])
        };
        let carried = carried(joins);
        FnItem {
            name: if carry_ok { "carried" } else { "left_out" }.into(),
            math: false,
            params: vec![n.clone()],
            result: Type::U8,
            body: block(
                vec![
                    let_mut(&x, Expr::var(&n)),
                    let_mut(&ok, Expr::Proof(zero_le(theory, x.term()))),
                    Stmt::Expr(for_(
                        &Binder::new("i", Type::U8),
                        Expr::u8(0),
                        Expr::var(&n),
                        state,
                        carried,
                        body,
                    )),
                ],
                Expr::var(&x_after),
            ),
        }
    };
    for carry_ok in [false, true] {
        let item = build(carry_ok);
        for byte in [0, 3, 200] {
            assert_eq!(
                agreed(&mut session.clone(), &item, byte),
                Value::u8(byte.wrapping_add(byte))
            );
        }
        let reference = session.declare_fn(&item).unwrap();
        assert_eq!(
            carried_arity(&session, reference),
            if carry_ok { 2 } else { 1 }
        );
    }
}

/// fn clamp(n: u8) -> (out: u8, @[out <= 3]) {
///     let mut x = 0;
///     let mut ok: @[x <= 3] = <0 <= 3 along x == 0>;
///     if n == 0 { x = 0; <refresh>; } else { }
///     (x, ok)
/// }
/// The join names `x` and `ok`, `ok` typed over the joined `x`; the else
/// arm restates `ok` over the `x` of entry, which is still current there.
/// The then arm refreshes `ok` after assigning `x`, or does not, in which
/// case it supplies the old `ok` in its tuple, evidence about the `x` of
/// entry where `x <= 3` over the joined `x` is wanted: lowering passes it
/// on, and the checker rejects it.
fn joining_with_evidence(theory: Theory, refresh: bool) -> FnItem {
    let n = Binder::new("n", Type::U8);
    let x = Binder::new("x", Type::U8);
    let ok = Binder::new("ok", at_most_three(&x));
    let x_then = Binder::new("x", Type::U8);
    let ok_then = Binder::new("ok", at_most_three(&x_then));
    let ok_else = Binder::new("ok", at_most_three(&x));
    let x_join = Binder::new("x", Type::U8);
    let ok_join = Binder::new("ok", at_most_three(&x_join));
    let (x_is_zero, x_then_is_zero) = (HypId::fresh(), HypId::fresh());
    let otherwise = vec![assign(
        &ok,
        vec![],
        Expr::Proof(refreshed(theory, &x, x_is_zero)),
        &ok_else,
    )];
    let mut then = vec![assign_with(&x, Expr::u8(0), &x_then, x_then_is_zero)];
    if refresh {
        then.push(assign(
            &ok,
            vec![],
            Expr::Proof(refreshed(theory, &x_then, x_then_is_zero)),
            &ok_then,
        ));
    }
    let value = data_with_evidence(|out| u8_le(out, Term::U8(3)));
    FnItem {
        name: "joining_with_evidence".into(),
        math: false,
        params: vec![n.clone()],
        result: value.clone(),
        body: block(
            vec![
                let_mut_with(&x, x_is_zero, Expr::u8(0)),
                let_mut(&ok, Expr::Proof(refreshed(theory, &x, x_is_zero))),
                Stmt::Expr(if_(
                    is_zero(&n),
                    unit_block(then),
                    unit_block(otherwise),
                    Type::Tuple(vec![]),
                    Some(joined(vec![(&x, &x_join), (&ok, &ok_join)])),
                )),
            ],
            Expr::Tuple {
                ty: value,
                fields: vec![Expr::var(&x_join), Expr::var(&ok_join)],
            },
        ),
    }
}

#[test]
fn a_join_carries_tracked_evidence_typed_over_the_joined_versions() {
    let (mut session, _, theory) = setup();
    let honest = joining_with_evidence(theory, true);
    for byte in [0, 1] {
        assert!(matches!(
            agreed(&mut session.clone(), &honest, byte),
            Value::Tuple(fields) if matches!(fields.as_slice(), [Value::Int(_, 0), _])
        ));
    }
    let reference = session.declare_fn(&honest).unwrap();
    // The tuple: x, ok, and the value of the branch.
    let (arity, _) = joining_match(&session, reference);
    assert_eq!(arity, 3);
    assert!(matches!(
        session.declare_fn(&joining_with_evidence(theory, false)),
        Err(LowerError::Exec(ExecError::Kernel(_)))
    ));
}
