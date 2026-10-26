//! Lowered text of a Locus library `a`, the same for both programs.
pub fn div(n: u32, d: u32, _nonzero: ::__locus::Erased) -> u32 {
    n / d
}
pub fn witness() -> Option<::__locus::Erased> {
    Some(::__locus::prove!())
}
pub struct Pos {
    pub v: u32,
    pub inv: ::__locus::ErasedData,
}
#[doc(hidden)]
pub use ::__locus as __l;
/// Calls Locus's macro through `a`'s own dependency.
#[macro_export]
macro_rules! assert_dollar {
    ($e:expr) => { $crate::__l::prove!($e) };
}
/// Calls Locus's macro by the path the caller's crate sees.
#[macro_export]
macro_rules! assert_abs {
    ($e:expr) => { ::__locus::prove!($e) };
}
