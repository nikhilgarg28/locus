//! Spike case for question 2: `litmus/logical-layer/logic-calls-logic-function.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May logical code call a logic function on a runtime value's model?
//~ spec: logic.code.no-runtime-calls
//~ rfc: 0002

logic fn double(n: Nat) -> Nat {
    n + n
}

fn f(v: Vec<u32>) -> usize {
    let twice = logic! { double(v.len()) };
    v.len()
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
