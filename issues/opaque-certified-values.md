# What is still awkward at the boundary with unverified code

- **Kind:** design
- **Status:** later
- **Blocks:** Giving a proof holder to generic code that Locus does not verify; a derive of another crate on a type with a logical field
- **Rules:** `logic.boundary.plain-data`, `logic.boundary.loans`, `logic.boundary.logical-data`, `export.invariants-closed`

## Scope

RFC 0002's decision 61 keeps exposed logical data inside verified code. Two later decisions let unverified code touch what it cannot misuse:

- **Decision 73, sealed types.** Unverified code may hold a value of a struct or enum of a verified crate whose logical data sits in its own fields. Such a value can be printed, stored in any container, sorted, and shown to other crates, and unverified code can neither build one nor open it.
- **Decision 76, loans.** Unverified code may be lent, by shared reference, exposed logical data that lies in the structural part of its value's snapshot. It can read it, and can neither change it nor give anything logical back. So the standard library's derived `Debug` works on a type with a logical field ([`logical-layer/derived-debug-on-type-with-proof-field.accept.lc`](../litmus/logical-layer/derived-debug-on-type-with-proof-field.accept.lc)), and an `Option<@P>` can be printed ([`logical-layer/proof-holder-formatted.accept.lc`](../litmus/logical-layer/proof-holder-formatted.accept.lc)).

This issue records what stays awkward after both, so that it is not forgotten.

What is still rejected, each with an example where there is one:
- a derive of another crate that hands every field to a trait of its own, such as `serde::Serialize`, unless the logical fields are skipped, since a marker implements only the standard traits;
- exposed logical data given by value or by `&mut` to code that Locus does not verify, the standard library's included ([`logical-layer/option-through-black-box.reject.lc`](../litmus/logical-layer/option-through-black-box.reject.lc)), or lent to code that hands the reference back ([`logical-layer/share-proof-holder-with-unverified.reject.lc`](../litmus/logical-layer/share-proof-holder-with-unverified.reject.lc));
- a loan of logical data inside a cell ([`logical-layer/proof-cell-formatted.reject.lc`](../litmus/logical-layer/proof-cell-formatted.reject.lc)), or of a function that takes a proof ([`logical-layer/proof-function-lent-by-reference.reject.lc`](../litmus/logical-layer/proof-function-lent-by-reference.reject.lc));
- a proof about a value that the type does not hold, which cannot be sealed: `(i: usize, @(i < xs.len()))` exposes its proof unless a struct holds `xs` too;
- an enum with a proof field shown to unverified crates, since Rust makes its fields public;
- a generic type whose logical data comes in through a type argument ([`logical-layer/own-generic-type-to-unverified.reject.lc`](../litmus/logical-layer/own-generic-type-to-unverified.reject.lc)).

## Questions it must answer

1. What a derive from another crate may be given, since `Serialize` reads fields and `Deserialize` builds the type and so must go through a verified constructor.
2. Whether a permission, which is a sealed type that is not `Copy` ([permissioned-pointers](permissioned-pointers.md)), should be allowed into unverified hands at all, where a duplicate would be more than a wrong fact.
3. The arguments, in [soundness-model](soundness-model.md): that a sealed value handed back is one that verified code built, that a loan cannot be written through, and that nothing exposed comes back.
4. Whether a result that the caller discards, as in `std::hint::black_box(&found);`, need be held to the rule that an unverified function's result exposes no logical data.
5. Whether following a type parameter through the generic bodies of other crates is worth its cost. RFC 0002's decision 77 says not for now.

## Log

### 2026-11-16, conversation with the owner on how to build Locus

Raised when the question of derives came up under the rule as it first stood, in which a type with proof fields counted as exposed logical data: a derived `Debug` hands each field to the formatter, and the formatting code is unverified. The reply offered two relaxations, letting unverified code hold a value with an invariant without opening it, and lending a proof by shared reference, and recommended leaving both out of the first proposal.

### 2026-11-16, conversation with the owner on how to build Locus, continued

After seeing what the strict rule cost, a type with a private proof field that could be neither printed nor exported, the owner took the first relaxation:

> This sounds reasonable - can we make it failproof while offering such a relaxation? If so, let's do it

RFC 0002's decision 73 records it as sealed types, with three conditions: other crates reach no field that holds logical data or that a proof reads; the crate's own unverified functions still name no type that holds logical data; and `unsafe` code keeps such values intact (`assumed.unsafe.invariants`). The issue is renamed in its title to what remains. The second relaxation is narrowed above to references that cannot be written through.

### 2026-11-16, conversation with the owner on the standard library

Told that the standard library's derived `Debug` was still rejected on a type with a proof field, the owner said:

> This feels like a bad limitation - we should find a way to fix it I guess?

and agreed to the further relaxation that this issue had recorded. RFC 0002's decision 76 records it as loans, with the rule `logic.boundary.loans`. It is tighter in three ways than the sentence this issue carried until now, "a shared reference to exposed logical data that holds no pointer and no cell":
- What may be lent is decided from the snapshot: every logical type must lie in its structural part. So a `Vec<@P>` is lent, and a value of a type that Locus's library gives no snapshot is not.
- A function's signature and a closure's captures are left out. Whoever holds a reference to a function that takes a proof could call it, with a marker copied from another loan.
- The unverified function's result exposes no logical data, even behind a shared reference, since a reference that comes back could be to another value. So `std::hint::black_box(&found)` stays rejected. The reply to the owner had listed that example as one that would be accepted; it is not, and question 4 above asks whether a discarded result should matter.

A proof field prints as `Erased`, the marker's name, as RFC 0001 had already noted when it kept logical fields as marker fields.

The owner also agreed to leave the boundary at signatures (decision 77): Locus does not follow a type parameter through the generic bodies of other crates, the standard library's included.

### 2026-11-16, conversation with the owner on reaching the standard library

RFC 0002's decisions 80 to 83 end the specifications of standard functions. So the one code that may be given exposed logical data, apart from a loan, is Locus's own library, and "without a specification" above now reads "that Locus does not verify". The standard library's `Option::take` on an `Option<@P>` joins the list of what is rejected ([`logical-layer/lend-proof-holder-to-specified.reject.lc`](../litmus/logical-layer/lend-proof-holder-to-specified.reject.lc)); `match` does the same work ([`logical-layer/proof-holder-taken-apart.accept.lc`](../litmus/logical-layer/proof-holder-taken-apart.accept.lc)).

### 2026-11-23, a collection of Locus's library is a sealed type

RFC 0002's decision 113 puts a logical field in each collection of Locus's library. So a `Vec<u32>` of Locus's holds logical data in a field of its own and exposes none: by `logic.boundary.logical-data` it is a sealed type, where before it was plain data. Unverified code of another crate may hold one, move it, and give it back, and what comes back is known only by its type. An unverified function of a Locus crate names none, and takes the standard library's `Vec` or a slice.

A `Vec` of Locus's whose elements hold proofs is still not lent to unverified code, also by shared reference: the standard library's vector inside it has no snapshot, so `logic.boundary.loans` does not apply. So `{:?}` on a `Vec<Option<@P>>` stays among what is awkward.
