//! Expressions to typed trees: the entry points, and the dispatch on the kind
//! of expression to the module that elaborates it.
//!
//! Elaboration is bidirectional: an expression is elaborated against the
//! type expected of it when there is one, which is how a `_` learns what to
//! prove and how a tuple learns that its second field speaks of its first.

use crate::ast::{self, BinaryOp, ExprKind};
use crate::kernel::{Proof, Term, Type, same_type};
use crate::source::Span;
use crate::typed::{Expr, FnRef, value_term};

use super::control::Branch;
use super::env::{Elab, Env, Global};

pub(super) struct Value {
    pub expr: Expr,
    pub ty: Type,
    /// The expression transfers control and produces no value.
    pub never: bool,
}

impl Value {
    pub fn new(expr: Expr, ty: Type) -> Self {
        Self {
            expr,
            ty,
            never: false,
        }
    }
}

pub(super) fn unit_type() -> Type {
    Type::Tuple(Vec::new())
}

impl Env<'_> {
    /// The kernel term that stands for the value, as lowering will state it.
    pub fn term(&mut self, value: &Value, span: Span) -> Elab<Term> {
        match value_term(&value.expr) {
            Ok(term) => Ok(term),
            Err(error) => self.internal(error, span),
        }
    }

    pub fn infer(&mut self, expr: &ast::Expr) -> Elab<Value> {
        self.expr(expr, None)
    }

    pub fn check(&mut self, expr: &ast::Expr, expected: &Type) -> Elab<Value> {
        let value = self.expr(expr, Some(expected))?;
        self.coerce(value, expected, expr.span)
    }

    /// Accepts a value where `expected` is wanted. Evidence of one claim is
    /// accepted for another when the solver can bridge them, which is what
    /// lets a fact about `result.0` serve as a fact about the `next` it was
    /// bound to.
    pub fn coerce(&mut self, value: Value, expected: &Type, span: Span) -> Elab<Value> {
        if value.never || same_type(&value.ty, expected) {
            return Ok(value);
        }
        if let (Type::Proof(found), Type::Proof(wanted)) = (&value.ty, expected) {
            let mark = self.mark();
            let term = self.term(&value, span)?;
            self.facts.push(super::env::Fact::new(
                Proof::OfTerm(term),
                (**found).clone(),
            ));
            let solved = self.solve(wanted, span, Some(found));
            self.close_names(mark);
            return Ok(Value::new(Expr::Proof(solved?), expected.clone()));
        }
        let (wanted, found) = (self.show_type(expected), self.show_type(&value.ty));
        self.fail(
            "L0220",
            format!("expected `{wanted}`, found `{found}`"),
            span,
        )
    }

    fn expr(&mut self, expr: &ast::Expr, expected: Option<&Type>) -> Elab<Value> {
        match &expr.kind {
            ExprKind::Error => Err(()),
            ExprKind::Group(inner) => self.expr(inner, expected),
            ExprKind::Unit => Ok(Value::new(Expr::unit(), unit_type())),
            ExprKind::Bool(value) => Ok(Value::new(Expr::Bool(*value), Type::Bool)),
            // Until literals are typed, `u8` is the one integer type.
            ExprKind::Integer(literal) => match literal.suffix {
                None | Some(ast::IntegerSuffix::U8) => {
                    let value = literal.value.to_u64().map(u8::try_from);
                    match value {
                        Some(Ok(value)) => Ok(Value::new(Expr::U8(value), Type::U8)),
                        _ => self.fail(
                            "L0205",
                            format!(
                                "`{}` does not fit in `u8`, whose largest value is 255",
                                literal.value
                            ),
                            expr.span,
                        ),
                    }
                }
                Some(suffix) => self.fail(
                    "L0290",
                    format!("the type `{}` is not in Locus yet", suffix.name()),
                    expr.span,
                ),
            },
            ExprKind::String(_) => {
                self.fail("L0290", "string literals are not in Locus yet", expr.span)
            }
            ExprKind::Name(name) => self.name(name, expected),
            ExprKind::Hole => match expected {
                Some(Type::Proof(claim)) => {
                    let proof = self.solve(claim, expr.span, None)?;
                    Ok(Value::new(Expr::Proof(proof), Type::Proof(claim.clone())))
                }
                Some(other) => {
                    let shown = self.show_type(other);
                    self.fail(
                        "L0206",
                        format!("`_` asks for evidence, and a `{shown}` is needed here"),
                        expr.span,
                    )
                }
                None => {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0206",
                            "`_` asks for evidence, and nothing here says of what",
                            expr.span,
                        )
                        .note("state the claim with an annotation, `let evidence: @claim = _;`, or where it stands, `prove!(claim)`"),
                    );
                    Err(())
                }
            },
            ExprKind::Tuple(items) => self.tuple(items, expected, expr.span),
            ExprKind::Form {
                form,
                name_span,
                arguments,
            } => self.form(*form, *name_span, arguments, expected, expr.span),
            ExprKind::Forall { .. }
            | ExprKind::Exists { .. }
            | ExprKind::Binary {
                operator: BinaryOp::Implies,
                ..
            } => {
                let term = self.formula(expr)?;
                Ok(Value::new(Expr::Prop(term), Type::Prop))
            }
            ExprKind::Not(_)
            | ExprKind::Binary {
                operator: BinaryOp::And | BinaryOp::Or,
                ..
            } if expected.is_some_and(|ty| same_type(ty, &Type::Prop))
                || self.reads_as_prop(expr) =>
            {
                let term = self.formula(expr)?;
                Ok(Value::new(Expr::Prop(term), Type::Prop))
            }
            ExprKind::Not(inner) => self.not(expr, inner, expected),
            ExprKind::Binary {
                operator: operator @ (BinaryOp::And | BinaryOp::Or),
                left,
                right,
                ..
            } => self.short_circuit(expr, operator, left, right),
            ExprKind::Binary {
                operator,
                left,
                right,
                ..
            } => self.compare(expr, operator, left, right, expected),
            ExprKind::Unary { .. } | ExprKind::Cast { .. } => self.operator_not_yet(expr),
            ExprKind::Struct { name, fields } => self.struct_literal(name, fields, expr.span),
            ExprKind::Path(path) => self.variant(path, &[], expected, expr.span),
            ExprKind::Call { callee, arguments } => {
                self.call(callee, arguments, expected, expr.span)
            }
            ExprKind::Member { value, name } => self.member(expr, value, name),
            ExprKind::Index {
                value,
                index,
                index_span,
            } => self.index(expr, value, index, index_span),
            ExprKind::Block(block) => {
                let mark = self.mark();
                let result = self.block(block, expected);
                self.close_names(mark);
                let (block, ty, never) = result?;
                Ok(Value {
                    expr: Expr::Block(block),
                    ty,
                    never,
                })
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.conditional(
                condition,
                Branch::Block(then_branch),
                Branch::Expr(else_branch),
                expected,
                expr.span,
            ),
            ExprKind::Match { scrutinee, arms } => {
                self.match_(scrutinee, arms, expected, expr.span)
            }
            ExprKind::Loop {
                state,
                result,
                body,
            } => self.loop_(state, result, body, expr.span),
            ExprKind::For {
                index,
                lower,
                upper,
                state,
                body,
            } => self.for_(index, lower, upper, state, body, expr.span),
            ExprKind::Break(value) => self.break_(expr, value),
            ExprKind::Continue(arguments) => self.continue_(expr, arguments),
        }
    }

    fn name(&mut self, name: &ast::Name, expected: Option<&Type>) -> Elab<Value> {
        if let Some(local) = self.lookup(&name.text) {
            if local.poisoned {
                return Err(());
            }
            let (id, ty) = (local.id, local.ty.clone());
            return Ok(Value::new(
                Expr::Var {
                    id,
                    name: name.text.clone(),
                    ty: ty.clone(),
                },
                ty,
            ));
        }
        match self.globals.get(&name.text).cloned() {
            Some(Global::Fn(info)) if info.constant => self.call_fn(&info, &[], name.span),
            // A `math fn` returning evidence is evidence of its general claim,
            // where evidence is expected or where nothing in particular is,
            // as the second argument of `fold!` or `rewrite!`.
            Some(Global::Fn(info))
                if matches!(expected, Some(Type::Proof(_)))
                    || (expected.is_none()
                        && matches!(info.reference, FnRef::Math(_))
                        && matches!(info.result, Type::Proof(_))) =>
            {
                self.function_as_evidence(&info, name.span)
            }
            Some(Global::Fn(_)) => self.fail(
                "L0290",
                "a function used as a value is not supported yet; call it",
                name.span,
            ),
            Some(_) => self.fail(
                "L0204",
                format!("`{}` is not a value", name.text),
                name.span,
            ),
            None if self.failed.contains(&name.text) => Err(()),
            None => self.fail("L0204", format!("unknown name `{}`", name.text), name.span),
        }
    }
}
