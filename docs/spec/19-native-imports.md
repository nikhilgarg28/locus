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

<!-- spec: 1.30:4 dynamic-semantics -->
A native call evaluates arguments once in source order and calls the original Rust path. Its effects remain even when a surrounding function returns only evidence. Native panic, abort and divergence yield no normal-return result. The two Locus interpreters report unsupported native execution; they do not simulate arbitrary Rust or treat that report as a program panic.

<!-- spec: 1.30:13 example -->
~~~rust prose cargo-fixture-tests-native-imports
import renamed::tick;
pub fn observable() -> @(1 == 1) {
    tick(); // The native side effect remains in generated Rust.
    _
}
~~~

## Errors and native validation

<!-- spec: 1.30:5 normative -->
In source, an unknown native name reports L0513 with the searched path and selected configuration. Using a retained but unavailable item reports L0514 with the missing capability. Extraction failures report L0512; they never create an empty successful import. Unsupported items do not prevent importing an otherwise usable module.

<!-- spec: 1.30:6 legality-rule -->
Every used supported function is checked against Cargo's actual native metadata with a Rust function-pointer signature assertion. A rustdoc-only item, including one exposed by `cfg(doc)`, cannot authorize a call. Used Rust trait interfaces also receive a native implementation probe. These checks establish physical signatures, never behavioral correctness or a model.

## Cargo configuration

<!-- spec: 1.30:7 normative -->
Extraction builds the host with the requested Cargo features, target and dependency selection in an isolated directory. It captures actual compiler invocations rather than guessing dependency features. Receipts include package/toolchain identity, native cfg flags and metadata hashes. Each new load re-extracts; there is no persistent interface cache. Ambiguous host/target library instances fail explicitly. Generated Rust must be compiled in the same Cargo configuration.

<!-- spec: 1.30:14 informative -->
The current host must compile before extraction. A host build script that recursively invokes these imports is rejected; staged generation for that case is deferred. Existing rustc wrapper environment overrides are rejected rather than silently replaced. Dependency-owned imports need a matching ordinary dependency in the host so generated Rust can name the same package. See [remaining interoperability work](../roadmap/interop.md#LOC-44).

## Logical boundary and tools

<!-- spec: 1.30:8 legality-rule -->
Plain imports introduce no evidence, propositions, model implementations, logical definitions or effect promises. Calls are unavailable in logical computation and in functions requiring any current effect promise. The checking and erased IRs independently reject native calls that consume or produce logical or invariant-bearing values.

<!-- spec: 1.30:9 normative -->
`locus import path` inspects the interface; `--json` emits it and `--out FILE` saves it without overwriting unrelated content. Rustc and rustdoc must match. Only the rustdoc child enables experimental JSON output; Cargo and native compilation use the ordinary installed toolchain. Unknown JSON format versions and malformed metadata fail closed with an actionable error. Nightly installation is not required; the current adapter accepts rustdoc JSON format 56.
