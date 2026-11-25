//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Int", "x03_nat_as_int_place::Rational")
//! "A `Nat` is used as an `Int` wherever an `Int` is expected" (RFC 0002):
//! a `Nat` place as the argument of an `Int` parameter, and as the
//! initializer of an `Int` field.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }
    logic fn negate(i: Int) -> Int { 0 - i }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(n: Nat) -> (&'static str, &'static str) {
        let i = logic! { negate(n) };
        let r = logic! { Rational { num: n, den: 1 } };
        (type_of(&i), type_of(&r))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
