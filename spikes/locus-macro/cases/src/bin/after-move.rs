//! Spike case for question 2: `litmus/observation/after-move.reject.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May logical code read a vector after the vector has moved?
//~ spec: observe.place.readable
//~ rfc: 0001

fn consume(v: Vec<u32>) -> usize {
    v.len()
}

fn f() -> usize {
    let v = vec![1, 2, 3];
    let count = consume(v);
    let n = logic! { v.len() }; //~ error: logical code reads `v` after `v` was moved
    count
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
