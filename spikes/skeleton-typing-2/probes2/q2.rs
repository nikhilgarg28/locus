// Probe q2 (P4): the callee's stub takes each parameter as `impl`-like
// generic, so that a place given to a parameter whose type is a type
// parameter is its model, with no ambiguity; a literal argument is a `Nat`
// where a `Nat` is wanted and an `Int` for a type parameter.
#![allow(dead_code, unused_variables)]
use std::marker::PhantomData;
fn stub<T>() -> T { panic!() }
#[derive(Clone, Copy, Debug)] pub struct Nat;
#[derive(Clone, Copy, Debug)] pub struct Int;
pub struct Seq<T>(PhantomData<fn() -> T>);
impl<T> Clone for Seq<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for Seq<T> {}
pub trait Logical: Copy {}
impl Logical for Nat {} impl Logical for Int {} impl<T> Logical for Seq<T> {}
pub trait Model { type Logic; }
impl Model for Nat { type Logic = Nat; } impl Model for Int { type Logic = Int; }
impl<T> Model for Seq<T> { type Logic = Seq<T>; }
impl Model for u32 { type Logic = Nat; } impl Model for i32 { type Logic = Int; }
impl<T: Model> Model for Vec<T> { type Logic = Seq<T::Logic>; }
impl<T: Model + ?Sized> Model for &T { type Logic = T::Logic; }
pub trait Runtime: Model {}
impl Runtime for u32 {} impl Runtime for i32 {} impl<T: Model> Runtime for Vec<T> {} impl<T: Runtime + ?Sized> Runtime for &T {}

/// A place of logical code that has not been read yet.
pub struct Place<T: ?Sized>(PhantomData<fn() -> T>);
pub fn place<T: ?Sized>(_: &T) -> Place<T> { stub() }
pub struct Literal;
pub fn literal(_: u128) -> Literal { stub() }

pub struct Itself; pub struct ItsModel; pub struct ThePlace; pub struct ALiteral;
/// A parameter of a concrete type.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be given where a `{P}` is wanted in logical code",
    label = "this argument",
    note = "a place is given as it is, or as its model; a `Nat` becomes an `Int` when it is written `n as Int`")]
pub trait Arg<P, Via> {}
impl<P> Arg<P, Itself> for P {}
impl<R: Runtime + ?Sized> Arg<R::Logic, ItsModel> for Place<R> {}
impl<R> Arg<R, ThePlace> for Place<R> {}
impl Arg<Nat, ALiteral> for Literal {}
impl Arg<Int, ALiteral> for Literal {}
/// A parameter whose type is a type parameter that ranges over logical types.
pub trait ModelArg<T, Via> {}
impl<P: Logical> ModelArg<P, Itself> for P {}
impl<R: Model + ?Sized> ModelArg<R::Logic, ItsModel> for Place<R> {}
impl ModelArg<Int, ALiteral> for Literal {}

// logic fn double(n: Nat) -> Nat
fn double<A0: Arg<Nat, V0>, V0>(n: A0) -> Nat { stub() }
// logic fn same<T>(t: T) -> T
fn same<T: Logical, A0: ModelArg<T, V0>, V0>(t: A0) -> T { stub() }
// logic fn first<T>(xs: Seq<T>) -> T
fn first<T: Logical, A0: Arg<Seq<T>, V0>, V0>(xs: A0) -> T { stub() }
// logic fn total(v: &Vec<u32>) -> Nat      (a runtime type: the place itself)
fn total<'a, A0: Arg<&'a Vec<u32>, V0>, V0>(v: A0) -> Nat { stub() }

fn takes_nat(_: Nat) {} fn takes_int(_: Int) {} fn takes_seq(_: Seq<Nat>) {}
fn user(n: u32, k: Nat, v: &Vec<u32>, i: i32) {
    let _ = || {
        takes_nat(double(place(&n)));       // a runtime place: its model
        takes_nat(double(place(&k)));       // a ghost place: itself
        takes_nat(double(literal(3)));      // a literal where a Nat is wanted
        takes_nat(double(double(place(&n)))); // a computed value
        takes_nat(same(place(&n)));         // T is Nat: the model wins
        takes_int(same(place(&i)));
        takes_int(same(literal(0)));        // T is Int
        takes_seq(same(place(&v)));         // T is Seq<Nat>
        takes_nat(first(place(&v)));        // Seq<T> from a &Vec<u32>
        takes_nat(total(place(&v)));        // the place itself
        let x = same(literal(0));           // forward: an Int
        #[cfg(backward)] takes_nat(x);
        #[cfg(mismatch)] takes_nat(double(place(&v)));
        #[cfg(widen)] takes_int(first(place(&v)));
    };
}
fn main() { println!("q2 ok"); }
