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
        expected: Term,
        found: Term,
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
