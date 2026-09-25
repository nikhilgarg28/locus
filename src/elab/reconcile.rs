//! Logical surface classification and model observations. Kernel types retain
//! one boolean type; the checked surface mode distinguishes Bool from bool.
use super::env::{Elab, Env, Global};
use super::exprs::Value;
use crate::ast;
use crate::diagnostic::{Applicability, Diagnostic, Suggestion};
use crate::kernel::Type;
use crate::preview::Feature;
use crate::source::Span;
use crate::typed::Expr;

pub(super) fn is_logical_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Ghost(_)
        | Expr::Prop(_)
        | Expr::Proof(_)
        | Expr::Int(_)
        | Expr::LogicalApply { .. } => true,
        Expr::Block(block) => block.tail.as_deref().is_some_and(is_logical_expr),
        Expr::If {
            then_block,
            else_block,
            ..
        } => [then_block, else_block]
            .iter()
            .any(|block| block.tail.as_deref().is_some_and(is_logical_expr)),
        Expr::Match { arms, .. } => arms
            .iter()
            .any(|arm| arm.body.tail.as_deref().is_some_and(is_logical_expr)),
        Expr::Var { ty, .. } | Expr::CallFn { ty, .. } | Expr::CallMath { ty, .. } => ty.is_ghost(),
        _ => false,
    }
}

impl Env<'_> {
    pub(super) fn logical_spelling(&self, ty: &ast::Type) -> bool {
        match &ty.kind {
            ast::TypeKind::Scoped { name, .. } | ast::TypeKind::Named(name) => {
                matches!(name.text.as_str(), "Bool" | "Int" | "Nat" | "Prop")
                    || match self.types.get(&self.type_text(name)) {
                        Some(Global::Struct(info)) => self
                            .session
                            .program()
                            .definitions()
                            .is_erased_type(&Type::Struct(info.id)),
                        Some(Global::Enum(info)) => self
                            .session
                            .program()
                            .definitions()
                            .is_erased_type(&Type::Enum(info.id)),
                        _ => false,
                    }
            }
            ast::TypeKind::Proof(_) | ast::TypeKind::LogicalFunction { .. } => true,
            ast::TypeKind::Group(inner) => self.logical_spelling(inner),
            _ => false,
        }
    }

    pub(super) fn logical_value(&mut self, value: Value, span: Span) -> Elab<Value> {
        self.default_model(value, span)
    }

    pub(super) fn logic_block(
        &mut self,
        block: &ast::Block,
        expected: Option<&Type>,
    ) -> Elab<Value> {
        self.require_preview(Feature::LogicalSplit, "logic block", block.span)?;
        let mark = self.mark();
        let result_scope = self.result_scope();
        let result = self.logical("a logic block", |env| env.block(block, expected));
        let result = self.check_scope_result(&result_scope, result, block.span);
        self.close_names(mark);
        let (body, ty, _) = result?;
        if !self.session.program().definitions().is_erased_type(&ty)
            && !matches!(ty, Type::Bool | Type::Fn(..))
        {
            return self.fail(
                "L0270",
                "a logic block must produce a Logical value",
                block.span,
            );
        }
        let expr = Expr::Block(body);
        Ok(Value::new(
            if ty.is_ghost() {
                expr
            } else {
                Expr::Ghost(Box::new(expr))
            },
            ty,
        ))
    }

    pub(super) fn migrate_logical_type<T>(
        &mut self,
        span: Span,
        replacement: Option<String>,
    ) -> Elab<T> {
        let mut diagnostic = Diagnostic::error("L0271", "Ghost<T> has been replaced by a logical model type", span)
            .note("use Int for a machine integer, Bool for bool, or a user-defined logical model; observe data with x as ModelType");
        if let Some(replacement) = replacement {
            diagnostic = diagnostic.suggest(Suggestion {
                message: format!("replace with {replacement}"),
                span,
                replacement,
                applicability: Applicability::MachineApplicable,
            });
        }
        self.diagnostics.push(diagnostic);
        Err(())
    }
}
