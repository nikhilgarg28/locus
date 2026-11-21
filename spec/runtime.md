# Runtime code

What Locus knows about the values that runtime code computes. A proof is about values, and this chapter says which facts about them the language gives at each point of a function. [RFC 0004](../design/rfcs/0004-runtime-code.md) explains the design.

## The context

r[runtime.facts]
What Locus knows at a point of a function is the proofs in scope there and the *context*: facts about value names ([`core.values`](core.md)). Each fact has one source, a rule of this chapter: an understood operation or a call that has returned, a condition that was evaluated, a write, the bounds of a `for`, a join, the end of a borrow, or the top of a loop. A fact is in the context only at points that execution reaches after its source, and no rule gives a fact because a proof asks for one.

## Understood operations

r[runtime.operations]
Locus understands an operation of runtime code when Rust gives the operation syntax and it is applied to a type that stays Rust's: arithmetic, division, and remainder on machine integers; comparisons and the boolean operators; casts between integer types; indexing and range indexing of arrays and slices; building and matching tuples, structs, and enums; `if` and `match` as expressions, and `?`; taking a reference and following one, and following a `Box`; and `for` over a range of integers. It also understands these functions: `len` on arrays and slices; `Box::new`; and `checked_add`, `checked_sub`, and `checked_mul` on the integer types. `put` on a slot of Locus's library is a write to the place that the slot was taken from ([`contracts.slots`](contracts.md)). Every other call of a function or a method is a call, of which Locus knows what the callee's signature says ([`contracts.calls.unrelated`](contracts.md)).

r[runtime.operations.returned]
Each understood operation has a value, which is a logical expression of its operands, and a condition under which it returns. Once the operation has returned, Locus knows that the condition held and that the result is the value. It knows neither before the operation.

r[runtime.operations.conditions]
Indexing an array or a slice returns only if the index is less than the length, and range indexing only if the range lies within the length. Division and remainder return only if the divisor is not zero and, for a signed type, the quotient is in the type's range. Every other understood operation always returns. `checked_add`, `checked_sub`, and `checked_mul` give `Some` of the true result when it is in the type's range, and `None` otherwise. A cast between integer types gives the value wrapped to the target type.

r[runtime.arith.wrapping]
`+`, `-`, and `*` on machine integers, and negation, give `wrap` of the true result at the operation's type ([`core.wrap`](core.md)), whichever way the crate is built, and Locus takes no fact from one of them having returned. A cast between integer types gives `wrap` of its operand at the target type. When the true result is in the type's range, `wrap` of it is the true result. A shift gives the shift by the amount reduced to the type's width.

r[runtime.conditions]
The condition of an `if`, a `while`, or a `match` guard is evaluated once, to a `Bool` term over the value names of its operands, in Rust's order of evaluation: an understood operation contributes its value, and a call the value name of its result. Where the condition held, that term is in the context, and where it did not, its negation is. A `while` that ends because its condition did not hold gives the negation after the loop.

## Writes and borrows

r[runtime.writes]
A write gives the place it writes a new value name ([`proofs.versions`](proofs.md)). After a write to a field, the value that holds the field has a new name too, equal to its old value with that field replaced. After a write to an element of an array or a slice, the sequence has a new name, equal to the old one updated at that index and of the same length; the write returns only if the index is less than the length.

r[runtime.borrows]
A field of a struct, a component of a tuple, and a field of an enum's variant are places of their own, each with its value names, and the value of the whole at a point is its constructor applied to the values its parts have there. A mutable borrow of a place suspends that place's value name while the borrow lives, and no other place's: a sibling keeps its name, and may be read, written, and borrowed. The target of the reference has value names of its own: the first equals the value the place had, and each write through the reference gives it a new one. When the borrow ends, the value it ends with ([`contracts.final`](contracts.md)) equals the target's value name at that point, and the place gets a new value name equal to it. A borrow of an element of an array or a slice suspends the whole sequence, and at its end the sequence is its earlier value updated at that index. A reborrow is a borrow of a place inside the target, and ends the same way. Where reborrows outlive the function that made them, as when they are returned, the value the outer borrow ends with is known there as the constructor applied, for each part, to the value its reborrow ends with when one is outstanding, and to the part's current value when none is. A borrow given to a call ends when the call returns, unless the callee's result can hold it, as rustc decides from the signature; then it ends when the result's borrow does. Either way the caller then knows of the place what the callee's proofs say of the value the borrow ended with. Where a part of the value has been moved out, Locus gives the place a new value name at the end of the borrow and no fact about it: the place is known by its type.

## Proofs that are kept

r[runtime.proofs.assigned]
A proof variable declared with `let mut`, of a type `@P`, has its proposition written at its declaration, as a template over the places it reads. Each assignment makes a new version of the variable's contents, whose type is the template at the value names those places have at that point, and the assigned proof is checked against it. No version is ever given another type: after one of those places is written, the variable still holds a proof about the earlier values, which does not fit where a proof about the current values is required ([`proofs.versions`](proofs.md)). Locus does not assign it again by itself. Such a variable is not mutably borrowed, and no closure assigns it. A `let mut` local of any other type that holds a proof keeps the propositions of its declaration ([`logic.ghost.type-identity`](logical-layer.md)).

r[runtime.joins]
Where the branches of an `if` or a `match` meet, a place that some branch wrote gets a new value name, with the fact that it equals the `if` or the `match`, on the evaluated condition or scrutinee, of the values it had at the end of each branch. A fact that held at the end of only some branches is not in the context afterwards. A proof variable gets a new version there, which Locus builds by cases from the versions at the end of each branch: it proves the variable's proposition at the new value names when every branch's version did at that branch's, and otherwise it proves the `if` of the branches' propositions. A branch that does not reach the meeting point, because it returns, breaks, or panics, takes no part.

r[runtime.loops]
At the top of a loop, each place that the loop writes gets a new value name, about which the context holds no fact. A proof variable that the loop assigns is an invariant of the loop when its version proves its proposition at the value names of the path into the loop, and at those of every path from the body back to the top, a `continue` included. Locus checks each of those, and then checks the body with a version that proves the proposition at the top's new value names. A proof variable that the loop assigns and that is not an invariant has no version at the top, and reading it before it is assigned again is an error. One that the loop does not assign keeps its version. Leaving the loop because its condition did not hold carries what is known at the top, with the negated condition; leaving by `break` carries what is known at the `break`; and where those meet, a place that they leave with different names gets a new one. Locus asks for no proof that a loop ends.

r[runtime.loops.for-range]
`for i in a..b` evaluates `a` and then `b` once, before the first iteration. In its body, with `lo` and `hi` the values they had then, `lo <= i && i < hi` is in the context, and for `a..=b`, `lo <= i && i <= hi`. This is a fact about those two values, and about a place that `a` or `b` reads only while that place has not been written since. In any other `for` loop, the loop variable is known by its type.
