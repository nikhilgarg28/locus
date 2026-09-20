//! Kernel types, terms, and proofs.
//!
//! Binding is locally nameless. A variable bound inside a term (by `Forall`,
//! by a transport template, by a `ForallIntro` proof, or by an earlier field
//! of a product type) is a de Bruijn index; a variable of the surrounding
//! context is a globally unique identity. Terms that differ only in the names
//! of bound variables are therefore equal as data, and substituting a
//! context-level term under a binder needs no shifting. Hypotheses bound by
//! `ImpliesIntro` are indexed separately from term variables.
//!
//! Types, terms, and proofs are mutually recursive: a proof type mentions a
//! proposition, a product value carries proofs in its proof fields, and a
//! proof may be a term of proof type.

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

/// Identity of a declared struct; see `Definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructId(pub(super) usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Bool,
    U8,
    /// The type of propositions. Every value of this type is ghost.
    Prop,
    /// `@P`: the type of proofs of the proposition `P`. Ghost.
    Proof(Box<Term>),
    /// A telescope of field types. Field `i` is under `i` binders:
    /// `Bound(0)` in it is field `i - 1`, `Bound(1)` is field `i - 2`, and so
    /// on. A term occurs in a type only inside a `Proof`, so a value can
    /// influence what a later proof field says and never what data is stored.
    Tuple(Vec<Type>),
    /// A declared struct. Nominal: two declarations are different types.
    Struct(StructId),
}

impl Type {
    /// A ghost type has no runtime representation.
    pub fn is_ghost(&self) -> bool {
        matches!(self, Self::Prop | Self::Proof(_))
    }

    pub fn proof(prop: Term) -> Self {
        Self::Proof(Box::new(prop))
    }

    /// Builds a telescope. `fields(earlier)` receives the earlier fields as
    /// terms and returns the type of the next field, or `None` when done.
    pub fn tuple(mut fields: impl FnMut(&[Term]) -> Option<Type>) -> Self {
        let mut vars: Vec<VarId> = Vec::new();
        let mut telescope = Vec::new();
        loop {
            let earlier: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
            let Some(mut ty) = fields(&earlier) else {
                return Self::Tuple(telescope);
            };
            // Field `i` sees earlier field `j` as index `i - 1 - j`.
            let count = vars.len() as u32;
            for (j, var) in vars.iter().enumerate() {
                ty = ty.rebind(Depth::at(count - 1 - j as u32), Rebind::CloseVar(*var));
            }
            telescope.push(ty);
            vars.push(VarId::fresh());
        }
    }

