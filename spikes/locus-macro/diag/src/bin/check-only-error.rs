//! Question 3: an error that only the checking expansion has, logical code
//! reading a moved vector (the decided `observation/after-move.reject.lc`).
use locus::locus;

locus! {
    fn consume(v: Vec<u32>) -> usize {
        v.len()
    }

    fn f() -> usize {
        let v = vec![1, 2, 3];
        let count = consume(v);
        let n = logic! { v.len() };
        count
    }
}

fn main() {
    println!("{}", f());
}
