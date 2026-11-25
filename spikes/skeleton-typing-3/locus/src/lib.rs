//! Locus's library for the skeleton-typing spike, crate `locus`. Adapted
//! (copied, not depended on) from the first mode of
//! `spikes/marker-lowering/locus/src/lib.rs`.
//!
//! Round 3 (skeleton-typing-3): ONE family. `Model` has two associated
//! types, the snapshot and the model, and each type has exactly one
//! implementation: Locus's library writes it for the types of the language
//! and of the standard library, and `locus!` writes it where a program
//! declares a type. There is no blanket implementation, no fallback, and no
//! bound that lowering inserts: a type parameter is read in logical code
//! only where the program writes `T: Model`.
//!
//! - A machine integer's snapshot is a `Nat` or an `Int`: the bounded
//!   integer types of round 2 are gone.
//! - `capture(&place)` is the model, `snap(&place)` the snapshot, and
//!   `model::<X>(s)` (or `X::model(s)`) applies `X`'s model to a snapshot.
//! - `Model` is an `unsafe trait`: a hand-written implementation has to say
//!   `unsafe impl`, and lowering's own carries the expansion's mark, which
//!   the driver reads.
//!
//! As before, one artifact serves both expansions: the `macro_rules!` here
//! put `#[cfg(__locus_check)]` in their *output*, so the `cfg` is evaluated
//! in the crate where they expand.

// `locus!` writes `::locus::..` paths. The library's own logical traits are
// written in Locus, through `locus!`, so the crate names itself.
extern crate self as locus;

use core::fmt;
use core::marker::PhantomData;

/// Holds for exactly the logical types: each is its own snapshot and its
/// own model.
///
/// The draft gives it no members. This one has a hidden constructor, so that
/// the expansion of `logic! { .. }` can give a marker whose type rustc
/// infers. `'static` makes rustc refuse a marker that carries a lifetime.
pub trait Logical: Copy + 'static + Model<Snap = Self, Logic = Self> {
    #[doc(hidden)]
    fn __locus_marker() -> Self;
}

/// The one family. `Snap` is the type's snapshot: what `snap(x)` gives, with
/// no lifetime of the type. `Logic` is its model: what a place of the type
/// is where logical code uses it as a value. By default the model is the
/// snapshot.
///
/// Each type has one implementation, written where the type is declared.
/// It is `unsafe` to implement: only lowering and Locus's library do.
///
/// The two functions are the stubs of two logic functions: `X::model(s)`,
/// the model of a snapshot, and `X::valid(s)`, what is known of a snapshot
/// because it was captured from a place of type `X` (a `u8` is at most 255).
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be read in logical code",
    label = "this type has no snapshot that Locus knows",
    note = "for a type parameter, add the bound `{Self}: Model`",
    note = "for a type of a crate that Locus does not check, mark the field `#[opaque]` or wrap the type in a struct of your own"
)]
pub unsafe trait Model {
    type Snap: Logical;
    type Logic: Logical;
    #[doc(hidden)]
    fn __logic_model(s: Self::Snap) -> Self::Logic;
    #[doc(hidden)]
    fn __logic_valid(s: Self::Snap) -> Prop;
}
/// `Snap<T>`, as a program writes it.
pub type Snap<T> = <T as Model>::Snap;

/// The conditional form of a generated implementation (capture-family
/// `c11`), behind `LOCUS_SPIKE_DEFERRED=1`: a `where` clause behind a
/// higher-ranked lifetime, which rustc 1.97 does not check where the
/// implementation is declared. Not used by default.
pub trait Deferred<'a> {
    type Snap: Logical;
    #[doc(hidden)]
    fn __locus_valid(s: Self::Snap) -> Prop;
}
impl<'a, T: Model + ?Sized> Deferred<'a> for T {
    type Snap = T::Snap;
    fn __locus_valid(s: T::Snap) -> Prop {
        T::__logic_valid(s)
    }
}

