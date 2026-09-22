//! Big-step evaluation of closed terms. This is the one place the kernel
//! computes: a shortcut for a chain of the computation axioms (`literal`,
//! `definition`, `case_step`, `projection`, and the range axioms). It takes
//! no part in comparing terms. It is trusted.
//!
//! Proofs are never evaluated: they are irrelevant. The evaluator replaces
//! each one it meets by `Proof::Omitted`, which proves nothing. For that
//! reason the `evaluate` rule only offers results whose type contains no
//! proof, so no omitted proof can surface in a conclusion.
//!
//! Evaluation is recursive, and a call evaluates the callee's body, so the
//! depth of evaluation is not bounded by the depth of the input: a chain of
//! a thousand small functions, each calling the next, nests a thousand
//! deep. Two budgets bound it, both counted and neither timed: the number of
//! steps, and the depth of nesting. The second keeps the stack in bounds. As
//! in the checker, there is one small function per term form, because an
//! unoptimized build reserves stack for every arm of a large match at once.

use super::check::evaluate_primitive;
use super::defs::Definitions;
use super::error::KernelError;
use super::int::Integer;
use super::term::{ForLoop, Prim, Proof, Term, Type};

/// Counts evaluation steps, never time, so acceptance does not depend on
/// the machine.
pub(super) const STEP_LIMIT: usize = 2_000_000;

/// How deeply evaluation may nest. Measured in an unoptimized build on a
/// 2 MiB thread stack, evaluation alone nests 500 levels in every shape
/// tried and overflows at 700 in the worst one, arithmetic around a call.
/// The bound is well under half of that, because evaluation can begin deep
/// inside a proof that is itself nested up to the input depth bound, and the
/// two share one stack.
pub const MAX_EVAL_DEPTH: usize = 200;

pub(super) struct Evaluator<'d> {
    definitions: &'d Definitions,
    steps: usize,
    depth: usize,
}

impl<'d> Evaluator<'d> {
    pub(super) fn new(definitions: &'d Definitions) -> Self {
        Self {
            definitions,
            steps: 0,
            depth: 0,
        }
    }

