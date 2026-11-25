//! expect: runtime=reject check=reject
//! The other way round: the trait declares a method that runs, and this
//! `impl` gives it as a logic function.
use ::locus::locus;

locus! {
    trait OneByte {
        fn byte(&self) -> u8;
    }

    struct Letter(u8);

    impl OneByte for Letter {
        logic fn byte(l: Letter) -> Nat { 65 }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
