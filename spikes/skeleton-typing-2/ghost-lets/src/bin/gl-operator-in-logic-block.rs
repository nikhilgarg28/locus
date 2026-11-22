//! `litmus/logical-layer/operator-in-logic-block.accept.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May a `logic!` block compute with a ghost value that the source does not show to be ghost?
//~ spec: logic.ghost.shown-by-source
//~ rfc: 0002

use locus::std::vec::Vec;

fn next(v: &Vec<Nat>) -> usize {
    let k = v[0];
    let m = logic! { k + 1 };
    v.len()
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
