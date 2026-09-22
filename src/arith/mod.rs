//! The arithmetic procedure: finds certificates for the kernel's rule
//! `linear` (build task K7; the Linear arithmetic section of the kernel
//! contract in `atlas.html`).
//!
//! The procedure is outside the kernel and untrusted. It turns the goal and
//! the facts in scope into linear constraints over `Int`, each with the
//! kernel proof that stands behind it; adds the range of every machine
//! view and the quotient and remainder facts of every division by a literal
//! as axiom instances; eliminates the atoms by Fourier-Motzkin, keeping for
//! every derived inequality the combination of original constraints it came
//! from; and, when a contradiction is derived, hands those multipliers to
//! the kernel as a certificate. The kernel checks it before it is returned,
//! so a proof that comes out of here is one the kernel has accepted.
//!
//! Its effort is limited by counts, never by a clock: the atoms eliminated,
//! the constraints derived, the pairs of the certificate, the bits of any
//! number, the depth of nested runs, and the case splits, so that a file is
//! accepted or rejected the same way on every machine. When it gives up, it
//! says why, lists the constraints it had, and gives a counterexample when
//! it has one, the elimination's own point or a point of a small box: an
//! assignment to the atoms that satisfies every constraint it collected and
//! violates the goal, checked by evaluation before it is reported.
//!
//! One certificate expresses rational infeasibility: the rule never divides
//! and never rounds, and the discreteness of `Int` enters only where a
//! strict comparison is read as a weak one plus one. When that is not
//! enough, because the goal holds over the integers but not the rationals,
//! the elimination's own point says where: at an atom whose value would
//! have to be fractional, and the goal is proved in the two cases
//! `atom <= k` and `k + 1 <= atom`, by `int_le_total`, each case a
//! hypothesis for a smaller search. This is branch and bound, with the
//! number of splits a count like the others.

mod collect;
mod form;
mod fourier;
mod rational;

use std::collections::BTreeMap;
use std::fmt;

use crate::kernel::{
    Axiom, Context, HypRef, Integer, KernelError, MAX_LINEAR_ATOMS, MAX_LINEAR_PAIRS, Prelude,
    Proof, Term, Type, check_proof,
};

use collect::Fact;
use form::{Atoms, Form, Kind, comparison, negation};
use fourier::{Outcome, Row};

/// The counts that limit the procedure's effort. Every one is a count, so
/// that the outcome is the same on every machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    /// The most atoms eliminated, across a run and its nested runs.
    pub eliminations: usize,
    /// The most constraints derived by elimination, across a run and its
    /// nested runs. This is the count that Fourier-Motzkin's blow-up hits.
    pub derived: usize,
    /// The most pairs in the certificate; the rule has its own limit.
    pub pairs: usize,
    /// The most atoms in a problem; the rule has its own limit on a form.
    pub atoms: usize,
    /// The most bits in any coefficient or multiplier.
    pub bits: usize,
    /// The most nested runs, one inside another, that discharge the
    /// conditions of division axioms.
    pub depth: usize,
    /// The most case splits, across a run and its nested runs: each is a
    /// goal proved in the two cases `atom <= k` and `k + 1 <= atom`.
    pub branches: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            eliminations: 1024,
            derived: 16384,
            pairs: MAX_LINEAR_PAIRS,
            atoms: MAX_LINEAR_ATOMS,
            bits: 256,
            depth: 4,
            branches: 64,
        }
    }
}

/// What a run and its nested runs have spent of the shared counts.
#[derive(Clone, Debug, Default)]
pub(crate) struct Spent {
    eliminations: usize,
    derived: usize,
    branches: usize,
}

