//! Erasure, the erased tree's type checker, and the reference interpreter
//! (Architecture in atlas.html): the specification's programs, erased and run.

mod common;

use common::*;
use locus::erased::{
    EArm, EBlock, EExpr, EFn, EPattern, EPlace, EStmt, EType, Interpreter, Module, Outcome,
    RunError, TypeError, Value, check_module,
};
use locus::kernel::{Definitions, FnId, MachineInt, Op, Prim, Proof, Term, Type, VarId};
use locus::typed::{Binder, CompareOp, Expr, FnItem, FnRef, PanicForm};

const FUEL: u64 = 100_000;

fn run(module: &Module, callee: FnRef, arguments: Vec<Value>) -> Result<Outcome, RunError> {
    Interpreter::new(module, FUEL).call(callee, arguments)
}

/// Some function identity, for a module entry that nothing calls.
fn lonely_function() -> FnId {
    Definitions::new()
        .declare_fn(&Type::function(0, |_| Type::U8), |_| Term::U8(0))
        .unwrap()
}

fn with_evidence(byte: u8) -> Outcome {
    Outcome::Value(Value::Tuple(vec![Value::u8(byte), Value::Proved]))
}

#[test]
fn erasure_keeps_the_shape_and_fills_ghost_positions_with_markers() {
    let (mut session, _, _) = setup();
    let increment = session.declare_fn(&increment(false)).unwrap();
    let module = session.erased();
    assert_eq!(check_module(module), Ok(()));

    // fn increment(n: u8) -> (u8, Proved) { let out = n.wrapping_add(1); (out, Proved) }
    let function = &module.fns[0];
    assert_eq!(function.name, "increment");
    assert_eq!(
        function.result,
        EType::Tuple(vec![EType::Int(MachineInt::U8), EType::Proved])
    );
    assert_eq!(function.params.len(), 1);
    assert_eq!(function.body.stmts.len(), 1);
    let Some(EExpr::Tuple(fields)) = function.body.tail.as_deref() else {
        panic!("the tail is still a tuple")
    };
    assert!(matches!(
        fields.as_slice(),
        [EExpr::Var { .. }, EExpr::Proved]
    ));

    assert_eq!(
        run(module, increment, vec![Value::u8(41)]),
        Ok(with_evidence(42))
    );
    assert_eq!(
        run(module, increment, vec![Value::u8(255)]),
        Ok(with_evidence(0))
    );
}

#[test]
fn the_programs_of_the_specification_run() {
    let (mut session, _, theory) = setup();
    let preserve_fn = session.declare_fn(&preserve(theory, false, true)).unwrap();
    let preserve_math = session.declare_fn(&preserve(theory, true, true)).unwrap();
    let walk = session.declare_fn(&bounded_walk(theory, true)).unwrap();
    let count_pure = session.declare_fn(&counting_loop(false, None)).unwrap();
    let increment_id = exec_id(session.declare_fn(&increment(false)).unwrap());
    let count_calls = session
        .declare_fn(&counting_loop(false, Some(increment_id)))
        .unwrap();
    let module = session.erased();
    assert_eq!(check_module(module), Ok(()));

    for byte in [0u8, 1, 7, 200, 255] {
        let argument = vec![Value::u8(byte)];
        for callee in [preserve_fn, preserve_math, walk] {
            assert_eq!(
                run(module, callee, argument.clone()),
                Ok(with_evidence(byte)),
                "{callee:?} at {byte}"
            );
        }
        for callee in [count_pure, count_calls] {
            assert_eq!(
                run(module, callee, argument.clone()),
                Ok(Outcome::Value(Value::u8(byte))),
                "{callee:?} at {byte}"
            );
        }
    }
}

#[test]
fn a_divergent_call_still_diverges_after_erasure() {
    // fn caller() -> u8 { let impossible = spin(); 0 }
    // The proof spin advertises is erased; the call is not. The caller must
    // run out of fuel, not return 0.
    let (mut session, prelude, _) = setup();
    let spin_id = exec_id(session.declare_fn(&spin(prelude)).unwrap());
    let caller = session
        .declare_fn(&caller_of_spin(prelude, spin_id))
        .unwrap();
    let module = session.erased();
    assert_eq!(check_module(module), Ok(()));

    assert_eq!(run(module, caller, vec![]), Ok(Outcome::OutOfFuel));
    assert_eq!(
        run(module, FnRef::Exec(spin_id), vec![]),
        Ok(Outcome::OutOfFuel)
    );
    // The erased caller really does contain the call.
    let body = &module.fns[1].body;
    assert!(matches!(
        body.stmts.as_slice(),
        [EStmt::Let {
            value: EExpr::Call { .. },
            ..
        }]
    ));
}

