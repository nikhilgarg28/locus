//! expect: runtime=ok check=ok
//! A logic function in an `impl` with a bound, for a generic logical type,
//! called from logical code. The probe must write the type's argument, and
//! `locus!` has no logical type to write there: it writes the untyped
//! marker, which satisfies no bound. With one marker, `LogicalData` stood
//! for every argument.
use ::locus::locus;

locus! {
    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }
    impl OrderSpec for Int {
        logic fn le(a: Int, b: Int) -> Bool { a <= b }
    }

    #[derive(Logical)]
    struct Pair<T> { first: T, second: T }

    impl<T: OrderSpec> Pair<T> {
        logic fn least(p: Pair<T>) -> T {
            if T::le(p.first, p.second) { p.first } else { p.second }
        }
    }

    fn run(p: Pair<Int>) -> u32 {
        let low = logic! { Pair::least(p) };
        1
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
