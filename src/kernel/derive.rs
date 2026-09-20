//! Derived proof forms. Nothing here is trusted: each function only builds a
//! proof out of the kernel's rules, and the result is checked like any other.
//! A bug here can produce a proof the kernel rejects, never a false theorem.
//!
//! These are the `rewrite`, `unfold`, `fold`, and chain forms of specification
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
use super::term::{FnId, Proof, Term, Type};

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

// --- Builders that need no context ------------------------------------------
//
// The forms above read the sides of an equation off a proof, which needs a
// context. Inside a declaration's body there is none to hand, so these take
// the terms explicitly. They are just as untrusted.

/// From `eq: x == y` at `ty`, a proof of `y == x`.
pub fn symm_at(ty: &Type, x: &Term, eq: Proof) -> Proof {
    let (ty, x) = (ty.clone(), x.clone());
    Proof::transport(
        eq,
        |hole| Term::eq(ty, hole, x.clone()),
        Proof::Refl(x.clone()),
    )
}

/// From a proof of the predicate call `call`, a proof of its body.
pub fn unfold_claim(call: &Term, proof: Proof) -> Proof {
    Proof::transport(Proof::Definition(call.clone()), |hole| hole, proof)
}

/// From a proof of the body of the predicate call `call`, a proof of `call`.
/// `body` is what `call` unfolds to.
pub fn fold_claim(call: &Term, proof: Proof) -> Proof {
    let back = symm_at(&Type::Prop, call, Proof::Definition(call.clone()));
    Proof::transport(back, |hole| hole, proof)
}

/// An equational chain `start == ... == current` at one type, built a link
/// at a time. This is the chain form of specification section 8.4. The
/// kernel checks every link when the finished proof is checked; a wrong
/// intermediate term shows up there as a mismatch.
pub struct Chain {
    ty: Type,
    start: Term,
    proof: Proof,
}

impl Chain {
    pub fn new(ty: Type, start: Term) -> Self {
        let proof = Proof::Refl(start.clone());
        Self { ty, start, proof }
    }

    /// Extends the chain with `eq: current == next`.
    pub fn step(self, eq: Proof) -> Self {
        self.rewrite(|hole| hole, eq)
    }

    /// Extends the chain with `eq: next == current`.
    pub fn step_rev(self, next: &Term, eq: Proof) -> Self {
        let ty = self.ty.clone();
        self.rewrite_rev(&ty, |hole| hole, next, eq)
    }

    /// With `current` being `context(from)` and `eq: from == to`, moves to
    /// `context(to)`.
    pub fn rewrite(self, context: impl FnOnce(Term) -> Term, eq: Proof) -> Self {
        let (ty, start) = (self.ty.clone(), self.start.clone());
        let proof = Proof::transport(eq, |hole| Term::eq(ty, start, context(hole)), self.proof);
        Self { proof, ..self }
    }

    /// With `current` being `context(from)` and `eq: to == from` at
    /// `inner`, moves to `context(to)`.
    pub fn rewrite_rev(
        self,
        inner: &Type,
        context: impl FnOnce(Term) -> Term,
        to: &Term,
        eq: Proof,
    ) -> Self {
        self.rewrite(context, symm_at(inner, to, eq))
    }

    /// The proof of `start == current`.
    pub fn finish(self) -> Proof {
        self.proof
    }
}
