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

/// Identity of a declared enum; see `Definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnumId(pub(super) usize);

/// Identity of a declared proposition; see `Definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropId(pub(super) usize);

/// Identity of a declared math function; see `Definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FnId(pub(super) usize);

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
    /// A declared enum. Nominal.
    Enum(EnumId),
    /// A total function type. The parameters form a telescope and the result
    /// type is under all of them. Ordinary `fn` never reaches the kernel.
    Fn(Vec<Type>, Box<Type>),
}

impl Type {
    /// A ghost type has no runtime representation.
    pub fn is_ghost(&self) -> bool {
        match self {
            Self::Prop | Self::Proof(_) => true,
            // A function into a ghost type is a proof or a predicate.
            Self::Fn(_, result) => result.is_ghost(),
            Self::Bool | Self::U8 | Self::Tuple(_) | Self::Struct(_) | Self::Enum(_) => false,
        }
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
            let Some(ty) = fields(&earlier) else {
                return Self::Tuple(telescope);
            };
            telescope.push(ty.close_over(&vars));
            vars.push(VarId::fresh());
        }
    }

    /// Builds a function type of the given arity. `signature(params)` is
    /// called with 0, 1, ..., `arity` parameters in scope: the first `arity`
    /// calls return parameter types and the last returns the result type.
    pub fn function(arity: usize, mut signature: impl FnMut(&[Term]) -> Type) -> Self {
        let mut vars: Vec<VarId> = Vec::new();
        let mut telescope = Vec::new();
        loop {
            let earlier: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
            let ty = signature(&earlier).close_over(&vars);
            if vars.len() == arity {
                return Self::Fn(telescope, Box::new(ty));
            }
            telescope.push(ty);
            vars.push(VarId::fresh());
        }
    }

    /// Puts a type under binders for `vars`: variable `j` of `n` becomes
    /// index `n - 1 - j`.
    pub(super) fn close_over(&self, vars: &[VarId]) -> Type {
        let count = vars.len() as u32;
        let mut ty = self.clone();
        for (j, var) in vars.iter().enumerate() {
            ty = ty.rebind(Depth::at(count - 1 - j as u32), Rebind::CloseVar(*var));
        }
        ty
    }

    pub(super) fn rebind(&self, depth: Depth, op: Rebind<'_>) -> Type {
        match self {
            Self::Bool | Self::U8 | Self::Prop | Self::Struct(_) | Self::Enum(_) => self.clone(),
            Self::Proof(prop) => Self::Proof(Box::new(prop.rebind(depth, op))),
            Self::Tuple(fields) => Self::Tuple(rebind_telescope(fields, depth, op)),
            Self::Fn(params, result) => Self::Fn(
                rebind_telescope(params, depth, op),
                Box::new(result.rebind(depth.under_vars(params.len() as u32), op)),
            ),
        }
    }
}

fn rebind_telescope(fields: &[Type], depth: Depth, op: Rebind<'_>) -> Vec<Type> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| field.rebind(depth.under_vars(index as u32), op))
        .collect()
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
    /// A declared math function used as a value.
    Fn(FnId),
    /// Application of a term of function type.
    Call(Box<Term>, Vec<Term>),
    /// A value of a declared enum: the variant's index and its payload.
    Variant(EnumId, usize, Vec<Term>),
    /// Case analysis on a `bool` (arms: false, true) or an enum (one arm per
    /// variant, in declaration order). The result type does not depend on
    /// the scrutinee and is not a proof type.
    Case {
        scrutinee: Box<Term>,
        result: Type,
        arms: Vec<TermArm>,
    },
    /// A declared proposition applied to its arguments.
    PropApp(PropId, Vec<Term>),
    /// Binds `Bound(0)` in its body.
    Exists(Type, Box<Term>),
    /// A value of any type, from a proof of a proposition with no variants.
    /// It marks a point that is never reached.
    Absurd(Box<Proof>, Type),
}

/// Builds the body of a term-level case arm from its payload variables.
pub type ArmBuilder<'a> = Box<dyn FnOnce(&[Term]) -> Term + 'a>;

/// An arm of a term-level case. The body is under `binders` binders, one per
/// payload field of the variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TermArm {
    pub binders: u32,
    pub body: Term,
}

