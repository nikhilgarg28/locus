//! Loops: `loop`, `while`, and `for` over a range, with `break` and
//! `continue`.
//!
//! What a loop carries, in the check IR, is the tuple of the bindings
//! declared outside it that its body assigns, or for a `while` its
//! condition (`Carried`). The elaborator finds that set by a scan of the
//! source before elaborating the body (`assigned_outside`), so that the body
//! works on fresh versions of those bindings, which is what lowering gives
//! them, and a proof in the body speaks of the version the checker binds.
//! After the loop each carried binding is at a version bound by projection
//! from the loop's result, exactly as after a branch that assigns
//! (`mutation.rs`); a `loop`'s value, what its `break` supplies, is the last
//! field of that result. Nothing here is trusted: lowering computes the
//! carried set itself and rejects a tree that names another, and a wrong
//! version in a proof is a proof the kernel rejects.
//!
//! Tracked evidence the body refreshes is carried like any binding, typed
//! in the state over the versions the body sees; the entry, every
//! `continue` and `break`, the end of the body, and the exit of a `while`
//! supply it at the current versions, so it must be valid there, and after
//! the loop it is valid over the versions after. Tracked evidence the body
//! does not refresh, but whose subject the loop assigns, is stale inside
//! the body and after the loop (`mutation.rs`).

use crate::ast::{self, ExprKind, PatternKind, RangeKind, StatementKind};
use crate::diagnostic::Diagnostic;
use crate::kernel::{HypId, Term, Type, VarId};
use crate::source::Span;
use crate::typed::{self, Binder, Carried, Expr};

use super::env::{Elab, Env, LoopTarget};
use super::exprs::{Value, unit_type};
use super::literals::untyped_literal;
use super::mutation::{Entry, JoinValue, Stale};

/// The names assigned somewhere in a loop's body, or in its condition,
/// that resolve outside the loop: a name declared inside, by a `let` or a
/// pattern, in the scope the assignment stands in, is local to the loop and
/// is left out. The scan reads the source as the elaborator will, one scope
/// at a time, so a name that shadows an outer one is the inner binding
/// from its declaration on. An assignment to a name that is not a mutable
/// binding is left to the elaborator to report.
fn assigned_outside(body: &ast::Block, condition: Option<&ast::Expr>) -> Vec<String> {
    let mut scan = Scan {
        scopes: vec![Vec::new()],
        found: Vec::new(),
    };
    if let Some(condition) = condition {
        scan.expr(condition);
    }
    scan.block(body);
    scan.found
}

struct Scan {
    scopes: Vec<Vec<String>>,
    found: Vec<String>,
}

impl Scan {
    fn declared(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .any(|scope| scope.iter().any(|n| n == name))
    }

    fn declare(&mut self, pattern: &ast::Pattern) {
        match &pattern.kind {
            PatternKind::Name { name, .. } => self
                .scopes
                .last_mut()
                .expect("a scope is open")
                .push(name.text.clone()),
            PatternKind::Wildcard
            | PatternKind::Unit
            | PatternKind::Bool(_)
            | PatternKind::Integer(_) => {}
            PatternKind::Group(inner) => self.declare(inner),
            PatternKind::Binding { name, pattern, .. } => {
                self.scopes
                    .last_mut()
                    .expect("a scope is open")
                    .push(name.text.clone());
                self.declare(pattern);
            }
            PatternKind::Evidence {
                constructor,
                evidence,
                ..
            } => {
                self.declare(constructor);
                self.declare(evidence);
            }
            PatternKind::Tuple(parts) => parts.iter().for_each(|part| self.declare(part)),
            PatternKind::Struct { fields, .. } => {
                fields.iter().for_each(|field| self.declare(&field.pattern));
            }
            PatternKind::Variant { arguments, .. } => {
                arguments
                    .iter()
                    .flatten()
                    .for_each(|part| self.declare(part));
            }
        }
    }

    fn scoped(&mut self, inside: impl FnOnce(&mut Self)) {
        self.scopes.push(Vec::new());
        inside(self);
        self.scopes.pop();
    }

