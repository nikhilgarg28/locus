//! Option (i), runtime flavor: another crate named `__locus`, with its own markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErasedData;
#[macro_export]
macro_rules! prove {
    ($($p:expr),* $(,)?) => { $crate::Erased };
}
