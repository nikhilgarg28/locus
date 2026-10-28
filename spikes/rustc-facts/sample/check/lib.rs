//! Spike input: a Locus source file, in provisional syntax.
use sample_dep::Meters;

fn total(xs: &[u32]) -> __locus::Nat { __locus::stub() }

pub fn first(v: &Vec<u32>, nonempty: __locus::Proof) -> u32 {
    v[0]
}

pub fn demo() -> u32 {
    let v = vec![1, 2, 3];
    let m = Meters(5); __locus::resolve(Meters);
    let known: __locus::Proof = { __locus::observe(&v); __locus::resolve(total); __locus::hole() };
    let x = first(&v, known);
    let kept: Option<__locus::Proof> = Some({ __locus::observe(&m.0); __locus::hole() });
    let moved = v;
    let late: __locus::Proof = { __locus::observe(&v); __locus::hole() };
    x + m.get() + moved[0]
}
// Everything below is synthesized. The lines above match src/lib.lc line for line.
#[allow(dead_code)]
mod __locus {
    #[derive(Clone, Copy)]
    pub struct Proof;
    pub struct Nat;
    pub fn observe<T: ?Sized>(_: &T) {}
    pub fn resolve<T>(_: T) {}
    pub fn hole() -> Proof {
        Proof
    }
    pub fn stub<T>() -> T {
        loop {}
    }
}
