//! `litmus/models/snapshot-unchanged.accept.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: Are two snapshots of a value that no write separates equal?
//~ spec: logic.eq.snapshots, models.snapshots
//~ rfc: 0002

struct Token {
    id: u32,
}

fn unchanged(t: &Token) -> u32 {
    let first = logic! { snap(t) };
    let second = logic! { snap(t) };
    let same: @(first === second) = _;
    t.id
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
