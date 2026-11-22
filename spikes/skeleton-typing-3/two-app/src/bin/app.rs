//! expect: runtime=ok check=ok
//! prints: run() => (13, 57, "two_lib::Rational", "two_lib::Sign", "app::Weight", "locus::Nat", "locus::Nat")
//! prints: across() => ("two_lib::Meter_Snap<locus::Nat>", "locus::Seq<locus::Nat>", "app::Rig_Snap<two_lib::Meter_Snap<locus::Nat>, two_lib::Stack_Snap<locus::Seq<locus::Nat>, locus::Nat>, two_lib::Pair_Snap<app::Scale_Snap<locus::Nat>, app::Scale_Snap<locus::Nat>>>", "locus::Nat", "locus::Int")
//! The second crate of the two-crate case. It uses the library's logical
//! types, logic functions, specification trait and runtime trait, gives
//! the specification trait to a logical type of its own, and the runtime
//! trait to a runtime type of its own.
use ::locus::locus;
use two_lib::{Bounded, Maybe, Meter, OrderSpec, Pair, Rational, Sign, Stack};

locus! {
    #[derive(Logical)]
    struct Weight { grams: Nat }

    impl OrderSpec for Weight {
        logic fn le(a: Weight, b: Weight) -> Prop { a.grams <= b.grams }
        logic fn le_refl(a: Weight) -> @(Weight::le(a, a)) { _ }
    }

    struct Scale { top: u32 }

    impl Bounded for Scale {
        logic fn limit_spec(s: Snap<Scale>) -> Nat { s.top }
        fn limit(&self) -> (l: u32, @(l == Scale::limit_spec(snap(self)))) { (self.top, _) }
        fn add(&self, x: u32, fits: @(x <= Scale::limit_spec(snap(self)))) -> (r: u32, @(r >= x)) { (x + 50, _) }
    }

    /// Round 3. A type of this crate that holds types of the library crate:
    /// its generated implementation is built from theirs.
    struct Rig { meter: Meter, stack: Stack, pair: Pair<Scale> }

    logic fn takes_nat(n: Nat) -> Nat { n }

    fn across() -> (&'static str, &'static str, &'static str, &'static str, &'static str) {
        let rig = Rig { meter: Meter { base: 1 }, stack: Stack { items: vec![1], cap: 3 }, pair: Pair { first: Scale { top: 1 }, second: Scale { top: 2 } } };
        // A snapshot made by the library's generic function, of the library's
        // type and of this crate's, used where the concrete type is wanted.
        let meter = two_lib::keep(&rig.meter);
        let whole = two_lib::keep(&rig);
        let stack = two_lib::read(&rig.stack);
        let n = logic! { takes_nat(two_lib::base_of(meter)) + takes_nat(whole.meter.base) + stack.len() + whole.pair.first.top };
        let agree = logic! { meter === whole.meter && stack === rig.stack && Stack::model(whole.stack) === stack };
        (type_of(&meter), type_of(&stack), type_of(&whole), type_of(&n), type_of(&logic! { Stack::room(snap(rig.stack)) }))
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
        let known: @(limit == Scale::limit_spec(snap(s)) && Meter::limit_spec(snap(m)) == 1000) = _;
        (two_lib::total(&m, 7, _), two_lib::total(&s, 7, _), type_of(&doubled), type_of(&sign), type_of(&heavier),
         type_of(&w), type_of(&logic! { Scale::limit_spec(snap(s)) }))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
    println!("across() => {:?}", across());
}
