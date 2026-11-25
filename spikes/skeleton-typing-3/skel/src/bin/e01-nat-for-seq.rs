//! expect: runtime=reject check=reject
//! A deliberate type error in logical code: a `Nat` where a `Seq` is
//! wanted, once as a place and once as a call.
use ::locus::locus;

locus! {
    logic fn count(xs: Seq<Nat>) -> Nat { xs.len() }

    fn run(v: &Vec<u32>) -> usize {
        let n = logic! { v.len() };
        let from_a_place = logic! { count(n) };
        let from_a_call = logic! { count(v.len()) };
        v.len()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&vec![1]));
}
