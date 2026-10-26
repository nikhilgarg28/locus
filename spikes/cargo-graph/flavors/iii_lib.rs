//! Option (iii): the one `__locus` crate; its `prove!` forwards `$crate` to
//! the flavor-switching procedural macro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErasedData;
pub const fn observe<T: ?Sized>(_: &T) {}
#[doc(hidden)]
pub use locus_macros as __pm;
#[macro_export]
macro_rules! prove {
    ($($t:tt)*) => { $crate::__pm::prove!{$crate; $($t)*} };
}
