# Logical types and models

- **Kind:** decision
- **Status:** open
- **Rules:** `logic.types.contents-logical`, `models.one-per-type`, `models.derived`, `logic.eq.runtime-values`

## Question

What logical types and models Locus has, and what logical code may read from runtime values.

## Status

[RFC 0002](https://github.com/nikhilgarg28/locus/blob/claude/beautiful-mccarthy-cmtyao/design/rfcs/0002-logical-layer.md), proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10), answers this. The issue closes when the owner accepts it. In brief:

- **Logical types.**
  - `Int` is built in.
  - `#[derive(Logical)]` declares logical data. It can only be derived, and fails unless every field and payload is logical.
  - Locus's library builds `Nat`, `Bool`, `Seq`, and the rest ([#11](standard-library.md)).
- **Every runtime type has exactly one model:**
  - declared with `impl Model`, by the type's own crate;
  - derived with `#[derive(Model)]`, from the models of its fields, each of which must have a real model;
  - declared by Locus's library for standard types;
  - or, by default, an opaque identity `Opaque<T>` of a value at a version, about which nothing else is known.
- **Equality.** On runtime values, `===` compares models, and never implies equal fields. This fixes a contradiction found in review, where field-wise equality of values without a model passed through a field whose model forgets something.
- **Fields are read as places.** Logical code reads a value's visible fields this way whatever its model is. It never reads inside an `UnsafeCell` or through a raw pointer, so no model reads data that changes while the value is shared.
- **Method calls in logical code.** Method calls, indexing and dereferences in logical code work on models, never on runtime methods or overloaded `Deref` and `Index`.
- **Logical and proof fields** stay in types, as zero-sized markers (`ErasedData` and `Erased`). Reading one is observed as a shared borrow of that field alone.

The examples that this issue used to list as open are decided in [#10](https://github.com/nikhilgarg28/locus/pull/10): `litmus/observation/before-move.accept.lc`, `after-mutable-borrow-ends.accept.lc`, and `proof-before-move.accept.lc`. They also depend on the minimum facts that RFC 0002 requires of the runtime semantics ([#6](runtime-semantics.md)).

## Still open, elsewhere

- The library's types, models, and specifications: [#11](standard-library.md).
- Generic `#[derive(Model)]`, and traits on logical types: [#12](traits-on-logical-types.md).
- An identity for reasoning about aliasing, such as which `Cell` two references share: later.

## Log

### 2026-10-25, opened on GitHub as #4

Moved into the repository from [GitHub issue #4](https://github.com/nikhilgarg28/locus/issues/4) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-26, comment on GitHub

> RFC 0002 (`design/rfcs/0002-logical-layer.md`, in [the RFC PR](https://github.com/nikhilgarg28/locus/pull/10)) proposes the design for this issue:
> - logical types classified by type;
> - ghost values;
> - one model per type, declared by the type's own crate;
> - models reading only frozen data;
> - logical code keeping Rust's meaning for places, with models applying to values.
>
> It decides the observation examples `before-move` and `after-mutable-borrow-ends` as accepted. `proof-before-move` now waits on [#6](runtime-semantics.md). The library's own types and models are tracked in [#11](standard-library.md).

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.
