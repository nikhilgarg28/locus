+++
id = "architecture"
title = "Architecture"
group = "Now"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T04:43:11.000Z"
route = "correctness/architecture.html"
order = 2
+++

# Architecture

The compiler elaborates a source-shaped typed program, validates its checking IR, and emits Rust from its erased form. This document describes those representations and their boundaries. The [language specification](specification.md) states the source rules; the [kernel contract](reference/kernel.md) states the logical rules. [Correctness](correctness.md) connects the implementation to tests and assumptions.

## Representations and pipeline

| Representation | Contents | Producer | Consumer |
|---|---|---|---|
| Surface AST | Source spans, names, syntax and annotations | Handwritten lexer/parser | Generic specialization and elaborator |
| Typed tree | Resolved identities, kernel types/terms/proofs, physical control flow, source names | Elaborator | Trusted validation, lowering and erasure |
| Check IR | Executable steps in let-normal form, SSA versions and explicit proof obligations | Lowering | Exec checker, kernel and check-IR interpreter |
| Erased tree | Physical values, control flow, borrow forms and marker positions | Erasure | Independent type checker, interpreter and Rust printer |

The elaborator orders items by their dependencies, resolves names while building the typed tree, and tries to construct evidence. There is no separately allocated resolved-AST stage. The typed tree is a candidate program until its logical terms, physical mode/layout, reference permissions and lowered IR have been accepted.

The check IR is not production output. Its interpreter is a reference for the checker-side meaning. Generated Rust comes from the erased source-shaped tree, preserving source control structure rather than reconstructing it from flattened IR.

## Source frontend and libraries

The lexer and Pratt/recursive-descent parser carry byte spans, bounded nesting and explicit work limits. Parser recovery yields multiple diagnostics; fuzz tests run malformed programs and deeply nested forms on constrained stacks. A parse success never establishes a type or proof.

`--library path.lc` loads ordinary declarations into the same checked module. A SourceBundle retains segments mapping virtual offsets to original files for diagnostic labels, suggestions and proof locations. Duplicate declarations are errors. Libraries gain no privilege by their path or by being shipped with the compiler.

Free and associated constants share one checking path. Associated names retain their owner through dependency ordering and Rust emission. Physical constant arithmetic checks the operator's safety premises and evaluates its closed term through the existing kernel rules, then lowers the resulting literal. Logical observations model this checked physical value; neither execution interpreter participates in acceptance.

Generic declarations are templates. Specialization substitutes concrete types throughout signatures, field types, propositions and bodies, then sends the result through the normal pipeline. Instance count and type-depth limits stop expanding polymorphic recursion. Uninstantiated generic bodies are not claimed to have passed universal checking. Runtime/logical mode is fixed by each declaration, not inferred anew per instance.

## Logical classification and physical layout

`logic fn` and logical blocks have checked total, effect-free bodies with Logical results. Ordinary functions remain executable irrespective of their promises or result types. Only the logical representation has unfolding equations. Calls to ordinary functions are opaque: callers receive exactly the declared result and its evidence fields.

Logical classification belongs to types. Nat, Int, Bool, Prop and proof types are logical; user logical structs/enums are checked as such. Runtime aggregates can contain logical fields. Ordinary enum discriminants and physical Box allocation remain runtime even with erased payloads.

The kernel uses one mathematical Bool, while the surface distinguishes physical bool from Logical Bool. ErasureLayout therefore accompanies kernel types through bindings, products, branches, function signatures and nominal fields. This is checked independently of proof search. A kernel-valid boolean term alone cannot authorize a runtime test of erased data.

The same separation is necessary for arrays, slices, vectors, shared references and boxes. Kernel snapshots describe contents, not storage layout or borrowing permission. Runtime layout and provenance are independently retained and checked before erasure.

## Logical definitions and the kernel

Logical definitions elaborate to immutable terms: variables, lambdas, products, constructors, applications and cases. Local lets are substituted into the resulting term; their equations are justified by checked computation. Proofs are explicit terms or proof-rule nodes; every constructor premise and every use of equality is checked independently of proof construction.

