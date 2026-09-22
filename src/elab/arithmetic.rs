//! The operators `+`, `-`, `*`, `/`, `%`, and unary minus.
//!
//! On `Int` they are the total primitives of the logic, `int_add` and the
//! rest, and may stand anywhere, since nothing of `Int` runs. On a machine
//! integer type they are the rows of the table in `src/kernel/ops.rs`, and
//! may panic, which decides where they may stand and what is known of
//! their result, by the rule of the Vision (Integers, in code and in
//! propositions):
//!
//! - In a proposition, or anywhere else nothing runs, an operator on a
//!   machine type is refused, because it may panic: the message offers
//!   `a as Int + b as Int` for the exact sum and `a.wrapping_add(b)` for
//!   the wrapped one.
//! - In code, `let s = a + b` becomes a statement of the check IR,
//!   `exec::OperateStmt`, whose result `s` is known by its equation to be
//!   the wrapped result, `s == (a as Int + b as Int) as T`, which holds in
//!   every build. Nothing else is known, unless the function promises
//!   `no_panic`: then the operator carries an obligation, the premises of
//!   `Row::fits` that say it does not panic, discharged here as a hole
//!   would be, and the exact result is known afterwards, `s as Int == a as
//!   Int + b as Int`. An operation that panics in every build, `/` or `%`,
//!   teaches its condition to what follows in either case: after `a / b`
//!   the divisor is known not to be zero, as `c` is known after
//!   `assert!(c)`.
//!
//! The obligation is discharged by the solver's three tiers, exact,
//! computed, and evaluation, with one adjustment for its shape: the
//! premises speak of the views of the operands, so a view of a literal is
//! computed on both sides, which lets `prove!(n as Int + 1 <= u32::MAX as
//! Int)` on the line before serve as the fact it is. When the tiers fail,
//! the arithmetic procedure of `src/arith` is asked, over the facts in
//! scope, because the premises of an unsigned type include a lower bound
//! that only the ranges of the views establish; E7 gives holes the same
//! tier, and this is the same call. A proof from any tier is checked by
//! the kernel before it is used. When every tier fails, the diagnostic
//! writes the premise out and says that a fact in scope stating it, or a
//! `prove!` of it just before, is what is needed.

use std::time::Instant;

