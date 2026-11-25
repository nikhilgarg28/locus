//! Locus's library for the skeleton-typing spike, crate `locus`. Adapted
//! (copied, not depended on) from the first mode of
//! `spikes/marker-lowering/locus/src/lib.rs`.
//!
//! What changed from that library:
//! - `sk` holds the stubs that a typing skeleton is written over: logical
//!   code, translated to a Rust expression inside a closure that is never
//!   called, so that rustc types it;
//! - `ModelOf` is what a place read as a value is, a projection: a runtime
//!   type's model, and a logical type itself;
//! - `Snap<T>` is a type family, `<T as Snapshot>::S`, where it was one
//!   marker for every `T`.
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
pub trait Logical: Copy + 'static + ModelOf<Logic = Self> + Snapshot<S = Self> {
    #[doc(hidden)]
    fn __locus_marker() -> Self;
}

/// What a place read as a value is, in logical code: a runtime type's model,
/// and a logical type itself. A type with no implementation has no model.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no model, so a value of it cannot be used as a value in logical code",
    label = "this is read as a value",
    note = "declare a model for `{Self}`, or write `snap(..)` to use its snapshot"
)]
pub trait ModelOf {
    type Logic;
}

/// The snapshot family: `Snap<T>` is `<T as Snapshot>::S`, which rustc
/// normalizes. No snapshot type has a lifetime parameter.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no snapshot type that rustc can name",
    label = "its snapshot is taken here",
    note = "a type parameter needs the bound `Snapshot`, and a type of another crate needs the library to declare its snapshot"
)]
pub trait Snapshot {
    type S: Logical;
}
pub type Snap<T> = <T as Snapshot>::S;

/// A logical type: it is its own model and its own snapshot.
macro_rules! own {
    ([$($p:ident),*] $t:ty) => {
        impl<$($p),*> ModelOf for $t {
            type Logic = Self;
        }
        impl<$($p: Logical),*> Snapshot for $t {
            type S = Self;
        }
    };
}

/// The marker for every proof: rustc never sees a proposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Erased;
own!([] Erased);
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
        own!([] $name);
        impl Logical for $name {
            fn __locus_marker() -> Self {
                $name
            }
        }
    )*};
}
marker!(Int, Nat, Bool, Prop);

/// The propositions `True` and `False`, as values for a typing skeleton.
#[allow(non_upper_case_globals)]
pub const True: Prop = Prop;
#[allow(non_upper_case_globals)]
pub const False: Prop = Prop;

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
own!([T] Seq<T>);
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

// ---------------------------------------------------------------- models

/// A runtime type with a model: where logical code passes a place of this
/// type to a parameter of its model's type, the place is taken to its model.
/// No logical type has it, so that an argument of a logical type is only
/// ever itself.
pub trait Runtime: ModelOf {}

