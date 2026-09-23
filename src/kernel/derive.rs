//! Derived proof forms. Nothing here is trusted: each function only builds a
//! proof out of the kernel's rules, and the result is checked like any other.
//! A bug here can produce a proof the kernel rejects, never a false theorem.
//!
//! These are the `rewrite`, `unfold`, `fold`, and chain forms of specification
//! section 8.4, plus symmetry and transitivity of equality. Each works out a
//! transport template by abstracting occurrences of a term.
//!
//! `unfold` and `fold` reach calls that mention a bound variable, such as the
//! one in `forall x { nonzero(x) }`, by going under `forall` and `exists`
//! and into the conclusion of an implication, rebuilding the binder around
//! the rewritten body. A call inside a declared proposition's arguments, or
//! under a binder within the premise of an implication, is not reached.
//! `rewrite` needs no such descent: the term it replaces is closed, and a
//! closed term is found wherever it occurs.

use super::check::{infer_proof, same};
use super::context::Context;
use super::error::KernelError;
use super::term::{FnId, Proof, ProofArm, Term, Type};

/// Bounds the unfolding loops below in steps, never in time.
use crate::limits::MAX_DERIVED_STEPS as STEP_LIMIT;

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

/// Calls present before this unfolding step. Newly introduced recursive calls
/// are left folded; expanding them again could diverge on symbolic arguments.
fn original_calls(prop: &Term, id: FnId) -> Vec<Term> {
    let calls = std::cell::RefCell::new(Vec::new());
    prop.find(&|term| {
        if is_call_to(term, id) && !calls.borrow().iter().any(|old| same(old, term)) {
            calls.borrow_mut().push(term.clone());
        }
        false
    });
    calls.into_inner()
}

