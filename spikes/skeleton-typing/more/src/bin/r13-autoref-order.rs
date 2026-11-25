//! expect: runtime=ok check=ok
//! prints: run() => (2, 2)
//! Can r10's divergence arise from the order of rustc's method lookup
//! alone, between two traits? One trait takes `self` by value and is
//! implemented for `Wrap<Int>` only; another takes `&self` and is
//! implemented for every `Wrap<T>`. rustc tries the receiver by value
//! before by reference. The second pair is the same through a `Box`:
//! by value on the box before on what it holds.
use ::locus::locus;

locus! {
    use std::marker::PhantomData;

    struct Wrap<T>(PhantomData<T>);

    trait ByValue {
        fn tag(self) -> u8;
    }
    impl ByValue for Wrap<Int> {
        fn tag(self) -> u8 { 1 }
    }
    trait ByReference {
        fn tag(&self) -> u8;
    }
    impl<T> ByReference for Wrap<T> {
        fn tag(&self) -> u8 { 2 }
    }

    trait OnBox {
        fn mark(&self) -> u8;
    }
    impl OnBox for Box<Wrap<Int>> {
        fn mark(&self) -> u8 { 1 }
    }
    trait Inside {
        fn mark(&self) -> u8;
    }
    impl<T> Inside for Wrap<T> {
        fn mark(&self) -> u8 { 2 }
    }

    fn of_nat(x: Wrap<Nat>, boxed: Box<Wrap<Nat>>) -> (u8, u8) { (x.tag(), boxed.mark()) }

    fn run() -> (u8, u8) { of_nat(Wrap(PhantomData), Box::new(Wrap(PhantomData))) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
