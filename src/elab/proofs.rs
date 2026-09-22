//! Evidence written out: proof constructors, `match` on evidence, the
//! `rewrite!`, `unfold!` and `fold!` forms, and applying evidence of a `forall`
//! or an implication. Each becomes an explicit kernel proof.

use crate::ast::{self, ExprKind, Form, PatternKind};
use crate::diagnostic::Diagnostic;
use crate::kernel::derive;
use crate::kernel::{HypId, KernelError, Proof, Term, Type, VarId, check_proof, infer_proof};
use crate::source::Span;
use crate::typed::{Binder, Expr, FnRef, value_term};

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

    /// `Name::Variant(payload)`, which proves `Name(...)`.
    pub fn construct(
        &mut self,
        info: &PropInfo,
        path: &ast::Path,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let name = path.last();
        let Some(index) = info
            .variants
            .iter()
            .position(|variant| variant.name == name.text)
        else {
            let message = format!("`{}` has no variant `{}`", info.name, name.text);
            return self.fail("L0212", message, name.span);
        };
        let variant = &info.variants[index];
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
        let what = format!("`{}::{}`", info.name, variant.name);
        let payload = self.arguments(arguments, &ids, &mut tys, &what, span)?;
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

    /// `match evidence { Name::Variant(payload) => evidence, ... }`.
    pub fn match_evidence(
        &mut self,
        scrutinee: Value,
        scrutinee_span: Span,
        arms: &[ast::MatchArm],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
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
            return Ok(Value::new(
                Expr::Absurd {
                    proof: evidence,
                    ty: ty.clone(),
                },
                ty,
            ));
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
            match &arm.pattern.kind {
                PatternKind::Wildcard => {
                    for slot in chosen.iter_mut().filter(|slot| slot.is_none()) {
                        *slot = Some(arm);
                    }
                }
                PatternKind::Variant { path, .. }
                    if path
                        .pair()
                        .is_some_and(|(prefix, _)| prefix.text == info.name) =>
                {
                    let name = path.last();
                    let Some(index) = info
                        .variants
                        .iter()
                        .position(|variant| variant.name == name.text)
                    else {
                        let message = format!("`{}` has no variant `{}`", info.name, name.text);
                        return self.fail("L0212", message, name.span);
                    };
                    if chosen[index].is_some() {
                        return self.fail("L0214", "this arm is never reached", arm.pattern.span);
                    }
                    chosen[index] = Some(arm);
                }
                _ => {
                    let message =
                        format!("an arm here is `{}::Variant(names...)` or `_`", info.name);
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

        let mut proof_arms = Vec::new();
        for (variant, arm) in info.variants.iter().zip(chosen) {
            let arm = arm.expect("every variant has an arm");
            let names: Vec<Option<&ast::Name>> = match &arm.pattern.kind {
                PatternKind::Variant { arguments, path } => {
                    let given = arguments.as_deref().unwrap_or(&[]);
                    if given.len() != variant.payload.len() {
                        let message = format!(
                            "`{}::{}` carries {} value(s), and the pattern names {}",
                            info.name,
                            variant.name,
                            variant.payload.len(),
                            given.len()
                        );
                        return self.fail("L0208", message, path.span);
                    }
                    let mut names = Vec::new();
                    for pattern in given {
                        match &pattern.kind {
                            PatternKind::Name {
                                name,
                                mutable: false,
                            } => names.push(Some(name)),
                            PatternKind::Wildcard => names.push(None),
                            _ => {
                                return self.fail(
                                    "L0290",
                                    "patterns inside a variant are names or `_` for now",
                                    pattern.span,
                                );
                            }
                        }
                    }
                    names
                }
                _ => vec![None; variant.payload.len()],
            };
            let mark = self.mark();
            let arm_result = (|| {
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
                    };
                    let declared = self.ctx.declare_with(binder.id, binder.ty.clone(), true);
                    self.kernel(declared, arm.pattern.span)?;
                    if name.is_some() {
                        self.bind(&binder.name, binder.id, &binder.ty);
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
                let (block, _, _) =
                    self.branch(&Branch::Expr(&arm.body), Some(&Type::proof(goal.clone())))?;
                let body = Value::new(Expr::Block(block), Type::proof(goal.clone()));
                let body = self.proof_of(&body, arm.body.span)?;
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
        self.proved(
            Proof::CaseProof {
                scrutinee: Box::new(evidence),
                goal,
                arms: proof_arms,
            },
            span,
        )
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
        let evidence = |env: &mut Self, expr: &ast::Expr| -> Elab<Proof> {
            let value = env.infer(expr)?;
            if !matches!(value.ty, Type::Proof(_)) {
                let shown = env.show_type(&value.ty);
                return env.fail(
                    "L0220",
                    format!("expected evidence, found `{shown}`"),
                    expr.span,
                );
            }
            env.proof_of(&value, expr.span)
        };
        if form == Form::Rewrite {
            let equation = evidence(self, first)?;
            let target = evidence(self, second)?;
            return match derive::rewrite(&mut self.ctx, &equation, &target) {
                Ok(proof) => self.proved(proof, span),
                Err(_) => self.fail(
                    "L0229",
                    "`rewrite!` takes evidence of an equation `a == b` first",
                    first.span,
                ),
            };
        }
        // The function whose defining equation the step uses: one that may
        // appear in a proposition, since the equation is one.
        let function = match &first.kind {
            ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                match self.globals.get(&name.text).cloned() {
                    Some(Global::Fn(info)) => {
                        self.admit_to_formula(&info, "a proposition", first.span)?;
                        match info.reference {
                            FnRef::Math(id) => Some(id),
                            FnRef::Exec(_) => None,
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
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
        let target = evidence(self, second)?;
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
                return self.fail(
                    "L0230",
                    format!("this is evidence of `{given}`, and `{wanted}` is needed"),
                    second.span,
                );
            }
            folded
        };
        match result {
            Ok(proof) => self.proved(proof, span),
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

    /// Evidence of a `forall` applied to a value, or of an implication
    /// applied to evidence of its premise.
    pub fn apply(&mut self, callee: Value, arguments: &[ast::Expr], span: Span) -> Elab<Value> {
        let mut current = callee;
        for argument in arguments {
            let Type::Proof(claim) = current.ty.clone() else {
                let shown = self.show_type(&current.ty);
                return self.fail("L0207", format!("a `{shown}` cannot be applied"), span);
            };
            let proof = self.proof_of(&current, span)?;
            let applied = match &*claim {
                Term::Forall(ty, _) => {
                    let value = self.check(argument, ty)?;
                    let term = self.term(&value, argument.span)?;
                    Proof::forall_elim(proof, term)
                }
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

    /// A `math fn` that returns evidence, named as evidence itself: its
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
        fn close(info: &FnInfo, id: crate::kernel::FnId, given: Vec<Term>) -> Proof {
            let Some(param) = info.params.get(given.len()) else {
                return Proof::OfTerm(Term::call(Term::Fn(id), given));
            };
            let mut ty = param.ty.clone();
            for (earlier, term) in info.params.iter().zip(&given) {
                ty = ty.replace_var(earlier.id, term);
            }
            match ty {
                Type::Proof(premise) => Proof::implies_intro(*premise, |evidence| {
                    let mut given = given;
                    given.push(Term::proof(evidence));
                    close(info, id, given)
                }),
                ty => Proof::forall_intro(ty, |value| {
                    let mut given = given;
                    given.push(value);
                    close(info, id, given)
                }),
            }
        }
        self.proved(close(info, id, Vec::new()), span)
    }
}
