+++
id = "generated-status"
title = "Generated status"
group = "Now"
created = "2026-09-23T04:03:42.000Z"
updated = "2026-09-24T08:48:31.000Z"
route = "performance/status.html"
order = 15
+++

# Generated status

Generated from a successful complete gate. Source edits invalidate this record until the extended gate refreshes it. Counts are measurements, not promises.

Measured at 2026-09-24T08:48:31.152990+00:00; source fingerprint `6d8ffa975122b3e30718ec479779f241754a0c65cfa75ed1bfaf163c3fb6b6e5`.

| Measurement | Value |
|---|---|
| Fast tests passed | 910 |
| Extended tests passed | 910 |
| Corpus files | 265 |
| Fast suite / advisory target | 208s / 120s |
| Kernel proof constructors / axiom constructors | 30 / 34 |
| Operative paragraphs / citations | 349 / 561 |

## Target proof measurements

| Workload | Search tiers | Locked replay hits |
|---|---|---|
| tests/corpus/target/library_buffer_model.lc | arithmetic: 23, computed: 8, evaluation: 3, exact: 3, stored: 3 | 40 |
| tests/corpus/target/library_finite_map.lc | computed: 2, exact: 2 | 4 |
| tests/corpus/target/library_integer.lc | arithmetic: 9, computed: 3, evaluation: 1, exact: 2 | 15 |
| tests/corpus/target/library_relations.lc | arithmetic: 2, computed: 1 | 3 |
| tests/corpus/target/library_runtime_list.lc | arithmetic: 9, computed: 2, evaluation: 1, exact: 2 | 14 |
| tests/corpus/target/library_seq_ops.lc | arithmetic: 12, computed: 3, evaluation: 2, exact: 2 | 19 |
| tests/corpus/target/lock.lc | arithmetic: 6, computed: 3, evaluation: 2 | 11 |
| tests/corpus/target/logical_mutual_trees.lc | arithmetic: 1, computed: 2 | 3 |
| tests/corpus/target/midpoint.lc | arithmetic: 5, evaluation: 1 | 6 |
| tests/corpus/target/percent.lc | computed: 1 | 1 |

## Trusted-base source inventory

Physical source lines, including comments and blanks. Mixed files are counted whole; this conservative count is distinct from the proof-kernel size.

| File | Lines | Scope |
|---|---|---|
| src/kernel/buffer.rs | 337 | kernel acceptance and primitive meaning |
| src/kernel/check.rs | 1953 | kernel acceptance and primitive meaning |
| src/kernel/context.rs | 241 | kernel acceptance and primitive meaning |
| src/kernel/defs.rs | 545 | kernel acceptance and primitive meaning |
| src/kernel/depth.rs | 288 | kernel acceptance and primitive meaning |
| src/kernel/eval.rs | 278 | kernel acceptance and primitive meaning |
| src/kernel/generics.rs | 222 | kernel acceptance and primitive meaning |
| src/kernel/int.rs | 230 | kernel acceptance and primitive meaning |
| src/kernel/linear.rs | 330 | kernel acceptance and primitive meaning |
| src/kernel/machine.rs | 149 | kernel acceptance and primitive meaning |
| src/kernel/measured.rs | 117 | kernel acceptance and primitive meaning |
| src/kernel/nat.rs | 316 | kernel acceptance and primitive meaning |
| src/kernel/ops.rs | 391 | kernel acceptance and primitive meaning |
| src/kernel/quantifiers.rs | 182 | kernel acceptance and primitive meaning |
| src/kernel/recursive.rs | 706 | kernel acceptance and primitive meaning |
| src/kernel/term.rs | 2472 | kernel acceptance and primitive meaning |
| src/typed/lower.rs | 2847 | typed lowering, layout, permissions and native boundary |
| src/typed/layout.rs | 249 | typed lowering, layout, permissions and native boundary |
| src/typed/shared.rs | 1064 | typed lowering, layout, permissions and native boundary |
| src/typed/buffer.rs | 487 | typed lowering, layout, permissions and native boundary |
| src/typed/trusted.rs | 88 | typed lowering, layout, permissions and native boundary |
| src/typed/tree.rs | 693 | typed lowering, layout, permissions and native boundary |
| src/exec/check.rs | 733 | check IR acceptance and native contracts |
| src/exec/buffer.rs | 136 | check IR acceptance and native contracts |
| src/exec/ir.rs | 265 | check IR acceptance and native contracts |
| src/erased/check.rs | 726 | erasure, independent layout checker, cleanup and Rust generation |
| src/erased/erase.rs | 999 | erasure, independent layout checker, cleanup and Rust generation |
| src/erased/cleanup.rs | 670 | erasure, independent layout checker, cleanup and Rust generation |
| src/erased/rust.rs | 1370 | erasure, independent layout checker, cleanup and Rust generation |
| src/erased/tree.rs | 316 | erasure, independent layout checker, cleanup and Rust generation |
| src/elab/items.rs | 2095 | mixed: Rust export/forgery boundary; remainder elaboration untrusted |
| src/build.rs | 102 | Rust crate/marker privacy boundary |
| src/project/load.rs | 483 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/project/resolve.rs | 1159 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/project/check.rs | 35 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/project/export.rs | 602 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/project/cargo.rs | 297 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/project/build.rs | 285 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/source.rs | 169 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/elab/env.rs | 639 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/elab/calls.rs | 656 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/elab/data.rs | 531 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/elab/mutation.rs | 1027 | module identity, source privacy, Cargo identity and Rust export boundary |
| src/elab/models.rs | 559 | mixed: canonical observation and derived-model privacy; proof construction untrusted |

## Benchmark history

Epoch `4798fc0b00fbe1bb59179d8dd9d3903af00a8877d0bc2c4279a397c3a1519b69`; 2 real records; 0.05 observed days.

Headline index: 95.65 (first record = 100; lower is faster).

The two-week observation window is still open; no historical samples are fabricated.

| Workload | Median history | Latest assessment |
|---|---|---|
| tests/corpus/target/library_buffer_model.lc | █▁ | stable |
| tests/corpus/target/library_finite_map.lc | █▁ | stable |
| tests/corpus/target/library_integer.lc | █▁ | stable |
| tests/corpus/target/library_relations.lc | █▁ | stable |
| tests/corpus/target/library_runtime_list.lc | █▁ | stable |
| tests/corpus/target/library_seq_ops.lc | █▁ | stable |
| tests/corpus/target/lock.lc | █▁ | stable |
| tests/corpus/target/logical_mutual_trees.lc | █▁ | stable |
| tests/corpus/target/midpoint.lc | █▁ | stable |
| tests/corpus/target/percent.lc | █▁ | stable |
