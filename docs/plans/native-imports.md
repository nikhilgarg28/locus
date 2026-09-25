+++
id = "native-imports-plan"
title = "Native Rust imports implementation plan"
group = "Plans"
route = "plans/native-imports.html"
order = 6
+++

# Native Rust imports implementation plan

This follows the locally committed opaque-spec implementation. Plain `import path [as alias]` acquires a physical Rust interface; it does not implement a spec or manufacture evidence. LOC-246 and LOC-249 in [interoperability](../roadmap/interop.md) own this work. Assumed spec realizations remain LOC-247.

## Ordered implementation

1. **Extraction and provenance.** Capture Cargo's actual native compiler invocations for the host's selected features/target. Run the installed rustdoc with those settings, isolating experimental JSON support to that subprocess. Pin and validate the upstream schema, translate into a versioned compiler representation, retain all public item kinds, and distinguish native Rust identity from Locus source identity. Provide a command to inspect that representation.
2. **Binding and diagnostics.** Parse imports, resolve aliases and public module paths, and preserve unsupported entities. Distinguish missing names, extraction/configuration failures and unsupported uses. Never replace an extraction failure with an empty interface. A plain import adds no logical contract, model or effect promise.
3. **A checked execution boundary.** Initially permit safe non-generic Rust functions over machine integers, bool and tuples of those types. Check used signatures against native metadata, including items exposed only under `cfg(doc)`. Preserve ordinary effects and evaluate arguments once. Both interpreters explicitly report that native execution is unavailable; neither invents results or panics. Opaque native values, references, generics, traits, async/unsafe execution and constants remain retained but unavailable until their checking rules are implemented under LOC-44.
4. **Cargo integration and receipts.** Preserve resolved package identities, dependency aliases, effective cfg/features, target and toolchain identity. Avoid persistent metadata cache reuse initially. Detect recursive Cargo/build-script extraction and report a clear error instead of deadlocking. Reject unsupported toolchain/schema/configuration cases. Stage-aware extraction for a host whose Rust build itself needs these generated bindings remains a follow-up under LOC-63.
5. **Acceptance and documentation.** Use real Cargo directories with renamed dependencies, public re-exports, hidden/unsupported entities, feature changes and native signature mismatches. Exercise extraction failures, malformed/new JSON schemas, missing names, source edits, alias collisions, logical/effect restrictions, Rust execution, interpreter refusal and native provenance. Update the manual, package guide, diagnostics, architecture, correctness, formal core and trust inventory. Run focused checks and the full extended gate, then commit separately without merging or pushing.

## Success criteria

A supported call reaches the original Rust function and makes no behavioral claim beyond its physical signature. Unsupported entities remain inspectable and produce capability errors when used. Feature/target changes cannot silently reuse an old interface. A rustdoc-only declaration cannot become a callable native symbol. Extraction works with the installed stable toolchain without installing nightly; unsupported rustdoc format versions fail with an actionable version message. The current implementation and remaining native-type/build integration limits must be stated explicitly rather than hidden behind successful parsing.


## Delivered slice

LOC-246 and LOC-249 are complete for the boundary described above. The full extended gate passed; the standard suite exceeded its advisory timing target. Eleven focused import regressions and the existing compiler/package suites exercise this implementation. The [current manual](../spec/19-native-imports.md) is authoritative; broader native types, sysroot/external metadata loading and staged build-script extraction remain explicit follow-ups.
