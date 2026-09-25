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

## Current extension: proof-returning Rust facades

Keep the checked Locus signature and its ordinary erased implementation. For an exported ordinary function or public inherent method, generate a second, data-only Rust entry that calls that implementation exactly once and projects its result. Proof arguments, logical data results, public invariant-bearing fields, proof-bearing enum payloads and logical callbacks remain forbidden. No import syntax or trait machinery is added here.

### Return projection

A direct proof result becomes `()`. In tuples, remove proof fields recursively; a nonempty tuple containing only proofs disappears as a component. If removing components leaves one component, unwrap that tuple layer; if several remain, retain their order. A tuple with no removed components keeps its arity, including singleton and empty tuples. Preserve physical unit values. Do not project through nominal structs/enums, references, collections, boxes or callbacks: their existing export checks still apply. This avoids changing data identity or runtime discriminants. Non-proof logical results (such as Nat or Prop) still fail export.

The public name selects the projected Rust entry. Locus calls, including calls from other methods, continue to select the original signature with its erased proof positions. The original entry stays private even when its owner type is public. This extension applies to project/module generation; the legacy flat emitter is unchanged. Cross-package runtime proof interfaces retain their existing explicit restriction: a dependency's data-only entry cannot silently substitute for its erased proof ABI.

### Implementation and validation