/// Why the procedure gave up.
#[derive(Clone, Debug)]
pub enum Reason {
    /// The goal is not `int_le(s, t)`, `s ==[Int] t`, or `False`.
    NotLinear(Term),
    /// Every atom was eliminated without a contradiction: the negated goal
    /// and the constraints are consistent over the rationals, so no
    /// certificate exists.
    Consistent,
    /// The named count ran out.
    Budget { name: &'static str, limit: usize },
    /// The certificate found was refused by the kernel. This is a bug in
    /// the procedure, reported rather than trusted.
    Rejected(KernelError),
    /// The goal needs the prelude, which the context does not have.
    NoPrelude,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLinear(goal) => write!(f, "the goal {goal} is not a linear comparison"),
            Self::Consistent => f.write_str(
                "no certificate exists: the constraints and the negated goal are consistent over the rationals",
            ),
            Self::Budget { name, limit } => {
                write!(f, "the budget `{name}` of {limit} ran out")
            }
            Self::Rejected(error) => write!(f, "the kernel refused the certificate: {error}"),
            Self::NoPrelude => f.write_str("the goal needs the prelude's False"),
        }
    }
}

/// The bound of the box a counterexample is sought in when the procedure
/// gave up on a budget and has no point of its own: every atom in
/// `-BOX..=BOX`.
pub const COUNTEREXAMPLE_BOX: i64 = 8;

/// The most atoms the box is searched over; it has `(2 * BOX + 1)^atoms`
/// points.
pub const COUNTEREXAMPLE_ATOMS: usize = 4;

/// A counterexample, or why there is none to report.
#[derive(Clone, Debug)]
pub enum Counterexample {
    /// An assignment to the atoms that satisfies every collected constraint
    /// and violates the goal, checked by evaluating the linear reading of
    /// each. It comes from the elimination, for any number of atoms, or
    /// from the box.
    Found(Vec<(Term, Integer)>),
    /// The box was searched and holds none.
    NoneInBox,
    /// None was sought: the procedure gave up before it had a point, and
    /// the problem has too many atoms for the box to be searched.
    NotSought,
}

/// The procedure gave up: the reason, the constraints it had, and a
/// counterexample when it found one. `Display` is the text a diagnostic
/// prints.
#[derive(Clone, Debug)]
pub struct GaveUp {
    pub reason: Reason,
    /// The constraints collected, as the kernel infers them, in order.
    pub constraints: Vec<Term>,
    pub counterexample: Counterexample,
}

impl fmt::Display for GaveUp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the arithmetic procedure gave up: {}", self.reason)?;
        if self.constraints.is_empty() {
            f.write_str("; it had no constraints")?;
        } else {
            f.write_str("; it had:")?;
            for constraint in &self.constraints {
                write!(f, "\n  {constraint}")?;
            }
        }
        match &self.counterexample {
            Counterexample::Found(assignment) if assignment.is_empty() => {
                f.write_str("\na counterexample: the goal is false as it stands, with no atoms")
            }
            Counterexample::Found(assignment) => {
                f.write_str("\na counterexample: ")?;
                for (index, (atom, value)) in assignment.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{atom} = {value}")?;
                }
                Ok(())
            }
            Counterexample::NoneInBox => write!(
                f,
                "\nno counterexample within -{COUNTEREXAMPLE_BOX}..{COUNTEREXAMPLE_BOX}"
            ),
            Counterexample::NotSought => Ok(()),
        }
    }
}

impl std::error::Error for GaveUp {}

impl GaveUp {
    /// A report with no constraints and no counterexample.
    fn bare(reason: Reason) -> Self {
        Self {
            reason,
            constraints: Vec::new(),
            counterexample: Counterexample::NotSought,
        }
    }
}

