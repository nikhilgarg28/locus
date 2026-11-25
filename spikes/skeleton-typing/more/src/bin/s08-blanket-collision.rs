//! expect: runtime=E0119 check=E0119
//! The collision. A program's own implementation for every logical type,
//! `impl<T: Logical> Named for T`, and the empty implementation that the
//! trait gets for the marker, which is `Logical` too. Here the marker's
//! implementation is written regardless (`always_marker_impl`).
use ::locus::locus;

locus! {
    #![spike(always_marker_impl)]

    trait Named {
        logic fn name() -> Nat;
    }
    impl<T: Logical> Named for T {
        logic fn name() -> Nat { 0 }
    }

    fn named<T: Named>(x: T) -> u32 { 1 }
    fn run(n: Nat) -> u32 { named(n) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
