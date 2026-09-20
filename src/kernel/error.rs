//! Kernel rejections. These carry kernel terms, not source locations; turning
//! them into Locus-level diagnostics is the elaborator's job.

use std::fmt;

use super::term::{HypId, Term, Type, VarId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KernelError {
    UnknownVariable(VarId),
    UnknownHypothesis(HypId),
    /// A bound index with no enclosing binder: the term is not well formed.
    DanglingBound,
    /// A ghost variable occurs where a runtime value is required.
    GhostInExecutable(VarId),
    /// A term of a ghost type occurs where a runtime value is required.
    GhostTypeInExecutable(Type),
    TypeMismatch {
        expected: Type,
        found: Type,
    },
    WrongArity {
        expected: usize,
        found: usize,
    },
    /// A proof proves a different proposition from the one required.
    ProofMismatch {
        expected: Box<Term>,
        found: Box<Term>,
    },
    /// A product value or pattern with the wrong number of fields.
    FieldCount {
        expected: usize,
        found: usize,
    },
    NoSuchField {
        index: usize,
        fields: usize,
    },
    NotAProduct(Type),
    UnknownStruct,
    UnknownFunction,
    UnknownEnum,
    UnknownProp,
    NoSuchVariant {
        index: usize,
        variants: usize,
    },
    /// A case must have exactly one arm per variant, each binding exactly
    /// what its variant provides.
    ArmCount {
        expected: usize,
        found: usize,
    },
    ArmBinders {
        expected: (usize, usize),
        found: (usize, usize),
    },
    /// Case analysis needs a `bool`, an enum, or a proof of a declared
    /// proposition.
    NotCaseable(Term),
    /// A parameter of a declared proposition cannot be a proof.
    ProofParameter(Type),
    /// A term-level case cannot have a proof type as its result; case
    /// analysis that produces a proof is a proof rule.
    ProofResult(Type),
    /// Absurdity needs a proof of a proposition with no variants.
    NotEmpty(Term),
    NotExistential(Term),
    /// Excluded middle needs the prelude's `Or` and `False`.
    NoPrelude,
    /// Input nested more deeply than the kernel accepts.
    TooDeep,
    /// A placeholder left by the evaluator was offered as a proof.
    OmittedProof,
    /// Evaluation needs a term with no free variables.
    NotClosed(Term),
    /// Evaluation offers only results that are plain data.
    NotPlainData(Type),
    /// Evaluation found a case where the claim is false.
    Refuted(Term),
    NotAFunction(Type),
    /// A derived form exceeded its step budget.
    StepLimit,
    /// A proof field of a product value must be written as a proof.
    ProofExpected(Term),
    /// `OfTerm` needs a term whose type is a proof type.
    NotAProofType(Type),
    /// Equality between proofs is not a proposition.
    EqualityAtProofType(Type),
    /// A computation axiom was applied to a term it does not reduce.
    NoComputationStep(Term),
    NotAnEquality(Term),
    NotAnImplication(Term),
    NotUniversal(Term),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownVariable(id) => write!(f, "variable {id:?} is not in the context"),
            Self::UnknownHypothesis(id) => write!(f, "hypothesis {id:?} is not in the context"),
            Self::DanglingBound => f.write_str("bound variable without an enclosing binder"),
            Self::GhostInExecutable(id) => {
                write!(
                    f,
                    "ghost variable {id:?} used where a runtime value is required"
                )
            }
            Self::GhostTypeInExecutable(ty) => {
                write!(
                    f,
                    "a term of ghost type {ty} used where a runtime value is required"
                )
            }
            Self::TypeMismatch { expected, found } => {
                write!(f, "expected a term of type {expected}, found {found}")
            }
            Self::WrongArity { expected, found } => {
                write!(f, "expected {expected} arguments, found {found}")
            }
            Self::ProofMismatch { expected, found } => {
                write!(
                    f,
                    "expected a proof of {expected}, found a proof of {found}"
                )
            }
            Self::FieldCount { expected, found } => {
                write!(f, "expected {expected} fields, found {found}")
            }
            Self::NoSuchField { index, fields } => {
                write!(f, "no field {index} in a product of {fields} fields")
            }
            Self::NotAProduct(ty) => write!(f, "expected a tuple or struct type, found {ty}"),
            Self::UnknownStruct => f.write_str("struct is not declared"),
            Self::UnknownFunction => f.write_str("function is not declared"),
            Self::UnknownEnum => f.write_str("enum is not declared"),
            Self::UnknownProp => f.write_str("proposition is not declared"),
            Self::NoSuchVariant { index, variants } => {
                write!(f, "no variant {index} among {variants} variants")
            }
            Self::ArmCount { expected, found } => {
                write!(f, "expected {expected} arms, found {found}")
            }
            Self::ArmBinders { expected, found } => write!(
                f,
                "an arm binds {} variables and {} hypotheses, expected {} and {}",
                found.0, found.1, expected.0, expected.1
            ),
            Self::NotCaseable(term) => write!(f, "case analysis does not apply to {term}"),
            Self::ProofParameter(ty) => {
                write!(f, "a proposition parameter cannot have the proof type {ty}")
            }
            Self::ProofResult(ty) => {
                write!(
                    f,
                    "a term-level case cannot have the proof type {ty} as its result"
                )
            }
            Self::NotEmpty(prop) => write!(f, "{prop} is not a proposition with no variants"),
            Self::NotExistential(prop) => {
                write!(f, "expected an existential proposition, found {prop}")
            }
            Self::NoPrelude => f.write_str("excluded middle needs the prelude declarations"),
            Self::TooDeep => f.write_str("input is nested more deeply than the kernel accepts"),
            Self::OmittedProof => f.write_str("an omitted proof proves nothing"),
            Self::NotClosed(term) => write!(f, "evaluation met the free variable {term}"),
            Self::NotPlainData(ty) => {
                write!(f, "evaluation offers only plain data, and {ty} is not")
            }
            Self::Refuted(term) => write!(f, "refuted by evaluation at {term}"),
            Self::NotAFunction(ty) => write!(f, "expected a function type, found {ty}"),
            Self::StepLimit => f.write_str("derived form exceeded its step budget"),
            Self::ProofExpected(term) => {
                write!(f, "a proof field must be given as a proof, found {term}")
            }
            Self::NotAProofType(ty) => write!(f, "expected a term of proof type, found {ty}"),
            Self::EqualityAtProofType(ty) => {
                write!(f, "equality cannot be formed at the proof type {ty}")
            }
            Self::NoComputationStep(term) => write!(f, "no computation step applies to {term}"),
            Self::NotAnEquality(prop) => write!(f, "expected a proof of an equality, found {prop}"),
            Self::NotAnImplication(prop) => {
                write!(f, "expected a proof of an implication, found {prop}")
            }
            Self::NotUniversal(prop) => {
                write!(f, "expected a proof of a universal claim, found {prop}")
            }
        }
    }
}

impl std::error::Error for KernelError {}
