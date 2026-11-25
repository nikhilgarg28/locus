//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => 2
//! A ghost local captured by a closure, which passes it to runtime code.
//! The closure's body is runtime code, so the local is one typed binding,
//! and rustc infers its type through the capture.
use ::locus::locus;

locus! {
    fn takes_nat(n: Nat) -> u32 { 1 }

    fn run(v: &Vec<u32>) -> u32 {
        let before = logic! { v.len() };
        let pass = move || takes_nat(before);
        pass() + pass()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
