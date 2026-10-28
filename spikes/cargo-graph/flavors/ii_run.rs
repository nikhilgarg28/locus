//! Option (ii), runtime flavor: `__locus` re-exports the same shared markers.
pub use locus_core::{Erased, ErasedData};
#[macro_export]
macro_rules! prove {
    ($($p:expr),* $(,)?) => { $crate::Erased };
}
