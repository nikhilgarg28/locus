//! A stand-in for Locus's library under the lowering this spike tries:
//! each logical type is a zero-sized Rust type of its own.
#![allow(dead_code)]
use core::marker::PhantomData;

/// Holds for exactly the logical types. `marker` stands for what the library can do privately.
pub trait Logical: Copy {
    fn marker() -> Self;
}

/// Every proof is this one marker, as today: rustc never sees a proposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;
impl Logical for Erased {
    fn marker() -> Self { Erased }
}

macro_rules! logical {
    ($($name:ident),*) => {$(
        #[derive(Clone, Copy, Debug)]
        pub struct $name;
        impl Logical for $name { fn marker() -> Self { $name } }
    )*};
}
logical!(Int, Nat, Bool, Prop);

/// A generic logical type. Zero-sized, `Copy` with no bound on `T`, and it owns no `T`.
pub struct Seq<T>(PhantomData<fn() -> T>);
impl<T> Clone for Seq<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for Seq<T> {}
impl<T: Logical> Logical for Seq<T> { fn marker() -> Self { Seq(PhantomData) } }
impl<T> Seq<T> {
    /// The stub of an inherent logic function.
    pub fn len(self) -> Nat { Nat }
}

impl<A: Logical, B: Logical> Logical for (A, B) {
    fn marker() -> Self { (A::marker(), B::marker()) }
}

/// `Snap<T>` that keeps `T`, lifetimes included.
pub struct SnapTyped<T: ?Sized>(PhantomData<fn() -> T>);
impl<T: ?Sized> Clone for SnapTyped<T> { fn clone(&self) -> Self { *self } }
impl<T: ?Sized> Copy for SnapTyped<T> {}
impl<T: ?Sized> Logical for SnapTyped<T> { fn marker() -> Self { SnapTyped(PhantomData) } }

/// `Snap<T>` as one marker for every `T`, through a projection that forgets `T`.
#[derive(Clone, Copy, Debug)]
pub struct SnapData;
impl Logical for SnapData { fn marker() -> Self { SnapData } }
pub trait HasSnap { type Marker: Logical; }
impl<T: ?Sized> HasSnap for T { type Marker = SnapData; }
pub type Snap<T> = <T as HasSnap>::Marker;

pub fn snap_typed<T: ?Sized>(_: &T) -> SnapTyped<T> { SnapTyped(PhantomData) }
pub fn snap_uniform<T: ?Sized>(_: &T) -> Snap<T> { SnapData }
pub fn use_ghost<G: Logical>(_: G) {}

/// What an expansion of `logic! { .. }` can give. The macro cannot name the logical type
/// of its contents, so the type is left to rustc's inference.
pub fn ghost<G: Logical>() -> G { G::marker() }

/// A ghost value with no logical type as far as rustc knows, and a conversion at each use.
#[derive(Clone, Copy, Debug)]
pub struct Untyped;
pub fn cast<G: Logical>(_: Untyped) -> G { G::marker() }

/// A trait for logical types, with the stub of a logic function.
pub trait OrdLogic: Logical {
    fn le(_a: Self, _b: Self) -> Bool { Bool }
}
impl OrdLogic for Int {}
impl OrdLogic for Nat {}
impl<T: OrdLogic> OrdLogic for Seq<T> {}

/// One trait for both kinds of type.
pub trait Describe { fn describe() -> &'static str; }
impl Describe for u32 { fn describe() -> &'static str { "u32, a runtime type" } }
impl Describe for Nat { fn describe() -> &'static str { "Nat, a logical type" } }
impl Describe for Int { fn describe() -> &'static str { "Int, a logical type" } }

/// The standard library's own traits, implemented for logical types.
impl core::ops::Add for Int { type Output = Int; fn add(self, _: Int) -> Int { Int } }
impl core::ops::Add for Nat { type Output = Nat; fn add(self, _: Nat) -> Nat { Nat } }
impl core::ops::Add<Nat> for Int { type Output = Int; fn add(self, _: Nat) -> Int { Int } }
impl From<Nat> for Int { fn from(_: Nat) -> Int { Int } }

pub fn takes_nat(_: Nat) {}
pub fn takes_int(_: Int) {}
pub fn keep<G: Logical>(g: G) -> G { g }
