//! Integer literals, the associated constants `T::MAX` and `T::MIN`, and
//! `as`.
//!
//! A literal takes the type expected of it when there is one, a machine
//! integer type or `Int`; else the type its suffix names; else `i32`, as in
//! Rust. It is then checked against the range of that type. A negative
//! literal is unary minus applied to a literal, and is read as one literal,
//! so that `-128i8` fits and `-129i8` does not. `Int` has literals of any
//! size, and none of `MAX` and `MIN`.
//!
//! `as` between machine types wraps, and is `cast[S, T]` in the kernel.
//! `x as Int` is the view of `x`, exact, and logic-only; `Int as T` is the
//! wrap into `T`, and stands only where nothing runs, as every `Int` does.

use crate::ast::{self, ExprKind, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::kernel::{Integer, MachineInt, Type};
use crate::source::Span;
use crate::typed::Expr;

use super::env::{Elab, Env};
use super::exprs::Value;

/// Whether an expression is a literal that takes its type from what stands
/// beside it: an integer literal without a suffix, possibly negated, or a
/// hole.
pub(super) fn untyped_literal(expr: &ast::Expr) -> bool {
    match &expr.kind {
        ExprKind::Integer(literal) => literal.suffix.is_none(),
        ExprKind::Hole => true,
        ExprKind::Group(inner) => untyped_literal(inner),
        ExprKind::Unary {
            operator: UnaryOp::Neg,
            expr: inner,
            ..
        } => matches!(&inner.kind, ExprKind::Integer(literal) if literal.suffix.is_none()),
        _ => false,
    }
}

impl Env<'_> {
    /// An integer literal, `negative` when it stands under a unary minus.
    pub(super) fn literal(
        &mut self,
        literal: &ast::IntegerLiteral,
        negative: bool,
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let value = Integer::from_parts(negative, literal.value.clone());
        if literal.suffix.is_none() && expected.is_some_and(|ty| self.is_natural(ty)) {
            if negative && !value.is_zero() {
                return self.fail("L0205", "Nat literals cannot be negative", span);
            }
            let evidence = crate::kernel::Proof::Evaluate(crate::kernel::Term::int_le(
                crate::kernel::Term::int(0),
                crate::kernel::Term::Int(value.clone()),
            ));
            return self.make_natural(
                Value::new(Expr::Int(value), Type::Int),
                Some(evidence),
                span,
            );
        }
        // A suffix names the type outright; without one the literal takes
        // the type expected of it, else `i32`. A suffixed literal where
        // another type is expected is a mismatch, reported by the caller.
        let ty = match (literal.suffix, expected) {
            (Some(suffix), _) => match self.machine_type(suffix.name()) {
                Some(ty) => ty,
                None => {
                    return self.fail(
                        "L0290",
                        format!("the type `{}` is not in Locus yet", suffix.name()),
                        span,
                    );
                }
            },
            (None, Some(Type::Int)) => return Ok(Value::new(Expr::Int(value), Type::Int)),
            (None, Some(expected)) if expected.as_machine().is_some() => {
                expected.as_machine().unwrap()
            }
            (None, _) if self.total => return Ok(Value::new(Expr::Int(value), Type::Int)),
            (None, _) => MachineInt::I32,
        };
        if !ty.contains(&value) {
            let message = format!(
                "`{value}` does not fit in `{}`, whose range is {} to {}",
                ty.name(),
                ty.min(),
                ty.max()
            );
            return self.fail("L0205", message, span);
        }
        let value = value
            .to_i128()
            .expect("a value of a machine type fits an i128");
        Ok(Value::new(Expr::Literal(ty, value), Type::machine(ty)))
    }

    /// `T::MAX` and `T::MIN` for a machine integer type `T`, as literals of
    /// that type. `None` when the path names no machine type.
    pub(super) fn associated_constant(&mut self, path: &ast::Path) -> Option<Elab<Value>> {
        let (prefix, name) = path.pair()?;
        if prefix.text == "Int" {
            return Some(self.fail(
                "L0204",
                format!(
                    "`Int` has no `{}`: the integers of the logic have no bounds",
                    name.text
                ),
                path.span,
            ));
        }
        let ty = self.machine_type(&prefix.text)?;
        let value = match name.text.as_str() {
            "MAX" => ty.max(),
            "MIN" => ty.min(),
            other => {
                return Some(self.fail(
                    "L0204",
                    format!(
                        "`{}` has no associated constant `{other}`; it has `MAX` and `MIN`",
                        ty.name()
                    ),
                    name.span,
                ));
            }
        };
        let value = value
            .to_i128()
            .expect("a bound of a machine type fits an i128");
        Some(Ok(Value::new(Expr::Literal(ty, value), Type::machine(ty))))
    }

    /// `inner as target`.
    pub(super) fn cast(
        &mut self,
        inner: &ast::Expr,
        target: &ast::Type,
        as_span: Span,
    ) -> Elab<Value> {
        let to = self.ty(target)?;
        if super::dynamic::borrowed_dyn(target).is_some() {
            let value = self.infer(inner)?;
            return self.coerce(value, &to, as_span);
        }
        if self.logical_spelling(target) {
            return self.model_cast(inner, &to, as_span);
        }
        self.suppress_models += 1;
        let value = self.infer(inner);
        self.suppress_models -= 1;
        let value = value?;
        if value.ty.is_ghost() && to.as_machine().is_some() {
            return self.fail(
                "L0272",
                "logical data cannot be converted into a runtime value",
                as_span,
            );
        }
        let from = value.ty.clone();
        match (from.as_machine(), &to) {
            (Some(_), Type::Int) => {}
            (Some(_), to) if to.as_machine().is_some() => {}
            // `self.ty` admits `Int` only where nothing runs, so the wrap
            // stands in a ghost position.
            (None, to) if matches!(from, Type::Int) && to.as_machine().is_some() => {}
            // `Int as Int` converts nothing.
            (None, Type::Int) if matches!(from, Type::Int) => return Ok(value),
            _ => {
                let (shown_from, shown_to) = (self.show_type(&from), self.show_type(&to));
                self.diagnostics.push(
                    Diagnostic::error(
                        "L0290",
                        format!("`as` from `{shown_from}` to `{shown_to}` is not in Locus yet"),
                        as_span,
                    )
                    .note("`as` converts between the machine integer types `u8` to `i64`, and from one of them to `Int` in a claim"),
                );
                return Err(());
            }
        }
        Ok(Value::new(
            Expr::Cast {
                expr: Box::new(value.expr),
                from,
                to: to.clone(),
            },
            to,
        ))
    }
}
