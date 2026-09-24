//! Source arithmetic and checked safety evidence.
//!
//! Nat and Int operations are logical; Nat results carry checked nonnegativity.
//! Machine operations lower to Operate. Before introducing a result or its
//! normal-return facts, bounded proof construction tries the row's safety
//! premises. Every found proof is kernel-checked. Missing evidence retains a
//! runtime check, except under no_panic where it is an error. Normal return
//! establishes exactness for overflow-checking operations in every build.
//! Explicit wrapping methods retain modular meaning. Division and remainder
//! establish their nonzero-divisor and signed-overflow exclusions.

use std::time::Instant;

use crate::arith::{self, Budget, Counterexample, GaveUp};
use crate::ast::{self, BinaryOp};
use crate::diagnostic::{ConsideredFact, Diagnostic};
use crate::kernel::derive::symm_at;
use crate::kernel::{
    Axiom, HypId, HypRef, MachineInt, Op, Panic, Prim, Proof, Row, Term, Type, VarId, check_proof,
    same, same_type,
};
use crate::source::Span;
use crate::typed::Expr;

use super::env::{Elab, Env, Fact, substitute};
use super::explain::free_variables;
use super::exprs::Value;
use super::items::{FoundProof, HoleReport};
use super::literals::untyped_literal;
use super::solve::{Step, forward};

/// One operand as elaborated, with where it was written.
struct Operand {
    value: Value,
    span: Span,
}

/// Why the arithmetic tier failed, as a note for a diagnostic.
pub(super) enum ArithmeticFailure {
    /// Values that satisfy the arithmetic facts and violate the goal, in
    /// source spelling: nothing linear would fill this.
    Counterexample(String),
    /// A count ran out: the goal was not decided either way.
    Budget(String),
}

impl ArithmeticFailure {
    pub fn note(&self) -> &str {
        match self {
            Self::Counterexample(note) | Self::Budget(note) => note,
        }
    }
}

