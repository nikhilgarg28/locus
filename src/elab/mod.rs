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

mod blocks;
mod calls;
mod control;
mod data;
mod env;
mod exprs;
mod forms;
mod items;
mod logic;
mod loops;
mod operators;
mod order;
mod patterns;
mod proofs;
mod show;
mod solve;
mod types;

pub use items::{Elaborated, FoundProof, HoleReport, ItemReport, elaborate};
