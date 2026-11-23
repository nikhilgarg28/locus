# Compatibility with Rust

Locus is a superset of Rust. This chapter states what that promises: Rust programs keep their meaning, Rust's errors stay Rust's, and Locus's syntax and names never take a meaning away from Rust code. How the compiler keeps these promises is described in [RFC 0001](../design/rfcs/0001-compiler-pipeline.md).

In this chapter, *rustc* means the release of rustc that Locus is pinned to. A program is checked in the configuration being built, or, when its crate is exported, in each of the configurations the crate declares ([Exporting to Rust](export.md)).

## Rust programs

r[compat.superset.same-meaning]
A program that rustc accepts is a Locus program with the same meaning, in every configuration Locus checks: Locus accepts it, and it behaves as rustc's compilation of it does. A `.rs` file renamed to `.lc` is the same program. The exceptions are a program that tests `__locus_check`, the `cfg` that Locus reserves for its checking compile, and a program that writes a name that Locus reserves ([`compat.names.reserved`](#locus-names)). Locus rejects both.

r[compat.errors.rustc]
An error that rustc reports in runtime code is reported by Locus as rustc reports it: with rustc's code and message, at the corresponding place in the source.

## Locus syntax

Locus syntax may appear only in `.lc` files. Within them, it is chosen so that no Rust program can be read as Locus.

r[compat.syntax.not-rust]
A Locus syntax form is recognized from tokens alone, without resolving names, and only where rustc rejects those tokens in code it compiles. Tokens that rustc accepts there keep their Rust meaning, even where they resemble Locus syntax. Locus syntax is not recognized inside macro invocations or attribute arguments.

r[compat.syntax.contextual-keywords]
`logic` is a keyword only immediately before `fn`, and `prop` only before the name of a proposition declaration. Everywhere else they are identifiers.

## Locus names

Locus's macros (`logic!`, `prop!`, `prove!`), its `verified` and `unverified` attributes, its `Logical` and `Model` derives, its `Model` trait, and the logical types and functions of its standard library are ordinary names, found the way the names in Rust's standard prelude are. Its `opaque` attribute, which marks a field ([`models.snapshots.opaque`](models.md#snapshots)), is read where the field is declared.

r[compat.names.shadowable]
A program's own definitions and imports take precedence over Locus's names, as they would over the standard prelude's, including a glob-imported macro with the same name as one of Locus's.

r[compat.names.no-methods]
Traits among Locus's names have no methods, associated functions, or associated constants that a program can name, so that having them in scope never changes what a method call or path in Rust code resolves to. A logic function that such a trait declares, as `Model` declares `model`, is not an item that Rust code can name: what rustc is given for it has a reserved name ([`compat.names.reserved`](#locus-names)).

r[compat.names.macros-expand]
Locus's macros are expanded like any other macro, wherever rustc expands macros: directly, inside another macro's input, in a macro's output, and under another name. A Locus macro name that is never expanded, as inside `stringify!`, is only tokens.

r[compat.names.reserved]
A program does not write an identifier that begins with `__logic_` or `__locus_`. Locus reserves both prefixes for the names that it writes for rustc and that must be the same in every crate, such as the name under which a trait holds what rustc is given for a logic function. A source that writes such an identifier is rejected, wherever it writes it: as the name of an item, a field, a parameter, or a local, in a path, and among the tokens of a macro's input. An identifier that only resembles a prefix, such as `__logic` or `_locus_x`, is not reserved.

r[compat.names.model-impls]
Only Locus writes an implementation of `Model`. A program declares a model in Locus code, as `impl Model for T` with no `unsafe`, where `T` is declared ([`models.declared-by-owner`](models.md#models)), or derives one, and from that Locus writes the implementation that rustc is given, as it writes one for every type that it lowers. In what rustc is given, `Model` is an `unsafe` trait. A program that writes `unsafe impl Model` is rejected, and so is an implementation of `Model` in plain Rust: in a `.rs` file, outside a `locus!` block, or in a crate that Locus does not lower. The implementation that Locus writes is not `unsafe` code of the program: a crate that forbids `unsafe` code may declare types and models, and the implementation is not one of the `unsafe impl`s that [`contracts.unsafe-impls`](contracts.md) rejects on a type with proof fields.
