//! expect: runtime=ok check=ok
//! prints: run() => ["locus::Nat", "locus::Nat", "locus::Seq<locus::Nat>", "(locus::Nat, locus::Bool)", "locus::Bool", "locus::Seq<locus::BoundedU32>", "a02_places::__locus_snap_Counter", "locus::Nat"]
//! A runtime place read as a value is its type's model: a field, a sum of
//! two fields of different machine types, the target of a `&mut`, a tuple
//! and its component. `snap(..)` gives the snapshot instead. `final!(r)` is
//! the value that `r` ends with.
use ::locus::locus;

locus! {
    struct Counter { hits: u32, limit: u8 }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn first_mut(pair: &mut (u32, u32)) -> (r: &mut u32, @(final!(pair) === (final!(r), pair.1))) {
        (&mut pair.0, _)
    }

    fn run(c: &Counter, r: &mut Vec<u32>, pair: (u32, bool)) -> Vec<&'static str> {
        let a = logic! { c.hits };
        let b = logic! { c.hits + c.limit };
        let d = logic! { *r };
        let e = logic! { pair };
        let f = logic! { pair.1 };
        let g = logic! { snap(*r) };
        let h = logic! { snap(c) };
        let mut two = (1u32, 2u32);
        let (one, related) = first_mut(&mut two);
        let end = logic! { final!(one) };
        let ends: @(final!(one) >= 0 && final!(one) === end) = _;
        *one = 5;
        vec![type_of(&a), type_of(&b), type_of(&d), type_of(&e), type_of(&f), type_of(&g), type_of(&h), type_of(&end)]
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&Counter { hits: 1, limit: 2 }, &mut vec![1], (1, true)));
}
