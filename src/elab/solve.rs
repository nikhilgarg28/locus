//! Filling a `_`: a fixed, bounded search for evidence of a stated claim.
//!
//! The search has tiers, tried in order, and nothing else:
//!
//! 1. a fact in scope that is the claim;
//! 2. the same after *normalizing* the claim and every fact: names bound by
//!    `let` are replaced by what they stand for, projections of written
//!    tuples and structs, matches on written constructors, and arithmetic on
//!    literals are computed, and calls of the program's own math functions
//!    are unfolded;
//! 3. reflexivity, conjunction, and closed evaluation;
//! 4. for a claim about one unknown byte, evaluation of all 256 cases under
//!    the known facts about that byte.
//!
//! Every limit counts steps. Whatever is found is an explicit proof, which
//! the kernel checks here before it is used and again when the function is
//! declared.

use std::time::Instant;

use crate::diagnostic::Diagnostic;
use crate::kernel::derive::symm;
use crate::kernel::{
    Axiom, HypId, KernelError, Prim, Proof, Term, Type, check_proof, evaluate_primitive,
    infer_proof, same,
};
use crate::source::Span;

use super::env::{Elab, Env, Fact, substitute};
use super::items::HoleReport;

const STEP_LIMIT: usize = 400;

/// One rewrite: the term before is `template[a]` and the term after is
/// `template[b]`, where `eq` proves `a == b`.
struct Step {
    eq: Proof,
    template: Term,
}

/// An equation in scope read as a rewrite of `name` to `value`.
struct Rewrite {
    /// The fact it came from, which it must not rewrite.
    source: usize,
    name: Term,
    value: Term,
    eq: Proof,
}

