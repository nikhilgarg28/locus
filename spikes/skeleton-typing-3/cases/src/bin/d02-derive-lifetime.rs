//! expect: runtime=error check=error
//! A logical type declares no lifetime parameter: no marker carries one.
//! `locus!` rejects it.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct View<'a, T> { items: Seq<T>, from: Snap<&'a u32> }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
