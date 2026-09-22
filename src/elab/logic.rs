//! Formulas: the contents of `prop!(...)`, `prove!(...)`, and proof types,
//! as kernel terms of type `Prop`. Inside a formula a comparison is a claim
//! and `&&`, `||`, `!` and `=>` are connectives; nothing is executed.
//!
//! A comparison is between two values of one type, in a formula as in code.
//! `==` and `!=` are equality at that type. An ordering between two machine
//! integers is the order of their views into `Int`, `int_le(view[T](a),
//! view[T](b))`, with `<` as `int_lt`, and `>` and `>=` as the same two with
//! their sides exchanged; between two `Int` it is `int_le` itself. That is
//! the one bridge from a machine value to the logic, and it is fixed here,
//! not searched for.

use crate::ast::{self, BinaryOp, ExprKind, Form};
use crate::diagnostic::Diagnostic;
use crate::kernel::{Term, Type, VarId, same_type};
use crate::source::Span;
use crate::typed::Binder;

use super::env::{Elab, Env, Global};
use super::exprs::Value;
use super::literals::untyped_literal;

impl Env<'_> {
    pub fn formula(&mut self, expr: &ast::Expr) -> Elab<Term> {
        let was_total = std::mem::replace(&mut self.total, true);
        let was_formula = self.formula.replace("a proposition");
        let result = self.formula_inner(expr);
        self.total = was_total;
        self.formula = was_formula;
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
                comparison if comparison.is_comparison() => {
                    self.comparison(*comparison, left, right)
                }
                // Arithmetic is a value, of `Int` in a formula, and a value
                // is not a proposition; the general arm says so, or the
                // operator says why it cannot stand here at all.
                arithmetic if arithmetic.is_arithmetic() => {
                    let value = self.infer(expr)?;
                    let value = self.coerce(value, &Type::Prop, expr.span)?;
                    self.term(&value, expr.span)
                }
                _ => self.operator_not_yet(expr),
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
            // A quantified variable exists only in the logic, where
            // `Ghost<T>` is `T`.
            let written = self.written(&parameter.ty)?;
            let binder = Binder {
                id: VarId::fresh(),
                name: parameter.name.text.clone(),
                ty: written.ty,
                ghost: written.ghost,
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
        let (left_value, right_value) = self.operands(left, right, true)?;
        let ty = left_value.ty.clone();
        let l = self.term(&left_value, left.span)?;
        let r = self.term(&right_value, right.span)?;
        let prelude = self.prelude;
        // An ordering compares two `Int`: the values themselves, or the
        // views of two machine integers.
        let (l, r) = match (&operator, ty.as_machine()) {
            (BinaryOp::Equal, _) => return Ok(Term::eq(ty, l, r)),
            (BinaryOp::NotEqual, _) => return Ok(prelude.not_prop(Term::eq(ty, l, r))),
            (_, Some(machine)) => (Term::view(machine, l), Term::view(machine, r)),
            (_, None) if same_type(&ty, &Type::Int) => (l, r),
            (_, None) => {
                let shown = self.show_type(&ty);
                return self.fail(
                    "L0223",
                    format!(
                        "ordering is defined on the integer types, `u8` to `i64` and `Int`, and this is `{shown}`"
                    ),
                    left.span,
                );
            }
        };
        Ok(match operator {
            BinaryOp::Less => Term::int_lt(l, r),
            BinaryOp::LessEqual => Term::int_le(l, r),
            BinaryOp::Greater => Term::int_lt(r, l),
            _ => Term::int_le(r, l),
        })
    }

    /// Both sides of a comparison, at one type. A literal without a suffix
    /// takes its type from the other side; otherwise each side is typed on
    /// its own, and two types are an error that says which cast would make
    /// them one. `formula` says where the comparison stands, which changes
    /// what the error suggests.
    pub fn operands(
        &mut self,
        left: &ast::Expr,
        right: &ast::Expr,
        formula: bool,
    ) -> Elab<(Value, Value)> {
        self.operands_of("a comparison", left, right, formula)
    }

    /// `operands` for any operator between two values of one type; `what`
    /// names it in the error.
    pub fn operands_of(
        &mut self,
        what: &str,
        left: &ast::Expr,
        right: &ast::Expr,
        formula: bool,
    ) -> Elab<(Value, Value)> {
        let (left_value, right_value) = match (untyped_literal(left), untyped_literal(right)) {
            (true, false) => {
                let right_value = self.infer(right)?;
                let left_value = self.check(left, &right_value.ty.clone())?;
                (left_value, right_value)
            }
            (false, true) => {
                let left_value = self.infer(left)?;
                let right_value = self.check(right, &left_value.ty.clone())?;
                (left_value, right_value)
            }
            _ => {
                let left_value = self.infer(left);
                let right_value = self.infer(right);
                (left_value?, right_value?)
            }
        };
        if same_type(&left_value.ty, &right_value.ty) {
            return Ok((left_value, right_value));
        }
        self.two_types(
            what,
            (left, &left_value.ty),
            (right, &right_value.ty),
            formula,
            left.span.through(right.span),
        )
    }

    /// The error for a comparison of two types, with the cast that would
    /// make them one: at a machine type in code, and into `Int` in a
    /// formula, where the cast is exact.
    fn two_types<T>(
        &mut self,
        what: &str,
        left: (&ast::Expr, &Type),
        right: (&ast::Expr, &Type),
        formula: bool,
        span: Span,
    ) -> Elab<T> {
        let (left_text, right_text) = (
            self.text(left.0.span).to_string(),
            self.text(right.0.span).to_string(),
        );
        let (left_shown, right_shown) = (self.show_type(left.1), self.show_type(right.1));
        let mut diagnostic = Diagnostic::error(
            "L0211",
            format!(
                "`{left_text}` is a `{left_shown}` and `{right_text}` is a `{right_shown}`; {what} is between two values of one type"
            ),
            span,
        );
        let machine = (left.1.as_machine(), right.1.as_machine());
        if let (Some(_), Some(to)) = machine {
            diagnostic = diagnostic.note(if formula {
                format!(
                    "compare `{left_text} as Int` with `{right_text} as Int`, which is exact, or cast `{left_text}` to `{}`, which wraps",
                    to.name()
                )
            } else {
                format!(
                    "cast one side: `{left_text} as {}` wraps into `{}`",
                    to.name(),
                    to.name()
                )
            });
        } else if formula
            && (machine.0.is_some() && same_type(right.1, &Type::Int)
                || machine.1.is_some() && same_type(left.1, &Type::Int))
        {
            let (value, other) = if machine.0.is_some() {
                (&left_text, &right_text)
            } else {
                (&right_text, &left_text)
            };
            diagnostic = diagnostic.note(format!(
                "compare `{value} as Int` with `{other}`: a machine integer enters the logic through `as Int`"
            ));
        }
        self.diagnostics.push(diagnostic);
        Err(())
    }

    fn prop_named(&self, callee: &ast::Expr) -> Option<std::rc::Rc<super::env::PropInfo>> {
        let ExprKind::Name(name) = &callee.kind else {
            return None;
        };
        if self.lookup(&name.text).is_some() {
            return None;
        }
        match self.types.get(&name.text) {
            Some(Global::Prop(info)) => Some(std::rc::Rc::clone(info)),
            _ => None,
        }
    }
}
