//! The two endings M1 added to the check IR, `return` and a panic, and the
//! four promises a function may make (Architecture in atlas.html), on
//! hand-built programs: what the checker accepts and rejects, and what the
//! check-IR interpreter then computes.

use std::rc::Rc;

use locus::erased::{Outcome, Value};
use locus::exec::{
    Arm, Block, CheckInterpreter, ExecError, ExecFn, ExecFnId, ForStmt, Program, Promise, Promises,
    Stmt, Tail,
};
use locus::kernel::theory::{self, Theory};
use locus::kernel::{
    Axiom, CmpOp, Definitions, HypId, KernelError, MachineInt, Op, Proof, Term, Type, VarId,
};
use locus::typed::FnRef;

const FUEL: u64 = 100_000;

struct World {
    definitions: Rc<Definitions>,
    theory: Theory,
}

fn world() -> World {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).expect("the theory checks");
    World {
        definitions: Rc::new(definitions),
        theory,
    }
}

fn u8_eq(left: Term, right: Term) -> Term {
    Term::eq(Type::U8, left, right)
}

fn var() -> (VarId, Term) {
    let id = VarId::fresh();
    (id, Term::var(id))
}

fn block(stmts: Vec<Stmt>, tail: Tail) -> Block {
    Block { stmts, tail }
}

fn arm(body: Block) -> Arm {
    Arm {
        payload: vec![],
        fact: HypId::fresh(),
        body,
    }
}

fn panic(message: &str, unreachable: Option<Proof>) -> Tail {
    Tail::Panic {
        message: message.into(),
        unreachable,
    }
}

/// `fn f(n: u8) -> u8 { body }`
fn u8_to_u8(n: VarId, body: Block) -> ExecFn {
    ExecFn {
        promises: Promises::default(),
        signature: Type::function(1, |_| Type::U8),
        params: vec![n],
        body,
    }
}

/// `fn f() -> u8 { body }`
fn u8_of_nothing(promises: Promises, body: Block) -> ExecFn {
    ExecFn {
        promises,
        signature: Type::function(0, |_| Type::U8),
        params: vec![],
        body,
    }
}

fn all_four() -> Promises {
    Promises {
        terminates: true,
        no_panic: true,
        no_alloc: true,
        no_io: true,
    }
}

fn only(promise: Promise) -> Promises {
    let mut promises = Promises::default();
    match promise {
        Promise::Terminates => promises.terminates = true,
        Promise::NoPanic => promises.no_panic = true,
        Promise::NoAlloc => promises.no_alloc = true,
        Promise::NoIo => promises.no_io = true,
    }
    promises
}

fn run(program: &Program, id: ExecFnId, arguments: Vec<Value>) -> Outcome {
    CheckInterpreter::new(program, FUEL)
        .call(FnRef::Exec(id), arguments)
        .expect("the program runs")
}

fn u8(value: u8) -> Outcome {
    Outcome::Value(Value::u8(value))
}

/// `(out: u8, @[out == n])`
fn preserved(n: &Term) -> Type {
    let n = n.clone();
    Type::tuple(move |earlier| match earlier {
        [] => Some(Type::U8),
        [out] => Some(Type::proof(u8_eq(out.clone(), n.clone()))),
        _ => None,
    })
}

// --- Return ------------------------------------------------------------------------

