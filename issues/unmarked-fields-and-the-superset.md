# A field of a type that Locus cannot see, and the superset

- **Kind:** decision
- **Status:** open
- **Blocks:** Relying on RFC 0002's decision 107 while it rests on a behaviour of rustc that is not established as intended, and the wording of commitment 5 of the vision
- **Rules:** `models.snapshots.opaque`, `models.snapshots.seen`, `compat.superset.same-meaning`

## Question

May Locus reject a struct that Rust accepts, because one of its fields has a type that Locus cannot see?

RFC 0002's decision 107 gives a snapshot and a model only to a type that Locus lowers or that its library declares. A field of any other type is marked `#[opaque]`, and `models.snapshots.opaque` says that a struct or an enum with such a field and no mark is rejected:

```rust prose valid Rust, which the rule rejects
struct Server {
    listener: std::net::TcpListener,
    port: u16,
}
```

Commitment 1 of the vision says that every valid Rust program is a valid Locus program with the same meaning. `compat.superset.same-meaning` says that Locus accepts a program that rustc accepts, and that a `.rs` file renamed to `.lc` is the same program. The rule applies to a struct declared in a `.lc` file or a `locus!` block, so a plain `.rs` file is untouched, and a `.rs` file renamed to `.lc` is not.

Commitment 5 says that the language gives every runtime value a snapshot. Decision 107 steps back from that as well: a value of a type that Locus cannot see has none.

Until Locus's library says which types of the standard library it declares, the same holds for a field of type `String`, `char`, or `f64`.

## What it blocks

Nothing is blocked from being written: the rule and its examples are in RFC 0002 as proposed. Accepting the RFC with them accepts an exception to commitment 1, which only the owner can make.

## Options

- **Keep the error at the declaration, and state the exception.** The vision and `compat.superset.same-meaning` then say that a type declared in Locus code needs a mark on each field of a type that Locus cannot see. The third round of the skeleton spike found that no declaration in its earlier sets needed one.
- **No error at the declaration.** The type then has no snapshot and no model, and the error comes where logical code first reads a value of it. This is what the recommendation put to the owner said. The third round of the spike built it, as a `where` clause behind a higher-ranked lifetime, and found that it refused every declared model that reads a field. Another way to build it is not known.
- **Lowering marks such fields itself.** It would need types before rustc runs, which is staged typing, and gives up building with stock `cargo build`.
- **Locus's library declares an opaque snapshot for every type of the standard library,** which removes the common cases, such as `String`, and leaves the types of other crates.

## Example

`litmus/models/foreign-field-unmarked.reject.lc` is decided as rejected, as decision 107 has it. Its outcome changes if the owner chooses the second option.

## Log

### 2026-11-22, raised while writing RFC 0002's decisions 99 to 108

The owner accepted a recommendation in which a type that Locus cannot see has no model until a field is marked or the type is wrapped, and in which a struct with an unmarked field still compiled. The third round of the spike made the error at the declaration its default, and RFC 0002 writes that. Neither the recommendation nor the spike speaks of commitment 1, so the question is put here.

### 2026-11-23, the rule is written without the error at the declaration

The third round of the skeleton spike had found that deferring the check, so that a struct with an unmarked field still compiles, refused every declared model that reads a field. [capture-family](../spikes/capture-family/README.md) now explains that and avoids it, in three more cases. With the snapshot and the model as two traits, only what lowering generates is deferred, the snapshot and the default model, and a model that the program declares is an implementation of its own with no clause. A struct with an unmarked field of type `TcpStream` then compiles and is used as plain Rust (`c14`), the error is at the capture when it is read in logical code (`c15`), and at the model when one is declared for it (`c16`). The three cases are hand-written; the toy lowering of the third round has one trait and does not generate this form.

So RFC 0002 now writes the rule as the recommendation that the owner approved had it. `models.snapshots.opaque` no longer rejects the struct: it has no snapshot and no model until the field is marked, and the error comes where a place of it is captured, where `snap` or `Snap<T>` is written of it, where a model is declared or derived for it, and where a `Model` bound is asked of it. Commitment 1 of the vision and `compat.superset.same-meaning` stand untouched, and the question above, as it was asked, is answered: no.

The example named above is now `litmus/models/foreign-field-unmarked.accept.lc`. `litmus/models/unmarked-struct-captured.reject.lc` and `litmus/models/unmarked-struct-model-declared.reject.lc` are the rejected side. The text above the log is left as it was written, apart from the `Blocks` line.

