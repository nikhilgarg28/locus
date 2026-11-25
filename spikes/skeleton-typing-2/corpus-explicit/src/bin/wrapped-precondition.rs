//! Spike case for question 2: `litmus/export/wrapped-precondition.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May an exported crate keep a function with a precondition `pub(locus)` and export a wrapper that checks it?
//~ spec: export.plain-interface, export.locus-visibility
//~ rfc: 0002
//~ mode: export

pub(locus) fn halve_even(n: u32, even: @(n % 2 == 0)) -> u32 {
    n / 2
}

pub fn halve(n: u32) -> Option<u32> {
    if n % 2 == 0 {
        Some(halve_even(n, _))
    } else {
        None
    }
}

//~ run: halve(10) => Some(5)
//~ run: halve(7) => None

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"halve(10)"#, halve(10));
    println!("{} => {:?}", r#"halve(7)"#, halve(7));
}
