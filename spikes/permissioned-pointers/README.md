# Spike: `Box` and `Vec` on permissioned pointers, with no `unsafe`

A throwaway experiment for [RFC 0002](../../design/rfcs/0002-logical-layer.md), decision 59, and for [permissioned-pointers](../../issues/permissioned-pointers.md). Nothing outside `spikes/` may depend on it.

## Question

The owner's direction is that Locus's own library builds its collections in verified Locus on permissioned pointers, so that they contain no `unsafe` code. A permissioned pointer is an address together with a ghost *permission*: a value that only one holder can have, that says which memory the holder may use, and that costs nothing at run time.

1. **Is a permissioned pointer enough to write `Box` and `Vec` with no `unsafe` code of their own?**
2. **What does rustc already enforce** about permissions, when a permission is a zero-sized value that is not `Copy`?
3. **What else is needed,** in the primitive layer, in Rust, and in Locus?

## Setup

Plain Rust, edition 2024, rustc 1.97.0. A crate and workspace of its own.

- `src/perm.rs` is the trusted layer, and the only file with `unsafe`: 9 `unsafe` blocks and 2 `unsafe impl`s, behind 11 functions.
  - `Ptr<T>` is an address. It is `Copy` and gives no right to the memory.
  - `Perm<T>` is the permission for one block of slots. It is not `Copy` or `Clone`, and holds a `PhantomData<T>`.
  - The primitives are `alloc`, `free`, `free_in_place`, `write`, `take`, `get`, `get_mut`, `slice`, `slice_mut`, `move_slots`, and `Perm::empty`. Each has a `requires` and an `ensures` in its comment, which in Locus would be proofs.
- `src/pbox.rs` and `src/pvec.rs` are the collections, 220 lines with `#![forbid(unsafe_code)]`: `PBox` with `new`, `into_inner`, `Deref`, `DerefMut`, and `Drop`; `PVec` with `push`, `pop`, `swap_remove`, growth, slices, `Drop`, and an owning iterator.
- Plain Rust cannot check the `requires`, so there are two builds:
  - the default build, in which `Perm<T>` is zero-sized and nothing is checked;
  - `--cfg checked`, in which `Perm<T>` carries the state a proof would speak of, the block's address and which slots are initialized, and every primitive asserts its `requires` against it. It also counts blocks allocated, freed, and leaked.
- `tests/collections.rs` drives both collections: 1,000 `String`s, elements that count their drops, zero-sized elements, an iterator dropped half way. `tests/contracts.rs` breaks each `requires` on purpose, to show the checked build notices. `cases/` holds eight programs that must not compile.

`./run.sh` runs all of it.

## Results

Run on 2026-11-09.

| What | Result |
|---|---|
| Default build, `tests/collections.rs` | Passes |
| Sizes in the default build | `Perm<u64>` 0, `PBox<u64>` 8, `Option<PBox<u64>>` 8, `PVec<u64>` 24 bytes: the same as `Box` and `Vec` |
| Checked build, `tests/collections.rs` | Passes: no primitive's `requires` failed. 15 blocks allocated, 15 freed, 0 leaked |
| Checked build, `tests/contracts.rs` | 6 of 6: a second `take`, a read of an uninitialized slot, a write past the end, another block's permission, a `free` of a block that holds values, and a use after `free_in_place` each panic |
| Valgrind 3.22.0 on the default build's test | 0 errors, 0 bytes lost |
| `cargo clippy --all-targets`, both builds | No warnings |
| `unsafe` in `pbox.rs` and `pvec.rs` | None, and `forbid(unsafe_code)` rejects any |

rustc rejects each of these, with no help from a verifier:

| Program in `cases/` | Error |
|---|---|
| `use-after-free`: a use of the permission after `free` consumed it | E0382 |
| `copy-permission`: one permission used twice | E0382 |
| `two-mutable-borrows`: two live `&mut` to one slot | E0499 |
| `write-while-shared`: a write while a slice of the block is live | E0502 |
| `not-send`: a `PVec<Rc<u32>>` moved to another thread | E0277 |
| `drop-consumes-permission`: `drop` giving its permission to `free` | E0507 |
| `destructure-type-with-drop`: taking a `PBox` apart in `into_inner` | E0509 |
| `unsafe-in-a-collection` | `forbid(unsafe_code)` |

