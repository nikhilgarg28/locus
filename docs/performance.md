+++
id = "performance"
title = "Performance"
group = "Now"
route = "performance.html"
order = 53
+++

# Measure the compiler. Keep the receipts.

Performance should be a reproducible record of work, not a number detached from its inputs. This page is built from immutable benchmark records on the repository's `locus-bench-data` branch.

<!-- component: performance -->

## What is measured

`locus bench` records parsing, frontend elaboration, lowering, IR checking, and erasure, with additional per-obligation search and proof measurements. Each workload is checked cold, then replayed with its newly produced certificates and proof search disabled. Benchmark replay happens in memory; local lockfiles do not influence the result.

The output includes certificate sizes, proof-tree sizes, attempted search tiers, compiler build settings, workload hashes, and raw samples. The headline compile metric is elapsed compilation time for the search pass. It excludes file I/O, library preflight parsing, and the additional certificate-serialization and kernel-recheck measurement pass. It does not measure the runtime of emitted Rust.

Per-obligation search times overlap frontend work; `kernel_recheck_ns` measures an additional independent recheck, not a disjoint slice of the headline total. The frontend elaboration metric is the residual duration after subtracting lowering, checking, and erasure. These are instrumented boundaries, not CPU profiling samples, and the reported fields must not all be summed together.

## Compare like with like

An epoch pins the workload and library contents, machine class, toolchain and build configuration, schema, and language options. Results from different epochs are not merged into one trend. Within an epoch, workload medians are normalized to the first run; the headline index is their geometric mean, with the baseline at 100.

Regression detection compares medians using pooled robust dispersion, based on median absolute deviation. A flag requires a slowdown beyond six pooled dispersions, using the current samples and up to ten preceding same-epoch records. At least three samples are required on each side; insufficient samples are reported as such. A flag invites investigation and does not fail proof checking. The policy avoids an arbitrary universal percentage threshold.

## Read the dates as carefully as the numbers

Every displayed result names the recorded source revision and whether its checkout was dirty. Historical measurements do not claim to describe the current compiler. A missing data branch produces an explicit empty state. The two-week observation window remains open until real same-epoch samples span two weeks; the site never fills gaps with invented runs.

Fast-suite duration is tracked separately from compiler benchmarks. Its 120-second target is advisory: exceeding it is reported, while failed checks still fail the gate. Generated full-suite status is tied to a source fingerprint and hides stale counts.

[Inspect the generated status](generated-status.md) or [read the raw records on GitHub](https://github.com/nikhilgarg28/locus/tree/locus-bench-data/records).

## Reproduce a measurement

~~~sh prose shell-commands
cargo run --release -- bench --samples 5
python3 tools/bench.py summary
tools/check.sh
tools/check.sh --extended
~~~

The full gate checks semantics and records its own completion. Benchmark history is appended as content-addressed JSON on the local data branch; publish that branch separately to share its records. See [development](development.md) for workload selection and freshness checks, and the [implementation](../tools/bench.py) and [measurement tests](../tools/test_bench.py) for the exact comparison policy.
