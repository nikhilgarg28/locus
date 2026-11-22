//! expect: runtime=annotate check=annotate
//! A ghost local mentioned by runtime code, where the tokens do not show
//! it: the name is inside a string literal. By the draft's rule the local
//! is one typed binding, its only use is generic, and the program must
//! write its type.
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> String {
        let before = logic! { v.len() };
        format!("{before:?}")
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
