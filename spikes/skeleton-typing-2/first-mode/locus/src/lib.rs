//! Locus's library for the marker-lowering spike, crate `locus`. Adapted
//! (copied, not depended on) from `spikes/locus-macro/locus/src/lib.rs`.
//!
//! What changed from that library: each logical type is a zero-sized Rust
//! type of its own, where it had one marker, `LogicalData`, for all of them.
//!
//! As before, one artifact serves both expansions: the `macro_rules!` here
//! put `#[cfg(__locus_check)]` in their *output*, so the `cfg` is evaluated
//! in the crate where they expand.

// `locus!` writes `::locus::..` paths. The library's own logical traits are
// written in Locus, through `locus!`, so the crate names itself.
extern crate self as locus;

use core::fmt;
use core::marker::PhantomData;

/// Holds for exactly the logical types.
///
/// The draft gives it no members. This one has a hidden constructor, so that
/// the expansion of `logic! { .. }` can give a marker whose type rustc
/// infers. `'static` makes rustc refuse a marker that carries a lifetime.
pub trait Logical: Copy + 'static {
    #[doc(hidden)]
    fn __locus_marker() -> Self;
}

/// The marker for every proof: rustc never sees a proposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;
impl Logical for Erased {
    fn __locus_marker() -> Self {
        Erased
    }
}

/// A logical type of the library with no parameters: `Clone`, `Copy`,
/// `Debug` and `Logical`, and no `==`, `Hash`, `Ord`, or `Default`.
macro_rules! marker {
    ($($name:ident),*) => {$(
        #[derive(Clone, Copy, Debug)]
        pub struct $name;
        impl Logical for $name {
            fn __locus_marker() -> Self {
                $name
            }
        }
    )*};
}
marker!(Int, Nat, Bool, Prop);

/// A generic logical type: zero-sized, `Copy` with no bound on `T`, and it
/// owns no `T`.
pub struct Seq<T> {
    __locus: PhantomData<fn() -> T>,
}
impl<T> Clone for Seq<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Seq<T> {}
impl<T> fmt::Debug for Seq<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Seq")
    }
}
impl<T: Logical> Logical for Seq<T> {
    fn __locus_marker() -> Self {
        Seq {
            __locus: PhantomData,
        }
    }
}

/// A logical tuple keeps its shape.
impl<A: Logical, B: Logical> Logical for (A, B) {
    fn __locus_marker() -> Self {
        (A::__locus_marker(), B::__locus_marker())
    }
}
impl<A: Logical, B: Logical, C: Logical> Logical for (A, B, C) {
    fn __locus_marker() -> Self {
        (
            A::__locus_marker(),
            B::__locus_marker(),
            C::__locus_marker(),
        )
    }
}

/// `Snap<T>` is one marker for every `T`: a projection that forgets `T`, and
/// with it `T`'s lifetimes.
#[derive(Clone, Copy, Debug)]
pub struct SnapData;
impl Logical for SnapData {
    fn __locus_marker() -> Self {
        SnapData
    }
}
pub trait HasSnap {
    type Marker: Logical;
}
impl<T: ?Sized> HasSnap for T {
    type Marker = SnapData;
}
pub type Snap<T> = <T as HasSnap>::Marker;

/// What a ghost local holds when only logical code mentions it: a marker
/// with no logical type for rustc. It is not `Logical`, so it cannot be
/// passed where a ghost value of some logical type is wanted.
#[derive(Clone, Copy, Debug)]
pub struct Untyped;

/// What `logic! { .. }` gives anywhere else: a marker of the type that rustc
/// infers from where it is used.
pub fn ghost<G: Logical>() -> G {
    G::__locus_marker()
}

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

/// The library's logical traits, and the stubs of its logic functions.
/// Written in Locus and lowered by `locus!`, as a program's are.
mod traits {
    use locus_macros::locus;

    locus! {
        // Arithmetic. `+` in logical code is `AddLogic::add`.
        pub trait AddLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn add(a: Self, b: Rhs) -> Self::Output;
        }

        // Conversions.
        pub trait FromLogic<T: Logical>: Logical {
            logic fn from(t: T) -> Self;
        }
        pub trait IntoLogic<U: Logical>: Logical {
            logic fn into(self) -> U;
        }
        impl<T: Logical, U: FromLogic<T>> IntoLogic<U> for T {
            logic fn into(self) -> U { U::from(self) }
        }

