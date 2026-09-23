+++
id = "target-examples"
title = "Target examples"
group = "Vision"
created = "2026-09-21T21:22:55.000Z"
updated = "2026-09-22T23:44:20.000Z"
route = "vision/target-examples.html"
order = 7
+++

# Target examples

Programs written by hand in the target syntax, before any of it is built, to test the spelling and to count what their proofs need. Nothing here runs yet. These examples use the 22 September type split: executable conditions use bool; evidence states claims over explicit Int models. Code that unpacks proof constructors is logical. Runtime promise attributes do not make ordinary helpers callable in logic. Each obligation is marked with what would discharge it:

- **exact**: the claim is a fact in scope as written;
- **computed**: it is, after computing: names bound by let replaced by what they stand for, projections of written values, matches on written constructors, and arithmetic on literals;
- **arithmetic**: it needs reasoning about order and sums, which means either the arithmetic procedure or a chain of lemma calls.

The findings are at the end.

## The lock, with 32-bit counts

~~~
#[derive(Clone, Copy)]
pub enum Event { Wrong, Right }

#[derive(Clone, Copy)]
pub struct Lock { failures: u32, open: bool }

prop within_limit(failures: Int) {
    Bounds => {
        prop!(failures <= 3)
    }
}

#[terminates] #[no_panic] #[no_io]
pub(super) fn step(
    lock: Lock,
    bounded: @within_limit(lock.failures as Int),
    event: Event,
) -> (next: Lock, @within_limit(next.failures as Int)) {
    match event {
        Event::Right => {
            let next = Lock { failures: 0, open: true };
            (next, within_limit::Bounds @ prove!((next.failures as Int) <= 3))             // computed: 0 <= 3
        }
        Event::Wrong => {
            if lock.failures < 3 {
                let fits = prove!(lock.failures as Int + 1 <= u32::MAX as Int); // arithmetic, from lock.failures < 3
                let next = Lock { failures: lock.failures + 1, open: false };   // the obligation of + is fits: exact
                (next, within_limit::Bounds @ prove!((next.failures as Int) <= 3))         // arithmetic
            } else {
                (Lock { failures: lock.failures, open: false }, bounded)        // computed: the field is lock.failures
            }
        }
    }
}

#[terminates] #[no_panic] #[no_io]
fn event_at(attempt: u32, correct: u32) -> Event {
    if attempt == correct { Event::Right } else { Event::Wrong }
}

// This loop has no termination proof in the initial implementation.
// run is an ordinary fn and is never called inside a proposition.
#[no_panic] #[no_io]
pub fn run(attempts: u32, correct: u32) -> (last: Lock, @within_limit(last.failures as Int)) {
    let mut lock = Lock { failures: 0, open: false };
    let mut ok: @within_limit(lock.failures as Int) = within_limit::Bounds @ prove!((lock.failures as Int) <= 3); // computed
    for attempt in 0..attempts {
        let (next, still) = step(lock, ok, event_at(attempt, correct));
        lock = next;        // ok is now invalid
        ok = still;         // exact, given that still is typed over next
    }
    (lock, ok)
}

#[terminates] #[no_panic] #[no_io]
pub(super) fn remaining(failures: u32, bounded: @within_limit(failures as Int)) -> (left: u32, @((left as Int) <= 3)) {
    let small = logic {
        let within_limit::Bounds @ small = bounded;
        small
    };                                                                        // model(failures) <= 3
    let left = 3 - failures;                                                    // the obligation of -: arithmetic, from small
    (left, prove!((left as Int) <= 3))                                                   // arithmetic
}
~~~

## A midpoint that cannot overflow

The specification is stated over Int, where the sum of two u32 cannot overflow; the code must avoid the overflow the specification is free of.

~~~
#[terminates] #[no_panic] #[no_io]
pub(super) fn midpoint(lo: u32, hi: u32, ordered: @((lo as Int) <= (hi as Int)))
    -> (mid: u32, @(mid as Int == (lo as Int + hi as Int) / 2))
{
    let half = (hi - lo) / 2;          // the obligation of -: arithmetic, from ordered. 2 is not zero: computed
    let mid = lo + half;               // the obligation of +: arithmetic
    (mid, prove!(mid as Int == (lo as Int + hi as Int) / 2))   // arithmetic, with division by a literal
}
~~~