/// An arm of a proof-level case. The body is under `vars` term binders and
/// `hyps` hypothesis binders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofArm {
    pub vars: u32,
    pub hyps: u32,
    pub body: Box<Proof>,
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
    /// Computation axiom: `f(args) == body[params := args]`, the defining
    /// equation of a declared math function, for the given call.
    Definition(Term),
    /// Computation axiom: a case on a known constructor equals its arm.
    CaseStep(Term),
    /// A constructor of a declared proposition. `params` instantiates the
    /// proposition's parameters for a variant without a stated conclusion and
    /// is empty for a variant with one.
    Construct {
        prop: PropId,
        variant: usize,
        params: Vec<Term>,
        payload: Vec<Term>,
    },
    /// Case analysis on a proof of a declared proposition. Each arm binds its
    /// variant's payload and, for a variant with a stated conclusion, one
    /// index equation per parameter.
    CaseProof {
        scrutinee: Box<Proof>,
        goal: Term,
        arms: Vec<ProofArm>,
    },
    /// Case analysis on data, to prove a goal. Each arm binds its variant's
    /// payload and the hypothesis `scrutinee == variant(payload)`.
    CaseData {
        scrutinee: Term,
        goal: Term,
        arms: Vec<ProofArm>,
    },
    /// `prop` is `exists (x: A) { B }`; `proof` proves `B[witness]`.
    ExistsIntro {
        prop: Term,
        witness: Term,
        proof: Box<Proof>,
    },
    /// The arm binds the witness and the hypothesis that it satisfies the
    /// body. The goal cannot mention the witness.
    ExistsElim {
        exists: Box<Proof>,
        goal: Term,
        arm: ProofArm,
    },
    /// `p || !p`, for the prelude's `Or` and `False`.
    ExcludedMiddle(Term),
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

    fn under_hyps(self, count: u32) -> Self {
        Self {
            hyps: self.hyps + count,
            ..self
        }
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
    /// Replace the bound hypothesis `index` binders out.
    OpenHyp {
        index: u32,
        id: HypId,
    },
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

    pub fn call(callee: Term, arguments: Vec<Term>) -> Self {
        Self::Call(Box::new(callee), arguments)
    }

    /// Builds `exists (x: ty) { body(x) }`.
    pub fn exists(ty: Type, body: impl FnOnce(Term) -> Term) -> Self {
        let var = VarId::fresh();
        let body = body(Self::Free(var)).close(var);
        Self::Exists(ty, Box::new(body))
    }

    /// Builds a case. Each arm is given its payload arity and receives that
    /// many payload variables.
    pub fn case(scrutinee: Term, result: Type, arms: Vec<(usize, ArmBuilder<'_>)>) -> Self {
        let arms = arms
            .into_iter()
            .map(|(arity, body)| {
                let vars: Vec<VarId> = (0..arity).map(|_| VarId::fresh()).collect();
                let payload: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
                TermArm {
                    binders: arity as u32,
                    body: body(&payload).close_over(&vars),
                }
            })
            .collect();
        Self::Case {
            scrutinee: Box::new(scrutinee),
            result,
            arms,
        }
    }

    /// Puts a term under binders for `vars`, as `Type::close_over`.
    pub(super) fn close_over(&self, vars: &[VarId]) -> Term {
        let count = vars.len() as u32;
        let mut term = self.clone();
        for (j, var) in vars.iter().enumerate() {
            term = term.rebind(Depth::at(count - 1 - j as u32), Rebind::CloseVar(*var));
        }
        term
    }

    /// Instantiates a term that is under `count` binders: binder `j` is
    /// replaced by `value(j)`, which must be locally closed.
    pub(super) fn instantiate(&self, count: usize, value: impl Fn(usize) -> Term) -> Term {
        let mut term = self.clone();
        for j in 0..count {
            let replacement = value(j);
            term = term.rebind(
                Depth::default(),
                Rebind::OpenVar {
                    index: (count - 1 - j) as u32,
                    replacement: &replacement,
                },
            );
        }
        term
    }

    /// Whether the term has no dangling bound index. Types and proofs inside
    /// it are not inspected; this is used only to choose rewrite targets.
    pub(super) fn is_closed(&self) -> bool {
        self.closed_at(0)
    }

    fn closed_at(&self, depth: u32) -> bool {
        let all = |terms: &[Term]| terms.iter().all(|term| term.closed_at(depth));
        match self {
            Self::Bound(index) => *index < depth,
            Self::Free(_) | Self::Bool(_) | Self::U8(_) | Self::Proof(_) | Self::Fn(_) => true,
            Self::Prim(_, arguments) => all(arguments),
            Self::Eq(_, left, right) => left.closed_at(depth) && right.closed_at(depth),
            Self::Implies(premise, conclusion) => {
                premise.closed_at(depth) && conclusion.closed_at(depth)
            }
            Self::Forall(_, body) => body.closed_at(depth + 1),
            Self::Tuple(_, values) | Self::Struct(_, values) => all(values),
            Self::Proj(target, _) => target.closed_at(depth),
            Self::Call(callee, arguments) => callee.closed_at(depth) && all(arguments),
            Self::Variant(_, _, payload) => all(payload),
            Self::Case {
                scrutinee, arms, ..
            } => {
                scrutinee.closed_at(depth)
                    && arms
                        .iter()
                        .all(|arm| arm.body.closed_at(depth + arm.binders))
            }
            Self::PropApp(_, arguments) => all(arguments),
            Self::Exists(_, body) => body.closed_at(depth + 1),
            Self::Absurd(_, _) => true,
        }
    }

    /// The first subterm, outermost and leftmost, that satisfies `wanted`.
    /// Types and proofs inside the term are not searched.
    pub(super) fn find(&self, wanted: &impl Fn(&Term) -> bool) -> Option<&Term> {
        fn first<'a>(terms: &'a [Term], wanted: &impl Fn(&Term) -> bool) -> Option<&'a Term> {
            terms.iter().find_map(|term| term.find(wanted))
        }
        if wanted(self) {
            return Some(self);
        }
        match self {
            Self::Free(_)
            | Self::Bound(_)
            | Self::Bool(_)
            | Self::U8(_)
            | Self::Proof(_)
            | Self::Fn(_)
            | Self::Absurd(_, _) => None,
            Self::Variant(_, _, payload) => first(payload, wanted),
            Self::Case {
                scrutinee, arms, ..
            } => scrutinee
                .find(wanted)
                .or_else(|| arms.iter().find_map(|arm| arm.body.find(wanted))),
            Self::PropApp(_, arguments) => first(arguments, wanted),
            Self::Exists(_, body) => body.find(wanted),
            Self::Prim(_, arguments) => first(arguments, wanted),
            Self::Eq(_, left, right) => left.find(wanted).or_else(|| right.find(wanted)),
            Self::Implies(premise, conclusion) => {
                premise.find(wanted).or_else(|| conclusion.find(wanted))
            }
            Self::Forall(_, body) => body.find(wanted),
            Self::Tuple(_, values) | Self::Struct(_, values) => first(values, wanted),
            Self::Proj(target, _) => target.find(wanted),
            Self::Call(callee, arguments) => {
                callee.find(wanted).or_else(|| first(arguments, wanted))
            }
        }
    }

    /// A template whose hole stands for every occurrence of `target` that
    /// `is_target` recognizes. `target` must be locally closed. Occurrences
    /// inside types and proofs are left alone, which keeps the template
    /// valid: opening it with `target` gives back this term.
    pub(super) fn abstract_over(&self, is_target: &impl Fn(&Term) -> bool) -> Term {
        self.abstract_at(is_target, 0)
    }

    fn abstract_at(&self, is_target: &impl Fn(&Term) -> bool, depth: u32) -> Term {
        if is_target(self) {
            return Self::Bound(depth);
        }
        let each = |terms: &[Term]| -> Vec<Term> {
            terms
                .iter()
                .map(|term| term.abstract_at(is_target, depth))
                .collect()
        };
        let boxed = |term: &Term, depth: u32| Box::new(term.abstract_at(is_target, depth));
        match self {
            Self::Free(_)
            | Self::Bound(_)
            | Self::Bool(_)
            | Self::U8(_)
            | Self::Proof(_)
            | Self::Fn(_)
            | Self::Absurd(_, _) => self.clone(),
            Self::Variant(id, index, payload) => Self::Variant(*id, *index, each(payload)),
            Self::Case {
                scrutinee,
                result,
                arms,
            } => Self::Case {
                scrutinee: boxed(scrutinee, depth),
                result: result.clone(),
                arms: arms
                    .iter()
                    .map(|arm| TermArm {
                        binders: arm.binders,
                        body: arm.body.abstract_at(is_target, depth + arm.binders),
                    })
                    .collect(),
            },
            Self::PropApp(id, arguments) => Self::PropApp(*id, each(arguments)),
            Self::Exists(ty, body) => Self::Exists(ty.clone(), boxed(body, depth + 1)),
            Self::Prim(prim, arguments) => Self::Prim(*prim, each(arguments)),
            Self::Eq(ty, left, right) => {
                Self::Eq(ty.clone(), boxed(left, depth), boxed(right, depth))
            }
            Self::Implies(premise, conclusion) => {
                Self::Implies(boxed(premise, depth), boxed(conclusion, depth))
            }
            Self::Forall(ty, body) => Self::Forall(ty.clone(), boxed(body, depth + 1)),
            Self::Tuple(fields, values) => Self::Tuple(fields.clone(), each(values)),
            Self::Struct(id, values) => Self::Struct(*id, each(values)),
            Self::Proj(target, index) => Self::Proj(boxed(target, depth), *index),
            Self::Call(callee, arguments) => Self::Call(boxed(callee, depth), each(arguments)),
        }
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
            Self::Fn(_) => self.clone(),
            Self::Call(callee, arguments) => {
                Self::Call(Box::new(callee.rebind(depth, op)), each(arguments))
            }
            Self::Variant(id, index, payload) => Self::Variant(*id, *index, each(payload)),
            Self::Case {
                scrutinee,
                result,
                arms,
            } => Self::Case {
                scrutinee: Box::new(scrutinee.rebind(depth, op)),
                result: result.rebind(depth, op),
                arms: arms
                    .iter()
                    .map(|arm| TermArm {
                        binders: arm.binders,
                        body: arm.body.rebind(depth.under_vars(arm.binders), op),
                    })
                    .collect(),
            },
            Self::PropApp(id, arguments) => Self::PropApp(*id, each(arguments)),
            Self::Exists(ty, body) => Self::Exists(
                ty.rebind(depth, op),
                Box::new(body.rebind(depth.under_vars(1), op)),
            ),
            Self::Absurd(proof, ty) => {
                Self::Absurd(Box::new(proof.rebind(depth, op)), ty.rebind(depth, op))
            }
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
        self.rebind(Depth::default(), Rebind::OpenHyp { index: 0, id })
    }

    /// Instantiates an arm body that is under `vars.len()` term binders and
    /// `hyps.len()` hypothesis binders.
    pub(super) fn open_arm(&self, vars: &[VarId], hyps: &[HypId]) -> Proof {
        let mut proof = self.clone();
        for (j, var) in vars.iter().enumerate() {
            proof = proof.rebind(
                Depth::default(),
                Rebind::OpenVar {
                    index: (vars.len() - 1 - j) as u32,
                    replacement: &Term::Free(*var),
                },
            );
        }
        for (j, id) in hyps.iter().enumerate() {
            proof = proof.rebind(
                Depth::default(),
                Rebind::OpenHyp {
                    index: (hyps.len() - 1 - j) as u32,
                    id: *id,
                },
            );
        }
        proof
    }

    /// Builds an arm whose body receives `vars` payload variables and `hyps`
    /// hypotheses.
    pub fn arm(
        vars: usize,
        hyps: usize,
        body: impl FnOnce(&[Term], &[Proof]) -> Proof,
    ) -> ProofArm {
        let var_ids: Vec<VarId> = (0..vars).map(|_| VarId::fresh()).collect();
        let hyp_ids: Vec<HypId> = (0..hyps).map(|_| HypId::fresh()).collect();
        let terms: Vec<Term> = var_ids.iter().copied().map(Term::Free).collect();
        let proofs: Vec<Proof> = hyp_ids.iter().copied().map(Proof::hyp).collect();
        let mut proof = body(&terms, &proofs);
        for (j, var) in var_ids.iter().enumerate() {
            let depth = Depth::at((vars - 1 - j) as u32);
            proof = proof.rebind(depth, Rebind::CloseVar(*var));
        }
        for (j, id) in hyp_ids.iter().enumerate() {
            let depth = Depth::default().under_hyps((hyps - 1 - j) as u32);
            proof = proof.rebind(depth, Rebind::CloseHyp(*id));
        }
        ProofArm {
            vars: vars as u32,
            hyps: hyps as u32,
            body: Box::new(proof),
        }
    }

    pub(super) fn rebind(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        match self {
            Self::Hyp(HypRef::Free(id)) => match op {
                Rebind::CloseHyp(target) if target == *id => Self::Hyp(HypRef::Bound(depth.hyps)),
                _ => self.clone(),
            },
            Self::Hyp(HypRef::Bound(bound)) => match op {
                Rebind::OpenHyp { index, id } if *bound == depth.hyps + index => {
                    Self::Hyp(HypRef::Free(id))
                }
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
            Self::Definition(term) => Self::Definition(term.rebind(depth, op)),
            Self::CaseStep(term) => Self::CaseStep(term.rebind(depth, op)),
            Self::Construct {
                prop,
                variant,
                params,
                payload,
            } => Self::Construct {
                prop: *prop,
                variant: *variant,
                params: params.iter().map(|term| term.rebind(depth, op)).collect(),
                payload: payload.iter().map(|term| term.rebind(depth, op)).collect(),
            },
            Self::CaseProof {
                scrutinee,
                goal,
                arms,
            } => Self::CaseProof {
                scrutinee: Box::new(scrutinee.rebind(depth, op)),
                goal: goal.rebind(depth, op),
                arms: arms.iter().map(|arm| arm.rebind(depth, op)).collect(),
            },
            Self::CaseData {
                scrutinee,
                goal,
                arms,
            } => Self::CaseData {
                scrutinee: scrutinee.rebind(depth, op),
                goal: goal.rebind(depth, op),
                arms: arms.iter().map(|arm| arm.rebind(depth, op)).collect(),
            },
            Self::ExistsIntro {
                prop,
                witness,
                proof,
            } => Self::ExistsIntro {
                prop: prop.rebind(depth, op),
                witness: witness.rebind(depth, op),
                proof: Box::new(proof.rebind(depth, op)),
            },
            Self::ExistsElim { exists, goal, arm } => Self::ExistsElim {
                exists: Box::new(exists.rebind(depth, op)),
                goal: goal.rebind(depth, op),
                arm: arm.rebind(depth, op),
            },
            Self::ExcludedMiddle(prop) => Self::ExcludedMiddle(prop.rebind(depth, op)),
        }
    }
}

