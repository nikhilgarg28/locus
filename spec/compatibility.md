# Compatibility with Rust

Locus is a superset of Rust. This chapter states what that promises: Rust programs keep their meaning, Rust's errors stay Rust's, and Locus syntax never collides with Rust syntax. How the compiler keeps these promises is described in [RFC 0001](../design/rfcs/0001-compiler-pipeline.md).

## Rust programs

r[compat.superset.same-meaning]
A program that rustc accepts is a Locus program with the same meaning: Locus accepts it, and it behaves as rustc's compilation of it does. A `.rs` file renamed to `.lc` is the same program.

r[compat.errors.rustc]
An error that rustc reports in runtime code is reported by Locus as rustc reports it: with rustc's code and message, at the corresponding place in the source.

## Locus syntax

Locus syntax may appear only in `.lc` files. Within them, it is chosen so that no Rust program can be read as Locus.

r[compat.syntax.not-rust]
Locus syntax is recognized from tokens alone, without resolving names, and only where those tokens are not valid Rust. Tokens that are valid Rust keep their Rust meaning, even where they resemble Locus syntax.

r[compat.syntax.contextual-keywords]
`logic` and `prop` are keywords only where they introduce a Locus item: `logic` immediately before `fn`, `struct`, or `enum`, and `prop` before the name of a proposition declaration. Everywhere else they are identifiers.

r[compat.macros.none-built-in]
Locus defines no built-in macros. Every macro invocation means what it means in Rust.
