//! Physical boxes preserve allocation even when their payload erases.
use super::{
    env::{Elab, Env, Fact},
    exprs::Value,
};
use crate::{
    ast,
    kernel::{HypId, Proof, Term, Type, VarId},
    source::Span,
    typed::{ErasureLayout, Expr},
};
impl Env<'_> {
    pub(super) fn box_constructor(
        &mut self,
        callee: &ast::Expr,
        args: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Option<Elab<Value>> {
        let ast::ExprKind::Path(path) = &callee.kind else {
            return None;
        };
        let (owner, method) = path.pair()?;
        if owner.text != "Box" || method.text != "new" {
            return None;
        }
        Some((|| {
            self.require_preview(crate::preview::Feature::HeapViews, "Box allocation", span)?;
            if self.total {
                return self.fail(
                    "L0284",
                    "physical Box allocation is not allowed in logical computation",
                    span,
                );
            }
            let [arg] = args else {
                return self.fail("L0208", "Box::new takes exactly one argument", span);
            };
            let expected = match expected {
                Some(Type::Boxed(t)) => Some(&**t),
                _ => None,
            };
            let expected_logical =
                if let Some(ErasureLayout::Boxed(inner)) = self.layout_hints.get(&span).cloned() {
                    self.expect_layout(arg, &inner);
                    inner.is_logical()
                } else {
                    false
                };
            let value = self.runtime_arguments(|env| match expected {
                Some(t) => env.argument(
                    arg,
                    t,
                    expected_logical || env.session.program().definitions().is_erased_type(t),
                ),
                None => env.infer(arg),
            })?;
            let logical_payload = self.session.expression_layout(&value.expr).is_logical();
            let term = self.term(&value, arg.span)?;
            let ty = Type::Boxed(Box::new(value.ty.clone()));
            let result = VarId::fresh();
            let equation = HypId::fresh();
            let model = Term::Boxed(Box::new(term));
            self.declare_result(result, &ty, span)?;
            let claim = Term::eq(ty.clone(), Term::Free(result), model);
            self.assume(equation, claim.clone(), span)?;
            self.facts
                .push(Fact::definition(Proof::hyp(equation), claim));
            Ok(Value::new(
                Expr::BoxNew {
                    value: Box::new(value.expr),
                    result,
                    equation,
                    logical_payload,
                },
                ty,
            ))
        })())
    }
    pub(super) fn boxed_deref(&mut self, value: Value, span: Span) -> Elab<Value> {
        let Type::Boxed(inner) = value.ty else {
            return self.fail("L0266", "expected a Box", span);
        };
        Ok(Value::new(
            Expr::BoxDeref {
                value: Box::new(value.expr),
                ty: (*inner).clone(),
            },
            *inner,
        ))
    }
}
