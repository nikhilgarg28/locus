+++
id = "readme"
title = "Overview"
group = "Now"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T04:43:11.000Z"
route = "guide/overview.html"
order = 0
+++

# Locus: overview

Locus is a Rust-like language that puts propositions and explicit proofs in programs. A function can accept evidence, return it alongside data, or store it in an invariant-bearing type. The compiler checks that evidence and emits readable Rust with logical computation erased.

The intended use is the part of a crate with a precise correctness requirement: an arithmetic routine, a data structure, a state transition, or an interface that must preserve an invariant. The programmer supplies the specification. Proof construction can be automated, but every resulting certificate must pass the kernel.

## What is implemented

Executable code has machine arithmetic, mutable bindings, branches, loops, methods, and checked ownership and borrowing rules. Arrays, slices, vectors, and boxes have explicit physical representations. Logical computation has mathematical data, propositions, proof values, recursive definitions, and user-defined models of runtime objects. Generics are specialized and checked at each concrete instance.

The language implements a selected set of Rust-like features. The [specification](specification.md) defines that set; the [roadmap](roadmap.md) separates completed work from future interoperability. In particular, generated generic types currently have Locus-specific Rust identities, and the compiler does not accept arbitrary Rust crates as Locus source.

## What a proof guarantees

A checked proof establishes its stated proposition under the input assumptions and admitted contracts. Function postconditions generally describe normal return. Termination and absence of panic need their own checked promises where supported.

Evidence has no runtime content. Safe Rust callers obtain invariant-bearing values through the generated checked interface, whose visibility rules prevent forging its proof fields. This boundary relies on the compiler's layout, borrowing, erasure, and export checks as well as the logical kernel. [Correctness](correctness.md) explains the tests, trusted components, and unproved preservation obligations.

## Try it

Building the compiler requires Rust 1.91 or newer. Cargo resolves the dependencies pinned in `Cargo.lock`.

~~~sh prose shell-commands
cargo run -- check tests/corpus/target/lock.lc --holes
cargo run -- check tests/corpus/target/lock.lc --locked
cargo run -- run tests/corpus/target/lock.lc run 300 9
cargo run -- rust tests/corpus/target/lock.lc
cargo run -- build tests/corpus/target/percent.lc --out /tmp/percent_crate
~~~

An unlocked check records reusable certificates in `Locus.lock` beside the source. A locked check rechecks those certificates without proof search or writes. The `run` command uses the checking-IR interpreter; `rust` prints the emitted source and `build` creates a Rust crate.

Start with the [small examples](examples.md), then the [verified lock walkthrough](spec/16-walkthrough.md). For implementation details, see [architecture](architecture.md); for contributing and reproducing the checks, see [development](development.md). [Performance](performance.md) and [generated status](generated-status.md) report measurements with their revision and freshness information.
