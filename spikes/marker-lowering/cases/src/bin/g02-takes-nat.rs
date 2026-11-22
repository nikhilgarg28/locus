//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => ("locus::Nat", 1)
//! A ghost local passed to a function that takes a `Nat`: one binding, whose
//! type rustc infers from that use, backward. The name of the type it
//! inferred is printed.
use ::locus::locus;

locus! {
    fn takes_nat(n: Nat) -> u32 { 1 }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(v: &Vec<u32>) -> (&'static str, u32) {
        let before = logic! { v.len() };
        let name = type_of(&before);
        (name, takes_nat(before))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
