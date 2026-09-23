//! Blocks and the statements in them. A `let` binds through `patterns`,
//! and an assignment through `mutation`.
//!
//! Classification comes from the value's type. Binding a logical value
//! does not erase runtime effects in its initializer; lowering preserves
//! those effects before the logical result is replaced with a marker.

use crate::ast::{self, ExprKind, Form, StatementKind};
use crate::diagnostic::Diagnostic;
use crate::kernel::{Type, same_type};
use crate::source::Span;
use crate::typed::{self, Stmt};

use super::env::{Elab, Env};
use super::exprs::unit_type;

impl Env<'_> {
    /// Elaborates a block. The caller opens and closes the scope.
    ///
    /// An expression statement that transfers control or panics, of the
    /// never type, ends the block: it becomes the block's tail, since
    /// lowering makes an ending of it, and what is written after it is
    /// unreachable, reported as rustc reports it and not elaborated, as
    /// nothing runs it. The third result is whether the block ends that
    /// way.
    pub fn block(
        &mut self,
        block: &ast::Block,
        expected: Option<&Type>,
    ) -> Elab<(typed::Block, Type, bool)> {
        let mut stmts = Vec::new();
        let mut failed = false;
        // The never-typed expression statement that ends the block, and
        // whether what follows it was reported, once.
        let mut leaves: Option<Span> = None;
        let mut warned = false;
        for statement in &block.statements {
            if let Some(by) = leaves {
                self.unreachable(by, statement.span, "statement");
                warned = true;
                break;
            }
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
                        if let Some(annotation) = annotation {
                            self.expect_layout(value, &self.written_layout(annotation));
                        }
                        let annotated = match annotation {
                            Some(annotation) => Some(self.let_annotation(annotation)?),
                            None => None,
                        };
                        // A value that is a place is taken apart by the
                        // pattern, which moves the parts it binds (`moves.rs`).
                        let span = value.span;
                        self.mark_place_root(value);
                        let mut value = match &annotated {
                            Some(written) => self.argument(value, &written.ty, written.ghost)?,
                            None => self.infer(value)?,
                        };
                        if has_evidence_pattern(pattern) {
                            return self.open_named_let(pattern, value, statement.span);
                        }
                        let place = self.place_taken(&value, span);
                        let term = self.term(&value, statement.span)?;
                        let logical = !value.ty.is_ghost()
                            && (annotated.as_ref().is_some_and(|a| a.ghost)
                                || super::reconcile::is_logical_expr(&value.expr));
                        if logical {
                            value.expr = super::calls::ghost_value(value.expr);
                        }
                        let layout = self.session.expression_layout(&value.expr);
                        let typed = self.bind_pattern(pattern, term, logical)?;
                        self.register_pattern_layout(&typed, &layout);
                        self.move_by_pattern(place.as_ref(), pattern, &typed, span);
                        Ok(vec![Stmt::Let {
                            pattern: typed,
                            value: value.expr,
                        }])
                    })();
                    match result {
                        Ok(added) => stmts.extend(added),
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
                        if value.never {
                            leaves = Some(expr.span);
                        }
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
        if let Some(by) = leaves {
            if let Some(tail) = block.tail.as_deref()
                && !warned
            {
                self.unreachable(by, tail.span, "expression");
            }
            // The transfer of control is the block's tail, and supplies
            // whatever value the block was to have (`never_as`).
            let Some(Stmt::Expr(tail)) = stmts.pop() else {
                unreachable!("the statement that leaves was pushed last")
            };
            let ty = expected.cloned().unwrap_or_else(unit_type);
            let tail = Env::never_as(tail, &ty);
            return Ok((
                typed::Block {
                    stmts,
                    tail: Some(Box::new(tail)),
                },
                ty,
                true,
            ));
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
                    value.never,
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

    /// `L0247`, a warning: a statement, or the tail, after an expression
    /// statement that transfers control or panics, as rustc's
    /// `unreachable_code` warns. It is reported once per block, on the
    /// first thing that is unreachable, and what is unreachable is not
    /// elaborated, since nothing runs it.
    fn unreachable(&mut self, by: Span, at: Span, what: &str) {
        let mut diagnostic = Diagnostic::warning("L0247", format!("unreachable {what}"), at)
            .label(by, "any code following this expression is unreachable");
        diagnostic.labels[0].message = format!("unreachable {what}");
        self.diagnostics.push(diagnostic);
    }
}

fn has_evidence_pattern(pattern: &ast::Pattern) -> bool {
    match &pattern.kind {
        ast::PatternKind::Evidence { .. } => true,
        ast::PatternKind::Group(inner) | ast::PatternKind::Binding { pattern: inner, .. } => {
            has_evidence_pattern(inner)
        }
        _ => false,
    }
}
