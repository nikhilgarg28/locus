//! Spike case for question 2: `litmus/compatibility/pub-locus-as-tuple-field-type.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

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
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"wrap()"#, wrap());
}