impl ProofArm {
    fn rebind(&self, depth: Depth, op: Rebind<'_>) -> ProofArm {
        ProofArm {
            vars: self.vars,
            hyps: self.hyps,
            body: Box::new(
                self.body
                    .rebind(depth.under_vars(self.vars).under_hyps(self.hyps), op),
            ),
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
            Self::Enum(EnumId(id)) => write!(f, "enum#{id}"),
            Self::Fn(params, result) => {
                f.write_str("math fn(")?;
                for param in params {
                    write!(f, "{param}, ")?;
                }
                write!(f, ") -> {result}")
            }
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
            Self::Fn(FnId(id)) => write!(f, "fn#{id}"),
            Self::Call(callee, arguments) => {
                write!(f, "{callee}(")?;
                write_list(f, arguments)?;
                f.write_str(")")
            }
            Self::Variant(EnumId(id), index, payload) => {
                write!(f, "enum#{id}::{index}(")?;
                write_list(f, payload)?;
                f.write_str(")")
            }
            Self::Case {
                scrutinee, arms, ..
            } => {
                write!(f, "match {scrutinee} {{ ")?;
                for arm in arms {
                    write!(f, "{}, ", arm.body)?;
                }
                f.write_str("}")
            }
            Self::PropApp(PropId(id), arguments) => {
                write!(f, "prop#{id}(")?;
                write_list(f, arguments)?;
                f.write_str(")")
            }
            Self::Exists(ty, body) => write!(f, "exists (#: {ty}) {{ {body} }}"),
            Self::Absurd(_, ty) => write!(f, "absurd: {ty}"),
        }
    }
}
