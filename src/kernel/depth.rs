//! A bound on how deeply kernel input may nest.
//!
//! Checking, comparison, and substitution are recursive, so a deeply nested
//! term from untrusted source text could exhaust the stack. Every public
//! entry point measures its input first, with an explicit work list and no
//! recursion, and rejects input nested deeper than `MAX_DEPTH`.
//!
//! This bounds input. A term built by substitution during checking can be
//! deeper than any input, by at most the input depth for each substitution
//! a proof performs.

use super::error::KernelError;
use super::term::{Axiom, Proof, ProofArm, Term, Type};

/// Measured in an unoptimized build on a 2 MiB thread stack: nested
/// arithmetic and nested quantifiers check at depth 800, and the worst shape
/// found, a chain of transports, at 500 but not 600. The bound is about half
/// of that worst case. Optimized builds have far more room.
pub use crate::limits::MAX_KERNEL_DEPTH as MAX_DEPTH;

#[derive(Clone, Copy)]
pub(super) enum Node<'a> {
    Term(&'a Term),
    Proof(&'a Proof),
    Type(&'a Type),
}

impl<'a> From<&'a Term> for Node<'a> {
    fn from(term: &'a Term) -> Self {
        Self::Term(term)
    }
}

impl<'a> From<&'a Proof> for Node<'a> {
    fn from(proof: &'a Proof) -> Self {
        Self::Proof(proof)
    }
}

impl<'a> From<&'a Type> for Node<'a> {
    fn from(ty: &'a Type) -> Self {
        Self::Type(ty)
    }
}

