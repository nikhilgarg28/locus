//! `litmus/contracts/final-value-recovered-by-inference.reject.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May a ghost value be computed from the value a borrow will end with, when inference recovers that value from a proof's type?
//~ spec: contracts.final.pending, proofs.values.inferred
//~ rfc: 0005

struct Tally {
    hits: u32,
    count: Nat,
}

fn lend(t: &mut Tally) -> (r: &mut Tally, @(final!(r).count === final!(r).count)) {
    (t, _)
}

logic fn recover(n: Nat, same: @(n === n)) -> Nat {
    n
}

// If this were accepted, `r.count` would end as one more than itself.
fn absurd(t: &mut Tally) {
    let (r, about_the_end) = lend(t);
    let predicted = logic! { recover(_, about_the_end) }; //~ error: this value would be the count that `r` ends with, and `r`'s borrow has not ended: until it does, that value occurs only inside propositions and proofs
    r.count = logic! { predicted + 1 };
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
