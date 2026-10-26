//! Locus's library, crate `__locus`, in two flavors from one source.
//! `--cfg locus_check` builds the checking flavor; without it, the runtime
//! flavor. Only the three lowering macros at the end differ.
use std::marker::PhantomData;

/// The marker for proofs. A proof's traits are right for every proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;

/// The marker for logical data: no `==`, `Hash`, `Ord`, or `Default`.
#[derive(Clone, Copy, Debug)]
pub struct ErasedData;

/// Projection, so that a generic alias uses its parameter (no E0091).
pub trait Proj {
    type Out;
}
impl<T: ?Sized> Proj for PhantomData<T> {
    type Out = ErasedData;
}

/// Logical type paths stay as written; rustc resolves them to these.
pub type Int = ErasedData;
pub type Nat = ErasedData;
pub type Prop = ErasedData;
pub type Seq<T> = <PhantomData<T> as Proj>::Out;

/// An observation: a shared borrow that ends immediately.
pub fn observe<T: ?Sized>(_: &T) {}
/// A name probe for an item used by logical code.
pub fn probe<T>(_: T) {}
/// The body of every stub. Never runs.
pub fn stub<T>() -> T {
    panic!("a Locus stub was reached")
}

/// The macros a program writes, for invocations that rustc expands.
pub use locus_macros::{logic, prop, prove};

/// What every module sees, the way it sees std's prelude.
pub mod prelude {
    pub use super::{logic, prop, prove, Int, Nat, Prop, Seq};
}
/// The same, with std's 2024 prelude, for `#[prelude_import]` (unstable).
pub mod prelude_2024 {
    pub use super::prelude::*;
    pub use std::prelude::rust_2024::*;
}

/// `@(P)` lowers to `::__locus::Proof![P]`: the same marker in both flavors.
#[macro_export]
macro_rules! Proof {
    ($($p:tt)*) => { $crate::Erased };
}

/// Option (d) of the spike: the default, for a root that no ghost binding
/// shadows, observes the place precisely. Lowering shadows it with a local
/// `macro_rules! __locus_ghosts` after each binding that changes which names
/// are ghost.
#[macro_export]
macro_rules! __locus_ghosts {
    ($root:ident; $($borrow:tt)*) => { $crate::observe($($borrow)*) };
}

// Lowered logical code: `proof!` for proofs, `data!` for logical data,
// `observe!` for a proposition in a `let` annotation, read after the
// initializer. Each takes `[borrows of the places read] [items named]
// logical code`; lowering writes the `&`, so a borrow error points at the
// place in the program's own file.

#[cfg(locus_check)]
#[macro_export]
macro_rules! proof {
    ([$($place:expr),*] [$($item:expr),*] $($code:tt)*) => {{
        $($crate::observe($place);)* $($crate::probe($item);)* $crate::Erased
    }};
}
#[cfg(locus_check)]
#[macro_export]
macro_rules! data {
    ([$($place:expr),*] [$($item:expr),*] $($code:tt)*) => {{
        $($crate::observe($place);)* $($crate::probe($item);)* $crate::ErasedData
    }};
}
#[cfg(locus_check)]
#[macro_export]
macro_rules! observe {
    ($($place:expr),*) => { $($crate::observe($place);)* };
}

#[cfg(not(locus_check))]
#[macro_export]
macro_rules! proof {
    ($($t:tt)*) => { $crate::Erased };
}
#[cfg(not(locus_check))]
#[macro_export]
macro_rules! data {
    ($($t:tt)*) => { $crate::ErasedData };
}
#[cfg(not(locus_check))]
#[macro_export]
macro_rules! observe {
    ($($t:tt)*) => {};
}
