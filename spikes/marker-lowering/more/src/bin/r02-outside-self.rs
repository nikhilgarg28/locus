//! expect: runtime=ok check=ok
//! prints: run() => 3
//! ask: bound u32: Spec<Int> => holds
//! ask: bound u32: Spec<Nat> => holds
//! ask: bound u32: Spec<Bool> => does not hold
//! A reviewer's counterexample: the distinction is outside `Self`. A
//! logic-only trait with a parameter, implemented twice for one runtime
//! type, at two logical arguments; and a generic function bounded by one of
//! them.
use ::locus::locus;

locus! {
    trait Spec<A> {
        logic fn spec(x: Self) -> A;
    }
    impl Spec<Int> for u32 {
        logic fn spec(x: u32) -> Int { x }
    }
    impl Spec<Nat> for u32 {
        logic fn spec(x: u32) -> Nat { x }
    }

    fn as_int<T: Spec<Int>>(x: T) -> u32 { 1 }
    fn as_nat<T: Spec<Nat>>(x: T) -> u32 { 2 }

    fn run() -> u32 { as_int(7u32) + as_nat(7u32) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
