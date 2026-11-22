//! expect: runtime=ok check=ok
//! prints: run() => 2
//! ask: bound Pair<Rational>: OrderSpec => holds
//! ask: bound Pair<Int>: OrderSpec => holds
//! ask: bound Pair<Bool>: OrderSpec => does not hold
//! Two implementations for one runtime generic type at two logical types.
//! The self type is a runtime type, so its header is program text, and both
//! arguments are the marker.
use ::locus::locus;

locus! {
    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }

    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    struct Pair<T> { first: T, second: T }

    impl OrderSpec for Pair<Rational> {
        logic fn le(a: Pair<Rational>, b: Pair<Rational>) -> Bool { true }
    }
    impl OrderSpec for Pair<Int> {
        logic fn le(a: Pair<Int>, b: Pair<Int>) -> Bool { a.first <= b.first }
    }

    fn show<T: OrderSpec>(x: T) -> u32 { 1 }
    fn run(r: Rational, i: Int) -> u32 {
        show(Pair { first: r, second: r }) + show(Pair { first: i, second: i })
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost()));
}
