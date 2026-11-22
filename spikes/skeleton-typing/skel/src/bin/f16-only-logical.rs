//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => 3
//! fallback: mentioned by logical code alone
//! A ghost local whose initializer has no skeleton (a cast), and that only
//! logical code mentions: it is left untyped, the logical code that reads
//! it has no skeleton either, and the check after rustc finds nothing.
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> usize {
        let before = logic! { v.len() as Int };
        let same = prove!(before == v.len());
        let hs = vec![prove!(before == v.len())];
        v.len()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
