//! `litmus/observation/before-move.accept.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May logical code read a vector before the vector is moved?
//~ spec: observe.place.readable, models.observation
//~ rfc: 0001

fn consume(v: Vec<u32>) -> usize {
    v.len()
}

fn f() -> usize {
    let v = vec![1, 2, 3];
    let n = logic! { v.len() };
    consume(v)
}

//~ run: f() => 3

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