impl Env<'_> {
    /// `left op right` for one of `+`, `-`, `*`, `/`, `%`.
    pub(super) fn arithmetic(
        &mut self,
        expr: &ast::Expr,
        operator: BinaryOp,
        operator_span: Span,
        left: &ast::Expr,
        right: &ast::Expr,
        expected: Option<&Type>,
    ) -> Elab<Value> {
        let op = match operator {
            BinaryOp::Add => Op::Add,
            BinaryOp::Sub => Op::Sub,
            BinaryOp::Mul => Op::Mul,
            BinaryOp::Div => Op::Div,
            BinaryOp::Rem => Op::Rem,
            _ => unreachable!("only the arithmetic operators come here"),
        };
        // Two literals without a suffix take the type expected of them, as
        // `let x: u8 = 1 + 2` needs; otherwise both sides are typed as the
        // two sides of a comparison are, at one type.
        let (left_value, right_value) = match expected {
            Some(expected)
                if self.in_constant
                    && self.formula == Some("the value of a constant")
                    && expected.as_machine().is_some() =>
            {
                (self.check(left, expected)?, self.check(right, expected)?)
            }
            Some(expected)
                if untyped_literal(left)
                    && untyped_literal(right)
                    && (expected.as_machine().is_some()
                        || same_type(expected, &Type::Int)
                        || self.is_natural(expected)) =>
            {
                let left_value = self.check(left, expected)?;
                let right_value = self.check(right, expected)?;
                (left_value, right_value)
            }
            _ => {
                let what = format!("`{}`", operator.spelling());
                self.operands_of(&what, left, right, self.formula.is_some())?
            }
        };
        let operands = vec![
            Operand {
                value: left_value,
                span: left.span,
            },
            Operand {
                value: right_value,
                span: right.span,
            },
        ];
        self.operate(op, operands, operator_span, expr.span)
    }

    /// `-inner`, where `inner` is not a literal (a negative literal is one
    /// literal, read in `exprs`).
    pub(super) fn negate(
        &mut self,
        expr: &ast::Expr,
        operator_span: Span,
        inner: &ast::Expr,
        expected: Option<&Type>,
    ) -> Elab<Value> {
        let value = match expected {
            Some(expected) if untyped_literal(inner) => self.check(inner, expected)?,
            _ => self.infer(inner)?,
        };
        let operands = vec![Operand {
            value,
            span: inner.span,
        }];
        self.operate(Op::Neg, operands, operator_span, expr.span)
    }

    /// The operator on operands of one type: a term of `Int`, or a
    /// statement at a machine type, or a refusal.
    fn operate(
        &mut self,
        op: Op,
        operands: Vec<Operand>,
        operator_span: Span,
        span: Span,
    ) -> Elab<Value> {
        let ty = operands[0].value.ty.clone();
        if self.is_natural(&ty) {
            if op == Op::Neg {
                return self.fail(
                    "L0237",
                    "Nat has no unary minus; convert to Int first",
                    operator_span,
                );
            }
            let mut values = Vec::new();
            let mut nonnegative = Vec::new();
            for operand in operands {
                let term = self.term(&operand.value, operand.span)?;
                nonnegative.push(Proof::OfTerm(Term::proj(term, 1)));
                values.push(self.natural_integer(operand.value, operand.span)?);
            }
            let terms = values
                .iter()
                .map(|value| crate::typed::value_term(&value.expr).expect("logical Nat operand"))
                .collect::<Vec<_>>();
            if matches!(op, Op::Div | Op::Rem) && terms[1] == Term::int(0) {
                for axiom in [
                    Axiom::IntDivZero(terms[0].clone()),
                    Axiom::IntDivRem(terms[0].clone(), terms[1].clone()),
                ] {
                    let proof = Proof::Axiom(axiom);
                    let checked = crate::kernel::infer_proof(&mut self.ctx, &proof);
                    let claim = self.kernel(checked, span)?;
                    self.know(proof, claim);
                }
            }
            let evidence = match op {
                Op::Add => Some(Proof::linear(
                    Term::int_le(
                        Term::int(0),
                        Term::int_add(terms[0].clone(), terms[1].clone()),
                    ),
                    1,
                    vec![(nonnegative[0].clone(), 1), (nonnegative[1].clone(), 1)],
                )),
                Op::Mul => Some(Proof::implies_elim(
                    Proof::implies_elim(
                        Proof::Axiom(Axiom::IntLeMul(terms[0].clone(), terms[1].clone())),
                        nonnegative[0].clone(),
                    ),
                    nonnegative[1].clone(),
                )),
                Op::Div => Some(super::naturals::quotient_nonnegative(
                    terms[0].clone(),
                    terms[1].clone(),
                    nonnegative[0].clone(),
                    nonnegative[1].clone(),
                    self.theory.int_mul_le_mul_nonneg,
                )),
                Op::Rem => Some(Proof::implies_elim(
                    Proof::Axiom(Axiom::IntRemNonneg(terms[0].clone(), terms[1].clone())),
                    nonnegative[0].clone(),
                )),
                _ => None,
            };
            let value = Value::new(
                Expr::IntArith {
                    op,
                    operands: values.into_iter().map(|value| value.expr).collect(),
                },
                Type::Int,
            );
            return self.make_natural(value, evidence, operator_span);
        }
        if same_type(&ty, &Type::Int) {
            return Ok(Value::new(
                Expr::IntArith {
                    op,
                    operands: operands.into_iter().map(|o| o.value.expr).collect(),
                },
                Type::Int,
            ));
        }
        let Some(machine) = ty.as_machine() else {
            let shown = self.show_type(&ty);
            return self.fail(
                "L0238",
                format!(
                    "`{}` is defined on the integer types, `u8` to `i64` and `Int`, and this is `{shown}`",
                    op.symbol()
                ),
                operator_span,
            );
        };
        if op.row(machine).is_none() {
            return self.fail(
                "L0237",
                format!(
                    "unary `-` exists at the signed types only, and this is `{}`",
                    machine.name()
                ),
                operator_span,
            );
        }
        if self.in_constant && self.formula == Some("the value of a constant") {
            return self.operate_in_constant(op, machine, operands, operator_span);
        }
        if self.total {
            return self.fail("L0270", "machine arithmetic cannot execute in logic; observe operands through their logical model", operator_span);
        }
        self.operate_at_runtime(op, machine, operands, operator_span, span)
    }

    /// A physical constant must evaluate without panic. Check the same
    /// operator premises in the kernel, then use its closed evaluation rule.
    /// No runtime statement or interpreter is used to establish the value.
    fn operate_in_constant(
        &mut self,
        op: Op,
        ty: MachineInt,
        operands: Vec<Operand>,
        span: Span,
    ) -> Elab<Value> {
        let row = op.row(ty).expect("checked by the caller");
        let terms = operands
            .iter()
            .map(|operand| self.term(&operand.value, operand.span))
            .collect::<Elab<Vec<_>>>()?;
        for premise in row.fits(&self.prelude, &terms) {
            let safe = self
                .constant_condition(&premise)
                .is_some_and(|proof| check_proof(&mut self.ctx, &proof, &premise).is_ok());
            if !safe {
                return self.fail("L0235", format!(
                    "constant `{}` at `{}` must be evaluable without overflow or division by zero",
                    op.symbol(), ty.name()), span);
            }
        }
        let evaluated =
            crate::kernel::infer_proof(&mut self.ctx, &Proof::Evaluate(row.applied(&terms)));
        if let Ok(Term::Eq(_, _, value)) = evaluated
            && let Some((found, integer)) = value.machine_value()
            && found == ty
            && let Some(value) = integer.to_i128()
        {
            return Ok(Value::new(Expr::Literal(ty, value), Type::machine(ty)));
        }
        self.fail(
            "L0270",
            "constant arithmetic needs closed operands within the evaluation budget",
            span,
        )
    }

    /// Closed operator conditions need evaluation, not proof search or a
    /// stored certificate. Signed division also has an implication premise.
    fn constant_condition(&mut self, claim: &Term) -> Option<Proof> {
        if let Some(proof) = self.evaluated(claim) {
            return Some(proof);
        }
        let Term::Implies(premise, conclusion) = claim else {
            return None;
        };
        if let Some(proof) = self.evaluated(conclusion) {
            return Some(Proof::implies_intro((**premise).clone(), |_| proof));
        }
        let negation = self.prelude.not_prop((**premise).clone());
        let impossible = self.evaluated(&negation)?;
        Some(Proof::implies_intro((**premise).clone(), |assumption| {
            Proof::CaseProof {
                scrutinee: Box::new(Proof::implies_elim(impossible, assumption)),
                goal: (**conclusion).clone(),
                arms: vec![],
            }
        }))
    }

    /// The statement: the result is declared with its equation, the wrapped
    /// meaning is a fact, the obligation is discharged under `no_panic`,
    /// and what the statement teaches is assumed.
    fn operate_at_runtime(
        &mut self,
        op: Op,
        ty: MachineInt,
        operands: Vec<Operand>,
        operator_span: Span,
        span: Span,
    ) -> Elab<Value> {
        let row = op.row(ty).expect("checked by the caller");
        let mut terms = Vec::new();
        let mut exprs = Vec::new();
        let mut texts = Vec::new();
        for operand in operands {
            terms.push(self.term(&operand.value, operand.span)?);
            exprs.push(operand.value.expr);
            texts.push(self.text(operand.span).to_string());
        }
        let result = VarId::fresh();
        let equation = HypId::fresh();
        let applied = row.applied(&terms);
        let label = self
            .text(span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.labels.insert(result, label);
        let machine_type = Type::machine(ty);
        // The obligation speaks of the operands, and is discharged before
        // anything about the result is a fact.
        let premises = row.fits(&self.prelude, &terms);
        let mut fits = None;
        if self.promises.no_panic && row.panic() != Panic::Never {
            let mut proofs = Vec::new();
            for (index, premise) in premises.iter().enumerate() {
                proofs.push(self.obligation(row, index, premise, &texts, result, operator_span)?);
            }
            fits = Some(proofs);
        } else if row.panic() != Panic::Never {
            // Bounded, kernel-checked search in the PRE-operation context.
            // A failed attempt keeps the runtime check, without a diagnostic.
            fits = premises
                .iter()
                .map(|premise| {
                    self.stored_or(premise, |env| env.discharge(premise))
                        .map(|(proof, _)| proof)
                })
                .collect();
        }
        let defined = self.ctx.define_with(result, equation, &applied);
        self.kernel(defined, span)?;
        self.facts.push(Fact::definition(
            Proof::hyp(equation),
            Term::eq(machine_type.clone(), Term::var(result), applied),
        ));
        // The wrapped meaning, `op_model` moved onto the result: what holds
        // in every build.
        let meaning = row.meaning(&terms);
        let wrapped = Proof::Transport {
            eq: Box::new(symm_at(
                &machine_type,
                &Term::var(result),
                Proof::hyp(equation),
            )),
            template: Term::eq(machine_type.clone(), Term::Bound(0), meaning.clone()),
            proof: Box::new(Proof::Axiom(Axiom::OpModel(op, ty, terms.clone()))),
        };
        self.know(
            wrapped.clone(),
            Term::eq(machine_type, Term::var(result), meaning),
        );
        let mut learned = Vec::new();
        if row.panic() == Panic::Overflow {
            let claim = Term::eq(
                Type::Int,
                Term::view(ty, Term::var(result)),
                row.exact_term(&terms),
            );
            learned.push(self.learn(claim, span)?);
        }
        if row.panic() == Panic::Division {
            for (index, premise) in premises.into_iter().enumerate() {
                let hyp = self.learn(premise, span)?;
                learned.push(hyp);
                if index == 0 {
                    self.divisor_is_not_zero(ty, &terms[1], hyp);
                }
            }
            self.exact_division(row, &terms, result, wrapped);
        }
        Ok(Value::new(
            Expr::Operate {
                op,
                ty,
                operands: exprs,
                result,
                equation,
                fits,
                learned,
            },
            Type::machine(ty),
        ))
    }

    /// The exact result of a division, `view(q) == view(a) / view(b)`,
    /// known after `a / b` or `a % b` when the quotient or remainder can be
    /// shown to lie in the type's range, which the arithmetic procedure
    /// does for a divisor that is a literal, from the division facts it
    /// adds: the result is `wrap` of the exact value by the model, and
    /// `view_wrap` reads the view of `wrap(n)` back as `n` within the
    /// range. Nothing is assumed: the fact is a derivation the kernel
    /// checks here and again wherever it is used. When the range cannot be
    /// shown, nothing is known beyond the wrapped result.
    fn exact_division(&mut self, row: Row, terms: &[Term], result: VarId, wrapped: Proof) {
        let ty = row.ty;
        let exact = row.exact_term(terms);
        let lower = Term::int_le(Term::Int(ty.min()), exact.clone());
        let upper = Term::int_le(exact.clone(), Term::Int(ty.max()));
        let Some((lower, _)) = self.discharge(&lower) else {
            return;
        };
        let Some((upper, _)) = self.discharge(&upper) else {
            return;
        };
        // view(wrap(e)) == e, from the two bounds.
        let of_wrap = Proof::implies_elim(
            Proof::implies_elim(Proof::Axiom(Axiom::ViewWrap(ty, exact.clone())), lower),
            upper,
        );
        // view(result) == view(wrap(e)), from result == wrap(e).
        let views_equal = Self::views_of_equal(ty, &Term::var(result), wrapped);
        let viewed = Term::view(ty, Term::var(result));
        let proof = Proof::Transport {
            eq: Box::new(of_wrap),
            template: Term::eq(Type::Int, viewed.clone(), Term::Bound(0)),
            proof: Box::new(views_equal),
        };
        let claim = Term::eq(Type::Int, viewed, exact);
        if check_proof(&mut self.ctx, &proof, &claim).is_ok() {
            self.know(proof, claim);
        }
    }

    /// A hypothesis the checker will assume after the statement, under a
    /// fresh identity, known here as the checker states it and also with
    /// the views of its literals computed, which is how a claim written
    /// with a literal of `Int` reads.
    fn learn(&mut self, claim: Term, span: Span) -> Elab<HypId> {
        let hyp = HypId::fresh();
        self.assume(hyp, claim.clone(), span)?;
        let (computed, steps) = self.literal_views(&claim);
        if !steps.is_empty() {
            self.facts
                .push(Fact::new(forward(Proof::hyp(hyp), steps), computed));
        }
        Ok(hyp)
    }

    /// A derived fact, known as stated and with the views of its literals
    /// computed.
    fn know(&mut self, proof: Proof, claim: Term) {
        let (computed, steps) = self.literal_views(&claim);
        if !steps.is_empty() {
            self.facts
                .push(Fact::new(forward(proof.clone(), steps), computed));
        }
        self.facts.push(Fact::new(proof, claim));
    }

    /// The premise `view(b) != 0` of a division, as the claim `b != 0` at
    /// the machine type, which is how it is written in code: an equality
    /// of the values gives one of the views, and the view of the literal
    /// `0` is `0`.
    fn divisor_is_not_zero(&mut self, ty: MachineInt, divisor: &Term, premise: HypId) {
        let zero = Term::machine_int(ty, 0);
        let equal = Term::eq(Type::machine(ty), divisor.clone(), zero.clone());
        let divisor = divisor.clone();
        let proof = Proof::implies_intro(equal.clone(), |equal| {
            let views_equal = Self::views_of_equal(ty, &divisor, equal);
            let view_of_zero = Term::view(ty, zero);
            let as_int = Proof::Transport {
                eq: Box::new(Proof::Literal(view_of_zero)),
                template: Term::eq(Type::Int, Term::view(ty, divisor.clone()), Term::Bound(0)),
                proof: Box::new(views_equal),
            };
            Proof::implies_elim(Proof::hyp(premise), as_int)
        });
        let claim = self.prelude.not_prop(equal);
        if check_proof(&mut self.ctx, &proof, &claim).is_ok() {
            self.facts.push(Fact::new(proof, claim));
        }
    }

    // --- The obligation ---------------------------------------------------------

    /// Evidence of one premise of the row, or the diagnostic that says what
    /// would give it. Recorded as a hole, since it is filled as one.
    fn obligation(
        &mut self,
        row: Row,
        index: usize,
        premise: &Term,
        texts: &[String],
        result: VarId,
        operator_span: Span,
    ) -> Elab<Proof> {
        let started = Instant::now();
        let checkpoint = crate::measurement::checkpoint();
        self.normalization_exhausted = false;
        // The proofs file first; the tiers on a miss (`stored.rs`).
        let found = self.stored_or(premise, |env| env.discharge(premise));
        let (proof, tier) = match found {
            Some((proof, tier)) => (
                check_proof(&mut self.ctx, &proof, premise)
                    .ok()
                    .map(|()| proof),
                tier,
            ),
            None => (None, "unsolved"),
        };
        self.holes.push(HoleReport {
            span: operator_span,
            solved: proof.is_some(),
            tier,
            proof_size: proof.as_ref().map_or(0, super::solve::proof_size),
            micros: started.elapsed().as_micros(),
            measurements: crate::measurement::since(checkpoint),
            found: proof.as_ref().map(|proof| FoundProof {
                context: self.ctx.clone(),
                claim: premise.clone(),
                proof: proof.clone(),
            }),
        });
        if let Some(proof) = proof {
            return Ok(proof);
        }
        if self.locked_miss(premise, operator_span) {
            return Err(());
        }
        self.report_obligation(row, index, premise, texts, result, operator_span);
        Err(())
    }

    /// The tiers, in order: exact, a fact in scope that is the premise, the
    /// two read with the views of their literals computed, since the
    /// premise says `view(1u8)` where a claim says `1`; computed, with
    /// names replaced; evaluation; then the arithmetic procedure.
    pub(super) fn discharge(&mut self, premise: &Term) -> Option<(Proof, &'static str)> {
        let exact_timer = crate::measurement::start("exact");
        let (goal, mut steps) = self.literal_views(premise);
        let stated: Vec<Fact> = self.facts.iter().rev().cloned().collect();
        for fact in stated {
            let (claim, fact_steps) = self.literal_views(&fact.claim);
            if same(&claim, &goal) {
                let proof = forward(fact.proof, fact_steps);
                return Some((self.back_to_stated(proof, steps)?, "exact"));
            }
        }
        drop(exact_timer);
        let computed_timer = crate::measurement::start("computed");
        let known = self.knowledge();
        let (normal, more) = self.normalize(&goal, &known.definitions);
        steps.extend(more);
        let (normal, more) = self.literal_views(&normal);
        steps.extend(more);
        let mut found = None;
        for (_, fact) in known.facts.iter().rev() {
            let (claim, fact_steps) = self.literal_views(&fact.claim);
            if same(&claim, &normal) {
                found = Some((forward(fact.proof.clone(), fact_steps), "computed"));
                break;
            }
        }
        drop(computed_timer);
        let found = found
            .or_else(|| {
                self.computed_from(&normal, &known)
                    .map(|proof| (proof, "computed"))
            })
            .or_else(|| {
                self.nonzero_from_machine(&normal, &known)
                    .map(|proof| (proof, "computed"))
            })
            .or_else(|| self.evaluated(&normal).map(|proof| (proof, "evaluation")));
        if let Some((proof, tier)) = found {
            return Some((self.back_to_stated(proof, steps)?, tier));
        }
        let proof = {
            let _timer = crate::measurement::start("arithmetic");
            self.by_arithmetic(&normal, &known).ok()?
        };
        Some((self.back_to_stated(proof, steps)?, "arithmetic"))
    }

    /// The premise `view[T](b) != k` of a division from a fact `b != kT`
    /// at the machine type, which is how a nonzero divisor is written in
    /// code: an equality of the views gives one of the values, by the
    /// injectivity of the view, once the view of the literal is computed.
    fn nonzero_from_machine(
        &mut self,
        normal: &Term,
        known: &super::solve::Known,
    ) -> Option<Proof> {
        let Term::Implies(premise, conclusion) = normal else {
            return None;
        };
        if **conclusion != self.prelude.falsehood_prop() {
            return None;
        }
        let Term::Eq(Type::Int, viewed, literal) = &**premise else {
            return None;
        };
        let (Term::Prim(Prim::View(ty), operands), Term::Int(k)) = (&**viewed, &**literal) else {
            return None;
        };
        let [b] = operands.as_slice() else {
            return None;
        };
        if !ty.contains(k) {
            return None;
        }
        let (ty, b, k) = (*ty, b.clone(), Term::machine(*ty, k.clone()));
        let at_machine = self
            .prelude
            .not_prop(Term::eq(Type::machine(ty), b.clone(), k.clone()));
        let fact = known
            .facts
            .iter()
            .rev()
            .find(|(_, fact)| same(&fact.claim, &at_machine))?
            .1
            .proof
            .clone();
        let views_equal = Term::eq(Type::Int, viewed.as_ref().clone(), literal.as_ref().clone());
        Some(Proof::implies_intro(views_equal, |views_equal| {
            // view(b) == k, and view(kT) == k by computation, so the views
            // are equal, and so are the values.
            let view_of_k = Term::view(ty, k.clone());
            let literal_step = symm_at(&Type::Int, &view_of_k, Proof::Literal(view_of_k.clone()));
            let of_views = Proof::Transport {
                eq: Box::new(literal_step),
                template: Term::eq(Type::Int, Term::view(ty, b.clone()), Term::Bound(0)),
                proof: Box::new(views_equal),
            };
            Proof::implies_elim(fact, self.equal_of_views(ty, &b, &k, of_views))
        }))
    }

    /// The views of literals in the term computed, `view[T](3T)` to `3`,
    /// each step a computation axiom.
    pub(super) fn literal_views(&mut self, term: &Term) -> (Term, Vec<Step>) {
        self.compute_where(term, &|candidate| {
            matches!(candidate, Term::Prim(Prim::View(_), operands)
                if matches!(operands.as_slice(), [literal] if literal.machine_value().is_some()))
        })
    }

    // --- The arithmetic tier ------------------------------------------------------

    /// The arithmetic procedure over the facts in scope, on a goal with its
    /// names replaced and its literal views computed: the fourth tier of a
    /// hole (`solve`) and of an operator's obligation, the same call.
    ///
    /// The procedure reads the context, so the facts are assumed in a copy
    /// of it, each in every spelling it has (`assume_spellings`), and the
    /// certificate's hypotheses are then replaced by the facts' own proofs,
    /// so that the proof returned stands in the real context. A goal at a
    /// machine type is bridged to the views first: `a ==[T] b` is proved as
    /// `view(a) == view(b)` and closed by the injectivity of the view. An
    /// implication, the premise of a division or a claim `a != b`, has each
    /// antecedent assumed, in its spellings too, and the conclusion proved;
    /// the proof is closed over them.
    ///
    /// On failure, the procedure's report, when the goal was one it could
    /// read: the counterexample and the budget in it are what the
    /// diagnostics show.
    pub(super) fn by_arithmetic(
        &mut self,
        normal: &Term,
        known: &super::solve::Known,
    ) -> Result<Proof, Option<GaveUp>> {
        let (mut scratch, mut replacements) = self.arithmetic_context(known);
        let mut antecedents = Vec::new();
        let mut goal = normal.clone();
        while let Term::Implies(premise, conclusion) = goal {
            let id = HypId::fresh();
            scratch
                .assume_with(id, (*premise).clone())
                .map_err(|_| None)?;
            self.assume_spellings(
                &mut scratch,
                &mut replacements,
                Proof::hyp(id),
                &premise,
                false,
            );
            antecedents.push((id, *premise));
            goal = *conclusion;
        }
        // A machine equation is bridged to the views.
        let (goal, close): (Term, Option<(MachineInt, Term, Term)>) = match &goal {
            Term::Eq(ty, a, b) if ty.as_machine().is_some() => {
                let ty = ty.as_machine().expect("checked");
                (
                    Term::eq(
                        Type::Int,
                        Term::view(ty, (**a).clone()),
                        Term::view(ty, (**b).clone()),
                    ),
                    Some((ty, (**a).clone(), (**b).clone())),
                )
            }
            other => (other.clone(), None),
        };
        let proof = arith::prove(&scratch, Some(self.prelude), &goal, &Budget::default()).map_err(
            |gave_up| match gave_up.reason {
                arith::Reason::NotLinear(_) | arith::Reason::NoPrelude => None,
                _ => Some(gave_up),
            },
        )?;
        let proof = match close {
            Some((ty, a, b)) => self.equal_of_views(ty, &a, &b, proof),
            None => proof,
        };
        let mut proof = substitute(proof, &[], &replacements);
        for (id, premise) in antecedents.into_iter().rev() {
            let inner = proof;
            proof =
                Proof::implies_intro(premise, |assumed| substitute(inner, &[], &[(id, assumed)]));
        }
        replacements.clear();
        Ok(proof)
    }

    /// A copy of the context with every fact in scope assumed in each of
    /// its spellings, and the proof each assumption stands for.
    fn arithmetic_context(
        &mut self,
        known: &super::solve::Known,
    ) -> (crate::kernel::Context, Vec<(HypId, Proof)>) {
        let mut scratch = self.ctx.clone();
        let mut replacements = Vec::new();
        let stated: Vec<Fact> = self
            .facts
            .iter()
            .filter(|fact| !fact.definition)
            .cloned()
            .collect();
        let computed: Vec<Fact> = known.facts.iter().map(|(_, fact)| fact.clone()).collect();
        for fact in stated.into_iter().chain(computed) {
            self.assume_spellings(
                &mut scratch,
                &mut replacements,
                fact.proof,
                &fact.claim,
                true,
            );
        }
        (scratch, replacements)
    }

    /// Assumes a fact in the scratch context in every spelling the
    /// procedure can read it in: as it stands, unless it is already a
    /// hypothesis of the context; with the views of its literals computed,
    /// since `x <= 3` between machine values is `int_le(view(x),
    /// view(3T))` and the procedure reads `3`; and bridged to the views by
    /// the kernel's own steps when it is a claim at a machine type, `a ==[T]
    /// b` giving `view(a) == view(b)` by congruence, and the outcome of a
    /// comparison the branch taken knows, `c == true` or `c == false`,
    /// giving the proposition over the views by `cmp_reflect`, each again
    /// with its literal views computed. With `as_stated` false the claim
    /// itself is already assumed, and only the other spellings are added.
    fn assume_spellings(
        &mut self,
        scratch: &mut crate::kernel::Context,
        replacements: &mut Vec<(HypId, Proof)>,
        proof: Proof,
        claim: &Term,
        as_stated: bool,
    ) {
        let mut spellings: Vec<(Proof, Term)> = Vec::new();
        if as_stated {
            spellings.push((proof.clone(), claim.clone()));
        }
        if let Some((bridged, over_views)) = self.bridged_to_views(&proof, claim) {
            spellings.push((bridged, over_views));
        }
        for (proof, claim) in spellings {
            let (computed, steps) = self.literal_views(&claim);
            let mut forms = vec![(proof.clone(), claim)];
            if !steps.is_empty() {
                forms.push((forward(proof, steps), computed));
            }
            for (proof, claim) in forms {
                let is_hypothesis = |binding: crate::kernel::Binding<'_>| matches!(binding, crate::kernel::Binding::Hyp { prop, .. } if same(prop, &claim));
                if matches!(proof, Proof::Hyp(HypRef::Free(_)))
                    && scratch.bindings().any(is_hypothesis)
                {
                    // Already in the context as it stands.
                    continue;
                }
                let id = HypId::fresh();
                if scratch.assume_with(id, claim).is_ok() {
                    replacements.push((id, proof));
                }
            }
        }
    }

    /// A fact at a machine type read over the views: `a ==[T] b` as
    /// `view(a) == view(b)`, and `c == true` or `c == false` for a
    /// comparison `c` as what `cmp_reflect` says of it.
    fn bridged_to_views(&self, proof: &Proof, claim: &Term) -> Option<(Proof, Term)> {
        match claim {
            Term::Eq(ty, a, b) if ty.as_machine().is_some() => {
                let ty = ty.as_machine()?;
                let over_views = Term::eq(
                    Type::Int,
                    Term::view(ty, (**a).clone()),
                    Term::view(ty, (**b).clone()),
                );
                Some((Self::views_of_equal(ty, a, proof.clone()), over_views))
            }
            Term::Eq(Type::Bool, test, outcome) => {
                let Term::Bool(outcome) = **outcome else {
                    return None;
                };
                let Term::Prim(Prim::Cmp(op, ty), operands) = &**test else {
                    return None;
                };
                let [a, b] = operands.as_slice() else {
                    return None;
                };
                let positive = op.claim(Term::view(*ty, a.clone()), Term::view(*ty, b.clone()));
                let reflected = if outcome {
                    positive
                } else {
                    self.prelude.not_prop(positive)
                };
                let bridged = Proof::implies_elim(
                    Proof::Axiom(Axiom::CmpReflect((**test).clone(), outcome)),
                    proof.clone(),
                );
                Some((bridged, reflected))
            }
            _ => None,
        }
    }

    /// What the arithmetic procedure says of a goal it could not prove, for
    /// a diagnostic: the values it found that satisfy the arithmetic facts
    /// and violate the goal, or the budget that ran out; nothing when the
    /// goal is not one it reads or it has neither.
    pub(super) fn arithmetic_failure(
        &mut self,
        normal: &Term,
        known: &super::solve::Known,
    ) -> Option<ArithmeticFailure> {
        let Err(Some(gave_up)) = self.by_arithmetic(normal, known) else {
            return None;
        };
        let enumeration_limit = match &gave_up.counterexample {
            Counterexample::TooManyAtoms => Some(format!(
                "counterexample enumeration omitted: MAX_COUNTEREXAMPLE_ATOMS limit of {} was exceeded",
                crate::limits::MAX_COUNTEREXAMPLE_ATOMS
            )),
            Counterexample::NoneInBox => Some(format!(
                "counterexample search found none within -{}..{} (COUNTEREXAMPLE_BOX); this does not prove the claim",
                crate::limits::COUNTEREXAMPLE_BOX,
                crate::limits::COUNTEREXAMPLE_BOX
            )),
            _ => None,
        };
        if let Counterexample::Found(assignment) = gave_up.counterexample {
            if assignment.is_empty() {
                // A closed claim: false, unless it is `False` itself, which
                // facts the procedure does not read may still prove.
                if *normal == self.prelude.falsehood_prop() {
                    return None;
                }
                return Some(ArithmeticFailure::Counterexample(
                    "it is false as it stands".into(),
                ));
            }
            // The point is a counterexample only when every atom is one
            // the procedure models in full: a name, a variable or a field
            // of one, or a quotient or remainder by a literal over names,
            // `(lo + hi) / 2`, which the division facts pin down. An atom
            // that stands for something the procedure cannot read, the
            // view of a wrapped result or of a call, took any value in the
            // point, and the point says nothing about the claim; then
            // there is no counterexample to show. The names come first, in
            // source spelling; the view of a literal is the literal and is
            // not shown.
            let is_literal_view = |atom: &Term| {
                matches!(atom, Term::Prim(Prim::View(_), operands)
                    if matches!(operands.as_slice(), [literal] if literal.machine_value().is_some()))
            };
            let atoms: Vec<&Term> = assignment
                .iter()
                .map(|(atom, _)| atom)
                .filter(|atom| !is_literal_view(atom))
                .collect();
            if atoms.is_empty() || !atoms.iter().all(|atom| source_level(atom)) {
                return None;
            }
            let mut parts: Vec<String> = Vec::new();
            for names_first in [true, false] {
                for (atom, value) in &assignment {
                    if is_literal_view(atom) || is_name(atom) != names_first {
                        continue;
                    }
                    // A name reads bare, `lo = 5`; anything longer is
                    // quoted.
                    let text = self.show(atom);
                    let bare = text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
                    if bare {
                        parts.push(format!("{text} = {value}"));
                    } else {
                        parts.push(format!("`{text}` = {value}"));
                    }
                }
            }
            return Some(ArithmeticFailure::Counterexample(format!(
                "it fails when {}, which the arithmetic facts known here allow",
                parts.join(", ")
            )));
        }
        match gave_up.reason {
            arith::Reason::Budget { name, limit } => {
                let mut message = format!(
                    "the arithmetic procedure ran out of its `{name}` budget of {limit} before it could decide this"
                );
                if let Some(note) = enumeration_limit {
                    message.push_str("; ");
                    message.push_str(&note);
                }
                Some(ArithmeticFailure::Budget(message))
            }
            _ => enumeration_limit.map(ArithmeticFailure::Budget),
        }
    }

    /// `L0235`: the obligation was not discharged. The premise is written
    /// out as source, with what would discharge it.
    fn report_obligation(
        &mut self,
        row: Row,
        index: usize,
        premise: &Term,
        texts: &[String],
        result: VarId,
        operator_span: Span,
    ) {
        let (op, ty) = (row.op, row.ty);
        // An operand as it reads in a claim over `Int`: a literal is one
        // of `Int` as it stands, without its suffix; a name or a path is
        // viewed with `as Int`; anything else is parenthesized first.
        let as_int = |text: &String| {
            let digits = text.trim_start_matches('-');
            let number = digits
                .find(|c: char| !c.is_ascii_digit() && c != '_')
                .map_or(digits, |at| &digits[..at]);
            if !number.is_empty()
                && number.chars().all(|c| c.is_ascii_digit() || c == '_')
                && MachineInt::from_name(&digits[number.len()..]).is_some()
            {
                return format!("{}{number}", &text[..text.len() - digits.len()]);
            }
            if number == digits && !digits.is_empty() {
                return text.clone();
            }
            if text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            {
                format!("{text} as Int")
            } else {
                format!("({text}) as Int")
            }
        };
        let exact = match texts {
            [a, b] => format!("{} {} {}", as_int(a), op.symbol(), as_int(b)),
            [a] => format!("-({})", as_int(a)),
            _ => unreachable!("one or two operands"),
        };
        let (condition, stated) = match (row.panic(), index) {
            (Panic::Overflow, 0) => (
                format!("the {} `{exact}` must be at least {}", noun(op), ty.min()),
                format!("{} <= {exact}", ty.min()),
            ),
            (Panic::Overflow, _) => (
                format!("the {} `{exact}` must be at most {}", noun(op), ty.max()),
                format!("{exact} <= {}", ty.max()),
            ),
            (Panic::Division, 0) => (
                format!("the divisor `{}` must not be zero", texts[1]),
                format!("{} != 0", as_int(&texts[1])),
            ),
            (Panic::Division, _) => (
                format!(
                    "the pair must not be `{}::MIN {} -1`",
                    ty.name(),
                    op.symbol()
                ),
                format!(
                    "{} == {} => {} != -1",
                    as_int(&texts[0]),
                    ty.min(),
                    as_int(&texts[1])
                ),
            ),
            (Panic::Never, _) => unreachable!("the wrapping methods have no obligation"),
        };
        let how = match row.panic() {
            Panic::Division => "may panic",
            _ => "may overflow",
        };
        let mut diagnostic = Diagnostic::error(
            "L0235",
            format!(
                "`{}` on `{}` {how}, and `{}` promises no_panic",
                op.symbol(),
                ty.name(),
                self.item_name
            ),
            operator_span,
        )
        .note(format!("{condition}, and nothing known here shows it"));
        diagnostic.details.proof.claim = Some(stated.clone());
        // The facts that speak of the operands, nearest first, as stated,
        // and a counterexample when the arithmetic procedure has one.
        let subjects = free_variables(premise);
        let mut shown: Vec<String> = Vec::new();
        let mut omitted_facts = false;
        for fact in self.facts.clone().iter().rev() {
            // The result's own meaning says nothing about the premise.
            let theirs = free_variables(&fact.claim);
            if fact.definition || theirs.contains(&result) {
                continue;
            }
            if theirs.iter().any(|variable| subjects.contains(variable)) {
                let claim = self.show(&fact.claim);
                if !shown.contains(&claim) {
                    if shown.len() < crate::limits::MAX_DIAGNOSTIC_FACTS {
                        shown.push(claim);
                    } else {
                        omitted_facts = true;
                    }
                }
            }
        }
        if omitted_facts {
            diagnostic = diagnostic.note(format!(
                "additional facts omitted (MAX_DIAGNOSTIC_FACTS = {})",
                crate::limits::MAX_DIAGNOSTIC_FACTS
            ));
        }
        diagnostic.details.proof.facts_considered = Some(
            shown
                .iter()
                .map(|claim| ConsideredFact {
                    name: None,
                    claim: claim.clone(),
                })
                .collect(),
        );
        if !shown.is_empty() {
            let list: Vec<String> = shown.iter().map(|claim| format!("`{claim}`")).collect();
            diagnostic = diagnostic.note(format!("known here: {}", list.join(", ")));
        }
        let (goal, _) = self.literal_views(premise);
        let known = self.knowledge();
        let (normal, _) = self.normalize(&goal, &known.definitions);
        let (normal, _) = self.literal_views(&normal);
        let computed = self.show(&normal);
        if computed != stated {
            diagnostic.details.proof.claim_after_computing = Some(computed);
        }
        if let Some(failure) = self.arithmetic_failure(&normal, &known) {
            if let ArithmeticFailure::Counterexample(example) = &failure {
                diagnostic.details.proof.counterexample = Some(example.clone());
            }
            diagnostic = diagnostic.note(failure.note());
        }
        if self.normalization_exhausted {
            diagnostic = diagnostic.note(format!("proof construction reached MAX_NORMALIZATION_STEPS ({}); supply a smaller explicit proof step", crate::limits::MAX_NORMALIZATION_STEPS));
        }
        diagnostic = diagnostic.note(format!(
            "a fact in scope stating `{stated}`, or `prove!({stated});` just before this, is what is needed"
        ));
        self.diagnostics.push(diagnostic);
    }
}

