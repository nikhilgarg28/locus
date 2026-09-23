//! Branching: `if` and `match` on an enum. Each branch is elaborated under
//! the fact that it was the one taken. And `return`, which leaves every
//! branch and loop at once.

use crate::ast::{self, ExprKind, PatternKind};
use crate::diagnostic::{Diagnostic, Label};
use crate::kernel::{
    HypId, Term, Type, VarId, case_variants, same_type, telescope_entry, variant_term,
};
use crate::source::Span;
use crate::typed::{self, Binder, CompareOp, Expr, MatchArm, block_leaves, is_pure};

use super::env::{Elab, Env};
use super::exprs::{Value, unit_type};
use super::mutation::ArmEnd;

pub(super) enum Branch<'a> {
    Block(&'a ast::Block),
    Expr(&'a ast::Expr),
}

impl Env<'_> {
    pub fn branch(
        &mut self,
        branch: &Branch<'_>,
        expected: Option<&Type>,
    ) -> Elab<(typed::Block, Type, bool)> {
        match branch {
            Branch::Block(block) => self.block(block, expected),
            Branch::Expr(ast::Expr {
                kind: ExprKind::Block(block),
                ..
            }) => self.block(block, expected),
            Branch::Expr(expr) => {
                let value = match expected {
                    Some(expected) => self.check(expr, expected)?,
                    None => self.infer(expr)?,
                };
                Ok((
                    typed::Block {
                        stmts: Vec::new(),
                        tail: Some(Box::new(value.expr)),
                    },
                    value.ty,
                    value.never,
                ))
            }
        }
    }

    /// The comparison a condition performs, as a term, and whether the
    /// condition is its negation. Branch facts speak of the comparison
    /// performed: `a != b` tests `a == b` and exchanges the branches.
    pub(super) fn tested(&mut self, condition: &Value, span: Span) -> Elab<(Term, bool)> {
        let (tested, negated) = match &condition.expr {
            Expr::Compare {
                op: CompareOp::Ne,
                ty,
                left,
                right,
            } => (
                Expr::Compare {
                    op: CompareOp::Eq,
                    ty: ty.clone(),
                    left: left.clone(),
                    right: right.clone(),
                },
                true,
            ),
            other => (other.clone(), false),
        };
        let tested = self.term(&Value::new(tested, Type::Bool), span)?;
        Ok((tested, negated))
    }

    pub(super) fn conditional(
        &mut self,
        condition: &ast::Expr,
        then: Branch<'_>,
        otherwise: Branch<'_>,
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let condition_value = self.check(condition, &Type::Bool)?;
        if !self.total && super::reconcile::is_logical_expr(&condition_value.expr) {
            return self.fail(
                "L0272",
                "a runtime if requires bool; logical Bool cannot choose runtime behavior",
                condition.span,
            );
        }
        let (tested, negated) = self.tested(&condition_value, condition.span)?;
        let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
        let fact = |holds: bool| Term::eq(Type::Bool, tested.clone(), Term::Bool(holds != negated));

        // Each arm works on the versions of the mutable bindings it finds
        // and leaves them as it found them; the join carries what it
        // assigned to the code after the branch (`mutation.rs`).
        let entry = self.mutable_entry();
        // What each arm moves is joined the same way (`moves.rs`).
        let moves_entry = self.moves_now();
        let mark = self.mark();
        let result_scope = self.result_scope();
        let then_result = self
            .assume(then_fact, fact(true), condition.span)
            .and_then(|()| self.branch(&then, expected));
        let then_result = self.check_scope_result(&result_scope, then_result, span);
        let then_versions = self.versions_now(&entry);
        let then_moves = self.moves_now();
        let then_stale = self.stale_now(&entry);
        self.close(mark);
        self.restore_versions(&entry);
        self.restore_moves(&moves_entry);
        let (then_block, then_ty, then_never) = then_result?;

        let expected_else = match expected {
            Some(expected) => Some(expected.clone()),
            None if !then_never && !Env::mentions_arm_version(&entry, &then_versions, &then_ty) => {
                Some(then_ty.clone())
            }
            None => None,
        };
        let mark = self.mark();
        let else_result = self
            .assume(else_fact, fact(false), condition.span)
            .and_then(|()| self.branch(&otherwise, expected_else.as_ref()));
        let else_result = self.check_scope_result(&result_scope, else_result, span);
        let else_versions = self.versions_now(&entry);
        let else_moves = self.moves_now();
        let else_stale = self.stale_now(&entry);
        self.close(mark);
        self.restore_versions(&entry);
        let (else_block, else_ty, else_never) = else_result?;
        self.join_moves(
            &moves_entry,
            &[(then_moves, then_never), (else_moves, else_never)],
        );

        let never = then_never && else_never;
        let ty = match expected {
            Some(expected) => self.at_current_exit(expected).into_owned(),
            None if !then_never => then_ty.clone(),
            None => else_ty.clone(),
        };
        let result = VarId::fresh();
        let arms = [
            ArmEnd {
                versions: then_versions,
                stale: then_stale,
                ty: then_ty,
                never: then_never,
                leaves: block_leaves(&then_block),
            },
            ArmEnd {
                versions: else_versions,
                stale: else_stale,
                ty: else_ty,
                never: else_never,
                leaves: block_leaves(&else_block),
            },
        ];
        let (joined, ty) = self.join(&entry, &arms, ty, result, span)?;
        let expr = Expr::If {
            condition: Box::new(condition_value.expr),
            then_fact,
            else_fact,
            then_block,
            else_block,
            ty: ty.clone(),
            result,
            joined,
        };
        if !never
            && !is_pure(&expr)
            && !matches!(
                expr,
                Expr::If {
                    joined: Some(_),
                    ..
                }
            )
        {
            self.declare_result(result, &ty, span)?;
        }
        Ok(Value { expr, ty, never })
    }

    pub(super) fn match_(
        &mut self,
        scrutinee: &ast::Expr,
        arms: &[ast::MatchArm],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        // A matched place is read here; an arm whose pattern binds a part
        // that is not `Copy` moves it (`moves.rs`).
        self.mark_place_root(scrutinee);
        let scrutinee_value = self.infer(scrutinee)?;
        let place = self.place_taken(&scrutinee_value, scrutinee.span);
        if matches!(scrutinee_value.ty, Type::Proof(_)) {
            return self.match_evidence(scrutinee_value, scrutinee.span, arms, expected, span);
        }
        let Type::Enum(id) = &scrutinee_value.ty else {
            let shown = self.show_type(&scrutinee_value.ty);
            let message = format!("`match` takes apart an enum or evidence, and this is `{shown}`");
            return self.fail("L0212", message, scrutinee.span);
        };
        let info = self.enum_by_id(*id).expect("an enum type was declared");
        let scrutinee_term = self.term(&scrutinee_value, scrutinee.span)?;
        let payload_types =
            case_variants(&self.ctx, &scrutinee_value.ty).expect("an enum has variants");

        // Which source arm handles each variant.
        let mut chosen: Vec<Option<&ast::MatchArm>> = vec![None; info.variants.len()];
        for arm in arms {
            match &arm.pattern.kind {
                PatternKind::Wildcard => {
                    if chosen.iter().all(Option::is_some) {
                        return self.fail("L0214", "this arm is never reached", arm.pattern.span);
                    }
                    for slot in chosen.iter_mut().filter(|slot| slot.is_none()) {
                        *slot = Some(arm);
                    }
                }
                PatternKind::Variant { path, .. } | PatternKind::Struct { path, .. } => {
                    let (prefix, _) = self.variant_path(path)?;
                    if self.type_text(prefix) != info.name {
                        let message = format!(
                            "this arm is for `{}`, and the value matched is a `{}`",
                            prefix.text, info.name
                        );
                        return self.fail("L0212", message, path.span);
                    }
                    let Some((_, index)) = self.enum_variant(path)? else {
                        return Err(());
                    };
                    if chosen[index].is_some() {
                        return self.fail("L0214", "this arm is never reached", arm.pattern.span);
                    }
                    chosen[index] = Some(arm);
                }
                _ => {
                    return self.fail(
                        "L0290",
                        "an arm's pattern is `Enum::Variant(names...)`, `Enum::Variant { fields... }`, or `_`; other patterns are not supported yet",
                        arm.pattern.span,
                    );
                }
            }
        }
        let missing: Vec<String> = chosen
            .iter()
            .zip(&info.variants)
            .filter(|(arm, _)| arm.is_none())
            .map(|(_, variant)| format!("`{}::{}`", info.name, variant.name))
            .collect();
        if !missing.is_empty() {
            return self.fail(
                "L0213",
                format!("no arm handles {}", missing.join(", ")),
                span,
            );
        }

        let mut typed_arms = Vec::new();
        let mut ty: Option<Type> =
            expected.map(|expected| self.at_current_exit(expected).into_owned());
        let mut never = true;
        let entry = self.mutable_entry();
        let moves_entry = self.moves_now();
        let mut arm_moves = Vec::new();
        let mut ends = Vec::new();
        for (index, arm) in chosen.iter().enumerate() {
            let arm = arm.expect("every variant has an arm");
            let variant = &info.variants[index];
            let variant_name = &variant.name;
            let mark = self.mark();
            let result_scope = self.result_scope();
            let arm_result = (|| {
                let what = format!("`{}::{variant_name}`", info.name);
                let names =
                    self.pattern_names(&arm.pattern, &what, &variant.payload, variant.named)?;
                let mut payload: Vec<Binder> = Vec::new();
                let telescope = Type::Tuple(payload_types[index].clone());
                for (field, name) in names.iter().enumerate() {
                    let earlier: Vec<Term> = payload.iter().map(Binder::term).collect();
                    let field_ty = telescope_entry(&telescope, field, &earlier)
                        .expect("the payload has this field");
                    let binder = Binder {
                        id: VarId::fresh(),
                        name: name.map_or_else(|| "_".to_string(), |name| name.text.clone()),
                        ty: field_ty,
                        ghost: variant.payload[field].ghost,
                    };
                    self.session.register_binding_layout(
                        binder.id,
                        self.session.binding_layout(variant.payload[field].id),
                    );
                    let declared =
                        self.ctx
                            .declare_with(binder.id, binder.ty.clone(), binder.ghost);
                    self.kernel(declared, arm.pattern.span)?;
                    if name.is_some() {
                        self.bind(&binder.name, binder.id, &binder.ty, binder.ghost);
                    }
                    payload.push(binder);
                }
                self.move_by_arm(place.as_ref(), &payload, &names);
                let ids: Vec<VarId> = payload.iter().map(|binder| binder.id).collect();
                let fact = HypId::fresh();
                let claim = Term::eq(
                    scrutinee_value.ty.clone(),
                    scrutinee_term.clone(),
                    variant_term(&scrutinee_value.ty, index, &ids, &payload_types[index]),
                );
                self.assume(fact, claim, arm.pattern.span)?;
                // A logical pattern names exactly this constructor. Treat the
                // checked case equation as computation; normalization replays
                // every replacement through kernel equality transport.
                if self.total {
                    self.facts.last_mut().expect("just assumed").definition = true;
                }
                // An arm is checked against the type as declared, whose
                // exit binders stand for the versions current at its end.
                let body_result = self.branch(&Branch::Expr(&arm.body), expected.or(ty.as_ref()));
                let (body, body_ty, body_never) =
                    self.check_scope_result(&result_scope, body_result, arm.body.span)?;
                Ok((payload, fact, body, body_ty, body_never))
            })();
            let versions = self.versions_now(&entry);
            let moves = self.moves_now();
            let stale = self.stale_now(&entry);
            self.close(mark);
            self.restore_versions(&entry);
            self.restore_moves(&moves_entry);
            let (payload, fact, body, body_ty, body_never) = arm_result?;
            arm_moves.push((moves, body_never));
            if !body_never {
                never = false;
                if !Env::mentions_arm_version(&entry, &versions, &body_ty) {
                    ty.get_or_insert(body_ty.clone());
                }
            }
            ends.push(ArmEnd {
                versions,
                stale,
                ty: body_ty,
                never: body_never,
                leaves: block_leaves(&body),
            });
            typed_arms.push(MatchArm {
                variant_name: variant_name.clone(),
                payload,
                fact,
                body,
            });
        }
        let ty = ty.unwrap_or_else(unit_type);
        let result = VarId::fresh();
        self.join_moves(&moves_entry, &arm_moves);
        let (joined, ty) = self.join(&entry, &ends, ty, result, span)?;
        let expr = Expr::Match {
            scrutinee: Box::new(scrutinee_value.expr),
            enum_name: info.name.clone(),
            arms: typed_arms,
            ty: ty.clone(),
            result,
            joined,
        };
        if !never
            && !is_pure(&expr)
            && !matches!(
                expr,
                Expr::Match {
                    joined: Some(_),
                    ..
                }
            )
        {
            self.declare_result(result, &ty, span)?;
        }
        Ok(Value { expr, ty, never })
    }

    // --- return -----------------------------------------------------------------------

    /// `return`, or `return value`: the function ends here with the value,
    /// which is checked against its declared result type in the context of
    /// this point, exactly as the value of the body is, evidence owed
    /// included. The expression is never-typed: it produces no value and
    /// stands where any type is expected.
    pub(super) fn return_(
        &mut self,
        expr: &ast::Expr,
        value: Option<&ast::Expr>,
        expected: Option<&Type>,
    ) -> Elab<Value> {
        let keyword = Span::new(
            expr.span.file,
            expr.span.start,
            expr.span.start + "return".len(),
        );
        if let Some(place) = self.formula {
            return self.fail(
                "L0215",
                format!("`return` cannot appear in {place}: nothing there runs"),
                keyword,
            );
        }
        if self.total {
            return self.fail(
                "L0270",
                "a logic function uses its final expression instead of return",
                keyword,
            );
        }
        let Some(target) = &self.returns else {
            return self.internal("`return` outside the body of a function", keyword);
        };
        let (result_ty, never_fn, result_logical) =
            (target.result.clone(), target.never, target.logical);
        let result_layout = target.layout.clone();
        let item = self.item_name.clone();
        if never_fn {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0220",
                    format!("`return` in `{item}`, which is declared `-> !` and never returns"),
                    keyword,
                )
                .note("a function that never returns ends in a panic, a `loop` without `break`, or a call to a function declared `-> !`; give it a result type if it returns"),
            );
            return Err(());
        }
        let value = match value {
            Some(value) => {
                self.expect_layout(value, &result_layout);
                let before = self.diagnostics.len();
                match self.argument(value, &result_ty, result_logical && !result_ty.is_ghost()) {
                    Ok(value) => Some(Box::new(value.expr)),
                    Err(()) => {
                        self.owed_at_return(before, keyword);
                        return Err(());
                    }
                }
            }
            None if !same_type(&result_ty, &unit_type()) => {
                let shown = self.show_type(&result_ty);
                return self.fail(
                    "L0220",
                    format!("this `return` carries no value, and `{item}` returns `{shown}`"),
                    keyword,
                );
            }
            None => None,
        };
        let ty = expected.cloned().unwrap_or_else(unit_type);
        let result = VarId::fresh();
        self.declare_result(result, &ty, expr.span)?;
        Ok(Value {
            expr: Expr::Return {
                value,
                ty: ty.clone(),
                result,
            },
            ty,
            never: true,
        })
    }

    /// Evidence owed at an early return that could not be shown: the hole's
    /// diagnostic, as usual, is reported at the `return`, and the hole is
    /// where the evidence is owed.
    fn owed_at_return(&mut self, before: usize, keyword: Span) {
        for diagnostic in &mut self.diagnostics[before..] {
            if diagnostic.code != "L0230" {
                continue;
            }
            let Some(primary) = diagnostic.labels.iter_mut().find(|label| label.primary) else {
                continue;
            };
            let hole = std::mem::replace(&mut primary.span, keyword);
            primary.message = "at this early return".into();
            diagnostic.labels.push(Label {
                span: hole,
                message: "the evidence owed here".into(),
                primary: false,
            });
        }
    }
}