use crate::arith::{self, Budget, Counterexample};
use crate::ast::{self, BinaryOp};
use crate::diagnostic::Diagnostic;
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
                if untyped_literal(left)
                    && untyped_literal(right)
                    && (expected.as_machine().is_some() || same_type(expected, &Type::Int)) =>
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
        if self.formula.is_some() || self.total {
            return self.refuse_where_nothing_runs(op, machine, &operands, operator_span);
        }
        self.operate_at_runtime(op, machine, operands, operator_span, span)
    }

    /// `L0236`: an operator on a machine type where nothing runs, in a
    /// proposition or in a function of the logic. The two things that can
    /// be written instead are named.
    fn refuse_where_nothing_runs<T>(
        &mut self,
        op: Op,
        ty: MachineInt,
        operands: &[Operand],
        operator_span: Span,
    ) -> Elab<T> {
        let texts: Vec<String> = operands
            .iter()
            .map(|operand| self.text(operand.span).to_string())
            .collect();
        let exact = match texts.as_slice() {
            [a, b] => format!("{a} as Int {} {b} as Int", op.symbol()),
            [a] => format!("-({a} as Int)"),
            _ => unreachable!("one or two operands"),
        };
        let wrapped = match (op, texts.as_slice()) {
            (Op::Add, [a, b]) => Some(format!("{a}.wrapping_add({b})")),
            (Op::Sub, [a, b]) => Some(format!("{a}.wrapping_sub({b})")),
            (Op::Mul, [a, b]) => Some(format!("{a}.wrapping_mul({b})")),
            (Op::Neg, [a]) => Some(format!("{a}.wrapping_neg()")),
            _ => None,
        };
        let how = match op.panic() {
            Panic::Division => "may panic, on a zero divisor",
            _ => "may panic, on overflow",
        };
        let where_ = match self.formula {
            Some(place) => format!("so it is not {place}"),
            None => format!(
                "and `{}` is a function of the logic, which runs nothing",
                self.item_name
            ),
        };
        let what = match op.panic() {
            Panic::Division => "quotient",
            _ if op == Op::Neg => "negation",
            _ if op == Op::Add => "sum",
            _ if op == Op::Sub => "difference",
            _ => "product",
        };
        let what = if op == Op::Rem { "remainder" } else { what };
        let mut diagnostic = Diagnostic::error(
            "L0236",
            format!("`{}` on `{}` {how}, {where_}", op.symbol(), ty.name()),
            operator_span,
        );
        diagnostic = match wrapped {
            Some(wrapped) => diagnostic.note(format!(
                "write `{exact}` for the exact {what}, or `{wrapped}` for the wrapped one"
            )),
            None => diagnostic.note(format!(
                "write `{exact}` for the exact {what} on `Int`, which is total: `a / 0` is `0` and `a % 0` is `a`"
            )),
        };
        self.diagnostics.push(diagnostic);
        Err(())
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
        let defined = self.ctx.define_with(result, equation, &applied);
        self.kernel(defined, span)?;
        let label = self
            .text(span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.labels.insert(result, label);
        let machine_type = Type::machine(ty);
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
        self.know(wrapped, Term::eq(machine_type, Term::var(result), meaning));

        let premises = row.fits(&self.prelude, &terms);
        let mut fits = None;
        let mut learned = Vec::new();
        if self.promises.no_panic && row.panic() != Panic::Never {
            let mut proofs = Vec::new();
            for (index, premise) in premises.iter().enumerate() {
                proofs.push(self.obligation(row, index, premise, &texts, result, operator_span)?);
            }
            fits = Some(proofs);
            if row.panic() == Panic::Overflow {
                let claim = Term::eq(
                    Type::Int,
                    Term::view(ty, Term::var(result)),
                    row.exact_term(&terms),
                );
                learned.push(self.learn(claim, span)?);
            }
        }
        if row.panic() == Panic::Division {
            for (index, premise) in premises.into_iter().enumerate() {
                let hyp = self.learn(premise, span)?;
                learned.push(hyp);
                if index == 0 {
                    self.divisor_is_not_zero(ty, &terms[1], hyp);
                }
            }
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
        let found = self.discharge(premise);
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
            found: proof.as_ref().map(|proof| FoundProof {
                context: self.ctx.clone(),
                claim: premise.clone(),
                proof: proof.clone(),
            }),
        });
        if let Some(proof) = proof {
            return Ok(proof);
        }
        self.report_obligation(row, index, premise, texts, result, operator_span);
        Err(())
    }

    /// The tiers, in order: exact, a fact in scope that is the premise, the
    /// two read with the views of their literals computed, since the
    /// premise says `view(1u8)` where a claim says `1`; computed, with
    /// names replaced; evaluation; then the arithmetic procedure.
    fn discharge(&mut self, premise: &Term) -> Option<(Proof, &'static str)> {
        let (goal, mut steps) = self.literal_views(premise);
        let stated: Vec<Fact> = self.facts.iter().rev().cloned().collect();
        for fact in stated {
            let (claim, fact_steps) = self.literal_views(&fact.claim);
            if same(&claim, &goal) {
                let proof = forward(fact.proof, fact_steps);
                return Some((self.back_to_stated(proof, steps)?, "exact"));
            }
        }
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
        let proof = self.by_arithmetic(&normal, &known)?;
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
    fn literal_views(&mut self, term: &Term) -> (Term, Vec<Step>) {
        self.compute_where(term, &|candidate| {
            matches!(candidate, Term::Prim(Prim::View(_), operands)
                if matches!(operands.as_slice(), [literal] if literal.machine_value().is_some()))
        })
    }

    /// The arithmetic procedure over the facts in scope, on the goal with
    /// its names replaced and its literal views computed. The procedure
    /// reads the context, so the facts are assumed in a copy of it, each
    /// as it stands and as computed, with names replaced and the views of
    /// literals computed, so that `x <= 3` is read as the bound it is and
    /// a fact about `d` serves a goal about what `d` stands for; the
    /// certificate's hypotheses are then replaced by the facts' own
    /// proofs. This is the call E7 makes for a hole.
    fn by_arithmetic(&mut self, normal: &Term, known: &super::solve::Known) -> Option<Proof> {
        let (mut scratch, mut replacements) = self.arithmetic_context(known);
        // The premise of a division is an implication, `view(b) == 0 =>
        // False`, or two deep at a signed type: each antecedent is assumed
        // and the conclusion proved, and the proof is closed over them.
        let mut antecedents = Vec::new();
        let mut goal = normal.clone();
        while let Term::Implies(premise, conclusion) = goal {
            let id = HypId::fresh();
            scratch.assume_with(id, (*premise).clone()).ok()?;
            antecedents.push((id, *premise));
            goal = *conclusion;
        }
        let proof = arith::prove(&scratch, Some(self.prelude), &goal, &Budget::default()).ok()?;
        let mut proof = substitute(proof, &[], &replacements);
        for (id, premise) in antecedents.into_iter().rev() {
            let inner = proof;
            proof =
                Proof::implies_intro(premise, |assumed| substitute(inner, &[], &[(id, assumed)]));
        }
        replacements.clear();
        Some(proof)
    }

    /// A copy of the context with every fact in scope assumed in both
    /// spellings, and the proof each assumption stands for.
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
            let (claim, steps) = self.literal_views(&fact.claim);
            if steps.is_empty() && matches!(fact.proof, Proof::Hyp(HypRef::Free(_))) {
                // Already in the context as it stands.
                continue;
            }
            let id = HypId::fresh();
            if scratch.assume_with(id, claim).is_ok() {
                replacements.push((id, forward(fact.proof, steps)));
            }
        }
        (scratch, replacements)
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
        // The facts that speak of the operands, nearest first, as stated,
        // and a counterexample when the arithmetic procedure has one.
        let subjects = free_variables(premise);
        let mut shown: Vec<String> = Vec::new();
        for fact in self.facts.clone().iter().rev() {
            // The result's own meaning says nothing about the premise.
            let theirs = free_variables(&fact.claim);
            if fact.definition || theirs.contains(&result) {
                continue;
            }
            if theirs.iter().any(|variable| subjects.contains(variable)) {
                let claim = self.show(&fact.claim);
                if !shown.contains(&claim) && shown.len() < 6 {
                    shown.push(claim);
                }
            }
        }
        if !shown.is_empty() {
            let list: Vec<String> = shown.iter().map(|claim| format!("`{claim}`")).collect();
            diagnostic = diagnostic.note(format!("known here: {}", list.join(", ")));
        }
        let (goal, _) = self.literal_views(premise);
        let known = self.knowledge();
        let (normal, _) = self.normalize(&goal, &known.definitions);
        let (normal, _) = self.literal_views(&normal);
        if let Some(counterexample) = self.counterexample(&normal, &known) {
            diagnostic = diagnostic.note(counterexample);
        }
        diagnostic = diagnostic.note(format!(
            "a fact in scope stating `{stated}`, or `prove!({stated});` just before this, is what is needed"
        ));
        self.diagnostics.push(diagnostic);
    }

    /// What the arithmetic procedure found against the premise: values of
    /// the operands that satisfy every fact and violate it, when it has
    /// them.
    fn counterexample(&mut self, goal: &Term, known: &super::solve::Known) -> Option<String> {
        let (scratch, _) = self.arithmetic_context(known);
        let gave_up = arith::prove(&scratch, Some(self.prelude), goal, &Budget::default()).err()?;
        let Counterexample::Found(assignment) = gave_up.counterexample else {
            return None;
        };
        if assignment.is_empty() {
            return Some("it is false as it stands".into());
        }
        let is_literal_view = |atom: &Term| {
            matches!(atom, Term::Prim(Prim::View(_), operands)
                if matches!(operands.as_slice(), [literal] if literal.machine_value().is_some()))
        };
        let parts: Vec<String> = assignment
            .into_iter()
            .filter(|(atom, _)| !is_literal_view(atom))
            .map(|(atom, value)| format!("{} = {value}", self.show(&atom)))
            .collect();
        if parts.is_empty() {
            return None;
        }
        Some(format!("a counterexample: {}", parts.join(", ")))
    }
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
