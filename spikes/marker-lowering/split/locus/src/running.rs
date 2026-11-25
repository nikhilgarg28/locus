//! The runtime expansion's half of the split variant's library: the
//! one-marker library of `spikes/locus-macro`. Every logical value is
//! `LogicalData`, every proof is `Erased`, and the name of each logical type
//! is an alias of the marker. A trait all of whose items are logical is
//! implemented for every type, `Logical` among them.

use core::marker::PhantomData;

/// The marker for proofs.
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

/// It has no items, so every type has it in this expansion.
pub trait Logical {}
impl<T: ?Sized> Logical for T {}

pub type Int = LogicalData;
pub type Nat = LogicalData;
pub type Bool = LogicalData;
pub type Prop = LogicalData;
pub type Snap<T> = <PhantomData<T> as Proj>::Out;

/// The stub of `Seq`, which holds the stubs of its logic functions.
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct __locus_Seq<T> {
    __locus: PhantomData<fn() -> T>,
}
impl<T> Proj for __locus_Seq<T> {
    type Out = LogicalData;
}
pub type Seq<T> = <__locus_Seq<T> as Proj>::Out;

/// The types of which a harness can make a ghost value.
#[doc(hidden)]
pub trait __Marker {
    fn __locus_marker() -> Self;
}
impl __Marker for LogicalData {
    fn __locus_marker() -> Self {
        LogicalData
    }
}
impl __Marker for Erased {
    fn __locus_marker() -> Self {
        Erased
    }
}
pub fn ghost<G: __Marker>() -> G {
    G::__locus_marker()
}
/// A ghost value whose type is a type parameter. In this expansion the
/// parameter is the marker whenever the code runs, and nothing says so to
/// rustc: the value is made from nothing, which is sound for a type with no
/// bytes and no invariant.
pub fn ghost_of_parameter<G>() -> G {
    assert_eq!(core::mem::size_of::<G>(), 0, "a ghost value has no bytes");
    // SAFETY: a zero-sized marker, checked above.
    unsafe { core::mem::MaybeUninit::<G>::uninit().assume_init() }
}

/// An observation: a shared borrow that ends immediately.
pub fn observe<T: ?Sized>(_: &T) {}
/// A name probe for an item used by logical code.
pub fn probe<T>(_: T) {}
/// The body of every stub. Never runs.
pub fn stub<T>() -> T {
    panic!("a Locus stub was reached")
}

pub use locus_macros::{locus, logic, prop, prove};

/// The library's logical traits, lowered by `locus!` as a program's are: in
/// this expansion each is implemented for every type, and the `impl`s
/// written here expand to nothing.
mod traits {
    use locus_macros::locus;

    locus! {
        pub trait AddLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn add(a: Self, b: Rhs) -> Self::Output;
        }
        pub trait FromLogic<T: Logical>: Logical {
            logic fn from(t: T) -> Self;
        }
        pub trait IntoLogic<U: Logical>: Logical {
            logic fn into(self) -> U;
        }
        impl<T: Logical, U: FromLogic<T>> IntoLogic<U> for T {
            logic fn into(self) -> U { U::from(self) }
        }
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

/// What `locus!` imports into each module it expands in, by a glob.
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

// Lowered logical code. This expansion observes and probes nothing.

/// `prove!(..)` and a hole: a proof.
#[macro_export]
macro_rules! proof {
    ($($code:tt)*) => {
        $crate::Erased
    };
}
/// `logic! { .. }` and `prop!(..)`: the marker.
#[macro_export]
macro_rules! data {
    ($($code:tt)*) => {
        $crate::LogicalData
    };
}
/// `let r: M = logic! { .. };` where `M` is a type parameter.
#[macro_export]
macro_rules! generic {
    ($($code:tt)*) => {
        $crate::ghost_of_parameter()
    };
}
/// A proposition in a `let` annotation: nothing, in this expansion.
#[macro_export]
macro_rules! observe {
    ($($place:tt)*) => {};
}
/// The body of a logic function's stub.
#[macro_export]
macro_rules! stub_body {
    ($($names:tt)*) => {
        $crate::stub()
    };
}

/// Which expansion this crate's own code was compiled with, for the spike's
/// harness only: `locus!` never emits it.
#[macro_export]
macro_rules! __expansion {
    () => {
        "runtime"
    };
}
