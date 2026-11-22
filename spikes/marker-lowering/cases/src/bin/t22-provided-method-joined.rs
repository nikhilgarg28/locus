//! expect: runtime=error check=error
//! t21 in one invocation: `locus!` has the trait's declaration, and rejects
//! the `impl`.
use ::locus::locus;

locus! {
    trait Greet {
        fn hello(&self) -> u32 { 7 }
    }

    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    impl Greet for Rational {}
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
