//! expect: runtime=ok check=ok
//! `OrderSpec::le(r, r)`: the member named through its trait, as Rust allows
//! for an associated function. `locus!` cannot tell this path from
//! `Rational::le(r, r)`, and probes it the same way.
use ::locus::locus;

locus! {
    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }

    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    impl OrderSpec for Rational {
        logic fn le(a: Rational, b: Rational) -> Bool { a.num * b.den <= b.num * a.den }
    }

    fn through_trait(r: Rational) -> u32 {
        let p = prop!(OrderSpec::le(r, r));
        1
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
