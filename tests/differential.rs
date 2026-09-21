//! The two branches from the typed tree must agree (Architecture in atlas.html).
//! The checker sees the lowering of a typed tree; the machine runs its
//! erasure. Here a ghost-skipping interpreter for the check IR and the
//! reference interpreter for the erased tree run every program on the same
//! inputs and must produce the same result, including not returning.

mod common;

use common::*;
use locus::erased::{EExpr, Interpreter, RunError, Value, check_module};
use locus::exec::CheckInterpreter;
use locus::typed::{FnRef, Session};

const FUEL: u64 = 200_000;

/// Both interpreters' answers. Fuel is spent differently by the two, so
/// "did not return" is compared as such and not by how much was left.
fn both(session: &Session, callee: FnRef, arguments: &[Value]) -> [Result<Value, RunError>; 2] {
    let checked = CheckInterpreter::new(session.program(), FUEL).call(callee, arguments.to_vec());
    let erased = Interpreter::new(session.erased(), FUEL).call(callee, arguments.to_vec());
    [checked, erased]
}

fn assert_agree(session: &Session, callee: FnRef, arguments: &[Value]) -> Result<Value, RunError> {
    let [checked, erased] = both(session, callee, arguments);
    assert_eq!(checked, erased, "{callee:?} at {arguments:?}");
    erased
}

#[test]
fn lowering_and_erasure_agree_on_every_program_and_every_byte() {
    let (mut session, prelude, theory) = setup();
    let increment_ref = session.declare_fn(&increment(false)).unwrap();
    let increment_id = exec_id(increment_ref);
    let programs = [
        increment_ref,
        session.declare_fn(&increment(true)).unwrap(),
        session.declare_fn(&preserve(false, true)).unwrap(),
        session.declare_fn(&preserve(true, true)).unwrap(),
        session
            .declare_fn(&bounded_walk(prelude, theory, true))
            .unwrap(),
        session
            .declare_fn(&counting_loop(theory, false, None))
            .unwrap(),
        session
            .declare_fn(&counting_loop(theory, true, None))
            .unwrap(),
        session
            .declare_fn(&counting_loop(theory, false, Some(increment_id)))
            .unwrap(),
    ];
    assert_eq!(check_module(session.erased()), Ok(()));

    for callee in programs {
        for byte in 0..=255u8 {
            let result = assert_agree(&session, callee, &[Value::U8(byte)]);
            assert!(result.is_ok(), "{callee:?} at {byte}: {result:?}");
        }
    }
}

#[test]
fn both_branches_agree_that_a_divergent_call_does_not_return() {
    let (mut session, prelude, _) = setup();
    let spin_ref = session.declare_fn(&spin(prelude)).unwrap();
    let caller = session
        .declare_fn(&caller_of_spin(prelude, exec_id(spin_ref)))
        .unwrap();
    for callee in [spin_ref, caller] {
        assert_eq!(
            assert_agree(&session, callee, &[]),
            Err(RunError::OutOfFuel)
        );
    }
}

#[test]
fn the_comparison_has_teeth() {
    // If erasure read the tree differently from lowering, the interpreters
    // would disagree. Simulate that by exchanging the branches of preserve
    // in the erased module only.
    let (mut session, _, _) = setup();
    let preserve_ref = session.declare_fn(&preserve(false, true)).unwrap();
    let mut tampered = session.erased().clone();
    let Some(EExpr::If {
        then_block,
        else_block,
        ..
    }) = tampered.fns[0].body.tail.as_deref_mut()
    else {
        panic!("preserve ends in an if")
    };
    std::mem::swap(then_block, else_block);
    // Still well typed: the type checker cannot see this kind of mistake.
    assert_eq!(check_module(&tampered), Ok(()));

    let argument = vec![Value::U8(7)];
    let checked =
        CheckInterpreter::new(session.program(), FUEL).call(preserve_ref, argument.clone());
    let erased = Interpreter::new(&tampered, FUEL).call(preserve_ref, argument);
    assert_eq!(checked, Ok(Value::Tuple(vec![Value::U8(7), Value::Proved])));
    assert_eq!(erased, Ok(Value::Tuple(vec![Value::U8(0), Value::Proved])));
}
