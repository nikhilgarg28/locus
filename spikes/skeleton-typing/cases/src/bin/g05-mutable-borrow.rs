//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => 3
//! A mutable borrow of a ghost local, passed to a function that assigns
//! through it.
use ::locus::locus;

locus! {
    fn advance(n: &mut Nat, by: usize) -> usize {
        *n = logic! { *n + by };
        by
    }
    fn takes_nat(n: Nat) -> u32 { 1 }

    fn run(v: &Vec<u32>) -> usize {
        let mut seen = logic! { 0 };
        let mut total = 0;
        for x in v {
            total += advance(&mut seen, 1);
        }
        let all = prove!(seen == v.len());
        total
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
