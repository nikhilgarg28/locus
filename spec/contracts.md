# Contracts

A runtime function states its contract in its signature: proof parameters are preconditions, and proofs in its result are postconditions. Types state invariants in proof fields. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

## Values in a signature

r[contracts.entry-values]
In a function's signature, each parameter denotes its value when the function is called. For a `&mut` parameter, that is the value of its referent at entry.

r[contracts.final]
`final!(r)` denotes the value that the referent of the mutable reference `r`, a parameter, local, or result, has when the borrow ends: when neither `r` nor any reborrow of it is used again. When the borrow ends, the place that was borrowed holds that value. For a `&mut` parameter whose lifetime is a lifetime parameter of the function that appears nowhere else in its signature, bounds included, the borrow ends when the function returns, after its locals are dropped. `final!(r)` reads no place, and does not appear in preconditions.

## Proofs in data

r[contracts.invariants]
Every value of a type with proof fields satisfies them. Each construction of the type, including in code that macros and derives generate, proves them. No place based on a field that a proof field reads is assigned or mutably borrowed, nor is a proof field itself mutably borrowed. A field whose type holds a proof counts as a proof field. The same holds for a tuple component that a later component's proposition reads.

r[contracts.closed-to-rust]
No `unsafe impl` other than `Send` and `Sync` is allowed on a type with proof fields. A public enum whose proof fields read its payload fields is rejected.
