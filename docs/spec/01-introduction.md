+++
id = "language"
title = "Introduction"
group = "Now"
spec_chapter = 1
order = 100
route = "specification/introduction.html"
description = "The implemented language, its notation, and how to read this manual."
+++

# Introduction

<!-- spec: 1.0:1 informative -->
Locus is a Rust-like language in which functions can exchange data and checked evidence. This manual describes the current compiler. Each operative rule links to focused tests; each executable example is checked, and run examples are compared with generated Rust. Rule identifiers are permanent references, not the book’s current chapter numbers.

## A first guarantee

<!-- spec: 1.90:64 example -->
~~~locus run
#[no_panic]
fn next(n: u8, room: @(n < u8::MAX)) -> (out: u8, @(out == n + 1)) {
    let out = n + 1;
    (out, _)
}
fn example() -> u8 {
    let (value, correct) = next(41, prove!(41 < u8::MAX));
    value
}
//~ run: example() => 42
~~~

<!-- spec: 1.91:34 informative -->
The `room` parameter requires evidence that addition fits. `out` names the returned byte in the later proof field. `_` requests a proof of the expected claim; `prove!` writes the claim explicitly. The kernel checks both. At runtime the byte is computed, while its evidence is erased.

## Reading the manual

<!-- spec: 1.91:35 informative -->
Start with [types](02-types.md), [operators](03-integers.md), and [functions](04-functions.md). [Ownership](06-ownership.md) and [mutation](07-mutation.md) explain which values a proof describes. [Logical computation](08-logic.md), [propositions](09-propositions.md), and [proofs](10-proofs.md) build the verification vocabulary. [Models](11-models.md), [erasure](12-erasure.md), and [Rust interoperability](13-rust-interop.md) connect it to running code.

<!-- spec: 1.91:36 informative -->
The [examples](../examples.md) introduce practical patterns; [a verified lock](16-walkthrough.md) combines them. The [scope chapter](15-scope.md) lists current limits. Compiler internals live in [architecture](../architecture.md), the [kernel contract](../reference/kernel.md), and the [formal core](../reference/formal-core.md). Historical and future designs remain in the [Vision](../vision/target-language.md).

## Relationship to Rust

<!-- spec: 1.0:2 legality-rule -->
Supported runtime constructs use Rust-style tokens and Rust semantics. Locus adds logical computation, propositions, proof types, and checked promises. Every Rust keyword is reserved. This is a deliberately limited language, not a Rust superset; the [scope chapter](15-scope.md#not-in-the-language) identifies unsupported constructs.
