//! Spike case for question 2: `litmus/logical-layer/ghost-parameters.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Does a function with a proof parameter run as if the parameter were absent?
//~ spec: logic.ghost.erased
//~ rfc: 0002

fn halve(n: u32, even: @(n % 2 == 0)) -> u32 {
    n / 2
}

fn ten_halved() -> u32 {
    halve(10, _)
}

//~ run: ten_halved() => 5

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"ten_halved()"#, ten_halved());
}
