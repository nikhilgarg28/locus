//! Locus's library for the skeleton-typing spike, crate `locus`. Adapted
//! (copied, not depended on) from the first mode of
//! `spikes/marker-lowering/locus/src/lib.rs`.
//!
//! Round 2 (skeleton-typing-2) renames what rustc's messages show: the one
//! marker for every proof is `Proof`, the stubs live in `locus::logic`, and
//! `Model` is the trait that RFC 0002 names. The conversion policy is chosen
//! by features: see `Cargo.toml`.
//!
//! What changed from the first mode's library:
//! - `logic` holds the stubs that a typing skeleton is written over: logical
//!   code, translated to a Rust expression inside a closure that is never
//!   called, so that rustc types it;
//! - `Model` is what a place read as a value is, a projection: a runtime
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
pub trait Logical: Copy + 'static + Model<Logic = Self> + Snapshot<S = Self> {
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
pub trait Model {
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
        impl<$($p),*> Model for $t {
            type Logic = Self;
        }
        impl<$($p: Logical),*> Snapshot for $t {
            type S = Self;
        }
    };
}

/// The marker for every proof: rustc never sees a proposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Proof;
own!([] Proof);
impl Logical for Proof {
    fn __locus_marker() -> Self {
        Proof
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

/// Not a type of the language. Each built-in operator has one more
/// implementation, for this type on its right, so that rustc does not take
/// the right operand's type from the only implementation there would
/// otherwise be: `i + n` for an `Int` and a `Nat` is then "the operator is
/// not defined on these" (E0277, with the library's text, which says to
/// write `as Int`), and not a bare mismatch on `n` (E0308).
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct NoSuchOperand;
own!([] NoSuchOperand);
impl Logical for NoSuchOperand {
    fn __locus_marker() -> Self {
        NoSuchOperand
    }
}

/// The snapshot of a type that nothing declares one for: RFC 0002's
/// "opaque". One type for all of them, with no parameter, so that it holds
/// no lifetime of the type it is a snapshot of.
#[derive(Clone, Copy, Debug)]
pub struct Opaque;
own!([] Opaque);
impl Logical for Opaque {
    fn __locus_marker() -> Self {
        Opaque
    }
}

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
pub trait Runtime: Model {}

/// The models that Locus's library declares for the standard library's
/// types (RFC 0002): an unsigned integer is a `Nat`, a signed one an `Int`,
/// `bool` is `Bool`, a reference and a `Box` are transparent, and a vector,
/// a slice and an array are a `Seq` of their elements' models.
macro_rules! models {
    ($($model:ident: $($t:ty),*;)*) => {$($(
        impl Model for $t {
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
impl<T: Model + ?Sized> Model for &T {
    type Logic = T::Logic;
}
impl<T: Runtime + ?Sized> Runtime for &T {}
impl<T: Model + ?Sized> Model for &mut T {
    type Logic = T::Logic;
}
impl<T: Runtime + ?Sized> Runtime for &mut T {}
impl<T: Model + ?Sized> Model for Box<T> {
    type Logic = T::Logic;
}
impl<T: Runtime + ?Sized> Runtime for Box<T> {}
impl<T: Model> Model for Vec<T> {
    type Logic = Seq<T::Logic>;
}
impl<T: Model> Runtime for Vec<T> {}
impl<T: Model> Model for [T] {
    type Logic = Seq<T::Logic>;
}
impl<T: Model> Runtime for [T] {}
impl<T: Model, const N: usize> Model for [T; N] {
    type Logic = Seq<T::Logic>;
}
impl<T: Model, const N: usize> Runtime for [T; N] {}
/// A tuple's model is the tuple of its components' models, so a tuple of
/// logical types is its own. A tuple is not `Runtime`: rustc cannot be told
/// "a tuple with at least one runtime component" by two implementations
/// that do not overlap.
impl<A: Model, B: Model> Model for (A, B) {
    type Logic = (A::Logic, B::Logic);
}
impl<A: Model, B: Model, C: Model> Model for (A, B, C) {
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
/// called. rustc types it; nothing here ever runs, but for the functions
/// that take a skeleton's closure, drop it, and give the marker.
///
/// Every name here is one that rustc may show in a message, so each is
/// named as the programmer thinks of the thing: `logic` holds the skeleton
/// of a `logic!` block, `prove` of a `prove!`, `add` is `+`.
pub mod logic {
    use super::*;

    // What takes a skeleton.

    /// `logic! { .. }`: a marker of the type of the logical code.
    pub fn logic<G: Logical>(_: impl FnOnce() -> G) -> G {
        G::__locus_marker()
    }
    /// `prove!(P)`: a proof. The skeleton is the proposition's.
    pub fn prove<P: Proposition>(_: impl FnOnce() -> P) -> Proof {
        Proof
    }
    /// `prop!(P)`: the proposition as a value.
    pub fn prop<P: Proposition>(_: impl FnOnce() -> P) -> Prop {
        Prop
    }
    /// A proposition `@(P)` in a type: of a `let`, a parameter, a result, a field.
    pub fn proposition<P: Proposition>(_: impl FnOnce() -> P) {}
    /// The same with the named components of the type's tuples in scope.
    pub fn proposition1<A, P: Proposition>(_: impl FnOnce(A) -> P) {}
    pub fn proposition2<A, B, P: Proposition>(_: impl FnOnce(A, B) -> P) {}
    pub fn proposition3<A, B, C, P: Proposition>(_: impl FnOnce(A, B, C) -> P) {}
    /// The body of a logic function: its type is the function's result.
    pub fn logic_fn<T>(_: impl FnOnce() -> T) -> T {
        stub()
    }
    /// The body of a logic function that returns `Prop`.
    pub fn logic_fn_prop<P: Proposition>(_: impl FnOnce() -> P) -> Prop {
        stub()
    }

    // Reading places.

    /// A place read as a value: its type's model, or itself if it is logical.
    /// Also what `model(x)` is.
    pub fn model<T: Model + ?Sized>(_: &T) -> T::Logic {
        stub()
    }
    /// A place as an argument, a field's initializer, or the result of a
    /// logic function: the type that is wanted there selects what the place
    /// is read as, the place itself or its model.
    pub fn argument<T: GivenAs<P, How> + ?Sized, P, How>(_: &T) -> P {
        stub()
    }
    /// `snap(place)`.
    pub fn snap<T: Snapshot + ?Sized>(_: &T) -> Snap<T> {
        stub()
    }
    /// `final!(r)`: the place that `r` ends with, as a place of `r`'s target type.
    pub fn final_value<'a, T: ?Sized>(_: &'a &mut T) -> &'a T {
        stub()
    }

    pub struct Itself;
    pub struct ItsModel;
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` cannot be given where a `{P}` is wanted in logical code",
        label = "this place is a `{Self}`",
        note = "a place is given as it is, or as its type's model",
        note = "a `Nat` is made an `Int` by writing `as Int` after it; where a parameter's type is a type parameter, write `model(x)` or `snap(x)`"
    )]
    pub trait GivenAs<P, How> {}
    impl<T> GivenAs<T, Itself> for T {}
    impl<T: Runtime + ?Sized> GivenAs<T::Logic, ItsModel> for T {}

    // Literals, holes and proofs.

    /// An integer literal: an `Int` or a `Nat`.
    #[diagnostic::on_unimplemented(
        message = "an integer literal is an `Int` or a `Nat`, not a `{Self}`",
        label = "this literal"
    )]
    pub trait IntegerLiteral {}
    impl IntegerLiteral for Int {}
    impl IntegerLiteral for Nat {}
    pub fn literal<L: IntegerLiteral>(_: u128) -> L {
        stub()
    }
    /// An integer literal beside another operand: it has that operand's type.
    pub fn literal_like<L: IntegerLiteral>(_: &L, _: u128) -> L {
        stub()
    }
    /// `true` and `false`.
    pub fn boolean(_: bool) -> Bool {
        stub()
    }
    /// A hole `_`: whatever is wanted there, a proof or a value.
    pub fn hole<T>() -> T {
        stub()
    }
    /// A logical closure whose body is a proof: a proof of a `forall`.
    pub fn closure1<A>(_: impl Fn(A) -> Proof) -> Proof {
        stub()
    }
    pub fn closure2<A, B>(_: impl Fn(A, B) -> Proof) -> Proof {
        stub()
    }
    pub fn closure3<A, B, C>(_: impl Fn(A, B, C) -> Proof) -> Proof {
        stub()
    }

    // Propositions. The policy for `Bool` and `Prop` (P3) is chosen by a
    // feature. Two things vary: what a comparison gives, and whether a
    // `Bool` is a proposition where the grammar wants one.
    //
    // | | a comparison gives | a `Bool` where a proposition is wanted |
    // |---|---|---|
    // | no feature: V3, the owner's choice | a `Prop` | accepted |
    // | `p3-v2` | a `Prop` | written: `b === true` |
    // | `p3-v0`, the first round's rule | a `Bool` | accepted |
    // | `p3-v1` | a `Bool` | written: `b === true` |

    /// What stands where a proposition is wanted.
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` is not a proposition",
        label = "a proposition is wanted here, and this is a `{Self}`",
        note = "a `Bool` is made a proposition by writing `=== true` after it: `b === true`"
    )]
    pub trait Proposition {}
    impl Proposition for Prop {}
    #[cfg(not(any(feature = "p3-v1", feature = "p3-v2")))]
    impl Proposition for Bool {}

    /// `&&` and `||`: a `Bool` on two `Bool`s, and otherwise a `Prop`.
    #[diagnostic::on_unimplemented(
        message = "`&&` and `||` take two `Bool`s or two propositions, not a `{Self}` and a `{Rhs}`",
        label = "this is a `{Self}`, and the other side is a `{Rhs}`",
        note = "a `Bool` is made a proposition by writing `=== true` after it: `b === true`"
    )]
    pub trait Connective<Rhs> {
        type Output;
    }
    impl Connective<Bool> for Bool {
        type Output = Bool;
    }
    impl Connective<Prop> for Prop {
        type Output = Prop;
    }
    // Where a `Bool` and a `Prop` do not mix: one more implementation for
    // each, so that the message is the library's and not a bare mismatch
    // (see `NoSuchOperand`).
    #[cfg(any(feature = "p3-v1", feature = "p3-v2"))]
    impl Connective<NoSuchOperand> for Bool {
        type Output = Bool;
    }
    #[cfg(any(feature = "p3-v1", feature = "p3-v2"))]
    impl Connective<NoSuchOperand> for Prop {
        type Output = Prop;
    }
    #[cfg(not(any(feature = "p3-v1", feature = "p3-v2")))]
    impl Connective<Prop> for Bool {
        type Output = Prop;
    }
    #[cfg(not(any(feature = "p3-v1", feature = "p3-v2")))]
    impl Connective<Bool> for Prop {
        type Output = Prop;
    }
    pub fn and<A: Connective<B>, B>(_: A, _: B) -> A::Output {
        stub()
    }
    pub fn or<A: Connective<B>, B>(_: A, _: B) -> A::Output {
        stub()
    }
    /// `!`: on a `Bool` and on a proposition.
    #[diagnostic::on_unimplemented(
        message = "`!` is not defined on a `{Self}` in logical code",
        label = "neither a `Bool` nor a proposition"
    )]
    pub trait Negation {}
    impl Negation for Bool {}
    impl Negation for Prop {}
    pub fn not<A: Negation>(_: A) -> A {
        stub()
    }
    pub fn implies<A: Proposition, B: Proposition>(_: A, _: B) -> Prop {
        stub()
    }
    pub fn iff<A: Proposition, B: Proposition>(_: A, _: B) -> Prop {
        stub()
    }
    /// `===` and `!==`: on two values of one type, whatever it is.
    pub fn same<T>(_: T, _: T) -> Prop {
        stub()
    }
    pub fn differ<T>(_: T, _: T) -> Prop {
        stub()
    }
    /// The condition of an `if` in logical code: a `Bool`, or any
    /// proposition (classical logic decides it). Rust wants a `bool` there.
    #[diagnostic::on_unimplemented(
        message = "the condition of an `if` in logical code is a `Bool` or a proposition, not a `{Self}`",
        label = "this is a `{Self}`"
    )]
    pub trait Condition {}
    impl Condition for Bool {}
    impl Condition for Prop {}
    pub fn condition<C: Condition>(_: C) -> bool {
        stub()
    }
    pub fn forall1<A, P: Proposition>(_: impl Fn(A) -> P) -> Prop {
        stub()
    }
    pub fn forall2<A, B, P: Proposition>(_: impl Fn(A, B) -> P) -> Prop {
        stub()
    }
    pub fn forall3<A, B, C, P: Proposition>(_: impl Fn(A, B, C) -> P) -> Prop {
        stub()
    }
    pub fn exists1<A, P: Proposition>(_: impl Fn(A) -> P) -> Prop {
        stub()
    }
    pub fn exists2<A, B, P: Proposition>(_: impl Fn(A, B) -> P) -> Prop {
        stub()
    }
    pub fn exists3<A, B, C, P: Proposition>(_: impl Fn(A, B, C) -> P) -> Prop {
        stub()
    }

    // Comparisons. Rust's `==` and `<` give a `bool`, whatever the
    // implementation, so they are stubs. They give a `Prop` (V3, V2), as
    // Lean's do, or a `Bool` (V0, V1).

    #[cfg(any(feature = "p3-v0", feature = "p3-v1"))]
    pub type Comparison = Bool;
    #[cfg(not(any(feature = "p3-v0", feature = "p3-v1")))]
    pub type Comparison = Prop;

    #[diagnostic::on_unimplemented(
        message = "`==` is not defined on a `{Self}` and a `{Rhs}` in logical code",
        label = "this is a `{Self}`, and the other side is a `{Rhs}`",
        note = "a `Nat` is compared with an `Int` after it is made one: write `as Int` after the `Nat`",
        note = "`===` compares two values of any one logical type"
    )]
    pub trait Equality<Rhs = Self> {}
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` and a `{Rhs}` have no order in logical code",
        label = "this is a `{Self}`, and the other side is a `{Rhs}`",
        note = "a `Nat` is compared with an `Int` after it is made one: write `as Int` after the `Nat`"
    )]
    pub trait Ordered<Rhs = Self> {}
    impl Equality for Int {}
    impl Equality for Nat {}
    impl Ordered for Int {}
    impl Ordered for Nat {}
    #[cfg(not(feature = "mixed-ops"))]
    impl Equality<NoSuchOperand> for Int {}
    #[cfg(not(feature = "mixed-ops"))]
    impl Equality<NoSuchOperand> for Nat {}
    #[cfg(not(feature = "mixed-ops"))]
    impl Ordered<NoSuchOperand> for Int {}
    #[cfg(not(feature = "mixed-ops"))]
    impl Ordered<NoSuchOperand> for Nat {}
    #[cfg(feature = "mixed-ops")]
    impl Equality<Nat> for Int {}
    #[cfg(feature = "mixed-ops")]
    impl Equality<Int> for Nat {}
    #[cfg(feature = "mixed-ops")]
    impl Ordered<Nat> for Int {}
    #[cfg(feature = "mixed-ops")]
    impl Ordered<Int> for Nat {}
    impl Equality for Bool {}
    pub fn eq<A: Equality<B>, B>(_: A, _: B) -> Comparison {
        stub()
    }
    pub fn ne<A: Equality<B>, B>(_: A, _: B) -> Comparison {
        stub()
    }
    pub fn lt<A: Ordered<B>, B>(_: A, _: B) -> Comparison {
        stub()
    }
    pub fn le<A: Ordered<B>, B>(_: A, _: B) -> Comparison {
        stub()
    }
    pub fn gt<A: Ordered<B>, B>(_: A, _: B) -> Comparison {
        stub()
    }
    pub fn ge<A: Ordered<B>, B>(_: A, _: B) -> Comparison {
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

    // Conversions that the program writes: `e as T`.

    #[diagnostic::on_unimplemented(
        message = "`as` does not make a `{Self}` a `{T}` in logical code",
        label = "this is a `{Self}`",
        note = "`as` makes a `Nat` an `Int`, a `Bool` a `Prop`, and a `Prop` a `Bool`; an `Int` is made a `Nat` only with a proof that it is not negative"
    )]
    pub trait As<T> {}
    impl As<Int> for Nat {}
    impl As<Int> for Int {}
    impl As<Nat> for Nat {}
    impl As<Prop> for Bool {}
    impl As<Prop> for Prop {}
    impl As<Bool> for Prop {}
    impl As<Bool> for Bool {}
    pub fn cast<A: As<T>, T>(_: A) -> T {
        stub()
    }

    // Indexing and matching.

    #[diagnostic::on_unimplemented(
        message = "a `{Self}` cannot be indexed in logical code",
        label = "this is a `{Self}`"
    )]
    pub trait Indexable {
        type Item;
    }
    impl<T> Indexable for Seq<T> {
        type Item = T;
    }
    /// `xs[i]`. The index is a `Nat` here: RFC 0002 leaves it open.
    pub fn index<S: Indexable>(_: S, _: Nat) -> S::Item {
        stub()
    }

    /// What a `match` in logical code is on. A logical enum is a zero-sized
    /// struct with no payload, so that it can contain itself, and its
    /// variants are those of an enum that only this trait names. The
    /// patterns of a `match` reach them through the type alone, as
    /// `Variants::<Tree>::Leaf`, so nothing but the type has to be in scope.
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` cannot be matched in logical code",
        label = "no variants or fields that rustc knows"
    )]
    pub trait Matchable {
        type Variants;
    }
    pub type Variants<T> = <T as Matchable>::Variants;
    /// The cases of a declared proposition: a proof is matched through them.
    pub trait ProofCases {}
    #[diagnostic::on_unimplemented(
        message = "a `{Self}` cannot be matched in logical code",
        label = "no variants or fields that rustc knows"
    )]
    pub trait MatchedAs<V> {}
    impl<T: Matchable> MatchedAs<T::Variants> for T {}
    impl<V: ProofCases> MatchedAs<V> for Proof {}
    pub fn matched<T: MatchedAs<V>, V>(_: T) -> V {
        stub()
    }

    // A method call on a place (P5): a logic function declared on the
    // place's own type is found first, then one on its model.

    /// A name, as a type: `len` is `N<'l', N<'e', N<'n', ()>>>`.
    pub struct N<const C: char, Rest>(PhantomData<Rest>);
    /// `Self` declares a logic function of this name that takes `self`.
    /// `locus!` writes an implementation beside each inherent `impl`.
    pub trait Declares<Name> {
        type Owner;
    }
    impl<Name, T: Declares<Name> + ?Sized> Declares<Name> for &T {
        type Owner = T::Owner;
    }
    impl<Name, T: Declares<Name> + ?Sized> Declares<Name> for &mut T {
        type Owner = T::Owner;
    }
    impl<Name, T: Declares<Name> + ?Sized> Declares<Name> for Box<T> {
        type Owner = T::Owner;
    }
    pub struct Receiver<Name, T: ?Sized>(PhantomData<(fn() -> Name, fn() -> T)>);
    /// The receiver of a method call in logical code, before it is known
    /// where the method is declared.
    pub fn receiver<Name, T: ?Sized>(_: &T) -> Receiver<Name, T> {
        stub()
    }
    /// Chosen first, by Rust's method lookup: by value before by reference.
    pub trait OnTheType {
        type Out;
        fn __locus_receiver(&self) -> Self::Out;
    }
    impl<Name, T: Declares<Name> + ?Sized> OnTheType for Receiver<Name, T> {
        type Out = T::Owner;
        fn __locus_receiver(&self) -> T::Owner {
            stub()
        }
    }
    // `snap(place)`: the snapshot that is declared for the place's type, or
    // an opaque one where there is none, chosen in the same way.

    pub struct SnapshotOf<T: ?Sized>(PhantomData<fn() -> PhantomData<T>>);
    pub fn snapshot_of<T: ?Sized>(_: &T) -> SnapshotOf<T> {
        stub()
    }
    pub trait SnapDeclared {
        type Out;
        fn __locus_snap(&self) -> Self::Out;
    }
    impl<T: Snapshot + ?Sized> SnapDeclared for SnapshotOf<T> {
        type Out = T::S;
        fn __locus_snap(&self) -> T::S {
            stub()
        }
    }
    pub trait SnapOpaque {
        fn __locus_snap(&self) -> Opaque;
    }
    impl<T: ?Sized> SnapOpaque for &SnapshotOf<T> {
        fn __locus_snap(&self) -> Opaque {
            stub()
        }
    }

    pub trait OnTheModel<T: ?Sized> {
        fn __locus_receiver(&self) -> T::Logic
        where
            T: Model;
    }
    impl<Name, T: ?Sized> OnTheModel<T> for &Receiver<Name, T> {
        fn __locus_receiver(&self) -> T::Logic
        where
            T: Model,
        {
            stub()
        }
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
        #[diagnostic::on_unimplemented(
            message = "`+` is not defined on a `{Self}` and a `{Rhs}` in logical code",
            label = "this is a `{Self}`, and the other side is a `{Rhs}`",
            note = "a `Nat` is added to an `Int` after it is made one: write `as Int` after the `Nat`"
        )]
        pub trait AddLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn add(a: Self, b: Rhs) -> Self::Output;
        }
        #[diagnostic::on_unimplemented(
            message = "`-` is not defined on a `{Self}` and a `{Rhs}` in logical code",
            label = "this is a `{Self}`, and the other side is a `{Rhs}`",
            note = "a `Nat` and an `Int` are subtracted after the `Nat` is made an `Int`: write `as Int` after it"
        )]
        pub trait SubLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn sub(a: Self, b: Rhs) -> Self::Output;
        }
        #[diagnostic::on_unimplemented(
            message = "`*` is not defined on a `{Self}` and a `{Rhs}` in logical code",
            label = "this is a `{Self}`, and the other side is a `{Rhs}`",
            note = "a `Nat` and an `Int` are multiplied after the `Nat` is made an `Int`: write `as Int` after it"
        )]
        pub trait MulLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn mul(a: Self, b: Rhs) -> Self::Output;
        }
        #[diagnostic::on_unimplemented(
            message = "`/` is not defined on a `{Self}` and a `{Rhs}` in logical code",
            label = "this is a `{Self}`, and the other side is a `{Rhs}`",
            note = "a `Nat` and an `Int` are divided after the `Nat` is made an `Int`: write `as Int` after it"
        )]
        pub trait DivLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn div(a: Self, b: Rhs) -> Self::Output;
        }
        #[diagnostic::on_unimplemented(
            message = "`%` is not defined on a `{Self}` and a `{Rhs}` in logical code",
            label = "this is a `{Self}`, and the other side is a `{Rhs}`",
            note = "a `Nat` and an `Int` are divided after the `Nat` is made an `Int`: write `as Int` after it"
        )]
        pub trait RemLogic<Rhs: Logical = Self>: Logical {
            type Output: Logical;
            logic fn rem(a: Self, b: Rhs) -> Self::Output;
        }
        #[diagnostic::on_unimplemented(
            message = "unary `-` is not defined on a `{Self}` in logical code",
            label = "this is a `{Self}`",
            note = "the negation of a `Nat` is an `Int`: write `as Int` after the `Nat`"
        )]
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
        // `-` on `Nat`s gives an `Int`: a result type, not a conversion.
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
            pub logic fn take(self, n: Nat) -> Seq<T> { builtin }
            pub logic fn push(self, x: T) -> Seq<T> { builtin }
            pub logic fn insert(self, i: Nat, x: T) -> Seq<T> { builtin }
            pub logic fn last(self) -> T { builtin }
        }

        // The proposition that a `Bool` is true: how P3's V1 writes a
        // `Bool` where a proposition is wanted.
        pub logic fn holds(b: Bool) -> Prop { builtin }
        // From a proposition to a `Bool` (Lean's `decide`, classically):
        // one candidate for relating a runtime `bool` to a proposition.
        pub logic fn decide(p: Prop) -> Bool { builtin }
    }
}
pub use traits::*;