/// fn early(n: u8) -> (out: u8, @[out == n]) {
///     let m: (out: u8, @[out == n]) = match n == 0 {
///         false => (n, _),
///         true => return (0, evidence),
///     };
///     m
/// }
/// With the fact of the arm, `evidence` proves `0 == n`; without it, it
/// proves `0 == 0`, which is not what the result type asks of a return.
fn early_return(theory: Theory, use_the_fact: bool) -> ExecFn {
    let (n_id, n) = var();
    let (m_id, m) = var();
    let comparison = Term::cmp(CmpOp::Eq, MachineInt::U8, n.clone(), Term::U8(0));
    let when_true = HypId::fresh();
    // The fact reflects to an equality of the views, and the injectivity
    // of the view makes it one of the bytes.
    let views_equal = Proof::implies_elim(
        Proof::Axiom(Axiom::CmpReflect(comparison.clone(), true)),
        Proof::hyp(when_true),
    );
    let n_is_zero = Proof::OfTerm(Term::call(
        Term::Fn(theory.machine(MachineInt::U8).view_injective),
        vec![n.clone(), Term::U8(0), Term::proof(views_equal)],
    ));
    let target = n.clone();
    let zero_is_n = Proof::transport(
        n_is_zero,
        |hole| u8_eq(hole, target.clone()),
        Proof::Refl(n.clone()),
    );
    let evidence = if use_the_fact {
        zero_is_n
    } else {
        Proof::Refl(Term::U8(0))
    };
    ExecFn {
        promises: Promises::default(),
        signature: Type::function(1, |params| match params {
            [] => Type::U8,
            [n] => preserved(n),
            _ => unreachable!(),
        }),
        params: vec![n_id],
        body: block(
            vec![Stmt::Match {
                var: m_id,
                ty: preserved(&n),
                scrutinee: comparison,
                arms: vec![
                    arm(block(
                        vec![],
                        Tail::Value(Term::tuple(
                            &preserved(&n),
                            vec![n.clone(), Term::proof(Proof::Refl(n.clone()))],
                        )),
                    )),
                    Arm {
                        payload: vec![],
                        fact: when_true,
                        body: block(
                            vec![],
                            Tail::Return(Term::tuple(
                                &preserved(&n),
                                vec![Term::U8(0), Term::proof(evidence)],
                            )),
                        ),
                    },
                ],
            }],
            Tail::Value(m),
        ),
    }
}

#[test]
fn a_return_inside_a_branch_supplies_the_result_type_at_that_point() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let early = program.declare(early_return(world.theory, true)).unwrap();
    let with_evidence =
        |byte: u8| Outcome::Value(Value::Tuple(vec![Value::u8(byte), Value::Proved]));
    assert_eq!(run(&program, early, vec![Value::u8(0)]), with_evidence(0));
    assert_eq!(run(&program, early, vec![Value::u8(9)]), with_evidence(9));

    // Evidence about the wrong value is rejected at the return: the rest of
    // the function is the same as above and was accepted.
    assert!(matches!(
        program.declare(early_return(world.theory, false)),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
}

#[test]
fn a_return_of_the_wrong_type_is_rejected() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    // fn f() -> u8 { return true }
    let wrong = u8_of_nothing(
        Promises::default(),
        block(vec![], Tail::Return(Term::Bool(true))),
    );
    assert_eq!(
        program.declare(wrong).map(|_| ()),
        Err(ExecError::Kernel(KernelError::TypeMismatch {
            expected: Type::U8,
            found: Type::Bool,
        }))
    );
}

#[test]
fn a_return_inside_a_loop_inside_a_match_leaves_the_function() {
    // fn find(n: u8) -> u8 {
    //     let r: u8 = match n == 0 {
    //         false => {
    //             let w = loop (i: u8 = 0) -> u8 {
    //                 match i == n { false => continue(i + 1), true => return i + 100 }
    //             };
    //             w
    //         }
    //         true => 0,
    //     };
    //     r
    // }
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let (n_id, n) = var();
    let (r_id, r) = var();
    let (w_id, w) = var();
    let (i_id, i) = var();
    let searching = block(
        vec![Stmt::Loop {
            var: w_id,
            state: Type::Tuple(vec![Type::U8]),
            vars: vec![i_id],
            init: vec![Term::U8(0)],
            result: Type::U8,
            body: block(
                vec![],
                Tail::Match {
                    scrutinee: Term::cmp(CmpOp::Eq, MachineInt::U8, i.clone(), n.clone()),
                    arms: vec![
                        arm(block(
                            vec![],
                            Tail::Continue(vec![Term::op(
                                Op::WrappingAdd,
                                MachineInt::U8,
                                vec![i.clone(), Term::U8(1)],
                            )]),
                        )),
                        arm(block(
                            vec![],
                            Tail::Return(Term::op(
                                Op::WrappingAdd,
                                MachineInt::U8,
                                vec![i, Term::U8(100)],
                            )),
                        )),
                    ],
                },
            ),
        }],
        Tail::Value(w),
    );
    let find = u8_to_u8(
        n_id,
        block(
            vec![Stmt::Match {
                var: r_id,
                ty: Type::U8,
                scrutinee: Term::cmp(CmpOp::Eq, MachineInt::U8, n, Term::U8(0)),
                arms: vec![arm(searching), arm(block(vec![], Tail::Value(Term::U8(0))))],
            }],
            Tail::Value(r),
        ),
    );
    let find = program.declare(find).unwrap();
    assert_eq!(run(&program, find, vec![Value::u8(0)]), u8(0));
    // Three iterations, then the return.
    assert_eq!(run(&program, find, vec![Value::u8(3)]), u8(103));
}

