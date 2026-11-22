//! expect: runtime=ok check=ok
//! prints: run() => 2
//! Can r10's divergence arise from two traits? One trait is implemented for
//! `Wrap<Int>` only, and another with the same method name for every
//! `Wrap<T>`. With one marker both apply to `Wrap<Nat>` at the same step.
use ::locus::locus;

locus! {
    use std::marker::PhantomData;

    struct Wrap<T>(PhantomData<T>);

    trait Narrow {
        fn tag(&self) -> u8;
    }
    impl Narrow for Wrap<Int> {
        fn tag(&self) -> u8 { 1 }
    }
    trait Wide {
        fn tag(&self) -> u8;
    }
    impl<T> Wide for Wrap<T> {
        fn tag(&self) -> u8 { 2 }
    }

    fn of_nat(x: Wrap<Nat>) -> u8 { x.tag() }

    fn run() -> u8 { of_nat(Wrap(PhantomData)) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
