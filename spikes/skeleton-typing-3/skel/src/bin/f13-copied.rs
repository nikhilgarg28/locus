//! expect: runtime=ok check=ok
//! fallback: a plain copy
//! g13 with an initializer that has no skeleton (a cast): the first mode's
//! guess stands. `let again = before;` is a mention of `before` by runtime
//! code that says nothing of its type.
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> usize {
        let before = logic! { v.len() as Int };
        let again = before;
        let same = prove!(again == before);
        v.len()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