#[test]
fn a_return_inside_a_bounded_for_leaves_the_function() {
    // fn f(n: u8) -> u8 {
    //     let done = for i in 0..n () { match i == 2 { false => continue(), true => return 99 } };
    //     0
    // }
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let (n_id, n) = var();
    let (done_id, _) = var();
    let (i_id, i) = var();
    let f = u8_to_u8(
        n_id,
        block(
            vec![Stmt::For(Box::new(ForStmt {
                var: done_id,
                index: i_id,
                lower: HypId::fresh(),
                upper: HypId::fresh(),
                lo: Term::U8(0),
                hi: n.clone(),
                inclusive: false,
                state: Type::Tuple(vec![]),
                vars: vec![],
                init: vec![],
                body: block(
                    vec![],
                    Tail::Match {
                        scrutinee: Term::cmp(CmpOp::Eq, MachineInt::U8, i, Term::U8(2)),
                        arms: vec![
                            arm(block(vec![], Tail::Continue(vec![]))),
                            arm(block(vec![], Tail::Return(Term::U8(99)))),
                        ],
                    },
                ),
            }))],
            Tail::Value(Term::U8(0)),
        ),
    );
    let f = program.declare(f).unwrap();
    assert_eq!(run(&program, f, vec![Value::u8(2)]), u8(0));
    assert_eq!(run(&program, f, vec![Value::u8(5)]), u8(99));
}

#[test]
fn a_function_whose_every_path_returns_early() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let (b_id, b) = var();
    let (m_id, m) = var();
    let both_return = || {
        vec![
            arm(block(vec![], Tail::Return(Term::U8(1)))),
            arm(block(vec![], Tail::Return(Term::U8(2)))),
        ]
    };
    let signature = || {
        Type::function(1, |params| match params {
            [] => Type::Bool,
            _ => Type::U8,
        })
    };
    // fn in_tail(b: bool) -> u8 { match b { false => return 1, true => return 2 } }
    let in_tail = ExecFn {
        promises: Promises::default(),
        signature: signature(),
        params: vec![b_id],
        body: block(
            vec![],
            Tail::Match {
                scrutinee: b.clone(),
                arms: both_return(),
            },
        ),
    };
    // fn as_statement(b: bool) -> u8 {
    //     let m: u8 = match b { false => return 1, true => return 2 };
    //     m                       // never reached; m is declared all the same
    // }
    let as_statement = ExecFn {
        promises: Promises::default(),
        signature: signature(),
        params: vec![b_id],
        body: block(
            vec![Stmt::Match {
                var: m_id,
                ty: Type::U8,
                scrutinee: b,
                arms: both_return(),
            }],
            Tail::Value(m),
        ),
    };
    for function in [in_tail, as_statement] {
        let id = program.declare(function).unwrap();
        assert_eq!(run(&program, id, vec![Value::Bool(false)]), u8(1));
        assert_eq!(run(&program, id, vec![Value::Bool(true)]), u8(2));
    }
}

#[test]
fn a_return_counts_as_not_falling_through_and_a_value_still_does() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let (w_id, w) = var();
    let looping = |tail: Tail| {
        u8_of_nothing(
            Promises::default(),
            block(
                vec![Stmt::Loop {
                    var: w_id,
                    state: Type::Tuple(vec![]),
                    vars: vec![],
                    init: vec![],
                    result: Type::U8,
                    body: block(vec![], tail),
                }],
                Tail::Value(w.clone()),
            ),
        )
    };
    let returns = program.declare(looping(Tail::Return(Term::U8(4)))).unwrap();
    assert_eq!(run(&program, returns, vec![]), u8(4));
    let panics = program.declare(looping(panic("in a loop", None))).unwrap();
    assert_eq!(
        run(&program, panics, vec![]),
        Outcome::Panic("in a loop".into())
    );
    assert_eq!(
        program
            .declare(looping(Tail::Value(Term::U8(4))))
            .map(|_| ()),
        Err(ExecError::FallsThrough)
    );
}

// --- Panic -------------------------------------------------------------------------