/// The models that Locus's library declares for the standard library's
/// types (RFC 0002): an unsigned integer is a `Nat`, a signed one an `Int`,
/// `bool` is `Bool`, a reference and a `Box` are transparent, and a vector,
/// a slice and an array are a `Seq` of their elements' models.
macro_rules! models {
    ($($model:ident: $($t:ty),*;)*) => {$($(
        impl ModelOf for $t {
            type Logic = $model;
        }
        impl Runtime for $t {}
    )*)*};
}
models! {
    Nat: u8, u16, u32, u64, u128, usize;
    Int: i8, i16, i32, i64, i128, isize;
    Bool: bool;
}
impl<T: ModelOf + ?Sized> ModelOf for &T {
    type Logic = T::Logic;
}
impl<T: Runtime + ?Sized> Runtime for &T {}
impl<T: ModelOf + ?Sized> ModelOf for &mut T {
    type Logic = T::Logic;
}
impl<T: Runtime + ?Sized> Runtime for &mut T {}
impl<T: ModelOf + ?Sized> ModelOf for Box<T> {
    type Logic = T::Logic;
}
impl<T: Runtime + ?Sized> Runtime for Box<T> {}
impl<T: ModelOf> ModelOf for Vec<T> {
    type Logic = Seq<T::Logic>;
}
impl<T: ModelOf> Runtime for Vec<T> {}
impl<T: ModelOf> ModelOf for [T] {
    type Logic = Seq<T::Logic>;
}
impl<T: ModelOf> Runtime for [T] {}
impl<T: ModelOf, const N: usize> ModelOf for [T; N] {
    type Logic = Seq<T::Logic>;
}
impl<T: ModelOf, const N: usize> Runtime for [T; N] {}
/// A tuple's model is the tuple of its components' models, so a tuple of
/// logical types is its own. A tuple is not `Runtime`: rustc cannot be told
/// "a tuple with at least one runtime component" by two implementations
/// that do not overlap.
impl<A: ModelOf, B: ModelOf> ModelOf for (A, B) {
    type Logic = (A::Logic, B::Logic);
}
impl<A: ModelOf, B: ModelOf, C: ModelOf> ModelOf for (A, B, C) {
    type Logic = (A::Logic, B::Logic, C::Logic);
}

// ------------------------------------------------------------- snapshots

/// A machine integer's snapshot: a bounded integer, a type of its own for
/// each machine type, so that `Snap<u32>` is not `Snap<u8>`.
macro_rules! bounded {
    ($($name:ident: $t:ty),*) => {$(
        #[derive(Clone, Copy, Debug)]
        pub struct $name;
        own!([] $name);
        impl Logical for $name {
            fn __locus_marker() -> Self {
                $name
            }
        }
        impl Snapshot for $t {
            type S = $name;
        }
    )*};
}
bounded!(
    BoundedU8: u8, BoundedU16: u16, BoundedU32: u32, BoundedU64: u64, BoundedU128: u128,
    BoundedUsize: usize, BoundedI8: i8, BoundedI16: i16, BoundedI32: i32, BoundedI64: i64,
    BoundedI128: i128, BoundedIsize: isize
);
impl Snapshot for bool {
    type S = Bool;
}
/// A reference and a `Box` are transparent: their snapshot is their
/// target's, so it has none of their lifetimes.
impl<T: Snapshot + ?Sized> Snapshot for &T {
    type S = T::S;
}
impl<T: Snapshot + ?Sized> Snapshot for &mut T {
    type S = T::S;
}
impl<T: Snapshot + ?Sized> Snapshot for Box<T> {
    type S = T::S;
}
impl<T: Snapshot> Snapshot for Vec<T> {
    type S = Seq<T::S>;
}
impl<T: Snapshot> Snapshot for [T] {
    type S = Seq<T::S>;
}
impl<T: Snapshot, const N: usize> Snapshot for [T; N] {
    type S = Seq<T::S>;
}
impl<A: Snapshot, B: Snapshot> Snapshot for (A, B) {
    type S = (A::S, B::S);
}
impl<A: Snapshot, B: Snapshot, C: Snapshot> Snapshot for (A, B, C) {
    type S = (A::S, B::S, C::S);
}

/// A snapshot that nothing looks into, for a type of the standard library:
/// one type for each such type, generic over its arguments' snapshots.
macro_rules! opaque {
    ($($name:ident [$($p:ident),*]: $t:ty;)*) => {$(
        pub struct $name<$($p),*> {
            __locus: PhantomData<fn() -> ($($p,)*)>,
        }
        impl<$($p),*> Clone for $name<$($p),*> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<$($p),*> Copy for $name<$($p),*> {}
        impl<$($p),*> fmt::Debug for $name<$($p),*> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(stringify!($name))
            }
        }
        own!([$($p),*] $name<$($p),*>);
        impl<$($p: Logical),*> Logical for $name<$($p),*> {
            fn __locus_marker() -> Self {
                $name { __locus: PhantomData }
            }
        }
        impl<$($p: Snapshot),*> Snapshot for $t {
            type S = $name<$($p::S),*>;
        }
    )*};
}
opaque! {
    CellSnap [T]: core::cell::Cell<T>;
    RefCellSnap [T]: core::cell::RefCell<T>;
    OptionSnap [T]: Option<T>;
    StringSnap []: String;
    StrSnap []: str;
}
/// A raw pointer's snapshot is its address.
#[derive(Clone, Copy, Debug)]
pub struct Address;
own!([] Address);
impl Logical for Address {
    fn __locus_marker() -> Self {
        Address
    }
}
impl<T: ?Sized> Snapshot for *const T {
    type S = Address;
}
impl<T: ?Sized> Snapshot for *mut T {
    type S = Address;
}

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

