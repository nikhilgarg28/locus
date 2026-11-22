//! expect: runtime=ok check=ok
//! prints: run() => 1
//! A logic function with a default body, and an `impl` that gives no
//! member, in another invocation of `locus!` than the trait. Whether the
//! trait has a hidden trait for its stubs cannot be read from this `impl`.
use ::locus::locus;

locus! {
    trait Named {
        logic fn tag() -> Nat { 0 }
    }
}

locus! {
    struct Thing;

    impl Named for Thing {}

    fn run() -> u32 {
        let t = logic! { Thing::tag() };
        1
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
