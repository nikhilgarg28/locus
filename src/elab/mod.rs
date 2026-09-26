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
mod boxed;
mod calls;
mod closures;
mod collections;
mod control;
mod data;
mod dynamic;
mod env;
mod explain;
mod exprs;
mod forms;
mod generics;
mod items;
mod layout;
mod literals;
mod logic;
mod logical_data;
mod loops;
mod models;
mod moves;
mod mutation;
mod native;
mod naturals;
mod operators;
mod order;
mod patterns;
mod proofs;
mod quantifiers;
mod reconcile;
mod references;
mod scope;
mod show;
mod solve;
mod stored;
mod trusted;
mod types;

pub use items::{
    Elaborated, FoundProof, HoleReport, ItemReport, elaborate, elaborate_with,
    elaborate_with_options,
};
pub use solve::certificate_pairs;

use crate::ast;
use crate::source::SourceFile;
use crate::store::ProofStore;

/// Options for one elaboration; never shared across independent checks.
#[derive(Clone, Debug)]
pub struct Options {
    pub pointer_width: crate::kernel::PointerWidth,
    pub previews: crate::preview::Previews,
    pub check_moves: bool,
    pub module_access: Option<std::sync::Arc<crate::project::Access>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            pointer_width: crate::kernel::PointerWidth::HOST,
            previews: crate::preview::Previews::default(),
            check_moves: true,
            module_access: None,
        }
    }
}

impl env::Env<'_> {
    pub(super) fn machine_type(&self, name: &str) -> Option<crate::kernel::MachineInt> {
        self.pointer_width.machine(name)
    }

    pub(super) fn require_preview(
        &mut self,
        feature: crate::preview::Feature,
        construct: &str,
        span: crate::source::Span,
    ) -> env::Elab<()> {
        self.previews
            .require(feature, construct, span)
            .map_err(|diagnostic| {
                self.diagnostics.push(diagnostic);
            })
    }
}

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
    elaborate_with_store_and_options(source, program, store, &Options::default())
}

pub fn elaborate_with_store_and_options(
    source: &SourceFile,
    program: &ast::Program,
    store: ProofStore,
    options: &Options,
) -> (Elaborated, ProofStore) {
    crate::store::with_store(store, || elaborate_with_options(source, program, options))
}
