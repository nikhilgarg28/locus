//! expect: runtime=reject check=reject
//! A snapshot type in a header, behind an alias. The draft's rule rejects
//! it. `locus!` sees only the name `Shot`.
use ::locus::locus;

locus! {
    trait Describe: Logical {
        logic fn width() -> Nat;
    }

    type Shot = Snap<u32>;

    impl Describe for Shot {
        logic fn width() -> Nat { 32 }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