    fn block(&mut self, block: &ast::Block) {
        self.scoped(|scan| {
            for statement in &block.statements {
                match &statement.kind {
                    StatementKind::Let { pattern, value, .. } => {
                        scan.expr(value);
                        scan.declare(pattern);
                    }
                    StatementKind::Assign { place, value } => {
                        scan.expr(place);
                        scan.expr(value);
                        let mut root = place;
                        while let ExprKind::Member { value, .. }
                        | ExprKind::Index { value, .. }
                        | ExprKind::Subscript { value, .. } = &root.kind
                        {
                            root = value;
                        }
                        if let ExprKind::Name(name) = &root.kind
                            && !scan.declared(&name.text)
                            && !scan.found.contains(&name.text)
                        {
                            scan.found.push(name.text.clone());
                        }
                    }
                    StatementKind::Expression(expr) => scan.expr(expr),
                    StatementKind::Error => {}
                }
            }
            if let Some(tail) = &block.tail {
                scan.expr(tail);
            }
        });
    }

    fn expr(&mut self, expr: &ast::Expr) {
        match &expr.kind {
            // Closure bodies are logical code, checked independently; their
            // local updates never become an enclosing runtime loop's state.
            ExprKind::Closure { .. } => {}
            ExprKind::Logic(block)
            | ExprKind::Block(block)
            | ExprKind::Loop { body: block, .. } => self.block(block),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expr(condition);
                self.block(then_branch);
                self.expr(else_branch);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.scoped(|scan| {
                        scan.declare(&arm.pattern);
                        scan.expr(&arm.body);
                    });
                }
            }
            ExprKind::While {
                pattern,
                condition,
                body,
            } => self.scoped(|scan| {
                scan.expr(condition);
                pattern.iter().for_each(|pattern| scan.declare(pattern));
                scan.block(body);
            }),
            ExprKind::For {
                pattern,
                iterable,
                body,
            } => {
                self.expr(iterable);
                self.scoped(|scan| {
                    scan.declare(pattern);
                    scan.block(body);
                });
            }
            // A place lent by `&mut` is assigned by the call: a write to
            // its root.
            ExprKind::Ref {
                mutable: true,
                expr: inner,
            } => {
                let mut root = &**inner;
                while let ExprKind::Member { value, .. }
                | ExprKind::Index { value, .. }
                | ExprKind::Subscript { value, .. }
                | ExprKind::Group(value) = &root.kind
                {
                    root = value;
                }
                if let ExprKind::Name(name) = &root.kind
                    && !self.declared(&name.text)
                    && !self.found.contains(&name.text)
                {
                    self.found.push(name.text.clone());
                }
                self.expr(inner);
            }
            ExprKind::Group(inner)
            | ExprKind::Not(inner)
            | ExprKind::Unary { expr: inner, .. }
            | ExprKind::Ref { expr: inner, .. }
            | ExprKind::Cast { expr: inner, .. }
            | ExprKind::Member { value: inner, .. }
            | ExprKind::Index { value: inner, .. } => self.expr(inner),
            ExprKind::Break(inner) | ExprKind::Return(inner) => {
                inner.iter().for_each(|inner| self.expr(inner));
            }
            ExprKind::Array(items)
            | ExprKind::Tuple(items)
            | ExprKind::Form {
                arguments: items, ..
            } => {
                items.iter().for_each(|item| self.expr(item));
            }
            ExprKind::Struct { fields, .. } => {
                fields.iter().for_each(|field| self.expr(&field.value));
            }
            ExprKind::GenericApply { callee, .. } => self.expr(callee),
            ExprKind::Evidence {
                constructor: lower,
                evidence: upper,
                ..
            }
            | ExprKind::Subscript {
                value: lower,
                index: upper,
            }
            | ExprKind::Range { lower, upper, .. }
            | ExprKind::Binary {
                left: lower,
                right: upper,
                ..
            } => {
                self.expr(lower);
                self.expr(upper);
            }
            ExprKind::Call { callee, arguments } => {
                self.expr(callee);
                arguments.iter().for_each(|argument| self.expr(argument));
            }
            // A formula runs nothing and assigns nothing.
            ExprKind::Forall { .. }
            | ExprKind::Exists { .. }
            | ExprKind::Name(_)
            | ExprKind::Path(_)
            | ExprKind::Integer(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Unit
            | ExprKind::Hole
            | ExprKind::Continue
            | ExprKind::Error => {}
        }
    }
}

