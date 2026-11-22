//! expect: runtime=reject check=reject
//! fallback: a format string
//! g15 with an initializer that has no skeleton (a cast): the name is
//! inside a string literal, `locus!` sees no mention, and the local is
//! left untyped. rustc accepts. The check after rustc must not.
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> String {
        let before = logic! { v.len() as Int };
        format!("{before:?}")
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