// ------------------------------------------------------- typing skeletons

/// The stubs that a typing skeleton is written over. `locus!` translates
/// logical code into a Rust expression over these and over the stubs of the
/// program's logic functions, and puts it in a closure that is never
/// called. rustc types it; nothing here ever runs, but for `typed`, `proved`,
/// `prop_value` and `holds`, which drop the closure and give the marker.
#[doc(hidden)]
pub mod sk {
    use super::*;

    // What takes a skeleton.

    /// `logic! { .. }`: a marker of the type of the logical code.
    pub fn typed<G: Logical>(_: impl FnOnce() -> G) -> G {
        G::__locus_marker()
    }
    /// `prove!(P)`: a proof. The skeleton is the proposition's.
    pub fn proved<P: Truth>(_: impl FnOnce() -> P) -> Erased {
        Erased
    }
    /// `prop!(P)`: the proposition as a value.
    pub fn prop_value<P: Truth>(_: impl FnOnce() -> P) -> Prop {
        Prop
    }
    /// A proposition in a `let` annotation, read after the initializer.
    pub fn holds<P: Truth>(_: impl FnOnce() -> P) {}
    /// The same with the named components of the annotation's tuples in scope.
    pub fn holds1<A, P: Truth>(_: impl FnOnce(A) -> P) {}
    pub fn holds2<A, B, P: Truth>(_: impl FnOnce(A, B) -> P) {}
    pub fn holds3<A, B, C, P: Truth>(_: impl FnOnce(A, B, C) -> P) {}
    /// The body of a logic function's stub: its type is the function's result.
    pub fn body<T>(_: impl FnOnce() -> T) -> T {
        stub()
    }
    /// The body of a logic function that returns `Prop`: a `Bool` is a proposition.
    pub fn prop_body<P: Truth>(_: impl FnOnce() -> P) -> Prop {
        stub()
    }

    // Reading places.