/// A logical type: it is its own model and its own snapshot, and nothing
/// more is known of a value of it than its type says.
macro_rules! own {
    ([$($p:ident),*] $t:ty) => {
        unsafe impl<$($p: Logical),*> Model for $t {
            type Snap = Self;
            type Logic = Self;
            fn __logic_model(s: Self) -> Self {
                s
            }
            fn __logic_valid(_: Self) -> Prop {
                Prop
            }
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

/// The snapshot of a field that is marked `#[opaque]`: an unknown value
/// that moves and copies keep. One type for each type it stands for, so the
/// snapshots of two foreign types are two types.
pub struct Opaque<T: ?Sized> {
    __locus: PhantomData<fn() -> PhantomData<T>>,
}
impl<T: ?Sized> Clone for Opaque<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for Opaque<T> {}
impl<T: ?Sized> fmt::Debug for Opaque<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Opaque")
    }
}
unsafe impl<T: ?Sized + 'static> Model for Opaque<T> {
    type Snap = Self;
    type Logic = Self;
    fn __logic_model(s: Self) -> Self {
        s
    }
    fn __logic_valid(_: Self) -> Prop {
        Prop
    }
}
impl<T: ?Sized + 'static> Logical for Opaque<T> {
    fn __locus_marker() -> Self {
        Opaque {
            __locus: PhantomData,
        }
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

// ------------------------------------------------- the family, by the library

/// What the library's hand-written `valid` functions are written over: the
/// same stubs as a typing skeleton, by hand, so that rustc types them.
use logic as l;

/// A machine integer: a `Nat` or an `Int` for both its snapshot and its
/// model, which is then the identity. Its range is a fact about a captured
/// value (`valid`), and not about the type.
macro_rules! unsigned {
    ($($t:ty),*) => {$(
        unsafe impl Model for $t {
            type Snap = Nat;
            type Logic = Nat;
            fn __logic_model(s: Nat) -> Nat {
                s
            }
            fn __logic_valid(s: Nat) -> Prop {
                l::logic_fn_prop(|| l::le(s, l::literal::<Nat>(<$t>::MAX as u128)))
            }
        }
    )*};
}
macro_rules! signed {
    ($($t:ty),*) => {$(
        unsafe impl Model for $t {
            type Snap = Int;
            type Logic = Int;
            fn __logic_model(s: Int) -> Int {
                s
            }
            fn __logic_valid(s: Int) -> Prop {
                l::logic_fn_prop(|| {
                    l::and(
                        l::ge(s, l::neg(l::literal::<Int>(<$t>::MIN.unsigned_abs() as u128))),
                        l::le(s, l::literal::<Int>(<$t>::MAX as u128)),
                    )
                })
            }
        }
    )*};
}
unsigned!(u8, u16, u32, u64, u128, usize);
signed!(i8, i16, i32, i64, i128, isize);
unsafe impl Model for bool {
    type Snap = Bool;
    type Logic = Bool;
    fn __logic_model(s: Bool) -> Bool {
        s
    }
    fn __logic_valid(_: Bool) -> Prop {
        True
    }
}

/// A reference and a `Box` are transparent: the target's snapshot, the
/// target's model, and what is known of the target.
macro_rules! transparent {
    ($($t:ty),*) => {$(
        unsafe impl<T: Model + ?Sized> Model for $t {
            type Snap = T::Snap;
            type Logic = T::Logic;
            fn __logic_model(s: T::Snap) -> T::Logic {
                T::__logic_model(s)
            }
            fn __logic_valid(s: T::Snap) -> Prop {
                l::logic_fn_prop(|| T::__logic_valid(s))
            }
        }
    )*};
}
transparent!(&T, &mut T, Box<T>);

/// A slice and an array are a `Seq`, part by part. An array's length is
/// known of a captured one.
unsafe impl<T: Model> Model for [T] {
    type Snap = Seq<T::Snap>;
    type Logic = Seq<T::Logic>;
    fn __logic_model(_: Seq<T::Snap>) -> Seq<T::Logic> {
        stub()
    }
    fn __logic_valid(s: Seq<T::Snap>) -> Prop {
        l::logic_fn_prop(|| {
            l::forall1(|i: Nat| l::implies(l::lt(i, s.__logic_len()), T::__logic_valid(l::index(s, i))))
        })
    }
}
unsafe impl<T: Model, const N: usize> Model for [T; N] {
    type Snap = Seq<T::Snap>;
    type Logic = Seq<T::Logic>;
    fn __logic_model(_: Seq<T::Snap>) -> Seq<T::Logic> {
        stub()
    }
    fn __logic_valid(s: Seq<T::Snap>) -> Prop {
        l::logic_fn_prop(|| {
            l::and(
                l::eq(s.__logic_len(), l::literal::<Nat>(N as u128)),
                l::forall1(|i: Nat| {
                    l::implies(l::lt(i, s.__logic_len()), T::__logic_valid(l::index(s, i)))
                }),
            )
        })
    }
}
/// A tuple, part by part: so a tuple of logical types is its own.
unsafe impl<A: Model, B: Model> Model for (A, B) {
    type Snap = (A::Snap, B::Snap);
    type Logic = (A::Logic, B::Logic);
    fn __logic_model(s: (A::Snap, B::Snap)) -> (A::Logic, B::Logic) {
        (A::__logic_model(s.0), B::__logic_model(s.1))
    }
    fn __logic_valid(s: (A::Snap, B::Snap)) -> Prop {
        l::logic_fn_prop(|| l::and(A::__logic_valid(s.0), B::__logic_valid(s.1)))
    }
}
unsafe impl<A: Model, B: Model, C: Model> Model for (A, B, C) {
    type Snap = (A::Snap, B::Snap, C::Snap);
    type Logic = (A::Logic, B::Logic, C::Logic);
    fn __logic_model(s: (A::Snap, B::Snap, C::Snap)) -> (A::Logic, B::Logic, C::Logic) {
        (A::__logic_model(s.0), B::__logic_model(s.1), C::__logic_model(s.2))
    }
    fn __logic_valid(s: (A::Snap, B::Snap, C::Snap)) -> Prop {
        l::logic_fn_prop(|| {
            l::and(l::and(A::__logic_valid(s.0), B::__logic_valid(s.1)), C::__logic_valid(s.2))
        })
    }
}
/// `Option`, part by part: the library's logical `Maybe`.
unsafe impl<T: Model> Model for Option<T> {
    type Snap = Maybe<T::Snap>;
    type Logic = Maybe<T::Logic>;
    fn __logic_model(_: Maybe<T::Snap>) -> Maybe<T::Logic> {
        stub()
    }
    fn __logic_valid(s: Maybe<T::Snap>) -> Prop {
        l::logic_fn_prop(|| match l::matched(s) {
            l::Variants::<Maybe<T::Snap>>::Just(x) => T::__logic_valid(x),
            _ => True,
        })
    }
}
/// The standard library's own vector, as round 2 had it: a `Seq`. Kept
/// unless the feature `no-std-vec-model` is on, so that the sets of the earlier
/// rounds, which read a `std::vec::Vec` in logical code, can be compared.
/// The design gives it no model: see `Vec` below.
#[cfg(not(feature = "no-std-vec-model"))]
unsafe impl<T: Model> Model for ::std::vec::Vec<T> {
    type Snap = Seq<T::Snap>;
    type Logic = Seq<T::Logic>;
    fn __logic_model(_: Seq<T::Snap>) -> Seq<T::Logic> {
        stub()
    }
    fn __logic_valid(s: Seq<T::Snap>) -> Prop {
        l::logic_fn_prop(|| {
            l::forall1(|i: Nat| l::implies(l::lt(i, s.__logic_len()), T::__logic_valid(l::index(s, i))))
        })
    }
}

/// A snapshot that nothing looks into, for a type of the standard library:
/// one type for each such type, generic over its arguments' snapshots. Its
/// model is the snapshot.
macro_rules! unknown {
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
        unsafe impl<$($p: Model),*> Model for $t {
            type Snap = $name<$($p::Snap),*>;
            type Logic = $name<$($p::Snap),*>;
            fn __logic_model(s: Self::Snap) -> Self::Snap {
                s
            }
            fn __logic_valid(_: Self::Snap) -> Prop {
                True
            }
        }
    )*};
}
unknown! {
    CellSnap [T]: core::cell::Cell<T>;
    RefCellSnap [T]: core::cell::RefCell<T>;
    StringSnap []: String;
    StrSnap []: str;
}
// A raw pointer's snapshot is its address. `PhantomData` holds nothing.
marker!(Address, Nothing);
macro_rules! plain {
    ($($snap:ident: $t:ty;)*) => {$(
        unsafe impl<T: ?Sized> Model for $t {
            type Snap = $snap;
            type Logic = $snap;
            fn __logic_model(s: $snap) -> $snap {
                s
            }
            fn __logic_valid(_: $snap) -> Prop {
                True
            }
        }
    )*};
}
plain! {
    Address: *const T;
    Address: *mut T;
    Nothing: PhantomData<T>;
}

