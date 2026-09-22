//! Why a hole stayed open, and what would fill it.
//!
//! The diagnostic for an unfilled hole is the product of the rule that a
//! hole is filled by a fact that matches, after computing, or not at all.
//! It shows the claim as stated, the claim after computing when that
//! differs, and the facts in scope that speak of the same names, nearest
//! first. Then it says what the search that used to bridge claims would
//! have done, when it would have filled the hole, and the explicit form
//! that is accepted instead: a lemma call, `rewrite!`, `unfold!` or
//! `fold!`, a constructor, or a `prove!` stepping stone.
//!
//! Every form suggested was built and checked by the kernel against the
//! computed claim first, so the note never names a form that is refused.
//! When no such form is found, the note says which kind of step is missing.
//! A claim that fails for some byte the facts allow is refuted instead, and
//! the byte is named: nothing would fill it.

use crate::diagnostic::Diagnostic;
use crate::kernel::derive;
use crate::kernel::{
    FnId, HypId, KernelError, MachineInt, Prim, Proof, Term, Type, VarId, check_proof, infer_proof,
    same,
};
use crate::source::Span;

use super::env::{Env, Fact};
use super::solve::{Definition, Known, STEP_LIMIT, Step, Test, forward};

/// Something the removed search would have used: a fact in scope, perhaps
/// unfolded or read off a branch, with the text that names it in source.
struct Candidate {
    claim: Term,
    proof: Proof,
    /// `bounded`, `unfold!(within_limit, bounded)`, `prove!(n < 10)`.
    text: String,
}

/// What evaluating every byte says about a claim, under the facts about
/// that byte.
enum Cases {
    Refuted(Term, u8),
    Holds,
    Undecided,
}

/// The nesting of the search over the connectives.
const STRUCTURAL_DEPTH: usize = 8;

