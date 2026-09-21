//! Formulas: the contents of `prop!(...)`, `prove!(...)`, and proof types,
//! as kernel terms of type `Prop`. Inside a formula a comparison is a claim
//! and `&&`, `||`, `!` and `=>` are connectives; nothing is executed.

use crate::ast::{self, BinaryOp, ExprKind, Form};
use crate::kernel::{Term, Type, VarId, same_type};
use crate::typed::Binder;

use super::env::{Elab, Env, Global};

impl Env<'_> {
    pub fn formula(&mut self, expr: &ast::Expr) -> Elab<Term> {
        let was_total = std::mem::replace(&mut self.total, true);
        let result = self.formula_inner(expr);
        self.total = was_total;
        result
    }

    fn formula_inner(&mut self, expr: &ast::Expr) -> Elab<Term> {
        match &expr.kind {
            ExprKind::Group(inner) => self.formula_inner(inner),
            // A `prop!` inside a formula is redundant and means the same.
            ExprKind::Form {
                form: Form::Prop,
                arguments,
                ..
            } if arguments.len() == 1 => self.formula_inner(&arguments[0]),
            ExprKind::Bool(true) => Ok(self.prelude.truth_prop()),
            ExprKind::Bool(false) => Ok(self.prelude.falsehood_prop()),
            ExprKind::Not(inner) => {
                let inner = self.formula_inner(inner)?;
                Ok(self.prelude.not_prop(inner))
            }
            ExprKind::Binary {
                operator,
                left,
                right,
                ..
            } => match operator {
                BinaryOp::And | BinaryOp::Or | BinaryOp::Implies => {
                    let left = self.formula_inner(left);
                    let right = self.formula_inner(right);
                    let (left, right) = (left?, right?);
                    Ok(match operator {
                        BinaryOp::And => self.prelude.and_prop(left, right),
                        BinaryOp::Or => self.prelude.or_prop(left, right),
                        _ => Term::implies(left, right),
                    })
                }
                comparison => self.comparison(*comparison, left, right),
            },
            ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } => {
                let universal = matches!(expr.kind, ExprKind::Forall { .. });
                let mark = self.mark();
                let quantified = self.quantified(parameters, body);
                self.close(mark);
                let (binders, mut term) = quantified?;
                for binder in binders.iter().rev() {
                    let body = term;
                    let bind = |x: Term| body.replace_var(binder.id, &x);
                    term = if universal {
                        Term::forall(binder.ty.clone(), bind)
                    } else {
                        Term::exists(binder.ty.clone(), bind)
                    };
                }
                Ok(term)
            }
            // A declared proposition applied to its arguments.
            ExprKind::Call { callee, arguments } if self.prop_named(callee).is_some() => {
                let info = self.prop_named(callee).unwrap();
                self.prop_application(&info, arguments, expr.span)
            }
            ExprKind::Name(_) if self.prop_named(expr).is_some() => {
                let info = self.prop_named(expr).unwrap();
                self.prop_application(&info, &[], expr.span)
            }
            _ => {
                let value = self.infer(expr)?;
                if same_type(&value.ty, &Type::Bool) {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0221",
                            "this is a `bool`, and a proposition is needed",
                            expr.span,
                        )
                        .note("a runtime boolean is not a claim; state one with a comparison, such as `... == true`"),
                    );
                    return Err(());
                }
                let value = self.coerce(value, &Type::Prop, expr.span)?;
                self.term(&value, expr.span)
            }
        }
    }

    fn quantified(
        &mut self,
        parameters: &[ast::Parameter],
        body: &ast::Block,
    ) -> Elab<(Vec<Binder>, Term)> {
        let mut binders = Vec::new();
        for parameter in parameters {
            let ty = self.ty(&parameter.ty)?;
            let binder = Binder {
                id: VarId::fresh(),
                name: parameter.name.text.clone(),
                ty,
            };
            // A quantified variable exists only in the logic.
            self.declare(&binder, true, parameter.span)?;
            binders.push(binder);
        }
        if !body.statements.is_empty() {
            return self.fail(
                "L0290",
                "statements inside a quantifier are not supported yet",
                body.statements[0].span,
            );
        }
        let Some(tail) = body.tail.as_deref() else {
            return self.fail("L0222", "a quantifier needs a formula to state", body.span);
        };
        Ok((binders, self.formula_inner(tail)?))
    }

    fn comparison(
        &mut self,
        operator: BinaryOp,
        left: &ast::Expr,
        right: &ast::Expr,
    ) -> Elab<Term> {
        let (left_value, right_value) = self.operands(left, right)?;
        let ty = left_value.ty.clone();
        let l = self.term(&left_value, left.span)?;
        let r = self.term(&right_value, right.span)?;
        let prelude = self.prelude;
        Ok(match operator {
            BinaryOp::Equal => Term::eq(ty, l, r),
            BinaryOp::NotEqual => prelude.not_prop(Term::eq(ty, l, r)),
            _ if !same_type(&ty, &Type::U8) => {
                let shown = self.show_type(&ty);
                return self.fail(
                    "L0223",
                    format!("ordering is defined on `u8`, and this is `{shown}`"),
                    left.span,
                );
            }
            BinaryOp::Less => prelude.u8_lt_prop(l, r),
            BinaryOp::LessEqual => prelude.u8_le_prop(l, r),
            BinaryOp::Greater => prelude.u8_lt_prop(r, l),
            _ => prelude.u8_le_prop(r, l),
        })
    }

    /// Both sides of a comparison, at one type. A literal takes its type
    /// from the other side.
    pub fn operands(
        &mut self,
        left: &ast::Expr,
        right: &ast::Expr,
    ) -> Elab<(super::exprs::Value, super::exprs::Value)> {
        let literal = |expr: &ast::Expr| matches!(expr.kind, ExprKind::Integer(_) | ExprKind::Hole);
        if literal(left) && !literal(right) {
            let right_value = self.infer(right)?;
            let left_value = self.check(left, &right_value.ty.clone())?;
            Ok((left_value, right_value))
        } else {
            let left_value = self.infer(left)?;
            let right_value = self.check(right, &left_value.ty.clone())?;
            Ok((left_value, right_value))
        }
    }

    fn prop_named(&self, callee: &ast::Expr) -> Option<std::rc::Rc<super::env::PropInfo>> {
        let ExprKind::Name(name) = &callee.kind else {
            return None;
        };
        if self.lookup(&name.text).is_some() {
            return None;
        }
        match self.globals.get(&name.text) {
            Some(Global::Prop(info)) => Some(std::rc::Rc::clone(info)),
            _ => None,
        }
    }
}
