//! Option (i), checking flavor: a crate named `__locus` with its own markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErasedData;
pub const fn observe<T: ?Sized>(_: &T) {}
#[macro_export]
macro_rules! prove {
    ($($p:expr),* $(,)?) => {{ $( $crate::observe(&$p); )* $crate::Erased }};
}
