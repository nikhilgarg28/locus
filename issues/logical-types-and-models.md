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

### 2026-11-22, snapshots, models, and capture are in RFC 0002

The owner's decisions on snapshots, models, and capture are RFC 0002's decisions 99 to 108. The owner settled six constraints with a second reviewer, asked for a recommendation on the whole design, had the reviewer amend it, and on the amended recommendation said:

> Good - assuming spike succeeds, I think we should begin to now update PR with all the decisions that have been made across all RFCs but not yet persisted in github.

The third round of the skeleton spike then succeeded ([skeleton-typing-3](../spikes/skeleton-typing-3/README.md)). What is written:

- **A place that logical code reads is captured:** it stands for the model of the place's snapshot. With no model declared, the model is the snapshot. `models.no-default` is removed, and `models.default` replaces it.
- **The snapshot of a machine integer is a `Nat` or an `Int`.** A captured value is valid for its type (`models.valid`, `models.valid.captured`, `models.valid.not-assumed`), which replaces `models.integer-range`.
- **A logic function takes logical types only** (`logic.fn.parameters`), and a method call on a place resolves on the type of its capture (`models.capture.methods`).
- **A type parameter is read under a written bound** (`models.capture.bound`), which may now be on an `impl` and on a type's declaration (`models.bounds-on-functions`).
- **A type that Locus cannot see has neither a snapshot nor a model** until a field of it is marked `#[opaque]` (`models.snapshots.seen`, `models.snapshots.opaque`).
- **A model is declared with its type,** for the type at all its type arguments (`models.declared-by-owner`, `models.one-per-type`), and takes its snapshot and no proof (`models.total`).

Four decided examples change their outcome, and 26 logic functions in 19 examples no longer declare a parameter with a runtime type. The pull request lists each.

The owner saw the recommendations and the amendments, and not the rule texts. What the writing chose is item 30 of what is still for the owner in RFC 0002. Two of its points need the owner's word before anything else: that a captured value is valid is a fact that the language infers, and a struct that Rust accepts is now rejected when it has a field of a type that Locus cannot see and no mark ([unmarked-fields-and-the-superset](unmarked-fields-and-the-superset.md)).

One example is open and linked here: `litmus/models/method-on-a-snapshot.open.lc`, on whether `snap(s).room()` means `Stack::room(snap(s))`.

The summary under "Status" above predates this, and several of its lines no longer hold. RFC 0002 is the current text.

### 2026-11-23, an unmarked field does not reject its struct

The entry above says that a struct that Rust accepts is rejected when it has a field of a type that Locus cannot see and no mark. That was this change's first text of `models.snapshots.opaque`, and it is corrected: such a struct is declared and used as in Rust, and has no snapshot and no model until the field is marked, which is what the recommendation that the owner approved had. [unmarked-fields-and-the-superset](unmarked-fields-and-the-superset.md) has the record, and what is still open there.
