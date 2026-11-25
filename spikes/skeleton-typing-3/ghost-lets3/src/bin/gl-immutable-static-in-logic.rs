//! `litmus/logical-layer/immutable-static-in-logic.accept.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May logical code read an immutable static?
//~ spec: logic.code.statics
//~ rfc: 0002

static LIMIT: u32 = 100;

fn limit() -> u32 {
    let a = logic! { LIMIT };
    let b = logic! { LIMIT };
    let same: @(a === b) = _;
    LIMIT
}

//~ run: limit() => 100

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
