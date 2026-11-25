//! expect: runtime=error check=error
//! No snapshot type is written in the header of an `impl`: `locus!` rejects it.
use ::locus::locus;

locus! {
    trait Describe: Logical {
        logic fn width() -> Nat;
    }

    impl Describe for Snap<u32> {
        logic fn width() -> Nat { 32 }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