/// See `NoSuchOperand`. Written out, not by a `macro_rules!`, so that
/// rustc's messages do not show a macro of the library.
#[cfg(not(feature = "mixed-ops"))]
mod one_more {
    use super::*;
    use locus_macros::locus;
    locus! {
        impl AddLogic<NoSuchOperand> for Int {
            type Output = Int;
            logic fn add(a: Int, b: NoSuchOperand) -> Int { builtin }
        }
        impl AddLogic<NoSuchOperand> for Nat {
            type Output = Nat;
            logic fn add(a: Nat, b: NoSuchOperand) -> Nat { builtin }
        }
        impl SubLogic<NoSuchOperand> for Int {
            type Output = Int;
            logic fn sub(a: Int, b: NoSuchOperand) -> Int { builtin }
        }
        impl SubLogic<NoSuchOperand> for Nat {
            type Output = Nat;
            logic fn sub(a: Nat, b: NoSuchOperand) -> Nat { builtin }
        }
        impl MulLogic<NoSuchOperand> for Int {
            type Output = Int;
            logic fn mul(a: Int, b: NoSuchOperand) -> Int { builtin }
        }
        impl MulLogic<NoSuchOperand> for Nat {
            type Output = Nat;
            logic fn mul(a: Nat, b: NoSuchOperand) -> Nat { builtin }
        }
        impl DivLogic<NoSuchOperand> for Int {
            type Output = Int;
            logic fn div(a: Int, b: NoSuchOperand) -> Int { builtin }
        }
        impl DivLogic<NoSuchOperand> for Nat {
            type Output = Nat;
            logic fn div(a: Nat, b: NoSuchOperand) -> Nat { builtin }
        }
        impl RemLogic<NoSuchOperand> for Int {
            type Output = Int;
            logic fn rem(a: Int, b: NoSuchOperand) -> Int { builtin }
        }
        impl RemLogic<NoSuchOperand> for Nat {
            type Output = Nat;
            logic fn rem(a: Nat, b: NoSuchOperand) -> Nat { builtin }
        }
    }
}

/// P2, "a `Nat` is used as an `Int` wherever an `Int` is expected" (RFC
/// 0002). By default it is always written, as `n as Int`. With the feature
/// `mixed-ops` the arithmetic and comparison operators take an `Int` and a
/// `Nat`, which is the first round's default.
#[cfg(feature = "mixed-ops")]
mod mixed_ops {
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
}

/// What `locus!` imports into each module it expands in, by a glob. The
/// library's logical traits come with the hidden traits that hold their
/// logic functions, so that a probe finds those in every module.
pub mod prelude {
    pub use super::traits::*;
    pub use super::{Bool, Int, Logical, Nat, Opaque, Prop, Seq, Snap, logic, prop, prove};
    pub use super::{False, Model, Proof, Snapshot, True};
}

/// `@(P)` lowers to `::locus::Proof![P]`: the same marker in both expansions.
#[macro_export]
macro_rules! Proof {
    ($($p:tt)*) => {
        $crate::Proof
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
        $crate::Proof
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
