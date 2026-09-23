+++
id = "language-scope"
title = "Scope and trust"
group = "Now"
spec_chapter = 1
order = 114
route = "specification/scope.html"
description = "The current implementation boundary and the assumptions behind its guarantees."
+++

# Scope and trust

<!-- spec: 1.0:16 informative -->
A guarantee depends both on the implemented language and on the machinery trusted to check and emit it. This chapter identifies that boundary and lists constructs outside the current language, so examples and future plans are not mistaken for supported behavior.

## The trusted base

<!-- spec: 1.23:1 legality-rule -->
The end-to-end correctness boundary includes the logical kernel’s acceptance rules and primitive meanings; checked typed lowering, physical layouts, permissions and native adapters; the check-IR checker; erasure, its independent layout checker, cleanup and the Rust printer; and export/privacy checks in `src/elab/items.rs` and `src/build.rs`. The Rust toolchain and recorded native or foreign specifications are external assumptions. The complete conservative file inventory is `tools/trusted-base.json`; a file containing both trusted and untrusted work is counted in full. Parsers, proof search, arithmetic certificate construction, derived proof builders, theory-lemma builders, stored certificates and both interpreters supply candidates or tests rather than authority to prove a claim. In particular, `src/kernel/derive.rs`, `theory.rs` and classical-dependency discovery are not kernel acceptance rules merely because they live under `src/kernel`. Independent kernel, IR, layout and permission checks must reject malformed output from untrusted stages. The [kernel contract](../reference/kernel.md#trusted-base) gives the logical rules; the [formal core](../reference/formal-core.md) describes lowering and erasure obligations. Lean mechanization of those obligations remains deferred; the present evidence is executable checking and the hostile-input, differential and Rust-boundary suites.

## Not in the language

<!-- spec: 1.24:1 legality-rule -->
General runtime recursion; arbitrary trait implementations, dynamic dispatch and runtime closures; stored mutable references and interior mutability; `usize`/`isize`, floats, character and byte-string types; bit operators and compound assignment; modules/import resolution beyond explicit library input; general foreign Rust linking, `unsafe`, async; iterator-based `for`, `while let`, `if let`, `?`, loop labels and deep runtime patterns; user-defined macros and attributes. A parsed construct is accepted only when its typing, permissions, and erasure rules are implemented.
