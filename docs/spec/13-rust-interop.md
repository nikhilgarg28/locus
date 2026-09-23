+++
id = "language-rust-interop"
title = "Rust interoperability"
group = "Now"
spec_chapter = 1
order = 112
route = "specification/rust-interop.html"
description = "Readable generated Rust and the privacy boundary that protects verified interfaces."
+++

# Rust interoperability

<!-- spec: 1.0:14 informative -->
Generated Rust is the runtime form of a Locus program. Its public types and functions must preserve the verified boundary even when called from ordinary Rust, where erased proof markers no longer distinguish propositions. Visibility and validated data types provide that boundary.

## Visibility and the boundary with Rust

<!-- spec: 1.17:1 legality-rule -->
Items and fields are private unless marked `pub`; `pub(crate)`, `pub(super)`, `pub(self)`, and `pub(in path)` are parsed and printed as written. A function that takes evidence, as a parameter or inside a parameter's type, may be `pub(crate)` and never plain `pub`, because every proof erases to the same marker and a Rust caller could pass one obtained honestly to a function that wants evidence of something else; the error offers the two ways out, a narrower visibility or a validated type. What Rust sees takes no evidence: it takes plain data and checks it at runtime, as `Percent::checked` does, or takes a validated type whose fields are private. `tests/build.rs` and `tests/acceptance.rs` compile hand-written Rust against the generated crate of the target examples: the plain `pub` exports run, an evidence-taking function is E0603 or E0624, the marker cannot be named or built, a struct with a private field cannot be built or read, the marker replay attack does not compile, and a value a Rust caller still holds after catching a panic satisfies its invariant.

## What is generated

<!-- spec: 1.19:1 dynamic-semantics -->
`locus rust file.lc` prints one Rust module; `locus build a.lc b.lc --out dir [--name crate]` writes a crate: `Cargo.toml` with the crate's name and edition 2024 and no dependencies, `src/lib.rs` holding the single marker `Erased`, a `Copy` unit struct with a private field and a private constant of its own name, and one `src/<file>.rs` per Locus file declared as a `pub mod`. The Rust reads as the source: the same names, nesting, and declaration order, `if` as `if`, `match` as `match`, `let mut` and assignment as written (`mut` is dropped from a binding never assigned), the loops as written, `impl` blocks with `self` as written and calls as method calls, visibility as written, derives in the order written, integer literals with their type as a suffix, and proof positions filled by `Erased`. The crate compiles under `-D warnings`; the corpus test compiles every accepted file twice, with overflow checks on and off, and compares each run line with both interpreters.

<!-- spec: 1.19:2 dynamic-semantics -->
The test harness compares executions in two interpreters: one on the check IR, which skips logical computations, and one on the erased tree, each in two modes matching the two builds. Both report a value, a panic with its message and the values of the `&mut` parameters at that moment, or that they ran out of fuel, which is inconclusive and never a verdict. `tests/random_programs.rs` generates well-typed programs with arithmetic, panics, mutation in branches and loops, and `&mut` calls, and compares the two interpreters and the compiled Rust three ways.
