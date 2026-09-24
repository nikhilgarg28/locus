+++
summary = "Modules, packages and deeper integration with Cargo and the Rust ecosystem."
id = "p52"
name = "Modules, packages and Rust integration"
status = "active"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-24T07:50:46.312429+00:00"
route = "roadmap/interop.html"
order = 5
kind = "project"
+++

# Modules, packages and Rust integration

## Current project: reusable Locus components

This round implements source modules, explicit Rust export roots, Cargo package discovery, checked dependency interfaces, generated build receipts, and build-script integration. Traits and `cargo locus` are deferred. The standalone compiler remains the entry point. This work is isolated from the canonical-model/arithmetic project; LOC-233 through LOC-236 remain reserved for that project.

### Accepted design

- A file selects that entry regardless of its name. A directory selects exactly its `export.lc`; missing entries are errors. Only declared modules and Cargo dependencies with Locus library metadata are loaded. Loaded declarations are checked even when unused, subject to the existing generic-template checking limitation.
- Use Rust-style modules, paths, imports, aliases, re-exports and visibility. Resolve names and enforce privacy before elaboration. Diagnostics retain original files/spans. Ambiguous bindings, cycles and unsupported forms fail explicitly.
- Source `pub` means Locus visibility. The selected export module's reachable interface determines Rust exposure. Check inputs, outputs, public fields, enum payloads, reachable nominal types and public inherent methods. Logical positions cannot be public in Rust. Private invariant fields require a protected construction/mutation boundary.
- Runtime helpers may remain callable within verified code without becoming callable by handwritten Rust, even in the same crate. Independent components may emit separate files. Components sharing internal runtime code require coordinated generation, never a public proof-taking backdoor.
- Use one host `Cargo.toml` per package. Its Locus metadata identifies library/export roots and generation targets. Cargo resolves dependencies and aliases; Locus checks packaged source/proof material. Metadata is not a theorem authority. Dependency runtime types retain their existing Rust identity.
- Cross-package runtime interfaces with proof parameters/results, generic Rust ABI exports, general traits and conditional Locus source syntax remain explicit limits. Features/target selections used for dependency discovery must match generated Rust's configuration.
- Generated output belongs in a build directory. Receipts fingerprint sources, configuration, compiler, dependencies and emitted bytes. Rebuild or validate before reuse. A receipt is not a proof and cannot prevent deliberate edits to both output and receipt.
- The build-script library API supports ordinary Cargo consumers. Packages contain reusable `.lc` sources and proof material, not a producer's generated `OUT_DIR`.

### Implementation order and success criteria

1. **LOC-41:** real directory fixtures cover file/directory roots, external/inline modules, aliases/re-exports, namespaces, original diagnostic spans, ignored malformed files, private items and malformed graphs.
2. **LOC-237:** export validation follows complete exposure paths. Warning-denied valid Rust consumers run; hostile same-crate/downstream callers cannot forge evidence, call hidden helpers or mutate private invariants.
3. **LOC-42:** test host manifests, path/renamed dependencies, workspace ownership, theorem reuse, dependency identity and missing metadata. Proof storage is owned by the selected package/root.
4. **LOC-63:** standalone and build-script entry points use the same pipeline. Real Cargo hosts include multiple generated components and execute them. No Cargo subcommand is required.
5. **LOC-238:** test deterministic rebuilds, stale/tampered/missing artifacts, configuration and dependency changes, safe output ownership and consumers of extracted Cargo packages.
6. **LOC-239:** update implemented manual rules, guides and diagnostics with permanent test citations. Run focused filesystem fixtures, executable docs, differential tests and the complete compiler/site gate. Mark completion only with passing evidence and explicit remaining limits.

Tests create or copy real directory trees and exercise the public API/CLI. String-only parser tests supplement them; they cannot establish discovery, privacy, Cargo resolution, packaging or receipt correctness. Negative tests check diagnostic category and source location.

