//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Int", "locus::Int", "a03_struct::Pair<a03_struct::Rational>", "locus::Int", 0)
//! Field access and construction of `#[derive(Logical)]` structs: on a
//! ghost local, on the result of a call, through a generic struct. The
//! structs have their fields, and are zero-sized.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }
    #[derive(Logical)]
    struct Pair<T> { first: T, second: T }

    logic fn scale(r: Rational, k: Int) -> Rational { Rational { num: r.num * k, den: r.den } }
    logic fn swap<T>(p: Pair<T>) -> Pair<T> { Pair { first: p.second, second: p.first } }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(r: Rational) -> (&'static str, &'static str, &'static str, &'static str, usize) {
        let n = logic! { r.num };
        let s = logic! { scale(r, 2).den };
        let p = logic! { Pair { first: r, second: scale(r, 3) } };
        let q = logic! { swap(p).first.num };
        (type_of(&n), type_of(&s), type_of(&p), type_of(&q), std::mem::size_of_val(&p))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
