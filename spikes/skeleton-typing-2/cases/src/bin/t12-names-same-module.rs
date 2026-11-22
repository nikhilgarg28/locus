//! expect: runtime=ok check=ok
//! prints: run() => 15
//! The ways logical code names a logic function of a trait, each probed in
//! the checking expansion, in the module that declares the trait.
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

    #[derive(Clone, Copy)]
    struct Version { major: u32 }

    impl OrderSpec for Version {
        logic fn le(a: Version, b: Version) -> Bool { a.major <= b.major }
    }

    impl Version {
        fn through_self(&self) -> u32 {
            let p = prop!(Self::le(*self, *self));
            1
        }
    }

    fn through_parameter<T: OrderSpec>(a: &T) -> u32 {
        let p = prop!(T::le(*a, *a));
        2
    }

    fn through_type(r: Rational, v: &Version) -> u32 {
        let p = prop!(Rational::le(r, r) && Version::le(*v, *v));
        4
    }

    fn qualified<T: OrderSpec>(a: &T, r: Rational) -> u32 {
        let p = prop!(<T as OrderSpec>::le(*a, *a) && <Rational as OrderSpec>::le(r, r));
        8
    }

    fn run(r: Rational) -> u32 {
        let v = Version { major: 1 };
        v.through_self() + through_parameter(&v) + through_type(r, &v) + qualified(&v, r)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
