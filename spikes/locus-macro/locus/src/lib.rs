//! Locus's library for the macro-first spike, crate `locus`. Adapted (copied,
//! not depended on) from `spikes/one-text-lowering/library/locus.rs`.
//!
//! One artifact serves both expansions. The library itself is never compiled
//! differently: its `macro_rules!` put `#[cfg(__locus_check)]` in their
//! *output*, so the `cfg` is evaluated in the crate where they expand. Stock
//! `cargo build` gives the runtime expansion; `--cfg __locus_check` gives the
//! checking one.
use std::marker::PhantomData;

/// The marker for proofs. A proof's traits are right for every proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;

/// The marker for logical data: no `==`, `Hash`, `Ord`, or `Default`.
#[derive(Clone, Copy, Debug)]
pub struct LogicalData;

/// Projection, so that a generic alias uses its parameter (no E0091).
pub trait Proj {
    type Out;
}
impl<T: ?Sized> Proj for PhantomData<T> {
    type Out = LogicalData;
}

/// Logical type paths stay as written; rustc resolves them to these.
pub type Int = LogicalData;
pub type Nat = LogicalData;
pub type Prop = LogicalData;
pub type Seq<T> = <PhantomData<T> as Proj>::Out;

/// An observation: a shared borrow that ends immediately.
pub fn observe<T: ?Sized>(_: &T) {}
/// A name probe for an item used by logical code.
pub fn probe<T>(_: T) {}
/// The body of every stub. Never runs.
pub fn stub<T>() -> T {
    panic!("a Locus stub was reached")
}

/// `locus!` is the macro a program writes around items; `prove!`, `logic!`
/// and `prop!` are for the invocations that rustc expands, because `locus!`
/// did not rewrite them (inside another macro's input).
pub use locus_macros::{locus, logic, prop, prove};

/// What `locus!` imports into each module it expands in, by a glob.
pub mod prelude {
    pub use super::{Int, Nat, Prop, Seq, logic, prop, prove};
}

/// `@(P)` lowers to `::locus::Proof![P]`: the same marker in both expansions.
#[macro_export]
macro_rules! Proof {
    ($($p:tt)*) => {
        $crate::Erased
    };
}

// Lowered logical code: `proof!` for proofs, `data!` for logical data,
// `observe!` for a proposition in a `let` annotation, read after the
// initializer. Each takes `[borrows of the places read] [items named]
// logical code`. The observations and probes are kept only where the
// invocation expands under `--cfg __locus_check`.

#[macro_export]
macro_rules! proof {
    ([$($place:expr),*] [$($item:expr),*] $($code:tt)*) => {{
        #[cfg(__locus_check)]
        { $($crate::observe($place);)* $($crate::probe($item);)* }
        $crate::Erased
    }};
}
#[macro_export]
macro_rules! data {
    ([$($place:expr),*] [$($item:expr),*] $($code:tt)*) => {{
        #[cfg(__locus_check)]
        { $($crate::observe($place);)* $($crate::probe($item);)* }
        $crate::LogicalData
    }};
}
#[macro_export]
macro_rules! observe {
    ($($place:expr),*) => {
        #[cfg(__locus_check)]
        { $($crate::observe($place);)* }
    };
}

/// Which expansion this crate's own code was compiled with, for the spike's
/// harness only: `locus!` never emits it.
#[macro_export]
macro_rules! __expansion {
    () => {{
        #[cfg(__locus_check)]
        let e = "checking";
        #[cfg(not(__locus_check))]
        let e = "runtime";
        e
    }};
}