/// fn guarded(n: u8, h: @[n == 0]) -> u8 {
///     match n == 0 { false => panic!("impossible"), true => 0 }
/// }
/// In the false arm the fact `(n == 0) == false` and the hypothesis `n == 0`,
/// stated over the views as reflection gives it, contradict each other,
/// which is where the proof of `False` comes from.
fn guarded(promises: Promises, unreachable: fn(Proof, Proof) -> Option<Proof>) -> ExecFn {
    let (n_id, n) = var();
    let (h_id, h) = var();
    let comparison = Term::cmp(CmpOp::Eq, MachineInt::U8, n.clone(), Term::U8(0));
    let when_false = HypId::fresh();
    let n_is_not_zero = Proof::implies_elim(
        Proof::Axiom(Axiom::CmpReflect(comparison.clone(), false)),
        Proof::hyp(when_false),
    );
    ExecFn {
        promises,
        signature: Type::function(2, |params| match params {
            [] => Type::U8,
            [n] => Type::proof(Term::eq(
                Type::Int,
                Term::view(MachineInt::U8, n.clone()),
                Term::view(MachineInt::U8, Term::U8(0)),
            )),
            _ => Type::U8,
        }),
        params: vec![n_id, h_id],
        body: block(
            vec![],
            Tail::Match {
                scrutinee: comparison,
                arms: vec![
                    Arm {
                        payload: vec![],
                        fact: when_false,
                        body: block(
                            vec![],
                            panic("impossible", unreachable(n_is_not_zero, Proof::OfTerm(h))),
                        ),
                    },
                    arm(block(vec![], Tail::Value(Term::U8(0)))),
                ],
            },
        ),
    }
}

#[test]
fn a_panic_inside_a_branch_demands_nothing_without_the_promise() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let guarded_id = program
        .declare(guarded(Promises::default(), |_, _| None))
        .unwrap();
    assert_eq!(
        run(&program, guarded_id, vec![Value::u8(0), Value::Proved]),
        u8(0)
    );
    // The interpreter does not look at the evidence.
    assert_eq!(
        run(&program, guarded_id, vec![Value::u8(1), Value::Proved]),
        Outcome::Panic("impossible".into())
    );
    assert_eq!(program.promises(guarded_id), Some(Promises::default()));
}

#[test]
fn under_no_panic_a_panic_needs_a_proof_that_it_is_unreachable() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let no_panic = only(Promise::NoPanic);
    assert_eq!(
        program.declare(guarded(no_panic, |_, _| None)).map(|_| ()),
        Err(ExecError::PanicUnderNoPanic)
    );
    // (n == 0 => False) applied to n == 0.
    let id = program
        .declare(guarded(no_panic, |negation, h| {
            Some(Proof::implies_elim(negation, h))
        }))
        .unwrap();
    assert_eq!(program.promises(id), Some(no_panic));
    assert_eq!(run(&program, id, vec![Value::u8(0), Value::Proved]), u8(0));
    // A proof of something else is not a proof of False.
    assert!(matches!(
        program.declare(guarded(no_panic, |negation, _| Some(negation))),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
    // A wrong proof is rejected even when nothing was promised.
    assert!(matches!(
        program.declare(guarded(Promises::default(), |negation, _| Some(negation))),
        Err(ExecError::Kernel(KernelError::ProofMismatch { .. }))
    ));
}

// --- Promises ------------------------------------------------------------------------

/// `let r = callee(); r`
fn calls(promises: Promises, callee: ExecFnId) -> ExecFn {
    let (r_id, r) = var();
    u8_of_nothing(
        promises,
        block(
            vec![Stmt::Call {
                var: r_id,
                callee,
                arguments: vec![],
            }],
            Tail::Value(r),
        ),
    )
}

#[test]
fn each_promise_is_kept_only_by_calling_functions_that_make_it() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let plain = program
        .declare(u8_of_nothing(
            Promises::default(),
            block(vec![], Tail::Value(Term::U8(1))),
        ))
        .unwrap();
    let dependable = program
        .declare(u8_of_nothing(
            all_four(),
            block(vec![], Tail::Value(Term::U8(1))),
        ))
        .unwrap();
    assert_eq!(program.promises(dependable), Some(all_four()));
    for promise in Promise::ALL {
        assert_eq!(
            program.declare(calls(only(promise), plain)).map(|_| ()),
            Err(ExecError::CalleeBreaksPromise {
                promise,
                callee: plain
            }),
            "{}",
            promise.name()
        );
        let kept = program.declare(calls(only(promise), dependable)).unwrap();
        assert_eq!(program.promises(kept), Some(only(promise)));
        assert_eq!(run(&program, kept, vec![]), u8(1));
    }
    // A function that promises nothing may call anything.
    let careless = program.declare(calls(Promises::default(), plain)).unwrap();
    assert_eq!(run(&program, careless, vec![]), u8(1));
    // Every promise at once is refused by the first one broken.
    assert_eq!(
        program.declare(calls(all_four(), plain)).map(|_| ()),
        Err(ExecError::CalleeBreaksPromise {
            promise: Promise::Terminates,
            callee: plain
        })
    );
}