#[test]
fn a_panic_ends_the_call_and_everything_around_it() {
    // fn increment(n: u8) -> (u8, Proved) {
    //     let out = n.wrapping_add(panic!("no sum"));
    //     (out, Proved)
    // }
    // fn caller(n: u8) -> (u8, Proved) { increment(n) }
    let (mut session, _, _) = setup();
    let increment = session.declare_fn(&increment(false)).unwrap();
    let mut module = session.erased().clone();
    let [
        EStmt::Let {
            value: EExpr::Method { arguments, .. },
            ..
        },
    ] = module.fns[0].body.stmts.as_mut_slice()
    else {
        panic!("increment binds a sum")
    };
    arguments[0] = EExpr::Panic {
        form: PanicForm::Panic,
        argument: Some("no sum".into()),
    };
    let mut caller = module.fns[0].clone();
    caller.name = "caller".into();
    caller.reference = FnRef::Math(lonely_function());
    caller.body = EBlock {
        stmts: vec![],
        tail: Some(Box::new(EExpr::Call {
            callee: increment,
            name: "increment".into(),
            arguments: vec![first_parameter(&caller)],
        })),
    };
    let caller_ref = caller.reference;
    module.fns.push(caller);
    // A panic is accepted where a `u8` is expected.
    assert_eq!(check_module(&module), Ok(()));

    let panicked = Ok(Outcome::Panic("no sum".into()));
    assert_eq!(run(&module, increment, vec![Value::u8(1)]), panicked);
    assert_eq!(run(&module, caller_ref, vec![Value::u8(1)]), panicked);
}

/// The first parameter of a function, as an expression.
fn first_parameter(function: &EFn) -> EExpr {
    let (id, name, _) = &function.params[0];
    EExpr::Var {
        id: *id,
        name: name.clone(),
    }
}

#[test]
fn out_of_fuel_is_not_a_panic() {
    // fn spin() -> ... never returns and never panics. With any amount of
    // fuel the answer is out of fuel, which is an outcome of its own: not an
    // error, and not a panic with some message.
    let (mut session, prelude, _) = setup();
    let spin_ref = session.declare_fn(&spin(prelude)).unwrap();
    let module = session.erased();
    for fuel in [0, 1, 1_000] {
        let outcome = Interpreter::new(module, fuel).call(spin_ref, vec![]);
        assert_eq!(outcome, Ok(Outcome::OutOfFuel));
    }
    // A panic that is reached before the fuel runs out is a panic, and one
    // that is not reached is out of fuel.
    let mut module = module.clone();
    module.fns[0].body = EBlock {
        stmts: vec![EStmt::Expr(EExpr::Tuple(vec![]))],
        tail: Some(Box::new(EExpr::Panic {
            form: PanicForm::Panic,
            argument: Some("reached".into()),
        })),
    };
    assert_eq!(check_module(&module), Ok(()));
    assert_eq!(
        Interpreter::new(&module, FUEL).call(spin_ref, vec![]),
        Ok(Outcome::Panic("reached".into()))
    );
    assert_eq!(
        Interpreter::new(&module, 1).call(spin_ref, vec![]),
        Ok(Outcome::OutOfFuel)
    );
}

#[test]
fn a_function_with_no_runtime_form_is_not_emitted() {
    let (mut session, _, _) = setup();
    // math fn self_equal(n: u8) -> @[n == n] { _ }      a lemma
    let n = Binder::new("n", Type::U8);
    let claim = u8_eq(n.term(), n.term());
    let lemma_item = FnItem {
        name: "self_equal".into(),
        math: true,
        params: vec![n.clone()],
        result: Type::proof(claim.clone()),
        body: block(vec![], Expr::Proof(Proof::Refl(n.term()))),
    };
    let FnRef::Math(lemma_id) = session.declare_fn(&lemma_item).unwrap() else {
        panic!()
    };
    // fn uses_lemma(m: u8) -> (u8, @[m == m]) { (m, self_equal(m)) }
    let m = Binder::new("m", Type::U8);
    let m_term = m.term();
    let result = Type::Tuple(vec![
        Type::U8,
        Type::proof(u8_eq(m_term.clone(), m_term.clone())),
    ]);
    let user = FnItem {
        name: "uses_lemma".into(),
        math: false,
        params: vec![m.clone()],
        result: result.clone(),
        body: block(
            vec![],
            Expr::Tuple {
                ty: result,
                fields: vec![
                    Expr::var(&m),
                    Expr::CallMath {
                        id: lemma_id,
                        name: "self_equal".into(),
                        arguments: vec![Expr::var(&m)],
                        ty: Type::proof(u8_eq(m_term.clone(), m_term)),
                    },
                ],
            },
        ),
    };
    let user_ref = session.declare_fn(&user).unwrap();
    let module = session.erased();
    assert_eq!(check_module(module), Ok(()));
    let names: Vec<&str> = module.fns.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["uses_lemma"]);
    // The call to the lemma became its marker.
    assert_eq!(
        run(module, user_ref, vec![Value::u8(9)]),
        Ok(with_evidence(9))
    );
}

