//! `litmus/logical-layer/branch-on-logical.reject.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May runtime code branch on a logical value?
//~ spec: logic.ghost.no-runtime-control
//~ rfc: 0002

fn describe(v: &Vec<u32>) -> u32 {
    let count = logic! { v.len() };
    if count > 0 { //~ error: runtime code cannot branch on a logical value
        1
    } else {
        0
    }
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
