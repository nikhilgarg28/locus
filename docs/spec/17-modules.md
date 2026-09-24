+++
id = "language-modules"
title = "Modules and packages"
group = "Now"
spec_chapter = 1
order = 112
route = "specification/modules.html"
description = "Source namespaces, Cargo dependencies, and checked Rust export entries."
+++

# Modules and packages

<!-- spec: 1.28:1 informative -->
A Locus compilation starts at one source entry. Modules organize that program; Cargo supplies package versions and dependency paths. A separate export entry chooses the interface that Rust may call. See the [package guide](../packages.md) for complete directory layouts.

## Source files

<!-- spec: 1.28:2 legality-rule -->
An explicit file is the entry regardless of its name. A directory selects exactly `export.lc` in that directory. `mod child;` loads either `child.lc` or `child/mod.lc`; both existing is an error. `mod child { ... }` defines an inline module. Children of either form live under `child/`.

<!-- spec: 1.28:3 legality-rule -->
Only declared module files are loaded. An unrelated file, even one containing invalid syntax, has no effect. Every loaded declaration is checked, including unused declarations, subject to the existing restriction that generic bodies are checked when instantiated. Inclusion cycles, missing files, excessive nesting, and source-size limits are errors.

<!-- spec: 1.28:4 example -->
~~~locus prose multi-file-example-covered-by-tests-modules
// export.lc
mod arithmetic;
pub use arithmetic::increment;

// arithmetic.lc
pub fn increment(n: u8) -> u8 { n.wrapping_add(1) }
~~~

## Names and visibility

<!-- spec: 1.28:5 legality-rule -->
Names resolve within their module, with separate type and value namespaces. `crate::`, `self::`, and `super::` select lexical roots or ancestors. `use` supports explicit paths, aliases, grouped imports, and forward re-exports. Duplicate bindings, unresolved import cycles, and cyclic public module re-exports are errors. Imports name modules or declarations; glob imports and importing individual associated members or enum variants are not supported.

<!-- spec: 1.28:6 legality-rule -->
Items and struct fields are private to their module and descendants by default. `pub`, `pub(crate)`, `pub(super)`, `pub(self)`, and `pub(in ancestor)` widen access to their stated scope. A restricted path must name a lexical ancestor. Imports cannot widen the target declaration's visibility. Field reads, writes, construction, borrows, logical observations, method calls, and associated constants respect the same boundary.

<!-- spec: 1.28:7 example -->
~~~locus check
mod implementation {
    pub struct Counter { value: u8 }
    impl Counter {
        pub fn new(value: u8) -> Counter { Counter { value } }
        pub fn get(&self) -> u8 { self.value }
    }
}
pub use implementation::Counter;
~~~

<!-- spec: 1.28:8 legality-rule -->
The prefixes `__locus_` and `LocusM` are reserved in module sources for compiler identities. File-level promises apply to that file's functions and methods. Attributes on `mod` and `use` are rejected rather than silently ignored.

## Cargo packages

<!-- spec: 1.28:9 legality-rule -->
An explicit `--manifest-path` selects a Cargo package. Otherwise the project driver searches upward from the entry's directory for `Cargo.toml`. There is no `Locus.toml`. Cargo metadata supplies resolved package IDs, dependency aliases, features, target selection, and source directories; Locus does not guess registry cache paths.

<!-- spec: 1.28:10 legality-rule -->
`[package.metadata.locus]` may declare a `lib` source path and a `targets` array. Each target has `name`, `entry`, and optional `rust-module` (default: `name`; empty means the Rust crate root). Paths are relative to the package and cannot escape it. Unknown metadata keys are errors. The library is the package's Locus `crate::` root; a distinct export entry is checked as its child. Without a library, the selected entry is its own root.

<!-- spec: 1.28:11 legality-rule -->
A normal Cargo dependency with Locus library metadata may be imported by its Cargo alias. Each package has its own root, private namespace, and nominal identities. Dependency Locus declarations are checked in the importing compilation; metadata and stored certificates never establish a theorem by themselves. Build and dev dependencies do not become ordinary Locus imports.

<!-- spec: 1.28:12 legality-rule -->
Imported runtime items must have a reachable Rust export in a declared dependency target. Generated calls and nominal types refer to that dependency's Rust path, rather than a second generated implementation. An implementation targeting a named user type must belong to that type's Cargo package. A dependency's nominal type may be reachable from only one independently generated target; aliases within that target and re-exports of an existing dependency type preserve identity. General Rust trait import, generic runtime ABI export, and arbitrary unannotated Rust declarations are not supported.

## Rust export entries

<!-- spec: 1.28:13 legality-rule -->
The selected entry's public items and re-exports define the Rust interface. Public modules expose their public contents recursively. All reachable function inputs and results, public fields, enum payloads, callback signatures, public inherent methods, and public associated constants must be exportable. Logical positions are rejected in these interfaces. A physical struct may contain private logical fields; then its state remains private and Rust uses checked methods.

<!-- spec: 1.28:14 dynamic-semantics -->
Required executable helpers are emitted inside a private implementation module. Locus visibility alone does not make them callable from handwritten Rust, including Rust in the same host crate. The public facade re-exports only validated items. Separate entry builds are independent components; use one export entry with multiple public modules when interfaces must share private implementation or nominal types.

## Generated artifacts

<!-- spec: 1.28:15 dynamic-semantics -->
`locus build ENTRY --out-dir DIR [--name NAME]` emits `NAME.rs` and `NAME.locus.json` (`NAME` defaults to `locus`). The library API uses the same checking pipeline and defaults to Cargo's `OUT_DIR` when no output directory is supplied. Existing unrelated output files are not overwritten. Successful regeneration preserves identical file bytes.

<!-- spec: 1.28:16 dynamic-semantics -->
A build receipt records compiler identity, configuration, input hashes, resolved Cargo selection, and emitted-file hashes. Receipt validation detects changed or missing material. Generation always checks proofs; a receipt is neither proof evidence nor a defense against deliberately replacing the build tool or both artifacts and receipt. Generated Rust and receipts normally remain build artifacts.

<!-- spec: 1.28:17 dynamic-semantics -->
Project proof storage belongs to the host package's `Locus.lock`, keyed by the relative entry path; standalone projects use the entry's directory. Dependency sources are read without writing their lockfiles. `check` may update the host proof store, while build-script generation reads it without modifying sources. Locked proof checking rejects missing or stale certificates. Cargo's dependency lockfile is separate.
