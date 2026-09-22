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
//!    integers directly, one of machine values as the runtime test it is;
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
//! outside a linear sum, no function is unfolded unless `unfold!` or
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
    Axiom, CmpOp, Integer, MachineInt, Prim, Proof, Term, Type, check_proof, evaluate_primitive,
    infer_proof, same,
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
        // The proofs file first; the tiers on a miss (`stored.rs`).
        let found = self.stored_or(goal, |env| env.attempt(goal));
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
        if self.locked_miss(goal, span) {
            return Err(());
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
        let (normal, mut steps) = self.normalize(goal, &known.definitions);
        let found = self
            .computed_from(&normal, &known)
            .map(|proof| (proof, "computed"))
            .or_else(|| self.evaluated(&normal).map(|proof| (proof, "evaluation")));
        if let Some((proof, tier)) = found {
            return Some((self.back_to_stated(proof, steps)?, tier));
        }
        // The arithmetic procedure reads a literal, not the view of one.
        let (linear, more) = self.literal_views(&normal);
        steps.extend(more);
        let proof = self.by_arithmetic(&linear, &known).ok()?;
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
        for _ in 0..STEP_LIMIT {
            let Some(redex) = term.find(&|candidate| redex(candidate)).cloned() else {
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

    /// Reads a claim as the outcome of a runtime test, when it is one: the
    /// claim `c == true` or `c == false` about a comparison `c`; `a ==[T] b`
    /// at a machine type, which `eq[T](a, b)` decides; and the order of two
    /// views, `int_le(view[T](a), view[T](b))` for `le[T]`, with `int_add(
    /// view[T](a), 1i)` on the left for `lt[T]`, and an `Int` literal in
    /// the range of `T` in place of either view, which computing a view of
    /// a literal leaves behind; each also negated, as `P => False`.
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
                comparison_of(premise).map(|test| Test {
                    test,
                    outcome: false,
                })
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

/// Whether a computation axiom applies at the head of the term: a
/// projection of a written tuple or struct, a match on a written
/// constructor, or a primitive on literals. A call of a function is not
/// computed: that is unfolding, which only `unfold!` and `fold!` do. The
/// view of a literal is not computed either: `x <= 3` between machine
/// values is `int_le(view[T](x), view[T](3))`, and it stays in that shape,
/// the one the lemmas about `T` and the reflection of a test speak in, so
/// that a claim reads back as it was written.
fn computes(term: &Term) -> bool {
    if !term.is_closed() {
        return false;
    }
    match term {
        Term::Proj(target, _) => matches!(**target, Term::Tuple(..) | Term::Struct(..)),
        Term::Prim(Prim::View(_), _) => false,
        Term::Prim(prim, operands) => evaluate_primitive(*prim, operands).is_some(),
        Term::Case { scrutinee, .. } => {
            matches!(**scrutinee, Term::Bool(_) | Term::Variant(..))
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
        Proof::CaseData { arms, .. } => arms.iter().map(arm).sum(),
        Proof::ExistsElim {
            exists, arm: one, ..
        } => certificate_pairs(exists) + arm(one),
        Proof::ForStep { lower, upper, .. } => certificate_pairs(lower) + certificate_pairs(upper),
        Proof::NatInduction { base, step, .. } | Proof::IntInduction { base, step, .. } => {
            certificate_pairs(base) + arm(step)
        }
        Proof::Hyp(_)
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
        | Proof::EvaluateAll(_)
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
