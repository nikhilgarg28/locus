//! Branching: `if` and `match` on an enum. Each branch is elaborated under
//! the fact that it was the one taken.

use crate::ast::{self, ExprKind, PatternKind};
use crate::kernel::{HypId, Term, Type, VarId, case_variants, telescope_entry, variant_term};
use crate::source::Span;
use crate::typed::{self, Binder, CompareOp, Expr, MatchArm, is_pure};

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

    pub(super) fn conditional(
        &mut self,
        condition: &ast::Expr,
        then: Branch<'_>,
        otherwise: Branch<'_>,
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let condition_value = self.check(condition, &Type::Bool)?;
        // Branch facts speak of the comparison performed: `a != b` tests
        // `a == b` and exchanges the branches.
        let (tested, negated) = match &condition_value.expr {
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
        let tested = self.term(&Value::new(tested, Type::Bool), condition.span)?;
        let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
        let fact = |holds: bool| Term::eq(Type::Bool, tested.clone(), Term::Bool(holds != negated));

        // Each arm works on the versions of the mutable bindings it finds
        // and leaves them as it found them; the join carries what it
        // assigned to the code after the branch (`mutation.rs`).
        let entry = self.mutable_entry();
        let mark = self.mark();
        let then_result = self
            .assume(then_fact, fact(true), condition.span)
            .and_then(|()| self.branch(&then, expected));
        let then_versions = self.versions_now(&entry);
        self.close(mark);
        self.restore_versions(&entry);
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
        let else_versions = self.versions_now(&entry);
        self.close(mark);
        self.restore_versions(&entry);
        let (else_block, else_ty, else_never) = else_result?;

        let never = then_never && else_never;
        let ty = match expected {
            Some(expected) => expected.clone(),
            None if !then_never => then_ty.clone(),
            None => else_ty.clone(),
        };
        let result = VarId::fresh();
        let arms = [
            ArmEnd {
                versions: then_versions,
                ty: then_ty,
                never: then_never,
            },
            ArmEnd {
                versions: else_versions,
                ty: else_ty,
                never: else_never,
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
        let scrutinee_value = self.infer(scrutinee)?;
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
                    if prefix.text != info.name {
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
        let mut ty: Option<Type> = expected.cloned();
        let mut never = true;
        let entry = self.mutable_entry();
        let mut ends = Vec::new();
        for (index, arm) in chosen.iter().enumerate() {
            let arm = arm.expect("every variant has an arm");
            let variant = &info.variants[index];
            let variant_name = &variant.name;
            let mark = self.mark();
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
                    };
                    let declared = self.ctx.declare_with(binder.id, binder.ty.clone(), false);
                    self.kernel(declared, arm.pattern.span)?;
                    if name.is_some() {
                        self.bind(&binder.name, binder.id, &binder.ty);
                    }
                    payload.push(binder);
                }
                let ids: Vec<VarId> = payload.iter().map(|binder| binder.id).collect();
                let fact = HypId::fresh();
                let claim = Term::eq(
                    scrutinee_value.ty.clone(),
                    scrutinee_term.clone(),
                    variant_term(&scrutinee_value.ty, index, &ids, &payload_types[index]),
                );
                self.assume(fact, claim, arm.pattern.span)?;
                let (body, body_ty, body_never) =
                    self.branch(&Branch::Expr(&arm.body), ty.as_ref())?;
                Ok((payload, fact, body, body_ty, body_never))
            })();
            let versions = self.versions_now(&entry);
            self.close(mark);
            self.restore_versions(&entry);
            let (payload, fact, body, body_ty, body_never) = arm_result?;
            if !body_never {
                never = false;
                if !Env::mentions_arm_version(&entry, &versions, &body_ty) {
                    ty.get_or_insert(body_ty.clone());
                }
            }
            ends.push(ArmEnd {
                versions,
                ty: body_ty,
                never: body_never,
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
}
