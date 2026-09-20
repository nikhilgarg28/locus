//! The Locus proof kernel.
//!
//! This module is independent of the surface syntax: it checks explicit
//! kernel terms and nothing else. It performs no search, no inference beyond
//! reading a proof's conclusion off its structure, and no normalization.
//! Every rule implemented here is stated, with exact premises and conclusion,
//! in `docs/kernel-contract.md`; the two must change together.
//!
//! Implemented so far: gates K1 to K6 of `docs/core-plan.md`.

mod check;
mod classical;
mod context;
mod defs;
pub mod derive;
mod error;
mod nat;
mod term;
pub mod theory;

pub use check::{check_proof, check_type, infer_proof, infer_term, same, same_type};
pub use classical::proof_is_classical;
pub use context::{Context, Mode};
pub use defs::{Definitions, Prelude, PropVariant};
pub use error::KernelError;
pub use nat::Natural;
pub use term::{
    ArmBuilder, Axiom, EnumId, FnId, ForLoop, HypId, HypRef, Prim, Proof, ProofArm, PropId,
    StructId, Term, TermArm, Type, VarId,
};
