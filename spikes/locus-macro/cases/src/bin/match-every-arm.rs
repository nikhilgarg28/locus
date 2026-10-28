//! Spike case for question 2: `litmus/proofs/match-every-arm.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Does a match on a proof of a declared proposition give each arm's witnesses and condition?
//~ spec: proofs.props.matching
//~ rfc: 0002

prop Even(n: Nat) {
    Zero => { n == 0 }
    Step(m: Nat) => { n == m + 2 && Even(m) }
}

logic fn one_is_not_even(h: @Even(1)) -> @False {
    match h {
        Even::Zero @ one_is_zero => _,
        Even::Step(m) @ And::Intro(one_is_step, _) => _,
    }
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