Named proposition arms have witness telescopes and a computed Prop body. Their constructor requires a separate proof of that body. Case analysis introduces the witnesses and body evidence into the arm context. Eliminating an inhabited proposition produces evidence only; it cannot extract a witness into logical or physical data. Strict positivity is checked before recursive propositions enter Definitions.

Recursive logical enums are finite inductive data. Structural recursion and induction inspect checked constructor subdata. Int-measured recursion requires a proof of nonnegativity and strict decrease at each admitted recursive call. The descent proof cannot rely on the call it justifies. The internal CaseKnown proof rule reduces a checked branch using its scrutinee equation without rewriting proof-containing unreachable arms.

Library Exists and ForAll are ordinary named propositions over logical callables. Their checked schema registration is used when positivity needs to know that a quantifier is covariant in its predicate. Source quantifier syntax lowers to those library declarations. Native quantifier terms and proof rules remain an internal kernel/store representation used by existing certificates and dependent proof transport; they are not a second source-language lowering.

## Context, snapshots and scope

The context holds typed binding identities and proven hypotheses. A source spelling is only a label: shadowing creates a new identity. The elaborator mirrors the eventual kernel context to construct candidate proofs; lowering checks those proofs in the actual context it creates.

Each mutable binding has immutable SSA versions. An assignment creates a new version; a field assignment reconstructs its root around the new field. Claims capture the versions current when formed. A model observation does not retain a physical reference or move the source, but requires permission to read its live storage at that point.

Result scope is checked explicitly. Local logical definitions may be closed into a result; opaque free local identities may not escape through a proposition or proof type. Dependent products bind earlier components for the types of later components. A returned proof about an input retains the input telescope identity, not a dangling source-name lookup.

## Lowering ordinary execution

Lowering has one evaluation order: operands and call arguments left to right; assignment computes its right side before its destination. An expression that can execute an ordinary call, mutate, panic, diverge or transfer control is never discarded because its value has a logical type.

Pure physical expressions become kernel terms in executable mode. Effectful subexpressions become ordered IR statements under the identities already chosen by the typed tree. A call with logical output still becomes an ordinary call statement. A logical application in ordinary code evaluates any ordinary argument-producing subexpressions before erasing the application itself.

The checker handles the following forms:

| IR form | Checked meaning |
|---|---|
| Let | Check the value/type, bind its identity and defining equation. |
| Have | Check the evidence, then add its proposition as a fact. |
| Call | Check arguments against the callee telescope and promised effects; bind the declared result without unfolding its body. |
| Match | Check the scrutinee; bind payloads and branch equations; check every arm. |
| Loop | Check initial state, abstract state, continue edges and break results; make no termination claim. |
| For | Evaluate bounds once; each iteration knows its range bounds; check carried state and control exits. Empty/reversed ranges execute no iterations. |
| Operate | Check safety evidence before introducing the result; require it under no_panic. Otherwise retain a runtime overflow check. Normal return establishes the exact result. |
| Buffer | Check the native operation's arguments and bound evidence before introducing its normal-return content equation. |
| Box allocation | Preserve physical allocation and its possible failure; logical contents do not erase the allocation. |
| Return/break/continue | Check the value or state against the corresponding target in the current context. |
| Panic | No returning value; under no_panic require checked evidence that the ending is unreachable. |

A branch that changes outer bindings returns their new versions alongside its value. An arm that transfers control contributes no joining value. Lowering independently computes the set of assigned roots, so the elaborator cannot silently omit a mutation.

Loops carry bindings written by the body, including tracked evidence, as a state telescope. Entry values and back edges must have the same dependent state types. Loop-local bindings and shadowed names are not outer state. Runtime loops are not kernel logical loops, and their acceptance does not prove termination.

Tracked evidence is a mutable binding whose declared claim is read over current versions. Source flow analysis reports when it becomes stale; the lowered version types independently prevent stale evidence from satisfying a refreshed claim. Reestablishing the evidence is an ordinary checked assignment.

## Calls, borrowing and native storage

Call-scoped `&T` parameters are snapshots of their referents. `&mut T` parameters supply entry values and return exit values in the check IR's result telescope. `old!(x)` selects the entry version. A call writes its exit values back into the caller's original places, in order. Overlapping mutable loans are rejected by trusted lowering, not merely by source flow analysis.

