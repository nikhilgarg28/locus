//! expect: runtime=ok check=ok
//! prints: run() => ["locus::Nat", "locus::Int", "locus::Nat", "locus::Nat", "locus::Int", "locus::Int", "locus::Int"]
//! An integer literal is an `Int`, or a `Nat` where a `Nat` is expected: by
//! a parameter, by an annotation, by the other operand. Where nothing
//! gives it a type, it is an `Int`.
use ::locus::locus;

locus! {
    logic fn takes_nat(n: Nat) -> Nat { n }
    logic fn takes_int(i: Int) -> Int { i }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(v: &Vec<u32>, i: Int) -> Vec<&'static str> {
        let a = logic! { takes_nat(3) };
        let b = logic! { takes_int(3) };
        let c: Nat = logic! { 3 };
        let d = logic! { v.len() + 1 };
        let e = logic! { 1 + i };
        let f = logic! { 2 + 3 };
        let g = logic! { -3 };
        vec![type_of(&a), type_of(&b), type_of(&c), type_of(&d), type_of(&e), type_of(&f), type_of(&g)]
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&vec![1], ::locus::ghost()));
}