Writing lo + hi in the specification is rejected because it selects a runtime operation; writing (lo + hi) / 2 in the code is rejected under no_panic unless the sum can be shown to fit, which it cannot. The language makes the well-known bug hard to write on either side.

## A protected type, and a way in from Rust

~~~
pub struct Percent { value: u32, in_range: @((value as Int) <= 100) }      // not Copy: it moves

impl Percent {
    // Callable from Locus, where evidence exists.
    #[terminates] #[no_panic] #[no_io]
    pub(super) fn new(value: u32, in_range: @((value as Int) <= 100)) -> Percent {
        Percent { value, in_range }
    }

    // Callable from Rust: the check happens at runtime, and the evidence comes from the branch.
    #[terminates] #[no_panic] #[no_io]
    pub fn checked(value: u32) -> Option<Percent> {
        if value <= 100 { Some(Percent::new(value, prove!((value as Int) <= 100))) } else { None }   // exact
    }
}
~~~

new is visible to other Locus modules and is not exported to Rust. Being unable to construct evidence would not stop Rust from calling it: every proof erases to the same marker, and one obtained honestly from another function could be passed here. Rust cannot build a Percent directly either, because the fields are private. It calls checked. This example needs impl blocks and Option, which are later stages.

## Findings

1. Exact evidence has to mean exact after computing. With no computation at all, not even 0 <= 3 for a struct that was just written can be shown, since the surface has no forms for the kernel's projection and literal axioms and was never meant to. This matches the principle already recorded under Principles: computation steps are inserted silently, and a function's definition is unfolded only on request. What is dropped is search: using one fact to reach another, and unfolding without being asked.
2. Destructuring has to type each part over the names the pattern binds. In let (next, still) = step(...), still must have the type @within_limit(next.failures as Int), not a type about step(...).0. This is how a dependent pattern is opened, a fixed step and not a search, and without it evidence that comes out of a call can never be used.
3. Arithmetic is where the proofs are. Of the fourteen obligations above, three are exact, four are computed, and seven are arithmetic. Every one of the seven is linear. By hand each is three to five lemma calls, most of them moving between a comparison of u32 and a comparison of Int. The arithmetic procedure is what makes the language usable, and belongs early in the sequence.
4. A logical loop needs a checked termination argument; until that feature lands, reject it. Ordinary loops may diverge unless a supported termination proof is supplied. An ordinary fn remains unavailable inside logic even when termination is proved. A finite-range termination rule is useful independently of source function mode.
5. A predicate states its logical intent through prop and needs no promise attributes on its arms; within_limit now follows Predicates with named arms in the Target language. Three attributes on each executable helper are still heavy. A file-level default would help, but there is no way to opt out of one, so a file with one loop cannot use the termination default. This remains an ergonomic question for function promises, not for predicate declarations.
6. as Int is an explicit model observation, not runtime conversion. Proof conditions use these models, while executable if conditions retain machine comparison. The primitive specifications connect branch outcomes to their logical facts.

To append under Findings in Target examples (Vision), as finding 7:

7. The counts of finding 3 are not the compiler's. `locus check --stats` classifies the obligations of the lock as 11: 1 exact, 2 computed, 2 by evaluation, 6 by arithmetic; the midpoint as 6: 1 by evaluation, 5 by arithmetic; and Percent as 3, all computed. Three things account for the difference from three exact, four computed, and seven arithmetic. `0 <= 3` and `2 != 0` on literals are decided by evaluation, which the prediction folds into computed. An overflow row of the table has two premises, `min <= e` and `e <= max`, so each `+` and `-` under no_panic is two obligations where the prediction counts one, and the lower bound of a `u32` sum is a second arithmetic obligation at each of the four operators. And `ok = still` and `(next, bounded)` are typed assignments and values, checked by the kernel's comparison of types and never obligations, so the exact cases the prediction counted are no obligations at all: the one exact obligation is `fits` discharging the upper bound of `lock.failures + 1`. The counts are pinned by `the_proofs_are_the_ones_predicted` in tests/acceptance.rs.

8. With the default model rule of 22 September, every as Int in these examples on a machine operand inside a logical context may be omitted: @within_limit(lock.failures) and prove!(left <= 3) mean what the explicit spellings mean. The examples keep the explicit form so that both are exercised when they become the acceptance tests of the reconciliation.
