+++
id = "correctness"
title = "Correctness"
group = "Now"
route = "correctness.html"
order = 52
+++

# Why believe the compiler?

A checked proof is only one part of the story. Locus also has to preserve the meaning of the program through mutation, borrowing, erasure, and generated Rust. We keep those boundaries explicit and test them independently.

<!-- component: assurance -->

## 1. The specification is connected to tests

The manual, kernel contract, and formal core use permanent rule IDs. Tests cite those IDs directly. The specification gate rejects a missing rule, a dangling citation, and an operative rule without a focused test. Each rule in the published HTML exposes its associated tests and source locations.

A Rust test function counts as focused. A corpus example must have fewer than forty physical lines to satisfy focused coverage; larger cases can supply context without replacing focused checks. Coverage means a reviewable relationship exists. Review must still establish that the test actually exercises the rule.

Executable documentation fences declare whether they should check, run, or be rejected with specific diagnostics. The documentation harness feeds those complete examples to the compiler, including any lines hidden from the initial teaching excerpt. Expected results are compared with both interpreters and generated Rust. Prose-only grammar and shell fragments carry explicit exemptions. GitHub runs these checks on every branch push and pull request, before the website artifact is uploaded.

Sources: [specification gate](../tools/spec.py), [adversarial gate tests](../tools/test_spec.py), [executable documentation tests](../tests/atlas_fences.rs).

## 2. Proof construction does not decide truth

The elaborator and its bounded search try to build a certificate. The kernel checks each certificate independently against the current context and requested proposition. A found proof, a user-written proof, and a proof read from `Locus.lock` all cross this boundary.

Tests submit malformed proof terms, declarations, recursive definitions, and arithmetic certificates. Mutation tests alter valid certificates and try them against false claims identified by a separately written bounded evaluator. An accepted false claim is a failure; claims the evaluator cannot decide are skipped and counted. Lockfile readers reject malformed references, excessive expansion, and unsupported versions. Cache keys and displayed claims grant no authority.

Sources: [kernel soundness tests](../tests/kernel_soundness.rs), [arithmetic certificates](../tests/kernel_linear.rs), [proof-store tests](../tests/store.rs), [kernel contract](reference/kernel.md).

## 3. Check the program that the proof describes

The source-shaped typed tree has two paths: lowering produces the checking IR; erasure produces the source-shaped runtime tree. The checking IR exposes binding versions, calls, joins, loop state, and proof obligations. Separate validation checks physical layout and reference permissions before information disappears.

<!-- component: pipeline -->

Assignment creates a fresh logical snapshot. Dependent joins and loop state must carry evidence with the right versions. A logical observation still needs permission to read the physical object. The checker's tests attack stale evidence, invalid writebacks, overlapping borrows, and erased data used as runtime control.

The [formal core](reference/formal-core.md) states the relevant judgments. An inventory test requires it to cover every checking-IR constructor, so a new form cannot silently bypass the calculus. A prose calculus is not itself a machine-checked preservation theorem.

Sources: [IR checker tests](../tests/exec_check.rs), [lowering tests](../tests/typed_lower.rs), [layout tests](../tests/erasure_layout.rs), [formal-core inventory](../tests/formal_core.rs), [architecture](architecture.md).

## 4. Compare three execution paths

The checking-IR interpreter runs the checker-side meaning. The erased interpreter runs the runtime tree. The Rust compiler builds the emitted program with overflow checks enabled and disabled. Tests compare values, panic messages, and mutations completed before a panic across these executions. Checked arithmetic must agree under both Rust settings. A safety proof is checked before adding the operation's result facts, so its postcondition cannot remove its own check. Adversarial IR tests attempt exactly that circular justification.

Canonical models are checked logical definitions, and Nat is an Int paired with kernel-checked nonnegativity evidence. Their constructors and arithmetic proof builders add no axioms. Tests cover duplicate model rejection, physical read paths, stale snapshots, borrowed representations, and failed natural-number conversions. Correct erasure and the correspondence between physical operations and these observations remain compiler obligations, not a completed mechanized proof.