/// Proves `goal` from the facts in scope in `ctx`, or gives up.
///
/// The goal is `int_le(s, t)` (which `s < t` abbreviates), `s ==[Int] t`,
/// proved as two certificates joined by `int_le_antisymm`, or the prelude's
/// `False`. The prelude is passed because the kernel does not expose a
/// context's declarations; without it, `False` cannot be named and
/// conjunctions are not split. The proof returned has been accepted by
/// `check_proof` against `goal` in `ctx`.
pub fn prove(
    ctx: &Context,
    prelude: Option<Prelude>,
    goal: &Term,
    budget: &Budget,
) -> Result<Proof, GaveUp> {
    let mut spent = Spent::default();
    let mut search = Search {
        ctx: ctx.clone(),
        prelude,
        budget,
        spent: &mut spent,
        depth: 0,
    };
    search.prove_goal(goal)
}

/// One run of the procedure, with the counts shared by its nested runs.
struct Search<'a> {
    /// A copy of the caller's context, for the kernel calls.
    ctx: Context,
    prelude: Option<Prelude>,
    budget: &'a Budget,
    spent: &'a mut Spent,
    /// How many runs this one is nested inside.
    depth: usize,
}

/// The constraints of one goal.
#[derive(Default)]
struct Problem {
    atoms: Atoms,
    facts: Vec<Fact>,
    /// The `(a, k)` pairs whose division facts were added.
    divisions: Vec<(Term, Integer)>,
}

