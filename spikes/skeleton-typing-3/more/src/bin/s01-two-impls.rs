//! expect: runtime=ok check=ok
//! prints: run() => 6
//! ask: bound Int: OrderSpec => holds
//! ask: bound Rational: OrderSpec => holds
//! ask: bound Nat: OrderSpec => does not hold
//! ask: impl Int: OrderSpec => the impl on line 22
//! ask: impl Rational: OrderSpec => the impl on line 25
//! Two implementations of one logic-only trait, for the library's `Int` and
//! for a derived `Rational`, beside the empty implementation that the trait
//! gets for the marker. With one marker and no stubs this was E0119: both
//! were implementations for `LogicalData`.
use ::locus::locus;

locus! {
    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }

    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    impl OrderSpec for Int {
        logic fn le(a: Int, b: Int) -> Bool { a <= b }
    }
    impl OrderSpec for Rational {
        logic fn le(a: Rational, b: Rational) -> Bool { a.num * b.den <= b.num * a.den }
    }

    // Program text: each bound is met at the marker.
    fn show<T: OrderSpec>(x: T) -> u32 { 1 }

    fn run(i: Int, r: Rational, n: Nat) -> u32 {
        let p = prop!(Int::le(i, i) && Rational::le(r, r));
        show(i) + show(r) + show(logic! { r }) + show(n) + show(logic! { 5 }) + show(p)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost(), ::locus::ghost()));
}