Generated programs exercise arithmetic, branching, loops, mutable state, and calls through `&mut`. Each run records its seed, and `LOCUS_SEED` reproduces it; shrinking searches for a smaller counterexample. The fast gate uses a smaller sample, while the extended gate generates 10,000 programs. Boundary tests also compare machine operations with Rust across integer types.

The interpreters have different representations and control-flow implementations, but share some primitive machinery. Agreement can expose defects; shared mistakes can still escape it. No interpreter is designated the winner. A disagreement reports the disagreeing pair and its reproducer. Fuel exhaustion, depth exhaustion, and timeouts are **inconclusive**, never a successful comparison or proof that a program diverges.

Sources: [random programs](../tests/random_programs.rs), [differential tests](../tests/differential.rs), [machine operators](../tests/operators.rs), [corpus harness](../tests/common/corpus.rs).

## 5. Test the boundary Rust callers see

Proofs erase to a private zero-sized marker. That marker does not retain proposition identity in Rust, so the export checker restricts evidence-taking functions and protects invariant-bearing fields. Safe Rust callers use validated constructors and the permitted data interface.

Acceptance tests compile real Rust callers. Attempts to construct the marker, access restricted functions, fabricate invariant-bearing values, or replay unrelated evidence must fail. Successful exported interfaces must compile and run. Proof-returning functions additionally receive data-only facades. Tests compare their physical results with both execution interpreters, check tuple shapes and borrowed/owned values, and observe mutations before normal return and panic. Same-crate and downstream callers cannot reach the retained proof implementations. Generated Rust is also compiled with warnings denied.

Sources: [export-boundary acceptance tests](../tests/acceptance.rs), [crate-build tests](../tests/build.rs), [Rust output tests](../tests/rust_output.rs), [proof-output facade tests](../tests/export_facades.rs).

## 6. Make assumptions and failed checks visible

The trusted base includes more than the proof kernel: layout and permission validation, lowering and IR checking, erasure and Rust emission, the Rust toolchain, and admitted native contracts all matter. A `trusted` adapter records an explicit assumption and a reason. `locus audit` reports those boundaries along with unchecked panic sites, functions without termination promises, and classical dependencies.

Parsers and proof readers have counted resource bounds and hostile-input tests. Diagnostic goldens pin codes, locations, and JSON structure. A known-bug test pins an expected failure to an open roadmap task; an unexpected pass fails the suite rather than leaving a stale exemption. The gate emits its completion banner only after every required step succeeds.

Sources: [trusted-base manifest](../tools/trusted-base.json), [audit](../tests/audit.rs), [parser fuzzing](../tests/parser_fuzz.rs), [implementation limits](../tests/limits.rs), [gate interruption tests](../tools/test_gate.py).

## What remains unproved

The guarantee is conditional on the stated input assumptions and trust boundary, and generally concerns normal return. Panic or nontermination does not establish a postcondition. Tests do not exhaustively explore allocation failure, arbitrary external code, or every possible program.

