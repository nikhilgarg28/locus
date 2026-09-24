+++
id = "packages"
title = "Build and share a Locus component"
group = "Now"
route = "packages.html"
order = 42
+++

# Build and share a Locus component

Locus modules use familiar Rust paths and visibility. A component can be a directory beside your Rust code, or a reusable library shipped inside a Cargo package. [Modules and packages](spec/17-modules.md) specifies the rules; complete executable producer/consumer fixtures live in [tests/fixtures/packages](../tests/fixtures/packages).

## Start with one component

Create `locus/export.lc`. Its public declarations or `pub use` statements select the Rust API. It may load private implementation files with `mod implementation;`. Other `.lc` files are ignored unless declared.

~~~sh prose shell-commands
locus build locus/ --out-dir target/locus --name checked --offline
~~~

This writes `target/locus/checked.rs` and `checked.locus.json`. An arbitrary filename works too: pass the file explicitly. A standalone directory needs no Cargo manifest. If a manifest exists above the input, it owns dependency resolution.

The legacy `build files... --out crate-directory` command still produces a standalone flat-file crate. New module/package work should use `--out-dir`; the project driver also accepts a directory or module entry with `--out`. Legacy `--library` inclusion is not a package import and cannot be combined with module builds.

## Use build.rs

The compiler crate exposes `locus::project::Build`. Until a compiler release is published, add this repository as a path build dependency:

~~~toml prose host-cargo-configuration
[build-dependencies]
locus = { path = "../locus" }
~~~

~~~rust prose build-script-covered-by-tests-packages
fn main() {
    locus::project::Build::new("locus/export.lc")
        .name("checked")
        .offline(true)
        .generate()
        .expect("Locus verification failed")
        .cargo_rerun_directives();
}
~~~

Include the result where its Rust API should appear:

~~~rust prose host-inclusion-covered-by-tests-packages
pub mod checked {
    include!(concat!(env!("OUT_DIR"), "/checked.rs"));
}
~~~

Build another independent entry with another output name for another host module. To share private helpers and types across several public modules, put those modules under one export entry and generate them together. Two separate builds intentionally have separate nominal identities. When importing a dependency, Locus rejects a source type exposed by two independently generated targets, including types reached through signatures. Aliases within one target are fine.

`Build.cargo` exposes manifest, offline/locked, features, default-feature, and target options. Use the same feature and target selection as the host Cargo invocation. This round does not add Locus `cfg` syntax or infer every Cargo feature environment in a build script. `cargo locus` is not required.

## Publish a library interface

A reusable library has two interfaces:

- `lib.lc` exposes Locus declarations, including propositions, models, and theorems.
- `export.lc` exposes an ordinary Rust API. It typically contains `pub use crate::implementation::...` statements.

Configure both in the **same host Cargo.toml**, once per Cargo package:

~~~toml prose package-metadata-covered-by-tests-packages
[package.metadata.locus]
lib = "locus/lib.lc"

[[package.metadata.locus.targets]]
name = "checked"
entry = "locus/export.lc"
rust-module = "checked"
~~~

`rust-module` describes where the host includes that generated interface; Locus does not rewrite `src/lib.rs`. Each entry may have any filename. With a library configured, `crate::` in export entries refers to that library root. Prefer re-exports over declaring the same source files again in a second module tree.

Include the `.lc` sources and `Locus.lock` in Cargo's package files. `cargo package --list` should show them. Keep build.rs and the Rust inclusion module too. Do not ship a producer's `OUT_DIR`. A consumer must be able to compile the extracted package without access to the producer's checkout or build directory.

## Import a dependency

The consumer uses an ordinary Cargo dependency, including renaming when useful:

~~~toml prose cargo-dependency-example
[dependencies]
verified = { package = "verified_collections", version = "0.1" }
~~~

Its Locus code imports names from the dependency's library root:

~~~rust prose dependency-example-covered-by-tests-packages
use verified::runtime::Token;
use verified::theorems::reflexive;

pub fn round_trip(value: Token) -> Token {
    let byte = value.value();
    let proof = reflexive(byte as Int);
    value
}
~~~

Locus invokes `cargo metadata` and follows the alias `verified` to the resolved package. The example's generated Rust uses `::verified::checked::Token`, so Rust can pass a token from the original crate directly. The theorem disappears after checking. Dependency source is checked again; it is not accepted merely because a package claims to contain a theorem.

A runtime dependency must actually compile and include its declared generated interface. As with the host build, Cargo/build-script wiring, rustc, and the compiled dependency are part of the build trust boundary. Metadata alone does not certify arbitrary handwritten Rust behind a matching name. Receipts make the intended inputs and outputs reviewable; they do not establish what a malicious build script compiled.

Current limits include generic runtime ABI exports, proof-bearing runtime functions across packages, and specialized collection-enum ABI mappings. Expose a concrete runtime wrapper or keep the logical API in the Locus library. General trait support is a later project.

## Review and reproduce a build

~~~sh prose shell-commands
locus check locus/export.lc --offline
locus build locus/export.lc --out-dir target/locus --name checked --offline
locus build locus/export.lc --out-dir target/locus --name checked --offline --check-receipt
~~~

The final command reports `current` or `stale`. Rebuilding checks the program and regenerates stale output. A receipt hashes source, dependency manifests and lockfile selection, compiler identity, configuration, and generated bytes. It is an audit/cache record, separate from Cargo.lock and the kernel-checked certificates in Locus.lock.

Generated files are ordinary Rust and can be inspected. Keep them under `target`/`OUT_DIR`; changing their visibility or bodies is changing the program that was verified. Source control normally contains Locus sources, host wiring, Cargo.lock where appropriate, and Locus.lock.
