//! `litmus/observation/after-mutable-borrow-ends.accept.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May logical code read a vector once a mutable borrow of it has ended?
//~ spec: observe.place.readable, models.observation
//~ rfc: 0001

fn f() -> usize {
    let mut v = vec![1, 2, 3];
    let r = &mut v;
    r.push(4);
    let n = logic! { v.len() };
    v.len()
}

//~ run: f() => 4

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
