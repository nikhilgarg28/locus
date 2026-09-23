+++
summary = "Editor services, source-language tests and tools for everyday Locus development."
id = "p74"
name = "Developer tooling"
status = "planned"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/ecosystem-tooling.html"
order = 7
kind = "project"
+++

# Developer tooling

Versioned JSON diagnostics, locus explain, audit, syntax highlighting and the specification gates are implemented. The remaining authoring experience consists of source-language tests ([LOC-40](ecosystem-tooling.md#LOC-40)), an LSP ([LOC-62](ecosystem-tooling.md#LOC-62)), the broader workflow umbrella ([LOC-60](ecosystem-tooling.md#LOC-60)) and the optional Rust-macro experiment ([LOC-64](ecosystem-tooling.md#LOC-64)). Cargo integration is owned by [LOC-63](interop.md#LOC-63) in the modules/Rust project to avoid two build-tool backlogs.

<a id="LOC-40"></a>
## LOC-40 · User-authored Locus tests
<!-- task: {"id": "t48", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Repository Rust/corpus tests are complete infrastructure, not a source-language test facility. Remaining: test declarations/attributes, execution and proof checking of user tests, generated Rust integration and failure reporting. Coordinate Cargo tooling ([LOC-63](interop.md#LOC-63)).

<a id="LOC-60"></a>
## LOC-60 · Developer workflow and editor integration
<!-- task: {"id": "t75", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

The CLI, structured diagnostics/explanations and syntax highlighters are delivered ([LOC-200](process.md#LOC-200); editors/). Remaining: cohesive authoring workflows such as formatting, navigation and proof-hole interaction. This umbrella is tracked by concrete editor/test/Cargo tasks [LOC-40](ecosystem-tooling.md#LOC-40), [LOC-62](ecosystem-tooling.md#LOC-62), [LOC-63](interop.md#LOC-63); avoid a duplicate implementation checklist.

<a id="LOC-62"></a>
## LOC-62 · An LSP server
<!-- task: {"id": "t77", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: incremental diagnostics, source mapping, navigation, completions and proof-goal presentation using the versioned diagnostics surface. The existing editor highlighters are not an LSP. Use the checked CLI behavior and avoid a second proof checker.

<a id="LOC-64"></a>
## LOC-64 · A locus! Rust macro on-ramp
<!-- task: {"id": "t79", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining experiment: embed a useful checked Locus subset in a Rust macro or companion tooling, preserving kernel validation, source diagnostics and a clear trusted boundary. The compiler's macro-like built-in forms are not a native Rust procedural macro implementation.
