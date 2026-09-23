//! Filling a `_`: evidence of exactly the stated claim, after computing,
//! or a certificate of linear arithmetic.
//!
//! A hole is filled in one of four ways, tried in order, and no other:
//!
//! 1. **exact**: a fact in scope is the claim;
//! 2. **computed**: a fact in scope is the claim after *computing* both. A
//!    name bound by `let` (or by a pattern) is replaced by what it stands
//!    for; a projection of a written tuple or struct, a match on a written
//!    constructor, and arithmetic on literals are carried out; a claim
//!    `a == a` holds by reflexivity; and a comparison the branch taken
//!    knows `== true` or `== false` is the fact of that comparison (the
//!    kernel's `cmp_reflect`, which speaks of the views). Evidence of
//!    `p && q` is evidence of each part. Every step is fixed by the claim
//!    and the facts, so this is a procedure and not a search;
//! 3. **evaluation**: a closed comparison is run by the kernel: one of
//!    integers directly, one of machine values as the runtime test it is,
//!    or equality of proof-free data by evaluating both sides;
//! 4. **arithmetic**: a comparison or equation over `Int`, or a comparison
//!    of machine values, which is one over their views, is handed to the
//!    arithmetic procedure of `src/arith` with the facts in scope, each
//!    read as it stands and over the views (`arithmetic.rs`). What comes
//!    back is a certificate of the kernel's rule `linear`, a sum of the
//!    facts with coefficients, and nothing else: the procedure searches for
//!    the coefficients, and the kernel checks the sum. An operator's
//!    obligation under `no_panic` takes the same four tiers.
//!
//! Nothing else happens by itself: no fact is used to reach another
//! outside a linear sum, symbolic calls are unfolded only when `unfold!` or
//! `fold!` asks, and no claim is decided by trying every byte. Those steps
//! are written out, as a lemma call, a `prove!` stepping stone, or one of
//! the proof forms; `explain` says which when a hole stays open, and shows
//! the values the procedure found against the claim when it has them.
//!
//! Whatever is found is an explicit proof, which the kernel checks here
//! before it is used and again when the function is declared.

use std::time::Instant;

use crate::kernel::derive::symm;
use crate::kernel::{
    Axiom, CmpOp, Context, Integer, MachineInt, Prim, Proof, Term, Type, check_proof,
    evaluate_primitive, infer_proof, same,
};
use crate::source::Span;

use super::env::{Elab, Env, Fact};
use super::items::{FoundProof, HoleReport};

pub(super) use crate::limits::MAX_NORMALIZATION_STEPS as STEP_LIMIT;

/// One computing step: the term before is `template[a]` and the term after
/// is `template[b]`, where `eq` proves `a == b`.
#[derive(Clone)]
pub(super) struct Step {
    pub eq: Proof,
    pub template: Term,
}

/// A `let` equation read as what computing replaces `name` by.
pub(super) struct Definition {
    pub name: Term,
    pub value: Term,
    pub eq: Proof,
}

/// Carries evidence of a claim along the steps that computed the claim.
pub(super) fn forward(proof: Proof, steps: Vec<Step>) -> Proof {
    steps
        .into_iter()
        .fold(proof, |proof, step| Proof::Transport {
            eq: Box::new(step.eq),
            template: step.template,
            proof: Box::new(proof),
        })
}

/// A claim read as the outcome of a test: `test == outcome`, where `test`
/// is a runtime comparison at a machine type, `Prim::Cmp`.
#[derive(Clone)]
pub(super) struct Test {
    pub test: Term,
    pub outcome: bool,
}

