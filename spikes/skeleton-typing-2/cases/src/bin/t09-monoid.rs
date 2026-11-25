//! expect: runtime=ok check=ok
//! prints: run() => 6
//! The draft's `Monoid`: a program's trait for the library's type and for a
//! program's logical type, a lemma, a logic function with a default body
//! that one implementation leaves as it is, and generic code over the bound.
use ::locus::locus;

locus! {
    trait Monoid: Logical {
        logic fn unit() -> Self;
        logic fn op(a: Self, b: Self) -> Self;
        logic fn assoc(a: Self, b: Self, c: Self)
            -> @(Self::op(Self::op(a, b), c) === Self::op(a, Self::op(b, c)));
        // A default body, as in Rust.
        logic fn twice(a: Self) -> Self { Self::op(a, a) }
    }

    impl Monoid for Int {
        logic fn unit() -> Int { 0 }
        logic fn op(a: Int, b: Int) -> Int { a + b }
        logic fn assoc(a: Int, b: Int, c: Int) -> @((a + b) + c === a + (b + c)) { _ }
    }

    #[derive(Logical)]
    enum Sign { Plus, Minus }

    impl Monoid for Sign {
        logic fn unit() -> Sign { Sign::Plus }
        logic fn op(a: Sign, b: Sign) -> Sign { if a === b { Sign::Plus } else { Sign::Minus } }
        logic fn assoc(a: Sign, b: Sign, c: Sign)
            -> @(Sign::op(Sign::op(a, b), c) === Sign::op(a, Sign::op(b, c))) { _ }
        logic fn twice(a: Sign) -> Sign { Sign::Plus }
    }

    logic fn fold<M: Monoid>(xs: Seq<M>) -> M {
        if xs.len() == 0 { M::unit() } else { M::op(xs[0], fold(xs.skip(1))) }
    }

    fn thrice<M: Monoid>(m: M) -> (r: M, @(r === M::op(m, M::twice(m)))) {
        let r = logic! { M::op(m, M::twice(m)) };
        let regrouped = logic! { M::assoc(m, m, m) };
        (r, _)
    }

    fn run(i: Int, s: Sign) -> u32 {
        let (three_i, why_i) = thrice(i);
        let (three_s, why_s) = thrice(s);
        let total: Int = logic! { fold(Seq::empty()) + Int::twice(three_i) };
        let sign = logic! { Sign::op(three_s, Sign::unit()) };
        takes_int(total) + takes_sign(sign)
    }

    fn takes_int(i: Int) -> u32 { 2 }
    fn takes_sign(s: Sign) -> u32 { 4 }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost()));
}