/// A variable, or a field of one, as a term of `Int`: the view of one, or
/// one of `Int` itself.
fn is_name(atom: &Term) -> bool {
    fn path(term: &Term) -> bool {
        match term {
            Term::Free(_) => true,
            Term::Proj(target, _) => path(target),
            _ => false,
        }
    }
    match atom {
        Term::Prim(Prim::View(_), operands) => matches!(operands.as_slice(), [x] if path(x)),
        other => path(other),
    }
}

/// Whether an atom of the arithmetic procedure is one it models in full: a
/// name, or a quotient or remainder by a literal of a linear form over
/// names and literals.
fn source_level(atom: &Term) -> bool {
    fn linear(term: &Term) -> bool {
        match term {
            Term::Int(_) => true,
            Term::Prim(Prim::IntAdd | Prim::IntSub | Prim::IntMul | Prim::IntNeg, operands) => {
                operands.iter().all(linear)
            }
            Term::Prim(Prim::IntDiv | Prim::IntRem, operands) => {
                matches!(operands.as_slice(), [dividend, Term::Int(_)] if linear(dividend))
            }
            other => is_name(other),
        }
    }
    is_name(atom)
        || matches!(atom, Term::Prim(Prim::IntDiv | Prim::IntRem, operands)
            if matches!(operands.as_slice(), [dividend, Term::Int(_)] if linear(dividend)))
}

/// What an operator computes, for a message.
fn noun(op: Op) -> &'static str {
    match op {
        Op::Add => "sum",
        Op::Sub => "difference",
        Op::Mul => "product",
        Op::Neg => "negation",
        Op::Div => "quotient",
        Op::Rem => "remainder",
        _ => "result",
    }
}
