+++
summary = "Modules, packages and deeper integration with Cargo and the Rust ecosystem."
id = "p52"
name = "Modules, packages and Rust integration"
status = "planned"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/interop.html"
order = 5
kind = "project"
+++

# Modules, packages and Rust integration

The compiler already emits readable Rust crates, enforces a protected export boundary and supports reviewed trusted Rust contracts. Remaining integration starts with modules/imports ([LOC-41](interop.md#LOC-41)), then packages/shared crate roots ([LOC-42](interop.md#LOC-42)), reviewed header separation ([LOC-39](interop.md#LOC-39)), features ([LOC-43](interop.md#LOC-43)) and Cargo integration ([LOC-63](interop.md#LOC-63)). The current --library mechanism is ordered source inclusion, not a package system. Broader Rust types/traits and std ABI mappings remain explicit work in [LOC-44](interop.md#LOC-44).

<a id="LOC-39"></a>
## LOC-39 · Reviewed headers and implementation separation
<!-- task: {"id": "t47", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: a compiler-checked boundary between a reviewed contract/header and its implementation, including dependency/signature hashes and diagnostics for disagreement. Export visibility exists ([LOC-183](core-build.md#LOC-183)), but files loaded with --library share one namespace and are not this feature. Depends on module organization ([LOC-41](interop.md#LOC-41)).

<a id="LOC-41"></a>
## LOC-41 · Modules, imports and cross-file resolution
<!-- task: {"id": "t49", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

--library provides explicit ordered compilation-unit inclusion (tests/reconcile_library_cli.rs), not modules or imports. Remaining: module namespaces, use paths, visibility across modules and dependency handling. This is the prerequisite for packages, shared crate-root lockfiles and header separation.

<a id="LOC-42"></a>
## LOC-42 · Packages and crate-root proof storage
<!-- task: {"id": "t50", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: package roots/dependency manifests and multi-file checking. P12 currently writes one Locus.lock per source directory; a single shared crate-root lockfile requires an actual crate/module input model. Coordinate [LOC-41](interop.md#LOC-41), [LOC-63](interop.md#LOC-63) and [LOC-232](process.md#LOC-232); do not reopen completed single-file lockfile migration.

<a id="LOC-43"></a>
## LOC-43 · Conditional compilation and features
<!-- task: {"id": "t51", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: supported cfg/feature syntax and checking of selected configurations, including proof and generated-Rust agreement. Preview lifecycle flags are compiler development gates ([LOC-199](process.md#LOC-199)), not user Cargo features. Depends on package/configuration semantics ([LOC-42](interop.md#LOC-42)).

<a id="LOC-44"></a>
## LOC-44 · Broader Rust ecosystem interoperability
<!-- task: {"id": "t53", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Readable generated crates and protected exports are delivered ([LOC-183](core-build.md#LOC-183)); explicit trusted Rust contracts, reasons and audit are delivered ([LOC-229](reconciliation.md#LOC-229), [LOC-196](process.md#LOC-196)). Remaining: real crate dependencies/types/traits, well-specified ABI mappings, build integration and cross-crate contracts. In particular current Option/Result are generated specialized enums, not std ABI aliases. Coordinate [LOC-21](generics.md#LOC-21), [LOC-41](interop.md#LOC-41) and [LOC-63](interop.md#LOC-63).

<a id="LOC-63"></a>
## LOC-63 · Cargo and build.rs integration
<!-- task: {"id": "t78", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

locus build emits a Rust crate today; no dedicated Cargo subcommand/dependency build integration is claimed. Remaining: deterministic build-script/subcommand support, dependencies, caching, source diagnostics and proof-lock ownership. Depends on modules/packages ([LOC-41](interop.md#LOC-41), [LOC-42](interop.md#LOC-42)); distinct from this repository's own provenance build.rs.
