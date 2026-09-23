# Locus

*Review the guarantees, check the implementation, and use the result from ordinary Rust.*

Locus is a Rust-like language in which propositions and proofs are ordinary parts of a program. Explicit proof evidence is checked by a small kernel, proofs are erased, and the rest is emitted as plain Rust. The transition of a lock that tolerates three wrong codes, from `tests/corpus/target/lock.lc`:

```rust
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
```

`@P` is the type of evidence for a claim and `prove!(F)` states one where it stands; the `+` under `no_panic` carries the obligation that it does not overflow, which the arithmetic procedure discharges from `lock.failures < 3`. The file's `run` loops over `step` with the bound as tracked evidence, and a Rust caller of the generated crate can call `run` and never `step`, which takes evidence.

```sh
cargo run -- check tests/corpus/target/lock.lc --holes    # types and proofs, and how each obligation was filled
cargo run -- run tests/corpus/target/lock.lc run 300 9    # (Lock { failures: 3, open: false }, Erased)
cargo run -- rust tests/corpus/target/lock.lc             # the generated Rust
cargo run -- build tests/corpus/target/lock.lc tests/corpus/target/percent.lc --out /tmp/lock_crate
tools/check.sh                                            # fmt, clippy, and the fast tests
tools/check.sh --extended                                 # the long runs, and a line per exit criterion of the build
```

Requires Rust 1.91 or newer (the compiler is validated with 1.91.1); the full development gate also needs Python 3 and Node.js. `cargo run --release -- bench` records a benchmark baseline; the fast gate requires recent history. **Everything else is in [atlas.html](atlas.html).** Open it in a browser: it holds the overview, the language as built, the architecture, the kernel contract, the target language and the design notes, a table of what is built and what is left, and the projects and tasks, and it is where they are edited. `python3 tools/atlas.py serve` opens it from a local address, where it saves itself as changes are made; the same tool reads and writes the documents from the command line.
