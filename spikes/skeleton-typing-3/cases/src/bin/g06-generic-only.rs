//! expect: runtime=annotate check=annotate
//! A ghost local passed only to a generic function: rustc has nothing to
//! infer its type from, and the program must write it (g07).
use ::locus::locus;

locus! {
    fn keep<G: Logical>(g: G) -> G { g }

    fn run(v: &Vec<u32>) -> usize {
        let before = logic! { v.len() };
        let kept = keep(before);
        v.len()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
