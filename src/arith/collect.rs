//! Collecting the constraints of a problem, each with the kernel proof that
//! stands behind it. Nothing enters without one: a hypothesis by `hyp`, a
//! variable of proof type by `of_term`, as evidence passed to a function
//! is declared, and so is each proof field of a variable of tuple type, a
//! part of a conjunction by case analysis on it, the range of a machine
//! view by the range axioms, the quotient and remainder of a division by a
//! literal by the division axioms with their conditions discharged. The
//! claim recorded for each is the one the ordinary checker infers for its
//! proof, never one the procedure states itself.

use crate::kernel::{
    Axiom, Binding, Integer, Prim, Proof, Term, Type, infer_proof, same, telescope_entry,
};

use super::form::{Form, Kind, read_constraint};
use super::{Problem, Search};

/// A constraint with its proof.
#[derive(Clone, Debug)]
pub(super) struct Fact {
    pub(super) proof: Proof,
    /// What the kernel says `proof` proves.
    pub(super) claim: Term,
    pub(super) form: Form,
    pub(super) kind: Kind,
}

impl Search<'_> {
    /// The facts in scope for `problem`: the hypotheses, then, for every
    /// atom that occurs in the goal or in a collected fact, the facts about
    /// that atom, until no atom is left unvisited. Fails only on the atom
    /// budget.
    pub(super) fn collect(&mut self, problem: &mut Problem) -> Result<(), &'static str> {
        let mut evidence: Vec<(Proof, Term)> = Vec::new();
        for binding in self.ctx.bindings() {
            match binding {
                Binding::Hyp { id, prop } => evidence.push((Proof::hyp(id), prop.clone())),
                Binding::Var { id, ty, .. } => carried(&Term::var(id), ty, &mut evidence),
            }
        }
        for (proof, prop) in evidence {
            self.add_hypothesis(problem, proof, &prop);
        }
        let mut visited = 0;
        while visited < problem.atoms.len() {
            if problem.atoms.len() > self.budget.atoms {
                return Err("atoms");
            }
            let atom = problem.atoms.term(visited).clone();
            visited += 1;
            self.add_facts_about(problem, &atom);
        }
        Ok(())
    }

    /// A hypothesis, or each part of a conjunction, if it is a constraint.
    pub(super) fn add_hypothesis(&mut self, problem: &mut Problem, proof: Proof, prop: &Term) {
        if let Some(prelude) = self.prelude
            && let Term::PropApp(id, parts) = prop
            && *id == prelude.and
            && let [left, right] = parts.as_slice()
        {
            // `And` has one variant, `Intro(left: @p, right: @q)`: a case
            // on the proof binds the two parts, and each is a proof term.
            for (which, part) in [(0, left), (1, right)] {
                let projection = Proof::CaseProof {
                    scrutinee: Box::new(proof.clone()),
                    goal: part.clone(),
                    arms: vec![Proof::arm(2, 0, |payload, _| {
                        Proof::OfTerm(payload[which].clone())
                    })],
                };
                self.add_hypothesis(problem, projection, part);
            }
            return;
        }
        self.add_fact(problem, proof);
    }

    /// Adds `proof` as a fact when what it proves is a constraint. The
    /// claim is inferred by the kernel; a proof it refuses is dropped.
    pub(super) fn add_fact(&mut self, problem: &mut Problem, proof: Proof) -> Option<usize> {
        let claim = infer_proof(&mut self.ctx, &proof).ok()?;
        let (form, kind) = read_constraint(&mut problem.atoms, self.prelude, &claim)?;
        problem.facts.push(Fact {
            proof,
            claim,
            form,
            kind,
        });
        Some(problem.facts.len() - 1)
    }

    /// The facts about one atom: the range of a view, and the division
    /// facts of a quotient or remainder by a literal.
    fn add_facts_about(&mut self, problem: &mut Problem, atom: &Term) {
        match atom {
            Term::Prim(Prim::View(ty), arguments) => {
                if let [value] = arguments.as_slice() {
                    self.add_fact(problem, Proof::Axiom(Axiom::ViewLower(*ty, value.clone())));
                    self.add_fact(problem, Proof::Axiom(Axiom::ViewUpper(*ty, value.clone())));
                }
            }
            Term::Prim(Prim::IntDiv | Prim::IntRem, arguments) => {
                if let [dividend, Term::Int(divisor)] = arguments.as_slice()
                    && !divisor.is_zero()
                {
                    self.add_division(problem, dividend, divisor);
                }
            }
            _ => {}
        }
    }

    /// The facts of `a / k` and `a % k` for a literal `k != 0`, once per
    /// pair: the decomposition `a == (a / k) * k + a % k`, the bound on the
    /// remainder away from zero, whose condition on the sign of `k` is
    /// evaluated, and the sign of the remainder when the sign of `a` can
    /// be proved by the procedure itself, with the budget shared; when it
    /// cannot, the bound towards zero. Both `a / k` and `a % k` are atoms
    /// afterwards, so the facts are added at the first of the two seen.
    fn add_division(&mut self, problem: &mut Problem, dividend: &Term, divisor: &Integer) {
        if problem
            .divisions
            .iter()
            .any(|(a, k)| k == divisor && same(a, dividend))
        {
            return;
        }
        problem.divisions.push((dividend.clone(), divisor.clone()));
        let k = Term::Int(divisor.clone());
        let (a, zero) = (dividend.clone(), Term::int(0));
        let mp = Proof::implies_elim;
        self.add_fact(
            problem,
            Proof::Axiom(Axiom::IntDivRem(a.clone(), k.clone())),
        );
        // The bound away from zero and its condition, decided by evaluation.
        let (upper, lower, condition) = if divisor.is_negative() {
            (
                Axiom::IntRemUpperNeg(a.clone(), k.clone()),
                Axiom::IntRemLowerNeg(a.clone(), k.clone()),
                Term::int_lt(k.clone(), zero.clone()),
            )
        } else {
            (
                Axiom::IntRemUpperPos(a.clone(), k.clone()),
                Axiom::IntRemLowerPos(a.clone(), k.clone()),
                Term::int_lt(zero.clone(), k.clone()),
            )
        };
        let evaluated = Proof::Evaluate(condition);
        self.add_fact(problem, mp(Proof::Axiom(upper), evaluated.clone()));
        // The sign of the remainder follows the sign of the dividend.
        let nonneg = Term::int_le(zero.clone(), a.clone());
        if let Some(proof) = self.discharge(problem, &nonneg) {
            self.add_fact(
                problem,
                mp(
                    Proof::Axiom(Axiom::IntRemNonneg(a.clone(), k.clone())),
                    proof,
                ),
            );
            return;
        }
        self.add_fact(problem, mp(Proof::Axiom(lower), evaluated));
        let nonpos = Term::int_le(a.clone(), zero);
        if let Some(proof) = self.discharge(problem, &nonpos) {
            self.add_fact(problem, mp(Proof::Axiom(Axiom::IntRemNonpos(a, k)), proof));
        }
    }

    /// A proof of `condition`: a collected fact that states it exactly, or
    /// else a nested run of the procedure, one level deeper and on the same
    /// counts.
    fn discharge(&mut self, problem: &Problem, condition: &Term) -> Option<Proof> {
        if let Some(fact) = problem
            .facts
            .iter()
            .find(|fact| same(&fact.claim, condition))
        {
            return Some(fact.proof.clone());
        }
        if self.depth >= self.budget.depth {
            return None;
        }
        self.depth += 1;
        let found = self.prove_goal(condition).ok();
        self.depth -= 1;
        found
    }
}

/// The proofs a variable carries, by its type: itself when it is a proof,
/// and the proof fields of a tuple, each about the fields before it. A
/// struct's fields need the declarations, which the kernel keeps to itself,
/// so a struct carries nothing here.
fn carried(value: &Term, ty: &Type, into: &mut Vec<(Proof, Term)>) {
    match ty {
        Type::Proof(claim) => into.push((Proof::OfTerm(value.clone()), (**claim).clone())),
        Type::Tuple(fields) => {
            let earlier: Vec<Term> = (0..fields.len())
                .map(|index| Term::proj(value.clone(), index))
                .collect();
            for index in 0..fields.len() {
                if let Some(field) = telescope_entry(ty, index, &earlier[..index]) {
                    carried(&earlier[index], &field, into);
                }
            }
        }
        _ => {}
    }
}
