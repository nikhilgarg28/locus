//! Spike case for question 2: `litmus/compatibility/raw-identifier-logic.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May `logic` and `prop` be used as names, raw or not, even before a call?
//~ spec: compat.syntax.contextual-keywords
//~ rfc: 0001

fn r#logic(prop: u32) -> u32 {
    prop + 1
}

fn call() -> u32 {
    logic(1) + r#logic(2)
}

//~ run: call() => 5

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"call()"#, call());
}
