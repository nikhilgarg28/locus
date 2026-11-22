//! expect: runtime=ok check=ok
//! t15 with `#![spike(logic_members_in_trait)]`.
use ::locus::locus;

locus! {
    #![spike(logic_members_in_trait)]
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
