// Probe s1: an opaque snapshot for every type that nothing declares one
// for, as a blanket implementation beside the declared ones.
use std::marker::PhantomData;
pub struct Seq<T>(PhantomData<T>);
pub struct Opaque;
pub struct BoundedU32;
pub trait Snapshot { type S; }
impl Snapshot for u32 { type S = BoundedU32; }
impl<T: Snapshot> Snapshot for Vec<T> { type S = Seq<T::S>; }
impl<T> Snapshot for T { type S = Opaque; }
fn main() {}
