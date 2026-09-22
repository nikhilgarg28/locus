# Locus

*Review the guarantees, check the implementation, and use the result from ordinary Rust.*

Locus is a Rust-like language in which propositions and proofs are ordinary parts of a program. Explicit proof evidence is checked by a small kernel, proofs are erased, and the rest is emitted as plain Rust. The transition of a lock that tolerates three wrong codes, from `tests/corpus/target/lock.lc`:

```rust
#[terminates] #[no_panic] #[no_io]
fn within_limit(failures: u32) -> Prop {
    prop!(failures <= 3)
}

#[terminates] #[no_panic] #[no_io]
pub(crate) fn step(
    lock: Lock,
    bounded: @within_limit(lock.failures),
    event: Event,
) -> (next: Lock, @within_limit(next.failures)) {
    match event {
        Event::Right => {
            let next = Lock { failures: 0, open: true };
            (next, fold!(within_limit, prove!(next.failures <= 3)))
        }
        Event::Wrong => {
            if lock.failures < 3 {
                let fits = prove!(lock.failures as Int + 1 <= u32::MAX as Int);
                let next = Lock { failures: lock.failures + 1, open: false };
                (next, fold!(within_limit, prove!(next.failures <= 3)))
            } else {
                (Lock { failures: lock.failures, open: false }, bounded)
            }
        }
    }
}
```

`@P` is the type of evidence for a claim and `prove!(F)` states one where it stands; the `+` under `no_panic` carries the obligation that it does not overflow, which the arithmetic procedure discharges from `lock.failures < 3`. The file's `run` loops over `step` with the bound as tracked evidence, and a Rust caller of the generated crate can call `run` and never `step`, which takes evidence.

```sh
cargo run -- check tests/corpus/target/lock.lc --holes    # types and proofs, and how each obligation was filled
cargo run -- run tests/corpus/target/lock.lc run 300 9    # (Lock { failures: 3, open: false }, Proved)
cargo run -- rust tests/corpus/target/lock.lc             # the generated Rust
cargo run -- build tests/corpus/target/lock.lc tests/corpus/target/percent.lc --out /tmp/lock_crate
tools/check.sh                                            # fmt, clippy, and the fast tests
tools/check.sh --extended                                 # the long runs, and a line per exit criterion of the build
```

Requires Rust 1.85 or newer. **Everything else is in [atlas.html](atlas.html).** Open it in a browser: it holds the overview, the language as built, the architecture, the kernel contract, the target language and the design notes, a table of what is built and what is left, and the projects and tasks, and it is where they are edited. `python3 tools/atlas.py serve` opens it from a local address, where it saves itself as changes are made; the same tool reads and writes the documents from the command line.
