# The logical layer

Locus adds logical types, whose values exist only for checking, and logical code, which computes with them. This chapter says what is logical, where logical values may appear, and what logical code may do. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

Every type is a runtime type or a logical type. Runtime types are Rust's types, including the program's own structs and enums. Logical types are `Int`, `Prop`, proof types `@P`, logical function types, tuples of logical types, types declared with `#[derive(Logical)]`, the model types that `#[derive(Model)]` generates, the snapshot type `Snap<T>` of each runtime type `T`, and the logical types of Locus's library, such as `Nat`, which is an `Int` with a proof that it is not negative.

## Logical types

r[logic.types.contents-logical]
The fields, payloads, and type arguments of a logical type are logical types, and so are the parameters and results of a logical function type. The one exception is `Snap<T>`, whose argument names a runtime type.

r[logic.types.tuples]
A tuple type is logical when it has at least one component and all its components are logical. The unit type `()` is a runtime type.

r[logic.types.positive]
A logical type, or a proposition declared with `prop`, refers to itself, directly or through other logical types, snapshot types and derived model types included, only in strictly positive positions: never in a parameter of a logical function type, which includes the premise of `==>`, the operand of `!`, and the domain of `forall` and `exists`.

r[logic.arith.nat-subtraction]
Arithmetic on `Int` and `Nat` is exact. Subtracting one `Nat` from another gives an `Int`, which may be negative.

## Ghost values

Values of logical types, proofs included, are *ghost*: they exist only for checking. They may appear in logical code, as parameters, results, locals, and fields of logical type, and inside runtime types as type arguments.

r[logic.ghost.erased]
Values of logical types have no runtime representation. At run time, a ghost value is a zero-sized marker: `Erased` for a proof, and `LogicalData` for any other logical value. Inside a runtime type and as the type of a field, a logical type is its marker, and a tuple keeps its shape, with a marker for each logical component. Logical code is never evaluated at run time.

r[logic.ghost.marker-traits]
`Erased` implements `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash`, which are right for proofs because any two proofs of a proposition are equal. `LogicalData` implements only `Clone`, `Copy`, and `Debug`. Neither implements a trait that makes a value from nothing, such as `Default`.

r[logic.ghost.types-forward]
Locus never infers a logical type backward from a later use. A local whose type holds logical values gets that type from its annotation, or from what initializes it: a call's result, Locus syntax, or another value whose type is known.

r[logic.ghost.interior-mutability]
A type with interior mutability may hold logical values. Its contents are part of no version and no model, so Locus knows only their types.

r[logic.ghost.runtime-parts-kept]
A runtime type that holds logical values keeps its runtime parts: an `Option` keeps its tag, and a `Vec` its length. Building one with a proof in it needs that proof, and taking the proof out gives back a proof of the same proposition.

r[logic.ghost.runtime-code-kept]
Erasure removes only logical code. A runtime call, condition, or statement runs as written, even when the expression it belongs to has a ghost value.

r[logic.ghost.forgotten]
Locus knows what logical values a runtime value holds only through code it checks and standard library code whose specification Locus's library declares. A value passed to a generic parameter of any other code, or turned into a trait object of a trait without a specification, holds markers in place of its logical values when it comes back, and no proof of them; a value of an invariant type still satisfies its invariant ([`logic.unsafe.invariant`](#unsafe-code)). So does a value read through a raw pointer, given another type by a pointer cast or by `transmute`, made by `mem::zeroed` or `MaybeUninit::assume_init`, or returned by foreign code or inline assembly.

r[logic.ghost.generic-instantiations]
The program's own generic code is checked again at each instantiation whose type arguments contain logical types.

r[logic.ghost.no-runtime-control]
Runtime control flow never depends on a logical value.

r[logic.ghost.positions-logical]
An argument for a proof parameter, or for a parameter of logical type, is a binding that holds a ghost value, or logical code, never a call of a runtime function. Elsewhere, a runtime call that produces a ghost value stays where it is. Outside logical code, a runtime value is never taken to its model implicitly: where a ghost value is expected, as such an argument or as the initializer of a logical field or a ghost local, a runtime value is written as logical code, such as `logic! { x }`.

