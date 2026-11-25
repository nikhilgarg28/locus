//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => 3
//! g06 with the type written.
use ::locus::locus;

locus! {
    fn keep<G: Logical>(g: G) -> G { g }

    fn run(v: &Vec<u32>) -> usize {
        let before: Nat = logic! { v.len() };
        let kept = keep(before);
        v.len()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
