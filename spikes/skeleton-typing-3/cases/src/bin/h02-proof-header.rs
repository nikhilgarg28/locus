//! expect: runtime=error check=error
//! No proof type is written in the header of an `impl`: RFC 0002's
//! `impl From<Option<@(1 == 2)>> for Absurd` stays rejected.
use ::locus::locus;

locus! {
    struct Absurd;

    impl From<Option<@(1 == 2)>> for Absurd {
        fn from(_: Option<@(1 == 2)>) -> Absurd { Absurd }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
