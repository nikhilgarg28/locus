//! expect: runtime=ok check=ok
//! prints: run() => 15
//! ask: impl Rational: OrderSpec => the impl on line 25
//! ask: in through_parameter: impl T: OrderSpec => the bound in scope
//! ask: in through_parameter: bound T: OrderSpec => holds
//! ask: bound Rational: OrderSpec => holds
//! The spellings of a trait's logic function in logical code, with the
//! trait imported by name from another module. `locus!` probes the name of
//! the type or trait before the last segment, by `use path as _;`, and
//! nothing for a type parameter. Which member and which implementation is
//! the driver's to answer.
use ::locus::locus;

locus! {
    mod spec {
        pub trait OrderSpec {
            logic fn le(a: Self, b: Self) -> Bool;
        }
    }
    use spec::OrderSpec;

    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    impl OrderSpec for Rational {
        logic fn le(a: Rational, b: Rational) -> Bool { a.num * b.den <= b.num * a.den }
    }

    fn through_trait(r: Rational) -> u32 {
        let p = prop!(OrderSpec::le(r, r));
        1
    }
    fn through_type(r: Rational) -> u32 {
        let p = prop!(Rational::le(r, r));
        2
    }
    fn through_parameter<T: OrderSpec>(a: &T) -> u32 {
        let p = prop!(T::le(*a, *a));
        4
    }
    fn qualified<T: OrderSpec>(a: &T, r: Rational) -> u32 {
        let p = prop!(<T as OrderSpec>::le(*a, *a) && <Rational as spec::OrderSpec>::le(r, r));
        8
    }

    fn run(r: Rational) -> u32 {
        through_trait(r) + through_type(r) + through_parameter(&r) + qualified(&r, r)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
