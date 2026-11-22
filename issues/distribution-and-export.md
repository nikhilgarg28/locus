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
- **Interfaces across crates.** RFC 0001 now gives each verified crate an *interface*, its contracts, ghost signatures, invariants, and models in Locus's terms, which its dependents are checked against. Open: where it lives, whether it is published or rebuilt from sources, and how stored proofs travel with it ([#13](proof-search-and-storage.md)).
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
- contracts are visible only to code that Locus verifies;
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

### 2026-11-09, fourth review of PR #10

RFC 0002's decision 52 adds a question for this issue: how a project names the unchecked crates it trusts to be sound. The RFC says only that the list is in the manifest, that the standard library and crates with no `unsafe` code need no entry, and that each result's manifest names the listed crates it relied on. Still to decide here: the list's form, whether a workspace shares one, and whether a crate published for Locus users carries its own.

### 2026-11-09, conversation with the owner about PR #24

RFC 0002's decision 54 changes what the list of trusted crates does. A crate with `unsafe` code that is not on it is no longer handled by tracking what it was given: a function that calls into it is unverified, and is itself what a result lists. That reading of decision 52 is the reviewer's proposal and still for the owner. A manifest also lists each unverified function with `unsafe` code, and each `unsafe impl`, of a Locus crate that a result passes through.

### 2026-11-16, proposal made at the owner's request

The owner asked for an audit of the decisions that are still open, with a proposal for each. This is the proposal for this issue. Nothing in it is decided.

**New with RFC 0002's decision 81:** what an exported crate does with the types of `locus::std`, and what a program does that already has a crate named `locus`.

Options for exported crates:
1. **Depend on Locus's library as an ordinary Rust crate.** Its runtime part is small, has no proofs after erasure, and is itself exported Rust. The exported crate names `locus::std::vec::Vec` as it names any dependency's type.
2. **Rewrite Locus's types to the standard library's when exporting,** with each wrapper method written out as the standard calls it makes.
3. **Forbid them in exported crates.**

Proposal: option 1, with one rule: what an exported crate shows to other crates uses the standard library's types, converting at the edge, which costs nothing. So a Rust user sees `Vec<u32>` in every signature, and the vision's "no runtime support" still holds in the sense that matters: the dependency is plain Rust with no Locus toolchain. The strongest argument against is that the exported source reads less like what a Rust programmer would write inside function bodies; option 2 fixes that at the price of a rewriting pass that would have to be validated.

For the name: `locus` is reserved in a Locus crate's dependencies, as `std` and `core` are.

For the rest of this issue, the proposal is to decide only what the first build needs, and leave publishing until build mode works: configurations under `[package.metadata.locus]`, the list of trusted crates per workspace, and each published crate carrying the list of assumptions its results rest on.

### 2026-11-22, a public field of a sealed type

`export.plain-interface` said in its lead that nothing other crates can reach exposes logical data, and in its clauses that no reachable field holds any. So `pub struct Pair { pub first: Interval, pub second: Interval }` was refused while a function that returns an `Option<Interval>` was accepted. Shown this among six rules that a review had left unsettled, with the proposal to say "exposes" throughout, the owner answered:

> tentatively confirm the 6 open questions

RFC 0002's decision 92 records it. A field that other crates can reach may have a sealed type, in a struct and in a variant of an enum; it may not expose logical data or be read by a proof field. `export.invariants-closed` and `contracts.invariants.observed` change with it, and the first stage follows. Three examples are added: `export/public-field-of-sealed-type.accept.lc`, `export/public-enum-with-sealed-payload.accept.lc`, and `export/public-sealed-field-read-by-proof.reject.lc`. It stays open to revisit (RFC 0002, item 26).
