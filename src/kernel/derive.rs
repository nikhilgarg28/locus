//! Derived proof forms. Nothing here is trusted: each function only builds a
//! proof out of the kernel's rules, and the result is checked like any other.
//! A bug here can produce a proof the kernel rejects, never a false theorem.
//!
//! These are the `rewrite`, `unfold`, and `fold` forms of specification
//! section 8.4, plus symmetry and transitivity of equality. Each works out a
//! transport template by abstracting occurrences of a term.
//!
//! Limit: an occurrence that mentions a variable bound inside the
//! proposition, such as the call in `forall x { nonzero(x) }`, is not closed
//! and is left alone. Reaching under a quantifier takes a `forall_elim`,
//! the rewrite, and a `forall_intro`; the elaborator will do that.

use super::check::{infer_proof, same};
use super::context::Context;
use super::error::KernelError;
use super::term::{FnId, Proof, Term};

/// Bounds the unfolding loops below in steps, never in time.
const STEP_LIMIT: usize = 10_000;

fn equation(ctx: &mut Context, eq: &Proof) -> Result<(Term, Term), KernelError> {
    match infer_proof(ctx, eq)? {
        Term::Eq(_, left, right) => Ok((*left, *right)),
        other => Err(KernelError::NotAnEquality(other)),
    }
}

/// From `eq: a == b`, a proof of `b == a`.
pub fn symm(ctx: &mut Context, eq: &Proof) -> Result<Proof, KernelError> {
    let Term::Eq(ty, left, _) = infer_proof(ctx, eq)? else {
        return Err(KernelError::NotAnEquality(infer_proof(ctx, eq)?));
    };
    let a = (*left).clone();
    Ok(Proof::transport(
        eq.clone(),
        |hole| Term::eq(ty, hole, a),
        Proof::Refl(*left),
    ))
}

/// From `ab: a == b` and `bc: b == c`, a proof of `a == c`.
pub fn trans(ctx: &mut Context, ab: &Proof, bc: &Proof) -> Result<Proof, KernelError> {
    let Term::Eq(ty, a, _) = infer_proof(ctx, ab)? else {
        return Err(KernelError::NotAnEquality(infer_proof(ctx, ab)?));
    };
    Ok(Proof::transport(
        bc.clone(),
        |hole| Term::eq(ty, *a, hole),
        ab.clone(),
    ))
}

/// From `eq: a == b` and `proof: P`, a proof of `P` with every closed
/// occurrence of `a` replaced by `b`.
pub fn rewrite(ctx: &mut Context, eq: &Proof, proof: &Proof) -> Result<Proof, KernelError> {
    let (a, _) = equation(ctx, eq)?;
    let prop = infer_proof(ctx, proof)?;
    Ok(Proof::Transport {
        eq: Box::new(eq.clone()),
        template: prop.abstract_over(&|term| same(term, &a)),
        proof: Box::new(proof.clone()),
    })
}

fn is_call_to(term: &Term, id: FnId) -> bool {
    matches!(term, Term::Call(callee, _) if **callee == Term::Fn(id)) && term.is_closed()
}

/// From `proof: P`, a proof of `P` with every closed call to `id` replaced
/// by the function's body.
pub fn unfold(ctx: &mut Context, id: FnId, proof: &Proof) -> Result<Proof, KernelError> {
    let mut prop = infer_proof(ctx, proof)?;
    let mut proof = proof.clone();
    let mut unfolded_any = false;
    for _ in 0..STEP_LIMIT {
        let Some(call) = prop.find(&|term| is_call_to(term, id)).cloned() else {
            return if unfolded_any {
                Ok(proof)
            } else {
                Err(KernelError::NoComputationStep(prop))
            };
        };
        let step = Proof::Definition(call.clone());
        let (_, body) = equation(ctx, &step)?;
        let template = prop.abstract_over(&|term| same(term, &call));
        prop = template.open(&body);
        proof = Proof::Transport {
            eq: Box::new(step),
            template,
            proof: Box::new(proof),
        };
        unfolded_any = true;
    }
    Err(KernelError::StepLimit)
}

/// From `proof: P`, a proof of `goal`, where unfolding every closed call to
/// `id` in `goal` gives `P`. The kernel checks that it does.
pub fn fold(ctx: &mut Context, id: FnId, proof: &Proof, goal: &Term) -> Result<Proof, KernelError> {
    // Unfold the goal step by step, remembering each step, then replay the
    // steps backwards from the given proof.
    let mut steps = Vec::new();
    let mut prop = goal.clone();
    while let Some(call) = prop.find(&|term| is_call_to(term, id)).cloned() {
        if steps.len() == STEP_LIMIT {
            return Err(KernelError::StepLimit);
        }
        let step = Proof::Definition(call.clone());
        let (_, body) = equation(ctx, &step)?;
        let template = prop.abstract_over(&|term| same(term, &call));
        prop = template.open(&body);
        steps.push((symm(ctx, &step)?, template));
    }
    if steps.is_empty() {
        return Err(KernelError::NoComputationStep(goal.clone()));
    }
    let mut proof = proof.clone();
    for (eq, template) in steps.into_iter().rev() {
        proof = Proof::Transport {
            eq: Box::new(eq),
            template,
            proof: Box::new(proof),
        };
    }
    Ok(proof)
}
