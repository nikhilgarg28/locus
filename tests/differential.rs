//! The two branches from the typed tree must agree (Architecture in atlas.html).
//! The checker sees the lowering of a typed tree; the machine runs its
//! erasure. Here a ghost-skipping interpreter for the check IR and the
//! reference interpreter for the erased tree run every program on the same
//! inputs and must end in the same outcome: the same value, or a panic with
//! the same message.
//!
//! Out of fuel is not an outcome of the program, it is the absence of one
//! within a budget, and the two interpreters spend fuel differently. So a
//! comparison in which either side ran out of fuel is inconclusive: it is
//! not agreement, even when both sides ran out, and it is not disagreement,
//! even when the other side returned or panicked. A test that expects a
//! program not to return says so by expecting out of fuel on both sides.

mod common;

use common::*;
use locus::erased::{EExpr, Interpreter, Outcome, RunError, Value, check_module};
use locus::exec::CheckInterpreter;
use locus::typed::{FnRef, Session};

const FUEL: u64 = 200_000;

type Answer = Result<Outcome, RunError>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Comparison {
    /// Both sides ended in this outcome, which is a value or a panic.
    Agree(Outcome),
    /// The sides ended differently, or one of them could not run.
    Disagree,
    /// A side ran out of fuel, so nothing was learned.
    Inconclusive,
}

fn compare(checked: &Answer, erased: &Answer) -> Comparison {
    match (checked, erased) {
        (Err(_), _) | (_, Err(_)) => Comparison::Disagree,
        (Ok(Outcome::OutOfFuel), _) | (_, Ok(Outcome::OutOfFuel)) => Comparison::Inconclusive,
        (Ok(checked), Ok(erased)) if checked == erased => Comparison::Agree(erased.clone()),
        _ => Comparison::Disagree,
    }
}

/// Both interpreters' answers.
fn both(session: &Session, callee: FnRef, arguments: &[Value]) -> [Answer; 2] {
    let checked = CheckInterpreter::new(session.program(), FUEL).call(callee, arguments.to_vec());
    let erased = Interpreter::new(session.erased(), FUEL).call(callee, arguments.to_vec());
    [checked, erased]
}

#[test]
fn lowering_and_erasure_agree_on_every_program_and_every_byte() {
    let (mut session, prelude, theory) = setup();
    let increment_ref = session.declare_fn(&increment(false)).unwrap();
    let increment_id = exec_id(increment_ref);
    let programs = [
        increment_ref,
        session.declare_fn(&increment(true)).unwrap(),
        session.declare_fn(&preserve(theory, false, true)).unwrap(),
        session.declare_fn(&preserve(theory, true, true)).unwrap(),
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
            let [checked, erased] = both(&session, callee, &[Value::u8(byte)]);
            assert!(
                matches!(
                    compare(&checked, &erased),
                    Comparison::Agree(Outcome::Value(_))
                ),
                "{callee:?} at {byte}: {checked:?} and {erased:?}"
            );
        }
    }
}

#[test]
fn a_divergent_call_runs_out_of_fuel_in_both_branches() {
    let (mut session, prelude, _) = setup();
    let spin_ref = session.declare_fn(&spin(prelude)).unwrap();
    let caller = session
        .declare_fn(&caller_of_spin(prelude, exec_id(spin_ref)))
        .unwrap();
    for callee in [spin_ref, caller] {
        let [checked, erased] = both(&session, callee, &[]);
        assert_eq!(checked, Ok(Outcome::OutOfFuel));
        assert_eq!(erased, Ok(Outcome::OutOfFuel));
        // Which is what the test expects of them, and not an agreement.
        assert_eq!(compare(&checked, &erased), Comparison::Inconclusive);
    }
}

#[test]
fn outcomes_are_compared_as_outcomes() {
    let value = |byte| Ok(Outcome::Value(Value::u8(byte)));
    let panic = |message: &str| Ok(Outcome::Panic(message.into()));
    let out_of_fuel = Ok(Outcome::OutOfFuel);
    let stuck = Err(RunError::Stuck("somewhere".into()));

    assert_eq!(
        compare(&value(1), &value(1)),
        Comparison::Agree(Outcome::Value(Value::u8(1)))
    );
    assert_eq!(compare(&value(1), &value(2)), Comparison::Disagree);
    // A panic agrees with a panic that has the same message, and only that.
    assert_eq!(
        compare(&panic("overflow"), &panic("overflow")),
        Comparison::Agree(Outcome::Panic("overflow".into()))
    );
    assert_eq!(
        compare(&panic("overflow"), &panic("underflow")),
        Comparison::Disagree
    );
    assert_eq!(compare(&value(1), &panic("overflow")), Comparison::Disagree);
    // Out of fuel is no panic, agrees with nothing, and contradicts nothing.
    assert_ne!(out_of_fuel, panic("out of fuel"));
    for other in [value(1), panic("overflow"), out_of_fuel.clone()] {
        assert_eq!(compare(&out_of_fuel, &other), Comparison::Inconclusive);
        assert_eq!(compare(&other, &out_of_fuel), Comparison::Inconclusive);
    }
    // An interpreter that could not run the program is never an agreement.
    assert_eq!(compare(&stuck, &stuck), Comparison::Disagree);
    assert_eq!(compare(&stuck, &out_of_fuel), Comparison::Disagree);
}

#[test]
fn the_comparison_has_teeth() {
    // If erasure read the tree differently from lowering, the interpreters
    // would disagree. Simulate that by exchanging the branches of preserve
    // in the erased module only.
    let (mut session, _, theory) = setup();
    let preserve_ref = session.declare_fn(&preserve(theory, false, true)).unwrap();
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

    let argument = vec![Value::u8(7)];
    let returned = |byte| {
        Ok(Outcome::Value(Value::Tuple(vec![
            Value::u8(byte),
            Value::Proved,
        ])))
    };
    let checked =
        CheckInterpreter::new(session.program(), FUEL).call(preserve_ref, argument.clone());
    let erased = Interpreter::new(&tampered, FUEL).call(preserve_ref, argument.clone());
    assert_eq!(checked, returned(7));
    assert_eq!(erased, returned(0));
    assert_eq!(compare(&checked, &erased), Comparison::Disagree);

    // Likewise if erasure put a panic where lowering has none. Nothing in
    // the check IR panics yet, so this is the one way the two can differ by
    // a panic today.
    let Some(EExpr::If { then_block, .. }) = tampered.fns[0].body.tail.as_deref_mut() else {
        panic!("preserve ends in an if")
    };
    then_block.tail = Some(Box::new(EExpr::Panic {
        message: "not in the check IR".into(),
    }));
    else_block_too(&mut tampered);
    assert_eq!(check_module(&tampered), Ok(()));
    let erased = Interpreter::new(&tampered, FUEL).call(preserve_ref, argument);
    assert_eq!(erased, Ok(Outcome::Panic("not in the check IR".into())));
    assert_eq!(compare(&checked, &erased), Comparison::Disagree);
}

/// Makes the other branch of preserve's `if` the same panic, so that the
/// function panics at every input.
fn else_block_too(module: &mut locus::erased::Module) {
    let Some(EExpr::If {
        then_block,
        else_block,
        ..
    }) = module.fns[0].body.tail.as_deref_mut()
    else {
        panic!("preserve ends in an if")
    };
    else_block.tail = then_block.tail.clone();
}