/// Carries evidence of a claim along the steps that rewrote the claim.
fn forward(proof: Proof, steps: Vec<Step>) -> Proof {
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
struct Test {
    test: Term,
    outcome: bool,
}

struct Attempt {
    proof: Option<Proof>,
    tier: &'static str,
    /// A byte for which the claim fails although the known facts hold.
    counterexample: Option<(Term, u8)>,
}

impl Env<'_> {
    /// Evidence for `goal`, or a diagnostic. `given` is the claim of the
    /// evidence the programmer supplied, when this is a conversion.
    pub fn solve(&mut self, goal: &Term, span: Span, given: Option<&Term>) -> Elab<Proof> {
        let started = Instant::now();
        let attempt = self.attempt(goal);
        let checked = attempt.proof.and_then(|proof| {
            check_proof(&mut self.ctx, &proof, goal)
                .ok()
                .map(|()| proof)
        });
        self.holes.push(HoleReport {
            span,
            solved: checked.is_some(),
            tier: attempt.tier,
            proof_size: checked
                .as_ref()
                .map_or(0, |proof| format!("{proof:?}").len()),
            micros: started.elapsed().as_micros(),
        });
        if let Some(proof) = checked {
            return Ok(proof);
        }
        self.report_unsolved(goal, span, given, attempt.counterexample);
        Err(())
    }

    fn attempt(&mut self, goal: &Term) -> Attempt {
        if let Some(fact) = self.facts.iter().rev().find(|fact| same(&fact.claim, goal)) {
            return Attempt {
                proof: Some(fact.proof.clone()),
                tier: "fact",
                counterexample: None,
            };
        }
        let (known, rewrites) = self.knowledge();
        let (normal_goal, goal_steps) = self.normalize(goal, &rewrites, None);
        let mut facts: Vec<Fact> = known
            .iter()
            .enumerate()
            .map(|(index, fact)| {
                let (claim, steps) = self.normalize(&fact.claim, &rewrites, Some(index));
                Fact {
                    proof: forward(fact.proof.clone(), steps),
                    claim,
                }
            })
            .collect();
        let mut counterexample = None;
        let (proof, tier) = match self.prove(&normal_goal, &mut facts, &mut counterexample, 0) {
            Some((proof, tier)) => (Some(proof), tier),
            None => (None, "unsolved"),
        };
        // Back from the normalized claim to the claim as stated.
        let proof = proof.and_then(|mut proof| {
            for step in goal_steps.into_iter().rev() {
                proof = Proof::Transport {
                    eq: Box::new(symm(&mut self.ctx, &step.eq).ok()?),
                    template: step.template,
                    proof: Box::new(proof),
                };
            }
            Some(proof)
        });
        Attempt {
            proof,
            tier,
            counterexample,
        }
    }

    // --- Normalization ----------------------------------------------------------

    /// The term with projections, matches and literal arithmetic computed.
    pub fn computed(&mut self, term: &Term) -> Term {
        self.compute(term, false).0
    }

    /// What is known, with every claim computed and conjunctions taken
    /// apart, and the rewrites it gives: the equations `name == value`, read
    /// from left to right, latest first. A name is a variable or a field of
    /// one: what a `let` bound, or a part of what a call returned.
    fn knowledge(&mut self) -> (Vec<Fact>, Vec<Rewrite>) {
        fn is_name(term: &Term) -> bool {
            match term {
                Term::Free(_) => true,
                Term::Proj(target, _) => is_name(target),
                _ => false,
            }
        }
        let mut known = Vec::new();
        for fact in self.facts.clone() {
            let (claim, steps) = self.compute(&fact.claim, true);
            let proof = forward(fact.proof, steps);
            self.take_apart(Fact { proof, claim }, &mut known);
        }
        let rewrites = known
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(source, fact)| match &fact.claim {
                Term::Eq(_, left, right)
                    if is_name(left) && right.find(&|term| same(term, left)).is_none() =>
                {
                    Some(Rewrite {
                        source,
                        name: (**left).clone(),
                        value: (**right).clone(),
                        eq: fact.proof.clone(),
                    })
                }
                _ => None,
            })
            .collect();
        (known, rewrites)
    }

    /// Evidence of `p && q` is evidence of `p` and evidence of `q`.
    fn take_apart(&self, fact: Fact, known: &mut Vec<Fact>) {
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
                    self.take_apart(
                        Fact {
                            proof,
                            claim: part.clone(),
                        },
                        known,
                    );
                }
            }
            _ => known.push(fact),
        }
    }

    /// Rewrites names to what they stand for and computes, until neither
    /// applies. `except` is the fact being normalized, which must not
    /// rewrite itself away.
    fn normalize(
        &mut self,
        term: &Term,
        rewrites: &[Rewrite],
        except: Option<usize>,
    ) -> (Term, Vec<Step>) {
        let mut term = term.clone();
        let mut steps = Vec::new();
        loop {
            let before = steps.len();
            // A value may mention a name defined later in the list, so go
            // round until nothing changes; the limit bounds a cycle.
            let mut changed = true;
            while changed && steps.len() < STEP_LIMIT {
                changed = false;
                for rewrite in rewrites {
                    if Some(rewrite.source) == except
                        || term
                            .find(&|candidate| same(candidate, &rewrite.name))
                            .is_none()
                    {
                        continue;
                    }
                    let template = term.abstract_over(&|candidate| same(candidate, &rewrite.name));
                    term = template.open(&rewrite.value);
                    steps.push(Step {
                        eq: rewrite.eq.clone(),
                        template,
                    });
                    changed = true;
                }
            }
            let (computed, more) = self.compute(&term, true);
            term = computed;
            steps.extend(more);
            if steps.len() == before || steps.len() >= STEP_LIMIT {
                return (term, steps);
            }
        }
    }

    /// Computes projections of written tuples and structs, matches on
    /// written constructors, and arithmetic on literals; with `unfold`, also
    /// calls of the program's own math functions.
    fn compute(&mut self, term: &Term, unfold: bool) -> (Term, Vec<Step>) {
        let mut term = term.clone();
        let mut steps = Vec::new();
        for _ in 0..STEP_LIMIT {
            let Some(redex) = term
                .find(&|candidate| {
                    self.computes(candidate) && (unfold || !matches!(candidate, Term::Call(..)))
                })
                .cloned()
            else {
                break;
            };
            let eq = match &redex {
                Term::Proj(..) => Proof::Projection(redex.clone()),
                Term::Prim(..) => Proof::Literal(redex.clone()),
                Term::Case { .. } => Proof::CaseStep(redex.clone()),
                _ => Proof::Definition(redex.clone()),
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

    /// Whether a computation axiom applies at the head of the term.
    fn computes(&self, term: &Term) -> bool {
        if !term.is_closed() {
            return false;
        }
        match term {
            Term::Proj(target, _) => matches!(**target, Term::Tuple(..) | Term::Struct(..)),
            Term::Prim(prim, operands) => evaluate_primitive(*prim, operands).is_some(),
            Term::Case { scrutinee, .. } => {
                matches!(**scrutinee, Term::Bool(_) | Term::Variant(..))
            }
            // The program's own functions; the prelude's orderings stay folded,
            // because the facts about bytes are stated with them.
            Term::Call(callee, _) => {
                matches!(**callee, Term::Fn(id) if self.fn_by_id(id).is_some())
            }
            _ => false,
        }
    }

    // --- The tiers ----------------------------------------------------------------

    fn prove(
        &mut self,
        goal: &Term,
        facts: &mut Vec<Fact>,
        counterexample: &mut Option<(Term, u8)>,
        depth: usize,
    ) -> Option<(Proof, &'static str)> {
        if let Some(fact) = facts.iter().rev().find(|fact| same(&fact.claim, goal)) {
            return Some((fact.proof.clone(), "fact after normalizing"));
        }
        if let Term::Eq(_, left, right) = goal
            && same(left, right)
        {
            return Some((Proof::Refl((**left).clone()), "reflexivity"));
        }
        if let Some(wanted) = self.as_test(goal)
            && let Some((evidence, tier)) = self.prove_test(&wanted, facts, counterexample)
        {
            return Some((self.reflect_test(goal, &wanted, evidence), tier));
        }
        if depth >= 8 {
            return None;
        }
        let prelude = self.prelude;
        match goal {
            Term::PropApp(id, _) if *id == prelude.truth => Some((
                Proof::Construct {
                    prop: *id,
                    variant: 0,
                    params: Vec::new(),
                    payload: Vec::new(),
                },
                "trivial",
            )),
            Term::PropApp(id, arguments) if *id == prelude.and => {
                let (left, _) = self.prove(&arguments[0], facts, counterexample, depth + 1)?;
                let (right, _) = self.prove(&arguments[1], facts, counterexample, depth + 1)?;
                Some((
                    Proof::Construct {
                        prop: *id,
                        variant: 0,
                        params: arguments.clone(),
                        payload: vec![Term::proof(left), Term::proof(right)],
                    },
                    "conjunction",
                ))
            }
            Term::PropApp(id, arguments) if *id == prelude.or => (0..2).find_map(|side| {
                let (proof, _) = self.prove(&arguments[side], facts, counterexample, depth + 1)?;
                Some((
                    Proof::Construct {
                        prop: *id,
                        variant: side,
                        params: arguments.clone(),
                        payload: vec![Term::proof(proof)],
                    },
                    "disjunction",
                ))
            }),
            // Evidence of `p => q` is evidence of `q` that may use `p`.
            Term::Implies(premise, conclusion) => {
                let scope = self.ctx.checkpoint();
                let assumed = self.ctx.assume((**premise).clone()).ok()?;
                let known = facts.len();
                self.take_apart(
                    Fact {
                        proof: Proof::hyp(assumed),
                        claim: (**premise).clone(),
                    },
                    facts,
                );
                let body = self.prove(conclusion, facts, counterexample, depth + 1);
                facts.truncate(known);
                self.ctx.rollback(scope);
                let (body, _) = body?;
                Some((
                    Proof::implies_intro((**premise).clone(), |given| {
                        substitute(body, &[], &[(assumed, given)])
                    }),
                    "implication",
                ))
            }
            Term::Forall(ty, body) => {
                let scope = self.ctx.checkpoint();
                let variable = self.ctx.declare_ghost(ty.clone()).ok()?;
                let instance = body.open(&Term::Free(variable));
                let proof = self.prove(&instance, facts, counterexample, depth + 1);
                self.ctx.rollback(scope);
                let (proof, _) = proof?;
                Some((
                    Proof::forall_intro(ty.clone(), |given| {
                        substitute(proof, &[(variable, given)], &[])
                    }),
                    "generalization",
                ))
            }
            // `False` follows from a refuted fact whose claim can be shown.
            Term::PropApp(id, _) if *id == prelude.falsehood => {
                let refuted: Vec<Fact> = facts
                    .iter()
                    .filter(|fact| {
                        matches!(&fact.claim, Term::Implies(_, conclusion) if **conclusion == prelude.falsehood_prop())
                    })
                    .cloned()
                    .collect();
                refuted.into_iter().find_map(|fact| {
                    let Term::Implies(premise, _) = &fact.claim else {
                        return None;
                    };
                    let (premise, _) = self.prove(premise, facts, counterexample, depth + 1)?;
                    Some((Proof::implies_elim(fact.proof, premise), "contradiction"))
                })
            }
            _ => None,
        }
    }

    /// Reads a claim as the outcome of a runtime test, when it is one.
    fn as_test(&self, claim: &Term) -> Option<Test> {
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

    fn test_claim(test: &Test) -> Term {
        Term::eq(Type::Bool, test.test.clone(), Term::Bool(test.outcome))
    }

    /// From evidence of a claim, evidence of `test == outcome`.
    fn test_evidence(&self, claim: &Term, test: &Test, evidence: Proof) -> Proof {
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
    fn reflect_test(&self, claim: &Term, test: &Test, evidence: Proof) -> Proof {
        if matches!(claim, Term::Eq(Type::Bool, ..)) {
            return evidence;
        }
        Proof::implies_elim(
            Proof::Axiom(Axiom::Reflect(test.test.clone(), test.outcome)),
            evidence,
        )
    }

    fn prove_test(
        &mut self,
        wanted: &Test,
        facts: &[Fact],
        counterexample: &mut Option<(Term, u8)>,
    ) -> Option<(Proof, &'static str)> {
        let known: Vec<(Test, Proof)> = facts
            .iter()
            .filter_map(|fact| {
                let test = self.as_test(&fact.claim)?;
                let evidence = self.test_evidence(&fact.claim, &test, fact.proof.clone());
                Some((test, evidence))
            })
            .collect();
        if let Some((_, evidence)) = known
            .iter()
            .rev()
            .find(|(test, _)| test.outcome == wanted.outcome && same(&test.test, &wanted.test))
        {
            return Some((evidence.clone(), "fact after normalizing"));
        }
        // A closed test is run.
        let run = Proof::Evaluate(wanted.test.clone());
        if let Ok(Term::Eq(_, _, value)) = infer_proof(&mut self.ctx, &run)
            && *value == Term::Bool(wanted.outcome)
        {
            return Some((run, "evaluation"));
        }
        self.by_cases(wanted, &known, counterexample)
    }

    /// A claim about one unknown byte, by evaluating all 256 cases.
    fn by_cases(
        &mut self,
        wanted: &Test,
        known: &[(Test, Proof)],
        counterexample: &mut Option<(Term, u8)>,
    ) -> Option<(Proof, &'static str)> {
        let mut unknowns = Vec::new();
        if !bytes_of(&wanted.test, &mut unknowns) || unknowns.len() != 1 {
            return None;
        }
        let unknown = unknowns.pop().expect("there is one");
        // The facts that speak of this byte and nothing else.
        let relevant: Vec<&(Test, Proof)> = known
            .iter()
            .filter(|(test, _)| {
                let mut theirs = Vec::new();
                bytes_of(&test.test, &mut theirs) && theirs.len() == 1 && same(&theirs[0], &unknown)
            })
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
        let holds = |test: &Test, rest: Term| {
            if test.outcome {
                choose(&test.test, Term::Bool(true), rest)
            } else {
                choose(&test.test, rest, Term::Bool(true))
            }
        };
        let conclusion = if wanted.outcome {
            wanted.test.clone()
        } else {
            choose(&wanted.test, Term::Bool(true), Term::Bool(false))
        };
        let bodies: Vec<Term> = {
            // bodies[k] is the body from fact k onward; the last is the conclusion.
            let mut bodies = vec![conclusion];
            for (test, _) in relevant.iter().rev() {
                let rest = bodies.last().expect("nonempty").clone();
                bodies.push(holds(test, rest));
            }
            bodies.reverse();
            bodies
        };
        let all = Proof::EvaluateAll(bodies[0].abstract_over(&|term| same(term, &unknown)));
        let mut current = Proof::forall_elim(all, unknown.clone());
        match infer_proof(&mut self.ctx, &current) {
            Ok(_) => {}
            Err(KernelError::Refuted(Term::U8(byte))) => {
                *counterexample = Some((unknown, byte));
                return None;
            }
            Err(_) => return None,
        }
        // current: bodies[0] == true. Discharge each fact in turn.
        let truth = |term: Term| Term::eq(Type::Bool, term, Term::Bool(true));
        for (index, (test, evidence)) in relevant.iter().enumerate() {
            let rest = bodies[index + 1].clone();
            let decided = holds(
                &Test {
                    test: Term::Bool(test.outcome),
                    outcome: test.outcome,
                },
                rest.clone(),
            );
            // Put the known outcome where the test was, then take that arm.
            let rest_for_template = rest.clone();
            let outcome = test.outcome;
            current = Proof::transport(
                (*evidence).clone(),
                |hole| {
                    truth(holds(
                        &Test {
                            test: hole,
                            outcome,
                        },
                        rest_for_template,
                    ))
                },
                current,
            );
            current = Proof::transport(Proof::CaseStep(decided), truth, current);
        }
        if wanted.outcome {
            return Some((current, "all 256 cases"));
        }
        // current: (if test { false } else { true }) == true. Had the test
        // come out true, that would read `false == true`.
        let test = wanted.test.clone();
        let otherwise = Proof::arm(0, 1, |_, hyps| hyps[0].clone());
        let impossible = Proof::arm(0, 1, |_, hyps| {
            let came_out_true = hyps[0].clone();
            let flipped = |scrutinee: Term| choose(&scrutinee, Term::Bool(true), Term::Bool(false));
            let stuck = Proof::transport(
                came_out_true.clone(),
                |hole| truth(flipped(hole)),
                current.clone(),
            );
            let false_is_true =
                Proof::transport(Proof::CaseStep(flipped(Term::Bool(true))), truth, stuck);
            let true_is_false = Proof::transport(
                false_is_true,
                |hole| Term::eq(Type::Bool, hole, Term::Bool(false)),
                Proof::Refl(Term::Bool(false)),
            );
            let test = test.clone();
            Proof::transport(
                true_is_false,
                |hole| Term::eq(Type::Bool, test, hole),
                came_out_true,
            )
        });
        Some((
            Proof::CaseData {
                scrutinee: wanted.test.clone(),
                goal: Self::test_claim(wanted),
                arms: vec![otherwise, impossible],
            },
            "all 256 cases",
        ))
    }

    // --- When nothing works -----------------------------------------------------

    fn report_unsolved(
        &mut self,
        goal: &Term,
        span: Span,
        given: Option<&Term>,
        counterexample: Option<(Term, u8)>,
    ) {
        // The claim as stated, with the values written for its names put in
        // their fields; then the claim with everything computed.
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
        let (known_facts, rewrites) = self.knowledge();
        let (normal, _) = self.normalize(goal, &rewrites, None);
        let normal_text = self.show(&normal);
        if normal_text != claim {
            diagnostic = diagnostic.note(format!("after computing, the claim is `{normal_text}`"));
        }
        if let Some((unknown, byte)) = counterexample {
            let unknown = self.show(&unknown);
            diagnostic = diagnostic.note(format!(
                "it fails when `{unknown}` is {byte}, which the facts known here allow"
            ));
        }
        // The facts that speak of something the claim speaks of.
        let mut subjects = Vec::new();
        free_variables(&normal, &mut subjects);
        let mut known = Vec::new();
        for (index, fact) in known_facts.iter().enumerate().rev() {
            // An equation for a name is shown by substitution, not as a fact.
            if rewrites.iter().any(|rewrite| rewrite.source == index) {
                continue;
            }
            let (normal_fact, _) = self.normalize(&fact.claim, &rewrites, Some(index));
            let mut theirs = Vec::new();
            free_variables(&normal_fact, &mut theirs);
            if !theirs.iter().any(|variable| subjects.contains(variable)) {
                continue;
            }
            let text = self.show(&normal_fact);
            if !known.contains(&text) {
                known.push(text);
            }
            if known.len() == 6 {
                break;
            }
        }
        diagnostic = if known.is_empty() {
            diagnostic.note("nothing known here speaks of the values in this claim")
        } else {
            let list: Vec<String> = known.iter().rev().map(|fact| format!("`{fact}`")).collect();
            diagnostic.note(format!("known here: {}", list.join(", ")))
        };
        self.diagnostics.push(diagnostic);
    }
}

/// Collects the unknown bytes a test speaks of. False when the test also
/// depends on something that is not a byte, which evaluation cannot range
/// over.
fn bytes_of(term: &Term, unknowns: &mut Vec<Term>) -> bool {
    bytes_in(term, false, unknowns)
}

fn bytes_in(term: &Term, is_byte: bool, unknowns: &mut Vec<Term>) -> bool {
    match term {
        Term::Bool(_) | Term::U8(_) => true,
        Term::Prim(
            Prim::U8Eq | Prim::U8Lt | Prim::U8Le | Prim::WrappingAdd | Prim::WrappingSub,
            operands,
        ) => operands
            .iter()
            .all(|operand| bytes_in(operand, true, unknowns)),
        Term::Case {
            scrutinee,
            result: Type::Bool | Type::U8,
            arms,
        } if arms.iter().all(|arm| arm.binders == 0)
            && matches!(
                **scrutinee,
                Term::Prim(Prim::U8Eq | Prim::U8Lt | Prim::U8Le, _)
            ) =>
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

fn free_variables(term: &Term, out: &mut Vec<crate::kernel::VarId>) {
    // `find` visits subterms outermost first; record each and keep looking.
    let seen = std::cell::RefCell::new(Vec::new());
    let _ = term.find(&|candidate| {
        if let Term::Free(id) = candidate {
            seen.borrow_mut().push(*id);
        }
        false
    });
    out.extend(seen.into_inner());
}
