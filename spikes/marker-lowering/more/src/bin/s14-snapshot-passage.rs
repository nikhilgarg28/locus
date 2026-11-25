//! expect: runtime=ok check=ok
//! prints: run() => (3, 0)
//! A snapshot of a vector passed where a sequence of snapshots is wanted:
//! one logical type written two ways. And a `Vec<Nat>` used as a
//! `Vec<Seq<Int>>`: two logical types. In this mode rustc accepts both, as
//! it does with one marker, and telling them apart is Locus's.
use ::locus::locus;

locus! {
    fn sequence(s: Seq<Snap<u32>>) -> u32 { 3 }
    fn bridge(s: Snap<Vec<u32>>) -> u32 { sequence(s) }

    fn mismatch() -> usize {
        let v: Vec<Nat> = Vec::new();
        let w: Vec<Seq<Int>> = v;
        w.len()
    }

    fn run(v: &Vec<u32>) -> (u32, usize) {
        let s: Snap<Vec<u32>> = logic! { snap(*v) };
        (bridge(s), mismatch())
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&vec![1, 2, 3]));
}