    /// A place read as a value: its type's model, or itself if it is logical.
    pub fn model_of<T: ModelOf + ?Sized>(_: &T) -> T::Logic {
        stub()
    }
    /// A place as an argument, a field's initializer, or the result of a
    /// logic function: the type that is wanted there selects what the place
    /// is read as, the place itself or its model.
    pub fn arg<T: ArgAs<P, Via> + ?Sized, P, Via>(_: &T) -> P {
        stub()
    }
    /// `snap(place)`.
    pub fn snap<T: Snapshot + ?Sized>(_: &T) -> Snap<T> {
        stub()
    }
    /// `final!(r)`: the place that `r` ends with, as a place of `r`'s target type.
    pub fn final_of<'a, T: ?Sized>(_: &'a &mut T) -> &'a T {
        stub()
    }

    pub struct ViaSelf;
    pub struct ViaModel;
    pub struct ViaWiden;
    pub struct ViaModelWiden;
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` cannot be given where a `{P}` is wanted in logical code",
        label = "this place is a `{Self}`",
        note = "a place is given as it is, or as its type's model"
    )]
    pub trait ArgAs<P, Via> {}
    impl<T> ArgAs<T, ViaSelf> for T {}
    impl<T: Runtime + ?Sized> ArgAs<T::Logic, ViaModel> for T {}

    // Literals, holes and proofs.

    /// An integer literal: an `Int` or a `Nat`, whichever rustc infers.
    pub trait Literal {}
    impl Literal for Int {}
    impl Literal for Nat {}
    pub fn lit<L: Literal>(_: u128) -> L {
        stub()
    }
    /// An integer literal beside another operand: it has that operand's type.
    pub fn lit_like<L: Literal>(_: &L, _: u128) -> L {
        stub()
    }
    /// `true` and `false`.
    pub fn truth(_: bool) -> Bool {
        stub()
    }
    /// A hole `_`: whatever is wanted there, a proof or a value.
    pub fn hole<T>() -> T {
        stub()
    }
    /// A logical closure whose body is a proof: a proof of a `forall`.
    pub fn lambda1<A>(_: impl Fn(A) -> Erased) -> Erased {
        stub()
    }
    pub fn lambda2<A, B>(_: impl Fn(A, B) -> Erased) -> Erased {
        stub()
    }
    pub fn lambda3<A, B, C>(_: impl Fn(A, B, C) -> Erased) -> Erased {
        stub()
    }

    // Propositions.

    /// What stands where a proposition is wanted: a `Prop`, or a `Bool`.
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` is not a proposition",
        label = "a `Prop` or a `Bool` is wanted here"
    )]
    pub trait Truth {}
    impl Truth for Bool {}
    impl Truth for Prop {}
    /// `&&` and `||`: a `Bool` on two `Bool`s, and otherwise a `Prop`.
    pub trait Connective<Rhs> {
        type Output: Truth;
    }
    impl Connective<Bool> for Bool {
        type Output = Bool;
    }
    impl Connective<Prop> for Bool {
        type Output = Prop;
    }
    impl Connective<Bool> for Prop {
        type Output = Prop;
    }
    impl Connective<Prop> for Prop {
        type Output = Prop;
    }
    pub fn and<A: Connective<B>, B>(_: A, _: B) -> A::Output {
        stub()
    }
    pub fn or<A: Connective<B>, B>(_: A, _: B) -> A::Output {
        stub()
    }
    pub fn not<A: Truth>(_: A) -> A {
        stub()
    }
    pub fn implies<A: Truth, B: Truth>(_: A, _: B) -> Prop {
        stub()
    }
    pub fn iff<A: Truth, B: Truth>(_: A, _: B) -> Prop {
        stub()
    }
    /// `===` and `!==`: on two values of one type, whatever it is.
    pub fn same<T>(_: T, _: T) -> Prop {
        stub()
    }
    pub fn differ<T>(_: T, _: T) -> Prop {
        stub()
    }
    /// The condition of an `if` in logical code. Rust wants a `bool` there.
    pub fn cond<C: Truth>(_: C) -> bool {
        stub()
    }
    pub fn forall1<A, P: Truth>(_: impl Fn(A) -> P) -> Prop {
        stub()
    }
    pub fn forall2<A, B, P: Truth>(_: impl Fn(A, B) -> P) -> Prop {
        stub()
    }
    pub fn forall3<A, B, C, P: Truth>(_: impl Fn(A, B, C) -> P) -> Prop {
        stub()
    }
    pub fn exists1<A, P: Truth>(_: impl Fn(A) -> P) -> Prop {
        stub()
    }
    pub fn exists2<A, B, P: Truth>(_: impl Fn(A, B) -> P) -> Prop {
        stub()
    }
    pub fn exists3<A, B, C, P: Truth>(_: impl Fn(A, B, C) -> P) -> Prop {
        stub()
    }

    // Comparisons. Rust's `==` and `<` give a `bool`, whatever the
    // implementation, so they are stubs that give a `Bool`.

    #[diagnostic::on_unimplemented(
        message = "`==` is not defined on a `{Self}` and a `{Rhs}` in logical code",
        label = "no `==` for these",
        note = "`===` compares two values of any one logical type"
    )]
    pub trait EqLogic<Rhs = Self> {}
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` and a `{Rhs}` have no order in logical code",
        label = "no order for these"
    )]
    pub trait OrdLogic<Rhs = Self> {}
    macro_rules! comparable {
        ($($a:ident $b:ident),*) => {$(
            impl EqLogic<$b> for $a {}
            impl OrdLogic<$b> for $a {}
        )*};
    }
    comparable!(Int Int, Nat Nat);
    impl EqLogic for Bool {}
    pub fn eq<A: EqLogic<B>, B>(_: A, _: B) -> Bool {
        stub()
    }
    pub fn ne<A: EqLogic<B>, B>(_: A, _: B) -> Bool {
        stub()
    }
    pub fn lt<A: OrdLogic<B>, B>(_: A, _: B) -> Bool {
        stub()
    }
    pub fn le<A: OrdLogic<B>, B>(_: A, _: B) -> Bool {
        stub()
    }
    pub fn gt<A: OrdLogic<B>, B>(_: A, _: B) -> Bool {
        stub()
    }
    pub fn ge<A: OrdLogic<B>, B>(_: A, _: B) -> Bool {
        stub()
    }

    // Arithmetic: the logic functions of the library's logical traits.

    pub fn add<A: AddLogic<B>, B: Logical>(_: A, _: B) -> A::Output {
        stub()
    }
    pub fn sub<A: SubLogic<B>, B: Logical>(_: A, _: B) -> A::Output {
        stub()
    }
    pub fn mul<A: MulLogic<B>, B: Logical>(_: A, _: B) -> A::Output {
        stub()
    }
    pub fn div<A: DivLogic<B>, B: Logical>(_: A, _: B) -> A::Output {
        stub()
    }
    pub fn rem<A: RemLogic<B>, B: Logical>(_: A, _: B) -> A::Output {
        stub()
    }
    pub fn neg<A: NegLogic>(_: A) -> A::Output {
        stub()
    }

    // Indexing and matching.

    pub trait IndexLogic {
        type Item;
    }
    impl<T> IndexLogic for Seq<T> {
        type Item = T;
    }
    /// `xs[i]`. The index is a `Nat` here: RFC 0002 leaves it open.
    pub fn index<S: IndexLogic>(_: S, _: Nat) -> S::Item {
        stub()
    }

    /// What a `match` in logical code is on. A logical enum is a zero-sized
    /// struct with no payload, so that it can contain itself, and its
    /// variants are those of an enum beside it, its view. A proof is matched
    /// through the view of its proposition, which the patterns name.
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` cannot be matched in logical code",
        label = "no variants or fields that rustc knows"
    )]
    pub trait HasView<V> {}
    pub trait PropView {}
    impl<V: PropView> HasView<V> for Erased {}
    pub fn view<T: HasView<V>, V>(_: T) -> V {
        stub()
    }
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

        pub trait SubLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn sub(a: Self, b: Rhs) -> Self::Output;
        }
        pub trait MulLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn mul(a: Self, b: Rhs) -> Self::Output;
        }
        pub trait DivLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn div(a: Self, b: Rhs) -> Self::Output;
        }
        pub trait RemLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn rem(a: Self, b: Rhs) -> Self::Output;
        }
        pub trait NegLogic: Logical {
            type Output: Logical;
            logic fn neg(a: Self) -> Self::Output;
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

        // `-` on `Nat`s gives an `Int`.
        impl SubLogic for Int {
            type Output = Int;
            logic fn sub(a: Int, b: Int) -> Int { builtin }
        }
        impl SubLogic for Nat {
            type Output = Int;
            logic fn sub(a: Nat, b: Nat) -> Int { builtin }
        }
        impl MulLogic for Int {
            type Output = Int;
            logic fn mul(a: Int, b: Int) -> Int { builtin }
        }
        impl MulLogic for Nat {
            type Output = Nat;
            logic fn mul(a: Nat, b: Nat) -> Nat { builtin }
        }
        impl DivLogic for Int {
            type Output = Int;
            logic fn div(a: Int, b: Int) -> Int { builtin }
        }
        impl DivLogic for Nat {
            type Output = Nat;
            logic fn div(a: Nat, b: Nat) -> Nat { builtin }
        }
        impl RemLogic for Int {
            type Output = Int;
            logic fn rem(a: Int, b: Int) -> Int { builtin }
        }
        impl RemLogic for Nat {
            type Output = Nat;
            logic fn rem(a: Nat, b: Nat) -> Nat { builtin }
        }
        impl NegLogic for Int {
            type Output = Int;
            logic fn neg(a: Int) -> Int { builtin }
        }

        impl<T> Seq<T> {
            pub logic fn empty() -> Seq<T> { builtin }
            pub logic fn len(self) -> Nat { builtin }
            pub logic fn skip(self, n: Nat) -> Seq<T> { builtin }
        }
    }
}
pub use traits::*;

