//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => 3
//! A ghost value that is a component of a runtime local. `pair` is not a
//! ghost local, so the draft's rule for ghost locals does not speak of it;
//! today's lowering accepts this program, and RFC 0002 asks for no
//! annotation.
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> usize {
        let pair = (v.len(), logic! { v.len() });
        let same = prove!(pair.1 == pair.0);
        pair.0
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
