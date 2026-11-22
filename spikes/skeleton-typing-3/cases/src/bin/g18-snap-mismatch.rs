//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => 1
//! The cost of one marker for every snapshot: a snapshot of a `Vec<u32>`
//! passed where a `Snap<bool>` is wanted. Locus rejects this program by its
//! own typing, as it rejects a proof of the wrong proposition, and this
//! spike has no such check. What is shown is that rustc cannot reject it:
//! both types are the one marker. (Compare d03, which rustc now rejects.)
use ::locus::locus;

locus! {
    fn wants_flag(flag: Snap<bool>) -> u32 { 1 }

    fn run(v: &Vec<u32>) -> u32 {
        let before: Snap<Vec<u32>> = logic! { snap(*v) };
        wants_flag(before)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
