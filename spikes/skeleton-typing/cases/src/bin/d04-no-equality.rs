//! expect: runtime=E0369 check=E0369
//! RFC 0002's protection stays: a marker has no `==`, so a derive of
//! `PartialEq` on a runtime type with a logical field fails, as it did with
//! `LogicalData`.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    #[derive(PartialEq)]
    struct Priced {
        cents: u32,
        exact: Rational,
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
