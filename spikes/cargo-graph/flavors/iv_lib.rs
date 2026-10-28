//! Option (iv), not in the brief: the one `__locus` crate; `prove!` expands to
//! a `cfg`-gated observation, and the `cfg` is evaluated in the crate where the
//! macro is expanded. The driver passes `--cfg __locus_check` to checking
//! compiles only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErasedData;
pub const fn observe<T: ?Sized>(_: &T) {}
#[macro_export]
macro_rules! prove {
    ($($p:expr),* $(,)?) => {{
        #[cfg(__locus_check)]
        { $( $crate::observe(&$p); )* }
        $crate::Erased
    }};
}