// ------------------------------------------------------- the library's `Vec`

/// Locus's own vector (recommendation 8): the standard library's, and a
/// ghost field for the elements' snapshots, in order.
///
/// - **No bound on `T` on the struct.** The ghost field's type is nominal in
///   `T`: `Contents<T>` is a zero-sized marker that names `T` and asks
///   nothing of it. What it holds, `Seq<Snap<T>>`, is a type only where
///   `T: Model`, so that type is on the observer and not on the field.
/// - **Its snapshot is `VecSnap<Snap<T>>`,** which names `T`'s snapshot and
///   not `T`: a snapshot of a `Vec<&mut X>` keeps no borrow.
///   `snap(v).elements()` is the `Seq<Snap<T>>`, and the model is
///   `Seq<T::Logic>`.
/// - **Only the wrapper's own methods change it.** It dereferences to the
///   standard vector by shared reference only; `into_std` and `from_std`
///   are the way out and back.
pub struct Vec<T> {
    inner: ::std::vec::Vec<T>,
    contents: Contents<T>,
}

/// The type of the ghost field: nominal in `T`, with no bound.
pub struct Contents<T> {
    __locus: PhantomData<fn() -> PhantomData<T>>,
}
impl<T> Contents<T> {
    /// The library's own: a ghost field is written with the sequence it
    /// now holds. The sequence's type needs `T: Model`; the field's does not.
    #[allow(dead_code)]
    fn holding(_: Seq<T::Snap>) -> Self
    where
        T: Model,
    {
        Contents { __locus: PhantomData }
    }
    const NEW: Self = Contents { __locus: PhantomData };
}
/// Read in logical code, the ghost field is the sequence: this is the
/// observer's bound.
unsafe impl<T: Model> Model for Contents<T> {
    type Snap = Seq<T::Snap>;
    type Logic = Seq<T::Snap>;
    fn __logic_model(s: Seq<T::Snap>) -> Seq<T::Snap> {
        s
    }
    fn __logic_valid(s: Seq<T::Snap>) -> Prop {
        l::logic_fn_prop(|| {
            l::forall1(|i: Nat| l::implies(l::lt(i, s.__logic_len()), T::__logic_valid(l::index(s, i))))
        })
    }
}

