//! Kernel rejections. These carry kernel terms, not source locations; turning
//! them into Locus-level diagnostics is the elaborator's job.

use std::fmt;

use super::linear::LinearError;
use super::machine::MachineInt;
use super::ops::Op;
use super::term::{HypId, Term, Type, VarId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KernelError {
    UnknownVariable(VarId),
    UnknownHypothesis(HypId),
    /// A bound index with no enclosing binder: the term is not well formed.
    DanglingBound,
    /// A ghost variable occurs where a runtime value is required.
    GhostInExecutable(VarId),
    /// A math function with no runtime form is named where a runtime value
    /// is required: its body needs a ghost value to compute its result.
    LogicalFunctionInExecutable,
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
    /// An identity chosen by the caller is already bound in this context.
    DuplicateBinding,
    /// Input nested more deeply than the kernel accepts.
    TooDeep,
    /// A placeholder left by the evaluator was offered as a proof.
    OmittedProof,
    /// Evaluation nested more deeply than the kernel allows.
    EvaluationTooDeep,
    /// Evaluation needs a term with no free variables.
    NotClosed(Term),
    /// Evaluation offers only results that are plain data.
    NotPlainData(Type),
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
    /// `u8` has one spelling, `Type::U8` and `Term::U8`; the machine forms
    /// at `MachineInt::U8` are not terms.
    MachineFormOfU8,
    /// A machine integer literal whose value is outside its type's range.
    OutOfRange(Term),
    /// A linear certificate refused by the rule itself, not by the proof
    /// of one of its pairs.
    Linear(LinearError),
    /// The table of primitive operations has no row for this operation at
    /// this type: a negation at an unsigned type.
    NoRow(Op, MachineInt),
    /// `op_exact` at a row that cannot overflow: a wrapping method, `/`,
    /// or `%`, whose only axiom is `op_model`.
    NoOverflow(Op, MachineInt),
}

impl From<LinearError> for KernelError {
    fn from(error: LinearError) -> Self {
        Self::Linear(error)
    }
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
            Self::LogicalFunctionInExecutable => {
                f.write_str("a logical-only function used where a runtime value is required")
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
            Self::DuplicateBinding => f.write_str("the identity is already bound in this context"),
            Self::TooDeep => f.write_str("input is nested more deeply than the kernel accepts"),
            Self::OmittedProof => f.write_str("an omitted proof proves nothing"),
            Self::EvaluationTooDeep => {
                f.write_str("evaluation nested more deeply than the kernel allows")
            }
            Self::NotClosed(term) => write!(f, "evaluation met the free variable {term}"),
            Self::NotPlainData(ty) => {
                write!(f, "evaluation offers only plain data, and {ty} is not")
            }
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
            Self::MachineFormOfU8 => f.write_str("u8 is written u8, not as a machine form"),
            Self::OutOfRange(term) => write!(f, "{term} is outside the range of its type"),
            Self::Linear(error) => write!(f, "linear certificate: {error}"),
            Self::NoRow(op, ty) => {
                write!(f, "the table has no row for {} at {}", op.name(), ty.name())
            }
            Self::NoOverflow(op, ty) => write!(
                f,
                "{}[{}] cannot overflow, so op_exact does not apply to it",
                op.name(),
                ty.name()
            ),
        }
    }
}

impl std::error::Error for KernelError {}
