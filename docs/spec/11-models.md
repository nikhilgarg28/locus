+++
id = "language-models"
title = "Models and heap data"
group = "Now"
spec_chapter = 1
order = 110
route = "specification/models.html"
description = "Observe owned data through checked models and understand the native trust boundary."
+++

# Models and heap data

<!-- spec: 1.0:12 informative -->
A model gives runtime data an immutable logical description that a proposition can mention. For heap data, the description is a snapshot: later mutations produce new observations. The collection and borrowing rules below connect those snapshots to permitted runtime accesses.

## Collections and snapshots

<!-- spec: 1.26:1 dynamic-semantics -->
Arrays, slices and Vec have immutable content snapshots with separately checked physical layouts. Length is a mathematical observation bounded by u64::MAX; runtime len returns u64. Indexing requires evidence of its bounds. Updates and push produce new snapshots and normal-return equations; allocation failure or panic does not invalidate partial-correctness claims about successful returns. Source `Vec::new()` and `Vec::from([elements])` use registered Rust implementations.

## Shared references

<!-- spec: 1.26:2 dynamic-semantics -->
Shared references in locals, fields and results carry checked lifetimes and typed provenance: their storage root, path and version. Local borrow lifetimes can be inferred; returned reference signatures use explicit input lifetime names. Locus checks those permissions before erasure, including uses in model observations. A reference cannot escape local storage, and an overlapping write/move invalidates its later use. Mutable loans remain call-scoped. Rust compilation supplies an independent oracle, not permission for an erased access that Rust never sees. These rules are enabled by default and tested in `tests/reconcile_shared.rs`.

## Native contracts

<!-- spec: 1.26:3 dynamic-semantics -->
A foreign declaration is `trusted "reviewable reason" fn name(parameters) -> Result = Vec::len;`, with the registered native path selected explicitly. This initial bridge supports Vec len, get and push. The signature must retain the native runtime argument/result shapes, including evidence slots. The body and its logical postcondition are deliberately not checked against each other. The header is well formed, the runtime implementation is registered, and the mandatory reason and claimed promises appear in `locus audit`. A false trusted specification can make a caller's claim false; the audit identifies that assumption, and tests demonstrate this boundary. Such a declaration is never a logical function or an unfoldable kernel definition.

## Permission restrictions

<!-- spec: 1.26:4 dynamic-semantics -->
The current shared-reference tier admits references in locals, tuples, structs, enum payloads and results. Returned/stored references use explicit lifetime parameters. Shared-reference leaves inside owned Vec/array/Box payloads, stored mutable references, interior mutability, and raw pointer addresses remain unsupported. Branch and loop-back-edge permissions are joined conservatively and may reject some Rust-valid programs. A logical model observation is a permission use even though it erases. Indexed borrowed lookup retains `&items[index]` and separately checked bounds in generated code.

## Evidence and witness scope

<!-- spec: 1.26:5 dynamic-semantics -->
Proof elimination is stricter than logical computation: a match on evidence returns evidence, not arbitrary Logical data. Witnesses introduced by a proof pattern stay inside the derivation and cannot escape as Int, Seq, or a predicate closure. Witness-free one-arm evidence may be destructured by let; witness-bearing patterns use a proof-producing match. This preserves proof irrelevance; erasure by itself would not justify a witness extractor.

## Logical elements in physical containers

<!-- spec: 1.26:6 dynamic-semantics -->
Physical Vec, arrays and parameter slices may contain Logical elements, including Int, Bool and evidence. They emit Vec<Erased>, [Erased; N] or borrowed marker slices while retaining lengths and ordinary argument effects, using Rust's zero-sized-element storage behavior. Mixed payloads erase their logical leaves while retaining runtime fields. Reading a logical element produces a logical value, never a runtime bool or integer.