impl Search<'_> {
    fn prove_goal(&mut self, goal: &Term) -> Result<Proof, GaveUp> {
        // An equation is two inequalities and antisymmetry.
        if let Term::Eq(Type::Int, left, right) = goal {
            let forward = self.prove_goal(&Term::int_le((**left).clone(), (**right).clone()))?;
            let backward = self.prove_goal(&Term::int_le((**right).clone(), (**left).clone()))?;
            let proof = Proof::implies_elim(
                Proof::implies_elim(
                    Proof::Axiom(Axiom::IntLeAntisymm((**left).clone(), (**right).clone())),
                    forward,
                ),
                backward,
            );
            return self.checked(goal, proof, Vec::new());
        }
        let falsehood = self.prelude.map(|prelude| prelude.falsehood_prop());
        let is_false = falsehood.as_ref().is_some_and(|f| f == goal);
        let sides = comparison(goal).map(|(l, r)| (l.clone(), r.clone()));
        if sides.is_none() && !is_false {
            return Err(GaveUp::bare(Reason::NotLinear(goal.clone())));
        }

        // The negated goal is read first, so that its atoms come first;
        // the goal False contributes nothing, and the facts alone must be
        // contradictory.
        let mut problem = Problem::default();
        let negated = sides
            .as_ref()
            .map(|(left, right)| negation(&mut problem.atoms, left, right));
        if let Err(name) = self.collect(&mut problem) {
            let limit = self.budget.atoms;
            return Err(self.gave_up(Reason::Budget { name, limit }, &problem, negated.as_ref()));
        }
        // The rows: each fact under its own index, and the negated goal
        // under the index past the last fact, so that a refutation's
        // multiplier on it is the goal's coefficient.
        let goal_index = problem.facts.len();
        let mut rows = problem.rows();
        if let Some(form) = &negated {
            rows.push(Row::original(goal_index, form.clone(), Kind::Inequality));
        }
        let stages = match fourier::eliminate(rows, self.budget, self.spent) {
            Outcome::Refuted(multipliers) => {
                let goal_coefficient = multipliers
                    .get(&goal_index)
                    .cloned()
                    .unwrap_or_else(Integer::zero);
                let pairs = self.pairs(&problem, &multipliers);
                if pairs.len() > self.budget.pairs {
                    let limit = self.budget.pairs;
                    let reason = Reason::Budget {
                        name: "pairs",
                        limit,
                    };
                    return Err(self.gave_up(reason, &problem, negated.as_ref()));
                }
                let proof = if is_false || !goal_coefficient.is_zero() {
                    Proof::Linear {
                        goal: goal.clone(),
                        goal_coefficient,
                        pairs,
                    }
                } else {
                    // The facts contradict each other without the goal:
                    // the rule wants a positive coefficient on a comparison
                    // goal, so the certificate proves False and the goal
                    // follows by a case with no arms.
                    let Some(falsehood) = falsehood else {
                        return Err(self.gave_up(Reason::NoPrelude, &problem, negated.as_ref()));
                    };
                    Proof::CaseProof {
                        scrutinee: Box::new(Proof::Linear {
                            goal: falsehood,
                            goal_coefficient: Integer::zero(),
                            pairs,
                        }),
                        goal: goal.clone(),
                        arms: Vec::new(),
                    }
                };
                return self.checked(goal, proof, problem.claims());
            }
            Outcome::Budget { name, limit } => {
                return Err(self.gave_up(
                    Reason::Budget { name, limit },
                    &problem,
                    negated.as_ref(),
                ));
            }
            Outcome::Consistent(stages) => stages,
        };

        // Consistent over the rationals, so no single certificate exists.
        // The elimination gives a point: integral, it is a counterexample
        // within the fragment, and the goal is not provable from these
        // facts; fractional at some atom, the goal is proved in the two
        // cases `atom <= floor` and `floor + 1 <= atom`, by int_le_total,
        // each under its case as a hypothesis.
        let (atom, floor) = match fourier::point(&stages, problem.atoms.len()) {
            Ok(point) => {
                let counterexample = problem.checked_point(&point, negated.as_ref());
                return Err(GaveUp {
                    reason: Reason::Consistent,
                    constraints: problem.claims(),
                    counterexample,
                });
            }
            Err(split) => split,
        };
        if self.spent.branches >= self.budget.branches {
            let limit = self.budget.branches;
            let reason = Reason::Budget {
                name: "branches",
                limit,
            };
            return Err(self.gave_up(reason, &problem, negated.as_ref()));
        }
        self.spent.branches += 1;
        let atom = problem.atoms.term(atom).clone();
        let bound = Term::Int(floor);
        let below = Term::int_le(atom.clone(), bound.clone());
        let above = Term::int_lt(bound.clone(), atom.clone());
        let mut arms = Vec::new();
        for case in [below, above] {
            // `case => goal`, with the case assumed in the scratch context
            // while the arm is searched for, so that the kernel reads it as
            // a fact like any other.
            let checkpoint = self.ctx.checkpoint();
            let mut outcome = None;
            let implication = Proof::implies_intro(case.clone(), |hypothesis| {
                let Proof::Hyp(HypRef::Free(id)) = hypothesis else {
                    unreachable!("implies_intro binds a free hypothesis")
                };
                let found = match self.ctx.assume_with(id, case.clone()) {
                    Ok(()) => self.prove_goal(goal),
                    Err(error) => Err(GaveUp::bare(Reason::Rejected(error))),
                };
                let body = found.clone().unwrap_or(Proof::Omitted);
                outcome = Some(found);
                body
            });
            self.ctx.rollback(checkpoint);
            match outcome.expect("the body was built") {
                Ok(_) => arms.push(implication),
                Err(mut gave_up) => {
                    // The report lists this problem's constraints. A
                    // counterexample from the case satisfies them too, since
                    // the case's constraints are these plus the case; when
                    // the case has none, this problem's box may.
                    gave_up.constraints = problem.claims();
                    if !matches!(gave_up.counterexample, Counterexample::Found(_)) {
                        gave_up.counterexample = problem.searched_box(negated.as_ref());
                    }
                    return Err(gave_up);
                }
            }
        }
        let [below, above] = <[Proof; 2]>::try_from(arms).expect("two arms");
        let proof = Proof::CaseProof {
            scrutinee: Box::new(Proof::Axiom(Axiom::IntLeTotal(atom, bound))),
            goal: goal.clone(),
            arms: vec![
                Proof::arm(1, 0, |payload, _| {
                    Proof::implies_elim(below, Proof::OfTerm(payload[0].clone()))
                }),
                Proof::arm(1, 0, |payload, _| {
                    Proof::implies_elim(above, Proof::OfTerm(payload[0].clone()))
                }),
            ],
        };
        self.checked(goal, proof, problem.claims())
    }

    /// The pairs of a certificate: each fact with a multiplier, in order.
    /// A multiplier past the facts, on the negated goal, is not a pair.
    fn pairs(
        &self,
        problem: &Problem,
        multipliers: &BTreeMap<usize, Integer>,
    ) -> Vec<(Proof, Integer)> {
        multipliers
            .iter()
            .filter(|(index, _)| **index < problem.facts.len())
            .map(|(index, multiplier)| (problem.facts[*index].proof.clone(), multiplier.clone()))
            .collect()
    }

    /// The proof, once the kernel has accepted it against the goal.
    fn checked(
        &mut self,
        goal: &Term,
        proof: Proof,
        constraints: Vec<Term>,
    ) -> Result<Proof, GaveUp> {
        match check_proof(&mut self.ctx, &proof, goal) {
            Ok(()) => Ok(proof),
            Err(error) => Err(GaveUp {
                constraints,
                ..GaveUp::bare(Reason::Rejected(error))
            }),
        }
    }

    /// The report of a failure, with a counterexample when one can be found
    /// in the box.
    fn gave_up(&self, reason: Reason, problem: &Problem, negated: Option<&Form>) -> GaveUp {
        GaveUp {
            reason,
            constraints: problem.claims(),
            counterexample: problem.searched_box(negated),
        }
    }
}

