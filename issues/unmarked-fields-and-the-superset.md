# A field of a type that Locus cannot see, and the superset

- **Kind:** decision
- **Status:** open
- **Blocks:** Accepting RFC 0002's decision 107 as it is written, and what RFC 0001 says of a `.rs` file renamed to `.lc`
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
