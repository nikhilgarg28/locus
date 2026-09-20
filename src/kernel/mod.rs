//! The Locus proof kernel.
//!
//! This module is independent of the surface syntax: it checks explicit
//! kernel terms and nothing else. It performs no search, no inference beyond
//! reading a proof's conclusion off its structure, and no normalization.
//! Every rule implemented here is stated, with exact premises and conclusion,
//! in `docs/kernel-contract.md`; the two must change together.
//!
//! Implemented so far: gates K1 to K3 of `docs/core-plan.md`.

mod check;
mod context;
mod defs;
pub mod derive;
mod error;
mod term;

pub use check::{check_proof, check_type, infer_proof, infer_term, same, same_type};
pub use context::{Context, Mode};
pub use defs::Definitions;
pub use error::KernelError;
pub use term::{FnId, HypId, HypRef, Prim, Proof, StructId, Term, Type, VarId};
