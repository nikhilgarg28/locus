# Permissioned pointers, and a standard library with no `unsafe` code of its own

- **Kind:** design
- **Status:** open
- **Blocks:** The standard library's RFC ([standard-library](standard-library.md)), which is to build Locus's collections on them
- **Rules:** `logic.unverified.plain-rust`, `logic.unsafe.invariant`

## Scope

RFC 0002's decision 59 records the owner's direction: Locus's own library provides permissioned pointers and builds `Box`, `Vec`, and the other collections on them in verified Locus, with no `unsafe` code of their own, and Rust's standard library stays available without proofs.

A permissioned pointer is an address together with a ghost permission: a value that only one holder can have, that says which memory the holder may use and what it holds, and that is erased like any ghost value. Verus's `PPtr` and `PointsTo` are the model.

This issue is the design of the pointers and of what they need from the language. A [spike](../spikes/permissioned-pointers/README.md) answered the first question, whether the pointers are all it takes: `Box` and `Vec` can be written with no `unsafe` code on about ten primitives, at the size and cost of the standard ones, and rustc enforces the ownership half of the discipline by itself. The pointers are not all it takes, and the questions below are the rest.

## Questions it must answer

1. **Ghost values that are not `Copy`.** Every ghost value is `Copy` today (RFC 0002, decision 10), and a proof is irrelevant, so it can be made twice. A permission must be neither. What kind of logical type it is, what its marker is, whether one may be dropped, which is a leak, and where one may appear, given that propositions are not linear.
2. **The primitives and their contracts.** Which operations the trusted layer has, and the `requires` and `ensures` of each. The spike needed eleven: allocate, free by value and through `&mut`, write, take, borrow shared and mutably, borrow a range as a slice either way, move a range, and a permission for nothing. These are contracts on code that Locus does not verify, the form that [boundary-contracts](boundary-contracts.md) defers, needed here only inside Locus's own library.
3. **What a permission is in the logic:** its address, its capacity, and what each slot holds, so that a collection's invariant and its model can speak of them. Whether one permission covers a block, as in the spike, or each slot has its own, which needs ghost collections of values that are not `Copy`.
4. **A mutable borrow of a slot.** Its `ensures` speaks of the value the borrow ends with, and it borrows a field that the collection's invariant reads, which `contracts.invariants.no-mutable-borrow` forbids outside an `open!` block.
5. **Leaving an invariant broken for good.** `Drop::drop` has only `&mut self`, and Rust does not let a type with a destructor be taken apart, so `drop`, `into_inner`, and `into_iter` each end with a value whose invariant is broken and that nothing observes again. `open!` always restores the invariant.
6. **Contracts on trait methods.** `v[i]`, `*b`, and `for x in v` go through `Index`, `Deref`, and `IntoIterator`. Without contracts on their methods a Locus `Vec` is verified inside and opaque to its users ([traits-on-logical-types](traits-on-logical-types.md)).
7. **Which part of Rust's library Locus must still understand.** Swapping two places cannot be written in safe Rust, so `mem::swap` and `mem::replace` are primitives too. Iterating a range, integer methods, slices, and `Option`'s methods are library code as well. Whether Locus's library restates them, specifies them, or treats some as part of the language.
8. **Shared ownership and interior mutability.** `Rc`, `Arc`, `Cell`, `RefCell`, and `Mutex` need permissions that several holders share and ghost state behind a runtime check. A permission with one holder does not give them.
9. **What the compiler gives `Box` alone,** such as moving out of `*b` and coercing to `Box<dyn Trait>`, which a library type cannot have.
10. **Meeting Rust crates.** A Locus `Vec` is not `std::vec::Vec`. How one becomes the other, and what `Vec` names in a `.lc` file.
11. **The soundness of the primitive layer:** the argument that its `unsafe` code keeps Rust's rules, aliasing included, whenever each `requires` holds. It is what replaces `trusted.library-specs` for collections, and it belongs with [soundness-model](soundness-model.md).
12. **Whether a program's own `unsafe` code can use the same pointers,** so that `assumed.unsafe.sound` is discharged for a function instead of assumed.

## Log

### 2026-11-09, conversation with the owner about PR #24

The owner set the direction and asked for a check of the claim behind it:

> Also we should record in RFC that Locus's own stdlib will support permissioned pointers and build the rest of the collections (say box/vector etc) using them and hence avoiding unsafe code. We will also have access to Rust's stdlib within Locus but with no proofs. In fact, can you please verify that permission pointers is all it takes to build say box or vec without unsafe?

The spike was written to check it. It ran `Box` and `Vec` with `unsafe` forbidden over a layer of eleven primitives, in a build that asserts every primitive's `requires` at run time: no assertion failed, every block was freed, and Valgrind found no error. rustc rejected a use after free, a copied permission, two mutable borrows of one slot, and a vector of `Rc` sent to another thread. It also rejected a `drop` that gives its permission away and an `into_inner` that takes the box apart, which is where questions 2 and 5 come from. Miri could not be installed, so Rust's aliasing rules were not tested. No proof was involved: the spike shows that the Rust can be written, not that Locus can verify it.
