+++
id = "platform-types-and-structs-plan"
title = "Pointer-sized integers and struct forms"
group = "Plans"
route = "plans/platform-types-and-structs.html"
order = 6
+++

# Pointer-sized integers and struct forms

Implement [LOC-103](../roadmap/memory-layout.md#LOC-103), [LOC-104](../roadmap/generics.md#LOC-104) and [LOC-112](../roadmap/generics.md#LOC-112) together on `feat/platform-and-structs`. The user explicitly selected migration of collection lengths and runtime indices from u64 to usize as an acceptance requirement. This branch is separate from the spec/import integration.

## Contract and scope

- Resolve usize/isize against the program's selected Rust target, never an unrelated compiler-host width. Initially support the existing 32/64-bit target range and diagnose unsupported target layouts. Keep pointer-sized types distinct from equally wide fixed-width types.
- Models remain Nat for unsigned values and Int for signed values. Bounds, casts, wrapping operations, checked arithmetic and no_panic obligations use the selected width. A proof may depend on that width; persisted evidence and generated Rust must not silently reuse a different width.
- Discover target information through the standalone compiler and Cargo build API. Record it in build receipts and proof identities, and make emitted Rust reject a different target width. Cross-target checking must not require a target standard library merely to discover its layout.
- Collection lengths, physical indices and array-length parameters use usize. Logical lengths stay Nat and observe the selected usize bound. Migrate library source, examples, diagnostics and tests together; retain explicit casts where a genuinely fixed-width API is intended.
- An omitted function result means exactly unit, not inferred return type. Apply the rule consistently to functions, methods and spec/native headers. Ordinary return checking and the logical-result restriction still apply.
- Support named, tuple and unit struct declarations as distinct nominal forms. Tuple fields are accessed positionally; optional names can bind earlier values in dependent field types without exposing named accessors. Unit structs have a value constructor and retain nominal identity. Construction and destructuring must preserve ownership, privacy and proof dependencies.
- Preserve these forms in generated Rust and through modules, generics, specs, logical derivation and exports. Never expose a constructor that bypasses private invariant fields.

## Ordered implementation

1. Add the unit-result grammar and focused positive/negative signature tests.
2. Add tuple/unit struct shapes through parsing, resolution, checking, patterns, erasure and Rust output. Reuse the existing nominal product and proof-field checking rules.
3. Add explicit target-width machine types and target selection. Extend kernel arithmetic, proof serialization, source checking and generated-Rust guards without ambient mutable target state.
4. Migrate collection length/index types, native adapter signatures, models, library code, checked documentation and corpus expectations.
5. Exercise real Cargo target selection, receipts and hostile cross-width reuse. Update all current contracts, roadmap status, diagnostics and trusted-base records with delivered behavior.
6. Run focused suites, executable documentation, site/highlighter checks and the complete extended gate. Commit locally on this branch; merge/push of this new work requires separate user instruction.

## Acceptance

- Under both 32-bit and 64-bit layouts, check boundaries, MIN/MAX, casts, wrapping, arithmetic overflow/underflow and no_panic proofs. A width-specific theorem cannot replay as evidence for a different claim on another target.
- Both interpreters agree with the selected-width model. Host-compatible emitted Rust agrees in debug/release overflow modes; wrong-width compilation fails clearly. Report unavailable cross-target execution honestly rather than calling simulation native execution.
- Vec, array and slice length/index operations accept usize and reject the former u64 implicit convention. Bounds evidence and zero-sized/logical element behavior remain correct.
- Tuple/unit struct constructors, positions, patterns, generics, modules and methods work. False evidence, private access, nominal mismatches, invalid moves and invariant-breaking partial updates fail.
- Omitted returns behave exactly like explicit unit, including spec matching and effectful calls. Bodies returning a value are rejected unless their result is declared.
- All current Markdown examples are checked; completed task statuses require passing implementation and validation, with remaining limitations stated explicitly.

## Delivered and validated

All three tasks are implemented, including the collection migration. The complete `tools/check.sh --extended` gate passed: the debug suite took 539 seconds and the release stress suite took 1,151 seconds. The debug duration exceeded the advisory 120-second budget. The randomized execution run generated 10,000 programs and compared 81,965 executed cases with no disagreements or inconclusive cases; 37 programs were rejected by the checker before execution.

The pointer-width tests check both 32-bit and 64-bit semantics against independent fixed-width arithmetic oracles. Generated Rust was executed on the host target, with overflow checks enabled and disabled; cross-width Rust compilation was tested for rejection. This is not a claim of native execution on a 32-bit target. The site, checked Markdown fences, diagnostic snapshots and editor highlighters passed, and desktop/mobile rendering was inspected.

Final roadmap and wording updates are documentation-only. They deliberately invalidate the earlier generated measurement page rather than presenting its source fingerprint as current. The compiler validation above remains the recorded result for the implementation; final documentation changes receive the standard gate before the local commit.