The delivered work is recorded in [LOC-240](#LOC-240) (projection and emission), [LOC-241](#LOC-241) (behavior and hostile callers), and [LOC-242](#LOC-242) (documentation and complete validation).

## Specifications and native imports

The [specification design](../vision/spec-interfaces.md) and [separate import design](../vision/rust-imports.md) define this work. LOC-39 owns manual interfaces; LOC-44 owns native interoperability. Implement in this order, with an explicit stop before imports:

1. LOC-243: parse concrete module/type spec headers and manual bodies; retain original spans and reject deferred grammar deliberately.
2. LOC-244: completeness, exact signatures, inherited promises, privacy and alias-resistant implementation checks; pass all bodies through existing proof checking.
3. LOC-245: real-directory positive/negative fixtures, compiled Rust and hostile clients, executable documentation and complete validation. Then discuss the import design live.
4. LOC-246: native identity discovery and rustc validation for explicit specs under Cargo configuration. No behavioral assumptions may conceal a signature mismatch.
5. LOC-247: audited output-proof adapters, caller-owned input obligations, result projection and effect preservation.
6. LOC-248: generation-only CLI, inspected editable output, explicit unsupported-member diagnostics; no update command.
7. LOC-249: packaging, changed dependency/configuration provenance, hostile callers, end-to-end Cargo tests and import documentation/gates.
8. LOC-250: broaden manual specs after the concrete subset: elaborated/alpha-renamed matching, split implementations, generics coordinated with LOC-21/LOC-22, transparent structs/enums, proposition interfaces and logical opacity.

Success for manual specs means: an unused unimplemented declaration fails; a false promised proof still fails; signature disagreement cites both locations; implementation helpers cannot leak; aliases cannot bypass type-spec completeness; external implementation files execute after Rust generation; no new trusted proof rule is introduced. Success for imports additionally requires correct native identity and audited conditional assumptions. Those latter claims cannot be marked complete by passing manual-spec tests.

<a id="LOC-39"></a>
## LOC-39 · Reviewed headers and implementation separation
<!-- task: {"id": "t47", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

The concrete implementation is delivered in LOC-243 through LOC-245, with the full design and ordered plan below. Existing source fingerprints cover header inputs. Broader signature equivalence, generic/trait interfaces and logical opacity remain in LOC-250. Module organization (LOC-41) supplies ownership and file loading.

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

Implemented reachable-interface validation and a private Rust backend with a public facade. Tests reject logical inputs, non-proof logical results, nested public leaks and incompatible public methods, then compile safe consumers and hostile same-crate accesses. Generic ABI exports and specialized collection-enum dependency ABI fail explicitly.

<a id="LOC-238"></a>
## LOC-238 · Reproducible build receipts and Cargo publication
<!-- task: {"id": "interop-238", "status": "done", "priority": 3} -->

Implemented SHA-256 receipts, compiler/configuration/Cargo selection provenance, deterministic regeneration and output ownership checks. Tests modify inputs and artifacts, preserve handwritten files, and build a consumer from an extracted Cargo archive after deleting the producer checkout. Receipts remain advisory build artifacts, separate from checked proof certificates.

<a id="LOC-239"></a>
## LOC-239 · Module/package acceptance and documentation
<!-- task: {"id": "interop-239", "status": "done", "priority": 3} -->

Implemented the module/package manual, package/build guide, diagnostic explanations, and checked examples. Acceptance includes qualified types, independent-target identity, generic-dependency ABI rejection, canonical and derived models across modules/packages, associated constants, and checked arithmetic after proof erasure. The filesystem, Cargo, diagnostic and resource-limit suites cover these boundaries; the extended compiler/site gate is the integration requirement. See the freshness-aware [generated status](../generated-status.md) for measured results. Traits remain the next project.

<a id="LOC-240"></a>
## LOC-240 · Define and emit proof-returning Rust facades
<!-- task: {"id": "interop-240", "status": "done", "priority": 3} -->

Implemented the return projection in `src/erased/facade.rs`, selected by export validation and emitted through the existing Rust printer. The original implementation stays private; verified internal calls retain its proof positions. Public wrappers project free-function and inherent-method results, including borrowed and owned values, without duplicating execution. Constants, callbacks and unsupported dependency proof ABIs retain explicit restrictions.

<a id="LOC-241"></a>
## LOC-241 · Exercise facade behavior and hostile callers
<!-- task: {"id": "interop-241", "status": "done", "priority": 3} -->

Implemented six facade integration tests and a real multi-file fixture, with module/package regressions. Coverage includes tuple projection, aliases, receivers, ownership and borrowing, internal proof consumption, control flow, exactly-once mutation, panic-state writes, deterministic builds and receipt invalidation. Rust consumers compile with warnings denied in both overflow modes; hostile same-crate and downstream callers cannot reach private proof implementations. Both interpreters agree with the relevant source behaviors. Logical inputs, public proof fields, unsupported containers/callbacks and dependency proof ABIs remain rejected.

<a id="LOC-242"></a>
## LOC-242 · Document and validate facade exports
<!-- task: {"id": "interop-242", "status": "done", "priority": 3} -->

Updated the manuals, checked examples, package guide, architecture, formal-core obligation, correctness account, trusted-file inventory and L0504 diagnostics. New operative rules have focused citations. The complete extended compiler/site gate passed on 24 September 2026, with 917 tests passing in each of the standard and release suites, including 81,944 generated-program execution cases with no disagreements; the standard suite exceeded its advisory timing target. Desktop and narrow layouts were inspected. This completion-note edit follows that gate, so the generated measurement display is invalidated rather than relabeled fresh. Nominal/container projection, legacy emission, cross-package proof ABIs and imports remain outside this extension.

<a id="LOC-243"></a>
## LOC-243 · Spec grammar and module/type realizations
<!-- task: {"id":"interop-243","status":"done","priority":3} -->

Implemented concrete function/logic-function/constant headers, spec mod/type, inline/file-loaded module implementations and opaque private struct representations. Parser fuzz fragments and a nesting-limit regression cover the new productions.

<a id="LOC-244"></a>
## LOC-244 · Checked interface matching
<!-- task: {"id":"interop-244","status":"done","priority":3} -->

Implemented exact signature checks including evidence, inherited promises, complete unique realizations and rejection of extra public members. Canonical resolution closes alias/cross-module bypasses. The 16 tests in `tests/specifications.rs` cover false outputs, mutation, input obligations, logic mode, duplicates, private fields, file discovery and generated Rust.

<a id="LOC-245"></a>
## LOC-245 · Manual-spec acceptance and documentation
<!-- task: {"id":"interop-245","status":"done","priority":3} -->

Added real directories, two source-aware diagnostic fixtures with JSON/text/explain goldens, executable manual examples and traceability. Updated architecture and trust-boundary documentation; inspected the page at desktop and narrow widths. All regression tests pass; current full-gate measurements belong in the freshness-checked [generated status](../generated-status.md), including the advisory timing target. Import implementation remains pending live review.

<a id="LOC-246"></a>
## LOC-246 · Bind explicit specs to native Rust interfaces
<!-- task: {"id":"interop-246","status":"backlog","priority":3} -->

Implement import path [as spec], native lookup and physical signature checks, including host visibility, constants, ownership and dependency aliases. Unsupported forms fail explicitly. Depends on live design review and LOC-245.

<a id="LOC-247"></a>
## LOC-247 · Audited native proof adapters
<!-- task: {"id":"interop-247","status":"backlog","priority":3} -->

Generate output-only assumptions, preserve caller proof inputs and exactly-once effects, handle mutable snapshots and panic behavior. Reject unsupported logical data/struct invariants. Inventory all native assumptions and retain export restrictions.

<a id="LOC-248"></a>
## LOC-248 · Generate editable native spec source
<!-- task: {"id":"interop-248","status":"backlog","priority":2} -->

Separate generate command; no automatic extraction in imports, no update command and no overwriting source. Test omissions, unsupported members, extraction failures and handwritten fallback against the same binder.

<a id="LOC-249"></a>
## LOC-249 · Native import packaging and acceptance
<!-- task: {"id":"interop-249","status":"backlog","priority":3} -->

Cargo-host/dependency fixtures, publication identity, configuration changes, stale metadata and receipts, same-crate/downstream hostile consumers, diagnostic snapshots, documentation and full gates. Behavioral assumptions remain visible after compatible-signature dependency updates.

<a id="LOC-250"></a>
## LOC-250 · Expand specification expressiveness
<!-- task: {"id":"interop-250","status":"backlog","priority":1} -->

Elaborated signature matching, binder renaming, separately partitioned implementation blocks, generic specs/methods and traits (including spec trait and native trait binding, with per-implementation logical laws), transparent fields/enums, nested header interfaces and uses, proposition contracts, constant value contracts, logical opacity and declaration-only checked artifacts. Each extension needs explicit completeness, identity and proof-boundary tests; coordinate rather than duplicate the generic/trait projects.
