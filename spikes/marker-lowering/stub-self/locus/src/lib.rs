//! Locus's library for the spike's second mode, "stub-self", crate `locus`.
//!
//! Program text is lowered as `spikes/locus-macro` lowers it: every logical
//! value is the one marker `LogicalData`, every proof is `Erased`, and the
//! name of each logical type is an alias of the marker. Beside that, each
//! logical type has a stub: a zero-sized type of its own, `__locus_Int`,
//! `__locus_Seq<T>`, of which no value is ever made. A stub stands only
//! where an `impl` names a logical type, so that implementations for two
//! logical types are implementations for two Rust types. Questions about
//! stubs are asked of rustc afterwards, by the driver in ../oracle.
//!
//! As in the first mode, one artifact serves both expansions.

// `locus!` writes `::locus::..` paths, and the library's own logical traits
// are written in Locus, through `locus!`.
extern crate self as locus;

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

/// Holds for the marker, for `Erased`, and for every stub. It has a hidden
/// constructor, for the one place where runtime code needs a ghost value
/// whose type is a type parameter. A stub's constructor is never run.
pub trait Logical: Copy + 'static {
    #[doc(hidden)]
    fn __locus_marker() -> Self;
}
impl Logical for LogicalData {
    fn __locus_marker() -> Self {
        LogicalData
    }
}
impl Logical for Erased {
    fn __locus_marker() -> Self {
        Erased
    }
}

/// A logical tuple keeps its shape, of markers in program text and of stubs
/// where an `impl` gives one as an associated type.
impl<A: Logical, B: Logical> Logical for (A, B) {
    fn __locus_marker() -> Self {
        (A::__locus_marker(), B::__locus_marker())
    }
}
impl<A: Logical, B: Logical, C: Logical> Logical for (A, B, C) {
    fn __locus_marker() -> Self {
        (A::__locus_marker(), B::__locus_marker(), C::__locus_marker())
    }
}

/// A stub of the library: a zero-sized type of its own for a logical type
/// with no parameters, and the alias that program text uses.
macro_rules! logical {
    ($($name:ident $stub:ident),*) => {$(
        pub type $name = LogicalData;
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy)]
        pub struct $stub;
        impl Proj for $stub {
            type Out = LogicalData;
        }
        impl Logical for $stub {
            fn __locus_marker() -> Self {
                stub()
            }
        }
        impl SnapOf for $stub {
            type S = $stub;
        }
        impl __Stub for $stub {}
        impl __Kind for $stub {
            type K = __StubKind;
        }
    )*};
}
logical!(Int __locus_Int, Nat __locus_Nat, Bool __locus_Bool, Prop __locus_Prop);

/// The stub of a generic logical type keeps its parameter and owns none.
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct __locus_Seq<T> {
    __locus: PhantomData<fn() -> T>,
}
impl<T> Clone for __locus_Seq<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for __locus_Seq<T> {}
impl<T> Proj for __locus_Seq<T> {
    type Out = LogicalData;
}
impl<T: Logical> Logical for __locus_Seq<T> {
    fn __locus_marker() -> Self {
        stub()
    }
}
impl<T> SnapOf for __locus_Seq<T> {
    type S = __locus_Seq<T>;
}
impl<T> __Stub for __locus_Seq<T> {}
impl<T> __Kind for __locus_Seq<T> {
    type K = __StubKind;
}
pub type Seq<T> = <__locus_Seq<T> as Proj>::Out;

/// `Snap<T>` is the marker, for every `T`, in program text.
pub type Snap<T> = <PhantomData<T> as Proj>::Out;

