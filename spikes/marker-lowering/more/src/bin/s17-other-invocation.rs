//! expect: runtime=ok check=ok
//! prints: run() => 1
//! ask: bound Rational: OrderSpec => holds
//! A logical type declared in one invocation of `locus!`, and an `impl` for
//! it in another. The second invocation sees only the name `Rational`, an
//! alias of the marker, and cannot know to write the stub.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }
}

locus! {
    impl OrderSpec for Rational {
        logic fn le(a: Rational, b: Rational) -> Bool { a.num * b.den <= b.num * a.den }
    }

    fn show<T: OrderSpec>(x: T) -> u32 { 1 }
    fn run(r: Rational) -> u32 { show(r) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
