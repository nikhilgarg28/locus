//! Checked logical closures and explicit beta equality.
use locus::kernel::{
    Context, Definitions, KernelError, Mode, Proof, Term, Type, VarId, check_proof, infer_proof,
    infer_term,
};
use std::rc::Rc;

#[test]
#[doc = "spec: 2.31:1, 2.31:2"]
fn lambdas_capture_values_and_beta_checks_the_exact_application() {
    let mut ctx = Context::with_definitions(Rc::new(Definitions::new()));
    let captured = ctx.declare(Type::Int).unwrap();
    let x = VarId::fresh();
    let closure = Term::lambda_over(
        &[(x, Type::Int)],
        &Type::Int,
        Term::int_add(Term::Free(x), Term::Free(captured)),
    );
    assert_eq!(
        infer_term(&mut ctx, &closure, Mode::Logical),
        Ok(Type::Fn(vec![Type::Int], Box::new(Type::Int)))
    );
    assert_eq!(
        infer_term(&mut ctx, &closure, Mode::Executable),
        Err(KernelError::LogicalFunctionInExecutable)
    );
    let call = Term::call(closure.clone(), vec![Term::int(3)]);
    let claim = Term::eq(
        Type::Int,
        call.clone(),
        Term::int_add(Term::int(3), Term::Free(captured)),
    );
    assert_eq!(
        check_proof(&mut ctx, &Proof::Definition(call.clone()), &claim),
        Ok(())
    );
    let wrong = Term::eq(
        Type::Int,
        call.clone(),
        Term::int_add(Term::int(4), Term::Free(captured)),
    );
    assert!(check_proof(&mut ctx, &Proof::Definition(call), &wrong).is_err());
    assert!(
        infer_term(
            &mut ctx,
            &Term::call(closure, vec![Term::Bool(true)]),
            Mode::Logical
        )
        .is_err()
    );
}

#[test]
fn nested_binders_keep_captures_distinct_and_evaluate_under_existing_budgets() {
    let x = VarId::fresh();
    let y = VarId::fresh();
    let inner = Term::lambda_over(
        &[(y, Type::Int)],
        &Type::Int,
        Term::int_add(Term::Free(x), Term::Free(y)),
    );
    let outer = Term::lambda_over(
        &[(x, Type::Int)],
        &Type::Fn(vec![Type::Int], Box::new(Type::Int)),
        inner,
    );
    let value = Term::call(Term::call(outer, vec![Term::int(2)]), vec![Term::int(3)]);
    let mut ctx = Context::new();
    assert_eq!(
        check_proof(
            &mut ctx,
            &Proof::Evaluate(value.clone()),
            &Term::eq(Type::Int, value, Term::int(5))
        ),
        Ok(())
    );
}

#[test]
fn dependent_proof_results_are_checked_without_comparing_proof_objects() {
    let x = VarId::fresh();
    let claim = Term::eq(Type::Int, Term::Free(x), Term::Free(x));
    let closure = Term::lambda_over(
        &[(x, Type::Int)],
        &Type::proof(claim),
        Term::proof(Proof::Refl(Term::Free(x))),
    );
    let call = Term::call(closure, vec![Term::int(9)]);
    let mut ctx = Context::new();
    assert_eq!(
        check_proof(
            &mut ctx,
            &Proof::OfTerm(call.clone()),
            &Term::eq(Type::Int, Term::int(9), Term::int(9))
        ),
        Ok(())
    );
    assert!(matches!(
        infer_proof(&mut ctx, &Proof::Definition(call)),
        Err(KernelError::EqualityAtProofType(_))
    ));
    let malformed = Term::Lambda {
        params: vec![Type::Int],
        result: Type::Int,
        body: Box::new(Term::Bound(1)),
    };
    assert!(infer_term(&mut ctx, &malformed, Mode::Logical).is_err());
    let wrong_result = Term::Lambda {
        params: vec![Type::Int],
        result: Type::Bool,
        body: Box::new(Term::Bound(0)),
    };
    assert!(infer_term(&mut ctx, &wrong_result, Mode::Logical).is_err());
}