The formal core states the preservation and simulation obligations connecting checked programs, lowering, and erasure. Their Lean mechanization is deferred; there is no machine-checked compiler-correctness theorem. The [roadmap](roadmap.md) and [formal-core obligations](reference/formal-core.md#spec-3.6:1) record the remaining work.

## Module and package boundaries

Name resolution, source privacy, reachable export validation and Cargo-to-Rust identity mapping are part of the compiler correctness boundary. Module/package fixtures exercise real directory trees, private field/method access, erased-type leaks, same-crate Rust attacks, and producer/consumer Cargo builds. A dependency's theorem is checked from its Locus source; metadata and receipts are not proof authorities.

The host and dependencies must compile the generated interface at the Rust path they advertise. Build-script wiring and the Rust dependency implementation remain build assumptions. Receipt hashes detect accidental stale or edited artifacts; they cannot establish what a deliberately altered build compiled. The project pipeline always checks proofs before emission. General trait bounds, generic runtime ABI export, and broader native ABI mappings remain separate work.

Canonical models obey that source boundary as well: their bodies may inspect representation fields in the defining module, while consumers can select only visible model fields. Derived models retain their source module and field visibility. Module and Cargo fixtures exercise these rules together with associated constants and checked arithmetic.

Scoped generic evidence adds no axiom. Its trusted check is nominal-family instantiation: constructor payloads and match arms must agree on logical arguments. Direct kernel tests reject malformed indices and changed claims; source tests cover scope escape, stale snapshots, mutation and export restrictions. The optional-search test compares checked IR, erased execution and Rust with overflow checks both on and off over finite lists, while independently checking each answer against list membership and logical sortedness against adjacent-pair comparison. This is evidence for the implementation, not a proof of search completeness.

Logical observation regressions cross declaration and argument reference depths, compare claim identities, and reject stale references, stale proofs, moved values and invalid runtime calls. Custom-model and structural-model cases check that observation preserves physical read permissions and evaluates ordinary argument computations once. These checks exercise frontend normalization and existing typed permission checking; they introduce no trusted equality between different snapshots.

## Reviewed interfaces still need proofs

[Concrete specs](spec/18-specifications.md) do not turn declarations into assumptions. Tests in `tests/specifications.rs` reject missing bodies, altered proof signatures, false output evidence, weakened effect promises and alias-based extra implementations. Real files exercise split headers/definitions and original diagnostic locations; generated Rust runs under warning denial and rejects private representation access. Kernel checking of the implementation remains necessary after matching the header.

## Checked spec adapters

A type spec supplies a contract, not evidence. Both the manual representation body and its generated opaque adapter pass the ordinary compiler and kernel checks. Tests cover missing/duplicate implementations, changed propositions, false evidence, mutable effects and hostile Rust clients. Resolved signature matching and adapter/privacy lowering are counted in the compiler trust inventory. Generic bodies remain checked at concrete instantiation; declarations alone do not certify all instantiations. Module specs and more general representation conversions remain deferred.

## Native Rust interfaces

Plain imports retain all public item kinds and distinguish native provenance from checked Locus source. Only the supported physical signatures become executable declarations; no imported metadata supplies a proof, model or effect promise. Used calls are checked against actual Cargo metadata as well as rustdoc output. Tests deliberately expose mismatched and `cfg(doc)`-only signatures to ensure documentation cannot authorize a native call.

The extractor, schema normalizer and native execution bridge are whole-compiler correctness assumptions, together with Cargo/rustc/rustdoc. Experimental JSON output is enabled only in the rustdoc child. Schema drift fails closed. Real package tests exercise aliases, features, target selection, source changes, macro/trait retention, diagnostics, output ownership and executed Rust side effects. Both interpreters explicitly decline arbitrary native execution; that refusal is not evidence of runtime agreement. Native correctness remains conditional on Rust's type/safety guarantees, without stronger behavioral assumptions. See [native imports](spec/19-native-imports.md) for current restrictions.

## Trait contract preservation

Concrete trait implementations are checked bodies, not assumed laws. Signature tests cover renamed binders, wrong proof claims, missing items, aliases and conflicting implementations. Default regressions include an override that invalidates an inherited proof and a logical call cycle. Interpreter and generated-Rust tests exercise dispatch and mutation in proof-returning methods. Imported trait probes validate the physical Rust interface under the selected Cargo configuration; they establish no behavioral property.

Trait selection and forwarding remain compiler-correctness assumptions and are included in the trusted-file inventory. Export rejects logical members and promises, including proof results rather than silently projecting a different trait. Generic/universal trait checking and interior-mutability observation rules remain explicit roadmap work.