Stored shared references add typed provenance: storage roots, projection paths, current versions, scope and declared result lifetimes. The permission walk runs before lowering, while logical observations remain visible. It rejects escapes and reads through references invalidated by overlapping writes or moves. Generated Rust is checked as an independent borrowing oracle; it cannot validate erased reads that never appear in its input. Stored mutable aliases and interior mutability remain outside this tier.

Buffer<T> is an immutable finite sequence of element snapshots with a u64 length bound. Arrays, slices and Vec share that mathematical content representation but retain distinct runtime layouts. Bounds proofs authorize logical reads; they do not authorize a physical borrow. Native helpers preserve element order and carry explicit allocation/panic effects. Runtime push yields its length equation only on normal return.

A model implementation is the one canonical checked logical definition for its physical source. `model!(place)` selects a physical read path before applying that model; ordinary logical named-field access selects the model first. `#[derive(Model)]` constructs fieldwise models only when requested. Nat is encoded by a checked logical Int/nonnegativity product, and unsigned observations use the existing machine range proof. These constructions add no kernel axioms.

A model implementation uses an authorized shared observation. Its source layout participates in resolution: a shared slice observer may accept compatible array/vector borrows; an array-specific implementation must not silently apply to a different physical shape. Definitions may compose into larger models. The specialization pass retains physical source-type hints for `model!` dependency ordering, including nested fields; elaboration independently checks the actual path and model.

Runtime generic instances currently emit distinct named Rust structs/enums. In particular, source `Option<T>` and `Result<T, E>` use checked prelude templates but their generated ABI is a specialized Locus enum, not `std::option::Option` or `std::result::Result`. Rust callers use the emitted type and variants. Mapping recognized prelude types to the standard Rust ABI belongs to the later interop work and must preserve the export/forgery checks.

## Explicit trust boundary and audit

`trusted "reason" fn ... = Rust::item;` declares an unchecked specification of a registered native implementation. The reason is grammar attached to the item. The header is checked for well-formedness and physical argument/result compatibility; its logical postcondition and claimed promises are deliberately assumptions. The initial source registry exposes Vec len, get and push.

The execution program records each trusted adapter with its reason and backend. It is an executable contract, never an unfoldable kernel function or an unrecorded kernel axiom. Both interpreters execute the registered native behavior. A deliberately wrong specification is accepted only at this recorded boundary and is a test of what the audit must disclose, not evidence that the specification is true.

`locus audit` reports foreign assumptions, built-in native contracts, functions without termination promises, unchecked panic sites/operators, and classical dependencies. Audit output is informational; it does not strengthen a proof or suppress a failed check.

## Erasure and cleanup

Logical values become a single private zero-sized marker, Erased. Runtime aggregates preserve their data fields, physical tags and marker positions. Logical declarations have no emitted implementations. Ordinary calls retain their effects even when all inputs/results erase.

An erased arithmetic node retains a `proven_safe` bit derived from accepted safety evidence. The Rust printer emits a plain operator when set, and `checked_*().expect(...)` otherwise. Both paths evaluate operands once in source order. This emission decision belongs to the compiler correctness boundary; the marker bit alone is not a proof.

Erasure preserves eager computation: `derive(mutate_and_prove(&mut x))` runs the mutation exactly once and then yields a marker. A logical value is not inspected to choose runtime control. Logical Bool operators in ordinary code preserve eager ordinary operands; erasing them must not introduce physical short-circuit behavior.

The printer removes unused erased bindings by default, keeping effectful right sides as statements. It removes unused pattern components without removing their producing calls. If such a statement never returns, it becomes the terminal expression and unreachable marker tails disappear. The cleanup is tested against both interpreters and generated Rust with warnings denied.

An Erased value carries no claim identity in Rust. Export safety therefore comes from a checked privacy boundary: evidence-taking functions remain restricted, and safe Rust constructs validated values through checked constructors with private invariant fields. Replaying a marker from another result cannot satisfy a publicly accessible raw-proof parameter because that interface is rejected.