#[test]
fn terminates_forbids_a_loop_wherever_it_stands() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let (b_id, b) = var();
    let (m_id, m) = var();
    let (w_id, w) = var();
    // fn f(b: bool) -> u8 {
    //     let m: u8 = match b { false => { let w = loop () -> u8 { break 1 }; w }, true => 2 };
    //     m
    // }
    let loop_in_an_arm = |promises: Promises| ExecFn {
        promises,
        signature: Type::function(1, |params| match params {
            [] => Type::Bool,
            _ => Type::U8,
        }),
        params: vec![b_id],
        body: block(
            vec![Stmt::Match {
                var: m_id,
                ty: Type::U8,
                scrutinee: b.clone(),
                arms: vec![
                    arm(block(
                        vec![Stmt::Loop {
                            var: w_id,
                            state: Type::Tuple(vec![]),
                            vars: vec![],
                            init: vec![],
                            result: Type::U8,
                            body: block(vec![], Tail::Break(Term::U8(1))),
                        }],
                        Tail::Value(w.clone()),
                    )),
                    arm(block(vec![], Tail::Value(Term::U8(2)))),
                ],
            }],
            Tail::Value(m.clone()),
        ),
    };
    assert!(program.declare(loop_in_an_arm(Promises::default())).is_ok());
    assert_eq!(
        program
            .declare(loop_in_an_arm(only(Promise::Terminates)))
            .map(|_| ()),
        Err(ExecError::LoopUnderTerminates)
    );

    // fn g(n: u8) -> u8 { let done = for i in 0..n () { continue() }; 0 }
    let (n_id, n) = var();
    let (done_id, _) = var();
    let bounded = |promises: Promises| ExecFn {
        promises,
        ..u8_to_u8(
            n_id,
            block(
                vec![Stmt::For(Box::new(ForStmt {
                    var: done_id,
                    index: VarId::fresh(),
                    lower: HypId::fresh(),
                    upper: HypId::fresh(),
                    lo: Term::U8(0),
                    hi: n.clone(),
                    inclusive: false,
                    state: Type::Tuple(vec![]),
                    vars: vec![],
                    init: vec![],
                    body: block(vec![], Tail::Continue(vec![])),
                }))],
                Tail::Value(Term::U8(0)),
            ),
        )
    };
    assert!(program.declare(bounded(Promises::default())).is_ok());
    assert_eq!(
        program
            .declare(bounded(only(Promise::Terminates)))
            .map(|_| ()),
        Err(ExecError::LoopUnderTerminates)
    );
}

#[test]
fn the_other_promises_do_not_mind_a_loop_or_a_panic() {
    let world = world();
    let mut program = Program::new((*world.definitions).clone());
    let (w_id, w) = var();
    let mut promises = all_four();
    promises.terminates = false;
    // fn f() -> u8 { let w = loop () -> u8 { break 1 }; w }
    let looping = ExecFn {
        promises,
        ..u8_of_nothing(
            promises,
            block(
                vec![Stmt::Loop {
                    var: w_id,
                    state: Type::Tuple(vec![]),
                    vars: vec![],
                    init: vec![],
                    result: Type::U8,
                    body: block(vec![], Tail::Break(Term::U8(1))),
                }],
                Tail::Value(w),
            ),
        )
    };
    let id = program.declare(looping).unwrap();
    assert_eq!(run(&program, id, vec![]), u8(1));

    let mut promises = all_four();
    promises.no_panic = false;
    let panicking = u8_of_nothing(promises, block(vec![], panic("allowed", None)));
    let id = program.declare(panicking).unwrap();
    assert_eq!(run(&program, id, vec![]), Outcome::Panic("allowed".into()));
}
