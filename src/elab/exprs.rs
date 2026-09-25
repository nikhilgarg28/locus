//! Expressions to typed trees: the entry points, and the dispatch on the kind
//! of expression to the module that elaborates it.
//!
//! Elaboration is bidirectional: an expression is elaborated against the
//! type expected of it when there is one, which is how a `_` learns what to
//! prove and how a tuple learns that its second field speaks of its first.

use crate::ast::{self, BinaryOp, ExprKind, PatternKind, UnaryOp};
use crate::kernel::{Proof, Term, Type, same_type};
use crate::source::Span;
use crate::typed::{self, Block, Expr, FnRef, Pattern, Stmt, is_pure, value_term};

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
    /// `L0215`, at the keyword of a `loop`, `while`, or `for` that cannot
    /// stand where it does: in a function that promises `terminates`, or in
    /// a formula. A bounded `for` counts as a loop; the promise is kept by
    /// what a function calls, and loops that keep it come with recursion.
    fn loop_refused<T>(&mut self, expr: &ast::Expr) -> Elab<T> {
        let keyword = match &expr.kind {
            ExprKind::Loop { .. } => "loop",
            ExprKind::While { .. } => "while",
            _ => "for",
        };
        let text = self.text(expr.span);
        let at = text.find(keyword).unwrap_or(0);
        let span = Span::new(
            expr.span.file,
            expr.span.start + at,
            expr.span.start + at + keyword.len(),
        );
        let diagnostic = match self.formula {
            Some(place) => crate::diagnostic::Diagnostic::error(
                "L0215",
                format!("`{keyword}` cannot appear in {place}: nothing there runs"),
                span,
            ),
            None => crate::diagnostic::Diagnostic::error(
                "L0215",
                format!(
                    "`{keyword}` cannot appear in `{}`, which promises terminates",
                    self.item_name
                ),
                span,
            )
            .note("a function that promises terminates contains no loop of any kind, a bounded `for` included, and calls only functions that promise it; loops that keep the promise come with recursion"),
        };
        self.diagnostics.push(diagnostic);
        Err(())
    }

    /// A never-typed value where `expected` is wanted. A transfer of control
    /// and a panic carry the type expected of them already, or end the
    /// block they stand in, and are left as they are. A call to a function
    /// declared `-> !` yields evidence of `False` in the logic; where a
    /// value of another type is wanted, the call runs and the evidence gives
    /// the value by `match {}`, which nothing ever reaches.
    pub(super) fn never_as(expr: Expr, expected: &Type) -> Expr {
        match &expr {
            Expr::CallFn { result, ty, .. } if !same_type(ty, expected) => {
                let absurd = Expr::Absurd {
                    proof: Proof::OfTerm(Term::var(*result)),
                    ty: expected.clone(),
                };
                Expr::Block(typed::Block {
                    stmts: vec![typed::Stmt::Expr(expr)],
                    tail: Some(Box::new(absurd)),
                })
            }
            _ => expr,
        }
    }

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
        // The result type of a function with `&mut` parameters speaks of
        // their values at return, which are their versions current here:
        // the substitution waits for the point where the value is checked,
        // since a branch or a call on the way may have made a version
        // (`references.rs`).
        let expected = &*self.at_current_exit(expected);
        if same_type(&value.ty, expected) {
            return Ok(value);
        }
        if self.is_natural(&value.ty) && *expected == Type::Int {
            return self.natural_integer(value, span);
        }
        // The never type coerces to any type. A `return`, `break`,
        // `continue`, or panic produces no value, and lowering ends the
        // block with it; a call to a function declared `-> !` produces
        // evidence of `False`, from which the wanted value follows.
        if value.never {
            return Ok(Value {
                expr: Env::never_as(value.expr, expected),
                ty: expected.clone(),
                never: true,
            });
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
            let solved = solved?;
            // The evidence found speaks of the value's term. A value with
            // effects, a call for one, is kept for them and for the
            // bindings its lowering makes, which the evidence mentions;
            // a pure value is in the evidence already.
            let expr = if is_pure(&value.expr) {
                Expr::Proof(solved)
            } else {
                Expr::Block(Block {
                    stmts: vec![Stmt::Let {
                        pattern: Pattern::Wildcard,
                        value: value.expr,
                    }],
                    tail: Some(Box::new(Expr::Proof(solved))),
                })
            };
            return Ok(Value::new(expr, expected.clone()));
        }
        let (wanted, found) = (self.show_type(expected), self.show_type(&value.ty));
        self.fail(
            "L0220",
            format!("expected `{wanted}`, found `{found}`"),
            span,
        )
    }

    pub(super) fn expr(&mut self, expr: &ast::Expr, expected: Option<&Type>) -> Elab<Value> {
        let value = self.expr_inner(expr, expected)?;
        let value = self.check_value_layout(value, expr.span)?;
        if self.total
            && (!self.in_constant || self.formula != Some("the value of a constant"))
            && self.suppress_models == 0
            && expected.is_none_or(|ty| {
                self.session.program().definitions().is_erased_type(ty) || matches!(ty, Type::Bool)
            })
        {
            self.logical_value(value, expr.span)
        } else {
            Ok(value)
        }
    }

    fn expr_inner(&mut self, expr: &ast::Expr, expected: Option<&Type>) -> Elab<Value> {
        match &expr.kind {
            ExprKind::Scoped { value, ty, contextual } => {
                // Expected types have already been instantiated over the actual
                // call arguments and current SSA versions by the elaborator.
                if *contextual && let Some(expected) = expected {
                    self.check(value, expected)
                } else {
                    let ty = self.ty(ty)?;
                    self.check(value, &ty)
                }
            }
            ExprKind::Array(fields) => self.array_literal(fields, expected, expr.span),
            ExprKind::Subscript { value, index } => self.buffer_operation(crate::kernel::BufferOp::Get,value,&[(**index).clone()],expr.span),
            ExprKind::GenericApply { .. } => {
                self.require_preview(crate::preview::Feature::LogicalData, "generic application", expr.span)?;
                self.fail("L0290", "generic application requires an instantiated declaration", expr.span)
            },
            ExprKind::Logic(block) => self.logic_block(block, expected),
            ExprKind::Evidence { constructor, evidence, .. } => self.named_evidence(constructor, evidence, expected, expr.span),
            ExprKind::Error => Err(()),
            ExprKind::Group(inner) => self.expr(inner, expected),
            ExprKind::Unit => Ok(Value::new(Expr::unit(), unit_type())),
            ExprKind::Bool(value) => Ok(Value::new(Expr::Bool(*value), Type::Bool)),
            ExprKind::Integer(literal) => self.literal(literal, false, expected, expr.span),
            // A negative literal is one literal, so that `-128i8` fits.
            ExprKind::Unary {
                operator: UnaryOp::Neg,
                expr: inner,
                ..
            } if matches!(inner.kind, ExprKind::Integer(_)) => {
                let ExprKind::Integer(literal) = &inner.kind else {
                    unreachable!("matched just above")
                };
                self.literal(literal, true, expected, expr.span)
            }
            ExprKind::String(_) => {
                self.fail("L0290", "string literals are not in Locus yet", expr.span)
            }
            ExprKind::Name(name) => self.name(name, expected),
            ExprKind::Closure { parameters, body } => self.logical_closure(parameters, body, expected, expr.span),
            ExprKind::Hole => match expected.map(|expected| self.at_current_exit(expected)) {
                Some(expected) if matches!(*expected, Type::Proof(_)) => {
                    let Type::Proof(claim) = &*expected else {
                        unreachable!("matched just above")
                    };
                    let proof = self.solve(claim, expr.span, None)?;
                    Ok(Value::new(Expr::Proof(proof), Type::Proof(claim.clone())))
                }
                Some(other) => {
                    let shown = self.show_type(&other);
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
                ..
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
                operator_span,
                left,
                right,
            } if operator.is_arithmetic() => {
                self.arithmetic(expr, *operator, *operator_span, left, right, expected)
            }
            ExprKind::Binary {
                operator,
                left,
                right,
                ..
            } => self.compare(expr, operator, left, right, expected),
            ExprKind::Unary {
                operator: UnaryOp::Neg,
                operator_span,
                expr: inner,
            } => self.negate(expr, *operator_span, inner, expected),
            ExprKind::Unary {
                operator: UnaryOp::Deref,
                expr: inner,
                ..
            } => self.deref(inner, expected, expr.span),
            ExprKind::Cast {
                expr: inner,
                as_span,
                ty,
                ..
            } => self.cast(inner, ty, *as_span),
            ExprKind::Struct { path, fields } => match path.single() {
                Some(name) => self.struct_literal(name, fields, expected, expr.span),
                None => self.variant_literal(path, fields, expected, expr.span),
            },
            ExprKind::Path(path) => match self.associated_constant(path) {
                Some(constant) => constant,
                None if self.path_function(path).is_some_and(|info| info.constant) => {
                    let info = self.path_function(path).expect("matched constant");
                    self.call_fn(&info, &[], expr.span)
                }
                None if self.path_function(path).is_some() => self.fail(
                    "L0290",
                    "a function used as a value is not supported yet; call it",
                    expr.span,
                ),
                None => self.variant(path, &[], expected, expr.span),
            },
            ExprKind::Call { callee, arguments } => {
                let mut value = self.call(callee, arguments, expected, expr.span)?;
                // A call to a function declared `-> !` never returns.
                if let Expr::CallFn { id, .. } = &value.expr
                    && self.never_fns.contains(id)
                {
                    value.never = true;
                }
                Ok(value)
            }
            // A field of a local is copied or moved on its own (`moves.rs`).
            ExprKind::Member { value, name } => {
                let outermost = self.mark_place_root(expr);
                let value = self.member(expr, value, name);
                self.place_used(value, expr.span, outermost)
            }
            ExprKind::Index {
                value,
                index,
                index_span,
            } => {
                let outermost = self.mark_place_root(expr);
                let value = self.index(expr, value, index, index_span);
                self.place_used(value, expr.span, outermost)
            }
            ExprKind::Block(block) => {
                let entry = self.mutable_entry();
                let mark = self.mark();
                let result_scope = self.result_scope();
                let result = self.block(block, expected);
                let result = self.check_scope_result(&result_scope, result, block.span);
                // The block's statements are spliced into the enclosing
                // sequence, so what it assigned to an outer binding stays
                // assigned, and the facts about the versions it made stay
                // known (`mutation.rs`).
                let kept = self.facts_since(&mark, &entry);
                self.close_names(mark);
                self.facts.extend(kept);
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
            // Every loop form, before any is elaborated or reported as not
            // in Locus yet: under `terminates` there is no iteration, and a
            // proposition runs nothing.
            ExprKind::Loop { .. } | ExprKind::While { .. } | ExprKind::For { .. }
                if self.promises.terminates || self.formula.is_some() =>
            {
                self.loop_refused(expr)
            }
            ExprKind::Loop { body } => self.loop_(body, expected, expr.span),
            ExprKind::While {
                pattern: None,
                condition,
                body,
            } => self.while_(condition, body, expr.span),
            ExprKind::While {
                pattern: Some(_), ..
            } => self.fail(
                "L0290",
                "`while let` is not in Locus yet; it comes with the patterns E9 adds",
                expr.span,
            ),
            ExprKind::For {
                pattern,
                iterable,
                body,
                ..
            } => match (&pattern.kind, &iterable.kind) {
                (
                    PatternKind::Name {
                        name,
                        mutable: false,
                    },
                    ExprKind::Range { kind, lower, upper },
                ) => self.for_(name, *kind, lower, upper, body, expr.span),
                (_, ExprKind::Range { .. }) => self.fail(
                    "L0290",
                    "the index of a `for` over a range is a name; other patterns are not in Locus yet",
                    pattern.span,
                ),
                _ => self.fail(
                    "L0290",
                    "a `for` over anything but a range `lo..hi` or `lo..=hi` is not in Locus yet; iterators come later",
                    iterable.span,
                ),
            },
            ExprKind::Break(value) => self.break_(expr, value.as_deref()),
            ExprKind::Continue => self.continue_(expr),
            ExprKind::Range { .. } => self.fail(
                "L0290",
                "a range is read only in the header of a `for` for now",
                expr.span,
            ),
            ExprKind::Return(value) => self.return_(expr, value.as_deref(), expected),
            ExprKind::Ref { mutable, expr: inner } => self.shared_reference(inner,*mutable,expr.span),
        }
    }

    /// `*self`, read: the value behind the reference receiver of a method
    /// (O4). `*` is written on `self` alone, and only where `self` is a
    /// reference; a `&mut self` receiver is assigned through it as well
    /// (`mutation.rs`).
    fn deref(&mut self, inner: &ast::Expr, expected: Option<&Type>, span: Span) -> Elab<Value> {
        if let ExprKind::Name(name) = &inner.kind
            && self
                .lookup(&name.text)
                .is_some_and(|local| self.borrowed.contains(&local.binding.unwrap_or(local.id)))
        {
            return self.name(name, expected);
        }
        // Resolve the physical pointer before modeling its referent. In
        // particular, the Bool model must not replace an &Bool operand with
        // a logical marker before this dereference checks its provenance.
        self.suppress_models += 1;
        let value = self.lending(|env| env.infer(inner));
        self.suppress_models -= 1;
        let value = value?;
        if let Type::Boxed(element) = &value.ty {
            if !self.reading() && !self.is_copy(element) {
                self.consume_value_place(&value, span);
            }
            return self.boxed_deref(value, span);
        }
        if !matches!(
            self.session.expression_layout(&value.expr),
            crate::typed::ErasureLayout::Shared { .. }
        ) {
            return self.fail("L0266", "dereference requires a shared reference", span);
        }
        if !self.reading() && !self.is_copy(&value.ty) {
            return self.fail(
                "L0286",
                "cannot move a non-Copy value out of a shared reference",
                span,
            );
        }
        Ok(Value::new(Expr::Deref(Box::new(value.expr)), value.ty))
    }

    /// The `self` a `*self` is written on, when it is a reference
    /// receiver; `L0266` otherwise, in rustc's words where it has them.
    pub(super) fn deref_target<'e>(
        &mut self,
        inner: &'e ast::Expr,
        span: Span,
    ) -> Elab<&'e ast::Name> {
        let ExprKind::Name(name) = &inner.kind else {
            let text = self.text(inner.span).to_string();
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0266",
                    format!("`*` is written on `self` alone, and `*{text}` reads as `*({text})`"),
                    span,
                )
                .note("a field of the receiver is read as `self.f` and written as `self.f = v`; `*self` is the whole value behind a `&self` or `&mut self` receiver"),
            );
            return Err(());
        };
        let Some(local) = self.lookup(&name.text) else {
            return self.fail("L0204", format!("unknown name `{}`", name.text), name.span);
        };
        if local.poisoned {
            return Err(());
        }
        let id = local.binding.unwrap_or(local.id);
        let ty = local.ty.clone();
        if !self.borrowed.contains(&id) {
            let ty = self.show_type(&ty);
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0266",
                    format!("type `{ty}` cannot be dereferenced (E0614)"),
                    span,
                )
                .note(format!(
                    "`{0}` is taken by value here, and is the value itself: write `{0}`; `*{0}` is the value behind a `&self` or `&mut self` receiver",
                    name.text
                )),
            );
            return Err(());
        }
        Ok(name)
    }

    fn name(&mut self, name: &ast::Name, expected: Option<&Type>) -> Elab<Value> {
        if let Some(slot) = self.names.iter().rposition(|local| local.name == name.text) {
            self.check_closure_capture(slot, name.span)?;
            // A use of a value moves it, unless it is `Copy` or is read
            // where nothing runs (`moves.rs`).
            self.use_local(slot, name.span);
            let local = &self.names[slot];
            if local.poisoned {
                return Err(());
            }
            // Tracked evidence is read at its current version, typed over
            // the current versions of what it mentions, and only while it
            // is valid (`mutation.rs`).
            if let Some(stale) = local
                .tracked
                .as_ref()
                .and_then(|tracked| tracked.stale.clone())
            {
                return self.stale_error(slot, &stale, "L0245", "before using it", name.span);
            }
            let ghost = local.ghost;
            let (id, ty) = (local.id, self.version_type(slot));
            let expr = Expr::Var {
                id,
                name: name.text.clone(),
                ty: ty.clone(),
            };
            return Ok(Value::new(
                if ghost {
                    Expr::Ghost(Box::new(expr))
                } else {
                    expr
                },
                ty,
            ));
        }
        // A name is a value; a type of the same name is another thing.
        let global = self
            .values
            .get(&name.text)
            .or_else(|| self.types.get(&name.text))
            .cloned();
        match global {
            Some(Global::Fn(info)) if info.constant => self.call_fn(&info, &[], name.span),
            // A function of the logic returning evidence is evidence of its general claim,
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