impl Test {
    /// The comparison, its type, and its operands.
    pub fn parts(&self) -> Option<(CmpOp, MachineInt, &Term, &Term)> {
        match &self.test {
            Term::Prim(Prim::Cmp(op, ty), operands) => match operands.as_slice() {
                [a, b] => Some((*op, *ty, a, b)),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether a fact about `other` decides this test: the same comparison
    /// with the same outcome.
    pub fn decided_by(&self, other: &Test) -> bool {
        self.outcome == other.outcome && same(&self.test, &other.test)
    }
}

/// The facts in scope, computed: what a hole is filled from.
pub(super) struct Known {
    /// Every fact that is not a definition, computed and taken apart, with
    /// the index of the fact in scope it came from.
    pub facts: Vec<(usize, Fact)>,
    pub definitions: Vec<Definition>,
}

impl Env<'_> {
    /// Evidence for `goal`, or a diagnostic. `given` is the claim of the
    /// evidence the programmer supplied, when this is a conversion.
    pub fn solve(&mut self, goal: &Term, span: Span, given: Option<&Term>) -> Elab<Proof> {
        let started = Instant::now();
        let checkpoint = crate::measurement::checkpoint();
        self.normalization_exhausted = false;
        // The proofs file first; the tiers on a miss (`stored.rs`).
        let found = self.stored_or(goal, |env| env.attempt(goal));
        let (proof, tier) = match found {
            Some((proof, tier)) => {
                let _timer = crate::measurement::start("kernel");
                (
                    check_proof(&mut self.ctx, &proof, goal)
                        .ok()
                        .map(|()| proof),
                    tier,
                )
            }
            None => (None, "unsolved"),
        };
        self.holes.push(HoleReport {
            span,
            solved: proof.is_some(),
            tier,
            proof_size: proof.as_ref().map_or(0, proof_size),
            micros: started.elapsed().as_micros(),
            measurements: crate::measurement::since(checkpoint),
            found: proof.as_ref().map(|proof| FoundProof {
                context: self.ctx.clone(),
                claim: goal.clone(),
                proof: proof.clone(),
            }),
        });
        if let Some(proof) = proof {
            return Ok(proof);
        }
        if self.locked_miss(goal, span) {
            return Err(());
        }
        self.report_unsolved(goal, span, given);
        if self.normalization_exhausted
            && let Some(diagnostic) = self.diagnostics.last_mut()
        {
            diagnostic.notes.push(format!("proof construction reached MAX_NORMALIZATION_STEPS ({STEP_LIMIT}); supply a smaller explicit proof step"));
        }
        Err(())
    }

    /// Native read-over-push equality is a kernel computation, with both
    /// reads' bound proofs checked. Lift it through an integer observation
    /// by congruence; no extra arithmetic fact is assumed here.
    fn buffer_read_equation(&mut self, goal: &Term) -> Option<Proof> {
        let Term::Eq(ty, left, right) = goal else {
            return None;
        };
        let checked = |ctx: &mut Context, claim: &Term| {
            let proof = Proof::BufferStep(claim.clone());
            check_proof(ctx, &proof, claim).ok().map(|()| proof)
        };
        if matches!(&**left, Term::Buffer { .. }) && matches!(&**right, Term::Buffer { .. }) {
            return checked(&mut self.ctx, goal);
        }
        if *ty == Type::Int
            && let (Term::Prim(Prim::View(a), ls), Term::Prim(Prim::View(b), rs)) =
                (&**left, &**right)
            && a == b
            && let ([l], [r]) = (ls.as_slice(), rs.as_slice())
            && matches!(l, Term::Buffer { .. })
            && matches!(r, Term::Buffer { .. })
        {
            let equality = Term::eq(Type::machine(*a), l.clone(), r.clone());
            let equation = checked(&mut self.ctx, &equality)?;
            let proof = Proof::transport(
                equation,
                |hole| Term::eq(Type::Int, (**left).clone(), Term::view(*a, hole)),
                Proof::Refl((**left).clone()),
            );
            return check_proof(&mut self.ctx, &proof, goal)
                .ok()
                .map(|()| proof);
        }
        None
    }

    /// The tiers, in order.
    pub(super) fn attempt(&mut self, goal: &Term) -> Option<(Proof, &'static str)> {
        {
            let _timer = crate::measurement::start("exact");
            if let Some(fact) = self.facts.iter().rev().find(|fact| same(&fact.claim, goal)) {
                return Some((fact.proof.clone(), "exact"));
            }
        }
        let (known, normal, mut steps) = {
            let _timer = crate::measurement::start("computed");
            let known = self.knowledge();
            let (normal, steps) = self.normalize(goal, &known.definitions);
            (known, normal, steps)
        };
        let found = self
            .computed_from(&normal, &known)
            .map(|proof| (proof, "computed"))
            .or_else(|| self.evaluated(&normal).map(|proof| (proof, "evaluation")));
        if let Some((proof, tier)) = found {
            return Some((self.back_to_stated(proof, steps)?, tier));
        }
        if let Some(proof) = self.buffer_read_equation(&normal) {
            return Some((self.back_to_stated(proof, steps)?, "computed"));
        }
        // Runtime arithmetic and its explicit wrapping method have the same
        // checked model on normal return. Use that equation only as a local
        // fallback, keeping ordinary source spellings and exact matches intact.
        let (modeled, model_steps) =
            self.compute_where(&normal, &|term| matches!(term, Term::Prim(Prim::Op(..), _)));
        if !model_steps.is_empty()
            && let Some(proof) = self
                .computed_from(&modeled, &known)
                .or_else(|| self.evaluated(&modeled))
        {
            let mut all_steps = steps;
            all_steps.extend(model_steps);
            return Some((self.back_to_stated(proof, all_steps)?, "computed"));
        }
        if let Term::Eq(Type::Bool, test, outcome) = &normal
            && let Term::Bool(flag) = &**outcome
        {
            let relation = match &**test {
                Term::Prim(Prim::IntCmp(op), operands) => match operands.as_slice() {
                    [a, b] => Some(op.claim(a.clone(), b.clone())),
                    _ => None,
                },
                Term::Prim(Prim::Cmp(op, ty), operands) => match operands.as_slice() {
                    [a, b] => {
                        Some(op.claim(Term::view(*ty, a.clone()), Term::view(*ty, b.clone())))
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some(relation) = relation {
                let relation = if *flag {
                    relation
                } else {
                    self.prelude.not_prop(relation)
                };
                let (evidence, tier) = self.attempt(&relation)?;
                let proof = Proof::implies_elim(
                    Proof::Axiom(Axiom::CmpReify((**test).clone(), *flag)),
                    evidence,
                );
                return Some((self.back_to_stated(proof, steps)?, tier));
            }
        }
        // Negating a held logical comparison is the negation of its
        // reflected relation. Keep the conversion explicit in the proof.
        if let Term::Implies(premise, conclusion) = &normal
            && **conclusion == self.prelude.falsehood_prop()
            && let Term::Eq(Type::Bool, test, outcome) = &**premise
            && **outcome == Term::Bool(true)
            && let Term::Prim(Prim::IntCmp(op), operands) = &**test
            && let [a, b] = operands.as_slice()
        {
            let relation = op.claim(a.clone(), b.clone());
            let negated = self.prelude.not_prop(relation);
            let (refutation, tier) = self.attempt(&negated)?;
            let proof = Proof::implies_intro((**premise).clone(), |held| {
                let relation = Proof::implies_elim(
                    Proof::Axiom(Axiom::CmpReflect((**test).clone(), true)),
                    held,
                );
                Proof::implies_elim(refutation, relation)
            });
            return Some((self.back_to_stated(proof, steps)?, tier));
        }
        if let Term::Eq(ty, a, b) = &normal
            && let Some(machine) = ty.as_machine()
        {
            let relation = Term::eq(
                Type::Int,
                Term::view(machine, (**a).clone()),
                Term::view(machine, (**b).clone()),
            );
            let (evidence, tier) = self.attempt(&relation)?;
            let proof = self.equal_of_views(machine, a, b, evidence);
            return Some((self.back_to_stated(proof, steps)?, tier));
        }
        // The arithmetic procedure reads a literal, not the view of one.
        let (linear, more) = self.literal_views(&normal);
        steps.extend(more);
        for (_, fact) in &known.facts {
            let (claim, fact_steps) = self.literal_views(&fact.claim);
            if same(&claim, &linear) {
                return Some((
                    self.back_to_stated(forward(fact.proof.clone(), fact_steps), steps)?,
                    "computed",
                ));
            }
        }
        let proof = {
            let _timer = crate::measurement::start("arithmetic");
            self.by_arithmetic(&linear, &known).ok()?
        };
        Some((self.back_to_stated(proof, steps)?, "arithmetic"))
    }

    /// A proof of the computed claim, carried back to the claim as stated.
    pub(super) fn back_to_stated(&mut self, proof: Proof, steps: Vec<Step>) -> Option<Proof> {
        let mut proof = proof;
        for step in steps.into_iter().rev() {
            proof = Proof::Transport {
                eq: Box::new(symm(&mut self.ctx, &step.eq).ok()?),
                template: step.template,
                proof: Box::new(proof),
            };
        }
        Some(proof)
    }

    /// Tier 2, on a claim already computed: a computed fact that is the
    /// claim, reflexivity, or the outcome of a comparison the branch taken
    /// knows.
    pub(super) fn computed_from(&mut self, normal: &Term, known: &Known) -> Option<Proof> {
        let _timer = crate::measurement::start("computed");
        if let Some((_, fact)) = known
            .facts
            .iter()
            .rev()
            .find(|(_, fact)| same(&fact.claim, normal))
        {
            return Some(fact.proof.clone());
        }
        if let Term::Eq(_, left, right) = normal
            && same(left, right)
        {
            return Some(Proof::Refl((**left).clone()));
        }
        let wanted = self.as_test(normal)?;
        let candidates: Vec<(Term, Test, Proof)> = known
            .facts
            .iter()
            .rev()
            .filter_map(|(_, fact)| {
                let test = self.as_test(&fact.claim)?;
                wanted
                    .decided_by(&test)
                    .then(|| (fact.claim.clone(), test, fact.proof.clone()))
            })
            .collect();
        for (claim, test, proof) in candidates {
            let Some(evidence) = self.test_evidence(&claim, &test, proof) else {
                continue;
            };
            if let Some(proof) = self.reflect_test(normal, &test, evidence) {
                return Some(proof);
            }
        }
        None
    }

    /// Tier 3: a closed comparison, run. A comparison of integers, `int_le`
    /// or `==` at `Int`, is decided by the kernel as it stands, its
    /// negation included; a comparison of machine values is run as the
    /// runtime test it is and reflected.
    pub(super) fn evaluated(&mut self, normal: &Term) -> Option<Proof> {
        let _timer = crate::measurement::start("evaluation");
        let inner = match normal {
            Term::Implies(premise, conclusion) if **conclusion == self.prelude.falsehood_prop() => {
                &**premise
            }
            other => other,
        };
        let integers = matches!(inner, Term::Prim(Prim::IntLe, _) | Term::Eq(Type::Int, ..));
        if integers && inner.is_closed() {
            let run = Proof::Evaluate(inner.clone());
            return match infer_proof(&mut self.ctx, &run) {
                Ok(claim) if same(&claim, normal) => Some(run),
                _ => None,
            };
        }
        // Closed proof-free data can be compared by evaluating both sides.
        // The kernel refuses proof-bearing results, and the final equality is
        // composed from checked computation equations, never an unchecked test.
        if let Term::Eq(_, left, right) = normal {
            let lhs = Proof::Evaluate((**left).clone());
            let rhs = Proof::Evaluate((**right).clone());
            if let (Ok(Term::Eq(_, _, l)), Ok(Term::Eq(_, _, r))) = (
                infer_proof(&mut self.ctx, &lhs),
                infer_proof(&mut self.ctx, &rhs),
            ) && same(&l, &r)
            {
                let backwards = crate::kernel::derive::symm(&mut self.ctx, &rhs).ok()?;
                let result = crate::kernel::derive::trans(&mut self.ctx, &lhs, &backwards).ok()?;
                if check_proof(&mut self.ctx, &result, normal).is_ok() {
                    return Some(result);
                }
            }
        }
        let wanted = self.as_test(normal)?;
        let run = Proof::Evaluate(wanted.test.clone());
        match infer_proof(&mut self.ctx, &run) {
            Ok(Term::Eq(_, _, value)) if *value == Term::Bool(wanted.outcome) => {
                self.reflect_test(normal, &wanted, run)
            }
            _ => None,
        }
    }

    // --- Computing --------------------------------------------------------------

    /// The term with projections, matches and literal arithmetic computed,
    /// and no name replaced.
    pub fn computed(&mut self, term: &Term) -> Term {
        self.compute(term).0
    }

    /// What is known, computed: the facts other than definitions, each
    /// with its names replaced and its conjunctions taken apart, and the
    /// definitions themselves.
    pub(super) fn knowledge(&mut self) -> Known {
        let definitions: Vec<Definition> = self
            .facts
            .iter()
            .filter(|fact| fact.definition)
            .filter_map(|fact| match &fact.claim {
                Term::Eq(_, name, value) => Some(Definition {
                    name: (**name).clone(),
                    value: (**value).clone(),
                    eq: fact.proof.clone(),
                }),
                _ => None,
            })
            .collect();
        let mut facts = Vec::new();
        for (index, fact) in self.facts.clone().into_iter().enumerate() {
            if fact.definition {
                continue;
            }
            let (claim, steps) = self.normalize(&fact.claim, &definitions);
            let computed = Fact::new(forward(fact.proof, steps), claim);
            // Keep an explicitly supplied conjunction as well as its parts.
            // A beta-reduced predicate can ask for the same whole claim;
            // recognizing that value does not synthesize a conjunction.
            if matches!(&computed.claim, Term::PropApp(id, _) if *id == self.prelude.and) {
                facts.push((index, computed.clone()));
            }
            self.take_apart(computed, &mut |part| {
                if let Term::Eq(Type::Bool, test, outcome) = &part.claim
                    && let Term::Bool(flag) = &**outcome
                {
                    let relation = match &**test {
                        Term::Prim(Prim::IntCmp(op), operands) => match operands.as_slice() {
                            [a, b] => Some(op.claim(a.clone(), b.clone())),
                            _ => None,
                        },
                        Term::Prim(Prim::Cmp(op, ty), operands) => match operands.as_slice() {
                            [a, b] => Some(
                                op.claim(Term::view(*ty, a.clone()), Term::view(*ty, b.clone())),
                            ),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(relation) = relation {
                        let relation = if *flag {
                            relation
                        } else {
                            self.prelude.not_prop(relation)
                        };
                        let proof = Proof::implies_elim(
                            Proof::Axiom(Axiom::CmpReflect((**test).clone(), *flag)),
                            part.proof.clone(),
                        );
                        facts.push((index, Fact::new(proof, relation)));
                    }
                }
                if let Term::Implies(premise, conclusion) = &part.claim
                    && **conclusion == self.prelude.falsehood_prop()
                    && let Term::Eq(Type::Bool, test, outcome) = &**premise
                    && **outcome == Term::Bool(true)
                    && let Term::Prim(Prim::IntCmp(op), operands) = &**test
                    && let [a, b] = operands.as_slice()
                {
                    let relation = op.claim(a.clone(), b.clone());
                    let proof = Proof::implies_intro(relation.clone(), |relation| {
                        let held = Proof::implies_elim(
                            Proof::Axiom(Axiom::CmpReify((**test).clone(), true)),
                            relation,
                        );
                        Proof::implies_elim(part.proof.clone(), held)
                    });
                    facts.push((index, Fact::new(proof, self.prelude.not_prop(relation))));
                }
                facts.push((index, part));
            });
        }
        Known { facts, definitions }
    }

    /// Evidence of `p && q` is evidence of `p` and evidence of `q`.
    pub(super) fn take_apart(&self, fact: Fact, out: &mut impl FnMut(Fact)) {
        match &fact.claim {
            Term::PropApp(id, arguments) if *id == self.prelude.and => {
                for (index, part) in arguments.iter().enumerate() {
                    let proof = Proof::CaseProof {
                        scrutinee: Box::new(fact.proof.clone()),
                        goal: part.clone(),
                        arms: vec![Proof::arm(2, 0, |parts, _| {
                            Proof::OfTerm(parts[index].clone())
                        })],
                    };
                    self.take_apart(Fact::new(proof, part.clone()), out);
                }
            }
            _ => out(fact),
        }
    }

    /// Replaces names by what they stand for and computes, until neither
    /// applies.
    /// Keep a normalization pass within its counted certificate-step ceiling.
    /// Declining a whole checked rewrite preserves the earlier valid term; a
    /// failed search reports the exhaustion instead of silently truncating it.
    fn record_normalization(
        &mut self,
        term: &mut Term,
        steps: &mut Vec<Step>,
        next: Term,
        more: Vec<Step>,
    ) -> bool {
        if more.len() > STEP_LIMIT.saturating_sub(steps.len()) {
            self.normalization_exhausted = true;
            return false;
        }
        *term = next;
        steps.extend(more);
        true
    }

    pub(super) fn normalize(
        &mut self,
        term: &Term,
        definitions: &[Definition],
    ) -> (Term, Vec<Step>) {
        let mut term = term.clone();
        let mut steps = Vec::new();
        loop {
            let before = steps.len();
            // A value may mention a name defined later in the list, so go
            // round until nothing changes; the limit bounds the work.
            let mut changed = true;
            while changed && steps.len() < STEP_LIMIT {
                changed = false;
                for definition in definitions {
                    if same(&definition.name, &definition.value)
                        || term
                            .find(&|candidate| same(candidate, &definition.name))
                            .is_none()
                    {
                        continue;
                    }
                    let (rewritten, more) = self.rewrite_normal_step(&term, &definition.eq);
                    if !more.is_empty() {
                        if !self.record_normalization(&mut term, &mut steps, rewritten, more) {
                            return (term, steps);
                        }
                        changed = true;
                        if steps.len() == STEP_LIMIT {
                            break;
                        }
                    }
                }
            }
            let (computed, more) = self.compute(&term);
            if !self.record_normalization(&mut term, &mut steps, computed, more) {
                return (term, steps);
            }
            if steps.len() == before || steps.len() >= STEP_LIMIT {
                self.normalization_exhausted |= steps.len() >= STEP_LIMIT;
                return (term, steps);
            }
        }
    }

    /// Rewrite data together with proof arguments whose types mention it.
    /// Every dependent congruence and motive is checked before recording it.
    fn rewrite_normal_step(&mut self, term: &Term, equation: &Proof) -> (Term, Vec<Step>) {
        let Ok(Term::Eq(ty, from, to)) = infer_proof(&mut self.ctx, equation) else {
            return (term.clone(), vec![]);
        };
        if same(&from, &to) {
            return (term.clone(), vec![]);
        }
        let mut term = term.clone();
        let mut steps = Vec::new();
        for iteration in 0..STEP_LIMIT {
            // Work from the innermost operation outward: replacing a buffer
            // inside a call first retargets the buffer's bound evidence.
            let candidates = std::cell::RefCell::new(Vec::new());
            term.find(&|candidate| {
                let dependent = match candidate {
                    Term::Call(_, args)
                    | Term::Buffer {
                        arguments: args, ..
                    } => args.iter().any(|a| matches!(a, Term::Proof(_))),
                    _ => false,
                };
                if dependent && candidate.find(&|t| same(t, &from)).is_some() {
                    candidates.borrow_mut().push(candidate.clone());
                }
                false
            });
            let mut rewritten = None;
            for operation in candidates.into_inner().into_iter().rev() {
                let congruence = match &operation {
                    Term::Buffer { .. } => {
                        rewrite_buffer_arguments(&mut self.ctx, equation, &operation)
                    }
                    _ => crate::kernel::derive::rewrite_call_arguments(
                        &mut self.ctx,
                        equation,
                        &operation,
                    ),
                };
                let Ok(eq) = congruence else { continue };
                let Ok(Term::Eq(ty, _, replacement)) = infer_proof(&mut self.ctx, &eq) else {
                    continue;
                };
                if same(&operation, &replacement) {
                    continue;
                }
                let template = term.abstract_over(&|candidate| same(candidate, &operation));
                if crate::kernel::check_type(
                    &mut self.ctx,
                    &Type::Fn(vec![ty], Box::new(Type::proof(template.clone()))),
                )
                .is_err()
                {
                    continue;
                }
                rewritten = Some((template.open(&replacement), Step { eq, template }));
                break;
            }
            let Some((next, step)) = rewritten else { break };
            term = next;
            steps.push(step);
            self.normalization_exhausted |= iteration + 1 == STEP_LIMIT;
        }
        if steps.len() < STEP_LIMIT && term.find(&|candidate| same(candidate, &from)).is_some() {
            let template = term.abstract_over(&|candidate| same(candidate, &from));
            if crate::kernel::check_type(
                &mut self.ctx,
                &Type::Fn(vec![ty], Box::new(Type::proof(template.clone()))),
            )
            .is_ok()
            {
                term = template.open(&to);
                steps.push(Step {
                    eq: equation.clone(),
                    template,
                });
            }
        }
        self.normalization_exhausted |= steps.len() >= STEP_LIMIT;
        (term, steps)
    }

    /// Open only calls whose defining body exposes a computation step in
    /// the current context. Symbolic recursive calls stay opaque. This lets
    /// an explicit fold align constructor equations without expanding an
    /// unrelated symbolic tail on just one side of an equation.
    pub(super) fn computing_definitions(
        &mut self,
        term: &Term,
        function: crate::kernel::FnId,
        known: &Known,
    ) -> (Term, Vec<Step>) {
        let mut term = term.clone();
        let mut steps = Vec::new();
        for iteration in 0..STEP_LIMIT {
            let calls = std::cell::RefCell::new(Vec::new());
            term.find(&|candidate| {
                if matches!(candidate, Term::Call(callee, _) if **callee == Term::Fn(function))
                    && candidate.is_closed()
                {
                    calls.borrow_mut().push(candidate.clone());
                }
                false
            });
            let mut changed = None;
            for call in calls.into_inner() {
                let definition = Proof::Definition(call.clone());
                let Ok(claim @ Term::Eq(_, _, _)) = infer_proof(&mut self.ctx, &definition) else {
                    continue;
                };
                let Term::Eq(_, _, body) = &claim else {
                    unreachable!()
                };
                let (normal, reductions) = self.reduce_known_cases(&claim, known);
                let Term::Eq(_, left, right) = &normal else {
                    continue;
                };
                if !same(left, &call) || same(body, right) || same(&call, right) {
                    continue;
                }
                let equation = forward(definition, reductions);
                if check_proof(&mut self.ctx, &equation, &normal).is_err() {
                    continue;
                }
                let (next, more) = self.rewrite_normal_step(&term, &equation);
                if !more.is_empty() && !same(&next, &term) {
                    changed = Some((next, more));
                    break;
                }
            }
            let Some((next, more)) = changed else { break };
            if !self.record_normalization(&mut term, &mut steps, next, more) {
                break;
            }
            self.normalization_exhausted |= iteration + 1 == STEP_LIMIT;
            if steps.len() >= STEP_LIMIT {
                break;
            }
        }
        self.normalization_exhausted |= steps.len() >= STEP_LIMIT;
        (term, steps)
    }

    /// Computes projections of written tuples and structs, matches on
    /// written constructors, and arithmetic on literals.
    fn compute(&mut self, term: &Term) -> (Term, Vec<Step>) {
        self.compute_where(term, &computes)
    }

    /// `compute` at the redexes the predicate picks out, each of which
    /// must be one a computation axiom decides: a projection, a case, or
    /// a primitive on literals.
    pub(super) fn compute_where(
        &mut self,
        term: &Term,
        redex: &dyn Fn(&Term) -> bool,
    ) -> (Term, Vec<Step>) {
        let mut term = term.clone();
        let mut steps = Vec::new();
        for iteration in 0..STEP_LIMIT {
            let Some(redex) = term.find(&|candidate| redex(candidate)).cloned() else {
                break;
            };
            let eq = match &redex {
                Term::Buffer { .. } => Proof::BufferStep(redex.clone()),
                Term::Proj(..) => Proof::Projection(redex.clone()),
                Term::Call(callee, _) if matches!(&**callee, Term::Lambda { .. }) => {
                    Proof::Definition(redex.clone())
                }
                Term::Prim(Prim::Op(op, ty), operands)
                    if evaluate_primitive(Prim::Op(*op, *ty), operands).is_none() =>
                {
                    Proof::Axiom(Axiom::OpModel(*op, *ty, operands.clone()))
                }
                Term::Prim(..) => Proof::Literal(redex.clone()),
                Term::Case { scrutinee, .. } if matches!(&**scrutinee, Term::Prim(..)) => {
                    Proof::CaseKnown {
                        term: redex.clone(),
                        equation: Box::new(Proof::Literal((**scrutinee).clone())),
                    }
                }
                _ => Proof::CaseStep(redex.clone()),
            };
            let Ok(Term::Eq(_, _, _value)) = infer_proof(&mut self.ctx, &eq) else {
                break;
            };
            let (next, more) = self.rewrite_normal_step(&term, &eq);
            if more.is_empty() || same(&next, &term) {
                break;
            }
            if !self.record_normalization(&mut term, &mut steps, next, more) {
                break;
            }
            self.normalization_exhausted |= iteration + 1 == STEP_LIMIT;
            if steps.len() >= STEP_LIMIT {
                break;
            }
        }
        self.normalization_exhausted |= steps.len() >= STEP_LIMIT;
        (term, steps)
    }

    // --- Comparisons as tests -----------------------------------------------------

    /// Reads a claim as the outcome of a runtime test, when it is one: the
    /// claim `c == true` or `c == false` about a comparison `c`; `a ==[T] b`
    /// at a machine type, which `eq[T](a, b)` decides; and the order of two
    /// views, `int_le(view[T](a), view[T](b))` for `le[T]`, with `int_add(
    /// view[T](a), 1i)` on the left for `lt[T]`, and an `Int` literal in
    /// the range of `T` in place of either view, which computing a view of
    /// a literal leaves behind; each also negated, as `P => False`.
    /// Remove only the decidable Bool wrapper for diagnostic arithmetic.
    /// This builds no evidence; proof-producing conversions use CmpReflect.
    pub(super) fn diagnostic_relation(&self, claim: &Term) -> Term {
        match claim {
            Term::Eq(Type::Bool, test, outcome) if matches!(&**outcome, Term::Bool(_)) => {
                let relation = match &**test {
                    Term::Prim(Prim::IntCmp(op), args) => match args.as_slice() {
                        [a, b] => Some(op.claim(a.clone(), b.clone())),
                        _ => None,
                    },
                    _ => None,
                };
                match relation {
                    Some(p) if **outcome == Term::Bool(true) => p,
                    Some(p) => self.prelude.not_prop(p),
                    None => claim.clone(),
                }
            }
            Term::Implies(p, q) if **q == self.prelude.falsehood_prop() => {
                self.prelude.not_prop(self.diagnostic_relation(p))
            }
            _ => claim.clone(),
        }
    }

    pub(super) fn as_test(&self, claim: &Term) -> Option<Test> {
        match claim {
            Term::Eq(Type::Bool, test, outcome) => match **outcome {
                Term::Bool(outcome) => Some(Test {
                    test: (**test).clone(),
                    outcome,
                }),
                _ => None,
            },
            Term::Implies(premise, conclusion) if **conclusion == self.prelude.falsehood_prop() => {
                if let Term::Eq(Type::Bool, test, outcome) = &**premise
                    && **outcome == Term::Bool(true)
                {
                    Some(Test {
                        test: (**test).clone(),
                        outcome: false,
                    })
                } else {
                    comparison_of(premise).map(|test| Test {
                        test,
                        outcome: false,
                    })
                }
            }
            claim => comparison_of(claim).map(|test| Test {
                test,
                outcome: true,
            }),
        }
    }

    pub(super) fn test_claim(test: &Test) -> Term {
        Term::eq(Type::Bool, test.test.clone(), Term::Bool(test.outcome))
    }

    /// What `cmp_reflect` says of the test when it comes out as it does:
    /// the proposition over the views, or its negation.
    fn reflected_claim(&self, test: &Test) -> Option<Term> {
        let (op, ty, a, b) = test.parts()?;
        let claim = op.claim(Term::view(ty, a.clone()), Term::view(ty, b.clone()));
        Some(if test.outcome {
            claim
        } else {
            self.prelude.not_prop(claim)
        })
    }

    /// `a ==[T] b` from `view[T](a) ==[Int] view[T](b)`, by the injectivity
    /// of the view.
    pub(super) fn equal_of_views(
        &self,
        ty: MachineInt,
        a: &Term,
        b: &Term,
        views_equal: Proof,
    ) -> Proof {
        Proof::OfTerm(Term::call(
            Term::Fn(self.theory.machine(ty).view_injective),
            vec![a.clone(), b.clone(), Term::proof(views_equal)],
        ))
    }

    /// The converse, by congruence.
    pub(super) fn views_of_equal(ty: MachineInt, a: &Term, equal: Proof) -> Proof {
        let view_a = Term::view(ty, a.clone());
        Proof::transport(
            equal,
            |hole| Term::eq(Type::Int, view_a.clone(), Term::view(ty, hole)),
            Proof::Refl(view_a.clone()),
        )
    }

    /// From evidence of a claim, evidence of `test == outcome`. The claim
    /// is computed, as the facts are; `None` when what the test reflects to
    /// does not compute to it.
    pub(super) fn test_evidence(
        &mut self,
        claim: &Term,
        test: &Test,
        evidence: Proof,
    ) -> Option<Proof> {
        if matches!(claim, Term::Eq(Type::Bool, ..)) {
            return Some(evidence);
        }
        let (op, ty, a, b) = test.parts()?;
        let (a, b) = (a.clone(), b.clone());
        // Evidence of what reflection speaks of, over the views.
        let of_views = match (op, test.outcome) {
            (CmpOp::Eq, true) => Self::views_of_equal(ty, &a, evidence),
            (CmpOp::Eq, false) => {
                // `!(a == b)` gives `!(v(a) == v(b))`: an equality of the
                // views gives one of the values.
                let equal_views = Term::eq(
                    Type::Int,
                    Term::view(ty, a.clone()),
                    Term::view(ty, b.clone()),
                );
                let refute = |views_equal| {
                    Proof::implies_elim(evidence, self.equal_of_views(ty, &a, &b, views_equal))
                };
                Proof::implies_intro(equal_views, refute)
            }
            _ => {
                let reflected = self.reflected_claim(test)?;
                let (computed, steps) = self.compute(&reflected);
                if !same(&computed, claim) {
                    return None;
                }
                self.back_to_stated(evidence, steps)?
            }
        };
        let goal = Self::test_claim(test);
        // In the arm where the test came out the other way, reflection
        // contradicts the evidence.
        let contradiction = |other: Proof| {
            let reflected = Proof::implies_elim(
                Proof::Axiom(Axiom::CmpReflect(test.test.clone(), !test.outcome)),
                other,
            );
            let falsehood = if test.outcome {
                // reflected: claim => False
                Proof::implies_elim(reflected, of_views.clone())
            } else {
                // of_views: claim => False; reflected: claim
                Proof::implies_elim(of_views.clone(), reflected)
            };
            Proof::CaseProof {
                scrutinee: Box::new(falsehood),
                goal: goal.clone(),
                arms: Vec::new(),
            }
        };
        let agree = Proof::arm(0, 1, |_, hyps| hyps[0].clone());
        let differ = Proof::arm(0, 1, |_, hyps| contradiction(hyps[0].clone()));
        Some(Proof::CaseData {
            scrutinee: test.test.clone(),
            goal: goal.clone(),
            // The arms of a bool are `false`, then `true`.
            arms: if test.outcome {
                vec![differ, agree]
            } else {
                vec![agree, differ]
            },
        })
    }

    /// From evidence of `test == outcome`, evidence of the claim, which is
    /// computed. `None` when what the test reflects to does not compute to
    /// the claim.
    pub(super) fn reflect_test(
        &mut self,
        claim: &Term,
        test: &Test,
        evidence: Proof,
    ) -> Option<Proof> {
        if matches!(claim, Term::Eq(Type::Bool, ..)) {
            return Some(evidence);
        }
        let (op, ty, a, b) = test.parts()?;
        let (a, b) = (a.clone(), b.clone());
        let reflected = Proof::implies_elim(
            Proof::Axiom(Axiom::CmpReflect(test.test.clone(), test.outcome)),
            evidence,
        );
        let (stated, proof) = match (op, test.outcome) {
            // An equality of the views is one of the values.
            (CmpOp::Eq, true) => (
                Term::eq(Type::machine(ty), a.clone(), b.clone()),
                self.equal_of_views(ty, &a, &b, reflected),
            ),
            (CmpOp::Eq, false) => {
                let equal = Term::eq(Type::machine(ty), a.clone(), b.clone());
                let proof = Proof::implies_intro(equal.clone(), |equal| {
                    Proof::implies_elim(reflected, Self::views_of_equal(ty, &a, equal))
                });
                (self.prelude.not_prop(equal), proof)
            }
            _ => (self.reflected_claim(test)?, reflected),
        };
        let (computed, steps) = self.compute(&stated);
        same(&computed, claim).then(|| forward(proof, steps))
    }
}

/// The runtime comparison a proposition is decided by, when it is one; see
/// `Env::as_test`.
fn comparison_of(claim: &Term) -> Option<Term> {
    match claim {
        Term::Eq(ty, a, b) => {
            let ty = ty.as_machine()?;
            Some(Term::cmp(CmpOp::Eq, ty, (**a).clone(), (**b).clone()))
        }
        Term::Prim(Prim::IntLe, sides) => {
            let [left, right] = sides.as_slice() else {
                return None;
            };
            // `a < b` is `a + 1 <= b`.
            let (op, left) = match left {
                Term::Prim(Prim::IntAdd, parts) => match parts.as_slice() {
                    [a, Term::Int(one)] if *one == Integer::from(1i64) => (CmpOp::Lt, a),
                    _ => (CmpOp::Le, left),
                },
                _ => (CmpOp::Le, left),
            };
            let (ty, a, b) = machine_sides(left, right)?;
            Some(Term::cmp(op, ty, a, b))
        }
        _ => None,
    }
}

/// Two `Int` terms as the machine values they are the views of: two views
/// at one type, or a view and an `Int` literal in the range of its type.
fn machine_sides(left: &Term, right: &Term) -> Option<(MachineInt, Term, Term)> {
    let view = |term: &Term| match term {
        Term::Prim(Prim::View(ty), operand) => match operand.as_slice() {
            [x] => Some((*ty, x.clone())),
            _ => None,
        },
        _ => None,
    };
    let literal = |ty: MachineInt, term: &Term| match term {
        Term::Int(value) if ty.contains(value) => Some(Term::machine(ty, value.clone())),
        _ => None,
    };
    match (view(left), view(right)) {
        (Some((ty, a)), Some((other, b))) if ty == other => Some((ty, a, b)),
        (Some((ty, a)), None) => Some((ty, a, literal(ty, right)?)),
        (None, Some((ty, b))) => Some((ty, literal(ty, left)?, b)),
        _ => None,
    }
}

/// Buffer operations carry dependent bound proofs. Re-express the operation
/// as an ordinary checked lambda, use call congruence, and beta-reduce both
/// sides. This is only a certificate builder; it adds no trusted rule.
fn rewrite_buffer_arguments(
    ctx: &mut Context,
    equation: &Proof,
    operation: &Term,
) -> Result<Proof, crate::kernel::KernelError> {
    use crate::kernel::{BufferOp, KernelError, Mode, VarId, infer_term};
    let Term::Buffer {
        op,
        element,
        arguments,
    } = operation
    else {
        return Err(KernelError::NoComputationStep(operation.clone()));
    };
    let result = infer_term(ctx, operation, Mode::Logical)?;
    let mut parameters = Vec::new();
    let mut operands = Vec::new();
    let mut parameter = |ty: Type| {
        let id = VarId::fresh();
        let value = if matches!(ty, Type::Proof(_)) {
            Term::proof(Proof::OfTerm(Term::var(id)))
        } else {
            Term::var(id)
        };
        parameters.push((id, ty));
        operands.push(value.clone());
        value
    };
    let source = parameter(Type::Buffer(Box::new(element.clone())));
    match op {
        BufferOp::Get | BufferOp::Set => {
            let index = parameter(Type::Int);
            for claim in crate::kernel::buffer::bounds(element, &source, &index) {
                parameter(Type::proof(claim));
            }
            if *op == BufferOp::Set {
                parameter(element.clone());
            }
        }
        BufferOp::Push => {
            parameter(element.clone());
            parameter(Type::proof(Term::int_lt(
                crate::kernel::buffer::length(element.clone(), source),
                Term::Int(crate::kernel::MachineInt::U64.max()),
            )));
        }
        _ => return Err(KernelError::NoComputationStep(operation.clone())),
    }
    let lambda = Term::lambda_over(
        &parameters,
        &result,
        Term::Buffer {
            op: *op,
            element: element.clone(),
            arguments: operands,
        },
    );
    let call = Term::call(lambda, arguments.clone());
    let congruence = crate::kernel::derive::rewrite_call_arguments(ctx, equation, &call)?;
    let Term::Eq(_, _, changed) = infer_proof(ctx, &congruence)? else {
        unreachable!("checked equality builder")
    };
    let before = crate::kernel::derive::symm(ctx, &Proof::Definition(call))?;
    let result = crate::kernel::derive::trans(ctx, &before, &congruence)?;
    crate::kernel::derive::trans(ctx, &result, &Proof::Definition(*changed))
}

/// Whether a computation axiom applies at the head of the term: a
/// projection of a written tuple or struct, a match on a written
/// constructor, or a primitive on literals. A call of a function is not
/// computed: that is unfolding, which only `unfold!` and `fold!` do. The
/// view of a literal is not computed either: `x <= 3` between machine
/// values is `int_le(view[T](x), view[T](3))`, and it stays in that shape,
/// the one the lemmas about `T` and the reflection of a test speak in, so
/// that a claim reads back as it was written.
fn computes(term: &Term) -> bool {
    if matches!(term, Term::Call(callee, _) if matches!(&**callee, Term::Lambda { .. })) {
        return true;
    }
    if !term.is_closed() {
        return false;
    }
    match term {
        Term::Buffer {
            op: crate::kernel::BufferOp::Length,
            arguments,
            ..
        } => matches!(
            arguments.first(),
            Some(Term::Buffer {
                op: crate::kernel::BufferOp::Literal
                    | crate::kernel::BufferOp::Set
                    | crate::kernel::BufferOp::Push,
                ..
            })
        ),
        Term::Buffer {
            op: crate::kernel::BufferOp::Get,
            arguments,
            ..
        } => matches!(
            (arguments.first(), arguments.get(1)),
            (
                Some(Term::Buffer {
                    op: crate::kernel::BufferOp::Literal,
                    ..
                }),
                Some(Term::Int(_))
            )
        ),
        Term::Proj(target, _) => matches!(**target, Term::Tuple(..) | Term::Struct(..)),
        Term::Prim(Prim::View(_), _) => false,
        Term::Prim(prim, operands) => evaluate_primitive(*prim, operands).is_some(),
        Term::Case { scrutinee, .. } => {
            matches!(**scrutinee, Term::Bool(_) | Term::Variant(..))
                || matches!(&**scrutinee, Term::Prim(prim, operands) if matches!(evaluate_primitive(*prim, operands), Some(Term::Bool(_))))
        }
        _ => false,
    }
}

/// The pairs of the `linear` certificates a proof is built from, added up:
/// the size of what the arithmetic tier found, for a report. An equation
/// is two certificates, and a case split one per case. The proofs behind
/// the pairs are the facts' own and are not counted, so a certificate that
/// uses a fact proved by an earlier certificate counts the pair, not the
/// earlier certificate.
pub fn certificate_pairs(proof: &Proof) -> usize {
    let arm = |arm: &crate::kernel::ProofArm| certificate_pairs(&arm.body);
    match proof {
        Proof::CaseKnown { equation, .. } => certificate_pairs(equation),
        Proof::Linear { pairs, .. } => pairs.len(),
        Proof::Transport { eq, proof, .. } => certificate_pairs(eq) + certificate_pairs(proof),
        Proof::ImpliesIntro { body, .. } | Proof::ForallIntro { body, .. } => {
            certificate_pairs(body)
        }
        Proof::ImpliesElim(left, right) => certificate_pairs(left) + certificate_pairs(right),
        Proof::ForallElim(proof, _) | Proof::ExistsIntro { proof, .. } => certificate_pairs(proof),
        Proof::CaseProof {
            scrutinee, arms, ..
        } => certificate_pairs(scrutinee) + arms.iter().map(arm).sum::<usize>(),
        Proof::CaseData { arms, .. } | Proof::DataInduction { arms, .. } => {
            arms.iter().map(arm).sum()
        }
        Proof::PropInduction {
            scrutinee, arms, ..
        } => certificate_pairs(scrutinee) + arms.iter().map(arm).sum::<usize>(),
        Proof::ExistsElim {
            exists, arm: one, ..
        } => certificate_pairs(exists) + arm(one),
        Proof::ForStep { lower, upper, .. } => certificate_pairs(lower) + certificate_pairs(upper),
        Proof::IntInduction { base, step, .. } => certificate_pairs(base) + arm(step),
        Proof::BufferStep(_)
        | Proof::BufferBound { .. }
        | Proof::Hyp(_)
        | Proof::OfTerm(_)
        | Proof::Refl(_)
        | Proof::Projection(_)
        | Proof::Literal(_)
        | Proof::Definition(_)
        | Proof::CaseStep(_)
        | Proof::Construct { .. }
        | Proof::ExcludedMiddle(_)
        | Proof::ForEmpty(_)
        | Proof::Omitted
        | Proof::Evaluate(_)
        | Proof::Axiom(_) => 0,
    }
}

/// Roughly the number of nodes in a proof: one per constructor in its debug
/// form. The kernel has no size measure of its own, and this one is only
/// reported, never relied on.
pub(super) fn proof_size(proof: &Proof) -> usize {
    let text = format!("{proof:?}");
    let mut nodes = 0;
    let mut previous = ' ';
    for character in text.chars() {
        if character.is_ascii_uppercase() && !previous.is_ascii_alphanumeric() {
            nodes += 1;
        }
        previous = character;
    }
    nodes
}

impl Env<'_> {
    /// Reduce a case using its original checked branch equation. Rewriting its
    /// scrutinee could invalidate proof terms inside the unselected arms.
    pub(super) fn reduce_known_cases(&mut self, term: &Term, known: &Known) -> (Term, Vec<Step>) {
        let (mut term, mut steps) = self.normalize(term, &known.definitions);
        for iteration in 0..STEP_LIMIT {
            if steps.len() >= STEP_LIMIT {
                break;
            }
            let cases = std::cell::RefCell::new(Vec::new());
            term.find(&|candidate| {
                if matches!(candidate, Term::Case { .. }) && candidate.is_closed() {
                    cases.borrow_mut().push(candidate.clone());
                }
                false
            });
            let mut selected = None;
            'candidate: for case in cases.into_inner() {
                let Term::Case { scrutinee, .. } = &case else {
                    unreachable!()
                };
                for (_, fact) in &known.facts {
                    let Term::Eq(_, tested, _) = &fact.claim else {
                        continue;
                    };
                    if !same(tested, scrutinee) {
                        continue;
                    }
                    let eq = Proof::CaseKnown {
                        term: case.clone(),
                        equation: Box::new(fact.proof.clone()),
                    };
                    if let Ok(Term::Eq(_, _, value)) = infer_proof(&mut self.ctx, &eq) {
                        selected = Some((case, *value, eq));
                        break 'candidate;
                    }
                }
            }
            let Some((case, value, eq)) = selected else {
                break;
            };
            let template = term.abstract_over(&|candidate| same(candidate, &case));
            term = template.open(&value);
            steps.push(Step { eq, template });
            let (computed, more) = self.normalize(&term, &known.definitions);
            if !self.record_normalization(&mut term, &mut steps, computed, more) {
                break;
            }
            self.normalization_exhausted |= iteration + 1 == STEP_LIMIT;
            if steps.len() >= STEP_LIMIT {
                break;
            }
        }
        self.normalization_exhausted |= steps.len() >= STEP_LIMIT;
        (term, steps)
    }
}
