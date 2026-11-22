//! expect: runtime=ok check=ok
//! prints: run() => 1
//! ask: bound Int: Named => holds
//! ask: bound Seq<Nat>: Named => holds
//! ask: bound u32: Named => does not hold
//! ask: impl Int: Named => the impl on line 17, at [Int]
//! s08 as this mode lowers it by default: a trait with an implementation
//! for a bare type parameter, in the same invocation, gets no implementation
//! for the marker. The program's own implementation serves the marker, which
//! is `Logical`, and the stubs.
use ::locus::locus;

locus! {
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
