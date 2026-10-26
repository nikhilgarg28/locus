# The logical layer

Locus adds logical types, whose values exist only for checking, and logical code, which computes with them. This chapter says what is logical, where logical values may appear, and what logical code may do. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

Every type is a runtime type or a logical type. Runtime types are Rust's types, including the program's own structs and enums. Logical types are `Int`, `Prop`, proof types `@P`, logical function types, tuples of logical types, types declared with `#[derive(Logical)]`, `Opaque<T>` for each runtime type `T` without a model, and the logical types of Locus's library, such as `Nat`, which is an `Int` with a proof that it is not negative.

## Logical types

r[logic.types.contents-logical]
The fields, payloads, and type arguments of a logical type are logical types. The one exception is `Opaque<T>`, whose argument names a runtime type.

r[logic.types.positive]
A logical type, or a proposition declared with `prop`, refers to itself, directly or through other logical types, only in strictly positive positions: never in a parameter of a logical function type, which includes the premise of `==>`, the operand of `!`, and the domain of `forall` and `exists`.

r[logic.arith.nat-subtraction]
Arithmetic on `Int` and `Nat` is exact. Subtracting one `Nat` from another gives an `Int`, which may be negative.

## Ghost values

Values of logical types, proofs included, are *ghost*: they exist only for checking. They may appear in logical code, as parameters, results, locals, and fields of logical type, and inside runtime types as type arguments.

r[logic.ghost.erased]
Values of logical types have no runtime representation. Parameters and locals of logical type, proof parameters included, are absent from the program that runs, and a result whose type is entirely logical is `()`. Inside a runtime type, including a tuple, and as the type of a field, a logical type is the zero-sized marker type `Erased`. Arguments for removed parameters are not evaluated at run time.

r[logic.ghost.types-forward]
Locus never infers a logical type backward from a later use. A local whose type holds logical values gets that type from its annotation, or from what initializes it: a call's result, Locus syntax, or another value whose type is known.

r[logic.ghost.interior-mutability]
A type with interior mutability may hold logical values. Its contents are part of no version and no model, so Locus knows only their types.

r[logic.ghost.runtime-parts-kept]
A runtime type that holds logical values keeps its runtime parts: an `Option` keeps its tag, and a `Vec` its length. Building one with a proof in it needs that proof, and taking the proof out gives back a proof of the same proposition.

r[logic.ghost.runtime-code-kept]
Erasure removes only logical code. A runtime call, condition, or statement runs as written, even when the expression it belongs to has a ghost value.

r[logic.ghost.forgotten]
Locus knows what logical values a runtime value holds only through code it checks and standard library code whose specification Locus's library declares. A value passed to a generic parameter of any other code, or turned into a trait object of a trait without a specification, holds `Erased` in place of its logical values when it comes back, and no proof.

r[logic.ghost.generic-instantiations]
The program's own generic code is checked again at each instantiation whose type arguments contain logical types.

r[logic.ghost.no-runtime-control]
Runtime control flow never depends on a logical value.

r[logic.ghost.positions-logical]
An argument for a proof parameter, or for a parameter of logical type, is logical code: a call to a runtime function cannot be one. Elsewhere, a runtime call that produces a ghost value stays where it is.

r[logic.ghost.no-trait-impls]
Apart from `impl Model`, no logical type appears in an `impl`'s self type, its trait's arguments, or its associated types, and no trait method's signature holds logical values.

## The boundary with unchecked code

*Unchecked code* is code that Locus neither checks nor has a specification for: a crate's plain-Rust dependencies, standard library code that Locus's library does not specify, and code reached from either through trait objects or function pointers. When a crate is exported, every crate that uses it is unchecked too. A type holding logical values is *completable* when every value of its erased form is the erasure of one of its values: `Nat` and `Vec<Nat>` are, and `@P` and `Option<@P>` are not.

r[logic.boundary.calls]
A function whose parameters hold logical values that are not completable never reaches unchecked code: not as a function value or function pointer, not through generic code or a trait object, and not as an `extern`, `#[no_mangle]`, or `#[export_name]` function. Its calls are written outside any other macro's input or output.

r[logic.boundary.writes]
No value through which a place holding logical values that are not completable can be written reaches unchecked code, as an argument or inside another value. Such a place is behind a `&mut` or `*mut` in the value's type or its type arguments, at any depth, or inside a type that Locus's library specifies as writable through a shared handle, such as `Cell`, `RefCell`, and `Mutex`. No raw borrow of such a place is taken. What a closure or a value of the program's own type holds privately does not count.

## Logical code

Logical code is the bodies of logic functions and proposition declarations; the contents of `logic!`, `prop!`, and `prove!`; propositions in types and proof holes; and ghost arguments. In it, a runtime place keeps its Rust meaning, and a runtime value used as a value is its type's [model](models.md).

r[logic.code.logical-values]
Logical code computes only with logical values, and every logical computation produces one: the result type of a logic function is logical. A runtime value enters logical code only where logical code reads a place, and is a logical value there: its type's model, or an `Opaque<T>` for a type without one.

r[logic.eq.same]
`a === b` is the proposition that the logical values `a` and `b` are equal, and `a !== b` its negation. Every logical type has it, and it cannot be overloaded.

r[logic.eq.opaque]
Two values of an `Opaque<T>` are equal exactly when they are the same value. Where every field of `T` is visible, two values of the same variant whose fields are equal are equal.

r[logic.eq.comparisons]
`==`, `!=`, `<`, `<=`, `>`, and `>=` in logical code give a `Bool`. For now they are defined only on `Int`, `Nat`, and `Bool`, where `a == b` is true exactly when `a === b`, and each ordering agrees with the order of the integers. A `Bool` used as a proposition stands for the proposition that it is true.

r[logic.code.no-runtime-calls]
Logical code does not call runtime functions or methods, and does not mutate runtime state.

r[logic.fn.total]
Every recursive call of a logic function is on a structurally smaller part of one of its arguments, or decreases the function's declared measure.
