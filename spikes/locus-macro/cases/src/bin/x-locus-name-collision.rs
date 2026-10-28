//! Spike case for question 2, not generated: `pub-locus-as-tuple-field-type.rs`
//! as first written, with `use locus::locus;`. The example declares its own
//! `pub struct locus;`, inside the `locus!` block, so the crate's name and the
//! struct collide (E0659).
use locus::locus;

locus! {
//~ question: Does `pub (locus)` keep its Rust meaning as a tuple field whose type is named `locus`?
//~ spec: compat.syntax.not-rust
//~ rfc: 0001

#[allow(non_camel_case_types)]
pub struct locus;

#[allow(unused_parens)]
pub struct Wrapper(pub (locus));

pub fn wrap() -> u32 {
    let _w = Wrapper(locus);
    1
}

//~ run: wrap() => 1

}

fn main() {
    println!("expansion: {}", locus::__expansion!());
    println!("{} => {:?}", r#"wrap()"#, wrap());
}