## Answers

**1. Yes for the code, no for the claim "that is all it takes".** `Box` and `Vec` were written with no `unsafe` code, at the layout and cost of the standard ones. But the pointer and its permission are only the middle of what is needed. The list is under 3.

**2. rustc does the ownership half.** With the permission a zero-sized value that is not `Copy`:

- one holder at a time, no use after it is given away, and no two live mutable borrows of a slot, all from the borrow checker;
- `Send`, `Sync`, variance, and the drop check, all from the permission's `PhantomData<T>`, so no collection needs an `unsafe impl`;
- no runtime cost.

What rustc cannot do is the other half: that the permission is for this address, that the index is in the block, and that the slot is initialized. Those are the `requires`, and they need proofs.

**3. What else is needed.**

In the primitive layer:

- **More than one pointer operation.** `Vec` needs a permission for a block with a state per slot, and operations to move a range, to borrow a range as a Rust slice, and to free through `&mut`. A permission for one value is enough for `Box` only.
- **A permission for nothing,** which any code may make, so that safe code can move a real permission out of a `&mut` place by swapping.
- **Freeing through `&mut`.** `Drop::drop` has only `&mut self`, so it cannot give a permission to a function that consumes it (E0507).

In Rust:

- **A type with a destructor cannot be taken apart** (E0509). `into_inner` and `into_iter` keep the destructor from running with `ManuallyDrop`, work through `&mut`, and leave a value whose invariant is broken and never restored.
- **A few standard functions stay necessary.** The collections call `mem::replace`, `ManuallyDrop::new`, and `drop`, and implement `Deref`, `Drop`, and `Iterator`. Swapping two places cannot be written in safe Rust at all, so it is a primitive too.

In Locus, none of which this spike could test:

- **Ghost values that are not `Copy`.** Every ghost value is `Copy` today (RFC 0002, decision 10). A permission is a ghost value that must not be.
- **Contracts on the primitives.** Each has a `requires` and an `ensures`. That is the contract form that [boundary-contracts](../../issues/boundary-contracts.md) defers, here needed only for Locus's own library.
- **The permission's state in the logic:** its address, its capacity, and what each slot holds, so that a collection's invariant and model can speak of them.
- **A mutable borrow of a slot,** whose `ensures` speaks of the value the borrow ends with, and which borrows a field that the collection's invariant reads.
- **Leaving an invariant broken for good,** at the end of `drop` and when a value is consumed without its destructor. `open!` restores the invariant; these never do.
- **Contracts on trait methods.** `v[i]`, `*b`, and `for x in v` go through `Index`, `Deref`, and `IntoIterator`, whose methods can state nothing yet.
- **The soundness of the primitive layer** stays trusted: that its `unsafe` code keeps Rust's rules whenever the `requires` hold. It replaces a specification of each standard function with about ten primitives and one argument.

Not covered by permissioned pointers at all:

- **Shared ownership and interior mutability.** `Rc`, `Arc`, `Cell`, `RefCell`, and `Mutex` need permissions that several holders share, and ghost state behind a runtime check.
- **What the compiler gives `Box` alone:** moving out of `*b`, coercing to `Box<dyn Trait>`, and matching through it.
- **Meeting Rust crates.** A Locus `Vec` is not `std::vec::Vec`, so passing one to a Rust crate needs a conversion.

## What this did not test

- **No proofs.** The checked build tests the `requires` only on the paths the tests ran. That the collections meet them on every path is what Locus would prove.
- **Rust's aliasing rules.** Miri could not be installed in the session. Valgrind finds invalid and leaked memory, not a reference used against Stacked Borrows or Tree Borrows. The argument that the primitives respect those rules, each reference lives no longer than the borrow of the permission it came from, is on paper.
- A panic in an element's destructor, growth by `realloc`, allocators other than the global one, unsized element types, and concurrency.
- How any of this reads as Locus source.