r[logic.ghost.shown-by-source]
Outside delimited logical code, a binding is *shown to be ghost* only when it is a parameter of logical type, its type annotation holds a logical type, it is initialized by logical code, or it is bound by a proof pattern. An operator, method call, or field access there is logical code only when each operand is logical code or a binding shown to be ghost. Any other ghost value can be passed on and stored, but computing with it needs `logic!`.

r[logic.ghost.no-trait-impls]
Apart from `impl Model`, no logical type appears in an `impl`'s self type, its trait's arguments, or its associated types, and no trait method's signature holds logical values.

r[logic.ghost.no-unions]
No field of a union has a type that carries a forgeable proof or is an invariant type, since reading another field would make its value from memory of a different type.

## The boundary with unchecked code

r[logic.boundary.unchecked]
*Unchecked code* is code that Locus neither checks nor has a specification for: a crate's plain-Rust dependencies, standard library code that Locus's library does not specify, and code reached from either through trait objects or function pointers. When a crate is exported, every crate that uses it is unchecked too.

r[logic.boundary.completable]
A type holding logical values is *completable* when every value of its erased form is the erasure of one of its values. Locus judges this from types alone, and treats every case its judgment does not cover as not completable. `Int`, and the logical types that Locus's library declares completable, such as `Nat`, `Bool`, and `Seq<T>` for a completable `T`, are completable. A `#[derive(Logical)]` type is completable when one of its constructors has no proof fields and only completable fields, without recurring. A runtime type or a tuple is completable when every logical type it holds is. Proof types and logical function types are not.

r[logic.boundary.payload]
A value *carries a forgeable proof* when a logical type that is not completable appears in its type as a type argument, at any depth, as in `@P`, `Option<@P>`, or `Vec<(i: usize, @(i < n))>`. Writing such a place could store a marker taken from a proof of another proposition. A type of the program's own is an *invariant type* when it, or a type it holds, has a proof field; unchecked code can neither construct one nor write the fields its proofs read, because a proof field is reachable from no other code, so a value of an invariant type may pass through unchecked code and is still believed. These are distinct: `Option<@P>` carries a forgeable proof and is not an invariant type; `Interval` is an invariant type and carries no forgeable proof; `Vec<Interval>` carries neither.

r[logic.boundary.calls]
A function or closure with a parameter that carries a forgeable proof, like any other value that can be called with such an argument, is never coerced to a function pointer, never turned into a trait object, and is passed to a generic parameter only of the program's own code. It is not an `extern`, `#[no_mangle]`, or `#[export_name]` function. So it never reaches unchecked code, or specified standard library code, which could call it without its proof. What calling a value gives access to is [`logic.boundary.access`](#the-boundary-with-unchecked-code)'s concern.

r[logic.boundary.access]
A value *grants write access* to a place that carries a forgeable proof when code holding the value can store into that place. Locus decides this from the value's type: a `&mut` or `*mut` to such a place grants it; a type of the program's own grants whatever its fields' types grant, following its own definition; a standard library type grants what Locus's library describes for it, which covers the mutable handles `Cell`, `RefCell`, `Mutex`, `RwLock`, `OnceCell`, `OnceLock`, the sending halves of channels, and the guards, mutable iterators, and entries that give mutable access to their contents; a callable grants what calling it yields, through its result or the places it captures. Any other type whose value could hold such access, including one whose type arguments carry a forgeable proof and one Locus has no description for, is taken to grant it. What a closure or a value of the program's own type holds in fields whose types its definition fixes, with no forgeable proof among them, does not grant access.

r[logic.boundary.writes]
No value that grants write access to a place carrying a forgeable proof reaches unchecked code, as an argument, inside another value, or as the result of a callable that does; no raw pointer to such a place is made, by a raw borrow, by a cast or coercion of a reference to it, or by a call that returns a raw pointer into it, such as `as_mut_ptr`; and no value granting such access is coerced to a trait object or otherwise given a type that hides the access from Locus. A completable value that unchecked code may have written comes back as any value of its type: Locus forgets what it knew about it.