Project exports add a one-way return projection after ordinary erasure. `erased/facade.rs` describes proof omission and tuple projection; `project/export.rs` validates all surviving types and retains source tuple indices in diagnostics. The Rust printer emits both a private implementation with the original erased result and a public forwarding entry with the projected result. Internal calls target the private implementation; public methods keep the original source spelling. The wrapper calls once and destructures the completed result, preserving physical values, references and effects. No kernel proof rule or logical assumption is added.

## Proof construction, persistence and diagnostics

A hole tries stored evidence, exact facts, checked computation, closed evaluation and bounded linear arithmetic. Every successful tier produces an explicit proof checked by the kernel. Implicit unfolding, arbitrary rewrite search and general induction search are not performed. Diagnostics can suggest the missing explicit step without silently taking it.

Stored proof keys include the context and claim with stable declaration names, including qualified method names. Version-2 `Locus.lock` is one TOML file per source directory: relative source-path tables hold obligation keys, readable function/ordinal labels, conclusions, and named proof steps. The reader and step expansion are untrusted; parsing never makes a proof authoritative. Locked checking disables search and accepts only certificates that recheck against the current obligation, without changing storage. Successful unlocked checking canonicalizes the checked source's used entries and preserves unrelated source tables. Legacy version-1 `<source>.proofs` input is migrated only when no lockfile table exists for that source, after a successful check and write. Malformed, stale or malicious stored proofs are rejected or searched again according to the selected mode. The TOML parser and lock/step readers are outside the proof kernel; they retain explicit resource bounds.

Diagnostics retain source spans and suggested fixes. SourceBundle maps library spans to original files. Golden tests compare rendered diagnostics and public CLI output; negative corpus directives pin codes and locations. Search costs and proof sizes are measurements, never limits expressed as wall-clock time.

## Agreement and the trusted base

The test harness compares the check-IR interpreter, erased interpreter and compiled Rust on values and observable panic behavior, including mutations before a caught panic. Rust builds with overflow checks enabled and disabled are both exercised; Locus arithmetic has identical checked behavior in each. The interpreters use different program representations but share some primitive machinery; agreement is testing evidence, not three independent soundness proofs. Fuel exhaustion or a timeout is inconclusive, never agreement or proof of divergence; interpreters need not consume equal fuel.

Trusted components include the kernel and primitive meanings; logical/runtime layout and permission validation; lowering and the exec checker; erasure/cleanup and Rust printing; and the Rust toolchain. Registered native semantics and explicit foreign contracts are additional assumptions. Parsing, elaboration, proof search, stored proof input, derived lemmas and both interpreters are outside the proof kernel. Their placement does not itself prove that parsing and elaboration faithfully implement the source specification. The [trusted-file inventory](../tools/trusted-base.json) conservatively counts the implemented validation and emission boundary, not a mechanically established minimal trusted base.

Adversarial tests submit malformed kernel declarations and proofs, wrong descent claims, nonpositive recursive propositions, forged arithmetic certificates, stale versions and invalid borrow shapes. Differential tests compare mathematical machine models with Rust and compare well-typed generated programs across all three execution paths. These tests do not prove erasure preservation. The [formal core](reference/formal-core.md#spec-3.6:1) states the conditional preservation and simulation obligations; their Lean mechanization remains deferred.

## Documentation and measurement boundaries

Canonical Markdown specifications carry permanent rule IDs, which tests cite. The documentation harness checks executable examples, including three-way agreement for run fences. The [development workflow](development.md) describes the traceability gate, known-bug lifecycle, and commands for reproducing it.

[Generated status](generated-status.md) binds measured counts to a source fingerprint. [Performance records](performance.md) retain workload, toolchain, machine, and compiler identities. A past successful gate or benchmark is evidence about its recorded inputs, not a claim that every later checkout passes.

## Module and package front end

`src/project` expands declared module files while retaining source spans, resolves lexical and Cargo package namespaces, then supplies canonical names and source privacy information to elaboration. Each package keeps its own crate root. The existing typed/kernel pipeline checks the combined program. Derived model names participate in the same module namespace; their definitions are still created and checked by elaboration. Canonical observations invoke the checked model body without treating it as a private inherent method, while model field selection retains source privacy. Reachable Rust interface checking grants emission visibility independently from Locus visibility; foreign runtime identities map to dependency paths. The shared build API emits an includable component and an input/output receipt. See the [package guide](packages.md).
