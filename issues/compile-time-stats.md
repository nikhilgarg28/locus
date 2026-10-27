# Track compile-time cost: --stats from the first compiler, history later

- **Kind:** task
- **Status:** later

Recorded so that it is not forgotten; nothing to do until the compiler exists.

## From the compiler's first version

`locus check --stats` emits JSON with:
- time per phase: parse, elaborate, proof search, kernel check, erase, generate Rust, rustc
- counters that do not vary between runs: kernel steps, proof sizes, search nodes

Adding this at the start is cheap; adding it to a compiler later is tedious.

## Later, by RFC

- Counters are exact, so CI can fail a PR that grows them by more than some threshold. Timings on shared CI runners vary by 10–20%, so they only show trends.
- Benchmark programs beyond the accepted litmus examples.
- Where history lives (a data branch, CI artifacts, or a service), and charts on the website.

v0 built a benchmark data branch with tracked runner configurations before the compiler was stable. This waits until there is something worth measuring.

## Log

### 2026-10-25, opened on GitHub as #2

Moved into the repository from [GitHub issue #2](https://github.com/nikhilgarg28/locus/issues/2) on 2026-10-27. The text above is the issue's as it was then.
