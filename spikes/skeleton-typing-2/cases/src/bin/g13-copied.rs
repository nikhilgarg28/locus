//! expect: runtime=annotate check=annotate
//! A ghost local copied to another, and only logical code reads either. No
//! use is generic, and yet rustc can infer nothing: `let again = before;`
//! is a mention by runtime code that says nothing of the type. By the
//! draft's rule the program writes the type.
use ::locus::locus;

locus! {
    fn run(v: &Vec<u32>) -> usize {
        let before = logic! { v.len() };
        let again = before;
        let same = prove!(again == before);
        v.len()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
