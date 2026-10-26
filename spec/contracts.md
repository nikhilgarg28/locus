# Contracts

A runtime function states its contract in its signature: proof parameters are preconditions, and proofs in its result are postconditions. Types state invariants in proof fields. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

## Values in a signature

r[contracts.entry-values]
In a function's signature, each parameter denotes its value when the function is called. For a `&mut` parameter, that is the value of its referent at entry.

r[contracts.final]
`final!(r)` denotes the value that the referent of the mutable reference `r` has when the borrow ends: when neither `r` nor any reborrow of it is used again. When the borrow ends, the place that was borrowed holds that value. `final!(r)` reads no place.

## Proofs in data

r[contracts.invariants]
Every value of a type with proof fields satisfies them. Each construction of the type, including in code that macros and derives generate, proves them. A field that a proof field reads is not assigned or mutably borrowed on its own.
