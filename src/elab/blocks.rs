//! Blocks and the statements in them. A `let` binds through `patterns`,
//! and an assignment through `mutation`.
//!
//! A `let` whose value has no runtime form, `let g: Ghost<T> = ...`,
//! `let n: Int = ...`, `let c: Prop = ...`, or `let g = snapshot!(x)`, is
//! a logic-only context: its value is elaborated where nothing runs, so
//! every call in it is one a proposition admits and every name in it is
//! read, and the value is wrapped as a `Ghost` value, which erasure makes a
//! marker. Erasure then leaves the whole `let` out. A `let` of evidence is
//! not one: a call that returns evidence is how an ordinary function
//! establishes a fact, and it stays.

use crate::ast::{self, ExprKind, Form, StatementKind};
use crate::kernel::{Type, same_type};
use crate::typed::{self, Expr, Stmt};

use super::env::{Elab, Env};
use super::exprs::{Value, unit_type};
use super::types::logical_data;

/// Whether the expression is `snapshot!(...)`, possibly in parentheses.
fn is_snapshot(expr: &ast::Expr) -> bool {
    match &expr.kind {
        ExprKind::Group(inner) => is_snapshot(inner),
        ExprKind::Form {
            form: Form::Snapshot,
            ..
        } => true,
        _ => false,
    }
}

impl Env<'_> {
    /// Elaborates a block. The caller opens and closes the scope.
    pub fn block(
        &mut self,
        block: &ast::Block,
        expected: Option<&Type>,
    ) -> Elab<(typed::Block, Type, bool)> {
        let mut stmts = Vec::new();
        let mut failed = false;
        let mut never = false;
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Error => failed = true,
                // `let mut` is read off the pattern's name, where the parser
                // records it too.
                StatementKind::Assign { place, value } => {
                    match self.assign_statement(place, value, statement.span) {
                        Ok(stmt) => {
                            // The place is whole again (`moves.rs`).
                            self.assigned(&stmt, place.span);
                            stmts.push(stmt);
                        }
                        Err(()) => failed = true,
                    }
                }
                StatementKind::Let {
                    pattern,
                    annotation,
                    value,
                    ..
                } => {
                    let result = (|| {
                        let annotated = match annotation {
                            Some(annotation) => Some(self.let_annotation(annotation)?),
                            None => None,
                        };
                        let ghost_let = match &annotated {
                            Some(written) => written.ghost || logical_data(&written.ty),
                            None => is_snapshot(value),
                        };
                        if ghost_let {
                            let expected = annotated.as_ref().map(|written| written.ty.clone());
                            let value =
                                self.logical("the value of a `let` with no runtime form", |env| {
                                    match &expected {
                                        Some(ty) => env.check(value, ty),
                                        None => env.infer(value),
                                    }
                                })?;
                            let expr = match value.expr {
                                ghost @ Expr::Ghost(_) => ghost,
                                other => Expr::Ghost(Box::new(other)),
                            };
                            let term =
                                self.term(&Value::new(expr.clone(), value.ty), statement.span)?;
                            // Declared `Ghost<T>`, or bound to a snapshot: a
                            // `Prop` or an `Int` is ghost by its type.
                            let ghost = annotated.as_ref().is_none_or(|written| written.ghost);
                            let typed = self.bind_pattern(pattern, term, ghost)?;
                            return Ok(Stmt::Let {
                                pattern: typed,
                                value: expr,
                            });
                        }
                        // A value that is a place is taken apart by the
                        // pattern, which moves the parts it binds (`moves.rs`).
                        let span = value.span;
                        self.mark_place_root(value);
                        let value = match &annotated {
                            Some(written) => self.check(value, &written.ty)?,
                            None => self.infer(value)?,
                        };
                        let place = self.place_taken(&value, span);
                        let term = self.term(&value, statement.span)?;
                        let typed = self.bind_pattern(pattern, term, false)?;
                        self.move_by_pattern(place.as_ref(), pattern, &typed, span);
                        Ok(Stmt::Let {
                            pattern: typed,
                            value: value.expr,
                        })
                    })();
                    match result {
                        Ok(stmt) => stmts.push(stmt),
                        Err(()) => {
                            self.poison(pattern);
                            failed = true;
                        }
                    }
                }
                // `prove!(claim);` keeps its fact in scope and is no statement.
                StatementKind::Expression(ast::Expr {
                    kind:
                        ExprKind::Form {
                            form: Form::Prove,
                            arguments,
                            ..
                        },
                    span,
                }) => {
                    if self.prove_statement(&arguments[0], *span).is_err() {
                        failed = true;
                    }
                }
                StatementKind::Expression(expr) => match self.infer(expr) {
                    Ok(value) => {
                        never |= value.never;
                        stmts.push(Stmt::Expr(value.expr));
                    }
                    Err(()) => failed = true,
                },
            }
        }
        if failed {
            // What follows may depend on a binding that was not made.
            return Err(());
        }
        match block.tail.as_deref() {
            Some(tail) => {
                let value = match expected {
                    Some(expected) => self.check(tail, expected)?,
                    None => self.infer(tail)?,
                };
                Ok((
                    typed::Block {
                        stmts,
                        tail: Some(Box::new(value.expr)),
                    },
                    value.ty,
                    value.never || never,
                ))
            }
            None if never => {
                // `continue;` or `break;` written as a statement ends the block.
                let tail = match stmts.pop() {
                    Some(Stmt::Expr(expr)) => expr,
                    other => {
                        stmts.extend(other);
                        return self.fail(
                            "L0216",
                            "statements follow a transfer of control",
                            block.span,
                        );
                    }
                };
                Ok((
                    typed::Block {
                        stmts,
                        tail: Some(Box::new(tail)),
                    },
                    unit_type(),
                    true,
                ))
            }
            None => {
                let ty = unit_type();
                if let Some(expected) = expected
                    && !same_type(expected, &ty)
                {
                    let shown = self.show_type(expected);
                    return self.fail(
                        "L0220",
                        format!("this block ends without a value, and `{shown}` is expected"),
                        block.span,
                    );
                }
                Ok((typed::Block { stmts, tail: None }, ty, false))
            }
        }
    }
}