fn unfold_closed(ctx: &mut Context, id: FnId, proof: &Proof) -> Result<Option<Proof>, KernelError> {
    let mut prop = infer_proof(ctx, proof)?;
    let mut proof = proof.clone();
    let mut unfolded_any = false;
    for call in original_calls(&prop, id) {
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
    Ok(unfolded_any.then_some(proof))
}

/// Unfolds at the top, then under `forall`, under `exists`, and in the
/// conclusion of an implication, where a call may mention the bound
/// variable. Returns the new proof and whether anything changed.
fn unfold_deep(ctx: &mut Context, id: FnId, proof: Proof) -> Result<(Proof, bool), KernelError> {
    let (proof, changed) = match unfold_closed(ctx, id, &proof)? {
        Some(unfolded) => (unfolded, true),
        None => (proof, false),
    };
    let scope = ctx.len();
    let result = match infer_proof(ctx, &proof)? {
        Term::Forall(ty, _) => {
            let var = ctx.push_bound(ty.clone());
            let instance = Proof::forall_elim(proof.clone(), Term::Free(var));
            unfold_deep(ctx, id, instance).map(|(inner, inner_changed)| {
                let rebuilt = Proof::ForallIntro {
                    ty,
                    body: Box::new(inner.close_var(var)),
                };
                (rebuilt, inner_changed)
            })
        }
        Term::Implies(premise, _) => {
            let hyp = ctx.push_hyp((*premise).clone());
            let applied = Proof::implies_elim(proof.clone(), Proof::hyp(hyp));
            unfold_deep(ctx, id, applied).map(|(inner, inner_changed)| {
                let rebuilt = Proof::ImpliesIntro {
                    hyp: *premise,
                    body: Box::new(inner.close_hyp(hyp)),
                };
                (rebuilt, inner_changed)
            })
        }
        Term::Exists(ty, body) => {
            let var = ctx.push_bound(ty.clone());
            let hyp = ctx.push_hyp(body.open(&Term::Free(var)));
            match unfold_deep(ctx, id, Proof::hyp(hyp)) {
                Ok((inner, true)) => infer_proof(ctx, &inner).map(|unfolded| {
                    let goal = Term::Exists(ty, Box::new(unfolded.close(var)));
                    let arm = Proof::ExistsIntro {
                        prop: goal.clone(),
                        witness: Term::Free(var),
                        proof: Box::new(inner),
                    };
                    let rebuilt = Proof::ExistsElim {
                        exists: Box::new(proof.clone()),
                        goal,
                        arm: ProofArm {
                            vars: 1,
                            hyps: 1,
                            body: Box::new(arm.close_var(var).close_hyp(hyp)),
                        },
                    };
                    (rebuilt, true)
                }),
                Ok((_, false)) => Ok((proof.clone(), false)),
                Err(error) => Err(error),
            }
        }
        _ => Ok((proof.clone(), false)),
    };
    ctx.truncate(scope);
    let (rebuilt, inner_changed) = result?;
    if inner_changed {
        Ok((rebuilt, true))
    } else {
        Ok((proof, changed))
    }
}

/// From `proof: P`, a proof of `P` with every call to `id` replaced by the
/// function's body, including calls under `forall` and `exists` and in the
/// conclusion of an implication.
pub fn unfold(ctx: &mut Context, id: FnId, proof: &Proof) -> Result<Proof, KernelError> {
    match unfold_deep(ctx, id, proof.clone())? {
        (unfolded, true) => Ok(unfolded),
        (_, false) => Err(KernelError::NoComputationStep(infer_proof(ctx, proof)?)),
    }
}

/// Folds closed calls only: the goal's closed calls, unfolded, must give the
/// proposition `proof` proves.
fn fold_closed(
    ctx: &mut Context,
    id: FnId,
    proof: &Proof,
    goal: &Term,
) -> Result<Option<Proof>, KernelError> {
    // Unfold the goal step by step, remembering each step, then replay the
    // steps backwards from the given proof.
    let mut steps = Vec::new();
    let mut prop = goal.clone();
    for call in original_calls(&prop, id) {
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
        return Ok(None);
    }
    let mut proof = proof.clone();
    for (eq, template) in steps.into_iter().rev() {
        proof = Proof::Transport {
            eq: Box::new(eq),
            template,
            proof: Box::new(proof),
        };
    }
    Ok(Some(proof))
}

/// Follows the shape of the goal: under `forall` and `exists`, and across an
/// implication, whose premise is unfolded on the way in.
fn fold_deep(
    ctx: &mut Context,
    id: FnId,
    proof: Proof,
    goal: &Term,
) -> Result<(Proof, bool), KernelError> {
    let scope = ctx.len();
    let result = match goal {
        Term::Forall(ty, body) => {
            let var = ctx.push_bound(ty.clone());
            let instance = Proof::forall_elim(proof, Term::Free(var));
            fold_deep(ctx, id, instance, &body.open(&Term::Free(var))).map(|(inner, changed)| {
                let rebuilt = Proof::ForallIntro {
                    ty: ty.clone(),
                    body: Box::new(inner.close_var(var)),
                };
                (rebuilt, changed)
            })
        }
        Term::Implies(premise, conclusion) => {
            let hyp = ctx.push_hyp((**premise).clone());
            unfold_deep(ctx, id, Proof::hyp(hyp)).and_then(|(unfolded_premise, premise_changed)| {
                let applied = Proof::implies_elim(proof, unfolded_premise);
                fold_deep(ctx, id, applied, conclusion).map(|(inner, changed)| {
                    let rebuilt = Proof::ImpliesIntro {
                        hyp: (**premise).clone(),
                        body: Box::new(inner.close_hyp(hyp)),
                    };
                    (rebuilt, changed || premise_changed)
                })
            })
        }
        Term::Exists(ty, body) => match infer_proof(ctx, &proof) {
            Ok(Term::Exists(_, unfolded_body)) => {
                let var = ctx.push_bound(ty.clone());
                let hyp = ctx.push_hyp(unfolded_body.open(&Term::Free(var)));
                fold_deep(ctx, id, Proof::hyp(hyp), &body.open(&Term::Free(var))).map(
                    |(inner, changed)| {
                        let arm = Proof::ExistsIntro {
                            prop: goal.clone(),
                            witness: Term::Free(var),
                            proof: Box::new(inner),
                        };
                        let rebuilt = Proof::ExistsElim {
                            exists: Box::new(proof),
                            goal: goal.clone(),
                            arm: ProofArm {
                                vars: 1,
                                hyps: 1,
                                body: Box::new(arm.close_var(var).close_hyp(hyp)),
                            },
                        };
                        (rebuilt, changed)
                    },
                )
            }
            Ok(other) => Err(KernelError::NotExistential(other)),
            Err(error) => Err(error),
        },
        _ => fold_closed(ctx, id, &proof, goal).map(|folded| match folded {
            Some(folded) => (folded, true),
            None => (proof, false),
        }),
    };
    ctx.truncate(scope);
    result
}

/// From `proof: P`, a proof of `goal`, where unfolding every call to `id` in
/// `goal` gives `P`. The kernel checks that it does.
pub fn fold(ctx: &mut Context, id: FnId, proof: &Proof, goal: &Term) -> Result<Proof, KernelError> {
    match fold_deep(ctx, id, proof.clone(), goal)? {
        (folded, true) => Ok(folded),
        (_, false) => Err(KernelError::NoComputationStep(goal.clone())),
    }
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

/// Congruence for a call whose proof arguments depend on the replaced data.
/// Quantifying those evidence slots keeps the equality motive well typed;
/// directly replacing a data argument while retaining its old proof does not.
/// This builder adds no kernel rule and its complete certificate is checked.
pub fn rewrite_call_arguments(
    ctx: &mut Context,
    eq: &Proof,
    call: &Term,
) -> Result<Proof, KernelError> {
    use super::{Mode, infer_term, telescope_entry};
    let Term::Call(callee, original) = call else {
        return Err(KernelError::NoComputationStep(call.clone()));
    };
    let Type::Fn(params, _result) = infer_term(ctx, callee, Mode::Logical)? else {
        return Err(KernelError::NoComputationStep(call.clone()));
    };
    let return_type = infer_term(ctx, call, Mode::Logical)?;
    if matches!(return_type, Type::Proof(_)) {
        return Err(KernelError::EqualityAtProofType(return_type));
    }
    let (from, to) = equation(ctx, eq)?;
    let telescope = Type::Tuple(params.clone());
    let mut changed = Vec::new();
    let mut evidence = Vec::new();
    for (index, arg) in original.iter().enumerate() {
        let ty = telescope_entry(&telescope, index, &changed).ok_or(KernelError::DanglingBound)?;
        if let Type::Proof(_) = ty {
            let old = match arg {
                Term::Proof(p) => (**p).clone(),
                _ => Proof::OfTerm(arg.clone()),
            };
            let rewritten = Proof::transport(
                eq.clone(),
                |hole| {
                    let preceding: Vec<_> = original[..index]
                        .iter()
                        .map(|arg| arg.abstract_over(&|t| same(t, &from)).open(&hole))
                        .collect();
                    let Type::Proof(claim) = telescope_entry(&telescope, index, &preceding)
                        .expect("checked proof parameter")
                    else {
                        unreachable!()
                    };
                    *claim
                },
                old,
            );
            let value = Term::proof(rewritten);
            super::check::expect_type(ctx, &value, &ty, Mode::Logical)?;
            evidence.push(value.clone());
            changed.push(value);
        } else {
            changed.push(arg.abstract_over(&|t| same(t, &from)).open(&to));
        }
    }
    if evidence.is_empty() {
        return Err(KernelError::NoComputationStep(call.clone()));
    }
    // Recursive certificate construction keeps the fixed context explicit.
    #[allow(clippy::too_many_arguments)]
    fn motive(
        telescope: &Type,
        original: &[Term],
        args: Vec<Term>,
        from: &Term,
        to: &Term,
        callee: &Term,
        left: &Term,
        result: &Type,
    ) -> Term {
        let index = args.len();
        if index == original.len() {
            return Term::eq(
                result.clone(),
                left.clone(),
                Term::call(callee.clone(), args),
            );
        }
        let ty = super::telescope_entry(telescope, index, &args).expect("checked telescope");
        if matches!(ty, Type::Proof(_)) {
            Term::forall(ty, |proof| {
                let mut args = args;
                args.push(Term::proof(Proof::OfTerm(proof)));
                motive(telescope, original, args, from, to, callee, left, result)
            })
        } else {
            let mut args = args;
            args.push(original[index].abstract_over(&|t| same(t, from)).open(to));
            motive(telescope, original, args, from, to, callee, left, result)
        }
    }
    fn reflexive(telescope: &Type, original: &[Term], args: Vec<Term>, left: &Term) -> Proof {
        let index = args.len();
        if index == original.len() {
            return Proof::Refl(left.clone());
        }
        let ty = super::telescope_entry(telescope, index, &args).expect("checked telescope");
        if matches!(ty, Type::Proof(_)) {
            Proof::forall_intro(ty, |proof| {
                let mut args = args;
                args.push(Term::proof(Proof::OfTerm(proof)));
                reflexive(telescope, original, args, left)
            })
        } else {
            let mut args = args;
            args.push(original[index].clone());
            reflexive(telescope, original, args, left)
        }
    }
    let initial = reflexive(&telescope, original, vec![], call);
    let mut proof = Proof::transport(
        eq.clone(),
        |hole| {
            motive(
                &telescope,
                original,
                vec![],
                &from,
                &hole,
                callee,
                call,
                &return_type,
            )
        },
        initial,
    );
    for argument in evidence {
        proof = Proof::forall_elim(proof, argument);
    }
    let expected = Term::eq(
        return_type,
        call.clone(),
        Term::call((**callee).clone(), changed),
    );
    super::check::check_proof(ctx, &proof, &expected)?;
    Ok(proof)
}