Two things are still the owner's, and keep this issue open.

- **The form rests on rustc not checking a `where` clause behind a higher-ranked lifetime at a declaration.** That holds on rustc 1.97.0, and nobody has established it as intended. If it cannot be relied on, the choice returns between the error at the declaration, which is an exception to commitment 1, and staged typing, which gives up building with stock `cargo build`.
- **Commitment 5 of the vision** says that the language "gives every runtime value a snapshot, which is what the value is". Decision 107 steps back from that for a type that Locus cannot see, so the sentence needs the owner's edit. Proposed words, for the owner to take or change: the language "gives a snapshot, which is what the value is, to every runtime value whose type it can see: its own types, the types a Locus program declares, and those its library describes". `VISION.md` is not edited here.

### 2026-11-23, RFC 0001 says how rustc is given the rule

[RFC 0001](../design/rfcs/0001-compiler-pipeline.md#snapshots-and-models-as-rustc-is-given-them) now writes the form that the entry above describes, under "Snapshots and models, as rustc is given them": two traits, one that carries the snapshot and `Model`; what lowering generates, the snapshot's implementation and the default model, deferred behind a `where` clause that rustc checks only where the implementation is used; and a model that the program declares in an implementation of its own, with no such clause. It says there that the evidence is hand-written Rust, and that the toy lowering of the third round has one trait and does not generate this form.

Under "Costs and limits" the RFC states what this issue stays open for: the form rests on rustc not checking a `where` clause behind a higher-ranked lifetime where an implementation is declared, which holds on rustc 1.97.0 and which nobody has established as intended. If it cannot be relied on, the choice is between an error at the declaration, which is an exception to commitment 1 of the vision, and staged typing, which the RFC keeps in reserve among its alternatives and which gives up building with stock `cargo build`. It is item 7 of what is still for the owner there.

Nothing here is decided by that text, and this issue is not closed. The wording of commitment 5 of the vision is still the owner's, as the entry above says. Two other sentences of the vision are raised in [typed-logical-code-and-the-vision](typed-logical-code-and-the-vision.md).

### 2026-11-23, the vision's fifth commitment is edited

Shown the words proposed above for commitment 5, the owner first asked which types those are:

> sure but I'd like to understand what kind of types it won't be able to see?

The owner was told: a value that is code and not data, which is a closure, a function item, a function pointer, a trait object, and a type that `impl Trait` hides; a type of a crate that Locus does not lower; and a type of the standard library that Locus's library has not yet declared. Then, of this and the other recommendations then open:

> Follow all your recommendation - please go ahead and reflect them in the PR

`VISION.md` now has the proposed words, in the same PR: the language "gives a snapshot, which is what the value is, to every runtime value whose type it can see: its own types, the types a Locus program declares, and those its library describes". [RFC 0001](../design/rfcs/0001-compiler-pipeline.md) records it in decision 53.

This issue stays open for its other point: the form in which decision 107 is given to rustc rests on rustc not checking a `where` clause behind a higher-ranked lifetime at a declaration, which nobody has established as intended.

### 2026-11-23, the standard library's `Vec` is now a type that Locus cannot see

RFC 0002's decision 113 gives the standard library's `Vec`, and its other collections, neither a snapshot nor a model. So a field of type `std::vec::Vec<u32>` is now among the fields that this issue is about: the struct that holds it is declared and used as it is in Rust, and has no snapshot until the field is marked `#[opaque]` or the vector is Locus's. Before, Locus's library declared a snapshot for it.

Nothing in the question changes. The case is more common than `TcpListener`'s: a program that states something about a struct with a vector in it imports Locus's `Vec`.

One sentence of `models.snapshots.opaque` now misleads, and is for the owner with the rest of this issue. The rule says that a field whose type has no snapshot is marked, and also that a field whose type names a type parameter needs no mark. The second was written when a `Vec<T>` had a snapshot wherever `T` has one. For a field of type `std::vec::Vec<T>`, as for one of type `HashMap<K, V>` before this change, the type has no snapshot at any type argument, so the first sentence asks for the mark while the second says that none is needed. The reading that accepts less is the first: such a field is marked, or its struct has no snapshot. The rule is not reworded here, and no example pins the case.
