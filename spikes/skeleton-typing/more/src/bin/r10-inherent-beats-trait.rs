//! expect: runtime=ok check=ok
//! prints: run() => 2
//! A reviewer's counterexample for the split variant, where both expansions
//! compile and call different methods. `Wrap<T>` is a runtime type. It has
//! an inherent `tag` at `Wrap<Int>` only, and a trait's `tag` at every
//! `Wrap<T>`. With a type for each logical type, `Wrap<Nat>` has no inherent
//! `tag`, and rustc calls the trait's. With one marker, `Wrap<Int>` is
//! `Wrap<Nat>`, and the inherent method wins. No two implementations
//! conflict, and every `Self` is a runtime type.
use ::locus::locus;

locus! {
    use std::marker::PhantomData;

    struct Wrap<T>(PhantomData<T>);

    impl Wrap<Int> {
        fn tag(&self) -> u8 { 1 }
    }
    trait Tag {
        fn tag(&self) -> u8;
    }
    impl<T> Tag for Wrap<T> {
        fn tag(&self) -> u8 { 2 }
    }

    fn of_nat(x: Wrap<Nat>) -> u8 { x.tag() }

    fn run() -> u8 { of_nat(Wrap(PhantomData)) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