impl Env<'_> {
    pub(super) fn report_unsolved(&mut self, goal: &Term, span: Span, given: Option<&Term>) {
        // The claim as stated, with the values written for its names put in
        // their fields; then the claim with names replaced too.
        let stated = self.computed(goal);
        let claim = self.show(&stated);
        let mut diagnostic = match given {
            Some(given) => {
                let given = self.show(given);
                Diagnostic::error(
                    "L0230",
                    format!("this is evidence of `{given}`, and `{claim}` is needed"),
                    span,
                )
            }
            None => Diagnostic::error("L0230", format!("cannot show `{claim}`"), span),
        };
        let known = self.knowledge();
        let (normal, _) = self.normalize(goal, &known.definitions);
        let normal_text = self.show(&normal);
        if normal_text != claim {
            diagnostic = diagnostic.note(format!("after computing, the claim is `{normal_text}`"));
        }

        // The facts that speak of something the claim speaks of, nearest
        // first, each once, under its name when it has one.
        let subjects = free_variables(&normal);
        let mut shown: Vec<(String, Option<String>)> = Vec::new();
        for (index, fact) in known.facts.iter().rev() {
            let theirs = free_variables(&fact.claim);
            if !theirs.iter().any(|variable| subjects.contains(variable)) {
                continue;
            }
            let claim = self.show(&fact.claim);
            let name = self.fact_name(*index);
            if let Some((_, seen_name)) = shown.iter_mut().find(|(seen, _)| *seen == claim) {
                if seen_name.is_none() {
                    *seen_name = name;
                }
            } else if shown.len() < 6 {
                shown.push((claim, name));
            }
        }
        diagnostic = if shown.is_empty() {
            diagnostic.note("nothing known here speaks of the values in this claim")
        } else {
            let list: Vec<String> = shown
                .iter()
                .map(|(claim, name)| match name {
                    Some(name) => format!("`{name}: {claim}`"),
                    None => format!("`{claim}`"),
                })
                .collect();
            diagnostic.note(format!("known here: {}", list.join(", ")))
        };

        for note in self.bridge(&normal, &known) {
            diagnostic = diagnostic.note(note);
        }
        self.diagnostics.push(diagnostic);
    }

    /// The name of the fact at this index of the scope, when it is
    /// evidence bound to a name.
    fn fact_name(&self, index: usize) -> Option<String> {
        let fact = self.facts.get(index)?;
        let Proof::OfTerm(Term::Free(id)) = &fact.proof else {
            return None;
        };
        let label = self.labels.get(id)?;
        let is_name = !label.is_empty()
            && label
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_');
        is_name.then(|| label.clone())
    }

    // --- What the removed search would have done -------------------------------------

    /// The notes about the removed tiers: the counterexample when there is
    /// one, otherwise the explicit form that is accepted, or the kind of
    /// step that is missing.
    fn bridge(&mut self, normal: &Term, known: &Known) -> Vec<String> {
        let candidates = self.candidates(known);
        let (opened, _) = self.opened(normal, &known.definitions);
        let unfolds = self.program_calls(normal);
        let mut opened_candidates = Vec::new();
        for candidate in &candidates {
            opened_candidates.push(self.opened_candidate(candidate, &known.definitions));
        }
        // A claim some byte refutes has no proof: the facts, definitions
        // opened, allow that byte.
        let cases = self.all_cases(&opened, &opened_candidates);
        if let Cases::Refuted(unknown, byte) = cases {
            let unknown = self.show(&unknown);
            return vec![format!(
                "it fails when `{unknown}` is {byte}, which the facts known here allow"
            )];
        }
        let mut notes = Vec::new();

        // Unfolding without being asked.
        if let Some(form) = self.by_unfolding(normal, &opened, &opened_candidates) {
            notes.push(form);
            return notes;
        }
        // Rewriting by an equation in scope.
        if let Some(form) = self.by_rewriting(normal, &candidates, &opened_candidates) {
            notes.push(form);
            return notes;
        }
        // One lemma, where the proof by all 256 cases used to go.
        if let Some(form) = self.by_lemma(normal, &opened, &opened_candidates) {
            notes.push(form);
            return notes;
        }
        if let Some((equation, claim)) = self.would_rewrite(&opened, &opened_candidates) {
            notes.push(format!(
                "rewriting by the equation `{claim}` was removed; `rewrite!({equation}, h)` carries evidence `h` across it, replacing every occurrence of the left side, and a name bound by `let` is replaced by what it stands for only when computing"
            ));
            return notes;
        }
        if matches!(cases, Cases::Holds) {
            let unknown = self
                .as_test(&opened)
                .and_then(|test| single_byte(&test.test))
                .map_or_else(|| "the unknown".to_string(), |byte| self.show(&byte));
            notes.push(format!(
                "the proof by all 256 cases of `{unknown}` was removed; this holds for every value the facts allow, and the step from the facts to it is stated with a lemma call, such as `u8_le_trans`, or a `prove!` stepping stone"
            ));
            return notes;
        }
        // The search over the connectives.
        if let Some(form) = self.by_structure(&opened, &opened_candidates) {
            notes.push(form);
            return notes;
        }
        if !unfolds.is_empty() {
            let names: Vec<String> = unfolds
                .iter()
                .map(|(_, name)| format!("`{name}`"))
                .collect();
            notes.push(format!(
                "{} is not unfolded without being asked: `unfold!` opens a definition in evidence in scope, and `fold!` closes one over the evidence of its body",
                names.join(", ")
            ));
        }
        notes
    }

    /// Every fact, and every branch fact read as the comparison it decided.
    fn candidates(&mut self, known: &Known) -> Vec<Candidate> {
        let mut candidates: Vec<Candidate> = Vec::new();
        for (index, fact) in known.facts.iter().rev() {
            let name = self.fact_name(*index);
            // A fact known under a name and again without one, as the
            // evidence being converted is, goes by its name.
            if let Some(seen) = candidates
                .iter_mut()
                .find(|candidate| same(&candidate.claim, &fact.claim))
            {
                if let Some(name) = name
                    && seen.text.starts_with("prove!(")
                {
                    seen.proof = fact.proof.clone();
                    seen.text = name;
                }
                continue;
            }
            let text = match name {
                Some(name) => name,
                None => format!("prove!({})", self.show(&fact.claim)),
            };
            candidates.push(Candidate {
                claim: fact.claim.clone(),
                proof: fact.proof.clone(),
                text,
            });
            if let Term::Eq(Type::Bool, test, outcome) = &fact.claim
                && let Term::Bool(outcome) = **outcome
                && let Some(claim) = self.comparison_claim(test, outcome)
            {
                let test = Test {
                    test: (**test).clone(),
                    outcome,
                };
                let claim = self.computed(&claim);
                if let Some(proof) = self.reflect_test(&claim, &test, fact.proof.clone()) {
                    let text = format!("prove!({})", self.show(&claim));
                    candidates.push(Candidate { claim, proof, text });
                }
            }
        }
        candidates
    }

    /// The proposition a comparison decides: `a ==[T] b` for `eq[T](a, b)`,
    /// and the order of the views for `lt[T]` and `le[T]`, when the test is
    /// known true; its negation for one known false.
    fn comparison_claim(&self, test: &Term, outcome: bool) -> Option<Term> {
        let Term::Prim(Prim::Cmp(op, ty), operands) = test else {
            return None;
        };
        let [a, b] = operands.as_slice() else {
            return None;
        };
        let prelude = self.prelude;
        let positive = match op {
            crate::kernel::CmpOp::Eq => Term::eq(Type::machine(*ty), a.clone(), b.clone()),
            op => op.claim(Term::view(*ty, a.clone()), Term::view(*ty, b.clone())),
        };
        Some(if outcome {
            positive
        } else {
            prelude.not_prop(positive)
        })
    }

    /// A candidate with every call of the program's own functions unfolded,
    /// and the text that asks for that.
    fn opened_candidate(&mut self, candidate: &Candidate, definitions: &[Definition]) -> Candidate {
        let calls = self.program_calls(&candidate.claim);
        if calls.is_empty() {
            return Candidate {
                claim: candidate.claim.clone(),
                proof: candidate.proof.clone(),
                text: candidate.text.clone(),
            };
        }
        let (claim, steps) = self.opened(&candidate.claim, definitions);
        let text = calls
            .iter()
            .fold(candidate.text.clone(), |text, (_, name)| {
                format!("unfold!({name}, {text})")
            });
        Candidate {
            claim,
            proof: forward(candidate.proof.clone(), steps),
            text,
        }
    }

    /// The program's own functions called in a term, each once, outermost
    /// first.
    fn program_calls(&self, term: &Term) -> Vec<(FnId, String)> {
        let found = std::cell::RefCell::new(Vec::<(FnId, String)>::new());
        let _ = term.find(&|candidate| {
            if let Term::Call(callee, _) = candidate
                && let Term::Fn(id) = **callee
                && let Some(info) = self.fn_by_id(id)
                && !found.borrow().iter().any(|(seen, _)| *seen == id)
            {
                found.borrow_mut().push((id, info.name.clone()));
            }
            false
        });
        found.into_inner()
    }

    /// Unfolds every call of the program's own functions and computes, until
    /// nothing changes: what the removed tier did to both sides.
    fn opened(&mut self, term: &Term, definitions: &[Definition]) -> (Term, Vec<Step>) {
        let mut term = term.clone();
        let mut steps = Vec::new();
        for _ in 0..STEP_LIMIT {
            let before = steps.len();
            let (normal, more) = self.normalize(&term, definitions);
            term = normal;
            steps.extend(more);
            let Some(call) = term
                .find(&|candidate| {
                    candidate.is_closed()
                        && matches!(candidate, Term::Call(callee, _) if matches!(**callee, Term::Fn(id) if self.fn_by_id(id).is_some()))
                })
                .cloned()
            else {
                break;
            };
            let eq = Proof::Definition(call.clone());
            let Ok(Term::Eq(_, _, body)) = infer_proof(&mut self.ctx, &eq) else {
                break;
            };
            let template = term.abstract_over(&|candidate| same(candidate, &call));
            term = template.open(&body);
            steps.push(Step { eq, template });
            if steps.len() == before {
                break;
            }
        }
        (term, steps)
    }

    /// Whether a fact matches a claim, after computing: what tier 2 and 3
    /// decide, on claims already computed.
    fn leaf(&mut self, goal: &Term, candidates: &[Candidate]) -> Option<Proof> {
        let known = Known {
            facts: candidates
                .iter()
                .map(|candidate| {
                    (
                        usize::MAX,
                        Fact::new(candidate.proof.clone(), candidate.claim.clone()),
                    )
                })
                .collect(),
            definitions: Vec::new(),
        };
        self.computed_from(goal, &known)
            .or_else(|| self.evaluated(goal))
    }

    /// The fact that is the claim once definitions are unfolded, as
    /// `fold!` and `unfold!` write it.
    fn by_unfolding(
        &mut self,
        normal: &Term,
        opened: &Term,
        candidates: &[Candidate],
    ) -> Option<String> {
        let (proof, inner) = match candidates
            .iter()
            .find(|candidate| same(&candidate.claim, opened))
        {
            Some(inner) => (inner.proof.clone(), inner.text.clone()),
            None => {
                let proof = self.leaf(opened, candidates)?;
                (proof, format!("prove!({})", self.show(opened)))
            }
        };
        if self.program_calls(normal).is_empty() && !inner.starts_with("unfold!(") {
            return None;
        }
        let folded = self.folded(normal, proof)?;
        check_proof(&mut self.ctx, &folded, normal).ok()?;
        let text = self.folded_text(normal, &inner);
        Some(format!(
            "this is `{text}`: a definition is opened only by `unfold!` and closed only by `fold!`"
        ))
    }

    /// `proof` of the claim with the program's functions unfolded, folded
    /// back into `normal`.
    fn folded(&mut self, normal: &Term, proof: Proof) -> Option<Proof> {
        let mut proof = proof;
        for (id, _) in self.program_calls(normal) {
            proof = derive::fold(&mut self.ctx, id, &proof, normal).ok()?;
        }
        Some(proof)
    }

    fn folded_text(&self, normal: &Term, inner: &str) -> String {
        self.program_calls(normal)
            .iter()
            .fold(inner.to_string(), |text, (_, name)| {
                format!("fold!({name}, {text})")
            })
    }

    /// A fact carried across an equation in scope, as `rewrite!` writes it.
    fn by_rewriting(
        &mut self,
        normal: &Term,
        candidates: &[Candidate],
        opened_candidates: &[Candidate],
    ) -> Option<String> {
        let equations: Vec<&Candidate> = candidates
            .iter()
            .filter(|candidate| matches!(&candidate.claim, Term::Eq(ty, ..) if !matches!(ty, Type::Prop)))
            .collect();
        for equation in &equations {
            let Term::Eq(ty, a, b) = &equation.claim else {
                continue;
            };
            let flipped = ty
                .as_machine()
                .map(|machine| self.theory.machine(machine).eq_symm)
                .map(|symm| {
                    (
                        Proof::OfTerm(Term::call(
                            Term::Fn(symm),
                            vec![
                                (**a).clone(),
                                (**b).clone(),
                                Term::proof(equation.proof.clone()),
                            ],
                        )),
                        format!(
                            "{}_eq_symm({}, {}, {})",
                            ty.as_machine().map_or("", MachineInt::name),
                            self.show(a),
                            self.show(b),
                            equation.text
                        ),
                        (**b).clone(),
                        (**a).clone(),
                    )
                });
            let directions = std::iter::once((
                equation.proof.clone(),
                equation.text.clone(),
                (**a).clone(),
                (**b).clone(),
            ))
            .chain(flipped);
            for (eq, eq_text, from, to) in directions {
                // A fact carried across the equation.
                for target in candidates {
                    let Ok(proof) = derive::rewrite(&mut self.ctx, &eq, &target.proof) else {
                        continue;
                    };
                    if check_proof(&mut self.ctx, &proof, normal).is_ok() {
                        return Some(format!(
                            "this follows from `{}` by `rewrite!({eq_text}, {})`: an equation in scope rewrites nothing by itself",
                            target.text, target.text
                        ));
                    }
                }
                // The claim with `to` put back as `from`, shown some other way.
                if normal.find(&|term| same(term, &to)).is_none() {
                    continue;
                }
                let stated = normal.abstract_over(&|term| same(term, &to)).open(&from);
                let (inner, _) = self.opened(&stated, &[]);
                let Some(found) = self.leaf(&inner, opened_candidates) else {
                    continue;
                };
                let Some(found) = self.folded(&stated, found) else {
                    continue;
                };
                let Ok(proof) = derive::rewrite(&mut self.ctx, &eq, &found) else {
                    continue;
                };
                if check_proof(&mut self.ctx, &proof, normal).is_ok() {
                    let stated = self.show(&stated);
                    return Some(format!(
                        "this is `rewrite!({eq_text}, prove!({stated}))`: an equation in scope rewrites nothing by itself"
                    ));
                }
            }
        }
        None
    }

    /// Whether replacing names by what equations in scope say they equal,
    /// as the removed tier did, would have reached a fact: the equation it
    /// would have used first.
    fn would_rewrite(
        &mut self,
        opened: &Term,
        candidates: &[Candidate],
    ) -> Option<(String, String)> {
        fn is_name(term: &Term) -> bool {
            match term {
                Term::Free(_) => true,
                Term::Proj(target, _) => is_name(target),
                _ => false,
            }
        }
        let mut rewrites: Vec<(Term, Term, (String, String))> = Vec::new();
        for candidate in candidates {
            if let Term::Eq(ty, left, right) = &candidate.claim
                && !matches!(ty, Type::Prop)
                && is_name(left)
                && right.find(&|term| same(term, left)).is_none()
            {
                let text = (candidate.text.clone(), self.show(&candidate.claim));
                rewrites.push(((**left).clone(), (**right).clone(), text));
            }
        }
        let rewritten = |term: &Term| -> (Term, Option<(String, String)>) {
            let mut term = term.clone();
            let mut used = None;
            for _ in 0..STEP_LIMIT {
                let Some((name, value, text)) = rewrites
                    .iter()
                    .find(|(name, _, _)| term.find(&|candidate| same(candidate, name)).is_some())
                else {
                    break;
                };
                term = term
                    .abstract_over(&|candidate| same(candidate, name))
                    .open(value);
                used.get_or_insert_with(|| text.clone());
            }
            (term, used)
        };
        let (goal, used) = rewritten(opened);
        let used = used?;
        let facts: Vec<Term> = candidates
            .iter()
            .map(|candidate| rewritten(&candidate.claim).0)
            .collect();
        if facts.iter().any(|fact| same(fact, &goal))
            || matches!(&goal, Term::Eq(_, left, right) if same(left, right))
        {
            return Some(used);
        }
        let wanted = self.as_test(&goal)?;
        let known = facts.iter().any(|fact| {
            self.as_test(fact).is_some_and(|test| {
                test.outcome == wanted.outcome && same(&test.test, &wanted.test)
            })
        });
        let run = Proof::Evaluate(wanted.test.clone());
        let evaluated = matches!(infer_proof(&mut self.ctx, &run), Ok(Term::Eq(_, _, value)) if *value == Term::Bool(wanted.outcome));
        (known || evaluated).then_some(used)
    }

    /// One application of a callable lemma whose conclusion is the claim
    /// and whose premises are facts in scope.
    fn by_lemma(
        &mut self,
        normal: &Term,
        opened: &Term,
        candidates: &[Candidate],
    ) -> Option<String> {
        let lemmas = self.builtin_lemmas();
        for (name, id) in lemmas {
            let Some(info) = self.fn_by_id(id) else {
                continue;
            };
            let Type::Proof(conclusion) = &info.result else {
                continue;
            };
            let vars: Vec<VarId> = info.params.iter().map(|param| param.id).collect();
            let mut bindings: Vec<(VarId, Term)> = Vec::new();
            if !unify(conclusion, opened, &vars, &mut bindings) {
                continue;
            }
            // The premises first, since a premise may bind a parameter the
            // conclusion does not mention, as the middle term of `u8_le_trans`.
            let mut premises: Vec<(Term, String, Option<String>)> = Vec::new();
            let mut complete = true;
            for param in &info.params {
                let Type::Proof(premise) = &param.ty else {
                    continue;
                };
                let premise = bindings
                    .iter()
                    .fold((**premise).clone(), |premise, (var, term)| {
                        premise.replace_var(*var, term)
                    });
                let found = candidates.iter().find(|candidate| {
                    let mut extended = bindings.clone();
                    unify(&premise, &candidate.claim, &vars, &mut extended)
                });
                match found {
                    Some(candidate) => {
                        unify(&premise, &candidate.claim, &vars, &mut bindings);
                        premises.push((
                            Term::proof(candidate.proof.clone()),
                            candidate.text.clone(),
                            Some(candidate.text.clone()),
                        ));
                    }
                    // A closed premise, such as `250 <= 255`, is evaluated.
                    None => match self.evaluated(&premise) {
                        Some(proof) if premise.is_closed() => {
                            let text = format!("prove!({})", self.show(&premise));
                            premises.push((Term::proof(proof), text, None));
                        }
                        _ => {
                            complete = false;
                            break;
                        }
                    },
                }
            }
            if !complete {
                continue;
            }
            let mut arguments: Vec<Term> = Vec::new();
            let mut texts: Vec<String> = Vec::new();
            let mut from: Vec<String> = Vec::new();
            let mut premises = premises.into_iter();
            for param in &info.params {
                if matches!(param.ty, Type::Proof(_)) {
                    let (argument, text, source) = premises.next().expect("one per premise");
                    arguments.push(argument);
                    texts.push(text);
                    from.extend(source);
                    continue;
                }
                let Some((_, term)) = bindings.iter().find(|(var, _)| *var == param.id) else {
                    complete = false;
                    break;
                };
                arguments.push(term.clone());
                texts.push(self.show(term));
            }
            if !complete {
                continue;
            }
            let call = Proof::OfTerm(Term::call(Term::Fn(id), arguments));
            let Some(proof) = self.folded(normal, call) else {
                continue;
            };
            if check_proof(&mut self.ctx, &proof, normal).is_err() {
                continue;
            }
            let text = self.folded_text(normal, &format!("{name}({})", texts.join(", ")));
            let from: Vec<String> = from.iter().map(|text| format!("`{text}`")).collect();
            return Some(if from.is_empty() {
                format!("this is `{text}`")
            } else {
                format!("this follows from {} by `{text}`", from.join(" and "))
            });
        }
        None
    }

    /// Whether the search over the connectives would have found it, and the
    /// constructor or form that states the outermost step.
    fn by_structure(&mut self, goal: &Term, candidates: &[Candidate]) -> Option<String> {
        let prelude = self.prelude;
        let form = match goal {
            Term::PropApp(id, _) if *id == prelude.and => {
                "evidence of `p && q` is `And::Intro(_, _)`, one part at a time"
            }
            Term::PropApp(id, _) if *id == prelude.or => {
                "evidence of `p || q` is `Or::Left(_)` or `Or::Right(_)`"
            }
            Term::PropApp(id, _) if *id == prelude.truth => "evidence of `true` is `True::Intro`",
            Term::PropApp(id, _) if *id == prelude.falsehood => {
                "evidence of `false` is a refuted claim applied to its evidence, `h(prove!(p))` for `h: @(!p)`"
            }
            Term::Implies(..) => {
                "evidence of `p => q` is a `math fn` that takes evidence of `p` and returns evidence of `q`, named as a value"
            }
            Term::Forall(..) => {
                "evidence of `forall (x: T) { p }` is a `math fn` with `x` as a parameter, named as a value"
            }
            _ => return None,
        };
        let mut owned: Vec<Candidate> = candidates
            .iter()
            .map(|candidate| Candidate {
                claim: candidate.claim.clone(),
                proof: candidate.proof.clone(),
                text: candidate.text.clone(),
            })
            .collect();
        self.structurally(goal, &mut owned, 0)
            .then(|| format!("the search over `&&`, `||`, `=>` and `forall` was removed; {form}"))
    }

    fn structurally(&mut self, goal: &Term, candidates: &mut Vec<Candidate>, depth: usize) -> bool {
        if self.leaf(goal, candidates).is_some()
            || matches!(self.all_cases(goal, candidates), Cases::Holds)
        {
            return true;
        }
        if depth >= STRUCTURAL_DEPTH {
            return false;
        }
        let prelude = self.prelude;
        match goal {
            Term::PropApp(id, _) if *id == prelude.truth => true,
            Term::PropApp(id, arguments) if *id == prelude.and => {
                self.structurally(&arguments[0], candidates, depth + 1)
                    && self.structurally(&arguments[1], candidates, depth + 1)
            }
            Term::PropApp(id, arguments) if *id == prelude.or => {
                self.structurally(&arguments[0], candidates, depth + 1)
                    || self.structurally(&arguments[1], candidates, depth + 1)
            }
            Term::Implies(premise, conclusion) => {
                let scope = self.ctx.checkpoint();
                let Ok(assumed) = self.ctx.assume((**premise).clone()) else {
                    return false;
                };
                let known = candidates.len();
                self.take_apart(
                    Fact::new(Proof::hyp(assumed), (**premise).clone()),
                    &mut |part| {
                        candidates.push(Candidate {
                            claim: part.claim,
                            proof: part.proof,
                            text: String::new(),
                        });
                    },
                );
                let found = self.structurally(conclusion, candidates, depth + 1);
                candidates.truncate(known);
                self.ctx.rollback(scope);
                found
            }
            Term::Forall(ty, body) => {
                let scope = self.ctx.checkpoint();
                let Ok(variable) = self.ctx.declare_ghost(ty.clone()) else {
                    return false;
                };
                let instance = body.open(&Term::Free(variable));
                let found = self.structurally(&instance, candidates, depth + 1);
                self.ctx.rollback(scope);
                found
            }
            Term::PropApp(id, _) if *id == prelude.falsehood => {
                let refuted: Vec<Term> = candidates
                    .iter()
                    .filter_map(|candidate| match &candidate.claim {
                        Term::Implies(premise, conclusion)
                            if **conclusion == prelude.falsehood_prop() =>
                        {
                            Some((**premise).clone())
                        }
                        _ => None,
                    })
                    .collect();
                refuted
                    .iter()
                    .any(|premise| self.structurally(premise, candidates, depth + 1))
            }
            _ => false,
        }
    }

    /// Evaluates a claim about one unknown byte for all 256 values, under
    /// the facts that speak of that byte alone. Used only to say whether the
    /// claim is refuted, or would have been decided this way.
    fn all_cases(&mut self, goal: &Term, candidates: &[Candidate]) -> Cases {
        let Some(wanted) = self.as_test(goal) else {
            return Cases::Undecided;
        };
        let Some(unknown) = single_byte(&wanted.test) else {
            return Cases::Undecided;
        };
        let relevant: Vec<Test> = candidates
            .iter()
            .filter_map(|candidate| self.as_test(&candidate.claim))
            .filter(|test| single_byte(&test.test).is_some_and(|theirs| same(&theirs, &unknown)))
            .collect();
        // facts => claim, as a bool: `if h1 { if h2 { ... claim } else { true } } else { true }`.
        let choose = |test: &Term, if_false: Term, if_true: Term| {
            Term::case_with(
                test.clone(),
                Type::Bool,
                vec![
                    (Vec::new(), HypId::fresh(), if_false),
                    (Vec::new(), HypId::fresh(), if_true),
                ],
            )
        };
        let conclusion = if wanted.outcome {
            wanted.test.clone()
        } else {
            choose(&wanted.test, Term::Bool(true), Term::Bool(false))
        };
        let body = relevant.iter().rev().fold(conclusion, |rest, test| {
            if test.outcome {
                choose(&test.test, Term::Bool(true), rest)
            } else {
                choose(&test.test, rest, Term::Bool(true))
            }
        });
        let all = Proof::EvaluateAll(body.abstract_over(&|term| same(term, &unknown)));
        match infer_proof(&mut self.ctx, &all) {
            Ok(_) => Cases::Holds,
            Err(KernelError::Refuted(Term::U8(byte))) => Cases::Refuted(unknown, byte),
            Err(_) => Cases::Undecided,
        }
    }
}