#[test]
fn the_type_checker_guards_erasure() {
    let (mut session, _, _) = setup();
    let increment = session.declare_fn(&increment(false)).unwrap();
    let good = session.erased().clone();
    assert_eq!(check_module(&good), Ok(()));

    let broken = |edit: fn(&mut EFn)| {
        let mut module = good.clone();
        edit(&mut module.fns[0]);
        check_module(&module)
    };
    // A marker where data belongs.
    assert!(
        broken(|f| {
            f.body.tail = Some(Box::new(EExpr::Tuple(vec![EExpr::Proved, EExpr::Proved])));
        })
        .is_err()
    );
    // A ghost position dropped instead of filled.
    assert!(broken(|f| f.result = EType::Int(MachineInt::U8)).is_err());
    // A reference to something that is not in scope.
    assert!(
        broken(|f| {
            f.body = EBlock {
                stmts: vec![],
                tail: f.body.tail.clone(),
            };
        })
        .is_err()
    );
    // A call to a function that was not emitted: keep only a caller.
    let mut module = good.clone();
    let mut caller = module.fns[0].clone();
    caller.name = "caller".into();
    caller.reference = FnRef::Math(lonely_function());
    caller.body = EBlock {
        stmts: vec![],
        tail: Some(Box::new(EExpr::Call {
            callee: increment,
            name: "increment".into(),
            arguments: vec![EExpr::Literal(MachineInt::U8, 1)],
        })),
    };
    module.fns = vec![caller];
    assert!(check_module(&module).is_err());
}

// --- Return, and what a never-yielding value excuses ------------------------------

/// `fn f(n: u8) -> u8 { body }`, with `n` the parameter of `increment`.
fn u8_to_u8(session: &locus::typed::Session, body: EBlock) -> Module {
    let mut module = session.erased().clone();
    module.fns.truncate(1);
    module.fns[0].result = EType::Int(MachineInt::U8);
    module.fns[0].body = body;
    module
}

fn wrapping_add(receiver: EExpr, argument: EExpr) -> EExpr {
    EExpr::Method {
        prim: Prim::Op(Op::WrappingAdd, MachineInt::U8),
        receiver: Box::new(receiver),
        arguments: vec![argument],
    }
}

/// A name with its type, as a pattern and as the variable it binds.
fn bind(name: &str, ty: EType) -> (EPattern, EExpr) {
    let id = VarId::fresh();
    let pattern = EPattern::Bind {
        id,
        name: name.into(),
        ty,
        mutable: false,
    };
    let var = EExpr::Var {
        id,
        name: name.into(),
    };
    (pattern, var)
}

fn tail(expr: EExpr) -> EBlock {
    EBlock {
        stmts: vec![],
        tail: Some(Box::new(expr)),
    }
}

#[test]
fn a_return_leaves_the_function_from_inside_a_loop_and_has_the_result_type() {
    // fn f(n: u8) -> u8 {
    //     let mut i = 0;
    //     loop {
    //         if i == n { return <returned> } else { i = i.wrapping_add(1); }
    //     }
    // }
    let (mut session, _, _) = setup();
    let reference = session.declare_fn(&increment(false)).unwrap();
    let n = first_parameter(&session.erased().fns[0]);
    let i_id = VarId::fresh();
    let i = || EExpr::Var {
        id: i_id,
        name: "i".into(),
    };
    let looping = |returned: EExpr| {
        u8_to_u8(
            &session,
            EBlock {
                stmts: vec![EStmt::Let {
                    pattern: EPattern::Bind {
                        id: i_id,
                        name: "i".into(),
                        ty: EType::Int(MachineInt::U8),
                        mutable: true,
                    },
                    value: EExpr::Literal(MachineInt::U8, 0),
                }],
                tail: Some(Box::new(EExpr::Loop {
                    result: EType::Int(MachineInt::U8),
                    body: tail(EExpr::If {
                        condition: Box::new(EExpr::Compare {
                            op: CompareOp::Eq,
                            left: Box::new(i()),
                            right: Box::new(n.clone()),
                        }),
                        then_block: tail(EExpr::Return(Box::new(returned))),
                        else_block: EBlock {
                            stmts: vec![EStmt::Assign {
                                place: EPlace {
                                    id: i_id,
                                    name: "i".into(),
                                    path: vec![],
                                },
                                value: wrapping_add(i(), EExpr::Literal(MachineInt::U8, 1)),
                            }],
                            tail: None,
                        },
                    }),
                })),
            },
        )
    };
    let module = looping(wrapping_add(i(), EExpr::Literal(MachineInt::U8, 100)));
    assert_eq!(check_module(&module), Ok(()));
    assert_eq!(
        run(&module, reference, vec![Value::u8(0)]),
        Ok(Outcome::Value(Value::u8(100)))
    );
    // Three iterations, then the return.
    assert_eq!(
        run(&module, reference, vec![Value::u8(3)]),
        Ok(Outcome::Value(Value::u8(103)))
    );

    // A return must supply the function's result type.
    let wrong = looping(EExpr::Bool(true));
    assert_eq!(
        check_module(&wrong),
        Err(TypeError(
            "a returned value has type Bool, expected Int(U8)".into()
        ))
    );
}

