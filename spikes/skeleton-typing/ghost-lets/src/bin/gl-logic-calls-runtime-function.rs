//! `litmus/logical-layer/logic-calls-runtime-function.reject.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May logical code call a runtime function?
//~ spec: logic.code.no-runtime-calls
//~ rfc: 0002

fn count(v: &Vec<u32>) -> usize {
    v.len()
}

fn f(v: Vec<u32>) -> usize {
    let n = logic! { count(&v) }; //~ error: logical code cannot call the runtime function `count`
    v.len()
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
