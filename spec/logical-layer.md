# The logical layer

Locus adds logical types, whose values exist only for checking, and logical code, which computes with them. This chapter says what is logical, where logical values may appear, and what logical code may do. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

Every type is a runtime type or a logical type. Runtime types are Rust's types, including the program's own structs and enums. Logical types are `Int`, `Prop`, proof types `@P`, logical function types, tuples of logical types, types declared with `#[derive(Logical)]`, the model types that `#[derive(Model)]` generates, the snapshot type `Snap<T>` of each runtime type `T`, and the logical types of Locus's library, such as `Nat`, which is an `Int` with a proof that it is not negative.

## What a verified program means

The rules of this specification follow from one reading of a verified program, which [RFC 0002](../design/rfcs/0002-logical-layer.md#what-a-verified-program-means) explains. A verified function means a pure function of values in the logic. A place has a value at each point of the program, and a write gives it a new one, its next *version*; a `&mut` parameter is a value at entry and a value given back, which a signature's result names with `final!`. A proposition is about values, never about places, so it never changes its meaning; a value whose type reads another value is rebuilt when that one changes, with its proofs proved again, which is why a field that an invariant reads changes only when the whole value is replaced. Every type has an *erased* type, which is what rustc sees, and a meaning in the logic. Code that Locus does not verify is never believed about logical data: it never writes or returns a value whose type exposes logical data, and it is given one only as a loan by shared reference, which it reads and cannot change ([`logic.boundary.loans`](#the-boundary-with-unverified-code)). It may hold a value of a sealed type, whose fields it cannot reach, and what it gives back is known only by its type. Shared mutable state has no value in this reading: Locus knows only the type of what it holds, so that type never changes while the state exists. What this reading cannot express is rejected or opaque, never approximated, and a rule that contradicts it is a defect in the rule.

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

r[logic.ghost.type-identity]
Two types that hold logical values are the same type only when they are built the same way from the same types, and from propositions that convert to the same core term ([`core.conversion`](core.md)): the same formula, up to evaluation, about the same values. rustc compares only markers, so Locus checks this wherever one type must match another: an argument against its parameter, an assigned value against its place, an initializer against its field, and every use of a type parameter against the others. A proposition that reads another version of a place makes a different type, and so does one that is equivalent only by a proof, such as `0 < n` for `n > 0`. A proof by value is proved again instead, and a value that holds one is rebuilt from its parts.

r[logic.ghost.dependent-tuples]
A tuple type in which a later component's proposition reads an earlier component is one type. It instantiates a type parameter only as a whole, and never matches a generic tuple type such as `(A, B)`, whose components could then be replaced separately. A pattern takes such a tuple apart.

r[logic.ghost.interior-mutability]
A type with interior mutability may hold logical values. Its contents are part of no version, no snapshot, and no model, so Locus knows only their types: a proof taken out of a `Cell<Option<@P>>` is a proof of `P`, and a `Nat` taken out of a `Cell<Nat>` is some `Nat`. So the type of what such a type holds never changes while it exists: a proposition under interior mutability, at any depth, reads only what is fixed where its type is written, which is a local or a parameter at that point, a constant, or an immutable static. In the type of a field it reads no other field of the value, and in the type of a tuple component it reads no other component.

r[logic.ghost.runtime-parts-kept]
A runtime type that holds logical values keeps its runtime parts: an `Option` keeps its tag, and a `Vec` its length. Building one with a proof in it needs that proof, and taking the proof out gives back a proof of the same proposition.

r[logic.ghost.runtime-code-kept]
Erasure removes only logical code. A runtime call, condition, or statement runs as written, even when the expression it belongs to has a ghost value.

r[logic.ghost.generic-instantiations]
The boundary is checked for each instantiation. The program's own verified generic code is checked again at each instantiation whose type arguments expose logical data, and such an instantiation is rejected when a value whose type mentions one of those parameters reaches unverified code, in the function's body or in what it calls, other than as a loan ([`logic.boundary.loans`](#the-boundary-with-unverified-code)). A verified generic function's interface records, for each type parameter, whether it reaches unverified code. An unverified generic function is never instantiated with a type that exposes logical data ([`logic.boundary.plain-data`](#the-boundary-with-unverified-code)), except to be lent a value of it.

r[logic.ghost.no-runtime-control]
Runtime control flow never depends on a logical value.

r[logic.ghost.positions-logical]
An argument for a proof parameter, or for a parameter of logical type, is a binding that holds a ghost value, or logical code, never a call of a runtime function. Elsewhere, a runtime call that produces a ghost value stays where it is. Outside logical code, a runtime value is never taken to its model implicitly: where a ghost value is expected, as such an argument or as the initializer of a logical field or a ghost local, a runtime value is written as logical code, such as `logic! { x }`.

r[logic.ghost.shown-by-source]
Outside delimited logical code, a binding is *shown to be ghost* only when it is a parameter of logical type, its type annotation holds a logical type, it is initialized by logical code, or it is bound by a proof pattern. An operator, method call, or field access there is logical code only when each operand is logical code or a binding shown to be ghost. Any other ghost value can be passed on and stored, but computing with it needs `logic!`.

r[logic.ghost.no-trait-impls]
Apart from `impl Model`, no logical type appears in an `impl`'s self type, its trait's arguments, or its associated types, and no trait method's signature holds logical values. A runtime type that holds logical data may implement a trait; its methods take `Self`, so they are verified ([`logic.unverified.functions`](#unverified-functions-and-unsafe-code)), and unverified code may call them on a value of a sealed type.

r[logic.ghost.no-unions]
No field of a union has a type that holds a logical type ([`logic.unverified.plain-rust`](#unverified-functions-and-unsafe-code)), at any depth, as `Option<@P>`, `ManuallyDrop<Interval>`, `ManuallyDrop<Vec<Interval>>`, and `Nat` do. Reading another field would make its value from memory of a different type; and reading a union needs `unsafe`, so only an unverified function could read it, and none takes such a type.

## The boundary with unverified code

r[logic.boundary.unverified]
*Unverified code* is code that Locus does not verify: the crate's own unverified functions ([`logic.unverified.functions`](#unverified-functions-and-unsafe-code)), its plain-Rust dependencies, the standard library, and code reached from any of these through trait objects or function pointers. The operations that the language itself defines on Rust's built-in types, such as arithmetic, the length and indexing of arrays and slices, and building and matching an `Option`, are not calls of unverified code. When a crate is exported, every crate that uses it is unverified too. All other code of a Locus crate is *verified code*.

r[logic.boundary.logical-data]
A type *holds logical data* when a logical type appears in it at any depth: as the type itself; as a type argument or a tuple component; in a field of a struct, an enum, or a union that it names, private fields and the types of other crates included; in the parameters or the result of a function, a function pointer, or a closure; in what a closure captures; and in the type that an `impl Trait` or another opaque type hides. A struct or enum of a verified crate that holds logical data only in the declared types of its own fields, with none coming in through a type argument, is a *sealed type*. A type *exposes logical data* when it holds some that is not inside the fields of a sealed type. So `@P`, `Option<@P>`, `Vec<Nat>`, and a function that takes a proof expose logical data; a struct with a proof field holds logical data and exposes none, and neither does a reference to one, a vector of them, or a closure that takes one. A value whose type exposes logical data is never converted to a trait object, whose type would hide what it holds, except as a loan ([`logic.boundary.loans`](#the-boundary-with-unverified-code)).

r[logic.boundary.plain-data]
No exposed logical data reaches unverified code, apart from the two exceptions below. After type parameters are instantiated, no unverified function has a parameter, a result, or a type argument whose type exposes logical data, and none names a function, a static, or a constant whose type does. So verified code never gives unverified code a value whose type exposes logical data, whether by value, by mutable reference, inside another value, or captured in a closure, and never calls it at such a type; and unverified code calls a verified function only when that function's instantiated signature, its receiver included, exposes none. Unverified code may hold a value of a sealed type: it can move the value, drop it, and call the type's own functions on it, and what it gives back is a value of that type. One exception is inside Locus's own library: the types it provides in place of the standard library's collections, reference-counted pointers, cells, and locks keep the values they are given in the standard library's own types, and that is trusted, not checked ([`trusted.library`](../design/TRUST.md)). So a `Vec` or a `Cell` of Locus's library may hold exposed logical data, and the standard library's own `Vec::push`, `Option::take`, and `Cell::set` are unverified code, as `black_box` and `Any::downcast` are. The other is a loan ([`logic.boundary.loans`](#the-boundary-with-unverified-code)).

r[logic.boundary.loans]
Unverified code may be lent exposed logical data that it cannot change. After type parameters are instantiated, a parameter of an unverified function may be a shared reference to a value whose type exposes logical data, when every logical type in that type lies in the structural part of the value's snapshot ([`models.snapshots.opaque`](models.md#snapshots)): none inside an `UnsafeCell`, behind a raw pointer, or in a type whose snapshot is opaque ([`models.frozen-data`](models.md#snapshots)), and none in the signature or the captures of a function, a function pointer, or a closure. A type argument of the function may then expose logical data, when the instantiated signature holds it only inside such parameters. In verified code, a shared reference to such a value may be converted to a shared reference to a trait object whose own type holds no logical data. Nothing else is relaxed: the function's result and its other parameters expose no logical data, whether or not the caller uses the result. So unverified code reads what it is lent, cannot change it, and gives nothing logical back, and a marker that it copies out of a loan reaches no verified code.

r[logic.boundary.calls]
A function or closure whose signature exposes logical data is called only by verified code, each call checked against its contract. It is never coerced to a function pointer and never turned into a trait object; it is not an `extern`, `#[no_mangle]`, or `#[export_name]` function; and it is passed as a value only within verified code, to a generic parameter only of the program's own verified code, never to unverified code and never to a library operation, which could call it with arguments that its contract does not allow.

r[logic.boundary.foreign]
Only plain data crosses to foreign code, which is code that is not compiled from Rust. After type parameters are instantiated, no type that holds logical data ([`logic.boundary.logical-data`](#the-boundary-with-unverified-code)), sealed or not, appears in the signature of a function of a Locus crate that foreign code can call: one defined with an ABI that is not Rust's, or with `#[no_mangle]` or `#[export_name]`. Nor does one appear among the parameters or as the result of a function pointer type whose ABI is not Rust's, wherever that type appears in a Locus crate, written or inferred. As everywhere, a type holds logical data at any depth, so through a reference, a `Box`, and a raw pointer. The signature of a function declared in an `extern` block ([`logic.unverified.plain-rust`](#unverified-functions-and-unsafe-code)) and the type of a static that foreign code can reach ([`logic.code.statics`](#logical-code)) are held to the same. Foreign code could fill a value of such a type with any bytes, and reach the fields of one it is given. A function that foreign code can call is otherwise as any other function: it is verified or unverified by [`logic.unverified.functions`](#unverified-functions-and-unsafe-code), for which the `unsafe` of `#[unsafe(no_mangle)]` and `#[unsafe(export_name)]` is not `unsafe` code, and a verified one may build and use values that hold logical data in its body.

r[logic.boundary.escape]
A call changes only what its arguments let it change, and only while it holds them. A call may write a place only through a value it is given that reaches the place mutably. Which places a value can reach is decided from rustc's borrow facts, not from the value's visible type: a place is reachable when a loan of it flows into a lifetime of the value's type, mutably for a mutable loan and, for a shared loan, only where it reaches interior mutability, so a `&mut` held by a trait object, an opaque type, a closure, or a container counts. For verified code this follows from its verification; for unverified code it is the assumption [`assumed.unsafe.sound`](../design/TRUST.md). It holds for implicit calls too: a drop, an overloaded operator, or a callback. When the borrow ends, the place has a new version. Its value is what the contract of a verified function says, and otherwise unknown: Locus knows it only by its type, which for a sealed type includes its proof fields, and proofs about its earlier versions say nothing about it, until a write it sees makes a fact about it hold again. Once the borrow has ended, no later call changes the place, whatever the code that held the borrow did with its address. A value that unverified code returns is known only by its type, proof fields included, and from then on belongs to the code that received it. A verified function's summary says which of the places it is given it may write; an unverified function may write every place it is given mutably. The summary is computed from the function's body and the functions it calls; how is the runtime semantics ([runtime-semantics](../issues/runtime-semantics.md)).

## Unverified functions and unsafe code

r[logic.unverified.functions]
Every function of a Locus crate is verified or unverified. A function marked `#[verified]` is verified, and one marked `#[unverified]` is unverified. Without a mark, a function whose signature, its receiver included, holds logical data is verified. Any other function is unverified when it is an `unsafe fn`, or when its body, after macro expansion and including the closures written in it, contains an `unsafe` block or an expression whose type is a raw pointer, before or after coercion; otherwise it is verified. A function nested in another is judged by itself, and a foreign function is unverified. The initializer of a `const` or a `static` is treated as a function body. Verified code knows an unverified function only by its Rust type, as it knows a function of an unverified crate ([`logic.boundary.escape`](#the-boundary-with-unverified-code)).

r[logic.verified.restrictions]
A verified function is not an `unsafe fn`, and its body contains no `unsafe` block and no expression whose type is a raw pointer; `unsafe` that the compiler itself generates for an operation it has checked does not count. A mark chooses a function's kind and never waives a rule: a function that is verified by its mark or by its signature, and that breaks this one, is rejected. Where Locus has something to verify and cannot, the function is rejected, and never becomes unverified instead. A verified function with no logical code and no logical data has nothing to prove, and is accepted whatever Rust it uses.

r[logic.unverified.plain-rust]
An unverified function of a Locus crate is plain Rust. It holds no logical code: no proof parameter, proposition, proof, or proof hole, and no `logic!` or `prove!`. No type that holds logical data ([`logic.boundary.logical-data`](#the-boundary-with-unverified-code)) appears in its signature, in the type of an expression or pattern in its body, or in the signature of a function or the type of a static that its body names, so `#[unverified]` on a function whose signature holds logical data is rejected. So such a function handles no proof, no logical data, and no value of a type with proof fields, sealed or not, since it could reach the type's fields. The signature of a foreign function is held to the same rule. A type parameter of an unverified function is never instantiated with a type that exposes logical data ([`logic.boundary.plain-data`](#the-boundary-with-unverified-code)), except to be lent a value of it ([`logic.boundary.loans`](#the-boundary-with-unverified-code)); it may be instantiated with a sealed type, which the function as written does not name.

r[logic.unsafe.invariant]
A sealed type's proof fields are an obligation that Locus places on `unsafe` code beyond Rust's own: every value of the type satisfies them. The crate's own unverified functions meet it by handling no type that holds logical data ([`logic.unverified.plain-rust`](#unverified-functions-and-unsafe-code)), so no function of the crate makes such a value from memory of another type, by `transmute`, by a pointer cast, or by `mem::zeroed`. Generic `unsafe` code meets it when it is sound, since it cannot name the type's fields; other `unsafe` code, in the crate's dependencies or in unverified crates that use the crate, meets it as a trusted assumption, which a hand-written `unsafe impl` records for its type.

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
A constant of logical type is not defined in terms of itself, directly or through other constants or logic functions. The same holds for the logical data in the initializer of any constant or static whose type holds some: it does not depend on the item itself, directly, through other constants and statics, through logic functions, or through a field of the item. While an initializer is checked, the proof fields of the item it initializes are not available. A cycle of addresses among statics is not such a dependence.

r[logic.code.statics]
Logical code reads a static only when it is immutable. No `static mut`, and no static that is `extern`, `#[no_mangle]`, or `#[export_name]`, has a type that holds logical data.
