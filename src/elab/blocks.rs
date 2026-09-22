//! Blocks and the statements in them. A `let` binds through `patterns`,
//! and an assignment through `mutation`.

use crate::ast::{self, ExprKind, Form, StatementKind};
use crate::kernel::{Type, same_type};
use crate::typed::{self, Stmt};

use super::env::{Elab, Env};
use super::exprs::unit_type;

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
                        Ok(stmt) => stmts.push(stmt),
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
                        let value = match annotation {
                            Some(annotation) => {
                                let ty = self.ty(annotation)?;
                                self.check(value, &ty)?
                            }
                            None => self.infer(value)?,
                        };
                        let term = self.term(&value, statement.span)?;
                        let pattern = self.bind_pattern(pattern, term)?;
                        Ok(Stmt::Let {
                            pattern,
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