#[test]
fn what_follows_a_value_that_never_yields_is_still_checked() {
    let (mut session, prelude, _) = setup();
    session.declare_enum(&classified_enum(prelude)).unwrap();
    session.declare_fn(&increment(false)).unwrap();
    let panics = || EExpr::Panic {
        form: PanicForm::Panic,
        argument: Some("never".into()),
    };

    // let m: u8 = panic!("never"); <rest>; m.wrapping_add(1)
    let sequel = |first: EExpr, rest: EExpr| {
        let (m, m_var) = bind("m", EType::Int(MachineInt::U8));
        u8_to_u8(
            &session,
            EBlock {
                stmts: vec![
                    EStmt::Let {
                        pattern: m,
                        value: first,
                    },
                    EStmt::Expr(rest),
                ],
                tail: Some(Box::new(wrapping_add(
                    m_var,
                    EExpr::Literal(MachineInt::U8, 1),
                ))),
            },
        )
    };
    assert_eq!(
        check_module(&sequel(panics(), EExpr::Literal(MachineInt::U8, 0))),
        Ok(())
    );
    // The statement after the panic is checked.
    let ill = sequel(
        panics(),
        wrapping_add(EExpr::Bool(true), EExpr::Literal(MachineInt::U8, 1)),
    );
    assert!(check_module(&ill).is_err());
    // So is the tail, through the type the name carries.
    let (b, b_var) = bind("b", EType::Bool);
    let tail_ill = u8_to_u8(
        &session,
        EBlock {
            stmts: vec![EStmt::Let {
                pattern: b,
                value: panics(),
            }],
            tail: Some(Box::new(wrapping_add(
                b_var,
                EExpr::Literal(MachineInt::U8, 1),
            ))),
        },
    );
    assert!(check_module(&tail_ill).is_err());
    // The type a name carries must be the type of its value, when there is one.
    let (b, _) = bind("b", EType::Bool);
    let disagrees = u8_to_u8(
        &session,
        EBlock {
            stmts: vec![EStmt::Let {
                pattern: b,
                value: EExpr::Literal(MachineInt::U8, 1),
            }],
            tail: Some(Box::new(EExpr::Literal(MachineInt::U8, 2))),
        },
    );
    assert!(check_module(&disagrees).is_err());

    // A match on a scrutinee that never yields still has its arms checked,
    // against the enum the match names.
    let matching = |arm_value: EExpr| {
        let arm = |name: &str, value: EExpr| EArm {
            variant_name: name.into(),
            payload: vec![(VarId::fresh(), "v".into()), (VarId::fresh(), "h".into())],
            body: tail(value),
        };
        u8_to_u8(
            &session,
            tail(EExpr::Match {
                scrutinee: Box::new(panics()),
                enum_name: "Classified".into(),
                arms: vec![
                    arm("Zero", EExpr::Literal(MachineInt::U8, 0)),
                    arm("NonZero", arm_value),
                ],
            }),
        )
    };
    assert_eq!(
        check_module(&matching(EExpr::Literal(MachineInt::U8, 1))),
        Ok(())
    );
    assert!(check_module(&matching(EExpr::Bool(true))).is_err());
    // No arm is ever reached.
    let module = matching(EExpr::Literal(MachineInt::U8, 1));
    let reference = module.fns[0].reference;
    assert_eq!(
        run(&module, reference, vec![Value::u8(0)]),
        Ok(Outcome::Panic("never".into()))
    );
}
