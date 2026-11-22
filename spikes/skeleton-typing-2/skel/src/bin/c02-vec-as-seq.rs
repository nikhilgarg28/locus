//! expect: runtime=ok check=ok
//! prints: run() => (1, 2, 3, 4)
//! `Snap<Vec<u32>>` where a `Seq<Snap<u32>>` is wanted: rustc normalizes
//! both to one type. The same through a type parameter, through a slice
//! behind a reference, and for a struct of the program over a reference.
use ::locus::locus;

locus! {
    struct Pair<T> { a: T, b: T }
    fn sequence(s: Seq<Snap<u32>>) -> u32 { 1 }
    fn bridge(s: Snap<Vec<u32>>) -> u32 { sequence(s) }
    fn generic<T: Snapshot>(s: Snap<Vec<T>>) -> (Seq<Snap<T>>, u32) { (s, 2) }
    fn through_ref<'a, T: Snapshot>(s: Snap<&'a [T]>) -> (Seq<Snap<T>>, u32) { (s, 3) }
    fn pair_of<T: Snapshot>(s: Snap<Pair<&T>>) -> (Snap<Pair<T>>, u32) { (s, 4) }

    fn run(v: &Vec<u32>, p: Pair<&u32>) -> (u32, u32, u32, u32) {
        let s = logic! { snap(*v) };
        let q = logic! { snap(p) };
        (bridge(s), generic::<u32>(s).1, through_ref::<u32>(s).1, pair_of::<u32>(q).1)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&vec![1], Pair { a: &1, b: &2 }));
}
