# What is still awkward at the boundary with unverified code

- **Kind:** design
- **Status:** later
- **Blocks:** Using the standard library's derived `Debug` on a type with a logical field; giving a proof holder to generic code without a specification
- **Rules:** `logic.boundary.plain-data`, `logic.boundary.logical-data`, `export.invariants-closed`

## Scope

RFC 0002's decision 61 keeps exposed logical data inside verified code. Its decision 73 lets unverified code hold a value of a *sealed type*: a struct or enum of a verified crate whose logical data sits in its own fields. Such a value can be printed, stored in any container, sorted, and shown to other crates, and unverified code can neither build one nor open it.

This issue records what stays awkward after that, so that it is not forgotten, and one further relaxation.

What is still rejected, each with an example where there is one:
- the standard library's derived `Debug` on a type with a logical field, because what it generates hands every field to the formatter ([`logical-layer/derived-debug-on-type-with-proof-field.reject.lc`](../litmus/logical-layer/derived-debug-on-type-with-proof-field.reject.lc)); a `Debug` written by hand is accepted ([`logical-layer/debug-written-by-hand.accept.lc`](../litmus/logical-layer/debug-written-by-hand.accept.lc)). Derives of other crates that hand every field to their own code, such as `serde::Serialize`, are in the same position unless the logical fields are skipped;
- a proof holder lent by shared reference ([`logical-layer/share-proof-holder-with-unverified.reject.lc`](../litmus/logical-layer/share-proof-holder-with-unverified.reject.lc)), and any other exposed logical data given to code without a specification;
- a proof about a value that the type does not hold, which cannot be sealed: `(i: usize, @(i < xs.len()))` exposes its proof unless a struct holds `xs` too;
- an enum with a proof field shown to unverified crates, since Rust makes its fields public;
- a generic type whose logical data comes in through a type argument ([`logical-layer/own-generic-type-to-unverified.reject.lc`](../litmus/logical-layer/own-generic-type-to-unverified.reject.lc)).

The further relaxation: **a shared reference to exposed logical data that holds no pointer and no cell,** such as `&Option<@P>` or the reference to a proof field that a derived `Debug` makes, may be given to unverified code, also as a trait object. It is believed sound for two reasons: such a reference cannot be written through, and nothing that exposes logical data is accepted back from unverified code, so a marker it copies has nowhere to go. It has no argument written down.

## Questions it must answer

1. Whether the further relaxation is worth a second notion beside the sealed type, when its main use is one derive. The alternative is a derive of Locus's own.
2. Which types hold "no pointer and no cell", decided from their fields alone, so that no description of standard types is needed.
3. What a derive from another crate may be given, since `Serialize` reads fields and `Deserialize` builds the type and so must go through a verified constructor.
4. Whether a permission, which is a sealed type that is not `Copy` ([permissioned-pointers](permissioned-pointers.md)), should be allowed into unverified hands at all, where a duplicate would be more than a wrong fact.
5. The arguments, in [soundness-model](soundness-model.md): that a sealed value handed back is one that verified code built, and that nothing exposed comes back.

## Log

### 2026-11-16, conversation with the owner on how to build Locus

Raised when the question of derives came up under the rule as it first stood, in which a type with proof fields counted as exposed logical data: a derived `Debug` hands each field to the formatter, and the formatting code is unverified. The reply offered two relaxations, letting unverified code hold a value with an invariant without opening it, and lending a proof by shared reference, and recommended leaving both out of the first proposal.

### 2026-11-16, conversation with the owner on how to build Locus, continued

After seeing what the strict rule cost, a type with a private proof field that could be neither printed nor exported, the owner took the first relaxation:

> This sounds reasonable - can we make it failproof while offering such a relaxation? If so, let's do it

RFC 0002's decision 73 records it as sealed types, with three conditions: other crates reach no field that holds logical data or that a proof reads; the crate's own unverified functions still name no type that holds logical data; and `unsafe` code keeps such values intact (`assumed.unsafe.invariants`). The issue is renamed in its title to what remains. The second relaxation is narrowed above to references that cannot be written through.
