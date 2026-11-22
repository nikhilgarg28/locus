//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Int")
//! `if` on a logical `Bool`, and on a proposition.
use ::locus::locus;

locus! {
    logic fn max(a: Int, b: Int) -> Int { if a < b { b } else { a } }
    logic fn sign(a: Int) -> Int { if a < 0 { -1 } else if a == 0 { 0 } else { 1 } }
    #[derive(Logical)]
    enum Side { Left, Right }
    logic fn flip(s: Side) -> Side { if s === Side::Left { Side::Right } else { Side::Left } }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(x: u32, y: u32, i: Int) -> (&'static str, &'static str) {
        let m = logic! { if x < y { y } else { x } };
        let s = logic! { sign(max(i, 0)) };
        (type_of(&m), type_of(&s))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(1, 2, ::locus::ghost()));
}
