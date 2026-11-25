//! `litmus/observation/logic-in-closure.reject.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May logical code appear inside a closure body?
//~ spec: observe.captures.rejected
//~ rfc: 0001

fn f() -> usize {
    let v = vec![1, 2, 3];
    let count = || {
        let n = logic! { v.len() }; //~ error: logical code inside a closure is not supported
        v.len()
    };
    count()
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
