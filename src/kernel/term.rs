//! Kernel types, terms, and proofs.
//!
//! Binding is locally nameless. A variable bound inside a term (by `Forall`,
//! by a transport template, or by a `ForallIntro` proof) is a de Bruijn index;
//! a variable of the surrounding context is a globally unique identity. Terms
//! that differ only in the names of bound variables are therefore equal as
//! data, and substituting a context-level term under a binder needs no
//! shifting. Hypotheses bound by `ImpliesIntro` are indexed separately from
//! term variables.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn fresh_id() -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    assert!(id != u64::MAX, "kernel identity space exhausted");
    id
}

/// Identity of a context variable. Identities are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VarId(u64);

impl VarId {
    pub fn fresh() -> Self {
        Self(fresh_id())
    }
}

/// Identity of a context hypothesis. Identities are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HypId(u64);

impl HypId {
    pub fn fresh() -> Self {
        Self(fresh_id())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Bool,
    U8,
    /// The type of propositions. Every value of this type is ghost.
    Prop,
}

impl Type {
    /// A ghost type has no runtime representation.
    pub fn is_ghost(&self) -> bool {
        matches!(self, Self::Prop)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prim {
    WrappingAdd,
    WrappingSub,
}

/// Data terms and propositions. A proposition is a term of type `Prop`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Term {
    Free(VarId),
    Bound(u32),
    Bool(bool),
    U8(u8),
    Prim(Prim, Vec<Term>),
    /// `a == b` at the given type.
    Eq(Type, Box<Term>, Box<Term>),
    Implies(Box<Term>, Box<Term>),
    /// Binds `Bound(0)` in its body.
    Forall(Type, Box<Term>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypRef {
    Free(HypId),
    Bound(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proof {
    Hyp(HypRef),
    Refl(Term),
    /// From `eq: a == b` and `proof: template[a]`, conclude `template[b]`.
    /// The template binds `Bound(0)` as its hole.
    Transport {
        eq: Box<Proof>,
        template: Term,
        proof: Box<Proof>,
    },
    /// Binds hypothesis `Bound(0)` in its body.
    ImpliesIntro {
        hyp: Term,
        body: Box<Proof>,
    },
    ImpliesElim(Box<Proof>, Box<Proof>),
    /// Binds term variable `Bound(0)` in its body.
    ForallIntro {
        ty: Type,
        body: Box<Proof>,
    },
    ForallElim(Box<Proof>, Term),
}

/// One traversal serves opening and closing of both kinds of binder.
#[derive(Clone, Copy)]
pub(super) enum Rebind<'a> {
    OpenVar(&'a Term),
    CloseVar(VarId),
    OpenHyp(HypId),
    CloseHyp(HypId),
}

impl Term {
    pub fn var(id: VarId) -> Self {
        Self::Free(id)
    }

    pub fn eq(ty: Type, left: Term, right: Term) -> Self {
        Self::Eq(ty, Box::new(left), Box::new(right))
    }

    pub fn implies(premise: Term, conclusion: Term) -> Self {
        Self::Implies(Box::new(premise), Box::new(conclusion))
    }

    /// Builds `forall (x: ty) { body(x) }`.
    pub fn forall(ty: Type, body: impl FnOnce(Term) -> Term) -> Self {
        let var = VarId::fresh();
        let body = body(Self::Free(var)).close(var);
        Self::Forall(ty, Box::new(body))
    }

    pub fn wrapping_add(left: Term, right: Term) -> Self {
        Self::Prim(Prim::WrappingAdd, vec![left, right])
    }

    pub fn wrapping_sub(left: Term, right: Term) -> Self {
        Self::Prim(Prim::WrappingSub, vec![left, right])
    }

    /// Replaces the outermost bound variable with a locally closed term.
    pub(super) fn open(&self, replacement: &Term) -> Term {
        self.rebind(0, Rebind::OpenVar(replacement))
    }

    /// Turns a context variable into the outermost bound variable.
    pub(super) fn close(&self, var: VarId) -> Term {
        self.rebind(0, Rebind::CloseVar(var))
    }

    pub(super) fn rebind(&self, depth: u32, op: Rebind<'_>) -> Term {
        match self {
            Self::Free(id) => match op {
                Rebind::CloseVar(var) if var == *id => Self::Bound(depth),
                _ => self.clone(),
            },
            Self::Bound(index) => match op {
                Rebind::OpenVar(replacement) if *index == depth => replacement.clone(),
                _ => self.clone(),
            },
            Self::Bool(_) | Self::U8(_) => self.clone(),
            Self::Prim(prim, arguments) => Self::Prim(
                *prim,
                arguments
                    .iter()
                    .map(|argument| argument.rebind(depth, op))
                    .collect(),
            ),
            Self::Eq(ty, left, right) => Self::Eq(
                ty.clone(),
                Box::new(left.rebind(depth, op)),
                Box::new(right.rebind(depth, op)),
            ),
            Self::Implies(premise, conclusion) => Self::Implies(
                Box::new(premise.rebind(depth, op)),
                Box::new(conclusion.rebind(depth, op)),
            ),
            Self::Forall(ty, body) => {
                Self::Forall(ty.clone(), Box::new(body.rebind(depth + 1, op)))
            }
        }
    }
}

impl Proof {
    pub fn hyp(id: HypId) -> Self {
        Self::Hyp(HypRef::Free(id))
    }

    /// Builds a transport whose template is `template(hole)`.
    pub fn transport(eq: Proof, template: impl FnOnce(Term) -> Term, proof: Proof) -> Self {
        let hole = VarId::fresh();
        Self::Transport {
            eq: Box::new(eq),
            template: template(Term::Free(hole)).close(hole),
            proof: Box::new(proof),
        }
    }

    /// Builds a proof of `hyp => Q` from a proof of `Q` that may use `hyp`.
    pub fn implies_intro(hyp: Term, body: impl FnOnce(Proof) -> Proof) -> Self {
        let id = HypId::fresh();
        let body = body(Self::hyp(id)).rebind(0, 0, Rebind::CloseHyp(id));
        Self::ImpliesIntro {
            hyp,
            body: Box::new(body),
        }
    }

    pub fn implies_elim(implication: Proof, premise: Proof) -> Self {
        Self::ImpliesElim(Box::new(implication), Box::new(premise))
    }

    /// Builds a proof of `forall (x: ty) { P(x) }` from a proof of `P(x)`.
    pub fn forall_intro(ty: Type, body: impl FnOnce(Term) -> Proof) -> Self {
        let var = VarId::fresh();
        let body = body(Term::Free(var)).rebind(0, 0, Rebind::CloseVar(var));
        Self::ForallIntro {
            ty,
            body: Box::new(body),
        }
    }

    pub fn forall_elim(universal: Proof, argument: Term) -> Self {
        Self::ForallElim(Box::new(universal), argument)
    }

    pub(super) fn open_var(&self, replacement: &Term) -> Proof {
        self.rebind(0, 0, Rebind::OpenVar(replacement))
    }

    pub(super) fn open_hyp(&self, id: HypId) -> Proof {
        self.rebind(0, 0, Rebind::OpenHyp(id))
    }

    /// `vars` and `hyps` count the enclosing binders of each kind.
    pub(super) fn rebind(&self, vars: u32, hyps: u32, op: Rebind<'_>) -> Proof {
        match self {
            Self::Hyp(HypRef::Free(id)) => match op {
                Rebind::CloseHyp(target) if target == *id => Self::Hyp(HypRef::Bound(hyps)),
                _ => self.clone(),
            },
            Self::Hyp(HypRef::Bound(index)) => match op {
                Rebind::OpenHyp(id) if *index == hyps => Self::Hyp(HypRef::Free(id)),
                _ => self.clone(),
            },
            Self::Refl(term) => Self::Refl(term.rebind(vars, op)),
            Self::Transport {
                eq,
                template,
                proof,
            } => Self::Transport {
                eq: Box::new(eq.rebind(vars, hyps, op)),
                template: template.rebind(vars + 1, op),
                proof: Box::new(proof.rebind(vars, hyps, op)),
            },
            Self::ImpliesIntro { hyp, body } => Self::ImpliesIntro {
                hyp: hyp.rebind(vars, op),
                body: Box::new(body.rebind(vars, hyps + 1, op)),
            },
            Self::ImpliesElim(implication, premise) => Self::ImpliesElim(
                Box::new(implication.rebind(vars, hyps, op)),
                Box::new(premise.rebind(vars, hyps, op)),
            ),
            Self::ForallIntro { ty, body } => Self::ForallIntro {
                ty: ty.clone(),
                body: Box::new(body.rebind(vars + 1, hyps, op)),
            },
            Self::ForallElim(universal, argument) => Self::ForallElim(
                Box::new(universal.rebind(vars, hyps, op)),
                argument.rebind(vars, op),
            ),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Bool => "bool",
            Self::U8 => "u8",
            Self::Prop => "Prop",
        })
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Free(VarId(id)) => write!(f, "v{id}"),
            Self::Bound(index) => write!(f, "#{index}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::U8(value) => write!(f, "{value}"),
            Self::Prim(prim, arguments) => {
                let name = match prim {
                    Prim::WrappingAdd => "wrapping_add",
                    Prim::WrappingSub => "wrapping_sub",
                };
                write!(f, "{name}(")?;
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{argument}")?;
                }
                f.write_str(")")
            }
            Self::Eq(_, left, right) => write!(f, "({left} == {right})"),
            Self::Implies(premise, conclusion) => write!(f, "({premise} => {conclusion})"),
            Self::Forall(ty, body) => write!(f, "forall (#: {ty}) {{ {body} }}"),
        }
    }
}
