//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => (15, 15)
//! A ghost local assigned in a loop. In `plain`, only logical code mentions
//! `seen`: the assignment `seen = logic! { .. };` is a ghost statement, and
//! its left side is not a mention by runtime code. In `passed`, runtime code
//! also passes it on, so it is one typed binding, assigned the same way.
use ::locus::locus;

locus! {
    fn takes_nat(n: Nat) -> u32 { 0 }

    fn plain(v: &Vec<u32>) -> u32 {
        let mut seen = logic! { 0 };
        let mut total = 0;
        for x in v {
            total += *x;
            seen = logic! { seen + 1 };
        }
        let all = prove!(seen == v.len());
        total
    }

    fn passed(v: &Vec<u32>) -> u32 {
        let mut seen = logic! { 0 };
        let mut total = 0;
        for x in v {
            total += *x;
            seen = logic! { seen + 1 };
        }
        total + takes_nat(seen)
    }

    fn run(v: &Vec<u32>) -> (u32, u32) { (plain(v), passed(v)) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
