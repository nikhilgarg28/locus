//! expect: runtime=ok check=ok
//! prints: run() => 2
//! Can r10's divergence arise from an associated constant? An inherent
//! constant at `Wrap<Int>` only, and a trait's constant at every `Wrap<T>`.
use ::locus::locus;

locus! {
    use std::marker::PhantomData;

    struct Wrap<T>(PhantomData<T>);

    impl Wrap<Int> {
        const TAG: u8 = 1;
    }
    trait HasTag {
        const TAG: u8;
    }
    impl<T> HasTag for Wrap<T> {
        const TAG: u8 = 2;
    }

    fn run() -> u8 { Wrap::<Nat>::TAG }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
