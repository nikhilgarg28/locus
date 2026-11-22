//! expect: runtime=reject check=reject
//! A `logic!` where runtime code wants a `usize`. Locus rejects it, and so
//! does rustc; what is recorded here is how rustc now says it. With one
//! marker it was E0308, "expected `usize`, found `LogicalData`".
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> usize {
        logic! { v.len() }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
