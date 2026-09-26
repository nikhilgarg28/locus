//! The Locus proof kernel.
//!
//! This module is independent of the surface syntax: it checks explicit
//! kernel terms and nothing else. It performs no search, no inference beyond
//! reading a proof's conclusion off its structure, and no normalization.
//! Every rule implemented here is stated, with exact premises and conclusion,
//! in the kernel contract in `atlas.html`; the two must change together.
//!
//! Implemented so far: gates K1 to K6 of the kernel contract in `atlas.html`,
//! `Int`, the integers of the logic, by axioms and native evaluation, the
//! model of each machine integer type over `Int`, the table of primitive
//! operations on those types, the runtime comparisons at every machine type
//! with their reflection into the order of the views, and the rule that
//! checks a certificate of linear arithmetic.

pub mod buffer;
mod check;
mod classical;
mod context;
mod defs;
mod depth;
pub mod derive;
mod error;
mod eval;
mod generics;
mod int;
mod linear;
mod machine;
mod measured;
mod nat;
mod ops;
mod quantifiers;
mod recursive;
mod term;
pub mod theory;

pub use buffer::BufferOp;
pub use check::{
    case_variants, check_call, check_proof, check_type, check_values, evaluate_primitive,
    infer_proof, infer_term, same, same_type, telescope_entry, variant_term,
};
pub use classical::proof_is_classical;
pub use context::Binding;
pub use context::{Checkpoint, Context, Mode};
pub use defs::{Definitions, Prelude, PropVariant};
pub use depth::MAX_DEPTH;
pub use error::KernelError;
pub use eval::MAX_EVAL_DEPTH;
pub use generics::{GenericDeclaration, GenericId, GenericInstance, TypeBound};
pub use int::Integer;
pub use linear::{
    CertificateText, LINEAR_TEXT_VERSION, LinearError, MAX_LINEAR_ATOMS, MAX_LINEAR_BITS,
    MAX_LINEAR_PAIRS,
};
pub use machine::{MachineInt, PointerWidth};
pub use nat::{Natural, ParseNumberError};
pub use ops::{Op, Panic, Row};
pub use quantifiers::Quantifiers;
pub use term::{
    ArmBuilder, Axiom, CmpOp, EnumId, FnId, ForLoop, HypId, HypRef, Prim, Proof, ProofArm, PropId,
    StructId, Term, TermArm, Type, VarId,
};