/// The snapshot family, for the driver's questions only: the stub that
/// stands for `Snap<T>`. No value has these types, so a lifetime in `T`
/// keeps no borrow. The snapshot of a vector is a sequence of snapshots,
/// the snapshot of a logical type is that type, a reference is seen through,
/// and any other runtime type has an opaque stub of its own.
pub trait SnapOf {
    type S;
}
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct __locus_Snap<T: ?Sized> {
    __locus: PhantomData<fn() -> T>,
}
impl<T: ?Sized> Clone for __locus_Snap<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for __locus_Snap<T> {}
impl<T: ?Sized + 'static> Logical for __locus_Snap<T> {
    fn __locus_marker() -> Self {
        stub()
    }
}
impl<T: SnapOf> SnapOf for Vec<T> {
    type S = __locus_Seq<T::S>;
}
impl<T: SnapOf + ?Sized> SnapOf for &T {
    type S = T::S;
}
impl<T: SnapOf + ?Sized> SnapOf for &mut T {
    type S = T::S;
}
macro_rules! opaque_snapshots {
    ($($t:ty),*) => {$(
        impl SnapOf for $t {
            type S = __locus_Snap<$t>;
        }
    )*};
}
opaque_snapshots!(u8, u32, usize, bool, String);

/// For three hand-written cases about a program's blanket implementation
/// beside the marker's: a helper trait that only stubs implement, and an
/// associated type that tells a stub from the marker.
#[doc(hidden)]
pub trait __Stub {}
#[doc(hidden)]
pub trait __Kind {
    type K;
}
#[doc(hidden)]
pub struct __StubKind;
#[doc(hidden)]
pub struct __MarkerKind;
impl __Kind for LogicalData {
    type K = __MarkerKind;
}

/// The helper trait does tell stubs from the marker here, in the crate that
/// declares both: an implementation for every stub beside the marker's. A
/// program's crate cannot rely on it (case s11).
#[doc(hidden)]
pub trait __InLibrary {}
impl<T: Logical + __Stub> __InLibrary for T {}
impl __InLibrary for LogicalData {}

/// A ghost value whose type is a type parameter, or the argument of a
/// harness: the marker of that type.
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

pub use locus_macros::{locus, logic, prop, prove};

/// The built-in exception to "a logical type implements no trait with a
/// method that runs": beside `Clone`, `Copy` and `Debug`, the law trait
/// `CloneEq`, implemented once for the marker and once for `Erased`. In
/// Locus its method is `fn clone_eq(&self) -> (copy: Self, @(snap(copy) === snap(*self)))`.
pub trait CloneEq: Clone {
    fn clone_eq(&self) -> (Self, Erased);
}
impl CloneEq for LogicalData {
    fn clone_eq(&self) -> (Self, Erased) {
        (*self, Erased)
    }
}
impl CloneEq for Erased {
    fn clone_eq(&self) -> (Self, Erased) {
        (*self, Erased)
    }
}
impl CloneEq for u32 {
    fn clone_eq(&self) -> (Self, Erased) {
        (*self, Erased)
    }
}
/// One adapter, for every element type, the marker included.
impl<T: CloneEq> CloneEq for Vec<T> {
    fn clone_eq(&self) -> (Self, Erased) {
        (self.iter().map(|x| x.clone_eq().0).collect(), Erased)
    }
}

/// The library's logical traits, and the stubs of its logic functions,
/// written in Locus and lowered by `locus!`, as a program's are.
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
    pub use super::{Bool, CloneEq, Int, Logical, Nat, Prop, Seq, Snap, logic, prop, prove};
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
// In this mode every probe is of a name: `use name as _;`.

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
/// `logic! { .. }` and `prop!(..)`: the marker.
#[macro_export]
macro_rules! data {
    ([$($place:tt)*] [$($item:tt)*] [$($name:tt)*] $($code:tt)*) => {{
        $crate::__locus_checks!([$($place)*] [$($item)*] [$($name)*]);
        $crate::LogicalData
    }};
}
/// `let r: M = logic! { .. };` where `M` is a type parameter: a marker of
/// that type.
#[macro_export]
macro_rules! generic {
    ([$($place:tt)*] [$($item:tt)*] [$($name:tt)*] $($code:tt)*) => {{
        $crate::__locus_checks!([$($place)*] [$($item)*] [$($name)*]);
        $crate::ghost()
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
/// The names that an item's logical code uses, where there is no body to
/// hold their probes: the logic functions of an `impl` for a logical type.
#[macro_export]
macro_rules! names {
    ($($name:path),*) => {
        #[cfg(__locus_check)]
        const _: () = {
            $({ #[allow(unused_imports)] use $name as _; })*
        };
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
