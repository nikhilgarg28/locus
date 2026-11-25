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

    /// A runtime trait with a proof in a method's signature. Its logic
    /// function is over the snapshot of the type that implements it, so the
    /// trait asks for `Model` (round 3).
    pub trait Bounded: Model {
        logic fn limit_spec(s: Snap<Self>) -> Nat;
        fn limit(&self) -> (l: u32, @(l == Self::limit_spec(snap(self))));
        fn add(&self, x: u32, fits: @(x <= Self::limit_spec(snap(self)))) -> (r: u32, @(r >= x));
    }

    /// A runtime type with no declared model: its model is its snapshot.
    pub struct Meter { pub base: u32 }

    impl Bounded for Meter {
        logic fn limit_spec(s: Snap<Meter>) -> Nat { 1000 as Nat }
        fn limit(&self) -> (l: u32, @(l == Meter::limit_spec(snap(self)))) { (1000, _) }
        fn add(&self, x: u32, fits: @(x <= Meter::limit_spec(snap(self)))) -> (r: u32, @(r >= x)) { (self.base + x, _) }
    }

    pub fn total<B: Bounded>(b: &B, x: u32, fits: @(x <= B::limit_spec(snap(b)))) -> u32 {
        let (r, above) = b.add(x, fits);
        r
    }

    // Round 3: snapshots and models through another crate.

    /// A runtime type with a declared model.
    pub struct Stack { pub items: Vec<u32>, pub cap: usize }
    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(s: Snap<Stack>) -> Seq<Nat> { s.items }
    }
    impl Stack {
        pub logic fn room(s: Snap<Stack>) -> Int { s.cap - s.items.len() }
    }

    /// A generic runtime type: its implementation is conditional on `T`'s.
    pub struct Pair<T> { pub first: T, pub second: T }

    /// A generic function that returns a snapshot, and one that returns a model.
    pub fn keep<T: Model>(x: &T) -> Snap<T> { logic! { snap(x) } }
    pub fn read<T: Model>(x: &T) -> T::Logic { logic! { *x } }

    /// A logic function over the snapshot of a type of this crate.
    pub logic fn base_of(m: Snap<Meter>) -> Nat { m.base }
}
