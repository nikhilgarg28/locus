//! Filling a `_`: evidence of exactly the stated claim, after computing.
//!
//! A hole is filled in one of three ways, tried in order, and no other:
//!
//! 1. **exact**: a fact in scope is the claim;
//! 2. **computed**: a fact in scope is the claim after *computing* both. A
//!    name bound by `let` (or by a pattern) is replaced by what it stands
//!    for; a projection of a written tuple or struct, a match on a written
//!    constructor, and arithmetic on literals are carried out; a claim
//!    `a == a` holds by reflexivity; and a comparison the branch taken
//!    knows `== true` or `== false` is the fact of that comparison (the
//!    kernel's `Reflect`). Evidence of `p && q` is evidence of each part.
//!    Every step is fixed by the claim and the facts, so this is a
//!    procedure and not a search;
//! 3. **evaluation**: a closed comparison is run by the kernel.
//!
//! Nothing else happens by itself: no fact is used to reach another, no
//! function is unfolded unless `unfold!` or `fold!` asks, and no claim is
//! decided by trying every byte. Those steps are written out, as a lemma
//! call, a `prove!` stepping stone, or one of the proof forms; `explain`
//! says which when a hole stays open.
//!
//! Whatever is found is an explicit proof, which the kernel checks here
//! before it is used and again when the function is declared.

use std::time::Instant;

use crate::kernel::derive::symm;
use crate::kernel::{
    Axiom, Prim, Proof, Term, Type, check_proof, evaluate_primitive, infer_proof, same,
};
use crate::source::Span;

use super::env::{Elab, Env, Fact};
use super::items::{FoundProof, HoleReport};

pub(super) const STEP_LIMIT: usize = 400;

/// One computing step: the term before is `template[a]` and the term after
/// is `template[b]`, where `eq` proves `a == b`.
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

