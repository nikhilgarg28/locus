// Probe 5: the expected type selects how a place is read, by two
// implementations told apart by a marker parameter. With a third route for
// snapshots, a type whose model is its snapshot's type is ambiguous.
#![allow(dead_code, unused_variables)]
use std::marker::PhantomData;
pub struct Nat; pub struct Bool; pub struct Bounded32;
pub struct Seq<T>(PhantomData<fn() -> T>);
pub trait Model { type Logic; }
pub trait Snapshot { type S; }
impl Model for u32 { type Logic = Nat; }       impl Snapshot for u32 { type S = Bounded32; }
impl Model for bool { type Logic = Bool; }     impl Snapshot for bool { type S = Bool; }
pub struct ViaSelf; pub struct ViaModel; pub struct ViaSnap;
pub trait ArgAs<P, Via> {}
impl<T> ArgAs<T, ViaSelf> for T {}
impl<T: Model> ArgAs<T::Logic, ViaModel> for T {}
impl<T: Snapshot> ArgAs<T::S, ViaSnap> for T {}
pub fn arg<T: ArgAs<P, Via>, P, Via>(_: &T) -> P { panic!() }
fn takes_nat(_: Nat) {} fn takes_bounded(_: Bounded32) {} fn takes_bool(_: Bool) {}
fn main() {
    let (n, b) = (1u32, true);
    let never = || {
        takes_nat(arg(&n));        // the model
        takes_bounded(arg(&n));    // the snapshot
        takes_bool(arg(&b));       // both give `Bool`
    };
}
