+++
id = "language-ownership"
title = "Ownership and borrowing"
group = "Now"
spec_chapter = 1
order = 105
route = "specification/ownership.html"
description = "Moves, Copy, references, permissions, and mutation through borrowed data."
+++

# Ownership and borrowing

<!-- spec: 1.0:7 informative -->
Ownership determines which code may use, move or borrow a value. Locus follows Rust’s runtime ownership model and also checks logical observations before erasure, so a proof cannot observe data through an access that the runtime permissions forbid.

## Moves and derive

<!-- spec: 1.16:1 legality-rule -->
Runtime values move, as in Rust. A value can be used again after a by-value use only when its type is `Copy`; `#[derive(...)]` asks for it from the closed runtime-trait list `Clone`, `Copy`, `PartialEq`, `Eq`, `Debug`, in any order. `Copy` needs `Clone`, and `Eq` needs `PartialEq`. The additional compiler marker `#[derive(Logical)]` checks logical field classification and erases the declaration. A type holding evidence may derive `Clone`, `Copy`, and `Debug`, whose Rust prints the marker, and not `PartialEq`, since evidence is erased. Use after a move is an error. Reassigning a moved `let mut` restores it, including before a loop’s back edge and in each branch. Reading a `Copy` field copies that field even when its containing value is not `Copy`; moving another field leaves only that field unavailable. A `match` that moves no fields leaves the value whole. A proposition may mention a value only while it is whole, and evidence is never moved. The source move analysis supplies diagnostics. The separately checked permission/layout boundary is necessary for logical observations, which rustc cannot see after erasure; rustc independently checks emitted runtime moves. `tests/moves.rs` compares rejected runtime move shapes against Rust E0382.

## References

<!-- spec: 1.13:1 dynamic-semantics -->
Mutable loans are call-scoped; shared references may also persist in locals, fields and results under the [lifetime and provenance rules](11-models.md#shared-references). A parameter may be `x: &T` or `x: &mut T`, and an argument for it is a lend of a place, `&x`, `&mut x.f`, `&mut pair.0`; a stored shared reference must be created by a borrow expression, and stored mutable references are unsupported. Inside the callee a `&T` parameter is the value lent, read through its fields or whole when it is `Copy`, and never moved out of; a `&mut T` parameter is a mutable binding, assigned whole or by path, and its final value is written back into the caller's place when the call ends. In the parameter types its name means the value at entry, in the result type the value at return, and `old!(x)` names the entry value where the result needs both. Lending `&mut x` needs a `let mut x`. Two arguments of one call may not overlap when either is `&mut`, and an argument may not read a root another argument lends by `&mut`: lowering refuses the overlap, and the message carries rustc's code. A field that evidence in the same struct depends on cannot be lent by `&mut`; the whole value is lent, and whoever holds it can only replace it with a valid one.

<!-- spec: 1.13:2 dynamic-semantics -->
When a call panics, the writes it made through `&mut` before the panic are real: both interpreters report the panic with the values of the `&mut` parameters at that moment, and a Rust caller that catches the panic finds them.