    pub(super) fn rebind(&self, depth: Depth, op: Rebind<'_>) -> Type {
        match self {
            Self::Bool | Self::U8 | Self::Prop | Self::Struct(_) => self.clone(),
            Self::Proof(prop) => Self::Proof(Box::new(prop.rebind(depth, op))),
            Self::Tuple(fields) => Self::Tuple(
                fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| field.rebind(depth.under_vars(index as u32), op))
                    .collect(),
            ),
        }
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
    /// A tuple value. It carries its telescope because a dependent product
    /// type cannot be inferred from the values alone.
    Tuple(Vec<Type>, Vec<Term>),
    Struct(StructId, Vec<Term>),
    /// Positional projection from a tuple or struct.
    Proj(Box<Term>, usize),
    /// A proof used as a value, of type `@P`. Every proof field of a product
    /// value has this form, which is what lets comparison ignore proofs
    /// without knowing any types.
    Proof(Box<Proof>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypRef {
    Free(HypId),
    Bound(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proof {
    Hyp(HypRef),
    /// A term of type `@P`, such as a projection of a proof field, proves `P`.
    OfTerm(Term),
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
    /// Computation axiom: `(v_0, ..., v_n).i == v_i`, for the given
    /// projection term.
    Projection(Term),
    /// Computation axiom: `op(literals) == literal`, by native evaluation.
    Literal(Term),
}

/// How many binders of each kind enclose the current position.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Depth {
    vars: u32,
    hyps: u32,
}

impl Depth {
    fn at(vars: u32) -> Self {
        Self { vars, hyps: 0 }
    }

    fn under_vars(self, count: u32) -> Self {
        Self {
            vars: self.vars + count,
            ..self
        }
    }

    fn under_hyp(self) -> Self {
        Self {
            hyps: self.hyps + 1,
            ..self
        }
    }
}

/// One traversal serves opening and closing of both kinds of binder.
#[derive(Clone, Copy)]
pub(super) enum Rebind<'a> {
    /// Replace the bound variable `index` binders out with a locally closed
    /// term. Other indices are left alone.
    OpenVar {
        index: u32,
        replacement: &'a Term,
    },
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

    /// A tuple value of the given tuple type.
    pub fn tuple(ty: &Type, values: Vec<Term>) -> Self {
        match ty {
            Type::Tuple(fields) => Self::Tuple(fields.clone(), values),
            _ => panic!("Term::tuple needs a tuple type"),
        }
    }

    pub fn proj(target: Term, index: usize) -> Self {
        Self::Proj(Box::new(target), index)
    }

    pub fn proof(proof: Proof) -> Self {
        Self::Proof(Box::new(proof))
    }

    /// Replaces the outermost bound variable with a locally closed term.
    pub(super) fn open(&self, replacement: &Term) -> Term {
        self.rebind(
            Depth::default(),
            Rebind::OpenVar {
                index: 0,
                replacement,
            },
        )
    }

    /// Turns a context variable into the outermost bound variable.
    pub(super) fn close(&self, var: VarId) -> Term {
        self.rebind(Depth::default(), Rebind::CloseVar(var))
    }

    pub(super) fn rebind(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let each = |terms: &[Term]| terms.iter().map(|term| term.rebind(depth, op)).collect();
        match self {
            Self::Free(id) => match op {
                Rebind::CloseVar(var) if var == *id => Self::Bound(depth.vars),
                _ => self.clone(),
            },
            Self::Bound(bound) => match op {
                Rebind::OpenVar { index, replacement } if *bound == depth.vars + index => {
                    replacement.clone()
                }
                _ => self.clone(),
            },
            Self::Bool(_) | Self::U8(_) => self.clone(),
            Self::Prim(prim, arguments) => Self::Prim(*prim, each(arguments)),
            Self::Eq(ty, left, right) => Self::Eq(
                ty.rebind(depth, op),
                Box::new(left.rebind(depth, op)),
                Box::new(right.rebind(depth, op)),
            ),
            Self::Implies(premise, conclusion) => Self::Implies(
                Box::new(premise.rebind(depth, op)),
                Box::new(conclusion.rebind(depth, op)),
            ),
            Self::Forall(ty, body) => Self::Forall(
                ty.rebind(depth, op),
                Box::new(body.rebind(depth.under_vars(1), op)),
            ),
            Self::Tuple(fields, values) => {
                let Type::Tuple(fields) = Type::Tuple(fields.clone()).rebind(depth, op) else {
                    unreachable!("rebinding preserves the shape of a type")
                };
                Self::Tuple(fields, each(values))
            }
            Self::Struct(id, values) => Self::Struct(*id, each(values)),
            Self::Proj(target, index) => Self::Proj(Box::new(target.rebind(depth, op)), *index),
            Self::Proof(proof) => Self::Proof(Box::new(proof.rebind(depth, op))),
        }
    }
}

/// The type of field `index` of a telescope, with each earlier field `j`
/// replaced by `earlier(j)`. The replacements must be locally closed.
pub(super) fn field_type(fields: &[Type], index: usize, earlier: impl Fn(usize) -> Term) -> Type {
    let mut ty = fields[index].clone();
    for j in 0..index {
        let replacement = earlier(j);
        ty = ty.rebind(
            Depth::default(),
            Rebind::OpenVar {
                index: (index - 1 - j) as u32,
                replacement: &replacement,
            },
        );
    }
    ty
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
        let body = body(Self::hyp(id)).rebind(Depth::default(), Rebind::CloseHyp(id));
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
        let body = body(Term::Free(var)).rebind(Depth::default(), Rebind::CloseVar(var));
        Self::ForallIntro {
            ty,
            body: Box::new(body),
        }
    }

    pub fn forall_elim(universal: Proof, argument: Term) -> Self {
        Self::ForallElim(Box::new(universal), argument)
    }

    pub(super) fn open_var(&self, replacement: &Term) -> Proof {
        self.rebind(
            Depth::default(),
            Rebind::OpenVar {
                index: 0,
                replacement,
            },
        )
    }

    pub(super) fn open_hyp(&self, id: HypId) -> Proof {
        self.rebind(Depth::default(), Rebind::OpenHyp(id))
    }

    pub(super) fn rebind(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        match self {
            Self::Hyp(HypRef::Free(id)) => match op {
                Rebind::CloseHyp(target) if target == *id => Self::Hyp(HypRef::Bound(depth.hyps)),
                _ => self.clone(),
            },
            Self::Hyp(HypRef::Bound(index)) => match op {
                Rebind::OpenHyp(id) if *index == depth.hyps => Self::Hyp(HypRef::Free(id)),
                _ => self.clone(),
            },
            Self::OfTerm(term) => Self::OfTerm(term.rebind(depth, op)),
            Self::Refl(term) => Self::Refl(term.rebind(depth, op)),
            Self::Transport {
                eq,
                template,
                proof,
            } => Self::Transport {
                eq: Box::new(eq.rebind(depth, op)),
                template: template.rebind(depth.under_vars(1), op),
                proof: Box::new(proof.rebind(depth, op)),
            },
            Self::ImpliesIntro { hyp, body } => Self::ImpliesIntro {
                hyp: hyp.rebind(depth, op),
                body: Box::new(body.rebind(depth.under_hyp(), op)),
            },
            Self::ImpliesElim(implication, premise) => Self::ImpliesElim(
                Box::new(implication.rebind(depth, op)),
                Box::new(premise.rebind(depth, op)),
            ),
            Self::ForallIntro { ty, body } => Self::ForallIntro {
                ty: ty.rebind(depth, op),
                body: Box::new(body.rebind(depth.under_vars(1), op)),
            },
            Self::ForallElim(universal, argument) => Self::ForallElim(
                Box::new(universal.rebind(depth, op)),
                argument.rebind(depth, op),
            ),
            Self::Projection(term) => Self::Projection(term.rebind(depth, op)),
            Self::Literal(term) => Self::Literal(term.rebind(depth, op)),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => f.write_str("bool"),
            Self::U8 => f.write_str("u8"),
            Self::Prop => f.write_str("Prop"),
            Self::Proof(prop) => write!(f, "@{prop}"),
            Self::Tuple(fields) => {
                f.write_str("(")?;
                for field in fields {
                    write!(f, "{field}, ")?;
                }
                f.write_str(")")
            }
            Self::Struct(StructId(id)) => write!(f, "struct#{id}"),
        }
    }
}

fn write_list(f: &mut fmt::Formatter<'_>, terms: &[Term]) -> fmt::Result {
    for (index, term) in terms.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{term}")?;
    }
    Ok(())
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
                write_list(f, arguments)?;
                f.write_str(")")
            }
            Self::Eq(_, left, right) => write!(f, "({left} == {right})"),
            Self::Implies(premise, conclusion) => write!(f, "({premise} => {conclusion})"),
            Self::Forall(ty, body) => write!(f, "forall (#: {ty}) {{ {body} }}"),
            Self::Tuple(_, values) => {
                f.write_str("(")?;
                write_list(f, values)?;
                f.write_str(")")
            }
            Self::Struct(StructId(id), values) => {
                write!(f, "struct#{id} {{ ")?;
                write_list(f, values)?;
                f.write_str(" }")
            }
            Self::Proj(target, index) => write!(f, "{target}.{index}"),
            Self::Proof(_) => f.write_str("<proof>"),
        }
    }
}