    pub(super) fn eval(&mut self, term: &Term) -> Result<Term, KernelError> {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return Err(KernelError::StepLimit);
        }
        if self.depth >= MAX_EVAL_DEPTH {
            return Err(KernelError::EvaluationTooDeep);
        }
        self.depth += 1;
        let result = self.eval_form(term);
        self.depth -= 1;
        result
    }

    fn eval_form(&mut self, term: &Term) -> Result<Term, KernelError> {
        match term {
            Term::Bool(_)
            | Term::U8(_)
            | Term::Nat(_)
            | Term::Int(_)
            | Term::Machine(..)
            | Term::Fn(_) => Ok(term.clone()),
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
            | Term::PropApp(..)
            | Term::Prim(Prim::IntLe, _) => Ok(term.clone()),
            Term::Free(_) => Err(KernelError::NotClosed(term.clone())),
            Term::Bound(_) => Err(KernelError::DanglingBound),
            Term::Absurd(..) => Err(stuck(term)),
            Term::Prim(..) => self.eval_prim(term),
            Term::Tuple(..) | Term::Struct(..) | Term::Variant(..) => self.eval_constructor(term),
            Term::Proj(..) => self.eval_proj(term),
            Term::Call(..) => self.eval_call(term),
            Term::Case { .. } => self.eval_case(term),
            Term::For(looped) => self.eval_for(term, looped),
        }
    }

    #[inline(never)]
    fn eval_prim(&mut self, term: &Term) -> Result<Term, KernelError> {
        let Term::Prim(prim, arguments) = term else {
            unreachable!("dispatched on this form")
        };
        let values = self.eval_all(arguments)?;
        // Multiplication is the one primitive whose result can be twice the
        // size of its operands, so a short term can square its way to a
        // number of any size. It is charged what the schoolbook product
        // costs, one step for each pair of 32-bit digits, which bounds the
        // size of every number and the work done on it by the step budget.
        //
        // Division makes nothing larger, but long division in `Natural` is
        // bit by bit: for each bit of the dividend it doubles and subtracts
        // a remainder as long as the divisor. Quotient and remainder are
        // charged that, one step for each bit of the dividend times each
        // 32-bit digit of the divisor.
        if let [Term::Int(a), Term::Int(b)] = values.as_slice() {
            let digits = |n: &Integer| n.magnitude().bit_length() / 32 + 1;
            let charge = match prim {
                Prim::IntMul => digits(a).saturating_mul(digits(b)),
                Prim::IntDiv | Prim::IntRem => a.magnitude().bit_length().saturating_mul(digits(b)),
                _ => 0,
            };
            self.steps = self.steps.saturating_add(charge);
            if self.steps > STEP_LIMIT {
                return Err(KernelError::StepLimit);
            }
        }
        evaluate_primitive(*prim, &values).ok_or_else(|| stuck(term))
    }

    #[inline(never)]
    fn eval_constructor(&mut self, term: &Term) -> Result<Term, KernelError> {
        Ok(match term {
            Term::Tuple(fields, values) => Term::Tuple(fields.clone(), self.eval_all(values)?),
            Term::Struct(id, values) => Term::Struct(*id, self.eval_all(values)?),
            Term::Variant(id, index, payload) => {
                Term::Variant(*id, *index, self.eval_all(payload)?)
            }
            _ => unreachable!("dispatched on this form"),
        })
    }

    #[inline(never)]
    fn eval_proj(&mut self, term: &Term) -> Result<Term, KernelError> {
        let Term::Proj(target, index) = term else {
            unreachable!("dispatched on this form")
        };
        match self.eval(target)? {
            Term::Tuple(_, values) | Term::Struct(_, values) => {
                values.into_iter().nth(*index).ok_or_else(|| stuck(term))
            }
            _ => Err(stuck(term)),
        }
    }

    #[inline(never)]
    fn eval_call(&mut self, term: &Term) -> Result<Term, KernelError> {
        let Term::Call(callee, arguments) = term else {
            unreachable!("dispatched on this form")
        };
        let Term::Fn(id) = self.eval(callee)? else {
            return Err(stuck(term));
        };
        let values = self.eval_all(arguments)?;
        let decl = self
            .definitions
            .function(id)
            .ok_or(KernelError::UnknownFunction)?;
        let body = decl.body.instantiate(values.len(), |j| values[j].clone());
        self.eval(&body)
    }

    #[inline(never)]
    fn eval_case(&mut self, term: &Term) -> Result<Term, KernelError> {
        let Term::Case {
            scrutinee, arms, ..
        } = term
        else {
            unreachable!("dispatched on this form")
        };
        let (index, payload) = match self.eval(scrutinee)? {
            Term::Bool(value) => (usize::from(value), Vec::new()),
            Term::Variant(_, index, payload) => (index, payload),
            _ => return Err(stuck(term)),
        };
        let arm = arms.get(index).ok_or_else(|| stuck(term))?;
        let body = arm.body.instantiate(payload.len(), |j| payload[j].clone());
        self.eval(&body)
    }

    #[inline(never)]
    fn eval_for(&mut self, term: &Term, looped: &ForLoop) -> Result<Term, KernelError> {
        let (Term::U8(lo), Term::U8(hi)) = (self.eval(&looped.lo)?, self.eval(&looped.hi)?) else {
            return Err(stuck(term));
        };
        // Iterations run one after another, not one inside another, so a
        // long loop costs steps and not depth.
        let mut state = self.eval(&looped.init)?;
        for index in lo..hi {
            let arguments = [Term::U8(index), state];
            let body = looped.body.instantiate(2, |j| arguments[j].clone());
            state = self.eval(&body)?;
        }
        Ok(state)
    }

    fn eval_all(&mut self, terms: &[Term]) -> Result<Vec<Term>, KernelError> {
        terms.iter().map(|term| self.eval(term)).collect()
    }
}

fn stuck(term: &Term) -> KernelError {
    KernelError::NoComputationStep(term.clone())
}

/// Whether values of the type are first-order data with no proof, no
/// proposition, and no function anywhere inside.
pub(super) fn is_plain_data(definitions: &Definitions, ty: &Type) -> bool {
    let all = |fields: &[Type]| fields.iter().all(|field| is_plain_data(definitions, field));
    match ty {
        Type::Bool | Type::U8 | Type::Nat | Type::Int | Type::Machine(_) => true,
        Type::Prop | Type::Proof(_) | Type::Fn(..) => false,
        Type::Tuple(fields) => all(fields),
        Type::Struct(id) => definitions.struct_fields(*id).is_some_and(all),
        Type::Enum(id) => definitions
            .enum_variants(*id)
            .is_some_and(|variants| variants.iter().all(|payload| all(payload))),
    }
}
