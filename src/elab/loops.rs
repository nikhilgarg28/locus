//! Loops: `loop` with its state and result, the bounded `for`, and the
//! `break` and `continue` that leave or advance them.

use crate::ast;
use crate::kernel::{HypId, Proof, Term, Type, VarId, check_proof};
use crate::source::Span;
use crate::typed::{self, Binder, Expr, is_pure};

use super::env::{Elab, Env, LoopTarget};
use super::exprs::{Value, unit_type};
use super::items::{FoundProof, HoleReport};
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
        let lo = self.check(lower, &Type::U8)?;
        let hi = self.check(upper, &Type::U8)?;
        let lo_term = self.term(&lo, lower.span)?;
        let hi_term = self.term(&hi, upper.span)?;
        let prelude = self.prelude;
        let ordered = self.range_evidence(&lo_term, &hi_term, lower.span.through(upper.span))?;

        let index = Binder {
            id: VarId::fresh(),
            name: index.text.clone(),
            ty: Type::U8,
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
                prelude.u8_le_prop(lo_term.clone(), index.term()),
                span,
            )?;
            self.assume(
                upper_fact,
                prelude.u8_lt_prop(index.term(), hi_term.clone()),
                span,
            )?;
            self.loops.push(LoopTarget {
                state: binders.clone(),
                advance: Some((index.id, Term::wrapping_add(index.term(), Term::U8(1)))),
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

    /// Evidence that the range `lo..hi` is ordered, `lo <= hi`. A range
    /// that starts at `0` is ordered by the lemma `u8_zero_le`, which the
    /// elaborator applies here because the range has no place to write it;
    /// any other range needs the fact in scope, as a hole does.
    fn range_evidence(&mut self, lo: &Term, hi: &Term, span: Span) -> Elab<Proof> {
        let claim = self.prelude.u8_le_prop(lo.clone(), hi.clone());
        if *lo != Term::U8(0) {
            return self.solve(&claim, span, None);
        }
        let started = std::time::Instant::now();
        let lemma = self.theory.u8_zero_le;
        let proof = Proof::OfTerm(Term::call(Term::Fn(lemma), vec![hi.clone()]));
        let checked = check_proof(&mut self.ctx, &proof, &claim);
        self.kernel(checked, span)?;
        self.holes.push(HoleReport {
            span,
            solved: true,
            tier: "u8_zero_le",
            proof_size: 3,
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
