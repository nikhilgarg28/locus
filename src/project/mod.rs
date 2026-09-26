//! Source modules and Cargo-hosted generation. This layer supplies resolved,
//! privacy-checked programs to the existing elaborator and kernel.
mod build;
pub use build::{Build, Built};
pub mod cargo;
mod check;
mod export;
mod load;
pub use check::{Checked, check};
pub use export::rust;
mod resolve;
pub use load::{Loaded, load};
pub use resolve::{Access, Export, Graph};

use crate::{diagnostic::Diagnostic, source::SourceMap};

#[derive(Debug)]
pub struct Error {
    pub sources: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for d in &self.diagnostics {
            writeln!(f, "{}", d.render(&self.sources, false))?;
        }
        Ok(())
    }
}
impl std::error::Error for Error {}
/// Driver-level failure using the same source-aware diagnostic format.
pub fn command_error(path: &std::path::Path, message: String) -> Error {
    build::driver("L0505", path, message)
}

pub(crate) mod specs;

pub(crate) use resolve::resolve as resolve_inline;

pub(crate) mod traits;

mod trait_bounds;
