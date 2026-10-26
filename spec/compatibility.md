# Compatibility with Rust

Locus is a superset of Rust. This chapter states what that promises: Rust programs keep their meaning, Rust's errors stay Rust's, and Locus's syntax and names never take a meaning away from Rust code. How the compiler keeps these promises is described in [RFC 0001](../design/rfcs/0001-compiler-pipeline.md).

In this chapter, *rustc* means the release of rustc that Locus is pinned to. A program is checked in the configuration being built, or, when its crate is exported, in each of the configurations the crate declares ([Exporting to Rust](export.md)).

## Rust programs

r[compat.superset.same-meaning]
A program that rustc accepts is a Locus program with the same meaning, in every configuration Locus checks: Locus accepts it, and it behaves as rustc's compilation of it does. A `.rs` file renamed to `.lc` is the same program.

r[compat.errors.rustc]
An error that rustc reports in runtime code is reported by Locus as rustc reports it: with rustc's code and message, at the corresponding place in the source.

## Locus syntax

Locus syntax may appear only in `.lc` files. Within them, it is chosen so that no Rust program can be read as Locus.

r[compat.syntax.not-rust]
A Locus syntax form is recognized from tokens alone, without resolving names, and only where rustc rejects those tokens in code it compiles. Tokens that rustc accepts there keep their Rust meaning, even where they resemble Locus syntax. Locus syntax is not recognized inside macro invocations or attribute arguments.

r[compat.syntax.contextual-keywords]
`logic` is a keyword only immediately before `fn`, and `prop` only before the name of a proposition declaration. Everywhere else they are identifiers.

## Locus names

Locus's macros (`logic!`, `prop!`, `prove!`, `fold!`, `unfold!`, `rewrite!`), its `Logical` derive, its `Model` trait, and the logical types and functions of its standard library are ordinary names, found the way the names in Rust's standard prelude are.

r[compat.names.shadowable]
A program's own definitions and imports take precedence over Locus's names, as they would over the standard prelude's, including a glob-imported macro with the same name as one of Locus's.

r[compat.names.no-methods]
Traits among Locus's names have no methods, associated functions, or associated constants, so that having them in scope never changes what a method call or path in Rust code resolves to.

r[compat.names.direct-only]
Locus's macros take effect only where they are invoked directly, by their own names, outside any other macro's input or output. An invocation of one of Locus's macros that rustc expands anywhere else, such as inside another macro's input, in a macro's output, or under another name, is rejected. A Locus macro name that is never expanded, as inside `stringify!`, is only tokens.