/// A claim read as the outcome of a test: `test == outcome`.
#[derive(Clone)]
pub(super) struct Test {
    pub test: Term,
    pub outcome: bool,
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
        let found = self.attempt(goal);
        let (proof, tier) = match found {
            Some((proof, tier)) => (
                check_proof(&mut self.ctx, &proof, goal)
                    .ok()
                    .map(|()| proof),
                tier,
            ),
            None => (None, "unsolved"),
        };
        self.holes.push(HoleReport {
            span,
            solved: proof.is_some(),
            tier,
            proof_size: proof.as_ref().map_or(0, proof_size),
            micros: started.elapsed().as_micros(),
            found: proof.as_ref().map(|proof| FoundProof {
                context: self.ctx.clone(),
                claim: goal.clone(),
                proof: proof.clone(),
            }),
        });
        if let Some(proof) = proof {
            return Ok(proof);
        }
        self.report_unsolved(goal, span, given);
        Err(())
    }

    /// The tiers, in order.
    fn attempt(&mut self, goal: &Term) -> Option<(Proof, &'static str)> {
        if let Some(fact) = self.facts.iter().rev().find(|fact| same(&fact.claim, goal)) {
            return Some((fact.proof.clone(), "exact"));
        }
        let known = self.knowledge();
        let (normal, steps) = self.normalize(goal, &known.definitions);
        let found = self
            .computed_from(&normal, &known)
            .map(|proof| (proof, "computed"))
            .or_else(|| self.evaluated(&normal).map(|proof| (proof, "evaluation")))?;
        let (proof, tier) = found;
        Some((self.back_to_stated(proof, steps)?, tier))
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
        let found = known.facts.iter().rev().find_map(|(_, fact)| {
            let test = self.as_test(&fact.claim)?;
            (test.outcome == wanted.outcome && same(&test.test, &wanted.test))
                .then(|| self.test_evidence(&fact.claim, &test, fact.proof.clone()))
        })?;
        Some(self.reflect_test(normal, &wanted, found))
    }

    /// Tier 3: a closed comparison, run.
    pub(super) fn evaluated(&mut self, normal: &Term) -> Option<Proof> {
        let wanted = self.as_test(normal)?;
        let run = Proof::Evaluate(wanted.test.clone());
        match infer_proof(&mut self.ctx, &run) {
            Ok(Term::Eq(_, _, value)) if *value == Term::Bool(wanted.outcome) => {
                Some(self.reflect_test(normal, &wanted, run))
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
            self.take_apart(computed, &mut |part| facts.push((index, part)));
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
                    if term
                        .find(&|candidate| same(candidate, &definition.name))
                        .is_none()
                    {
                        continue;
                    }
                    let template =
                        term.abstract_over(&|candidate| same(candidate, &definition.name));
                    term = template.open(&definition.value);
                    steps.push(Step {
                        eq: definition.eq.clone(),
                        template,
                    });
                    changed = true;
                }
            }
            let (computed, more) = self.compute(&term);
            term = computed;
            steps.extend(more);
            if steps.len() == before || steps.len() >= STEP_LIMIT {
                return (term, steps);
            }
        }
    }

    /// Computes projections of written tuples and structs, matches on
    /// written constructors, and arithmetic on literals.
    fn compute(&mut self, term: &Term) -> (Term, Vec<Step>) {
        let mut term = term.clone();
        let mut steps = Vec::new();
        for _ in 0..STEP_LIMIT {
            let Some(redex) = term.find(&|candidate| computes(candidate)).cloned() else {
                break;
            };
            let eq = match &redex {
                Term::Proj(..) => Proof::Projection(redex.clone()),
                Term::Prim(..) => Proof::Literal(redex.clone()),
                _ => Proof::CaseStep(redex.clone()),
            };
            let Ok(Term::Eq(_, _, value)) = infer_proof(&mut self.ctx, &eq) else {
                break;
            };
            let template = term.abstract_over(&|candidate| same(candidate, &redex));
            let next = template.open(&value);
            if next == term {
                break;
            }
            term = next;
            steps.push(Step { eq, template });
        }
        (term, steps)
    }

    // --- Comparisons as tests -----------------------------------------------------

    /// Reads a claim as the outcome of a runtime test, when it is one.
    pub(super) fn as_test(&self, claim: &Term) -> Option<Test> {
        let positive = |claim: &Term| -> Option<Term> {
            let comparison =
                |prim, a: &Term, b: &Term| Term::prim(prim, vec![a.clone(), b.clone()]);
            match claim {
                Term::Eq(Type::U8, a, b) => Some(comparison(Prim::U8Eq, a, b)),
                Term::Call(callee, arguments) => match (&**callee, arguments.as_slice()) {
                    (Term::Fn(id), [a, b]) if *id == self.prelude.u8_le => {
                        Some(comparison(Prim::U8Le, a, b))
                    }
                    (Term::Fn(id), [a, b]) if *id == self.prelude.u8_lt => {
                        Some(comparison(Prim::U8Lt, a, b))
                    }
                    _ => None,
                },
                _ => None,
            }
        };
        match claim {
            Term::Eq(Type::Bool, test, outcome) => match **outcome {
                Term::Bool(outcome) => Some(Test {
                    test: (**test).clone(),
                    outcome,
                }),
                _ => None,
            },
            Term::Implies(premise, conclusion) if **conclusion == self.prelude.falsehood_prop() => {
                positive(premise).map(|test| Test {
                    test,
                    outcome: false,
                })
            }
            claim => positive(claim).map(|test| Test {
                test,
                outcome: true,
            }),
        }
    }

    pub(super) fn test_claim(test: &Test) -> Term {
        Term::eq(Type::Bool, test.test.clone(), Term::Bool(test.outcome))
    }

    /// From evidence of a claim, evidence of `test == outcome`.
    pub(super) fn test_evidence(&self, claim: &Term, test: &Test, evidence: Proof) -> Proof {
        if matches!(claim, Term::Eq(Type::Bool, ..)) {
            return evidence;
        }
        let goal = Self::test_claim(test);
        // In the arm where the test came out the other way, reflection
        // contradicts the evidence.
        let contradiction = |other: Proof| {
            let reflected = Proof::implies_elim(
                Proof::Axiom(Axiom::Reflect(test.test.clone(), !test.outcome)),
                other,
            );
            let falsehood = if test.outcome {
                // reflected: claim => False
                Proof::implies_elim(reflected, evidence.clone())
            } else {
                // evidence: premise => False; reflected: premise
                Proof::implies_elim(evidence.clone(), reflected)
            };
            Proof::CaseProof {
                scrutinee: Box::new(falsehood),
                goal: goal.clone(),
                arms: Vec::new(),
            }
        };
        let agree = Proof::arm(0, 1, |_, hyps| hyps[0].clone());
        let differ = Proof::arm(0, 1, |_, hyps| contradiction(hyps[0].clone()));
        Proof::CaseData {
            scrutinee: test.test.clone(),
            goal: goal.clone(),
            // The arms of a bool are `false`, then `true`.
            arms: if test.outcome {
                vec![differ, agree]
            } else {
                vec![agree, differ]
            },
        }
    }

    /// From evidence of `test == outcome`, evidence of the claim.
    pub(super) fn reflect_test(&self, claim: &Term, test: &Test, evidence: Proof) -> Proof {
        if matches!(claim, Term::Eq(Type::Bool, ..)) {
            return evidence;
        }
        Proof::implies_elim(
            Proof::Axiom(Axiom::Reflect(test.test.clone(), test.outcome)),
            evidence,
        )
    }
}

/// Whether a computation axiom applies at the head of the term: a
/// projection of a written tuple or struct, a match on a written
/// constructor, or a primitive on literals. A call of a function is not
/// computed: that is unfolding, which only `unfold!` and `fold!` do.
fn computes(term: &Term) -> bool {
    if !term.is_closed() {
        return false;
    }
    match term {
        Term::Proj(target, _) => matches!(**target, Term::Tuple(..) | Term::Struct(..)),
        Term::Prim(prim, operands) => evaluate_primitive(*prim, operands).is_some(),
        Term::Case { scrutinee, .. } => {
            matches!(**scrutinee, Term::Bool(_) | Term::Variant(..))
        }
        _ => false,
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
