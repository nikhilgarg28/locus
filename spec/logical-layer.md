# The logical layer

Locus adds logical types, whose values exist only for checking, and logical code, which computes with them. This chapter says what is logical, where logical values may appear, and what logical code may do. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

Every type is a runtime type or a logical type. Runtime types are Rust's types, including the program's own structs and enums. Logical types are `Int`, `Prop`, proof types `@P`, logical function types, tuples of logical types, types declared with `#[derive(Logical)]`, and the logical types of Locus's library, such as `Nat`, which is an `Int` with a proof that it is not negative.

## Logical types

r[logic.types.contents-logical]
The fields, payloads, and type arguments of a logical type are logical types.

r[logic.arith.nat-subtraction]
Arithmetic on `Int` and `Nat` is exact. Subtracting one `Nat` from another gives an `Int`, which may be negative.

## Ghost values

Values of logical types, proofs included, are *ghost*: they exist only for checking. They may appear in logical code, as parameters, results, locals, and fields of logical type, and inside runtime types as type arguments.

r[logic.ghost.erased]
Values of logical types have no runtime representation. Parameters, result components, locals, and fields of logical type are absent from the program that runs, except that a proof parameter becomes a token that only code Locus checked can supply. Inside a runtime type, a logical type is `()`. Arguments for ghost positions are not evaluated at run time.

r[logic.ghost.runtime-parts-kept]
A runtime type that holds logical values keeps its runtime parts: an `Option` keeps its tag, and a `Vec` its length. Building one with a proof in it needs that proof, and taking the proof out gives back a proof of the same proposition.

r[logic.ghost.forgotten]
Locus knows what logical values a runtime value holds only through code it checks and standard library code whose specification Locus's library declares. A value passed to a generic parameter of any other code holds `()` in place of its logical values when it comes back, and no proof.

r[logic.ghost.no-runtime-control]
Runtime control flow never depends on a logical value.

r[logic.ghost.arguments-logical]
An argument for a parameter of logical type, for a proof parameter, or for a generic parameter instantiated with a logical type is logical code. A call to a runtime function cannot be such an argument.

## Logical code

Logical code is the bodies of logic functions and proposition declarations; the contents of `logic!`, `prop!`, and `prove!`; propositions in types and proof holes; and ghost arguments. In it, a runtime place keeps its Rust meaning, and a runtime value used as a logical value is its type's [model](models.md).

r[logic.code.no-runtime-calls]
Logical code does not call runtime functions or methods, and does not mutate runtime state.

r[logic.fn.total]
Every recursive call of a logic function is on a structurally smaller part of one of its arguments, or decreases the function's declared measure.
