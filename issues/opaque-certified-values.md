# Letting unverified code hold a value with an invariant, and borrow a proof

- **Kind:** design
- **Status:** later
- **Blocks:** Printing a value with a proof field through `Debug` or `Display`; storing one in another crate's container; sorting or hashing one with generic code; a public type with an invariant in an exported crate or, in the first stage, in any crate
- **Rules:** `logic.boundary.plain-data`, `logic.boundary.logical-data`, `export.invariants-closed`

## Scope

RFC 0002's decisions 61 and 62 keep every value whose type holds logical data inside verified code, and count a type with proof fields among them. The owner chose that strict reading to start with, because it makes the boundary's rule one sentence. This issue records what it rules out, and the two relaxations that would bring most of it back.

What the strict rule rejects, each with an example:
- formatting such a value through the standard library ([`logical-layer/display-on-type-with-proof-field.reject.lc`](../litmus/logical-layer/display-on-type-with-proof-field.reject.lc));
- passing one through generic code that has no specification ([`logical-layer/invariant-type-through-unverified.reject.lc`](../litmus/logical-layer/invariant-type-through-unverified.reject.lc), [`logical-layer/vec-of-invariant-type-to-unverified.reject.lc`](../litmus/logical-layer/vec-of-invariant-type-to-unverified.reject.lc));
- exporting a public struct with a private proof field ([`export/private-invariant-fields.reject.lc`](../litmus/export/private-invariant-fields.reject.lc));
- lending a proof holder by shared reference ([`logical-layer/share-proof-holder-with-unverified.reject.lc`](../litmus/logical-layer/share-proof-holder-with-unverified.reject.lc)).

The two relaxations:

1. **A sealed value.** Unverified code may hold a value of a type whose proofs read only its own fields. It can move the value, drop it, and call the type's own functions, and can neither build one nor see inside. The type itself is then the evidence, as a newtype with a private field is in Rust.
2. **A proof lent by shared reference.** Unverified code may be given `&T` for a `T` that holds a proof, with no interior mutability on the way.

Both are believed sound for one reason: nothing that holds logical data is accepted back from unverified code, so a marker it copies or makes has nowhere to go. Neither has an argument written down.

## Questions it must answer

1. Which types are sealed: every proposition reads only the type's own fields, no type argument holds logical data, and unverified code of the same crate does not name its fields.
2. What `&mut` to a sealed value allows. Unverified code could swap two valid values, which keeps each valid and changes what verified code knows about the place.
3. Whether a trait object may hide a sealed value, since the rule that a type is never hidden exists so that the boundary's check can see it.
4. What a derive from another crate may be given, since `Serialize` reads fields and `Deserialize` builds the type.
5. How an exported crate shows such a type, and what its Rust users may do with it.
6. The argument, in [soundness-model](soundness-model.md), that nothing comes back.

## Log

### 2026-11-16, conversation with the owner on how to build Locus

Raised when the question of derives came up under the strict rule: a derived `Debug` hands each field to the formatter, and the formatting code is unverified. The reply offered both relaxations and recommended leaving them out of the first proposal; the owner did not take them up, and went on to the rules for function kinds. They are recorded here so that the cost of the strict rule is not forgotten.
