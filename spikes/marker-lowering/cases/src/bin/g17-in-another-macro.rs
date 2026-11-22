//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => (1, 1)
//! A ghost local named inside another macro's input. In `proofs`, only a
//! `prove!` there mentions it. In `values`, the macro's input uses it as a
//! value, so its type is inferred from the vector's.
use ::locus::locus;

locus! {
    fn proofs(v: &Vec<u32>) -> usize {
        let before = logic! { v.len() };
        let hs = vec![prove!(before == v.len())];
        hs.len()
    }

    fn values(v: &Vec<u32>) -> usize {
        let before = logic! { v.len() };
        let ns: Vec<Nat> = vec![before];
        ns.len()
    }

    fn run(v: &Vec<u32>) -> (usize, usize) { (proofs(v), values(v)) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