/// Rejects input nested deeper than `MAX_DEPTH`.
pub(super) fn check_depth<'a>(
    roots: impl IntoIterator<Item = Node<'a>>,
) -> Result<(), KernelError> {
    let mut work: Vec<(Node<'a>, usize)> = roots.into_iter().map(|node| (node, 1)).collect();
    let mut children = Vec::new();
    while let Some((node, depth)) = work.pop() {
        if depth > MAX_DEPTH {
            return Err(KernelError::TooDeep);
        }
        children.clear();
        push_children(node, &mut children);
        work.extend(children.iter().map(|child| (*child, depth + 1)));
    }
    Ok(())
}

pub(super) fn push_children<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    match node {
        Node::Type(ty) => match ty {
            Type::Boxed(element) | Type::Buffer(element) => out.push(Node::Type(element)),
            Type::Bool
            | Type::U8
            | Type::Int
            | Type::Machine(_)
            | Type::Prop
            | Type::Struct(_)
            | Type::Enum(_) => {}
            Type::Proof(prop) => out.push(Node::Term(prop)),
            Type::Tuple(fields) => out.extend(fields.iter().map(Node::Type)),
            Type::Fn(params, result) => {
                out.extend(params.iter().map(Node::Type));
                out.push(Node::Type(result));
            }
        },
        Node::Term(term) => match term {
            Term::Boxed(value) => out.push(Node::Term(value)),
            Term::Buffer {
                element, arguments, ..
            } => {
                out.push(Node::Type(element));
                out.extend(arguments.iter().map(Node::Term));
            }
            Term::Free(_)
            | Term::Bound(_)
            | Term::Bool(_)
            | Term::U8(_)
            | Term::Int(_)
            | Term::Machine(..)
            | Term::Fn(_) => {}
            Term::Prim(_, terms) | Term::Struct(_, terms) | Term::Variant(_, _, terms) => {
                out.extend(terms.iter().map(Node::Term));
            }
            Term::PropApp(_, terms) => out.extend(terms.iter().map(Node::Term)),
            Term::Eq(ty, left, right) => {
                out.push(Node::Type(ty));
                out.push(Node::Term(left));
                out.push(Node::Term(right));
            }
            Term::Implies(left, right) => {
                out.push(Node::Term(left));
                out.push(Node::Term(right));
            }
            Term::Forall(ty, body) | Term::Exists(ty, body) => {
                out.push(Node::Type(ty));
                out.push(Node::Term(body));
            }
            Term::Tuple(fields, values) => {
                out.extend(fields.iter().map(Node::Type));
                out.extend(values.iter().map(Node::Term));
            }
            Term::Proj(target, _) => out.push(Node::Term(target)),
            Term::Proof(proof) => out.push(Node::Proof(proof)),
            Term::Lambda {
                params,
                result,
                body,
            } => {
                out.extend(params.iter().map(Node::Type));
                out.push(Node::Type(result));
                out.push(Node::Term(body));
            }
            Term::Call(callee, arguments) => {
                out.push(Node::Term(callee));
                out.extend(arguments.iter().map(Node::Term));
            }
            Term::Case {
                scrutinee,
                result,
                arms,
            } => {
                out.push(Node::Term(scrutinee));
                out.push(Node::Type(result));
                out.extend(arms.iter().map(|arm| Node::Term(&arm.body)));
            }
            Term::Absurd(proof, ty) => {
                out.push(Node::Proof(proof));
                out.push(Node::Type(ty));
            }
            Term::For(looped) => {
                out.push(Node::Term(&looped.lo));
                out.push(Node::Term(&looped.hi));
                out.push(Node::Proof(&looped.ordered));
                out.extend(looped.state.iter().map(Node::Type));
                out.push(Node::Term(&looped.init));
                out.push(Node::Term(&looped.body));
            }
        },
        Node::Proof(proof) => {
            let arm = |arm: &'a ProofArm| Node::Proof(&arm.body);
            match proof {
                Proof::CaseKnown { term, equation } => {
                    out.push(Node::Term(term));
                    out.push(Node::Proof(equation));
                }
                Proof::BufferStep(term) | Proof::BufferBound { value: term, .. } => {
                    out.push(Node::Term(term))
                }
                Proof::Hyp(_) | Proof::Omitted => {}
                Proof::OfTerm(term)
                | Proof::Refl(term)
                | Proof::Projection(term)
                | Proof::Literal(term)
                | Proof::Definition(term)
                | Proof::CaseStep(term)
                | Proof::ForEmpty(term)
                | Proof::Evaluate(term)
                | Proof::ExcludedMiddle(term) => out.push(Node::Term(term)),
                Proof::Transport {
                    eq,
                    template,
                    proof,
                } => {
                    out.push(Node::Proof(eq));
                    out.push(Node::Term(template));
                    out.push(Node::Proof(proof));
                }
                Proof::ImpliesIntro { hyp, body } => {
                    out.push(Node::Term(hyp));
                    out.push(Node::Proof(body));
                }
                Proof::ImpliesElim(left, right) => {
                    out.push(Node::Proof(left));
                    out.push(Node::Proof(right));
                }
                Proof::ForallIntro { ty, body } => {
                    out.push(Node::Type(ty));
                    out.push(Node::Proof(body));
                }
                Proof::ForallElim(universal, argument) => {
                    out.push(Node::Proof(universal));
                    out.push(Node::Term(argument));
                }
                Proof::Construct {
                    params, payload, ..
                } => out.extend(params.iter().chain(payload).map(Node::Term)),
                Proof::CaseProof {
                    scrutinee,
                    goal,
                    arms,
                } => {
                    out.push(Node::Proof(scrutinee));
                    out.push(Node::Term(goal));
                    out.extend(arms.iter().map(arm));
                }
                Proof::CaseData {
                    scrutinee,
                    goal,
                    arms,
                } => {
                    out.push(Node::Term(scrutinee));
                    out.push(Node::Term(goal));
                    out.extend(arms.iter().map(arm));
                }
                Proof::ExistsIntro {
                    prop,
                    witness,
                    proof,
                } => {
                    out.push(Node::Term(prop));
                    out.push(Node::Term(witness));
                    out.push(Node::Proof(proof));
                }
                Proof::ExistsElim {
                    exists,
                    goal,
                    arm: body,
                } => {
                    out.push(Node::Proof(exists));
                    out.push(Node::Term(goal));
                    out.push(arm(body));
                }
                Proof::ForStep {
                    looped,
                    lower,
                    upper,
                } => {
                    out.push(Node::Term(looped));
                    out.push(Node::Proof(lower));
                    out.push(Node::Proof(upper));
                }
                Proof::Axiom(axiom) => push_axiom(axiom, out),
                Proof::PropInduction {
                    scrutinee,
                    motive,
                    arms,
                } => {
                    out.push(Node::Proof(scrutinee));
                    out.push(Node::Term(&motive.body));
                    out.extend(arms.iter().map(arm));
                }
                Proof::DataInduction {
                    target,
                    motives,
                    arms,
                } => {
                    out.push(Node::Term(target));
                    out.extend(motives.iter().map(|(_, motive)| Node::Term(motive)));
                    out.extend(arms.iter().map(arm));
                }
                Proof::IntInduction {
                    motive,
                    base,
                    step,
                    target,
                } => {
                    out.push(Node::Term(motive));
                    out.push(Node::Proof(base));
                    out.push(arm(step));
                    out.push(Node::Term(target));
                }
                Proof::Linear { goal, pairs, .. } => {
                    out.push(Node::Term(goal));
                    out.extend(pairs.iter().map(|(proof, _)| Node::Proof(proof)));
                }
            }
        }
    }
}

fn push_axiom<'a>(axiom: &'a Axiom, out: &mut Vec<Node<'a>>) {
    out.extend(axiom.terms().into_iter().map(Node::Term));
}
