+++
id = "language-native-imports"
title = "Importing Rust"
group = "Now"
spec_chapter = 1
order = 114
route = "specification/native-imports.html"
description = "Inspect a native Rust interface and call its supported physical functions."
+++

# Importing Rust

<!-- spec: 1.30:10 informative -->
`use` names checked Locus declarations. `import` reads a Rust library's public interface. Importing adds no proof contract and does not implement a spec. The [package guide](../packages.md#inspect-and-call-rust) covers commands and build setup.

## Names and visibility

<!-- spec: 1.30:1 syntax -->
Write `import path [as alias];`. The final path component supplies the default binding. The path starts with an active ordinary Cargo dependency alias or `crate` for the owning package's Rust library. Imports have no visibility or attributes; use `pub use` to expose a supported item. `pub use name;` may expose the same privately imported binding. Other duplicate names follow ordinary scope rules.

<!-- spec: 1.30:11 example -->
~~~rust prose cargo-fixture-tests-native-imports
// Cargo.toml gives this Rust dependency the alias `renamed`.
import renamed::echo as native_echo;

pub fn echo(n: u8) -> u8 { native_echo(n) }
~~~

## Retained interfaces

<!-- spec: 1.30:2 normative -->
Extraction retains public modules, types, traits, associated items, constants, macros and functions, including async, unsafe and generic signatures. Every imported entity records Rust provenance and package/item identity. Retention permits inspection, not necessarily use. Types, values and macros retain distinct namespaces. A standalone import of an ambiguous spelling requires importing its containing module instead. Local re-exports preserve the target identity; external references without defining metadata stay explicitly unavailable.

<!-- spec: 1.30:12 informative -->
The compiler-owned interface has its own version and normalized callable signatures. Original rustdoc details remain available for inspection under a separately pinned upstream schema. Private implementation members are not exposed. Documentation-hidden public members are retained.

## Executable calls

<!-- spec: 1.30:3 legality-rule -->
Calls currently accept safe, non-generic Rust free functions with the Rust ABI, using `bool`, `u8`–`u64`, `i8`–`i64`, `usize`, `isize`, and nested tuples of those types by value. Unit is the empty tuple. Native types, standalone reference calls, generic calls, constants, macro expansion, async and unsafe calls remain unavailable. A restricted [trait interface](20-traits.md#rust-interoperability) supports concrete local implementations. Sysroot imports such as `std::vec::Vec` also require a future adapter; the built-in Locus Vec remains separate.
