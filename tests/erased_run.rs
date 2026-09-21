//! Erasure, the erased tree's type checker, and the reference interpreter
//! (Architecture in atlas.html): the specification's programs, erased and run.

mod common;

use common::*;
use locus::erased::{
    EBlock, EExpr, EFn, EStmt, EType, Interpreter, Module, RunError, Value, check_module,
};
use locus::kernel::{Definitions, FnId, Proof, Term, Type};
use locus::typed::{Binder, Expr, FnItem, FnRef};

const FUEL: u64 = 100_000;

fn run(module: &Module, callee: FnRef, arguments: Vec<Value>) -> Result<Value, RunError> {
    Interpreter::new(module, FUEL).call(callee, arguments)
}

/// Some function identity, for a module entry that nothing calls.
fn lonely_function() -> FnId {
    Definitions::new()
        .declare_fn(&Type::function(0, |_| Type::U8), |_| Term::U8(0))
        .unwrap()
}

fn with_evidence(byte: u8) -> Value {
    Value::Tuple(vec![Value::U8(byte), Value::Proved])
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
        EType::Tuple(vec![EType::U8, EType::Proved])
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
        run(module, increment, vec![Value::U8(41)]),
        Ok(with_evidence(42))
    );
    assert_eq!(
        run(module, increment, vec![Value::U8(255)]),
        Ok(with_evidence(0))
    );
}

#[test]
fn the_programs_of_the_specification_run() {
    let (mut session, prelude, theory) = setup();
    let preserve_fn = session.declare_fn(&preserve(false, true)).unwrap();
    let preserve_math = session.declare_fn(&preserve(true, true)).unwrap();
    let walk = session
        .declare_fn(&bounded_walk(prelude, theory, true))
        .unwrap();
    let count_pure = session
        .declare_fn(&counting_loop(theory, false, None))
        .unwrap();
    let count_math = session
        .declare_fn(&counting_loop(theory, true, None))
        .unwrap();
    let increment_id = exec_id(session.declare_fn(&increment(false)).unwrap());
    let count_calls = session
        .declare_fn(&counting_loop(theory, false, Some(increment_id)))
        .unwrap();
    let module = session.erased();
    assert_eq!(check_module(module), Ok(()));

    for byte in [0u8, 1, 7, 200, 255] {
        let argument = vec![Value::U8(byte)];
        for callee in [
            preserve_fn,
            preserve_math,
            walk,
            count_pure,
            count_math,
            count_calls,
        ] {
            assert_eq!(
                run(module, callee, argument.clone()),
                Ok(with_evidence(byte)),
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

    assert_eq!(run(module, caller, vec![]), Err(RunError::OutOfFuel));
    assert_eq!(
        run(module, FnRef::Exec(spin_id), vec![]),
        Err(RunError::OutOfFuel)
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
        run(module, user_ref, vec![Value::U8(9)]),
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
    assert!(broken(|f| f.result = EType::U8).is_err());
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
            arguments: vec![EExpr::U8(1)],
        })),
    };
    module.fns = vec![caller];
    assert!(check_module(&module).is_err());
}
