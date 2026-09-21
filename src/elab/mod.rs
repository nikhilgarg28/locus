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

mod env;
mod exprs;
mod items;
mod logic;
mod order;
mod proofs;
mod show;
mod solve;
mod types;

pub use items::{Elaborated, HoleReport, ItemReport, elaborate};
