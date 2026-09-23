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
This complete example maintains a lock’s failure count and returns evidence that the count stays within its limit. It brings together named propositions, evidence in function results, mutation and loop state; the source below is checked and executed by the documentation tests.

## The 32-bit lock

<!-- spec: 1.1:1 legality-rule -->
`tests/corpus/target/lock.lc` is the target example: named proposition arms, explicit evidence, tracked loop evidence, and checked machine arithmetic. Its program text is kept identical to the Vision target; run directives belong to the corpus harness.

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

<!-- spec: 1.1:3 legality-rule -->
`locus run tests/corpus/target/lock.lc run 300 9` returns `(Lock { failures: 3, open: false }, Erased)`. The evidence-taking functions are restricted exports; Rust callers use the public data interface. Proof counts and timing are measurements of the implementation, not language semantics.
