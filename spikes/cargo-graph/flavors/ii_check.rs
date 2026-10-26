//! Option (ii), checking flavor: `__locus` re-exports the shared markers.
pub use locus_core::{Erased, ErasedData, observe};
#[macro_export]
macro_rules! prove {
    ($($p:expr),* $(,)?) => {{ $( $crate::observe(&$p); )* $crate::Erased }};
}
