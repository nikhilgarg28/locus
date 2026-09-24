+++
summary = "Canonical models, explicit observations, natural-number models, and build-independent arithmetic."
id = "models-arithmetic"
name = "Canonical models and checked arithmetic"
status = "done"
route = "roadmap/models-arithmetic.html"
order = 0
kind = "project"
+++

# Canonical models and checked arithmetic

This project changes the accepted language contracts, superseding the multiple-destination model selection and build-dependent overflow rules recorded in the completed Reconciliation project. General user-defined traits remain owned by LOC-21. All four tasks are implemented and validated. The implementation sequence and acceptance criteria are retained below.

<a id="LOC-233"></a>
## LOC-233 · Checked arithmetic with proof-directed check elimination
<!-- task: {"id": "models-arithmetic-233", "status": "done", "priority": 3} -->

Machine addition, subtraction, multiplication and signed negation return the exact representable result or panic in every build. Wrapping methods remain modular. Search for checked safety evidence before introducing result facts; emit a plain operator only with that evidence, otherwise a checked operation. `no_panic` still requires evidence. Successful execution establishes the exact result. Update the checking IR, both interpreters, erasure and Rust emission together. Validate every width, boundary, panic and operand evaluation order under both Rust overflow settings; attack forged and circular evidence.

Implemented and validated by the [operator/IR tests](../../tests/exec_operations.rs), [emission tests](../../tests/canonical_models.rs), [operand effects corpus](../../tests/corpus/accept/checked_operand_effects.lc), and [locked replay tests](../../tests/store.rs). Runtime checks remain when an optional certificate is unavailable.

<a id="LOC-234"></a>
## LOC-234 · One canonical model and an explicit observation boundary
<!-- task: {"id": "models-arithmetic-234", "status": "done", "priority": 3} -->

Use `impl Model for T { type Logic = M; ... }` with one checked logical model method. Resolve ordinary logical expressions through the receiver's model. `model!(place)` resolves a physical read path before modeling its selected value, retaining read permissions and snapshot identity. No runtime call, arithmetic, mutation or move may hide inside this boundary. Opt-in structural derivation requires models for the selected fields and must diagnose missing models and overlap. Test custom model field names, missing receivers, nested places, bounds, shared/exclusive access, stale snapshots and erasure.

Implemented in the parser, dependency resolver, model elaboration and ownership boundary. [Focused model tests](../../tests/canonical_models.rs) and the checked buffer/boxed-list library pass. Constant observations preserve the checked initializer’s physical meaning; focused tests cover machine bounds, named constants and rejected runtime calls in logic. Associated constants support qualified names, `Self`, forward references and Rust visibility. Constant machine arithmetic is kernel-evaluated after checking its range and divisor premises; invalid initializers are compile-time errors. `const fn` declarations remain outside this scope. Manual models cover enums; structural derivation covers structs. General traits remain outside this project.

<a id="LOC-235"></a>
## LOC-235 · Nat models and logical integer conversions
<!-- task: {"id": "models-arithmetic-235", "status": "done", "priority": 3} -->

Expose Nat and Int; unsigned machine values and lengths model as Nat, signed values as Int. Nat subtraction requires non-underflow evidence. Nat-to-Int conversion is total; unchecked Int-to-Nat conversion is rejected. Keep kernel representations and proof rules explicit, with negative tests for forged naturals and conversion evidence. Migrate the checked library, examples and model implementations.

Implemented as a checked logical product of Int and nonnegativity evidence, without new kernel axioms. [Natural-number tests](../../tests/canonical_models.rs) cover arithmetic, division by zero, subtraction obligations, widening and rejected forged values. The library's inductive example is named Peano to distinguish it from built-in Nat.

<a id="LOC-236"></a>
## LOC-236 · Contract migration and acceptance
<!-- task: {"id": "models-arithmetic-236", "status": "done", "priority": 3} -->

Update the manual, formal core, kernel contract, diagnostics, vision and trusted-base inventory. Preserve stable rule IDs and attach focused tests. Run checked documentation, adversarial kernel/IR cases, execution differential tests, random programs and the extended gate. Record actual results and remaining limits; do not present a partial milestone as complete.

Accepted on 24 September 2026: `tools/check.sh --extended` emitted its complete receipt after 883 debug tests and 883 release tests passed. The fixed-seed run generated 10,000 programs and compared 82,210 execution cases without disagreement or inconclusive outcomes. The debug suite took 222 seconds, exceeding the 120-second advisory target; the release suite took 861 seconds. Constants also pass locked replay and an external Rust caller test. Final completion metadata is validated by the documentation checks; automatic measurement freshness remains governed by [Generated status](../generated-status.md).
