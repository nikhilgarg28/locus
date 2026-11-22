// Probe s4: an explicit opaque marker that lowering writes where it knows
// that a type is foreign: `Snap<Foreign<'a>>` as `OpaqueOf<Foreign<'a>>`.
// It tells two foreign types apart, and it keeps the type's lifetimes: the
// snapshot cannot be held after what the value borrowed is gone.
#![allow(dead_code, unused_variables)]
use std::marker::PhantomData;
#[derive(Clone, Copy)]
pub struct OpaqueOf<T: ?Sized>(PhantomData<fn() -> PhantomData<T>>);
fn snap_foreign<T: ?Sized>(_: &T) -> OpaqueOf<T> { OpaqueOf(PhantomData) }
struct Foreign<'a>(&'a mut u32);
fn main() {
    let mut n = 1;
    let f = Foreign(&mut n);
    let before = snap_foreign(&f);      // OpaqueOf<Foreign<'_>>
    drop(f);
    n += 1;                             // `n` is free again
    #[cfg(held)]
    let still = before;                 // the snapshot, held past the borrow
    println!("s4 ok: {n}");
}
