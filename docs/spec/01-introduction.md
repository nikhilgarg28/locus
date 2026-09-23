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
This manual describes the language accepted by the current compiler. Read [Values and types](02-types.md) through [Erasure](12-erasure.md) for its programming model, or start with [A verified lock](16-walkthrough.md) for a complete example. Logical data, checked logical recursion, models and shared references are enabled by default; no preview flags are needed. The [implementation boundary](15-scope.md) lists unsupported constructs. The [Vision](../vision/target-language.md) records design goals rather than additional accepted syntax. [Architecture](../architecture.md) explains the compiler; the [kernel contract](../reference/kernel.md) and [formal core](../reference/formal-core.md) state its checking rules. Permanent paragraph IDs connect these documents to focused tests.

<!-- spec: 1.0:2 legality-rule -->
Locus uses Rust-style tokens and preserves Rust semantics for its supported runtime constructs. Its additions include logical computation, propositions, evidence and checked function promises. The [scope chapter](15-scope.md#not-in-the-language) lists Rust constructs that are not supported. Every Rust keyword is reserved, so a permitted Locus name can also name the corresponding generated Rust item.