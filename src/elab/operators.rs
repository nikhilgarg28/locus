//! Operators outside a formula: `!`, `&&` and `||`, which are an `if`, and
//! the comparisons of `u8`. Whether `p || q` is a proposition instead is
//! decided here too.

use crate::ast::{self, BinaryOp, ExprKind, Form};
use crate::kernel::{Type, same_type};
use crate::typed::{CompareOp, Expr};

use super::control::Branch;
use super::env::{Elab, Env, Global};
use super::exprs::Value;

impl Env<'_> {
    /// Whether an expression written outside a formula is a proposition, as
    /// `p || q` is when `p` is a `Prop`.
    pub(super) fn reads_as_prop(&self, expr: &ast::Expr) -> bool {
        match &expr.kind {
            ExprKind::Group(inner) | ExprKind::Not(inner) => self.reads_as_prop(inner),
            ExprKind::Form {
                form: Form::Prop, ..
            }
            | ExprKind::Forall { .. }
            | ExprKind::Exists { .. } => true,
            ExprKind::Binary {
                operator,
                left,
                right,
                ..
            } => match operator {
                BinaryOp::Implies => true,
                BinaryOp::And | BinaryOp::Or => {
                    self.reads_as_prop(left) || self.reads_as_prop(right)
                }
                _ => false,
            },
            ExprKind::Name(name) => match self.lookup(&name.text) {
                Some(local) => same_type(&local.ty, &Type::Prop),
                None => matches!(self.globals.get(&name.text), Some(Global::Prop(_))),
            },
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                    match self.globals.get(&name.text) {
                        Some(Global::Fn(info)) => same_type(&info.result, &Type::Prop),
                        Some(Global::Prop(_)) => true,
                        _ => false,
                    }
                }
                _ => false,
            },
            _ => false,
        }
    }

    pub(super) fn not(
        &mut self,
        expr: &ast::Expr,
        inner: &ast::Expr,
        expected: Option<&Type>,
    ) -> Elab<Value> {
        let otherwise = ast::Expr {
            kind: ExprKind::Bool(true),
            span: expr.span,
        };
        let then = ast::Expr {
            kind: ExprKind::Bool(false),
            span: expr.span,
        };
        self.conditional(
            inner,
            Branch::Expr(&then),
            Branch::Expr(&otherwise),
            expected,
            expr.span,
        )
    }

    pub(super) fn short_circuit(
        &mut self,
        expr: &ast::Expr,
        operator: &BinaryOp,
        left: &ast::Expr,
        right: &ast::Expr,
    ) -> Elab<Value> {
        // Short-circuit evaluation is an `if`.
        let constant = ast::Expr {
            kind: ExprKind::Bool(*operator == BinaryOp::Or),
            span: expr.span,
        };
        let (then, otherwise) = if *operator == BinaryOp::And {
            (Branch::Expr(right), Branch::Expr(&constant))
        } else {
            (Branch::Expr(&constant), Branch::Expr(right))
        };
        self.conditional(left, then, otherwise, Some(&Type::Bool), expr.span)
    }

    pub(super) fn compare(
        &mut self,
        expr: &ast::Expr,
        operator: &BinaryOp,
        left: &ast::Expr,
        right: &ast::Expr,
        expected: Option<&Type>,
    ) -> Elab<Value> {
        if expected.is_some_and(|ty| same_type(ty, &Type::Prop)) {
            let term = self.formula(expr)?;
            return Ok(Value::new(Expr::Prop(term), Type::Prop));
        }
        let (left_value, right_value) = self.operands(left, right)?;
        if !same_type(&left_value.ty, &Type::U8) {
            let shown = self.show_type(&left_value.ty);
            self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0211",
                            format!("values of type `{shown}` cannot be compared at runtime in the core"),
                            expr.span,
                        )
                        .note("runtime comparison is defined on `u8`; inside a formula, `==` states equality at any type"),
                    );
            return Err(());
        }
        let op = match operator {
            BinaryOp::Equal => CompareOp::Eq,
            BinaryOp::NotEqual => CompareOp::Ne,
            BinaryOp::Less => CompareOp::Lt,
            BinaryOp::LessEqual => CompareOp::Le,
            BinaryOp::Greater => CompareOp::Gt,
            _ => CompareOp::Ge,
        };
        Ok(Value::new(
            Expr::Compare {
                op,
                left: Box::new(left_value.expr),
                right: Box::new(right_value.expr),
            },
            Type::Bool,
        ))
    }
}
