//! Operators outside a formula: `!`, `&&` and `||`, which are an `if`, and
//! the comparisons, between two values of one machine integer type, or two
//! `bool` for `==` and `!=`. Whether `p || q` is a proposition instead is
//! decided here too.

use crate::ast::{self, BinaryOp, ExprKind, Form};
use crate::diagnostic::Diagnostic;
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
        if !operator.is_comparison() {
            return self.operator_not_yet(expr);
        }
        if expected.is_some_and(|ty| same_type(ty, &Type::Prop)) {
            let term = self.formula(expr)?;
            return Ok(Value::new(Expr::Prop(term), Type::Prop));
        }
        let (left_value, right_value) = self.operands(left, right, false)?;
        let op = match operator {
            BinaryOp::Equal => CompareOp::Eq,
            BinaryOp::NotEqual => CompareOp::Ne,
            BinaryOp::Less => CompareOp::Lt,
            BinaryOp::LessEqual => CompareOp::Le,
            BinaryOp::Greater => CompareOp::Gt,
            _ => CompareOp::Ge,
        };
        let ty = left_value.ty.clone();
        let equality = matches!(op, CompareOp::Eq | CompareOp::Ne);
        if ty.as_machine().is_none() && !(equality && same_type(&ty, &Type::Bool)) {
            let shown = self.show_type(&ty);
            let (message, note) = if same_type(&ty, &Type::Bool) {
                (
                    "`bool` has no ordering".to_string(),
                    "`==` and `!=` compare two `bool`; the orderings compare two machine integers",
                )
            } else {
                (
                    format!("values of type `{shown}` cannot be compared at runtime in the core"),
                    "runtime comparison is defined on the machine integer types, and `==` and `!=` on `bool`; inside a formula, `==` states equality at any type",
                )
            };
            self.diagnostics
                .push(Diagnostic::error("L0211", message, expr.span).note(note));
            return Err(());
        }
        Ok(Value::new(
            Expr::Compare {
                op,
                ty,
                left: Box::new(left_value.expr),
                right: Box::new(right_value.expr),
            },
            Type::Bool,
        ))
    }

    /// The operators that parse and have no meaning yet: the arithmetic and
    /// bit operators, and unary minus on anything but a literal. Each names
    /// the commit that gives it one.
    #[inline(never)]
    pub(super) fn operator_not_yet<T>(&mut self, expr: &ast::Expr) -> Elab<T> {
        let (what, span, note) = match &expr.kind {
            ExprKind::Binary {
                operator,
                operator_span,
                ..
            } if operator.is_bitwise() => (
                format!("the `{}` operator", operator.spelling()),
                *operator_span,
                "bit operators and shifts come after the core",
            ),
            ExprKind::Binary {
                operator,
                operator_span,
                ..
            } => (
                format!("the `{}` operator", operator.spelling()),
                *operator_span,
                "it arrives with E6 (LOC-172): the operators `+ - * / %` with their panic conditions",
            ),
            ExprKind::Unary {
                operator,
                operator_span,
                ..
            } => (
                format!("unary `{}`", operator.spelling()),
                *operator_span,
                "it arrives with E6 (LOC-172): the operators `+ - * / %` with their panic conditions",
            ),
            _ => unreachable!("only an operator without a meaning is reported here"),
        };
        let mut diagnostic =
            Diagnostic::error("L0290", format!("{what} is not in Locus yet"), span).note(note);
        if let ExprKind::Binary {
            operator: operator @ (BinaryOp::Add | BinaryOp::Sub),
            ..
        } = &expr.kind
        {
            let method = if *operator == BinaryOp::Add {
                "wrapping_add"
            } else {
                "wrapping_sub"
            };
            diagnostic = diagnostic.note(format!(
                "until then, machine arithmetic says what happens on overflow: write `a.{method}(b)`"
            ));
        }
        self.diagnostics.push(diagnostic);
        Err(())
    }
}
