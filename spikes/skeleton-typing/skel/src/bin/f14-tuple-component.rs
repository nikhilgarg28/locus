//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => 3
//! fallback: a tuple component
//! g14 with an initializer that has no skeleton (a cast): a ghost value
//! that is a component of a runtime local.
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> usize {
        let pair = (v.len(), logic! { v.len() as Int });
        let same = prove!(pair.1 == pair.0);
        pair.0
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
