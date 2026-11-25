//! Spike case for question 2: `litmus/compatibility/locus-macro-inside-rust-macro.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May a Locus macro be invoked inside another macro's input?
//~ spec: compat.names.macros-expand
//~ rfc: 0001

fn check(x: u32) -> usize {
    let proofs: Vec<@(x == x)> = vec![prove!(x == x)];
    proofs.len()
}

//~ run: check(3) => 1

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"check(3)"#, check(3));
}
