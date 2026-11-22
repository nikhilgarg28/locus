//! expect: runtime=ok check=ok
//! prints: run() => 3
//! ask: bound Seq<Rational>: OrderSpec => holds
//! ask: bound Seq<Seq<Int>>: OrderSpec => holds
//! ask: bound Pair<Rational>: OrderSpec => holds
//! ask: bound Pair<Seq<Int>>: OrderSpec => holds
//! ask: bound Bool: OrderSpec => does not hold
//! ask: bound Seq<Bool>: OrderSpec => does not hold
//! ask: bound Pair<Bool>: OrderSpec => does not hold
//! ask: impl Seq<Seq<Int>>: OrderSpec => the impl on line 40, at [Seq<Int>]
//! ask: impl Pair<Rational>: OrderSpec => the impl on line 46, at [Rational]
//! ask: in generic: bound Seq<T>: OrderSpec => holds
//! ask: in generic: bound Pair<Seq<T>>: OrderSpec => holds
//! ask: in generic: bound T: Monoid => does not hold
//! ask: in generic: impl T: OrderSpec => the bound in scope
//! A generic logical self type, `Seq<T>`, and a generic runtime one,
//! `Pair<T>`. The driver is asked about stubs, a runtime type over a stub,
//! and the same under a generic function's bound.
use ::locus::locus;

locus! {
    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }
    trait Monoid: Logical {
        logic fn unit() -> Self;
    }

    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    struct Pair<T> { first: T, second: T }

    impl OrderSpec for Int {
        logic fn le(a: Int, b: Int) -> Bool { a <= b }
    }
    impl OrderSpec for Rational {
        logic fn le(a: Rational, b: Rational) -> Bool { a.num * b.den <= b.num * a.den }
    }
    impl<T: OrderSpec> OrderSpec for Seq<T> {
        logic fn le(a: Seq<T>, b: Seq<T>) -> Bool {
            a.len() == 0 || (b.len() > 0 && T::le(a[0], b[0]))
        }
    }
    // A runtime type: program text.
    impl<T: OrderSpec> OrderSpec for Pair<T> {
        logic fn le(a: Pair<T>, b: Pair<T>) -> Bool { T::le(a.first, b.first) }
    }

    fn generic<T: OrderSpec>(x: T) -> u32 { 1 }

    fn run(r: Rational, s: Seq<Seq<Int>>) -> u32 {
        generic(r) + generic(s) + generic(Pair { first: r, second: r })
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost()));
}
