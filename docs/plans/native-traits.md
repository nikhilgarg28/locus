+++
id = "native-traits-plan"
title = "Native traits implementation plan"
group = "Plans"
order = 31
route = "plans/native-traits.html"
+++

# Native traits implementation plan

Implement LOC-21 on `feat/native-traits`. A trait is an interface with zero or more concrete implementations, not a concrete type. Specs retain their distinct opaque identities and unique backing implementation.

1. Parse Rust-shaped traits, concrete implementations on named Locus structs/enums/spec types, associated items and qualified calls. Keep generic bounds, blanket implementations, supertraits and dynamic dispatch explicitly deferred.
2. Resolve trait identities across modules. Check coherence, completeness, associated substitutions, receiver and logical modes, proof slots and promises. Recheck inherited defaults for each implementation and reject logical dependency cycles. Lower selected methods to ordinary checked functions without kernel axioms.
3. Resolve methods with traits in scope; diagnose ambiguity and support explicit `<Type as Trait>::method`. Preserve snapshots, ownership, logical observation and runtime effects.
4. Admit the supported Rust trait subset through the same interface machinery while preserving Rust identity. Retain unsupported metadata and report its limitation when used. Validate emitted implementations with Rust under the selected Cargo configuration.
5. Emit Rust-compatible traits and implementations. Reject an exported trait with logical methods/items, proofs, logical interface types or promises. Check associated bindings and reachable types; do not project proof-returning trait methods into a different trait.
6. Add focused parser, checking, default-override, coherence, visibility, proof, erasure and hostile-export regressions. Include real multi-file/Cargo fixtures, checked documentation and generated Rust compilation/execution. Run the extended validation gate before recording completion.

Update the manual, diagnostics, architecture, correctness account, examples and roadmap with implementation. Interior mutability and promise semantics remain explicit follow-up work; importing a shared-reference signature must not establish logical purity or state preservation.
