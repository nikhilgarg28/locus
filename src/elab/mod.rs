//! The elaborator: surface syntax to typed trees.
//!
//! Nothing here is trusted. The elaborator resolves names, works out types,
//! fills each `_` with an explicit proof, and hands every item to the
//! `Session`, which lowers it and has the kernel check it. A mistake here
//! produces a program the kernel rejects, never an accepted wrong one.
//!
//! While it works, the elaborator keeps a kernel context that mirrors the
//! one the checker will build, with the same identities, so that it can ask
//! the kernel what a term's type is and test a proof before using it.

mod arithmetic;
mod blocks;
mod calls;
mod control;
mod data;
mod env;
mod explain;
mod exprs;
mod forms;
mod items;
mod literals;
mod logic;
mod loops;
mod moves;
mod mutation;
mod operators;
mod order;
mod patterns;
mod proofs;
mod show;
mod solve;
mod stored;
mod types;

pub use items::{Elaborated, FoundProof, HoleReport, ItemReport, elaborate, elaborate_with};
pub use solve::certificate_pairs;

use crate::ast;
use crate::source::SourceFile;
use crate::store::ProofStore;

/// `elaborate` with the proofs file of the source: each obligation is looked
/// up in `store` before it is searched for, and what the search finds is
/// recorded there (`stored.rs`). The store comes back with what the run did
/// to it. Without a store, `elaborate` searches every obligation and
/// records nothing.
pub fn elaborate_with_store(
    source: &SourceFile,
    program: &ast::Program,
    store: ProofStore,
) -> (Elaborated, ProofStore) {
    crate::store::with_store(store, || elaborate(source, program))
}
