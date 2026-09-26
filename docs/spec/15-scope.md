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
The manual describes the implemented language. A parser accepting a token sequence is not enough: typing, permissions, verification, and erasure must all support its meaning. Future designs belong in the Vision, not in the current contract.

## The trusted base

<!-- spec: 1.23:1 legality-rule -->
End-to-end guarantees depend on the kernel and primitive semantics, checked program lowering and permissions, the check-IR checker, erasure and layout checking, Rust emission and export checks, the Rust toolchain, and declared native assumptions. Search, certificate construction, and interpreters do not authorize proofs. The [trusted-base inventory](../../tools/trusted-base.json), [kernel contract](../reference/kernel.md#trusted-base), and [formal core](../reference/formal-core.md) state the boundary.

<!-- spec: 1.91:32 informative -->
Locus is tested through independent checks, hostile inputs, interpreter comparison, and compiled Rust. The compiler is not formally verified; mechanized preservation remains deferred. See [correctness](../correctness.md) for the evidence and its limits.

## Not in the language

<!-- spec: 1.24:1 legality-rule -->
- General runtime recursion, runtime closures, generic trait bounds, and dynamic dispatch.
- Stored mutable references, interior mutability, raw pointers, unsafe code, and async.
- 128-bit integers, floats, characters, and byte strings.
- Bit operators, compound assignment, iterator-based `for`, `while let`, `if let`, `?`, labels, and deep enum patterns.
- Arbitrary Rust type import and unsupported trait features, generic cross-package runtime ABI, and user macros or attributes.

<!-- spec: 1.91:33 informative -->
Use [modules and Cargo packages](17-modules.md) for source composition and the registered [native contract bridge](11-models.md#native-contracts) for supported Rust adapters. The [roadmap](../roadmap.md) distinguishes implemented, partial, and deferred work.
