//! expect: runtime=ok check=ok
//! ask: bound Snap<Vec<u32>>: OrderSpec => holds
//! ask: bound Seq<Snap<u32>>: OrderSpec => holds
//! ask: impl Snap<Vec<u32>>: OrderSpec => the impl on line 33, at [Snap<u32>]
//! ask: impl Seq<Snap<u32>>: OrderSpec => the impl on line 33, at [Snap<u32>]
//! ask: bound Snap<&mut Vec<u32>>: OrderSpec => holds
//! ask: bound Snap<Vec<bool>>: OrderSpec => does not hold
//! ask: bound Snap<Nat>: OrderSpec => holds
//! ask: bound Nat: OrderSpec => holds
//! ask: impl Snap<Nat>: OrderSpec => the impl on line 27
//! ask: impl Nat: OrderSpec => the impl on line 27
//! ask: bound Snap<Vec<Nat>>: OrderSpec => holds
//! ask: bound Snap<Bool>: OrderSpec => does not hold
//! A reviewer's point: in the driver's questions, the stub for
//! `Snap<Vec<u32>>` must be the stub for `Seq<Snap<u32>>`, and the stub for
//! `Snap<Nat>` the stub for `Nat`. The library has a type family over stubs,
//! `SnapOf`, used for questions only. The header that names `Snap<u32>` is
//! written with the rejection of snapshot types in headers turned off.
use ::locus::locus;

locus! {
    #![spike(allow_ghost_headers)]

    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }
    impl OrderSpec for Nat {
        logic fn le(a: Nat, b: Nat) -> Bool { a <= b }
    }
    impl OrderSpec for Snap<u32> {
        logic fn le(a: Snap<u32>, b: Snap<u32>) -> Bool { a <= b }
    }
    impl<T: OrderSpec> OrderSpec for Seq<T> {
        logic fn le(a: Seq<T>, b: Seq<T>) -> Bool { a.len() <= b.len() }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
