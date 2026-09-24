+++
id = "language-walkthrough"
title = "A verified lock"
group = "Now"
spec_chapter = 1
order = 115
route = "specification/walkthrough.html"
description = "A complete example that carries an invariant through branches, calls, and a loop."
+++

# A verified lock

<!-- spec: 1.0:17 informative -->
This example verifies one precise property: a lock’s failure count never exceeds three. It does not prove a security policy or that the lock eventually opens. The runtime code processes events; its evidence records the bound through transitions and a loop.

## The contract

<!-- spec: 1.1:1 legality-rule -->
The [lock program](../../tests/corpus/target/lock.lc) exercises named proposition arms, proof-bearing results, tracked loop evidence, and safe machine arithmetic. Its source is checked by the acceptance suite; the complete program below is also checked and executed by documentation tests.

<!-- spec: 1.91:37 informative -->
`within_limit` names the bound. `step` requires evidence for its input lock and returns a new lock with evidence for the new count. A correct event resets the count; an incorrect event increments below the limit and otherwise leaves it unchanged.

## The complete program

<!-- spec: 1.1:2 example -->
~~~locus run
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
//~ run: run(3, 2) => (Lock { failures: 0, open: true }, Erased)
~~~

## Why each part checks

<!-- spec: 1.91:38 informative -->
1. **Construction:** the initial count is zero, so its bound is immediate.
2. **Transition:** the branch `failures < 3` establishes room for the increment; `fits` states the arithmetic step explicitly.
3. **Propagation:** destructuring `step` binds `still` as evidence about the newly returned `next`.
4. **Mutation:** assigning `lock` invalidates `ok`; assigning `still` refreshes it before the loop’s next iteration.
5. **Consumption:** `remaining` opens the named proof and uses the bound to justify subtraction.

<!-- spec: 1.1:3 legality-rule -->
Running `run(300, 9)` returns `(Lock { failures: 3, open: false }, Erased)`. Evidence-taking helpers have restricted visibility; Rust callers use the public runtime interface. The loop’s result is proved on normal return, with no current termination certificate. Proof counts and timings are implementation measurements, not language semantics.
