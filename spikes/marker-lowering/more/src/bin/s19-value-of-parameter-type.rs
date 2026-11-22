//! expect: runtime=ok check=ok
//! prints: run() => 6
//! A ghost value whose type is a type parameter, made by `logic!` in
//! generic code that runs. t09 writes `let r = logic! { .. };` and returns
//! `r` as an `M`: in this mode `logic!` gives the one marker, which is not
//! an `M`. Here the `let` says `: M`, and `locus!`, which sees that `M` is a
//! type parameter, makes the value through `M`'s bound.
use ::locus::locus;

locus! {
    trait Monoid: Logical {
        logic fn unit() -> Self;
        logic fn op(a: Self, b: Self) -> Self;
    }
    impl Monoid for Int {
        logic fn unit() -> Int { 0 }
        logic fn op(a: Int, b: Int) -> Int { a + b }
    }

    fn twice<M: Monoid>(m: M) -> (r: M, @(r === M::op(m, m))) {
        let r: M = logic! { M::op(m, m) };
        (r, _)
    }

    fn takes_int(i: Int) -> u32 { 6 }

    fn run(i: Int) -> u32 {
        let (doubled, why) = twice(i);
        takes_int(doubled)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