/// What a loop's elaboration leaves for the tree: the slots of the carried
/// bindings, the versions its body saw, its body, and its target with the
/// exits recorded in it.
struct Elaborated<T> {
    slots: Vec<usize>,
    state: Vec<Binder>,
    inside: T,
    target: LoopTarget,
}

impl Env<'_> {
    /// Brings the bindings a loop carries to fresh versions for its body,
    /// in declaration order, and declares them in the mirrored context,
    /// where they stand for the abstract state the checker will declare.
    fn enter_loop(
        &mut self,
        body: &ast::Block,
        condition: Option<&ast::Expr>,
        mut target: LoopTarget,
        span: Span,
    ) -> Elab<(Vec<usize>, Vec<Binder>)> {
        let mut slots: Vec<usize> = assigned_outside(body, condition)
            .iter()
            .filter_map(|name| self.names.iter().rposition(|local| local.name == *name))
            .filter(|&slot| self.names[slot].binding.is_some() && !self.names[slot].poisoned)
            .collect();
        slots.sort_unstable();
        slots.dedup();
        // Evidence that is stale on entry is not carried: the versions the
        // body makes of it stay in the body, and it is stale after the
        // loop. Lowering allows this for a binding of proof type alone; any
        // other binding is carried, and the entry supplies the state at the
        // current versions, so tracked evidence among them must be valid.
        slots.retain(|&slot| {
            let local = &self.names[slot];
            !matches!(local.ty, Type::Proof(_))
                || local
                    .tracked
                    .as_ref()
                    .is_none_or(|tracked| tracked.stale.is_none())
        });
        self.require_valid(&slots, "before the loop, which carries it", target.head)?;
        let mut state = Vec::new();
        for &slot in &slots {
            // Typed over the versions the body sees of what it mentions,
            // which are in place by now: the slots are in declaration order.
            let inside = Binder {
                id: VarId::fresh(),
                name: self.names[slot].name.clone(),
                ty: self.version_type(slot),
                ghost: false,
            };
            self.session.register_binding_layout(
                inside.id,
                self.session.binding_layout(self.names[slot].id),
            );
            let declared = self.ctx.declare_with(inside.id, inside.ty.clone(), false);
            self.kernel(declared, span)?;
            self.names[slot].id = inside.id;
            self.labels.insert(inside.id, inside.name.clone());
            self.learn_from(&Term::var(inside.id), &inside.ty);
            state.push(inside);
        }
        self.assigned_by_loop(&slots, target.head);
        target.carried = slots.clone();
        self.loops.push(target);
        Ok((slots, state))
    }

    /// Tracked evidence the loop does not carry, but which speaks of a
    /// binding it does, is stale from the loop on: inside the body, where
    /// that binding is at a version the body sees, and after the loop.
    fn assigned_by_loop(&mut self, carried: &[usize], head: Span) {
        let bindings: Vec<(VarId, String)> = carried
            .iter()
            .map(|&slot| {
                let local = &self.names[slot];
                (
                    local.binding.expect("carried bindings are mutable"),
                    local.name.clone(),
                )
            })
            .collect();
        for (slot, local) in self.names.iter_mut().enumerate() {
            if carried.contains(&slot) {
                continue;
            }
            let Some(tracked) = &mut local.tracked else {
                continue;
            };
            // Already stale: the assignment that made it so is the one to
            // name.
            if tracked.stale.is_some() {
                continue;
            }
            if let Some((_, dep)) = bindings.iter().find(|(binding, _)| {
                tracked
                    .deps
                    .iter()
                    .any(|(mentioned, _)| mentioned == binding)
            }) {
                tracked.stale = Some(Stale {
                    dep: dep.clone(),
                    span: head,
                    by_loop: true,
                    lent: false,
                });
            }
        }
    }

    /// Elaborates a loop's inside under its own scope: `inside` runs after
    /// the carried bindings are at their fresh versions and the target is
    /// in place, and whatever it bound or assumed ends with the loop.
    fn in_loop<T>(
        &mut self,
        body: &ast::Block,
        condition: Option<&ast::Expr>,
        target: LoopTarget,
        span: Span,
        inside: impl FnOnce(&mut Self) -> Elab<T>,
    ) -> Elab<Elaborated<T>> {
        let mark = self.mark();
        let elaborated = (|| {
            let (slots, state) = self.enter_loop(body, condition, target, span)?;
            let inside = inside(self);
            let target = self.loops.pop().expect("the target pushed at entry");
            Ok(Elaborated {
                slots,
                state,
                inside: inside?,
                target,
            })
        })();
        self.close(mark);
        // The versions before the loop; the join brings in those after it.
        if let Ok(elaborated) = &elaborated {
            self.restore_versions(&elaborated.target.entry);
        }
        elaborated
    }

    /// After a loop: the carried bindings at their versions after it, and
    /// for a `loop` its value, joined from the exits as a branch's arms
    /// are. Returns what the tree carries and the loop's type.
    fn leave_loop(
        &mut self,
        entry: &Entry,
        elaborated: &Elaborated<impl Sized>,
        fallback: Type,
        value: Option<(VarId, HypId)>,
        span: Span,
    ) -> Elab<(Carried, Type)> {
        let assigned = entry.positions(&elaborated.slots);
        let value = JoinValue {
            fallback,
            bound: value,
        };
        let (tuple, joins, ty) = self.join_over(
            entry,
            &assigned,
            &elaborated.target.exits,
            value,
            "before the loop is left",
            span,
        )?;
        self.leave_loop_moves(&elaborated.target.moves);
        self.assigned_by_loop(&elaborated.slots, elaborated.target.head);
        Ok((Carried { tuple, joins }, ty))
    }

    /// The slots of the bindings the innermost loop carries.
    fn carried_slots(&self) -> Vec<usize> {
        self.loops
            .last()
            .map(|target| target.carried.clone())
            .unwrap_or_default()
    }

    /// The keyword a loop starts with, for a message.
    fn head_span(&self, span: Span) -> Span {
        let text = self.text(span);
        let keyword = ["loop", "while", "for"]
            .into_iter()
            .find(|keyword| text.starts_with(keyword))
            .map_or(0, str::len);
        Span::new(span.file, span.start, span.start + keyword)
    }

    /// The body of a loop, which produces no value: it ends in `()`, or in
    /// a transfer of control.
    fn loop_body(&mut self, body: &ast::Block) -> Elab<typed::Block> {
        let (block, _, never) = self.block(body, Some(&unit_type()))?;
        // The end of the body is a back edge (`moves.rs`), and starts the
        // next pass with the current versions, as a `continue` does.
        if !never {
            self.back_edge();
            let end = Span::new(
                body.span.file,
                body.span.end.saturating_sub(1),
                body.span.end,
            );
            self.require_valid(&self.carried_slots(), "before the loop repeats", end)?;
        }
        Ok(block)
    }

    /// `loop { body }`. Its value is what `break` supplies, checked against
    /// the type expected of the loop when there is one; a loop that never
    /// breaks produces none.
    pub(super) fn loop_(
        &mut self,
        body: &ast::Block,
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let entry = self.mutable_entry();
        let target = LoopTarget {
            result: expected.cloned(),
            layout: self.layout_hints.get(&span).cloned(),
            valued: true,
            entry: entry.clone(),
            exits: Vec::new(),
            moves: self.loop_moves(),
            carried: Vec::new(),
            head: self.head_span(span),
        };
        let elaborated = self.in_loop(body, None, target, span, |env| env.loop_body(body))?;
        let never = elaborated.target.exits.is_empty();
        let (result, equation) = (VarId::fresh(), HypId::fresh());
        self.session
            .register_binding_layout(result, elaborated.target.layout.clone().unwrap_or_default());
        let fallback = expected.map_or_else(unit_type, |expected| {
            self.at_current_exit(expected).into_owned()
        });
        let (carried, ty) = self.leave_loop(
            &entry,
            &elaborated,
            fallback,
            Some((result, equation)),
            span,
        )?;
        Ok(Value {
            expr: Expr::Loop {
                state: elaborated.state,
                carried,
                ty: ty.clone(),
                result,
                equation,
                body: elaborated.inside,
            },
            ty,
            never,
        })
    }

    /// `while condition { body }`. The condition is elaborated inside the
    /// loop, at the versions each pass starts with, and the body under the
    /// fact that it held; its `false` is an exit at the versions the
    /// condition left. Nothing of the exit test is known after the loop
    /// yet: carrying it out needs the evidence of M4.
    pub(super) fn while_(
        &mut self,
        condition: &ast::Expr,
        body: &ast::Block,
        span: Span,
    ) -> Elab<Value> {
        let entry = self.mutable_entry();
        let target = LoopTarget {
            result: None,
            layout: None,
            valued: false,
            entry: entry.clone(),
            exits: Vec::new(),
            moves: self.loop_moves(),
            carried: Vec::new(),
            head: self.head_span(span),
        };
        let elaborated = self.in_loop(body, Some(condition), target, span, |env| {
            let condition_value = env.check(condition, &Type::Bool)?;
            if !env.total
                && env
                    .session
                    .expression_layout(&condition_value.expr)
                    .is_logical()
            {
                return env.fail(
                    "L0272",
                    "a runtime while requires bool; logical Bool cannot choose runtime behavior",
                    condition.span,
                );
            }
            let (tested, negated) = env.tested(&condition_value, condition.span)?;
            let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
            env.loop_exit();
            // The exit supplies the state at the versions the condition left.
            env.require_valid(
                &env.carried_slots(),
                "before the loop is left",
                condition.span,
            )?;
            let exit = env.arm_end(&entry, unit_type(), false);
            env.loops
                .last_mut()
                .expect("inside the loop")
                .exits
                .push(exit);
            let mark = env.mark();
            let holds = Term::eq(Type::Bool, tested, Term::Bool(!negated));
            let block = env
                .assume(then_fact, holds, condition.span)
                .and_then(|()| env.loop_body(body));
            env.close(mark);
            Ok((condition_value.expr, then_fact, else_fact, block?))
        })?;
        let (carried, _) = self.leave_loop(&entry, &elaborated, unit_type(), None, span)?;
        let (condition, then_fact, else_fact, body) = elaborated.inside;
        Ok(Value::new(
            Expr::While {
                condition: Box::new(condition),
                then_fact,
                else_fact,
                state: elaborated.state,
                carried,
                body,
            },
            unit_type(),
        ))
    }

    /// `for index in lo..hi { body }`, or `lo..=hi`. The bounds have one
    /// machine type, the index's, and are evaluated before the loop; the
    /// body knows `lo <= index` and `index < hi`, or `index <= hi`, afresh
    /// on each pass, and nothing is asked about the order of the bounds,
    /// since an empty range runs no pass.
    pub(super) fn for_(
        &mut self,
        index: &ast::Name,
        kind: RangeKind,
        lower: &ast::Expr,
        upper: &ast::Expr,
        body: &ast::Block,
        span: Span,
    ) -> Elab<Value> {
        // The bounds have one machine type; a literal takes the other's.
        let (lo, hi) = if untyped_literal(lower) && !untyped_literal(upper) {
            let hi = self.infer(upper)?;
            let lo = self.check(lower, &hi.ty.clone())?;
            (lo, hi)
        } else {
            let lo = self.infer(lower)?;
            let hi = self.check(upper, &lo.ty.clone())?;
            (lo, hi)
        };
        let Some(ty) = lo.ty.as_machine() else {
            let shown = self.show_type(&lo.ty);
            return self.fail(
                "L0220",
                format!("the bounds of a `for` are machine integers, and this is `{shown}`"),
                lower.span,
            );
        };
        let lo_term = self.term(&lo, lower.span)?;
        let hi_term = self.term(&hi, upper.span)?;
        let view = |x: &Term| Term::view(ty, x.clone());
        let index = Binder {
            id: VarId::fresh(),
            name: index.text.clone(),
            ty: Type::machine(ty),
            ghost: false,
        };
        let (lower_fact, upper_fact) = (HypId::fresh(), HypId::fresh());
        let inclusive = kind == RangeKind::Inclusive;

        let entry = self.mutable_entry();
        let target = LoopTarget {
            result: None,
            layout: None,
            valued: false,
            entry: entry.clone(),
            exits: Vec::new(),
            moves: self.loop_moves(),
            carried: Vec::new(),
            head: self.head_span(span),
        };
        let elaborated = self.in_loop(body, None, target, span, |env| {
            // An empty range leaves the loop at entry (`moves.rs`).
            env.loop_exit();
            env.declare(&index, false, span)?;
            env.assume(
                lower_fact,
                Term::int_le(view(&lo_term), view(&index.term())),
                span,
            )?;
            let below = if inclusive {
                Term::int_le(view(&index.term()), view(&hi_term))
            } else {
                Term::int_lt(view(&index.term()), view(&hi_term))
            };
            env.assume(upper_fact, below, span)?;
            env.loop_body(body)
        })?;
        let (carried, _) = self.leave_loop(&entry, &elaborated, unit_type(), None, span)?;
        Ok(Value::new(
            Expr::For {
                index,
                lower: lower_fact,
                upper: upper_fact,
                lo: Box::new(lo.expr),
                hi: Box::new(hi.expr),
                inclusive,
                state: elaborated.state,
                carried,
                body: elaborated.inside,
            },
            unit_type(),
        ))
    }

    /// `break`, or `break value` in a `loop`. The first `break value` of a
    /// loop with no expected type fixes the loop's type, unless that type
    /// speaks of a version made inside the loop, which later breaks may not
    /// share; then each break is inferred on its own and the join reconciles
    /// them over the versions after the loop.
    pub(super) fn break_(&mut self, expr: &ast::Expr, value: Option<&ast::Expr>) -> Elab<Value> {
        let Some(target) = self.loops.last() else {
            return self.fail("L0217", "`break` outside a loop", expr.span);
        };
        let (valued, result, layout) =
            (target.valued, target.result.clone(), target.layout.clone());
        if let (Some(value), Some(layout)) = (value, &layout) {
            self.expect_layout(value, layout);
        }
        let value = match value {
            None => None,
            Some(_) if !valued => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "L0218",
                        "`break` with a value leaves a `while` or a `for`, which produce no value",
                        expr.span,
                    )
                    .note("only a `loop` has the value of its `break`; assign the value to a `let mut` declared before the loop and `break`"),
                );
                return Err(());
            }
            Some(value) => Some(match &result {
                Some(ty) => self.check(value, ty)?,
                None => self.infer(value)?,
            }),
        };
        let ty = value
            .as_ref()
            .map_or_else(unit_type, |value| value.ty.clone());
        if let (None, Some(result)) = (&value, &result)
            && !crate::kernel::same_type(result, &ty)
        {
            let shown = self.show_type(result);
            return self.fail(
                "L0220",
                format!("this `break` carries no value, and the loop produces `{shown}`"),
                expr.span,
            );
        }
        // The break supplies the loop's state at the current versions.
        self.require_valid(&self.carried_slots(), "before the loop is left", expr.span)?;
        let exit = {
            let target = self.loops.last().expect("checked above");
            self.arm_end(&target.entry, ty, false)
        };
        self.loop_exit();
        let value_layout = value
            .as_ref()
            .map(|value| self.session.expression_layout(&value.expr));
        let target = self.loops.last_mut().expect("checked above");
        if target.layout.is_none() {
            target.layout = value_layout;
        }
        if target.result.is_none()
            && !Env::mentions_arm_version(&target.entry, &exit.versions, &exit.ty)
        {
            target.result = Some(exit.ty.clone());
        }
        target.exits.push(exit);
        Ok(Value {
            expr: Expr::Break(value.map(|value| Box::new(value.expr))),
            ty: unit_type(),
            never: true,
        })
    }

    pub(super) fn continue_(&mut self, expr: &ast::Expr) -> Elab<Value> {
        if self.loops.is_empty() {
            return self.fail("L0217", "`continue` outside a loop", expr.span);
        }
        // A `continue` is a back edge (`moves.rs`).
        self.back_edge();
        // The next pass starts with the current versions.
        self.require_valid(&self.carried_slots(), "before the loop repeats", expr.span)?;
        Ok(Value {
            expr: Expr::Continue,
            ty: unit_type(),
            never: true,
        })
    }
}
