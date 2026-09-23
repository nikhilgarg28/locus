//! Evidence written out: proof constructors, `match` on evidence, the
//! `rewrite!`, `unfold!` and `fold!` forms, and applying evidence of a `forall`
//! or an implication. Each becomes an explicit kernel proof.

use crate::ast::{self, ExprKind, Form, PatternKind};
use crate::diagnostic::{Applicability, Diagnostic, Suggestion};
use crate::kernel::derive;
use crate::kernel::{
    Axiom, HypId, KernelError, Prim, Proof, Term, Type, VarId, check_proof, infer_proof,
};
use crate::source::Span;
use crate::typed::{self, Binder, Expr, FnRef, is_pure, value_term};

use super::control::Branch;
use super::env::{Elab, Env, FnInfo, Global, PropInfo, substitute};
use super::exprs::Value;
use super::solve::forward;

impl Env<'_> {
    /// The kernel proof an evidence-typed value stands for.
    pub fn proof_of(&mut self, value: &Value, span: Span) -> Elab<Proof> {
        Ok(match self.term(value, span)? {
            Term::Proof(proof) => *proof,
            other => Proof::OfTerm(other),
        })
    }

    /// A value from a proof, typed by what the kernel says it proves.
    pub fn proved(&mut self, proof: Proof, span: Span) -> Elab<Value> {
        let claim = infer_proof(&mut self.ctx, &proof);
        let claim = self.kernel(claim, span)?;
        Ok(Value::new(Expr::Proof(proof), Type::proof(claim)))
    }

    /// The declared proposition a claim applies, and its arguments.
    fn applied_prop(&mut self, claim: &Term) -> Option<(std::rc::Rc<PropInfo>, Vec<Term>)> {
        let claim = match claim {
            Term::PropApp(..) => claim.clone(),
            other => self.computed(other),
        };
        match claim {
            Term::PropApp(id, arguments) => Some((self.prop_by_id(id)?, arguments)),
            _ => None,
        }
    }

    /// The index of the variant `Name::Variant` names.
    pub(super) fn prop_variant(&mut self, info: &PropInfo, path: &ast::Path) -> Elab<usize> {
        let name = path.last();
        match info
            .variants
            .iter()
            .position(|variant| variant.name == name.text)
        {
            Some(index) => Ok(index),
            None => {
                let message = format!("`{}` has no variant `{}`", info.name, name.text);
                self.fail("L0212", message, name.span)
            }
        }
    }

    /// Construct a named arm: witnesses and the evidence slot are distinct.
    /// Arguments are evaluated in source order in the surrounding mode;
    /// erasing the proof never erases an ordinary argument-producing call.
    pub fn named_evidence(
        &mut self,
        constructor: &ast::Expr,
        evidence: &ast::Expr,
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        self.require_preview(
            crate::preview::Feature::NamedProps,
            "named-arm evidence construction",
            span,
        )?;
        let (path, tuple, named) = match &constructor.kind {
            ExprKind::Path(path) => (&**path, Some(&[][..]), None),
            ExprKind::Call { callee, arguments } => match &callee.kind {
                ExprKind::Path(path) => (&**path, Some(arguments.as_slice()), None),
                _ => {
                    return self.fail(
                        "L0275",
                        "the left of `@` is a proposition constructor such as `Pred::Arm(...)`",
                        constructor.span,
                    );
                }
            },
            ExprKind::Struct { path, fields } => (path, None, Some(fields.as_slice())),
            ExprKind::Group(inner) => return self.named_evidence(inner, evidence, expected, span),
            _ => {
                return self.fail(
                    "L0275",
                    "the left of `@` is a proposition constructor",
                    constructor.span,
                );
            }
        };
        let key = self.type_text(&path.segments[0]);
        let Some(Global::Prop(info)) = self.types.get(&key).cloned() else {
            return self.fail(
                "L0275",
                format!("`{key}` is not a declared proposition"),
                path.span,
            );
        };
        let index = self.prop_variant(&info, path)?;
        let variant = &info.variants[index];
        let Some(body) = &variant.body else {
            return self.fail(
                "L0275",
                "this legacy constructor has no separate evidence slot",
                constructor.span,
            );
        };
        let expected = expected.map(|ty| self.at_current_exit(ty).into_owned());
        let params = match expected.as_ref() {
            Some(Type::Proof(claim)) => match self.applied_prop(claim) {
                Some((wanted, arguments)) if wanted.id == info.id => arguments,
                _ => return self.fail("L0275", format!("this constructor proves `{}`, which differs from the expected claim", info.name), span),
            },
            _ if info.params.is_empty() => Vec::new(),
            _ => return self.fail("L0226", format!("which `{}(...)` this proves is not known; annotate the evidence with its claim", info.name), span),
        };
        let witnesses = &variant.payload[..variant.payload.len() - 1];
        let what = format!("`{}::{}`", info.name, variant.name);
        self.variant_shape(
            &what,
            witnesses,
            variant.named,
            named.is_some(),
            constructor.span,
        )?;
        let arguments = match (tuple, named) {
            (Some(arguments), _) => arguments.iter().collect::<Vec<_>>(),
            (_, Some(fields)) => self.values_by_name(&what, witnesses, fields, constructor.span)?,
            _ => unreachable!(),
        };
        if arguments.len() != witnesses.len() {
            return self.fail(
                "L0275",
                format!(
                    "{what} requires {} witness(es), but {} were supplied; evidence follows `@`",
                    witnesses.len(),
                    arguments.len()
                ),
                constructor.span,
            );
        }
        let mut body = body.clone();
        let mut tys: Vec<_> = witnesses.iter().map(|witness| witness.ty.clone()).collect();
        for (param, argument) in info.params.iter().zip(&params) {
            body = body.replace_var(param.id, argument);
            for ty in &mut tys {
                *ty = ty.replace_var(param.id, argument);
            }
        }
        let mut payload = vec![None; witnesses.len()];
        let mut effects = Vec::new();
        // Named fields may be written in a different order than their
        // declaration. Evaluation follows source order; the kernel payload
        // follows declaration order.
        let mut order: Vec<_> = (0..arguments.len()).collect();
        if named.is_some() {
            order.sort_by_key(|&index| arguments[index].span.start);
        }
        for index in order {
            let (witness, argument) = (&witnesses[index], arguments[index]);
            let value = self.check(argument, &tys[index])?;
            let term = self.term(&value, argument.span)?;
            body = body.replace_var(witness.id, &term);
            for ty in &mut tys {
                *ty = ty.replace_var(witness.id, &term);
            }
            payload[index] = Some(term);
            if !is_pure(&value.expr) {
                effects.push(typed::Stmt::Expr(value.expr));
            }
        }
        let mut payload: Vec<_> = payload
            .into_iter()
            .map(|term| term.expect("every witness checked"))
            .collect();
        let value = self.check(evidence, &Type::proof(body))?;
        payload.push(self.term(&value, evidence.span)?);
        if !is_pure(&value.expr) {
            effects.push(typed::Stmt::Expr(value.expr));
        }
        let mut value = self.proved(
            Proof::Construct {
                prop: info.id,
                variant: index,
                params,
                payload,
            },
            span,
        )?;
        if !effects.is_empty() {
            value.expr = Expr::Block(typed::Block {
                stmts: effects,
                tail: Some(Box::new(value.expr)),
            });
        }
        Ok(value)
    }

    /// `Name::Variant(payload)`, or `Name::Variant { field: payload }`
    /// with the values already in the fields' order, which proves
    /// `Name(...)`.
    pub fn construct(
        &mut self,
        info: &PropInfo,
        path: &ast::Path,
        arguments: &[&ast::Expr],
        braces: bool,
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let expected = expected.map(|expected| self.at_current_exit(expected).into_owned());
        let expected = expected.as_ref();
        let index = self.prop_variant(info, path)?;
        let variant = &info.variants[index];
        let what = format!("`{}::{}`", info.name, variant.name);
        if variant.body.is_some() {
            let mut diagnostic = Diagnostic::error(
                "L0276",
                format!(
                    "{what} needs its evidence outside the witnesses: write `{} @ evidence`",
                    self.text(span)
                ),
                span,
            );
            if !braces && arguments.len() == variant.payload.len() {
                let (evidence, witnesses) = arguments
                    .split_last()
                    .expect("an arm has an evidence parameter");
                let constructor = if witnesses.is_empty() {
                    path.text()
                } else {
                    format!(
                        "{}({})",
                        path.text(),
                        witnesses
                            .iter()
                            .map(|argument| self.text(argument.span))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                diagnostic = diagnostic.suggest(Suggestion {
                    message: "move the proof to the outside evidence slot".into(),
                    span,
                    replacement: format!("{constructor} @ {}", self.text(evidence.span)),
                    applicability: Applicability::MachineApplicable,
                });
            }
            self.diagnostics.push(diagnostic);
            return Err(());
        }
        self.variant_shape(&what, &variant.payload, variant.named, braces, span)?;
        let mut tys: Vec<Type> = variant
            .payload
            .iter()
            .map(|binder| binder.ty.clone())
            .collect();
        let mut params = Vec::new();
        if variant.conclusion.is_none() && !info.params.is_empty() {
            // Which `Name(...)` is proved comes from what is expected.
            let wanted = match expected {
                Some(Type::Proof(claim)) => self.applied_prop(claim),
                _ => None,
            };
            match wanted {
                Some((wanted, arguments)) if wanted.id == info.id => params = arguments,
                _ => {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0226",
                            format!("which `{}(...)` this proves is not known here", info.name),
                            span,
                        )
                        .note("state the claim with an annotation, such as `let evidence: @(p || q) = ...;`"),
                    );
                    return Err(());
                }
            }
            for ty in tys.iter_mut() {
                for (param, argument) in info.params.iter().zip(&params) {
                    *ty = ty.replace_var(param.id, argument);
                }
            }
        }
        let ids: Vec<VarId> = variant.payload.iter().map(|binder| binder.id).collect();
        // Evidence is erased with what it holds: nothing in it runs, and
        // its values are read, not moved (`moves.rs`).
        let ghosts = vec![false; ids.len()];
        let payload = self.logical("evidence", |env| {
            env.arguments_by_ref(arguments, &ids, &mut tys, &ghosts, &what, span)
        })?;
        let mut terms = Vec::new();
        for expr in &payload {
            match value_term(expr) {
                Ok(term) => terms.push(term),
                Err(error) => return self.internal(error, span),
            }
        }
        self.proved(
            Proof::Construct {
                prop: info.id,
                variant: index,
                params: if variant.conclusion.is_none() {
                    params
                } else {
                    Vec::new()
                },
                payload: terms,
            },
            span,
        )
    }

    /// Open an irrefutable named-arm pattern without introducing a runtime
    /// branch. For a witness-free single arm, its body follows by CaseProof.
    /// Witnesses stay scoped inside explicit matches until continuation
    /// elimination is supported; no data is projected out of an erased proof.
    pub(super) fn open_named_let(
        &mut self,
        pattern: &ast::Pattern,
        value: Value,
        span: Span,
    ) -> Elab<Vec<typed::Stmt>> {
        self.require_preview(
            crate::preview::Feature::NamedProps,
            "named proposition let pattern",
            pattern.span,
        )?;
        let Type::Proof(claim) = &value.ty else {
            return self.fail("L0275", "an evidence pattern requires a proof value", span);
        };
        let Some((info, arguments)) = self.applied_prop(claim) else {
            return self.fail("L0275", "this proof has no declared proposition arms", span);
        };
        if info.variants.len() != 1 {
            return self.fail("L0278", "this evidence pattern is refutable; use a logical match covering every proposition arm", pattern.span);
        }
        let (core, wholes) = evidence_pattern_parts(pattern);
        let PatternKind::Evidence {
            constructor,
            evidence: evidence_pattern,
            ..
        } = &core.kind
        else {
            return self.fail(
                "L0276",
                "write an evidence pattern as `Pred::Arm @ evidence`",
                core.span,
            );
        };
        let path = match &constructor.kind {
            PatternKind::Variant { path, .. } | PatternKind::Struct { path, .. } => path,
            _ => {
                return self.fail(
                    "L0275",
                    "an evidence pattern starts with a proposition constructor",
                    constructor.span,
                );
            }
        };
        if path.pair().is_none_or(|(owner, _)| owner.text != info.name) {
            return self.fail(
                "L0275",
                format!(
                    "this proof is of `{}`, not this pattern's proposition",
                    info.name
                ),
                path.span,
            );
        }
        let index = self.prop_variant(&info, path)?;
        let variant = &info.variants[index];
        let Some(body) = &variant.body else {
            return self.fail(
                "L0276",
                "this legacy proposition has no separate arm body",
                pattern.span,
            );
        };
        let what = format!("`{}::{}`", info.name, variant.name);
        self.named_proof_pattern_names(core, &what, variant)?;
        if variant.payload.len() != 1 {
            return self.fail("L0278", "a let cannot extract arbitrary witnesses from erased evidence; use a logical match and keep witnesses scoped inside its proof", pattern.span);
        }
        let mut body = body.clone();
        for (parameter, argument) in info.params.iter().zip(&arguments) {
            body = body.replace_var(parameter.id, argument);
        }
        let original = self.term(&value, span)?;
        let scrutinee = self.proof_of(&value, span)?;
        let opened = self.proved(
            Proof::CaseProof {
                scrutinee: Box::new(scrutinee),
                goal: body,
                arms: vec![Proof::arm(1, 0, |values, _| {
                    Proof::OfTerm(values[0].clone())
                })],
            },
            pattern.span,
        )?;
        let mut statements = Vec::new();
        if !is_pure(&value.expr) {
            statements.push(typed::Stmt::Expr(value.expr));
        }
        for whole in wholes {
            let alias = ast::Pattern {
                kind: PatternKind::Name {
                    name: whole.clone(),
                    mutable: false,
                },
                span: whole.span,
            };
            let bound = self.bind_pattern(&alias, original.clone(), false)?;
            statements.push(typed::Stmt::Let {
                pattern: bound,
                value: Expr::Proof(Proof::OfTerm(original.clone())),
            });
        }
        let opened_term = self.term(&opened, pattern.span)?;
        let bound = self.bind_pattern(evidence_pattern, opened_term, false)?;
        statements.push(typed::Stmt::Let {
            pattern: bound,
            value: opened.expr,
        });
        Ok(statements)
    }

    /// `match evidence { Name::Variant(payload) => evidence, ... }`.
    pub fn match_evidence(
        &mut self,
        scrutinee: Value,
        scrutinee_span: Span,
        arms: &[ast::MatchArm],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let expected = expected.map(|expected| self.at_current_exit(expected).into_owned());
        let expected = expected.as_ref();
        let Type::Proof(claim) = scrutinee.ty.clone() else {
            unreachable!("the caller saw evidence")
        };
        let Some((info, arguments)) = self.applied_prop(&claim) else {
            let shown = self.show(&claim);
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0212",
                    format!("this is evidence of `{shown}`, which has no variants to match"),
                    scrutinee_span,
                )
                .note("`match` takes apart evidence of a declared proposition, of `p && q`, or of `p || q`"),
            );
            return Err(());
        };
        let evidence = self.proof_of(&scrutinee, scrutinee_span)?;
        if info.variants.is_empty() {
            // Evidence of `False`: this point is never reached.
            if let Some(arm) = arms.first() {
                return self.fail("L0214", "this arm is never reached", arm.span);
            }
            let ty = expected.cloned().unwrap_or(Type::Tuple(Vec::new()));
            let mut result = Value::new(
                Expr::Absurd {
                    proof: evidence,
                    ty: ty.clone(),
                },
                ty,
            );
            if !is_pure(&scrutinee.expr) {
                result.expr = Expr::Block(typed::Block {
                    stmts: vec![typed::Stmt::Expr(scrutinee.expr)],
                    tail: Some(Box::new(result.expr)),
                });
            }
            return Ok(result);
        }
        let Some(Type::Proof(goal)) = expected else {
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0227",
                    "evidence can be taken apart only to produce other evidence",
                    span,
                )
                .note("proofs are erased, so which one was supplied cannot choose a value; state the claim this `match` proves with an annotation"),
            );
            return Err(());
        };
        let goal = (**goal).clone();

        let mut chosen: Vec<Option<&ast::MatchArm>> = vec![None; info.variants.len()];
        for arm in arms {
            let (pattern, _) = evidence_pattern_parts(&arm.pattern);
            let constructor = match &pattern.kind {
                PatternKind::Evidence { constructor, .. } => &**constructor,
                _ => pattern,
            };
            match &constructor.kind {
                PatternKind::Wildcard => {
                    for slot in chosen.iter_mut().filter(|slot| slot.is_none()) {
                        *slot = Some(arm);
                    }
                }
                PatternKind::Variant { path, .. } | PatternKind::Struct { path, .. }
                    if path
                        .pair()
                        .is_some_and(|(prefix, _)| prefix.text == info.name) =>
                {
                    let index = self.prop_variant(&info, path)?;
                    if chosen[index].is_some() {
                        return self.fail("L0214", "this arm is never reached", arm.pattern.span);
                    }
                    chosen[index] = Some(arm);
                }
                _ => {
                    let message = format!(
                        "an arm here is `{}::Variant(names...)`, `{}::Variant {{ fields... }}`, or `_`",
                        info.name, info.name
                    );
                    return self.fail("L0212", message, arm.pattern.span);
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

        // The match is a proof: its arms are erased, and a value named in
        // one is read, not moved (`moves.rs`).
        let ghost = self.moves.enter_ghost();
        let mut proof_arms = Vec::new();
        for (variant, arm) in info.variants.iter().zip(chosen) {
            let arm = arm.expect("every variant has an arm");
            let what = format!("`{}::{}`", info.name, variant.name);
            let (pattern, wholes) = evidence_pattern_parts(&arm.pattern);
            let names = self.named_proof_pattern_names(pattern, &what, variant)?;
            let mark = self.mark();
            let arm_result = (|| {
                let mut aliases = Vec::new();
                for whole in &wholes {
                    let id = VarId::fresh();
                    let term = Term::Proof(Box::new(evidence.clone()));
                    let result = self.ctx.define_with(id, HypId::fresh(), &term);
                    let ty = self.kernel(result, whole.span)?;
                    self.bind(&whole.text, id, &ty, false);
                    aliases.push((id, term));
                }
                let mut binders: Vec<Binder> = Vec::new();
                for (declared, name) in variant.payload.iter().zip(&names) {
                    let mut ty = declared.ty.clone();
                    if variant.conclusion.is_none() {
                        for (param, argument) in info.params.iter().zip(&arguments) {
                            ty = ty.replace_var(param.id, argument);
                        }
                    }
                    for (earlier, fresh) in variant.payload.iter().zip(&binders) {
                        ty = ty.replace_var(earlier.id, &fresh.term());
                    }
                    let binder = Binder {
                        id: VarId::fresh(),
                        name: name.map_or_else(|| "_".to_string(), |name| name.text.clone()),
                        ty,
                        ghost: false,
                    };
                    let declared = self.ctx.declare_with(binder.id, binder.ty.clone(), true);
                    self.kernel(declared, arm.pattern.span)?;
                    if name.is_some() {
                        self.bind(&binder.name, binder.id, &binder.ty, binder.ghost);
                    }
                    binders.push(binder);
                }
                // One equation per parameter, for a variant that states where it applies.
                let mut equations = Vec::new();
                for (position, stated) in variant.conclusion.iter().flatten().enumerate() {
                    let mut stated = stated.clone();
                    for (declared, fresh) in variant.payload.iter().zip(&binders) {
                        stated = stated.replace_var(declared.id, &fresh.term());
                    }
                    let id = HypId::fresh();
                    let claim = Term::eq(
                        info.params[position].ty.clone(),
                        arguments[position].clone(),
                        stated,
                    );
                    self.assume(id, claim, arm.pattern.span)?;
                    equations.push(id);
                }
                let expected = Type::proof(goal.clone());
                let (block, _, _) = self.logical("a proof match arm", |env| {
                    env.branch(&Branch::Expr(&arm.body), Some(&expected))
                })?;
                let body = Value::new(Expr::Block(block), Type::proof(goal.clone()));
                let body = self.proof_of(&body, arm.body.span)?;
                let body = substitute(body, &aliases, &[]);
                Ok((binders, equations, body))
            })();
            self.close(mark);
            let (binders, equations, body) = arm_result?;
            proof_arms.push(Proof::arm(binders.len(), equations.len(), |vars, hyps| {
                let vars: Vec<(VarId, Term)> = binders
                    .iter()
                    .map(|binder| binder.id)
                    .zip(vars.iter().cloned())
                    .collect();
                let hyps: Vec<(HypId, Proof)> = equations
                    .iter()
                    .copied()
                    .zip(hyps.iter().cloned())
                    .collect();
                substitute(body, &vars, &hyps)
            }));
        }
        self.moves.leave_ghost(ghost);
        let mut result = self.proved(
            Proof::CaseProof {
                scrutinee: Box::new(evidence),
                goal,
                arms: proof_arms,
            },
            span,
        )?;
        if !is_pure(&scrutinee.expr) {
            result.expr = Expr::Block(typed::Block {
                stmts: vec![typed::Stmt::Expr(scrutinee.expr)],
                tail: Some(Box::new(result.expr)),
            });
        }
        Ok(result)
    }

    pub(super) fn rewrite_equation(&mut self, mut equation: Proof) -> Proof {
        // Logical comparisons are Bool-valued tests lifted to Prop.
        // Rewriting needs the equality of Int values, not the equality
        // saying that their comparison returned true.
        if let Ok(Term::Eq(Type::Bool, test, outcome)) = infer_proof(&mut self.ctx, &equation)
            && matches!(
                (&*test, &*outcome),
                (Term::Prim(Prim::IntCmp(_), _), Term::Bool(true))
            )
        {
            equation = Proof::implies_elim(Proof::Axiom(Axiom::CmpReflect(*test, true)), equation);
        }
        // A source equation between observations of machine values can
        // also rewrite those values inside a machine operation. This is
        // justified by the checked injectivity lemma, not syntax matching.
        if let Ok(Term::Eq(Type::Int, a, b)) = infer_proof(&mut self.ctx, &equation)
            && let (Term::Prim(Prim::View(left), xs), Term::Prim(Prim::View(right), ys)) =
                (&*a, &*b)
            && left == right
            && xs.len() == 1
            && ys.len() == 1
        {
            equation = self.equal_of_views(*left, &xs[0], &ys[0], equation);
        }
        equation
    }

    /// `rewrite!(eq, h)`, `unfold!(f, h)`, `fold!(f, h)`: transport, with
    /// the occurrences to replace worked out here.
    pub fn proof_form(
        &mut self,
        form: Form,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let [first, second] = arguments else {
            return self.fail(
                "L0208",
                format!("`{}!` takes two arguments", form.name()),
                span,
            );
        };
        let mut effects = Vec::new();
        let evidence =
            |env: &mut Self, expr: &ast::Expr, effects: &mut Vec<typed::Stmt>| -> Elab<Proof> {
                let value = env.infer(expr)?;
                if !matches!(value.ty, Type::Proof(_)) {
                    let shown = env.show_type(&value.ty);
                    return env.fail(
                        "L0220",
                        format!("expected evidence, found `{shown}`"),
                        expr.span,
                    );
                }
                let proof = env.proof_of(&value, expr.span)?;
                if !is_pure(&value.expr) {
                    effects.push(typed::Stmt::Expr(value.expr));
                }
                Ok(proof)
            };
        if form == Form::Rewrite {
            let mut equation = evidence(self, first, &mut effects)?;
            equation = self.rewrite_equation(equation);
            let target = evidence(self, second, &mut effects)?;
            return match derive::rewrite(&mut self.ctx, &equation, &target) {
                Ok(proof) => self.proved_with_effects(proof, effects, span),
                Err(_) => self.fail(
                    "L0229",
                    "`rewrite!` takes evidence of an equation `a == b` first",
                    first.span,
                ),
            };
        }
        if let ExprKind::Name(name) = &first.kind
            && matches!(self.types.get(&name.text), Some(Global::Prop(_)))
        {
            return self.fail("L0277", "a declared proposition is opened by matching and closed with its constructor; fold! and unfold! apply to logic functions", first.span);
        }
        // The function whose defining equation the step uses: one that may
        // appear in a proposition, since the equation is one.
        let named = match &first.kind {
            ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                match self.values.get(&name.text).cloned() {
                    Some(Global::Fn(info)) => Some(info),
                    _ => None,
                }
            }
            // A function of an `impl` block, `Type::name`.
            ExprKind::Path(path) => self.path_function(path),
            _ => None,
        };
        let selected = if let ExprKind::Cast { expr, ty, .. } = &first.kind {
            Some(self.model_definition_selector(expr, ty, first.span)?)
        } else {
            None
        };
        let function = if let Some((function, _)) = &selected {
            Some(*function)
        } else {
            match named {
                Some(info) => {
                    self.admit_to_formula(&info, "a proposition", first.span)?;
                    match info.reference {
                        FnRef::Math(id) => Some(id),
                        FnRef::Exec(_) => None,
                    }
                }
                None => None,
            }
        };
        let Some(function) = function else {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0229",
                    format!("`{}!` takes the name of a function first", form.name()),
                    first.span,
                )
                .note("the step uses the function's defining equation, so the function is one that may appear in a proposition"),
            );
            return Err(());
        };
        let target = evidence(self, second, &mut effects)?;
        if let Some((_, selected)) = &selected {
            let claim = if form == Form::Unfold {
                let claim = infer_proof(&mut self.ctx, &target);
                Some(self.kernel(claim, span)?)
            } else {
                expected.and_then(|ty| match ty {
                    Type::Proof(claim) => Some((**claim).clone()),
                    _ => None,
                })
            };
            if let Some(claim) = claim {
                let known = self.knowledge();
                let (claim, _) = self.normalize(&claim, &known.definitions);
                let (selected, _) = self.normalize(selected, &known.definitions);
                if claim
                    .find(&|term| crate::kernel::same(term, &selected))
                    .is_none()
                {
                    return self.fail(
                        "L0229",
                        "the selected model observation does not occur in this proof's claim",
                        first.span,
                    );
                }
            }
        }
        let result = if form == Form::Unfold {
            derive::unfold(&mut self.ctx, function, &target)
        } else {
            let Some(Type::Proof(goal)) = expected else {
                self.diagnostics.push(
                    crate::diagnostic::Diagnostic::error(
                        "L0229",
                        "`fold!` needs to know the claim it should produce",
                        span,
                    )
                    .note("state it with an annotation: `let folded: @name(x) = fold!(name, evidence);`"),
                );
                return Err(());
            };
            // Both sides are computed first, as evidence is matched:
            // `within_limit(next.failures)` with `next` a struct just
            // written is `within_limit(0)` to the definition, and evidence
            // of `out == n.wrapping_add(1)` with `out` bound to that sum is
            // evidence of a reflexive equation.
            let known = self.knowledge();
            let (computed, steps) = self.normalize(goal, &known.definitions);
            let target = match infer_proof(&mut self.ctx, &target) {
                Ok(claim) => {
                    let (_, target_steps) = self.normalize(&claim, &known.definitions);
                    forward(target, target_steps)
                }
                Err(_) => target,
            };
            let (constructor_body, constructor_steps) =
                self.computing_definitions(&computed, function, &known);
            if !constructor_steps.is_empty()
                && let Ok(target_claim) = infer_proof(&mut self.ctx, &target)
            {
                let (_, target_steps) = self.computing_definitions(&target_claim, function, &known);
                let opened_target = forward(target.clone(), target_steps);
                let mark = self.facts.len();
                if let Ok(claim) = infer_proof(&mut self.ctx, &opened_target) {
                    self.facts.push(super::env::Fact::new(opened_target, claim));
                }
                let fitted = self.attempt(&constructor_body).map(|(proof, _)| proof);
                self.facts.truncate(mark);
                if let Some(fitted) = fitted
                    && let Some(closed) = self.back_to_stated(fitted, constructor_steps)
                    && let Some(closed) = self.back_to_stated(closed, steps.clone())
                    && check_proof(&mut self.ctx, &closed, goal).is_ok()
                {
                    return self.proved_with_effects(closed, effects, span);
                }
            }
            // The defining body may itself contain computable Bool tests.
            // Recover its exact syntax from a temporary hypothetical goal,
            // then replay computation backwards before the defining equation.
            // The hypothesis is discarded and never enters the returned proof.
            let mut target = target;
            {
                let checkpoint = self.ctx.checkpoint();
                let hypothesis = HypId::fresh();
                let unfolded = self
                    .ctx
                    .assume_with(hypothesis, computed.clone())
                    .and_then(|_| derive::unfold(&mut self.ctx, function, &Proof::hyp(hypothesis)))
                    .and_then(|proof| infer_proof(&mut self.ctx, &proof));
                self.ctx.rollback(checkpoint);
                if let Ok(unfolded) = unfolded {
                    let (normal_body, body_steps) = self.reduce_known_cases(&unfolded, &known);
                    let mark = self.facts.len();
                    if let Ok(claim) = infer_proof(&mut self.ctx, &target) {
                        self.facts
                            .push(super::env::Fact::new(target.clone(), claim));
                    }
                    let fitted = self.attempt(&normal_body).map(|(proof, _)| proof);
                    self.facts.truncate(mark);
                    if let Some(fitted) = fitted
                        && check_proof(&mut self.ctx, &fitted, &normal_body).is_ok()
                        && let Some(uncomputed) = self.back_to_stated(fitted, body_steps)
                    {
                        target = uncomputed;
                    }
                }
            }
            let folded =
                derive::fold(&mut self.ctx, function, &target, &computed).and_then(|folded| {
                    self.back_to_stated(folded, steps)
                        .ok_or(KernelError::NoComputationStep((**goal).clone()))
                });
            // A fold that reaches the definition and then does not fit it
            // is evidence of the wrong claim, which is said as such.
            if let Ok(folded) = &folded
                && check_proof(&mut self.ctx, folded, goal).is_err()
                && let Ok(claim) = infer_proof(&mut self.ctx, &target)
            {
                let (given, wanted) = (self.show(&claim), self.show(goal));
                self.diagnostics.push(
                    crate::diagnostic::Diagnostic::error(
                        "L0230",
                        format!("this is evidence of `{given}`, and `{wanted}` is needed"),
                        second.span,
                    )
                    .claim(wanted),
                );
                return Err(());
            }
            folded
        };
        match result {
            Ok(proof) => self.proved_with_effects(proof, effects, span),
            Err(_) => {
                let text = self.text(first.span).to_string();
                self.fail(
                    "L0229",
                    format!("`{}!` found no use of `{text}` to work on", form.name()),
                    span,
                )
            }
        }
    }

    fn proved_with_effects(
        &mut self,
        proof: Proof,
        effects: Vec<typed::Stmt>,
        span: Span,
    ) -> Elab<Value> {
        let mut value = self.proved(proof, span)?;
        if !effects.is_empty() {
            value.expr = Expr::Block(typed::Block {
                stmts: effects,
                tail: Some(Box::new(value.expr)),
            });
        }
        Ok(value)
    }

    fn named_proof_pattern_names<'p>(
        &mut self,
        pattern: &'p ast::Pattern,
        what: &str,
        variant: &super::env::PropVariantInfo,
    ) -> Elab<Vec<Option<&'p ast::Name>>> {
        if variant.body.is_none() {
            return self.pattern_names(pattern, what, &variant.payload, variant.named);
        }
        match &pattern.kind {
            PatternKind::Wildcard => Ok(vec![None; variant.payload.len()]),
            PatternKind::Evidence { constructor, evidence, .. } => {
                let witnesses = &variant.payload[..variant.payload.len() - 1];
                let mut names = self.pattern_names(constructor, what, witnesses, variant.named)?;
                names.push(match &evidence.kind {
                    PatternKind::Name { name, mutable: false } => Some(name),
                    PatternKind::Wildcard => None,
                    _ => return self.fail("L0275", "the evidence slot binds a name or `_`; use a subsequent match for nested evidence", evidence.span),
                });
                Ok(names)
            }
            _ => self.fail("L0276", "a named proposition pattern keeps its evidence outside: `Pred::Arm(witnesses) @ evidence`", pattern.span),
        }
    }

    /// Evidence of a `forall` applied to a value, or of an implication
    /// applied to evidence of its premise.
    pub fn apply(&mut self, callee: Value, arguments: &[ast::Expr], span: Span) -> Elab<Value> {
        if matches!(callee.ty, Type::Fn(..)) {
            return self.apply_logical_callable(callee, arguments, span);
        }
        let mut current = callee;
        for argument in arguments {
            let Type::Proof(claim) = current.ty.clone() else {
                let shown = self.show_type(&current.ty);
                return self.fail("L0207", format!("a `{shown}` cannot be applied"), span);
            };
            let proof = self.proof_of(&current, span)?;
            if let Some((quantifiers, predicate)) = self.universal(&claim) {
                let value = self.check(argument, quantifiers.element())?;
                let term = self.term(&value, argument.span)?;
                let application = Term::call(predicate.clone(), vec![term.clone()]);
                let mut specialized = quantifiers.specialize(proof, predicate.clone(), term);
                if matches!(predicate, Term::Lambda { .. }) {
                    specialized =
                        Proof::transport(Proof::Definition(application), |p| p, specialized);
                }
                current = self.proved(specialized, span)?;
                continue;
            }
            let applied = match &*claim {
                Term::Implies(premise, _) => {
                    let value = self.check(argument, &Type::Proof(premise.clone()))?;
                    let premise = self.proof_of(&value, argument.span)?;
                    Proof::implies_elim(proof, premise)
                }
                other => {
                    let shown = self.show(other);
                    return self.fail(
                        "L0228",
                        format!("this is evidence of `{shown}`, which takes no argument"),
                        argument.span,
                    );
                }
            };
            current = self.proved(applied, span)?;
        }
        Ok(current)
    }

    /// A function of the logic that returns evidence, named as evidence itself: its
    /// parameters become quantifiers and premises, in order.
    pub fn function_as_evidence(&mut self, info: &FnInfo, span: Span) -> Elab<Value> {
        // Its claim is a proposition that mentions it at every argument.
        self.admit_to_formula(info, "a proposition", span)?;
        let FnRef::Math(id) = info.reference else {
            return self.internal(
                format!(
                    "`{}` may appear in a proposition and is not a kernel function",
                    info.name
                ),
                span,
            );
        };
        if !matches!(info.result, Type::Proof(_)) {
            return self.fail(
                "L0220",
                format!("`{}` does not return evidence", info.name),
                span,
            );
        }
        let proof = super::quantifiers::function_evidence(
            self.session.program().definitions(),
            &self.quantifiers,
            info,
            id,
        );
        match proof {
            Some(proof) => self.proved(proof, span),
            None => self.fail("L0290", "use a logical closure or state the ForAll type when naming this theorem as evidence", span),
        }
    }
}

/// Strip grouping and collect Rust `whole @ pattern` bindings. Those aliases
/// denote the original proof, never a new observation of its representation.
fn evidence_pattern_parts(pattern: &ast::Pattern) -> (&ast::Pattern, Vec<&ast::Name>) {
    let mut current = pattern;
    let mut wholes = Vec::new();
    loop {
        match &current.kind {
            PatternKind::Group(inner) => current = inner,
            PatternKind::Binding { name, pattern, .. } => {
                wholes.push(name);
                current = pattern;
            }
            _ => return (current, wholes),
        }
    }
}
