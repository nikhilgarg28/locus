//! Spike input: the runtime program Locus would emit for src/lib.lc, if it
//! checked. Cargo builds this file; the driver swaps in check/lib.rs instead.
use sample_dep::Meters;

pub fn first(v: &Vec<u32>, _nonempty: Erased) -> u32 {
    v[0]
}

pub fn demo() -> u32 {
    let v = vec![1, 2, 3];
    let m = Meters(5);
    let x = first(&v, Erased);
    let _kept: Option<Erased> = Some(Erased);
    let moved = v;
    x + m.get() + moved[0]
}

#[derive(Clone, Copy)]
pub struct Erased;
