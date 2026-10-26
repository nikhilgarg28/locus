# The logical layer

Locus adds logical types, whose values exist only for checking, and logical code, which computes with them. This chapter says what is logical, where logical values may appear, and what logical code may do. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

Every type is a runtime type or a logical type. Runtime types are Rust's types, including the program's own structs and enums. Logical types are `Nat`, `Prop`, proof types `@P`, logical function types, tuples of logical types, types declared with `#[derive(Logical)]`, and the logical types of Locus's library.

## Logical types

r[logic.types.contents-logical]
The fields, payloads, and type arguments of a logical type are logical types.

## Ghost values

Values of logical types, proofs included, are *ghost*: they exist only for checking.

r[logic.ghost.erased]
Values of logical types have no runtime representation. Parameters, result components, locals, and fields of logical type are absent from the program that runs, and arguments for them are not evaluated at run time.

r[logic.ghost.not-type-argument]
A logical type is never a type argument of a runtime type, and never instantiates a generic parameter of runtime code. The one exception is a proof in the payload of an `Option` or `Result` that is written as the declared type of a parameter, result, local, or field.

r[logic.ghost.no-runtime-control]
Runtime control flow never depends on a logical value.

r[logic.ghost.arguments-logical]
An argument for a parameter of logical type is logical code. A call to a runtime function cannot be such an argument.

## Logical code

Logical code is the bodies of logic functions and proposition declarations; the contents of `logic!`, `prop!`, and `prove!`; propositions in types and proof holes; and arguments for parameters of logical type. In it, a runtime place keeps its Rust meaning, and a runtime value used as a logical value is its type's [model](models.md).

r[logic.code.no-runtime-calls]
Logical code does not call runtime functions or methods, and does not mutate runtime state.

r[logic.fn.total]
Every recursive call of a logic function is on a structurally smaller part of one of its arguments, or decreases the function's declared measure.
