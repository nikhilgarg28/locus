//! `litmus/proofs/hole-written-out-with-an-earlier-value.accept.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May a proof cite a fact about a value that the program can no longer spell, by naming that value in a ghost `let` where it was current?
//~ spec: proofs.procedures.expand, proofs.given
//~ rfc: 0005

// What the hole in `(n, _)` expands to. The fact after `n += 1` is about the value
// `n` had before it, which `before` names.
fn increment(mut n: u32) -> (out: u32, @(out === u32::wrap(n + 1))) {
    let before = logic! { n };
    n += 1;
    (n, given(n === u32::wrap(before + 1)))
}

//~ run: increment(4).0 => 5

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