/// "A `Nat` is used as an `Int` wherever an `Int` is expected" (RFC 0002),
/// as far as implementations can say it. Rust has no such conversion between
/// two types of a library, so each place where it may happen is one more
/// implementation, and each of those is one more candidate where rustc has
/// to infer a type.
///
/// - An operator on an `Int` and a `Nat`: here, unless the feature
///   `homogeneous-ops` is on.
/// - A `Nat` place given where an `Int` is wanted (`sk::ViaWiden`): only
///   with the feature `widen-args`.
#[cfg(not(feature = "homogeneous-ops"))]
mod widen_ops {
    use super::*;
    use locus_macros::locus;

    locus! {
        impl AddLogic<Nat> for Int {
            type Output = Int;
            logic fn add(a: Int, b: Nat) -> Int { builtin }
        }
        impl AddLogic<Int> for Nat {
            type Output = Int;
            logic fn add(a: Nat, b: Int) -> Int { builtin }
        }
        impl SubLogic<Nat> for Int {
            type Output = Int;
            logic fn sub(a: Int, b: Nat) -> Int { builtin }
        }
        impl SubLogic<Int> for Nat {
            type Output = Int;
            logic fn sub(a: Nat, b: Int) -> Int { builtin }
        }
        impl MulLogic<Nat> for Int {
            type Output = Int;
            logic fn mul(a: Int, b: Nat) -> Int { builtin }
        }
        impl MulLogic<Int> for Nat {
            type Output = Int;
            logic fn mul(a: Nat, b: Int) -> Int { builtin }
        }
    }
    impl sk::EqLogic<Nat> for Int {}
    impl sk::EqLogic<Int> for Nat {}
    impl sk::OrdLogic<Nat> for Int {}
    impl sk::OrdLogic<Int> for Nat {}
}
#[cfg(feature = "widen-args")]
impl sk::ArgAs<Int, sk::ViaWiden> for Nat {}
/// And a runtime place whose model is a `Nat`.
#[cfg(feature = "widen-args")]
impl<T: Runtime<Logic = Nat> + ?Sized> sk::ArgAs<Int, sk::ViaModelWiden> for T {}

/// What `locus!` imports into each module it expands in, by a glob. The
/// library's logical traits come with the hidden traits that hold their
/// logic functions, so that a probe finds those in every module.
pub mod prelude {
    pub use super::traits::*;
    pub use super::{Bool, Int, Logical, Nat, Prop, Seq, Snap, logic, prop, prove};
    pub use super::{False, ModelOf, Snapshot, True};
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
/// Lowered logical code with its typing skeleton: `{ skeleton } code`. The
/// skeleton does what the observations and the probes did, in both
/// expansions: its closure borrows each place the logical code reads, and
/// rustc resolves each name in it. `sk` is the stub that takes it.
#[macro_export]
macro_rules! skeleton {
    ($sk:ident $skeleton:block $($code:tt)*) => {
        $crate::sk::$sk(|| $skeleton)
    };
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
