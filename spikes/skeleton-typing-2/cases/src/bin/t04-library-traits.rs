//! expect: runtime=ok check=ok
//! prints: run() => 15
//! The library's logical traits, `AddLogic`, `FromLogic` and `IntoLogic`
//! with its blanket implementation, used with a program's logical type.
//! They are written in Locus in the library and lowered by `locus!` there.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    impl AddLogic for Rational {
        type Output = Rational;
        logic fn add(a: Rational, b: Rational) -> Rational {
            Rational { num: a.num * b.den + b.num * a.den, den: a.den * b.den }
        }
    }

    // A program's type from the library's, and the library's from a program's.
    impl FromLogic<Int> for Rational {
        logic fn from(t: Int) -> Rational { Rational { num: t, den: 1 } }
    }
    impl FromLogic<Nat> for Rational {
        logic fn from(t: Nat) -> Rational { Rational { num: t, den: 1 } }
    }
    impl FromLogic<Rational> for Int {
        logic fn from(t: Rational) -> Int { t.num / t.den }
    }

    // The function the draft's third review gave as rejected against Rust's `Add`.
    logic fn twice<T: AddLogic<Output = T>>(x: T) -> T { x + x }

    logic fn widen<T: IntoLogic<Rational>>(t: T) -> Rational { t.into() }

    // Runtime functions with the same bounds on their ghost parameters, so
    // that rustc has to find each implementation.
    fn doubled<T: AddLogic<Output = T>>(x: T, d: T) -> u32 { 1 }
    fn widened<T: IntoLogic<Rational>>(t: T) -> u32 { 2 }
    fn narrowed<T: IntoLogic<Int>>(t: T) -> u32 { 4 }
    fn sum_of(a: Rational, b: Rational, sum: <Rational as AddLogic>::Output) -> u32 { 8 }

    fn run(n: Nat, i: Int, half: Rational) -> u32 {
        let whole = logic! { twice(half) };
        let from_n = widened(n);          // Nat: IntoLogic<Rational>, through the blanket implementation
        let from_i = widened(i);          // Int: IntoLogic<Rational>
        let same = widened(half);         // the reflexive conversion
        let back = narrowed(half);        // Rational: IntoLogic<Int>
        let up = narrowed(n);             // Nat: IntoLogic<Int>, the library's own conversion
        let sum = logic! { Rational::add(half, half) };
        let as_rational: Rational = logic! { widen(n) };
        doubled(half, whole) * doubled(n, logic! { n + n }) * doubled(i, i)
            + (from_n & from_i & same)
            + (back & up)
            + sum_of(half, half, sum)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost(), ::locus::ghost()));
}
