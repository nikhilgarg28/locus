//! The kernel bounds how deeply its input may nest, so that checking
//! untrusted input cannot exhaust the stack (the kernel contract in atlas.html).

use locus::kernel::derive::Chain;
use locus::kernel::{
    Context, Definitions, KernelError, MAX_DEPTH, MachineInt, Mode, Op, Proof, Term, Type,
    check_proof, infer_proof, infer_term,
};

fn nested_sum(depth: usize) -> Term {
    (0..depth).fold(Term::U8(0), |term, _| {
        Term::op(Op::WrappingAdd, MachineInt::U8, vec![term, Term::U8(1)])
    })
}

fn long_chain(links: usize) -> Proof {
    let start = Term::U8(7);
    (0..links)
        .fold(Chain::new(Type::U8, start.clone()), |chain, _| {
            chain.step(Proof::Refl(start.clone()))
        })
        .finish()
}

fn nested_foralls(depth: usize) -> (Term, Proof) {
    let claim = (0..depth).fold(Term::eq(Type::U8, Term::U8(1), Term::U8(1)), |body, _| {
        Term::forall(Type::U8, |_| body)
    });
    let proof = (0..depth).fold(Proof::Refl(Term::U8(1)), |body, _| {
        Proof::forall_intro(Type::U8, |_| body)
    });
    (claim, proof)
}

#[test]
#[doc = "spec: 2.21:1, 2.21:2, 2.21:3, 2.23:2"]
fn input_up_to_the_bound_is_checked_without_exhausting_the_stack() {
    let mut ctx = Context::new();
    let margin = 8;
    assert_eq!(
        infer_term(&mut ctx, &nested_sum(MAX_DEPTH - margin), Mode::Executable),
        Ok(Type::U8)
    );
    let goal = Term::eq(Type::U8, Term::U8(7), Term::U8(7));
    assert_eq!(
        check_proof(&mut ctx, &long_chain(MAX_DEPTH - margin), &goal),
        Ok(())
    );
    let (claim, proof) = nested_foralls(MAX_DEPTH - margin);
    assert_eq!(check_proof(&mut ctx, &proof, &claim), Ok(()));
}

#[test]
#[doc = "spec: 2.21:1"]
fn deeper_input_is_rejected_before_any_recursion() {
    let mut ctx = Context::new();
    assert_eq!(
        infer_term(&mut ctx, &nested_sum(MAX_DEPTH + 1), Mode::Logical),
        Err(KernelError::TooDeep)
    );
    assert_eq!(
        infer_proof(&mut ctx, &long_chain(MAX_DEPTH + 1)),
        Err(KernelError::TooDeep)
    );
    let (claim, proof) = nested_foralls(MAX_DEPTH + 1);
    assert_eq!(
        check_proof(&mut ctx, &proof, &claim),
        Err(KernelError::TooDeep)
    );
    // Far past the bound: measuring is iterative, so even this is safe. The
    // term is leaked because dropping one this deep is itself recursive;
    // that is Rust's drop glue, not the kernel.
    let enormous = nested_sum(200_000);
    assert_eq!(
        infer_term(&mut ctx, &enormous, Mode::Logical),
        Err(KernelError::TooDeep)
    );
    std::mem::forget(enormous);
    // Declarations and hypotheses are measured too.
    let deep_claim = Term::eq(Type::U8, nested_sum(MAX_DEPTH + 1), Term::U8(0));
    assert_eq!(ctx.assume(deep_claim), Err(KernelError::TooDeep));
    let mut definitions = Definitions::new();
    assert_eq!(
        definitions.declare_fn(&Type::function(0, |_| Type::U8), |_| nested_sum(
            MAX_DEPTH + 1
        )),
        Err(KernelError::TooDeep)
    );
}

/// `count` functions, each two nodes deep, each calling the one before.
fn call_chain(count: usize) -> (Definitions, Term) {
    let mut definitions = Definitions::new();
    let signature = Type::function(0, |_| Type::U8);
    let mut previous = definitions.declare_fn(&signature, |_| Term::U8(0)).unwrap();
    for _ in 0..count {
        previous = definitions
            .declare_fn(&signature, |_| {
                Term::op(
                    Op::WrappingAdd,
                    MachineInt::U8,
                    vec![Term::call(Term::Fn(previous), vec![]), Term::U8(1)],
                )
            })
            .unwrap();
    }
    (definitions, Term::call(Term::Fn(previous), vec![]))
}

#[test]
fn evaluation_depth_is_bounded_independently_of_input_depth() {
    use locus::kernel::MAX_EVAL_DEPTH;
    use std::rc::Rc;
    // A thousand small functions: every input is shallow, but evaluating the
    // last one nests through all of them. This must be an error, not a
    // stack overflow.
    let (definitions, last) = call_chain(1000);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    assert_eq!(
        infer_proof(&mut ctx, &Proof::Evaluate(last)),
        Err(KernelError::EvaluationTooDeep)
    );

    // A chain that fits evaluates, even when the evaluation begins at the
    // bottom of a proof nested nearly as deeply as input may be: the two
    // share one stack.
    let fits = MAX_EVAL_DEPTH / 2 - 2;
    let (definitions, last) = call_chain(fits);
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let value = Term::U8((fits % 256) as u8);
    let goal = Term::eq(Type::U8, last.clone(), value);
    let wrapped = (0..MAX_DEPTH - 16).fold(Proof::Evaluate(last.clone()), |proof, _| {
        let left = last.clone();
        Proof::transport(
            Proof::Refl(Term::U8(0)),
            |_| Term::eq(Type::U8, left.clone(), Term::U8((fits % 256) as u8)),
            proof,
        )
    });
    assert_eq!(check_proof(&mut ctx, &wrapped, &goal), Ok(()));
}