/// Matches `pattern`, in which `vars` stand for anything, against `term`,
/// extending `bindings`. A variable already bound must match what it was
/// bound to.
fn unify(pattern: &Term, term: &Term, vars: &[VarId], bindings: &mut Vec<(VarId, Term)>) -> bool {
    let all = |ps: &[Term], ts: &[Term], bindings: &mut Vec<(VarId, Term)>| {
        ps.len() == ts.len() && ps.iter().zip(ts).all(|(p, t)| unify(p, t, vars, bindings))
    };
    match (pattern, term) {
        (Term::Free(var), _) if vars.contains(var) => {
            if let Some((_, bound)) = bindings.iter().find(|(bound, _)| bound == var) {
                return same(bound, term);
            }
            bindings.push((*var, term.clone()));
            true
        }
        (Term::Prim(lp, la), Term::Prim(rp, ra)) => lp == rp && all(la, ra, bindings),
        (Term::Eq(lt, ll, lr), Term::Eq(rt, rl, rr)) => {
            lt == rt && unify(ll, rl, vars, bindings) && unify(lr, rr, vars, bindings)
        }
        (Term::Implies(lp, lc), Term::Implies(rp, rc)) => {
            unify(lp, rp, vars, bindings) && unify(lc, rc, vars, bindings)
        }
        (Term::Call(lc, la), Term::Call(rc, ra)) => {
            unify(lc, rc, vars, bindings) && all(la, ra, bindings)
        }
        (Term::Proj(lt, li), Term::Proj(rt, ri)) => li == ri && unify(lt, rt, vars, bindings),
        (Term::PropApp(li, la), Term::PropApp(ri, ra)) => li == ri && all(la, ra, bindings),
        (Term::Struct(li, la), Term::Struct(ri, ra)) => li == ri && all(la, ra, bindings),
        (Term::Variant(le, li, la), Term::Variant(re, ri, ra)) => {
            le == re && li == ri && all(la, ra, bindings)
        }
        (Term::Tuple(_, la), Term::Tuple(_, ra)) => all(la, ra, bindings),
        _ => same(pattern, term),
    }
}

