+++
id = "language-tooling"
title = "Checking and diagnostics"
group = "Now"
spec_chapter = 1
order = 113
route = "specification/tooling.html"
description = "Proof lockfiles, diagnostics, and the compiler commands."
+++

# Checking and diagnostics

<!-- spec: 1.0:15 informative -->
Use an unlocked check while writing proofs, then replay the resulting certificates in CI. Diagnostics identify the claim that failed, the facts available, and the source operation responsible. Stored proofs are reusable evidence, never trusted assertions.

## Found proofs are stored

<!-- spec: 1.20:1 legality-rule -->
`locus check file.lc` stores accepted certificates in `Locus.lock` beside the source. Every reused certificate is checked against the current obligation. `--locked` disables search and writes; a missing or stale certificate is an error. `--no-store` or `LOCUS_PROOFS=off` disables storage; `LOCUS_SEARCH=none` disables search while still allowing replay.

<!-- spec: 1.91:31 informative -->
Commit the lockfile with source changes. Its keys describe claims and contexts, so reformatting and local renaming need not discard proofs. The [development guide](../development.md#reuse-checked-proofs) covers replay and migration; the [architecture reference](../architecture.md#proof-construction-persistence-and-diagnostics) describes the format. A corrupted or stale entry can cause failure or renewed search; it cannot authorize a false claim.

<!-- spec: 1.27:12 legality-rule -->
A successful unlocked check canonically replaces only the checked source’s entries, preserving other files and dropping unused obligations. Sources sort by path and obligations by encounter order. Keys use kernel claims and contexts with stable declaration identities; locations are diagnostic labels. Identical inputs produce identical canonical proofs, diagnostics, and Rust.

## Diagnostics

<!-- spec: 1.21:1 legality-rule -->
Diagnostics have stable codes, source spans, notes, and applicable suggested edits. Source errors exit with status 1; command-line errors exit with 2. `check --error-format json` emits the versioned [diagnostic schema](../diagnostics/schema.md), retaining original entry/library locations. `locus explain CODE` gives the cause, an example, and a specification reference.

<!-- spec: 1.21:2 legality-rule -->
Failed proof construction (`L0230`) and arithmetic safety obligations (`L0235`) show the goal and up to six relevant facts. A reported counterexample is checked against those facts. Stale evidence identifies the invalidating write. When a missing explicit step is recognizable, the diagnostic suggests it. Parser recovery reports further errors under fixed work and depth limits.

<!-- spec: 1.90:63 example -->
~~~locus reject L0235
#[no_panic]
fn difference(lo: u32, hi: u32) -> u32 {
    hi - lo // Missing a precondition or branch establishing lo <= hi.
}
~~~

## The command line

<!-- spec: 1.22:1 example -->
~~~sh prose shell-commands
locus check file.lc --holes --stats
locus check file.lc --locked
locus run file.lc function 42
locus rust file.lc
locus audit file.lc
locus build file.lc --out generated
locus tokens file.lc
locus parse file.lc
locus ast file.lc
~~~

<!-- spec: 1.22:2 legality-rule -->
Repeat `--library path.lc` to include checked source libraries in the same compilation unit. Diagnostics retain their original paths. All current language features are enabled without preview flags; obsolete flags are rejected with removal guidance.

<!-- spec: 1.22:3 legality-rule -->
The `run` command accepts machine integers and booleans as command-line arguments. Documentation and corpus run directives can also express structured values, erased evidence, and the expected final contents of mutable arguments. Those directives belong to the test harness, not source-language syntax.
