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
