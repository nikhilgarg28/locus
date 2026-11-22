//! expect: runtime=ok check=ok
//! t13a to t13d in one program, with `#![spike(logic_members_in_trait)]`:
//! the stubs are items of the lowered trait itself, under their mangled
//! names, so whatever imports the trait imports them.
use ::locus::locus;

locus! {
    #![spike(logic_members_in_trait)]
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

    fn through_parameter<T: OrderSpec>(a: &T) -> u32 {
        let p = prop!(T::le(*a, *a));
        1
    }

    fn through_type(r: Rational) -> u32 {
        let p = prop!(Rational::le(r, r));
        2
    }

    fn qualified<T: OrderSpec>(a: &T) -> u32 {
        let p = prop!(<T as OrderSpec>::le(*a, *a));
        4
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
