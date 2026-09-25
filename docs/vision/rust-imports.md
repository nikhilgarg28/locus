+++
id = "rust-imports"
title = "Rust imports through specifications"
group = "Vision"
route = "vision/rust-imports.html"
order = 45
+++

# Rust imports through specifications

This is the design for a later round. The current round implements [specs and manual Locus implementations](spec-interfaces.md). Import syntax, native extraction and assumption-backed wrappers wait for a separate implementation discussion.

## Syntax and resolution

~~~text prose grammar
import-item = "import" RustPath ("as" SpecPath)? ";"
~~~

`import crate::helpers as counters;` realizes an existing module spec `counters`. `import std::vec::Vec;` requires a spec named `Vec`. Without `as`, the target is the last native path component; with `as`, it is an existing Locus spec, not a newly created alias. Use `pub use` for visibility. There is no `pub import`, separate `assume`, `manual`, `exact`, implicit extraction, or inline contract block.

The source path resolves in Rust's host/Cargo environment; the target resolves in Locus. Module specs bind modules and type specs bind types. Only declared members are visible through the spec; additional Rust members do not appear automatically. Missing, private, ambiguous or unsupported selected members fail binding.

An import is one realization, exclusive with manual implementation or another import, and owned by the spec's package. The first implementation should reject competing overlapping specs for one native type. Future multiple views must preserve Rust nominal identity and model coherence rather than invent independent types.

## Checked interfaces, assumed behavior

rustc checks the physical interface under the actual host toolchain, target and features: types, arity, receivers, ownership, mutability, lifetimes, visibility, constants and supported bounds. Signature extraction does not establish behavior.

For each native function, generate a wrapper that calls Rust exactly once and supplies assumption-backed evidence for the declared successful-return postcondition. `import` is the explicit trust boundary. Record native path, spec/member, proposition, source location, dependency identity and configuration in the trust audit and receipt. Kernel-checked consequences remain conditional on these imported axioms; they are not verification of the native implementation.

**Input proofs are caller obligations.** The adapter accepts supplied evidence, erases its physical argument positions and calls Rust. It never manufactures an input proof to make a call legal. The contract is conditional on valid inputs and required evidence. Output evidence appears only after normal return. Panics, aborts and divergence produce no return proof.

Runtime evaluation and mutation must be preserved. `&mut` output claims refer to final snapshots; `old!` refers to entry snapshots. Proof-only results project to Rust unit; tuples use the existing facade projection. The native result must match that physical shape exactly. Logical non-proof results such as Nat, Seq or Prop are initially rejected. Explicit no-panic/termination/effect promises would also be imported behavioral assumptions and must appear in the audit.

Reject imported `logic fn`, logical constants and model implementations initially. Native execution is not a transparent total kernel definition. Later support requires logical definitions and explicit audited correspondence laws, including purity/totality where needed.

## Struct proof fields

A proof field certifies particular accompanying values, not all instances of a nominal Rust type. Import must not attach extra proof fields to a raw Rust alias: safe Rust might freely construct or mutate a violating value.

Initially allow opaque physical native types with selected methods and explicit output evidence. Reject transparent imported structs/enums with logical payloads, extra hidden proof fields, and by-value representations without an exact checked native counterpart. An invariant-bearing value can instead be a distinct Locus wrapper with private fields and checked or imported constructor contracts. Every entrance and mutation path must uphold its invariant.

Evidence returned about an opaque native object is a snapshot claim. Interior mutability, shared aliases, callbacks and threads may invalidate live-state claims and require explicit models before stronger promises are supported. Later transparent physical structures require complete field/shape/type validation and a clear layout policy.

## Editable spec generation

Proposed separate command: `locus spec generate <RustPath> --manifest-path Cargo.toml --out path.lc`. It extracts supported physical declarations into an explicit source file, generates no behavioral proofs, and refuses to overwrite existing files. Users edit that source normally. **No update command in this phase:** regenerate to another file and review the diff.

Report every requested unsupported/omitted item by native path and reason. Extraction failure must not produce a success-shaped empty or silently incomplete interface. A deliberately selected subset need not describe every Rust member. Record generation provenance in comments without treating comments as trusted metadata. Handwritten specs use exactly the same physical-binding checks as generated ones.

Use Cargo's resolved package identities, aliases and path/git dependencies. Published Locus imports retain their package-relative Rust identity: `crate::` must not accidentally switch to the consumer's crate. Host-private paths require checking in the host's visibility context; a separate helper crate may not suffice. Avoid a Cargo build cycle while inspecting host code.

## Diagnostics and versioning

| Failure | Required behavior |
|---|---|
| Missing spec or wrong kind | Name the native path and expected existing spec. |
| Multiple realizations | Point to both imports/implementations. |
| Missing/private Rust member | Point to header member and native lookup failure. |
| Type/receiver/constant mismatch | Show expected and actual physical signatures with rustc notes. |
| Unsupported async/unsafe/variadic/generic/trait form | Name the exact member and unsupported construct; do not approximate. |
| Dependency/toolchain/feature/target change | Recheck binding and expose changed provenance of assumed contracts. |
| Stale extraction/cache/receipt | Revalidate current native interfaces; cached metadata is not authority. |
| Extraction unavailable | Accept handwritten complete specs subject to the same checks. |
| Unexportable logical surface | Reuse export exposure-path diagnostics and suggest a private wrapper/facade. |

A dependency upgrade may preserve a signature while changing behavior. Rechecking the interface does not renew confidence in an assumed behavioral contract; the audit must show the changed native identity/configuration. Never silently weaken a physical check into an assumption or fabricate a native constant value.

## Acceptance tests and initial restrictions

Use real Cargo hosts, renamed/path dependencies, package archives, host-private functions, changed signatures and feature/target configurations. Compile generated adapters with warnings denied. Verify caller proof obligations, audited output assumptions, exactly-once effects, proof-only mutation, panic behavior and hostile Rust callers. No unsafe native functions, traits/generic binding families, callbacks, async, interior-mutability models or layout-sensitive by-value mappings until their checks exist.

The [ordered plan](../roadmap/interop.md#specifications-and-native-imports) includes native validation, wrappers, generation, packaging/provenance and acceptance. These tasks remain pending until the manual-spec implementation is reviewed live.

## Importing Rust traits

Extend binding to an existing `spec trait` once Locus traits exist. Importing the physical Rust trait shape does not establish logical laws for every Rust implementation of it. A handwritten implementation might violate any added law. Establish or explicitly import stronger contracts for particular implementations, and require those checked/trusted implementations at generic proof-bearing bounds. Do not automatically quantify an imported logical contract over all present and future Rust implementors. Share native signature extraction and checking with type/module imports; trait coherence, associated types, generic selection and default methods need their own validation.
