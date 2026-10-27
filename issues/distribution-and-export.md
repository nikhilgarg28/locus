# Distribution and the export boundary

- **Kind:** decision
- **Status:** open
- **Blocks:** Publishing any Locus crate, and Locus crates depending on each other through a registry

## Question

How Locus crates are published and used, under the two modes of RFC 0001 (`design/rfcs/0001-compiler-pipeline.md`, proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10)):
- **Build mode,** the default, checks every crate that depends on a Locus crate, from its sources. Its checking and runtime programs are one lowered text compiled against two versions of Locus's library.
- **Export mode** erases each `.lc` file to readable `.rs` written beside it, for Rust users. `pub(locus)` marks what only Locus crates see.
- **Rules:** `export.plain-interface`, `export.locus-visibility`

This covers:
- **Package contents.**
  - A Locus crate published for Locus users includes its `.lc` files. An exported crate also includes its exported `.rs` files.
  - How an exported package's manifest names its exported crate root. Build mode needs `[lib] path = "src/lib.lc"`, which Cargo accepts (tested), finding no target when `src/lib.rs` is missing. Rust users need the exported `src/lib.rs`.
- **Recognizing Locus crates** among the dependencies in build mode.
- **Interfaces across crates.** RFC 0001 now gives each checked crate an *interface*, its contracts, ghost signatures, invariants, and models in Locus's terms, which its dependents are checked against. Open: where it lives, whether it is published or rebuilt from sources, and how stored proofs travel with it ([#13](proof-search-and-storage.md)).
- **Manifests.** RFC 0001 gives every result a manifest of what it relies on. Open: whether a published crate carries the manifests of its checks, so that a user can audit what was claimed without re-checking.
- **Verifying exported files.**
  - `cargo locus export --check` in a publisher's CI.
  - A command that checks a downloaded package's `.rs` files against its `.lc` sources, narrowing the trust assumption `assumed.export.files`.
  - Export mode already validates the exported program's typed bodies against the checked ones before writing it.
- **Configurations:** how an exported crate declares the configurations it is checked in, likely under `[package.metadata.locus]`. Corner cases are [#9](cfg-corner-cases.md).
- **Dependency versions and features** for exported crates, for example exact versions for Locus dependencies (`assumed.export.dependencies`).
- **Generated export adapters.** Wrappers with signatures you can review, that check a precondition at run time or drop a postcondition, so that authors need not write them by hand ([#18](external-review-ideas.md), item 3).
- **Traits with contracts at the export boundary,** together with [#12](traits-on-logical-types.md).
- **rustfmt:** `cargo fmt` reformats exported files, which makes them stale.
  - rustfmt skips a module whose `mod` declaration is `#[rustfmt::skip]`, and erasure adds that mark in `.lc` parents (tested).
  - The inner attribute `#[rustfmt::skip]` is unstable (E0658 on stable 1.97).
  - Open: an exported crate root, and exported files declared from plain Rust.

RFC 0001 fixes the principles:
- contracts are visible only to code that Locus checks;
- an exported interface is plain Rust, with no proofs, logical values, or contracts;
- an exported program refuses configurations that were not checked;
- the program that runs is validated against the program that was checked.

This RFC fixes the details.

## What it blocks

Publishing any Locus crate, and Locus crates depending on each other through a registry.

## Options

To be laid out in the RFC.

## Log

### 2026-10-25, opened on GitHub as #7

Moved into the repository from [GitHub issue #7](https://github.com/nikhilgarg28/locus/issues/7) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.