r[logic.boundary.escape]
Locus keeps a fact about a place only while no code that could write the place runs without a write Locus sees. A call to a function whose effect Locus does not know may write every place reachable through a `&mut` or a shared mutable handle it is passed, and every place that earlier escaped to unchecked code, and retains that access; so after it, Locus knows those places only by their types, until a write it sees, such as an assignment or a call whose postcondition gives the place a value, makes a fact about them hold again. A checked function's effect — the places it may write and the access it may retain — is summarized from its body and the functions it calls, and a specification may describe the effect of code Locus does not check more narrowly. How the summary is computed is the runtime semantics ([runtime-semantics](../issues/runtime-semantics.md)); until it is, every call to a function without a known effect is treated this way.

## Unsafe code

r[logic.unsafe.escape]
A place *escapes* from the point where its address reaches a raw pointer, by a raw borrow, by a cast or coercion of a reference to it, or by a call that returns a raw pointer into it, such as `as_mut_ptr`; from the point where an address into it is exposed as an integer; and from the point where a `&mut` to it, or a value granting write access to it, is passed to code whose effect Locus does not know. A `static mut`, and a static declared in an `extern` block, always counts as escaped. From then on Locus holds no fact about the place, the whole local, static, or referent it is part of, or anything they own, beyond their types and invariants, except where a write it sees gives the place a value again; and every later call to code whose effect it does not know may write it ([escape](#the-boundary-with-unchecked-code)).

r[logic.unsafe.no-conjuring]
Code that Locus checks never makes a value that carries a forgeable proof, or a value of an invariant type, from memory of another type. Where the source names such a type, no value of it is the target of a `transmute`, `transmute_copy`, or a cast from a pointer to another type, is read from a field of a union, or is taken from `mem::zeroed` or `MaybeUninit::assume_init`; and no `extern` function's signature names such a type in a parameter or result. The value is built from its runtime parts instead, with its proofs proved.

r[logic.unsafe.invariant]
An invariant type's proof fields are an obligation Locus places on `unsafe` code beyond Rust's own: every value of the type that code Locus checks can observe satisfies them, including one observed in a callback or while unwinding. Generic `unsafe` code meets this for free, since it cannot name the type's invariant; other `unsafe` code, in the crate's dependencies or, for an exported crate, in its users, meets it as a trusted assumption, which a hand-written `unsafe impl` records for its type. Opening an invariant under exclusive access and restoring it is not yet supported.

## Logical code

Logical code is the bodies of logic functions and proposition declarations; the contents of `logic!`, `prop!`, and `prove!`; propositions in types and proof holes; calls of logic functions and proof constructors; and operators on bindings shown to be ghost. In it, reading a runtime place gives its [snapshot](models.md), and a runtime value used as a value is its type's model.

r[logic.code.logical-values]
Logical code computes only with logical values, and every logical computation produces one: the result type of a logic function is logical. A runtime value enters logical code only where logical code reads a place, and is a logical value there: its snapshot, which is taken to its type's model wherever it is used as a value.

r[logic.eq.same]
`a === b` is the proposition that the logical values `a` and `b` are equal, and `a !== b` its negation. Every logical type has it, and it cannot be overloaded.

r[logic.eq.runtime-values]
On runtime values, `a === b` is the equality of their models. It implies nothing about the values beyond their models: not that their snapshots or their fields are equal.

r[logic.eq.snapshots]
Two snapshots are equal exactly when they are the same variant and their fields' snapshots are equal. The opaque parts of a snapshot, such as a `Cell`'s, are values that Locus knows nothing about, except that a move or a copy keeps them.

r[logic.eq.comparisons]
`==`, `!=`, `<`, `<=`, `>`, and `>=` in logical code give a `Bool`. For now they are defined only on `Int`, `Nat`, and `Bool`, where `a == b` is true exactly when `a === b`, and each ordering agrees with the order of the integers. A `Bool` used as a proposition stands for the proposition that it is true.

r[logic.code.no-runtime-calls]
Logical code does not call runtime functions or methods, and does not mutate runtime state.

r[logic.fn.total]
Every recursive call of a logic function is on a structurally smaller part of one of its arguments, or decreases the function's declared measure. For now, no logic function calls itself through another. A body that proof search completes is held to the same rule.

r[logic.const.well-founded]
A constant of logical type is not defined in terms of itself, directly or through other constants or logic functions.

r[logic.code.statics]
Logical code reads a static only when it is immutable. No `static mut`, and no static that is `extern`, `#[no_mangle]`, or `#[export_name]`, holds logical values that are not completable.
