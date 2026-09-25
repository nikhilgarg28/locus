+++
id = "rust-imports"
title = "Rust imports and native identity"
group = "Vision"
route = "vision/rust-imports.html"
order = 45
+++

# Rust imports and native identity

Plain imports and proof-bearing specs are separate layers. This replaces the earlier proposal in which `import` selected and assumed a spec. Manual opaque specs are the first implementation stage; native imports follow in a separate commit.

## Import the physical interface

```rust
import dependency::module;
import std::vec::Vec as RustVec;
```

The last path component is bound by default. `as` introduces an alias, not a spec realization. Use `pub use` for visibility. Extraction uses the host Cargo environment and rustdoc JSON, then translates into a versioned internal representation. Imported entities carry explicit foreign provenance and their original native identity. Aliases and re-exports must not create new Rust types.

Retain all publicly reachable kinds, including traits, associated items, constants, async and unsafe functions. Importing metadata does not imply Locus can use every feature. An unsupported use must name the native item and explain which capability is missing. A missing member must instead identify the searched native path and configuration. Never turn an extraction error into an empty successful interface.

Import supplies no propositions, proofs, models, purity claims or termination claims. A native call remains executable, with conservative effects and mutation treatment. Native constants must retain their actual values or be explicitly unavailable for logical reasoning; no fabricated values.

## Configuration and extraction

Use Cargo's resolved package IDs and dependency aliases. Key extracted metadata and receipts by compiler/rustdoc identity, target, resolved feature set, relevant configuration, lockfile and source identity. Reject stale or incompatible caches. `cfg(doc)` can expose items unavailable in executable Rust, so imported use also needs validation against the real native compilation context.

Users should not need to install nightly. Any use of experimental JSON output with the installed toolchain must be isolated to extraction, never globally enable unstable host/dependency source, and report unsupported toolchains clearly. Guard the incoming JSON format version and validate structural fields. Preserve unknown metadata only for inspection; never interpret an unknown callable/type shape as an accepted supported interface.

Host-crate introspection must avoid recursively invoking the host build script. Cross-target imports must not accidentally inspect the host architecture or default features. Missing target components, unavailable tools, Cargo resolution failure and rustdoc schema drift need concise actionable errors with underlying tool output available.

## A later assumed realization

```rust
// Proposed separately from plain import:
assume RustVec impl VerifiedVec;
```

This is an audited trust boundary, not native verification. Initially require no proof inputs; compare runtime signatures after omitting output proof slots only. Generated adapters call Rust exactly once and introduce output assumptions after normal return. Panic, abort and divergence produce no proof. Track native path, specification, proposition, package/toolchain/configuration and source location in the audit.

Logical results, extra proof fields and model implementations cannot be invented from native metadata. An invariant-bearing API needs a distinct opaque wrapper and reviewed construction/mutation contracts. Interior mutability, callbacks and shared aliases need explicit observation and invalidation policies before stronger claims are accepted.

## Editing and inspection

Provide inspection of extracted items and why a particular use is unsupported. A later spec-generation command can scaffold an editable file from that representation; it must not overwrite existing user source. No automatic update/merge command is planned initially.

## Acceptance

Use real Cargo packages and directories, renamed dependencies, public re-exports, changed source, feature and target configurations, unknown schema versions, corrupt caches and missing names. Compile supported generated calls in their real Cargo context. Retain unsupported entities for inspection and reject their use deliberately. Broader trait/generic/native operation support is distinct from retaining its metadata. The [roadmap](../roadmap/interop.md#specifications-and-native-imports) owns each implementation task.
