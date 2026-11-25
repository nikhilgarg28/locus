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

Locus's macros (`logic!`, `prop!`, `prove!`), its `verified` and `unverified` attributes, its `Logical` derive, its compiler-controlled `Snapshot` trait and ordinary library `View` trait, and the logical types and functions of its standard library are ordinary names, found the way the names in Rust's standard prelude are. `Opaque<T>` is a library wrapper with a language-defined unit snapshot; no field attribute changes capture.

r[compat.names.shadowable]
A program's own definitions and imports take precedence over Locus's names, as they would over the standard prelude's, including a glob-imported macro with the same name as one of Locus's.

r[compat.names.no-methods]
Traits among Locus's names have no methods, associated functions, or associated constants that a program can name, so that having them in scope never changes what a method call or path in Rust code resolves to. A logic function that such a trait declares, as `View` declares `view`, is not an item that Rust code can name: what rustc is given for it has a reserved name ([`compat.names.reserved`](#locus-names)).

r[compat.names.macros-expand]
Locus's macros are expanded like any other macro, wherever rustc expands macros: directly, inside another macro's input, in a macro's output, and under another name. A Locus macro name that is never expanded, as inside `stringify!`, is only tokens.

r[compat.names.reserved]
A program does not write an identifier that begins with `__logic_` or `__locus_`. Locus reserves both prefixes for the names that it writes for rustc and that must be the same in every crate, such as the name under which a trait holds what rustc is given for a logic function. A source that writes such an identifier is rejected, wherever it writes it: as the name of an item, a field, a parameter, or a local, in a path, and among the tokens of a macro's input. An identifier that only resembles a prefix, such as `__logic` or `_locus_x`, is not reserved.

r[compat.names.model-impls]
Only compiler lowering and designated library support implement `Snapshot`. In generated Rust it is an `unsafe` trait; a user-written implementation, safe or unsafe and in any crate, is rejected by the driver. Generated implementations are not unsafe code authored by the program, so `#![forbid(unsafe_code)]` remains compatible with ordinary declarations. `View` is separate: a program writes its ordinary safe implementation under normal trait and visibility rules, with no same-file requirement and no compiler-created default. No `derive(Model)` or source field attribute changes snapshot behavior.
