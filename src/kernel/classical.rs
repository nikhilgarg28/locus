//! Records which proofs depend on excluded middle. This is bookkeeping, not
//! checking: it lets a development be audited for classical reasoning.
//! Types are not searched; a proof that merely occurs inside a proposition
//! is part of a statement, not of the derivation.

use super::defs::Definitions;
use super::term::{Proof, ProofArm, Term};

/// Whether the proof uses excluded middle, directly or through a declared
/// function it calls.
pub fn proof_is_classical(definitions: &Definitions, proof: &Proof) -> bool {
    let term = |term: &Term| term_is_classical(definitions, term);
    let sub = |proof: &Proof| proof_is_classical(definitions, proof);
    let arms = |arms: &[ProofArm]| arms.iter().any(|arm| sub(&arm.body));
    match proof {
        Proof::CaseKnown { term, equation } => {
            term_is_classical(definitions, term) || proof_is_classical(definitions, equation)
        }
        Proof::BufferStep(term) | Proof::BufferBound { value: term, .. } => {
            term_is_classical(definitions, term)
        }
        Proof::ExcludedMiddle(_) => true,
        Proof::Hyp(_) | Proof::Omitted => false,
        Proof::OfTerm(inner)
        | Proof::Refl(inner)
        | Proof::Projection(inner)
        | Proof::Literal(inner)
        | Proof::Definition(inner)
        | Proof::CaseStep(inner)
        | Proof::Evaluate(inner) => term(inner),
        Proof::Transport { eq, proof, .. } => sub(eq) || sub(proof),
        Proof::ImpliesIntro { body, .. } | Proof::ForallIntro { body, .. } => sub(body),
        Proof::ImpliesElim(left, right) => sub(left) || sub(right),
        Proof::ForallElim(universal, argument) => sub(universal) || term(argument),
        Proof::Construct {
            params, payload, ..
        } => params.iter().chain(payload).any(term),
        Proof::CaseProof {
            scrutinee,
            arms: cases,
            ..
        } => sub(scrutinee) || arms(cases),
        Proof::CaseData {
            scrutinee,
            arms: cases,
            ..
        } => term(scrutinee) || arms(cases),
        Proof::ExistsIntro { witness, proof, .. } => term(witness) || sub(proof),
        Proof::ExistsElim { exists, arm, .. } => sub(exists) || sub(&arm.body),
        Proof::ForEmpty(inner) => term(inner),
        Proof::ForStep {
            looped,
            lower,
            upper,
        } => term(looped) || sub(lower) || sub(upper),
        Proof::Axiom(axiom) => axiom.terms().into_iter().any(term),
        Proof::PropInduction {
            scrutinee,
            arms: cases,
            ..
        } => sub(scrutinee) || arms(cases),
        Proof::DataInduction {
            target,
            arms: cases,
            ..
        } => term(target) || arms(cases),
        Proof::IntInduction {
            base, step, target, ..
        } => sub(base) || sub(&step.body) || term(target),
        Proof::Linear { goal, pairs, .. } => {
            term(goal) || pairs.iter().any(|(proof, _)| sub(proof))
        }
    }
}

pub(super) fn term_is_classical(definitions: &Definitions, term: &Term) -> bool {
    let any = |terms: &[Term]| {
        terms
            .iter()
            .any(|term| term_is_classical(definitions, term))
    };
    let sub = |term: &Term| term_is_classical(definitions, term);
    match term {
        Term::Boxed(value) => term_is_classical(definitions, value),
        Term::Buffer { arguments, .. } => arguments
            .iter()
            .any(|term| term_is_classical(definitions, term)),
        Term::Fn(id) => definitions.is_classical(*id),
        Term::Lambda { body, .. } => sub(body),
        Term::Proof(proof) | Term::Absurd(proof, _) => proof_is_classical(definitions, proof),
        Term::Free(_)
        | Term::Bound(_)
        | Term::Bool(_)
        | Term::U8(_)
        | Term::Int(_)
        | Term::Machine(..) => false,
        Term::Prim(_, terms)
        | Term::Tuple(_, terms)
        | Term::Struct(_, terms)
        | Term::Variant(_, _, terms)
        | Term::PropApp(_, terms) => any(terms),
        Term::Eq(_, left, right) | Term::Implies(left, right) => sub(left) || sub(right),
        Term::Forall(_, body) | Term::Exists(_, body) | Term::Proj(body, _) => sub(body),
        Term::Call(callee, arguments) => sub(callee) || any(arguments),
        Term::For(looped) => {
            proof_is_classical(definitions, &looped.ordered)
                || sub(&looped.lo)
                || sub(&looped.hi)
                || sub(&looped.init)
                || sub(&looped.body)
        }
        Term::Case {
            scrutinee, arms, ..
        } => sub(scrutinee) || arms.iter().any(|arm| sub(&arm.body)),
    }
}
