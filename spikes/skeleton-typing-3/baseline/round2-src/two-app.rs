//! expect: runtime=ok check=ok
//! prints: run() => (13, 57, "two_lib::Rational", "two_lib::Sign", "app::Weight", "locus::Nat", "locus::Nat")
//! The second crate of the two-crate case. It uses the library's logical
//! types, logic functions, specification trait and runtime trait, gives
//! the specification trait to a logical type of its own, and the runtime
//! trait to a runtime type of its own.
use ::locus::locus;
use two_lib::{Bounded, Maybe, Meter, OrderSpec, Rational, Sign};

locus! {
    #[derive(Logical)]
    struct Weight { grams: Nat }

    impl OrderSpec for Weight {
        logic fn le(a: Weight, b: Weight) -> Prop { a.grams <= b.grams }
        logic fn le_refl(a: Weight) -> @(Weight::le(a, a)) { _ }
    }

    struct Scale { top: u32 }

    impl Bounded for Scale {
        logic fn limit_spec(self: &Scale) -> Nat { self.top }
        fn limit(&self) -> (l: u32, @(l == Scale::limit_spec(self))) { (self.top, _) }
        fn add(&self, x: u32, fits: @(x <= Scale::limit_spec(self))) -> (r: u32, @(r >= x)) { (x + 50, _) }
    }

    /// A match on the library's logical enums, one of them generic.
    logic fn weight_of(s: Sign, m: Maybe<Nat>) -> Nat {
        match s {
            Sign::Negative => 0 as Nat,
            Sign::Zero => match m { Maybe::<Nat>::Nothing => 0 as Nat, Maybe::<Nat>::Just(n) => n },
            Sign::Positive(n) => n,
        }
    }

    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run() -> (u32, u32, &'static str, &'static str, &'static str, &'static str, &'static str) {
        let half = logic! { Rational { num: 1, den: 2 } };
        let doubled = logic! { half.scaled(2) };
        let sign = logic! { two_lib::sign_of(doubled) };
        let heavier = logic! { two_lib::max_of(Weight { grams: 3 as Nat }, Weight { grams: 4 as Nat }) };
        let above = logic! { two_lib::max_is_above(half, doubled) };
        let ordered: @(Rational::le(half, two_lib::max_of(half, doubled)) && Weight::le(heavier, heavier)) = _;
        let w = logic! { weight_of(sign, Maybe::Just(heavier.grams)) };
        let m = Meter { base: 6 };
        let s = Scale { top: 9 };
        let (limit, is_limit) = s.limit();
        // A parameter declared `&Scale` takes a place of that type: logical
        // code has no `&`, so the references are runtime locals.
        let (sr, mr) = (&s, &m);
        let known: @(limit == Scale::limit_spec(sr) && Meter::limit_spec(mr) == 1000) = _;
        (two_lib::total(&m, 7, _), two_lib::total(&s, 7, _), type_of(&doubled), type_of(&sign), type_of(&heavier),
         type_of(&w), type_of(&logic! { Scale::limit_spec(sr) }))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
