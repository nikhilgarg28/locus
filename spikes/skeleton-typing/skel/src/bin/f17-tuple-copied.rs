//! expect: runtime=reject check=reject
//! fallback: a tuple that holds an untyped component, copied whole
//! f14, and then runtime code copies the tuple: the copy holds a ghost
//! value that has no logical type for rustc.
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> usize {
        let pair = (v.len(), logic! { v.len() as Int });
        let same = prove!(pair.1 == pair.0);
        let copy = pair;
        copy.0
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
