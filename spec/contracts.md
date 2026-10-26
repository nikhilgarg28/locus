# Contracts

A runtime function states its contract in its signature: proof parameters are preconditions, and proofs in its result are postconditions. Types state invariants in proof fields. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

## Values in a signature

r[contracts.entry-values]
In a function's signature, each parameter denotes its value when the function is called. For a `&mut` parameter, that is the value of its referent at entry.

r[contracts.final]
`final!(r)` denotes the value that the referent of the mutable reference `r`, a parameter, local, or result, has when the borrow ends: when neither `r` nor any reborrow of it is used again. When the borrow ends, the place that was borrowed holds that value. For a `&mut` parameter whose lifetime is a lifetime parameter of the function that appears nowhere else in its signature, bounds included, the borrow ends when the function returns, after its locals are dropped. `final!(r)` reads no place, and does not appear in preconditions.

## Proofs in data

r[contracts.invariants]
Every value of a type with proof fields satisfies them. Each construction of the type proves them, including constructions in code that macros and derives generate, and a struct update.

r[contracts.invariants.no-mutable-borrow]
No place based on a proof field, or on a field that a proof field reads, is mutably borrowed.

r[contracts.invariants.assignment]
A field that a proof field reads may be assigned when each proof field that reads it is proved for the new value. A proof field may be assigned a value whose proofs prove its proposition, and that assignment, too, proves each proof field that reads it.

r[contracts.invariants.proof-holders]
A field whose type holds a proof, such as an `Option<@P>`, counts as a proof field. A tuple component that a later component's proposition reads follows the rules for a field that a proof field reads.

r[contracts.unsafe-impls]
No `unsafe impl` that a derive or another macro generates is allowed on a type with proof fields.