//! `litmus/observation/during-mutable-borrow.reject.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May logical code read a vector while it is mutably borrowed?
//~ spec: observe.place.readable
//~ rfc: 0001

fn f() -> usize {
    let mut v = vec![1, 2, 3];
    let r = &mut v;
    let n = logic! { v.len() }; //~ error: logical code reads `v` while `v` is mutably borrowed
    r.push(4);
    v.len()
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
