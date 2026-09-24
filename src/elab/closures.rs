//! Logical closures bind kernel variables; no runtime closure object exists.
//! The typed IR already has LogicalApply. A closure literal is represented as
//! applying a checked zero-argument lambda returning its lambda value. Both
//! lambdas and the application are kernel-checked and erased. This avoids a
//! second trusted introduction form in typed lowering.
use super::env::{Elab, Env};
use super::exprs::Value;
use super::types::pairs;
use crate::ast;
use crate::kernel::{Mode, Term, Type, infer_term, same_type};
use crate::source::Span;
use crate::typed::Expr;

impl Env<'_> {
    pub(super) fn check_closure_capture(&mut self, slot: usize, span: Span) -> Elab<()> {
        if self.explicit_model_depth == 0
            && self
                .closure_capture_boundary
                .is_some_and(|boundary| slot < boundary)
        {
            let local = &self.names[slot];
            if !local.ghost
                && !self
                    .session
                    .program()
                    .definitions()
                    .is_erased_type(&local.ty)
                && !matches!(local.ty, Type::Fn(..))
                && !self.models.iter().any(|model| model.source == local.ty)
            {
                return self.fail("L0283", format!("a logical closure cannot capture runtime `{}` without an explicit model observation", local.name), span);
            }
        }
        Ok(())
    }

    pub(super) fn logical_closure(
        &mut self,
        parameters: &[ast::Parameter],
        body: &ast::Expr,
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        self.require_preview(
            crate::preview::Feature::LogicalData,
            "logical closure",
            span,
        )?;
        let mark = self.mark();
        let capture = self.closure_capture_boundary.replace(self.names.len());
        let model_depth = std::mem::replace(&mut self.explicit_model_depth, 0);
        let returns = self.returns.take();
        let loops = std::mem::take(&mut self.loops);
        let result = self.logical("a logical closure", |env| {
            for parameter in parameters {
                if !env.logical_spelling(&parameter.ty) {
                    return env.fail(
                        "L0283",
                        "a logical closure's parameters must have Logical types",
                        parameter.ty.span,
                    );
                }
            }
            let binders = env.telescope(
                parameters
                    .iter()
                    .map(|parameter| (Some(&parameter.name), &parameter.ty, parameter.span)),
                true,
            )?;
            let terms: Vec<_> = binders.iter().map(|binder| binder.term()).collect();
            let result_type = if let Some(Type::Fn(inputs, result)) = expected {
                if inputs.len() != binders.len() {
                    return env.fail(
                        "L0283",
                        "closure parameter count does not match the logical callable type",
                        span,
                    );
                }
                let mut telescope = inputs.clone();
                telescope.push((**result).clone());
                let telescope = Type::Tuple(telescope);
                for (index, binder) in binders.iter().enumerate() {
                    let expected =
                        crate::kernel::telescope_entry(&telescope, index, &terms[..index])
                            .expect("known parameter");
                    if !same_type(&binder.ty, &expected) {
                        return env.fail(
                            "L0283",
                            "closure parameter type does not match the logical callable type",
                            parameters[index].span,
                        );
                    }
                }
                Some(
                    crate::kernel::telescope_entry(&telescope, inputs.len(), &terms)
                        .expect("known result"),
                )
            } else {
                None
            };
            let value = match &result_type {
                Some(ty) => env.check(body, ty)?,
                None => env.infer(body)?,
            };
            if !env
                .session
                .program()
                .definitions()
                .is_erased_type(&value.ty)
                && !matches!(value.ty, Type::Fn(..))
                && !super::reconcile::is_logical_expr(&value.expr)
            {
                return env.fail(
                    "L0283",
                    "a logical closure must return a Logical value",
                    body.span,
                );
            }
            let term = env.term(&value, body.span)?;
            let lambda = Term::lambda_over(&pairs(&binders), &value.ty, term);
            Ok((lambda, Type::function_over(&pairs(&binders), &value.ty)))
        });
        self.close(mark);
        self.closure_capture_boundary = capture;
        self.explicit_model_depth = model_depth;
        self.returns = returns;
        self.loops = loops;
        let (lambda, ty) = result?;
        let checked = infer_term(&mut self.ctx, &lambda, Mode::Logical);
        self.kernel(checked, span)?;
        let literal = Term::lambda_over(&[], &ty, lambda);
        Ok(Value::new(
            Expr::LogicalApply {
                callee: literal,
                arguments: Vec::new(),
                ty: ty.clone(),
            },
            ty,
        ))
    }
}
