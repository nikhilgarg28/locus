// Probe s2: the same through a helper trait. `SnapOr` is implemented for
// every type, and gives the declared snapshot where there is one. That
// needs "where there is one, else": specialization.
#![allow(dead_code)]
use std::marker::PhantomData;
pub struct Seq<T>(PhantomData<T>);
pub struct Opaque;
pub struct BoundedU32;
pub trait Snapshot { type S; }
impl Snapshot for u32 { type S = BoundedU32; }
impl<T: Snapshot> Snapshot for Vec<T> { type S = Seq<T::S>; }
pub trait SnapOr { type S; }
impl<T> SnapOr for T { default type S = Opaque; }
impl<T: Snapshot> SnapOr for T { type S = T::S; }
fn main() {}
