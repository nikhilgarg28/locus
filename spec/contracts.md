# Contracts

A runtime function states its contract in its signature: proof parameters are preconditions, and proofs in its result are postconditions. There is no other form: no separate clause states a precondition, so a signature without logical data asks nothing of its caller. Types state invariants in proof fields. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

## Values in a signature

r[contracts.entry-values]
In a function's signature, each parameter denotes its value when the function is called. For a `&mut` parameter, that is the value of its referent at entry.

r[contracts.final]
`final!(r)` denotes the value that the referent of the mutable reference `r` has when the borrow ends: when neither `r` nor any reborrow of it is used again. When the borrow ends, the place that was borrowed holds that value. `final!` appears only in the propositions of a signature's result, where `r` is a `&mut` parameter or a `&mut` in the result. It never appears in a parameter's type, and a body never writes it. What a body may do with a final value that a proof it holds mentions is `contracts.final.pending`. For a `&mut` parameter whose lifetime nothing else in the signature can hold, as rustc decides from the signature, counting the lifetimes that an `impl Trait` result or an `async fn` captures without naming them, the borrow ends when the function returns, after its locals are dropped. `final!(r)` reads no place.

r[contracts.final.pending]
Until a mutable borrow ends, the value it ends with occurs only inside propositions and proofs. An expression whose type is neither a proof type nor `Prop` is rejected when its core term mentions that value, however the value got there: inferred for a `_`, or returned by a logic function. So a proof about a borrow's final value may be bound, combined with other proofs, and returned before the borrow ends, and no ghost value is computed from it. Once the borrow has ended, the value is the one the place holds ([`runtime.borrows`](runtime.md)), and nothing restricts it.

## Calls

r[contracts.calls.unrelated]
What verified code knows about the result of a call is what the callee's contract says, a proof in its result, and nothing else: no fact is attached to a function from outside its signature. Two calls of one function with equal arguments are unrelated unless such a proof relates them: a fact about the result of the first is not a fact about the result of the second.

## Proofs in data

r[contracts.invariants]
Every value of a type with proof fields satisfies them. Each construction of the type proves them, including constructions in code that macros and derives generate, and a struct update.

r[contracts.invariants.reads]
A proof field *reads* the fields that its proposition mentions. A proposition in the type of a field mentions only other fields of the same struct, variant, or tuple, and constants: never a field whose type is a proof type, and never the value as a whole. It reads a field through its owned parts, a `Box`, and a shared reference, and never through a `&mut` or into a type with interior mutability. When it mentions a field whose own type has proof fields, it reads that whole field.

r[contracts.invariants.no-mutable-borrow]
No place based on a proof field, or on a field that a proof field reads, is mutably borrowed, whether by `&mut`, by `ref mut`, by a closure's capture, or by a binding that a pattern makes by reference when it matches through a `&mut`.

r[contracts.invariants.assignment]
A field that a proof field reads is never assigned, and neither is a place inside it; nor is it initialized again after its value was moved out. It changes only when the whole value is replaced, by an assignment to the value or to something that contains it, or by taking the value apart and building it again, and either way every proof field is proved for the new value. A field that no proof field reads is assigned like any other, and a proof field that no other proof field reads may be assigned a value whose proofs prove its proposition.

r[contracts.invariants.proof-holders]
A field whose type holds a proof, such as an `Option<@P>`, counts as a proof field. A tuple component that a later component's proposition reads follows the rules for a field that a proof field reads.

r[contracts.unsafe-impls]
No `unsafe impl` that a derive or another macro generates is allowed on a type with proof fields. A hand-written `unsafe impl` is the programmer's promise, under [`logic.unsafe.invariant`](logical-layer.md#unverified-functions-and-unsafe-code), that the type's invariant is upheld.

r[contracts.invariants.observed]
A value of a type with proof fields satisfies them at every point of the program. It is built with its proofs proved, and a field that a proof field reads never changes in place. Code that Locus does not verify may hold, move, copy, or drop such a value, or call the type's own verified functions, but cannot build one or reach the fields that hold logical data or that its proofs read ([`logic.boundary.plain-data`](logical-layer.md#the-boundary-with-unverified-code)). So nothing observes such a value in a broken state: not a destructor, not code that catches a panic, and not another thread. Dropping a value takes it apart for the last time: its own destructor is given a value that satisfies its proof fields, and the destructors of its fields run afterwards.

r[contracts.slots]
`take_out` of Locus's library moves the value out of the target of a mutable reference and gives a slot for that place, and `put` on the slot writes a value there. From the one call to the other the place holds no value. After `put` it holds the value that `put` was given, as after an assignment. A panic that would unwind past a slot aborts the process, so nothing observes the place without a value.

r[contracts.slots.local]
A slot is bound to a local variable by the `let` whose initializer is the call of `take_out`. That variable is used only as the receiver of `put`: it is not moved, borrowed, assigned, captured, stored, or returned. Every path that leaves the variable's scope, other than by a panic, has passed it to `put`, and no `.await` lies between the two calls. Only a verified function calls `take_out`, whose result holds a proof.