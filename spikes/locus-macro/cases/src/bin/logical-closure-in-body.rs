//! Spike case for question 2: `litmus/observation/logical-closure-in-body.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May a runtime body contain a logical closure that reads a runtime value?
//~ spec: observe.captures.rejected, observe.place.readable
//~ rfc: 0001

fn commutes(x: u32) -> u32 {
    let all: @(forall |n: Int| n + x === x + n) = logic! { |n: Int| prove!(n + x === x + n) };
    x
}

//~ run: commutes(3) => 3

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"commutes(3)"#, commutes(3));
}
