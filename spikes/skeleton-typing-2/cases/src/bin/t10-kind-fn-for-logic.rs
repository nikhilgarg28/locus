//! expect: runtime=reject check=reject
//! Whether a member is a logic function is part of its declaration in the
//! trait. This `impl` gives `le` as a function that runs.
use ::locus::locus;

locus! {
    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }

    #[derive(Clone, Copy)]
    struct Version { major: u32, minor: u32 }

    impl OrderSpec for Version {
        fn le(a: Version, b: Version) -> bool { a.major <= b.major }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
