+++
id = "spec-interfaces"
title = "Specifications and checked implementations"
group = "Vision"
route = "vision/spec-interfaces.html"
order = 13
+++

# Specifications and checked implementations

A spec names one opaque concrete type with one implementation. A trait describes a requirement that many types can implement. They can share member grammar and contract checking without sharing type identity. The [manual](../spec/18-specifications.md) defines the implemented subset; the [implementation plan](../plans/opaque-spec-types.md) records its validation obligations.

## A type and its representation

```rust
spec type Counter {
    fn new() -> Self;
    fn take(self) -> u8;
}
struct CounterImpl { value: u8 }
impl Counter for CounterImpl {
    fn new() -> Self { Self { value: 0 } }
    fn take(self) -> u8 { self.value }
}
```

`Counter` has its own nominal identity. Clients do not need a trait import or another generic parameter to use it. `CounterImpl` remains an ordinary type, with ordinary privacy and additional inherent methods. It cannot be implicitly converted into a `Counter`. An implementation contains exactly the declared members; backing helpers belong outside it.

The implementation and declaration are owned by one package. Every loaded spec has exactly one complete realization, including unused specs. A generic realization covers the complete declared family once. Current generic body checking happens at concrete instantiation; universal checking is separate generic-system work, not a claim made by this feature.

## Contracts and abstraction

Headers retain propositions and explicit proof slots. Manual bodies and generated adapters must establish them using ordinary kernel-checked evidence. Signatures match resolved names and consistently renamed binders. Source grouping and whitespace do not change a contract. Logical definitions remain transparent; logical opacity needs its own introduction, elimination and unfolding rules.

Associated types, constants and logical methods belong in the interface. The first version permits concrete associated bindings. Associated type families, arbitrary trait bounds, generic methods, exact-value constant contracts and general borrowed/container Self adapters need additional machinery. Each unsupported form must fail with a targeted diagnostic. Language-wide `const fn` is [LOC-251](../roadmap/generics.md#LOC-251).

Automatic adapters may not invent casts, allocations, clone operations or aliasing assumptions. The initial adapters use a private nominal wrapper and support owned Self results, tuple components, direct Self inputs and shared/mutable Self borrows. A proof referring to a representation must still type-check when crossing the public interface; matching an erased signature is insufficient.

## Module specs later

Module specs are deferred and their previous experimental syntax is rejected. The intended broader interface can contain opaque types, their associated item signatures and free items, with exactly one owned implementation. A possible surface is:

```rust
spec mod counters {
    type Counter {
        fn new() -> Self;
        fn read(&self) -> u8;
    }
    const DEFAULT: u8;
}
```

Before implementation, define representation bindings, sharing of existing types, associated-type equality, nested modules, completeness and export identity. Avoid building an implicit trait-object or functor system into this concrete interface feature. Transparent layouts, macros, proposition constructors and declaration-only artifacts need separate decisions.

## Rust import and assumed realization

Plain `import path [as alias]` obtains physical Rust interfaces and preserves foreign identity; it creates no behavioral proofs and is independent of specs. Extraction should retain traits, async and unsafe items even when Locus cannot use them. Unsupported use must explain the limitation rather than pretend the item is missing.

A future `assume ImportedType impl Spec` can generate Locus-only wrappers. It is a distinct audited trust boundary: input proof requirements are prohibited in that first design; output proofs can be omitted when matching the native signature and assumed only on normal return. Runtime parameter/receiver types, constants, associated identities, effects and mutation footprints must match. Import itself cannot establish these laws or invent a logical model. The [import vision](rust-imports.md) and [roadmap](../roadmap/interop.md) track this separate work.

## Remaining obligations

- Prove the adapter lowering preserves results, evaluation order, mutation and the safe Rust abstraction boundary; passing differential and hostile-client tests is evidence, not a mechanized preservation theorem.
- Generalize abstract models and proof contracts over Self without exposing representation capabilities.
- Expand Self adapters only with explicit lifetime/ownership rules. No reference casts or automatic container reconstruction.
- Define generic associated types, traits and native implementation selection alongside universal generic checking.
- Keep generated artifacts reproducible, audit native assumptions and report toolchain/schema/configuration mismatches before trusting cached imports.