/// The one unknown byte a test speaks of, when it speaks of exactly one and
/// of nothing else that is not a byte.
fn single_byte(test: &Term) -> Option<Term> {
    let mut unknowns = Vec::new();
    if bytes_in(test, false, &mut unknowns) && unknowns.len() == 1 {
        unknowns.pop()
    } else {
        None
    }
}

fn bytes_in(term: &Term, is_byte: bool, unknowns: &mut Vec<Term>) -> bool {
    match term {
        Term::Bool(_) | Term::U8(_) => true,
        Term::Prim(Prim::Cmp(_, MachineInt::U8) | Prim::Op(_, MachineInt::U8), operands) => {
            operands
                .iter()
                .all(|operand| bytes_in(operand, true, unknowns))
        }
        Term::Case {
            scrutinee,
            result: Type::Bool | Type::U8,
            arms,
        } if arms.iter().all(|arm| arm.binders == 0)
            && matches!(**scrutinee, Term::Prim(Prim::Cmp(_, MachineInt::U8), _)) =>
        {
            bytes_in(scrutinee, false, unknowns)
                && arms
                    .iter()
                    .all(|arm| bytes_in(&arm.body, is_byte, unknowns))
        }
        // Anything else is opaque: an unknown byte where a byte is expected,
        // and otherwise something evaluation cannot range over.
        other => {
            if !is_byte || !other.is_closed() || matches!(other, Term::Case { .. }) {
                return false;
            }
            if !unknowns.iter().any(|known| same(known, other)) {
                unknowns.push(other.clone());
            }
            true
        }
    }
}

fn free_variables(term: &Term) -> Vec<VarId> {
    // `find` visits subterms outermost first; record each and keep looking.
    let seen = std::cell::RefCell::new(Vec::new());
    let _ = term.find(&|candidate| {
        if let Term::Free(id) = candidate {
            seen.borrow_mut().push(*id);
        }
        false
    });
    seen.into_inner()
}
