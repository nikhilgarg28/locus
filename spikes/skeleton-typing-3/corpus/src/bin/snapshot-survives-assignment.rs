//! Spike case for question 2: `litmus/proofs/snapshot-survives-assignment.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Does a proof about a snapshot stay valid after the variable it came from is assigned?
//~ spec: proofs.versions
//~ rfc: 0002

fn snapshot() -> u8 {
    let mut n: u8 = 3;
    let before = logic! { n };
    let positive: @(before > 0) = _;
    n = 0;
    let still: @(before > 0) = positive;
    n
}

//~ run: snapshot() => 0

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"snapshot()"#, snapshot());
}