/// The snapshot of a `Vec<T>`, over `T`'s snapshot: the standard vector's
/// own part, of which nothing is known, and the contents.
pub struct VecSnap<S> {
    __locus: PhantomData<fn() -> S>,
}
impl<S> Clone for VecSnap<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for VecSnap<S> {}
impl<S> fmt::Debug for VecSnap<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Snap<Vec>")
    }
}
own!([S] VecSnap<S>);
impl<S: Logical> Logical for VecSnap<S> {
    fn __locus_marker() -> Self {
        VecSnap { __locus: PhantomData }
    }
}
unsafe impl<T: Model> Model for Vec<T> {
    type Snap = VecSnap<T::Snap>;
    type Logic = Seq<T::Logic>;
    /// `elements(s).map(model)`: the toy has no `map`, so it is a stub.
    fn __logic_model(_: VecSnap<T::Snap>) -> Seq<T::Logic> {
        stub()
    }
    fn __logic_valid(s: VecSnap<T::Snap>) -> Prop {
        l::logic_fn_prop(|| <Contents<T> as Model>::__logic_valid(s.__logic_elements()))
    }
}

impl<T> Vec<T> {
    /// Needs nothing of `T`.
    pub fn new() -> Self {
        Vec { inner: ::std::vec::Vec::new(), contents: Contents::NEW }
    }
    /// The way out: the standard vector, and Locus knows no more of it.
    pub fn into_std(self) -> ::std::vec::Vec<T> {
        self.inner
    }
    /// The way back.
    pub fn from_std(inner: ::std::vec::Vec<T>) -> Self {
        Vec { inner, contents: Contents::NEW }
    }
}
impl<T> Default for Vec<T> {
    fn default() -> Self {
        Self::new()
    }
}
/// Shared only: a standard method that takes `&self` can be called, and
/// one that takes `&mut self` cannot.
impl<T> core::ops::Deref for Vec<T> {
    type Target = ::std::vec::Vec<T>;
    fn deref(&self) -> &::std::vec::Vec<T> {
        &self.inner
    }
}
impl<'a, T> IntoIterator for &'a Vec<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}
/// `v[i] = x`. A trait's method has the trait's signature, so it can
/// return no proof and take none: this one has no contract.
impl<T> core::ops::Index<usize> for Vec<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        &self.inner[i]
    }
}
impl<T> core::ops::IndexMut<usize> for Vec<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        &mut self.inner[i]
    }
}

