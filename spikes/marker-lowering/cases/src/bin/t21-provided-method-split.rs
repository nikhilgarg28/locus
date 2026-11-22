//! expect: runtime=reject check=reject
//! A logical type implements only traits with no method that runs. `Greet`
//! has one, with a default body, and is declared in another invocation of
//! `locus!` than this `impl`, which gives no member. The draft rejects the
//! `impl`. `locus!` sees nothing in it that runs.
use ::locus::locus;

locus! {
    trait Greet {
        fn hello(&self) -> u32 { 7 }
    }
}

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    impl Greet for Rational {}

    fn run(half: Rational) -> u32 {
        half.hello()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
