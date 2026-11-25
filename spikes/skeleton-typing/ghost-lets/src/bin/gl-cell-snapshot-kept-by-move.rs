//! `litmus/logical-layer/cell-snapshot-kept-by-move.accept.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: Does moving a `Cell` keep its snapshot, opaque contents included?
//~ spec: logic.eq.snapshots, models.frozen-data
//~ rfc: 0002

use std::cell::Cell;

fn moved() -> u32 {
    let c = Cell::new(1u32);
    let before = logic! { snap(c) };
    let d = c;
    let kept: @(snap(d) === before) = _;
    d.get()
}

//~ run: moved() => 1

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
