//! The Locus proof kernel.
//!
//! This module is independent of the surface syntax: it checks explicit
//! kernel terms and nothing else. It performs no search, no inference beyond
//! reading a proof's conclusion off its structure, and no normalization.
//! Every rule implemented here is stated, with exact premises and conclusion,
//! in `docs/kernel-contract.md`; the two must change together.
//!
//! Implemented so far: gates K1 to K6 of `docs/kernel-contract.md`.

mod check;
mod classical;
mod context;
mod defs;
mod depth;
pub mod derive;
mod error;
mod eval;
mod nat;
mod term;
pub mod theory;

pub use check::{
    case_variants, check_call, check_proof, check_type, check_values, evaluate_primitive,
    infer_proof, infer_term, same, same_type, telescope_entry, variant_term,
};
pub use classical::proof_is_classical;
pub use context::{Checkpoint, Context, Mode};
pub use defs::{Definitions, Prelude, PropVariant};
pub use depth::MAX_DEPTH;
pub use error::KernelError;
pub use eval::MAX_EVAL_DEPTH;
pub use nat::Natural;
pub use term::{
    ArmBuilder, Axiom, EnumId, FnId, ForLoop, HypId, HypRef, Prim, Proof, ProofArm, PropId,
    StructId, Term, TermArm, Type, VarId,
};
