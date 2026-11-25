//! expect: runtime=error check=error
//! A logical type implements only traits with no method that runs. `locus!`
//! rejects this `impl`: it gives `le_checked`. The declarations are t01's.
use ::locus::locus;

locus! {
    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
        logic fn le_total(a: Self, b: Self) -> @(Self::le(a, b) || Self::le(b, a));
    }

    trait CheckedOrder: OrderSpec {
        fn le_checked(&self, other: &Self) -> (r: bool, @(r === Self::le(*self, *other)));
    }

    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    #[derive(Debug, Clone, Copy)]
    struct Version { major: u32, minor: u32 }

    // A logical type: the specification.
    impl OrderSpec for Rational {
        logic fn le(a: Rational, b: Rational) -> Bool { a.num * b.den <= b.num * a.den }
        logic fn le_total(a: Rational, b: Rational) -> @(Rational::le(a, b) || Rational::le(b, a)) { _ }
    }

    // A runtime type: both.
    impl OrderSpec for Version {
        logic fn le(a: Version, b: Version) -> Bool {
            a.major < b.major || (a.major == b.major && a.minor <= b.minor)
        }
        logic fn le_total(a: Version, b: Version) -> @(Version::le(a, b) || Version::le(b, a)) { _ }
    }

    impl CheckedOrder for Version {
        fn le_checked(&self, other: &Self) -> (r: bool, @(r === Self::le(*self, *other))) {
            let r = self.major < other.major || (self.major == other.major && self.minor <= other.minor);
            let agrees = prove!(r === Self::le(*self, *other));
            (r, agrees)
        }
    }

    // Generic logical code asks for the specification.
    logic fn is_max<T: OrderSpec>(m: T, a: T, b: T) -> Prop {
        T::le(a, m) && T::le(b, m)
    }

    // Generic code that runs asks for the code.
    fn max<'a, T: CheckedOrder>(a: &'a T, b: &'a T) -> (m: &'a T, @(is_max(*m, *a, *b))) {
        let (a_le_b, why) = a.le_checked(b);
        let total = logic! { T::le_total(*a, *b) };
        if a_le_b {
            (b, prove!(is_max(*b, *a, *b)))
        } else {
            (a, prove!(is_max(*a, *a, *b)))
        }
    }

    impl CheckedOrder for Rational {
        fn le_checked(&self, other: &Self) -> (r: bool, @(r === Self::le(*self, *other))) {
            (true, _)
        }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
