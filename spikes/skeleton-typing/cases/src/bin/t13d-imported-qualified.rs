//! expect: runtime=ok check=ok
//! A trait declared in another module and imported by name, as Rust asks
//! for a trait whose items are used. By Rust's rules this program is fine.
//! The stubs of `OrderSpec`'s logic functions are in the hidden trait beside
//! it (this spike's default), which `use spec::OrderSpec;` does not import.
//! Here: `<T as OrderSpec>::le(..)`. (The `impl` names the trait by its path.)
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

    impl spec::OrderSpec for Rational {
        logic fn le(a: Rational, b: Rational) -> Bool { a.num * b.den <= b.num * a.den }
    }

    fn qualified<T: OrderSpec>(a: &T) -> u32 {
        let p = prop!(<T as OrderSpec>::le(*a, *a));
        4
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
