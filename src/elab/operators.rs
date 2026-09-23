//! Operators outside a formula: `!`, `&&` and `||`, which are an `if`, and
//! the comparisons, between two values of one machine integer type, or two
//! `bool` for `==` and `!=`. Whether `p || q` is a proposition instead is
//! decided here too. The arithmetic operators are in `arithmetic`.

use crate::ast::{self, BinaryOp, ExprKind, Form};
use crate::diagnostic::Diagnostic;
use crate::kernel::{Type, same_type};
use crate::typed::{CompareOp, Expr};

use super::control::Branch;
use super::env::{Elab, Env, Global};
use super::exprs::Value;

impl Env<'_> {
    fn reads_as_logical(&self, expr: &ast::Expr) -> bool {
        if self.total {
            return true;
        }
        match &expr.kind {
            ExprKind::Logic(_) => true,
            ExprKind::Group(inner) | ExprKind::Not(inner) => self.reads_as_logical(inner),
            ExprKind::Name(name) => self
                .lookup(&name.text)
                .is_some_and(|local| local.ghost || local.ty.is_ghost()),
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Name(name) => {
                    matches!(self.values.get(&name.text), Some(Global::Fn(info)) if info.result_logical)
                }
                _ => false,
            },
            ExprKind::Cast { ty, .. } => self.logical_spelling(ty),
            ExprKind::Binary { left, right, .. } => {
                self.reads_as_logical(left) || self.reads_as_logical(right)
            }
            _ => false,
        }
    }

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
                None => matches!(self.types.get(&name.text), Some(Global::Prop(_))),
            },
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                    match self.values.get(&name.text) {
                        Some(Global::Fn(info)) => same_type(&info.result, &Type::Prop),
                        _ => matches!(self.types.get(&name.text), Some(Global::Prop(_))),
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
        {
            if self.reads_as_logical(inner) {
                let value = self.check(inner, &Type::Bool)?;
                return Ok(Value::new(
                    Expr::Ghost(Box::new(Expr::Compare {
                        op: CompareOp::Eq,
                        ty: Type::Bool,
                        left: Box::new(value.expr),
                        right: Box::new(Expr::Bool(false)),
                    })),
                    Type::Bool,
                ));
            }
        }
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
        if self.reads_as_logical(left) || self.reads_as_logical(right) {
            let left = self.check(left, &Type::Bool)?;
            let right = self.check(right, &Type::Bool)?;
            // Logical boolean operations are eager. Put both operands in
            // source order before the erased case, so runtime operands do
            // not become conditional on an erased value.
            let mut statements = Vec::new();
            let mut values = Vec::new();
            for (name, value) in [("logical_left", left), ("logical_right", right)] {
                let mut binder = crate::typed::Binder::new(name, Type::Bool);
                binder.ghost = true;
                self.session
                    .register_binding_layout(binder.id, crate::typed::ErasureLayout::Logical);
                let equation = crate::kernel::HypId::fresh();
                let term = self.term(&value, expr.span)?;
                self.define(binder.id, equation, &term, true, expr.span)?;
                self.facts.push(super::env::Fact::definition(
                    crate::kernel::Proof::hyp(equation),
                    crate::kernel::Term::eq(Type::Bool, crate::kernel::Term::var(binder.id), term),
                ));
                values.push(Expr::Var {
                    id: binder.id,
                    name: name.into(),
                    ty: Type::Bool,
                });
                statements.push(crate::typed::Stmt::Let {
                    pattern: crate::typed::Pattern::Bind {
                        binder,
                        equation,
                        mutable: false,
                    },
                    value: super::calls::ghost_value(value.expr),
                });
            }
            let right = crate::typed::Block {
                stmts: Vec::new(),
                tail: Some(Box::new(values.pop().unwrap())),
            };
            let constant = crate::typed::Block {
                stmts: Vec::new(),
                tail: Some(Box::new(Expr::Bool(*operator == BinaryOp::Or))),
            };
            let (then_block, else_block) = if *operator == BinaryOp::And {
                (right, constant)
            } else {
                (constant, right)
            };
            let result = crate::kernel::VarId::fresh();
            self.declare_result(result, &Type::Bool, expr.span)?;
            let operation = Expr::Ghost(Box::new(Expr::If {
                condition: Box::new(values.pop().unwrap()),
                then_fact: crate::kernel::HypId::fresh(),
                else_fact: crate::kernel::HypId::fresh(),
                then_block,
                else_block,
                ty: Type::Bool,
                result,
                joined: None,
            }));
            return Ok(Value::new(
                Expr::Block(crate::typed::Block {
                    stmts: statements,
                    tail: Some(Box::new(operation)),
                }),
                Type::Bool,
            ));
        }
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
        if ty.as_machine().is_none()
            && !(equality && same_type(&ty, &Type::Bool))
            && !(same_type(&ty, &Type::Int))
        {
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
        let logical = same_type(&ty, &Type::Int)
            || super::reconcile::is_logical_expr(&left_value.expr)
            || super::reconcile::is_logical_expr(&right_value.expr);
        let value = Expr::Compare {
            op,
            ty,
            left: Box::new(left_value.expr),
            right: Box::new(right_value.expr),
        };
        Ok(Value::new(
            if logical {
                Expr::Ghost(Box::new(value))
            } else {
                value
            },
            Type::Bool,
        ))
    }

    /// The operators that parse and have no meaning yet: the shifts and
    /// the bitwise operators, which come after the core.
    #[inline(never)]
    pub(super) fn operator_not_yet<T>(&mut self, expr: &ast::Expr) -> Elab<T> {
        let ExprKind::Binary {
            operator,
            operator_span,
            ..
        } = &expr.kind
        else {
            unreachable!("only an operator without a meaning is reported here")
        };
        self.diagnostics.push(
            Diagnostic::error(
                "L0290",
                format!("the `{}` operator is not in Locus yet", operator.spelling()),
                *operator_span,
            )
            .note("bit operators and shifts come after the core"),
        );
        Err(())
    }
}