impl Problem {
    fn claims(&self) -> Vec<Term> {
        self.facts.iter().map(|fact| fact.claim.clone()).collect()
    }

    /// Every fact as an original row under its own index.
    fn rows(&self) -> Vec<Row> {
        self.facts
            .iter()
            .enumerate()
            .map(|(index, fact)| Row::original(index, fact.form.clone(), fact.kind))
            .collect()
    }

    /// Whether the point satisfies every fact and the negated goal, each
    /// evaluated in its linear reading.
    fn refuted_at(&self, point: &[Integer], negated: Option<&Form>) -> bool {
        let holds = |form: &Form, kind: Kind| {
            let value = form.evaluate(point);
            match kind {
                Kind::Inequality => !value.is_negative(),
                Kind::Equation => value.is_zero(),
            }
        };
        negated.is_none_or(|form| holds(form, Kind::Inequality))
            && self.facts.iter().all(|fact| holds(&fact.form, fact.kind))
    }

    /// The point as a counterexample, once checked; a point that fails the
    /// check is a bug in the elimination, and nothing is reported.
    fn checked_point(&self, point: &[Integer], negated: Option<&Form>) -> Counterexample {
        if !self.refuted_at(point, negated) {
            return Counterexample::NotSought;
        }
        Counterexample::Found(
            point
                .iter()
                .enumerate()
                .map(|(index, value)| (self.atoms.term(index).clone(), value.clone()))
                .collect(),
        )
    }

    /// A counterexample in the box, by exhaustive search when the atoms are
    /// few enough, for a report that has no point from the elimination.
    fn searched_box(&self, negated: Option<&Form>) -> Counterexample {
        let n = self.atoms.len();
        if n > COUNTEREXAMPLE_ATOMS {
            return Counterexample::NotSought;
        }
        let mut point = vec![-COUNTEREXAMPLE_BOX; n];
        loop {
            let values: Vec<Integer> = point.iter().map(|v| Integer::from(*v)).collect();
            if self.refuted_at(&values, negated) {
                return self.checked_point(&values, negated);
            }
            // The next point of the box, as an odometer.
            let mut carry = 0;
            while carry < n {
                point[carry] += 1;
                if point[carry] <= COUNTEREXAMPLE_BOX {
                    break;
                }
                point[carry] = -COUNTEREXAMPLE_BOX;
                carry += 1;
            }
            if carry == n {
                return Counterexample::NoneInBox;
            }
        }
    }
}
