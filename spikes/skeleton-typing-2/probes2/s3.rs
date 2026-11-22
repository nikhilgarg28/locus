// Probe s3: the helper trait with a marker parameter, so that the two
// implementations do not overlap: `SnapVia<Declared>` where a snapshot is
// declared, `SnapVia<Fallback>` for every type. Coherence accepts it. A
// type position must then name the marker, which only lowering could
// choose; an expression lets rustc choose, and where both apply it cannot.
#![allow(dead_code)]
use std::marker::PhantomData;
pub struct Seq<T>(PhantomData<T>);
pub struct Opaque;
pub struct BoundedU32;
pub trait Snapshot { type S; }
impl Snapshot for u32 { type S = BoundedU32; }
impl<T: Snapshot> Snapshot for Vec<T> { type S = Seq<T::S>; }
pub struct Declared; pub struct Fallback;
pub trait SnapVia<How> { type S; }
impl<T: Snapshot> SnapVia<Declared> for T { type S = T::S; }
impl<T> SnapVia<Fallback> for T { type S = Opaque; }
fn snap<T: SnapVia<How>, How>(_: &T) -> T::S { panic!() }
struct Foreign;
fn main() {
    let _ = || {
        let only_the_fallback = snap(&Foreign);     // one candidate: an Opaque
        #[cfg(both)]
        let both_apply = snap(&1u32);               // two candidates
    };
}
