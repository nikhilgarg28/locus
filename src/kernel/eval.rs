//! Big-step evaluation of closed terms. This is the one place the kernel
//! computes: a shortcut for a chain of the computation axioms (`literal`,
//! `definition`, `case_step`, `projection`, and the range axioms). It takes
//! no part in comparing terms. It is trusted.
//!
//! Proofs are never evaluated: they are irrelevant. The evaluator replaces
//! each one it meets by `Proof::Omitted`, which proves nothing. For that
//! reason the `evaluate` rule only offers results whose type contains no
//! proof, so no omitted proof can surface in a conclusion.

use super::check::evaluate_primitive;
use super::defs::Definitions;
use super::error::KernelError;
use super::term::{Proof, Term, Type};

/// Counts evaluation steps, never time, so acceptance does not depend on
/// the machine.
pub(super) const STEP_LIMIT: usize = 2_000_000;

pub(super) struct Evaluator<'d> {
    definitions: &'d Definitions,
    steps: usize,
}

impl<'d> Evaluator<'d> {
    pub(super) fn new(definitions: &'d Definitions) -> Self {
        Self {
            definitions,
            steps: 0,
        }
    }

    pub(super) fn eval(&mut self, term: &Term) -> Result<Term, KernelError> {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return Err(KernelError::StepLimit);
        }
        match term {
            Term::Bool(_) | Term::U8(_) | Term::Nat(_) | Term::Fn(_) => Ok(term.clone()),
            // A proof is never inspected, so its contents are dropped. Keeping
            // them would let a loop's state grow with every iteration, since
            // each state's proofs mention the state before it.
            Term::Proof(_) => Ok(Term::proof(Proof::Omitted)),
            // A proposition is an opaque value: it may be stored and
            // projected, never inspected.
            Term::Eq(..)
            | Term::Implies(..)
            | Term::Forall(..)
            | Term::Exists(..)
            | Term::PropApp(..) => Ok(term.clone()),
            Term::Free(_) => Err(KernelError::NotClosed(term.clone())),
            Term::Bound(_) => Err(KernelError::DanglingBound),
            Term::Absurd(..) => Err(KernelError::NoComputationStep(term.clone())),
            Term::Prim(prim, arguments) => {
                let values = self.eval_all(arguments)?;
                evaluate_primitive(*prim, &values)
                    .ok_or_else(|| KernelError::NoComputationStep(term.clone()))
            }
            Term::Tuple(fields, values) => Ok(Term::Tuple(fields.clone(), self.eval_all(values)?)),
            Term::Struct(id, values) => Ok(Term::Struct(*id, self.eval_all(values)?)),
            Term::Variant(id, index, payload) => {
                Ok(Term::Variant(*id, *index, self.eval_all(payload)?))
            }
            Term::Proj(target, index) => match self.eval(target)? {
                Term::Tuple(_, values) | Term::Struct(_, values) => values
                    .into_iter()
                    .nth(*index)
                    .ok_or_else(|| KernelError::NoComputationStep(term.clone())),
                _ => Err(KernelError::NoComputationStep(term.clone())),
            },
            Term::Call(callee, arguments) => {
                let Term::Fn(id) = self.eval(callee)? else {
                    return Err(KernelError::NoComputationStep(term.clone()));
                };
                let values = self.eval_all(arguments)?;
                let decl = self
                    .definitions
                    .function(id)
                    .ok_or(KernelError::UnknownFunction)?;
                let body = decl.body.instantiate(values.len(), |j| values[j].clone());
                self.eval(&body)
            }
            Term::Case {
                scrutinee, arms, ..
            } => {
                let (index, payload) = match self.eval(scrutinee)? {
                    Term::Bool(value) => (usize::from(value), Vec::new()),
                    Term::Variant(_, index, payload) => (index, payload),
                    _ => return Err(KernelError::NoComputationStep(term.clone())),
                };
                let arm = arms
                    .get(index)
                    .ok_or_else(|| KernelError::NoComputationStep(term.clone()))?;
                let body = arm.body.instantiate(payload.len(), |j| payload[j].clone());
                self.eval(&body)
            }
            Term::For(looped) => {
                let (Term::U8(lo), Term::U8(hi)) = (self.eval(&looped.lo)?, self.eval(&looped.hi)?)
                else {
                    return Err(KernelError::NoComputationStep(term.clone()));
                };
                let mut state = self.eval(&looped.init)?;
                for index in lo..hi {
                    let arguments = [Term::U8(index), state];
                    let body = looped.body.instantiate(2, |j| arguments[j].clone());
                    state = self.eval(&body)?;
                }
                Ok(state)
            }
        }
    }

    fn eval_all(&mut self, terms: &[Term]) -> Result<Vec<Term>, KernelError> {
        terms.iter().map(|term| self.eval(term)).collect()
    }
}

/// Whether values of the type are first-order data with no proof, no
/// proposition, and no function anywhere inside.
pub(super) fn is_plain_data(definitions: &Definitions, ty: &Type) -> bool {
    let all = |fields: &[Type]| fields.iter().all(|field| is_plain_data(definitions, field));
    match ty {
        Type::Bool | Type::U8 | Type::Nat => true,
        Type::Prop | Type::Proof(_) | Type::Fn(..) => false,
        Type::Tuple(fields) => all(fields),
        Type::Struct(id) => definitions.struct_fields(*id).is_some_and(all),
        Type::Enum(id) => definitions
            .enum_variants(*id)
            .is_some_and(|variants| variants.iter().all(|payload| all(payload))),
    }
}
