//! Option (ii): the one marker crate that both flavors re-export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErasedData;
pub const fn observe<T: ?Sized>(_: &T) {}
