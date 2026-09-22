//! Loops: `loop` with its state and result, the bounded `for`, and the
//! `break` and `continue` that leave or advance them.

use crate::ast;
use crate::kernel::{HypId, MachineInt, Proof, Term, Type, VarId, check_proof};
use crate::source::Span;
use crate::typed::{self, Binder, Expr, is_pure};

use super::env::{Elab, Env, LoopTarget};
use super::exprs::{Value, unit_type};
use super::items::{FoundProof, HoleReport};
use super::literals::untyped_literal;
use super::types::tuple_over;

impl Env<'_> {
    /// The state binders of a loop, whose types may mention `index` and the
    /// state before them, and the initial values, which may not.
    fn loop_state(
        &mut self,
        state: &[ast::StateParameter],
        index: Option<(&Binder, &Term)>,
    ) -> Elab<Vec<(Binder, Expr)>> {
        let mark = self.mark();
        let binders = (|| {
            if let Some((index, _)) = index {
                self.declare(
                    index,
                    false,
                    state
                        .first()
                        .map_or(Span::new(self.source.id, 0, 0), |s| s.span),
                )?;
            }
            self.telescope(
                state
                    .iter()
                    .map(|parameter| (Some(&parameter.name), &parameter.ty, parameter.span)),
            )
        })();
        self.close(mark);
        let binders = binders?;

        let mut tys: Vec<Type> = binders
            .iter()
            .map(|binder| match index {
                Some((index, start)) => binder.ty.replace_var(index.id, start),
                None => binder.ty.clone(),
            })
            .collect();
        let mut result = Vec::new();
        for (position, parameter) in state.iter().enumerate() {
            let value = self.check(&parameter.initial, &tys[position].clone())?;
            let term = self.term(&value, parameter.initial.span)?;
            for later in tys[position + 1..].iter_mut() {
                *later = later.replace_var(binders[position].id, &term);
            }
            result.push((binders[position].clone(), value.expr));
        }
        Ok(result)
    }

    fn loop_body(&mut self, body: &ast::Block, what: &str) -> Elab<typed::Block> {
        let (block, _, never) = self.block(body, None)?;
        if !never {
            let message = format!("every path through {what} must end in `continue(...)`");
            let message = if what.contains("loop") {
                message.replace("`continue(...)`", "`continue(...)` or `break`")
            } else {
                message
            };
            return self.fail("L0216", message, body.span);
        }
        Ok(block)
    }

    pub(super) fn loop_(
        &mut self,
        state: &[ast::StateParameter],
        result: &ast::Type,
        body: &ast::Block,
        span: Span,
    ) -> Elab<Value> {
        if self.total {
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error("L0215", "`loop` cannot appear here", span)
                    .note("a `loop` may run forever, and a `math fn` must return; a bounded `for` always does"),
            );
            return Err(());
        }
        let state = self.loop_state(state, None)?;
        let result_ty = self.ty(result)?;
        let binders: Vec<Binder> = state.iter().map(|(binder, _)| binder.clone()).collect();

        let mark = self.mark();
        let body_result = (|| {
            for binder in &binders {
                self.declare(binder, false, span)?;
            }
            self.loops.push(LoopTarget {
                state: binders.clone(),
                advance: None,
                result: Some(result_ty.clone()),
            });
            let block = self.loop_body(body, "a loop");
            self.loops.pop();
            block
        })();
        self.close(mark);
        let body = body_result?;

        let result = VarId::fresh();
        self.declare_result(result, &result_ty, span)?;
        Ok(Value::new(
            Expr::Loop {
                state,
                result_ty: result_ty.clone(),
                body,
                result,
            },
            result_ty,
        ))
    }

    pub(super) fn for_(
        &mut self,
        index: &ast::Name,
        lower: &ast::Expr,
        upper: &ast::Expr,
        state: &[ast::StateParameter],
        body: &ast::Block,
        span: Span,
    ) -> Elab<Value> {
        // The bounds have one machine type; a literal takes the other's.
        let (lo, hi) = if untyped_literal(lower) && !untyped_literal(upper) {
            let hi = self.infer(upper)?;
            let lo = self.check(lower, &hi.ty.clone())?;
            (lo, hi)
        } else {
            let lo = self.infer(lower)?;
            let hi = self.check(upper, &lo.ty.clone())?;
            (lo, hi)
        };
        let Some(ty) = lo.ty.as_machine() else {
            let shown = self.show_type(&lo.ty);
            return self.fail(
                "L0220",
                format!("the bounds of a `for` are machine integers, and this is `{shown}`"),
                lower.span,
            );
        };
        let lo_term = self.term(&lo, lower.span)?;
        let hi_term = self.term(&hi, upper.span)?;
        let view = |x: &Term| Term::view(ty, x.clone());
        let ordered =
            self.range_evidence(ty, &lo_term, &hi_term, lower.span.through(upper.span))?;

        let index = Binder {
            id: VarId::fresh(),
            name: index.text.clone(),
            ty: Type::machine(ty),
        };
        let state = self.loop_state(state, Some((&index, &lo_term)))?;
        let binders: Vec<Binder> = state.iter().map(|(binder, _)| binder.clone()).collect();
        let (lower_fact, upper_fact) = (HypId::fresh(), HypId::fresh());

        let mark = self.mark();
        let body_result = (|| {
            self.declare(&index, false, span)?;
            for binder in &binders {
                self.declare(binder, false, span)?;
            }
            self.assume(
                lower_fact,
                Term::int_le(view(&lo_term), view(&index.term())),
                span,
            )?;
            self.assume(
                upper_fact,
                Term::int_lt(view(&index.term()), view(&hi_term)),
                span,
            )?;
            self.loops.push(LoopTarget {
                state: binders.clone(),
                advance: Some((index.id, Term::successor(ty, index.term()))),
                result: None,
            });
            let block = self.loop_body(body, "the body of a `for`");
            self.loops.pop();
            block
        })();
        self.close(mark);
        let body = body_result?;

        // The final state: the telescope at the upper bound.
        let ty = tuple_over(&binders).replace_var(index.id, &hi_term);
        let result = VarId::fresh();
        let expr = Expr::For {
            index,
            lower: lower_fact,
            upper: upper_fact,
            lo: Box::new(lo.expr),
            hi: Box::new(hi.expr),
            ordered,
            state,
            body,
            result,
        };
        if !is_pure(&expr) {
            self.declare_result(result, &ty, span)?;
        }
        Ok(Value::new(expr, ty))
    }

    /// Evidence that the range `lo..hi` is ordered, `lo <= hi` over the
    /// views. A range over an unsigned type that starts at `0` is ordered by
    /// the lemma `<T>_zero_le`, which the elaborator applies here because
    /// the range has no place to write it; any other range needs the fact
    /// in scope, as a hole does.
    fn range_evidence(&mut self, ty: MachineInt, lo: &Term, hi: &Term, span: Span) -> Elab<Proof> {
        let view = |x: &Term| Term::view(ty, x.clone());
        let claim = Term::int_le(view(lo), view(hi));
        let zero_le = self
            .theory
            .machine(ty)
            .unsigned
            .map(|lemmas| lemmas.zero_le);
        let (Some(zero_le), true) = (zero_le, *lo == Term::machine_int(ty, 0)) else {
            return self.solve(&claim, span, None);
        };
        let started = std::time::Instant::now();
        let proof = Proof::OfTerm(Term::call(Term::Fn(zero_le), vec![hi.clone()]));
        let checked = check_proof(&mut self.ctx, &proof, &claim);
        self.kernel(checked, span)?;
        let tier = self
            .theory
            .lemma_names()
            .into_iter()
            .find(|(_, id)| *id == zero_le)
            .map_or("zero_le", |(name, _)| name);
        self.holes.push(HoleReport {
            span,
            solved: true,
            tier,
            proof_size: super::solve::proof_size(&proof),
            micros: started.elapsed().as_micros(),
            found: Some(FoundProof {
                context: self.ctx.clone(),
                claim,
                proof: proof.clone(),
            }),
        });
        Ok(proof)
    }

    pub(super) fn break_(&mut self, expr: &ast::Expr, value: &ast::Expr) -> Elab<Value> {
        let Some(target) = self.loops.last().cloned() else {
            return self.fail("L0217", "`break` outside a loop", expr.span);
        };
        let Some(result) = target.result else {
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0218",
                    "a bounded `for` runs to the end of its range and has no `break`",
                    expr.span,
                )
                .note("carry a `bool` in the state to stop doing work early"),
            );
            return Err(());
        };
        let value = self.check(value, &result)?;
        Ok(Value {
            expr: Expr::Break(Box::new(value.expr)),
            ty: unit_type(),
            never: true,
        })
    }

    pub(super) fn continue_(&mut self, expr: &ast::Expr, arguments: &[ast::Expr]) -> Elab<Value> {
        let Some(target) = self.loops.last().cloned() else {
            return self.fail("L0217", "`continue` outside a loop", expr.span);
        };
        let mut tys: Vec<Type> = target
            .state
            .iter()
            .map(|binder| match &target.advance {
                Some((index, next)) => binder.ty.replace_var(*index, next),
                None => binder.ty.clone(),
            })
            .collect();
        let ids: Vec<VarId> = target.state.iter().map(|binder| binder.id).collect();
        let next = self.arguments(arguments, &ids, &mut tys, "the loop's state", expr.span)?;
        Ok(Value {
            expr: Expr::Continue(next),
            ty: unit_type(),
            never: true,
        })
    }
}
