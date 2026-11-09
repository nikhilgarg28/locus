# Contracts

A runtime function states its contract in its signature: proof parameters are preconditions, and proofs in its result are postconditions. Types state invariants in proof fields. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

## Values in a signature

r[contracts.entry-values]
In a function's signature, each parameter denotes its value when the function is called. For a `&mut` parameter, that is the value of its referent at entry.

r[contracts.final]
`final!(r)` denotes the value that the referent of the mutable reference `r` has when the borrow ends: when neither `r` nor any reborrow of it is used again. When the borrow ends, the place that was borrowed holds that value. `final!` appears only in the propositions of a signature's result, where `r` is a `&mut` parameter or a `&mut` in the result. It never appears in a parameter's type, and never in a body, so no ghost value and no proposition formed in a body is computed from a value that the program has yet to produce. For a `&mut` parameter whose lifetime nothing else in the signature can hold, as rustc decides from the signature, counting the lifetimes that an `impl Trait` result or an `async fn` captures without naming them, the borrow ends when the function returns, after its locals are dropped. `final!(r)` reads no place.

## Proofs in data

r[contracts.invariants]
Every value of a type with proof fields satisfies them. Each construction of the type proves them, including constructions in code that macros and derives generate, and a struct update.

r[contracts.invariants.no-mutable-borrow]
No place based on a proof field, or on a field that a proof field reads, is mutably borrowed, whether by `&mut`, by `ref mut`, or by a binding that a pattern makes by reference when it matches through a `&mut`. The one exception is a field that a proof field reads, inside an `open!` block for its value ([`contracts.invariants.open`](#proofs-in-data)).

r[contracts.invariants.assignment]
A field that a proof field reads may be assigned when each proof field that reads it is proved for the new value. A proof field may be assigned a value whose proofs prove its proposition, and that assignment, too, proves each proof field that reads it. Either assignment means building the value again with that field changed, so the proofs it needs are those that building it would need. Inside the body of a runtime closure or an async block, an assignment that needs such a proof is rejected, as logical code there is.

r[contracts.invariants.proof-holders]
A field whose type holds a proof, such as an `Option<@P>`, counts as a proof field. A tuple component that a later component's proposition reads follows the rules for a field that a proof field reads.

r[contracts.invariants.open]
`open!(v => { ... })` opens the invariant of `v`, a local or the target of a `&mut` whose type has proof fields, for the length of the block. In the block, the fields that `v`'s proof fields read may be mutably borrowed, and they and the proof fields may be assigned without proof. Nothing observes `v` there: it is not used as a whole, by value or by reference, and its proof fields are not read. No borrow of one of its fields outlives the block, and the block is left only at its end: not by `return`, `break`, `continue`, `?`, or `.await`. At the end of the block, every proof field of `v` is proved for the values its fields then have, as for an assignment. A panic that would unwind out of the block aborts the process instead, so that no destructor, `catch_unwind`, or other thread sees `v` while its invariant is open. A block is not written inside a runtime closure or an async block, nor inside a block that opens the same value.

r[contracts.unsafe-impls]
No `unsafe impl` that a derive or another macro generates is allowed on a type with proof fields. A hand-written `unsafe impl` is the programmer's promise, under [`logic.unsafe.invariant`](logical-layer.md#unsafe-code), that the type's invariant is upheld.

r[contracts.invariants.observed]
A value of a type with proof fields satisfies them wherever code that Locus checks can observe it, which is everywhere a Locus value is observed: an `open!` block lets nothing observe the value it opens. Code that Locus does not check may hold, move, copy, or drop such a value, or call the type's own checked functions, but cannot construct one or write the fields its proofs read.