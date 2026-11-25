//! `litmus/observation/logic-in-async-block.reject.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May logical code appear inside an async block?
//~ spec: observe.captures.rejected
//~ rfc: 0001

fn f() -> usize {
    let v = vec![1, 2, 3];
    let pending = async {
        let n = logic! { v.len() }; //~ error: logical code inside an async block is not supported
        v.len()
    };
    drop(pending);
    v.len()
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