        // With one marker, the reflexive conversion and `Int` from `Nat`
        // were one implementation written twice (E0119 in the first spike).
        impl<T: Logical> FromLogic<T> for T {
            logic fn from(t: T) -> T { t }
        }
        impl FromLogic<Nat> for Int {
            logic fn from(t: Nat) -> Int { builtin }
        }

        impl AddLogic for Int {
            type Output = Int;
            logic fn add(a: Int, b: Int) -> Int { builtin }
        }
        impl AddLogic for Nat {
            type Output = Nat;
            logic fn add(a: Nat, b: Nat) -> Nat { builtin }
        }

        impl<T> Seq<T> {
            pub logic fn empty() -> Seq<T> { builtin }
            pub logic fn len(self) -> Nat { builtin }
        }
    }
}
pub use traits::*;

/// What `locus!` imports into each module it expands in, by a glob. The
/// library's logical traits come with the hidden traits that hold their
/// logic functions, so that a probe finds those in every module.
pub mod prelude {
    pub use super::traits::*;
    pub use super::{Bool, Int, Logical, Nat, Prop, Seq, Snap, logic, prop, prove};
}

/// `@(P)` lowers to `::locus::Proof![P]`: the same marker in both expansions.
#[macro_export]
macro_rules! Proof {
    ($($p:tt)*) => {
        $crate::Erased
    };
}

// Lowered logical code. Each takes
// `[borrows of the places read] [items probed as values] [names probed] logical code`.
// The observations and probes are kept only where the invocation expands
// under `--cfg __locus_check`.
//
// A probe of a value is an expression, such as `T::__logic_le`. A probe of a
// name is `use name as _;`: it resolves the name and asks rustc to infer
// nothing, which a generic function used as a value would.

#[doc(hidden)]
#[macro_export]
macro_rules! __locus_checks {
    ([$($place:expr),*] [$($item:expr),*] [$($name:path),*]) => {
        #[cfg(__locus_check)]
        {
            $($crate::observe($place);)*
            $($crate::probe($item);)*
            $({ #[allow(unused_imports)] use $name as _; })*
        }
    };
}

/// `prove!(..)` and a hole: a proof.
#[macro_export]
macro_rules! proof {
    ([$($place:tt)*] [$($item:tt)*] [$($name:tt)*] $($code:tt)*) => {{
        $crate::__locus_checks!([$($place)*] [$($item)*] [$($name)*]);
        $crate::Erased
    }};
}
/// `logic! { .. }` whose value runtime code uses, or whose type the program
/// wrote: a marker of the type rustc infers.
#[macro_export]
macro_rules! data {
    ([$($place:tt)*] [$($item:tt)*] [$($name:tt)*] $($code:tt)*) => {{
        $crate::__locus_checks!([$($place)*] [$($item)*] [$($name)*]);
        $crate::ghost()
    }};
}
/// `logic! { .. }` bound to a ghost local that only logical code mentions.
#[macro_export]
macro_rules! untyped {
    ([$($place:tt)*] [$($item:tt)*] [$($name:tt)*] $($code:tt)*) => {{
        $crate::__locus_checks!([$($place)*] [$($item)*] [$($name)*]);
        $crate::Untyped
    }};
}
/// `prop!(..)`: a proposition as a value. Its type is always `Prop`.
#[macro_export]
macro_rules! prop_value {
    ([$($place:tt)*] [$($item:tt)*] [$($name:tt)*] $($code:tt)*) => {{
        $crate::__locus_checks!([$($place)*] [$($item)*] [$($name)*]);
        $crate::Prop
    }};
}
/// A proposition in a `let` annotation, read after the initializer.
#[macro_export]
macro_rules! observe {
    ($($place:expr),*) => {
        #[cfg(__locus_check)]
        { $($crate::observe($place);)* }
    };
}
/// The body of a logic function's stub: probes of the names its logical code
/// uses, then a panic.
#[macro_export]
macro_rules! stub_body {
    ([$($item:tt)*] [$($name:tt)*]) => {{
        $crate::__locus_checks!([] [$($item)*] [$($name)*]);
        $crate::stub()
    }};
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