mod vec_contracts {
    use super::{Vec, VecSnap};
    use locus_macros::locus;

    locus! {
        impl<S> VecSnap<S> {
            /// The observer: the elements' snapshots, in order.
            pub logic fn elements(self) -> Seq<S> { builtin }
        }

        /// The methods with contracts. Each speaks of the elements'
        /// snapshots, and so asks for `T: Model`.
        impl<T: Model> Vec<T> {
            pub fn push(&mut self, x: T)
                -> @(snap(final!(self)).elements() === snap(*self).elements().push(snap(x)))
            {
                let now = logic! { snap(self.contents).push(snap(x)) };
                self.inner.push(x);
                self.contents = super::Contents::holding(now);
                _
            }

            pub fn pop(&mut self)
                -> (r: Option<T>, @(snap(*self).elements().len() > 0
                    ==> snap(final!(self)).elements().push(snap(r).unwrap()) === snap(*self).elements()))
            {
                (self.inner.pop(), _)
            }

            pub fn len(&self) -> (n: usize, @(n == snap(*self).elements().len())) {
                (self.inner.len(), _)
            }

            pub fn get(&self, i: usize, in_bounds: @(i < snap(*self).elements().len()))
                -> (r: &T, @(snap(*r) === snap(*self).elements()[i]))
            {
                (&self.inner[i], _)
            }

            /// A mutable loan of one element: the contents afterwards are
            /// the contents before with the element's final value at `i`.
            pub fn get_mut(&mut self, i: usize, ok: @(i < self.len()))
                -> (r: &mut T, @(snap(final!(self)).elements() === snap(*self).elements().set(i, snap(final!(r)))))
            {
                (&mut self.inner[i], _)
            }

            pub fn insert(&mut self, i: usize, x: T)
                -> @(snap(final!(self)).elements() === snap(*self).elements().insert(i, snap(x)))
            {
                self.inner.insert(i, x);
                _
            }
        }

        impl<T> Maybe<T> {
            /// The payload of a `Just`.
            pub logic fn unwrap(self) -> T { builtin }
        }
    }
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

    /// A runtime place used as a value: its capture, the model of its
    /// snapshot. Field selection on the place happens first. A place of a
    /// logical type is itself.
    pub fn capture<T: Model + ?Sized>(_: &T) -> T::Logic {
        stub()
    }
    /// `snap(place)`: the snapshot.
    pub fn snap<T: Model + ?Sized>(_: &T) -> T::Snap {
        stub()
    }
    /// `model::<X>(s)`: `X`'s model, applied to a snapshot value. The type
    /// is written, because a snapshot type does not say what it is a
    /// snapshot of: `Snap<u32>` and `Snap<u64>` are both `Nat`. `X::model(s)`
    /// is the same function, by its path.
    pub fn model<X: Model + ?Sized>(s: X::Snap) -> X::Logic {
        X::__logic_model(s)
    }
    /// A name read inside a logic function, a `prop`, or a lemma's
    /// signature: every name there is a logical value, and is itself.
    pub fn value<T>(_: &T) -> T {
        stub()
    }
    /// `final!(r)`: the place that `r` ends with, as a place of `r`'s target type.
    pub fn final_value<'a, T: ?Sized>(_: &'a &mut T) -> &'a T {
        stub()
    }

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
            pub logic fn set(self, i: Nat, x: T) -> Seq<T> { builtin }
            pub logic fn last(self) -> T { builtin }
        }

        // `Option`'s snapshot and model (RFC 0002 names it).
        #[derive(Logical)]
        pub enum Maybe<T> { Nothing, Just(T) }

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
    pub use super::{False, Model, Proof, True};
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
