# Runtime code

What Locus knows about the values that runtime code computes. A proof is about values, and this chapter says which facts about them the language gives at each point of a function. [RFC 0004](../design/rfcs/0004-runtime-code.md) explains the design.

## Understood operations

r[runtime.operations]
Locus understands an operation of runtime code when Rust gives the operation syntax and it is applied to a type that stays Rust's: arithmetic, division, and remainder on machine integers; comparisons and the boolean operators; casts between integer types; indexing and range indexing of arrays and slices; building and matching tuples, structs, and enums; `if` and `match` as expressions, and `?`; taking a reference and following one, and following a `Box`; and `for` over a range of integers. It also understands these functions: `len` on arrays and slices; `Box::new`; and `checked_add`, `checked_sub`, and `checked_mul` on the integer types. Every other call of a function or a method is a call, of which Locus knows what the callee's signature says ([`contracts.calls.unrelated`](contracts.md)).

r[runtime.operations.returned]
Each understood operation has a value, which is a logical expression of its operands, and a condition under which it returns. Once the operation has returned, Locus knows that the condition held and that the result is the value. It knows neither before the operation.

r[runtime.operations.conditions]
Indexing an array or a slice returns only if the index is less than the length, and range indexing only if the range lies within the length. Division and remainder return only if the divisor is not zero and, for a signed type, the quotient is in the type's range. Every other understood operation always returns. `checked_add`, `checked_sub`, and `checked_mul` give `Some` of the true result when it is in the type's range, and `None` otherwise. A cast between integer types gives the value wrapped to the target type.

r[runtime.arith.wrapping]
`+`, `-`, and `*` on machine integers, and negation, give the true result wrapped to the type, whichever way the crate is built, and Locus takes no fact from one of them having returned. When the true result is in the type's range, the wrapped result is the true result. A shift gives the shift by the amount reduced to the type's width.

## Proofs that are kept

r[runtime.proofs.assigned]
A proof variable declared with `let mut` has its proposition written at its declaration, and may be assigned. An assignment is checked against that proposition at the values that the places it mentions have at that point. A proof is about the values it was made from: after one of those places is written, the variable holds a proof about the earlier values, which does not fit where a proof about the current values is required ([`proofs.versions`](proofs.md)). Locus does not assign it again by itself.

r[runtime.joins]
Where paths of a function join, Locus knows of the current values what it knows on every one of the paths. So a proof variable proves its proposition for the current values there only if it does at the end of each path.

r[runtime.loops]
The top of a loop is where the path into the loop joins the path from the end of each way through its body. There, a variable that the loop writes is known by its type and by the proof variables that prove their propositions for the current values on each of those paths, and a variable that the loop does not write keeps what was known of it. Locus asks for no proof that a loop ends.

r[runtime.loops.for-range]
In the body of `for i in a..b`, `a <= i && i < b` is known, and in the body of `for i in a..=b`, `a <= i && i <= b`. In any other `for` loop, the loop variable is known by its type.