The module/package implementation lives in `src/project`, with the [language rules](../spec/17-modules.md) and [package guide](../packages.md) as its public contract. Remaining integration covers reviewed trait/header contracts ([LOC-39](#LOC-39)), source configuration and Cargo feature inference ([LOC-43](#LOC-43)), and broader Rust ABI/type/trait support ([LOC-44](#LOC-44)). Legacy `--library` remains ordered flat-source inclusion.

<a id="LOC-39"></a>
## LOC-39 · Reviewed headers and implementation separation
<!-- task: {"id": "t47", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: a compiler-checked boundary between a reviewed contract/header and its implementation, including dependency/signature hashes and diagnostics for disagreement. Export visibility exists ([LOC-183](core-build.md#LOC-183)), but files loaded with --library share one namespace and are not this feature. Depends on module organization ([LOC-41](interop.md#LOC-41)).

<a id="LOC-41"></a>
## LOC-41 · Modules, imports and cross-file resolution
<!-- task: {"id": "t49", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented lexical namespaces, declared-file loading, grouped imports, aliases/re-exports, restricted visibility, and original-file diagnostics in `src/project/load.rs` and `resolve.rs`. Real directory, privacy, cycle, unused-file and compiled-Rust regressions are in `tests/modules.rs`. Globs and associated-member imports remain explicit syntax limits.

<a id="LOC-42"></a>
## LOC-42 · Packages and crate-root proof storage
<!-- task: {"id": "t50", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented Cargo metadata discovery, package roots and aliases, checked Locus dependency sources, original Rust nominal identities, and a host-package proof lock keyed by entry path. `tests/packages.rs` checks theorem reuse, read-only dependency sources, strict replay and cross-package privacy. Cargo resolution and proof storage remain separate.

<a id="LOC-43"></a>
## LOC-43 · Conditional compilation and features
<!-- task: {"id": "t51", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: supported cfg/feature syntax and checking of selected configurations, including proof and generated-Rust agreement. Preview lifecycle flags are compiler development gates ([LOC-199](process.md#LOC-199)), not user Cargo features. Depends on package/configuration semantics ([LOC-42](interop.md#LOC-42)).

<a id="LOC-44"></a>
## LOC-44 · Broader Rust ecosystem interoperability
<!-- task: {"id": "t53", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Readable generated crates and protected exports are delivered ([LOC-183](core-build.md#LOC-183)); explicit trusted Rust contracts, reasons and audit are delivered ([LOC-229](reconciliation.md#LOC-229), [LOC-196](process.md#LOC-196)). Module/package builds now preserve concrete dependency identities. Remaining: arbitrary native Rust types/traits, generic cross-package ABI mappings and reviewed cross-crate contracts. In particular current Option/Result are generated specialized enums, not std ABI aliases. The documentation audit also found that `Option<PrivateStruct>` can expose its private payload type through a generated public variant and trigger rustc’s `private_interfaces` warning. Make specialization visibility respect the source boundary and add a warning-denied regression; the current examples use a public type with private invariant-bearing fields. Coordinate [LOC-21](generics.md#LOC-21), [LOC-41](interop.md#LOC-41) and [LOC-63](interop.md#LOC-63).

<a id="LOC-63"></a>
## LOC-63 · Cargo and build.rs integration
<!-- task: {"id": "t78", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented standalone module builds and the shared `locus::project::Build` API for build.rs, including OUT_DIR generation and rerun directives. Real Cargo hosts compile and execute a producer and consumer. Generation is deterministic; receipts validate artifacts but never skip proof checking. `cargo locus` and automatic inference of all host feature settings remain deferred.

<a id="LOC-237"></a>
## LOC-237 · Reachable Rust interfaces and export diagnostics
<!-- task: {"id": "interop-237", "status": "done", "priority": 3} -->

Implemented reachable-interface validation and a private Rust backend with a public facade. Tests reject logical inputs/results, nested public leaks and incompatible public methods, then compile safe consumers and hostile same-crate accesses. Generic ABI exports and specialized collection-enum dependency ABI fail explicitly.

<a id="LOC-238"></a>
## LOC-238 · Reproducible build receipts and Cargo publication
<!-- task: {"id": "interop-238", "status": "done", "priority": 3} -->

Implemented SHA-256 receipts, compiler/configuration/Cargo selection provenance, deterministic regeneration and output ownership checks. Tests modify inputs and artifacts, preserve handwritten files, and build a consumer from an extracted Cargo archive after deleting the producer checkout. Receipts remain advisory build artifacts, separate from checked proof certificates.

<a id="LOC-239"></a>
## LOC-239 · Module/package acceptance and documentation
<!-- task: {"id": "interop-239", "status": "done", "priority": 3} -->

Implemented the module/package manual, package/build guide, diagnostic explanations, and checked examples. Acceptance includes qualified types, independent-target identity, generic-dependency ABI rejection, canonical and derived models across modules/packages, associated constants, and checked arithmetic after proof erasure. The filesystem, Cargo, diagnostic and resource-limit suites cover these boundaries; the extended compiler/site gate is the integration requirement. See the freshness-aware [generated status](../generated-status.md) for measured results. Traits remain the next project.
