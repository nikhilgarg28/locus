//! expect: runtime=ok check=ok
//! prints: run() => (2, 3)
//! `Vec<Nat>: CloneEq` through one adapter of the library,
//! `impl<T: CloneEq> CloneEq for Vec<T>`, and the one implementation for the
//! marker. `CloneEq` has a method that runs: it is the library's exception.
use ::locus::locus;

locus! {
    fn dup<T: CloneEq>(x: &T) -> T {
        let (copy, same) = x.clone_eq();
        copy
    }

    fn run(n: Nat, i: Int) -> (usize, usize) {
        let nats: Vec<Nat> = vec![n, n];
        let pairs: Vec<Vec<Int>> = vec![vec![i], vec![], vec![i, i]];
        (dup(&nats).len(), dup(&pairs).len())
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost()));
}
