# Spike: snapshots and models as one family, by a written bound

A throwaway experiment for the design of snapshots and models, and for [traits-on-logical-types](../../issues/traits-on-logical-types.md). Nothing outside `spikes/` may depend on it.

## Question

The second round of [skeleton-typing](../skeleton-typing-2/README.md) could not give every type a snapshot type on stable Rust. Its fallback, one opaque type for every type that has no implementation, makes a `u32` read through an unbounded type parameter a different type from a `u32` read directly, and makes two unrelated foreign types one type.

**If a type parameter is read in logical code only under a bound that the program writes, as [RFC 0002](../../design/rfcs/0002-logical-layer.md) already asks for a model, does rustc then do all of it, with no fallback?**

1. Do generic and concrete code agree on a snapshot's type, with references and `Box` transparent and no borrow kept?
2. Can the default model be the snapshot, and a declared model replace it, with one implementation for each type?
3. What does rustc say when the bound is missing, and for a type that can never have one, such as a closure?
4. Can a struct have a field of a type that Locus knows nothing about, without the whole struct failing to compile?

## Setup

Plain Rust, edition 2024, rustc 1.97.0. No lowering and no macro: each file is the Rust that a lowering would have to produce, written by hand, as in [logical-markers](../logical-markers/README.md).

- `lib.rs` stands in for Locus's library. One trait, `Model`, has two associated types: `Snap`, the snapshot, and `Logic`, the model. It is implemented for the machine integers with `Nat` and `Int` for both, and for `&T`, `&mut T`, `Box<T>`, and `[T]` in terms of `T`'s. `snap(&x)` and `capture(&x)` are stubs that return a value of each.
- `cases/` holds thirteen programs. The first line of each says whether it must compile, or which error it must give.

`./run.sh` compiles each and compares.

## Results

Run on 2026-11-22. All thirteen gave the outcome their first line states.

| Case | Outcome | What it shows |
|---|---|---|
| `c01` | compiles | `fn keep<T: Model>(x: &T) -> T::Logic`, used at `u32`, gives a `Nat` where a `Nat` is wanted. A `Box<u32>` and a `&[u32]` read as a `Nat` and a `Seq<Nat>`. A snapshot taken through a `&mut` is used after the borrow ends and the place is written |
| `c02` | compiles | A struct with no declared model has its snapshot as its model, and an integer field of the captured struct is a `Nat`, the same as the field read as a place. A struct with a declared model reads as that model. A wrapper with no model keeps its field's snapshot: `capture(&w.s)` is the `Seq<Nat>` and `capture(&w).s` is the snapshot |
| `c03` | E0308 | Using the second where the first was meant is a type error |
| `c04` | E0277 | With no bound: "`T` cannot be read in logical code", and rustc's own "consider restricting type parameter `T` with trait `Model`" |
| `c05` | E0277 | A closure has no model. A generic function that asks for one refuses it, and one that does not takes it |
| `c06` | E0119 | A declared model beside a generated default conflicts |
| `c07` | E0277 | A method that asks for `T: Model`, on a type that also reaches the standard `push` through `Deref`: where the element type is still being inferred at the call, rustc commits to the method that asks, and then reports the closure |
| `c08` | compiles | The same where the element type is known and has no model: the call falls through to the standard method, and returns `()` |
| `c09`, `c10` | E0277 | A struct with a field of type `TcpStream`: naming the field's snapshot, or a plain `where TcpStream: Model` on the generated implementation, is an error at the declaration |
| `c11` | compiles | The same clause behind a higher-ranked lifetime, `where for<'a> TcpStream: Has<'a>`, is not checked at the declaration. The struct has no model and is still an ordinary struct |
| `c12` | E0277 | As `c11`, and then the struct is captured: "`TcpStream` cannot be read in logical code" |
| `c13` | E0308 | The field marked opaque: its snapshot is `Opaque<TcpStream>`, a second field's is `Opaque<SocketAddr>`, and passing one where the other is wanted is a type error |

## What it means

1. **A written bound gives everything the fallback could not.** There is no blanket implementation and no fallback, so a type has one snapshot type and one model type wherever it is read. rustc normalizes both through generic code, and its message for a missing bound says what to write.
2. **The default model can be the snapshot with no overlap,** because each type has exactly one implementation, written where the type is declared. That needs lowering to know, at the declaration, whether the program declares a model.
3. **With `Nat` and `Int` as the integers' snapshots, capturing a struct and then reading a field agrees with reading the field and then capturing it,** for a struct of plain data. They differ only for a field whose type declares a model, and there the difference is a type error.
4. **Some types can never be read:** a closure, a function item, and any type of a crate that Locus does not lower. A trait cannot be implemented for a closure except by a blanket implementation, which would overlap every other. So an implicit bound on every type parameter is not an option: it would refuse every closure argument.
5. **A field of an unknown type need not break its struct.** The implementation can be made conditional in a way rustc checks only on use, and marking the field opaque gives it a snapshot type that is distinct for each foreign type.
6. **A method that asks for the bound does not reliably fall through to the standard one.** It does when the element type is known and not when it is being inferred, so a design should not rest on it.

## Not tested

- No lowering was written. Whether a macro can generate these implementations from a declaration alone is not shown, in particular for a field whose type is an associated type of a parameter.
- `c11` rests on rustc not checking a higher-ranked `where` clause at the declaration. It does not on 1.97.0; whether that is meant to stay was not looked into.
- `Opaque<T>` keeps the lifetimes of a foreign type that has lifetime parameters.
- Trait objects, two crates, a foreign trait's generic method, whose own type parameter cannot be given the bound, and a ghost field whose type mentions `T`'s snapshot in a struct that has no bound on `T`.
- Locus's own checks. rustc types these programs and proves nothing.
