//! The kernel bounds how deeply its input may nest, so that checking
//! untrusted input cannot exhaust the stack (docs/kernel-contract.md).

use locus::kernel::derive::Chain;
use locus::kernel::{
    Context, Definitions, KernelError, MAX_DEPTH, Mode, Proof, Term, Type, check_proof,
    infer_proof, infer_term,
};

fn nested_sum(depth: usize) -> Term {
    (0..depth).fold(Term::U8(0), |term, _| Term::wrapping_add(term, Term::U8(1)))
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
