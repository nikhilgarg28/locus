// Probe s5: the choice made in an expression by Rust's method lookup (by
// value before by reference), as the spike's `snap(x)` does. Declared
// where the type has a snapshot, opaque where it has none. For a type
// parameter with no bound it is opaque too, whatever the parameter is
// later: the caller's `Snap<u32>` and the callee's snapshot of its `T`
// are then two types.
#![allow(dead_code, unused_variables)]
use std::marker::PhantomData;
fn stub<T>() -> T { panic!() }
#[derive(Clone, Copy)] pub struct Opaque;
#[derive(Clone, Copy)] pub struct BoundedU32;
pub trait Snapshot { type S; }
impl Snapshot for u32 { type S = BoundedU32; }
pub struct SnapshotOf<T: ?Sized>(PhantomData<fn() -> PhantomData<T>>);
pub fn snapshot_of<T: ?Sized>(_: &T) -> SnapshotOf<T> { stub() }
pub trait SnapDeclared { type Out; fn the_snap(&self) -> Self::Out; }
impl<T: Snapshot + ?Sized> SnapDeclared for SnapshotOf<T> { type Out = T::S; fn the_snap(&self) -> T::S { stub() } }
pub trait SnapOpaque { fn the_snap(&self) -> Opaque; }
impl<T: ?Sized> SnapOpaque for &SnapshotOf<T> { fn the_snap(&self) -> Opaque { stub() } }
struct Foreign;
fn name<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
fn generic<T>(x: T) -> &'static str { let s: Opaque = (&snapshot_of(&x)).the_snap(); name(&s) }
fn bounded<T: Snapshot>(x: T) -> &'static str { let s: T::S = (&snapshot_of(&x)).the_snap(); std::any::type_name::<T::S>() }
fn main() {
    let _ = || {
        let declared: BoundedU32 = (&snapshot_of(&1u32)).the_snap();
        let opaque: Opaque = (&snapshot_of(&Foreign)).the_snap();
    };
    println!("s5: a u32 through an unbounded parameter: {}; through a bounded one: {}",
        std::any::type_name::<Opaque>(), std::any::type_name::<<u32 as Snapshot>::S>());
}
