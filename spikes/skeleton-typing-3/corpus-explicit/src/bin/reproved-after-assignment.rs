//! Spike case for question 2: `litmus/proofs/reproved-after-assignment.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May a proposition be proved again about a variable's new version?
//~ spec: proofs.versions
//~ rfc: 0002

fn reproved() -> u8 {
    let mut n: u8 = 3;
    let positive: @(n > 0) = _;
    n = n + 1;
    let again: @(n > 0) = _;
    n
}

//~ run: reproved() => 4

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"reproved()"#, reproved());
}
