+++
id = "language-erasure"
title = "Erasure"
group = "Now"
spec_chapter = 1
order = 111
route = "specification/erasure.html"
description = "What disappears, what remains, and why erasure must preserve observable effects."
+++

# Erasure

<!-- spec: 1.0:13 informative -->
Checking uses logical values that the generated Rust does not need to store or compute. Erasure removes that logical work while preserving ordinary evaluation, including effects from calls whose returned values are logical.

## Values and effects

<!-- spec: 1.18:1 dynamic-semantics -->
All Logical positions use one private zero-sized marker, `Erased`. Logical declarations and computations have no runtime implementation. Physical tuples and structs keep their layout positions, replacing logical fields with markers; ordinary enum tags and physical allocation remain observable. A physical `Box<T>` remains physical even when T is Logical.

<!-- spec: 1.18:2 dynamic-semantics -->
Erasure separates value production from execution. A logical operation disappears after its ordinary argument computations have run. An ordinary `fn` call remains even if it returns only evidence, takes only logical inputs, or promises to terminate and avoid panic. An ordinary function that takes `&mut` and returns a proof still performs its mutation at runtime. There is no result-type shortcut that deletes its effects.

<!-- spec: 1.18:3 dynamic-semantics -->
Unused erased local bindings and unused erased destructuring components are removed by default in generated Rust. Effectful right sides are retained as statements. Bindings whose evaluation transfers control become the block's terminal expression, so removing an unused marker does not produce unreachable Rust or change divergence. The cleanup is checked by interpreter comparison and Rust compilation with warnings denied.

<!-- spec: 1.18:4 dynamic-semantics -->
Proofs are never observed by runtime code. Matching evidence can establish more evidence; it cannot expose an existential witness as data, including Logical data. Physical containers such as `Option<@P>` keep a runtime discriminant, so choosing Some or None is ordinary computation, with its evidence payload erased.
