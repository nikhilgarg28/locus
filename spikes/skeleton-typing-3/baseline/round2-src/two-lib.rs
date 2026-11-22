//! The library crate of the two-crate case: logical types, logic
//! functions, a specification trait, and a runtime trait with a proof in a
//! method's signature. `two-app` uses all of them from another crate.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    pub struct Rational { pub num: Int, pub den: Int }

    #[derive(Logical)]
    pub enum Sign { Negative, Zero, Positive(Nat) }

    #[derive(Logical)]
    pub enum Maybe<T> { Nothing, Just(T) }

    pub logic fn sign_of(r: Rational) -> Sign {
        if r.num == 0 { Sign::Zero } else if r.num * r.den < 0 { Sign::Negative } else { Sign::Positive(1 as Nat) }
    }

    impl Rational {
        pub logic fn scaled(self, k: Int) -> Rational { Rational { num: self.num * k, den: self.den } }
    }

    /// A specification trait: an order on a logical type.
    pub trait OrderSpec: Logical {
        logic fn le(a: Self, b: Self) -> Prop;
        logic fn le_refl(a: Self) -> @(Self::le(a, a));
    }

    impl OrderSpec for Rational {
        logic fn le(a: Rational, b: Rational) -> Prop { a.num * b.den <= b.num * a.den }
        logic fn le_refl(a: Rational) -> @(Rational::le(a, a)) { _ }
    }

    /// Generic over the logical type, through the bound.
    pub logic fn max_of<T: OrderSpec>(a: T, b: T) -> T { if T::le(a, b) { b } else { a } }

    pub logic fn max_is_above<T: OrderSpec>(a: T, b: T) -> @(T::le(a, max_of(a, b))) { _ }

    /// A runtime trait with a proof in a method's signature.
    pub trait Bounded {
        logic fn limit_spec(self: &Self) -> Nat;
        fn limit(&self) -> (l: u32, @(l == Self::limit_spec(self)));
        fn add(&self, x: u32, fits: @(x <= Self::limit_spec(self))) -> (r: u32, @(r >= x));
    }

    pub struct Meter { pub base: u32 }

    impl Bounded for Meter {
        logic fn limit_spec(self: &Meter) -> Nat { 1000 as Nat }
        fn limit(&self) -> (l: u32, @(l == Meter::limit_spec(self))) { (1000, _) }
        fn add(&self, x: u32, fits: @(x <= Meter::limit_spec(self))) -> (r: u32, @(r >= x)) { (self.base + x, _) }
    }

    pub fn total<B: Bounded>(b: &B, x: u32, fits: @(x <= B::limit_spec(b))) -> u32 {
        let (r, above) = b.add(x, fits);
        r
    }
}
