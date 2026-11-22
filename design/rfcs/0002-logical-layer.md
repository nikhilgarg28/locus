# RFC 0002: The logical layer

- **Status:** proposed
- **Issue:** [logical-types-and-models](../../issues/logical-types-and-models.md), and parts of [proposition-syntax](../../issues/proposition-syntax.md) and [runtime-semantics](../../issues/runtime-semantics.md)
- **Supersedes:** none

## Summary

This RFC defines what Locus adds to Rust:
- logical types, propositions, and proofs;
- logic functions and logic blocks;
- models, which say what runtime data means in the logic;
- versions, which say which values a proposition is about;
- contracts on runtime functions, including what a `&mut` parameter means in one.

The design in brief:
- **One reading.** A verified function means a pure function of values in the logic. Every rule below follows from that reading, and what it cannot express is rejected or opaque ([What a verified program means](#what-a-verified-program-means)).
- **Logical types** describe values that exist only for checking:
  - `Int`, the language's one built-in data type;
  - propositions, of type `Prop`, and proofs of them, of type `@P`;
  - logical functions;
  - data declared with `#[derive(Logical)]`.

  Locus's library builds `Nat`, `Bool`, `Seq`, and the rest from these.
- **Proofs and logical values are ghost.** They have no runtime representation, and Locus tracks them itself.
  - At run time each is a zero-sized marker: `Erased` for a proof, and `LogicalData` for any other logical value. So `Vec<Nat>` is a `Vec<LogicalData>` at run time, and keeps its length. An exported program removes ghost parameters and locals, except where Rust still needs them, as for a function used as a value ([RFC 0001](0001-compiler-pipeline.md)).
  - The two markers implement different traits, so rustc itself rejects `==` on two `Vec<Nat>`s, and still accepts it on values that hold proofs, which are all equal.
  - Locus believes what a value holds only through code it verified, and through its own library. No other code is believed about a proof or a logical value, and none writes or returns one in the open: the most it is given is a loan of one by shared reference, which it cannot change (`logic.boundary.loans`). So no Rust code can forge or convert one.
  - Contracts are visible only to code that Locus verifies. In build mode, every crate that uses a Locus crate is verified. In export mode, what other crates can reach is plain Rust, with no logical values and no contracts.
- **Logical code is pure and total.** This covers logic functions, `logic!` blocks, propositions, and proofs. It computes only with logical values, reads runtime values through their **models**, and never calls runtime code.
- **Snapshots and models.** Every runtime value has a **snapshot**, `Snap<T>`, which is what it is, field by field. A type may also have a **model**, which is what it means: declared by hand, derived with `#[derive(Model)]` from its fields' models, or declared by Locus's library. There is no default model. Logical code reads places as snapshots, uses values through their models, and writes `snap(x)` to compare whole values. Equal models never imply equal snapshots.
- **A proposition is about versions.** Every write to a place makes a new version of it, as in SSA. A proposition is about the versions it read, so a proof never silently carries over to a later value.
- **A runtime function states its contract in its signature.** Proof parameters are preconditions, and proofs in the result are postconditions.
  - In a signature, every parameter means its value at entry.
  - For a `&mut` parameter `x`, `final!(x)` is the value its referent has when the borrow ends. This is a prophecy in the sense of RustHorn and Creusot. It is written only in a signature's result.
- **Types can carry proofs** as invariants of the program's own types, and inside any runtime type, such as `Option<@P>` or `Vec<(i: usize, @(i < n))>`. A field that a proof reads changes only when the whole value is replaced, so a value with an invariant satisfies it at every point of the program.
- **A function is verified whole, or not at all,** and which it is follows from the function itself. One whose signature holds logical data is verified. Any other is unverified when it contains `unsafe` code or handles a raw pointer, and verified otherwise. `#[verified]` and `#[unverified]` say so explicitly, and never waive a rule. An unverified function is plain Rust, with no logical data in its signature or its body, which verified code knows only by its Rust type. What verified code needs to know about a result, it checks at run time, or assumes with `unsafe assume!(_)`: a proof taken on trust, which every result that relies on it lists.
- **Unverified code is given no logical data that it could forge.** No value whose type *exposes* logical data reaches code that Locus does not verify, as an argument, a type argument, or a capture, and unverified code calls a verified function only through a signature that exposes none. A *sealed type*, a struct or enum of the program's own whose proofs read only its own fields, exposes nothing: unverified code may hold a value of one, and can neither build one nor open it. The rule is checked for each instantiation of generic code. It has two exceptions: Locus's own library, whose collections are trusted to keep what they are given, and a *loan*, a shared reference to logical data that unverified code can read and can neither change nor give back, which is how a derived `Debug` prints a value with a proof field.
- **The standard library is reached through Locus's own.** The types that the language itself uses, such as integers, slices, `Option`, and `Box`, stay Rust's, and the language defines what their built-in operations mean. For collections, reference-counted pointers, cells, and locks, Locus's library provides types of its own around the standard library's, with contracts in the signatures of their methods, and a program names them with an explicit `use locus::std::…`. Everything else is called as plain Rust. A contract is always a signature: no fact is attached to a function from outside it.
- **A verified claim holds relative to the contracts of what Locus does not verify.** Code that Locus does not verify is taken to be sound Rust: it changes only what it is given, while it holds it. A proof in shared mutable state reads nothing that can change under it. Programs need no new syntax for this.

## Motivation

[RFC 0001](0001-compiler-pipeline.md) decides how Locus checks Rust and erases its own additions, but not what those additions mean. Three issues wait on a common core:
- logical types and models ([logical-types-and-models](../../issues/logical-types-and-models.md));
- syntax ([proposition-syntax](../../issues/proposition-syntax.md));
- runtime semantics ([runtime-semantics](../../issues/runtime-semantics.md)).

That core is what a proof is, how logical code sees runtime data, and how a function promises something about what it returns or changes.

A second reason is soundness. Suppose every proof had the same zero-sized Rust type, and Locus believed a value's proposition from its type. Then safe, ordinary Rust could turn a proof of one proposition into a "proof" of another:

```rust
fn convert<T: 'static, U: 'static>(value: T) -> Option<U> {
    let boxed: Box<dyn Any> = Box::new(value);
    boxed.downcast::<U>().ok().map(|b| *b)
}

let trivial: @(1 == 1) = _;
match convert::<@(1 == 1), @(0 != 0)>(trivial) {
    Some(p) => divide(x, 0, p), // a "proof" that 0 != 0
    None => 0,
}
```

At run time both proof types would be the same Rust type, so the downcast would succeed, and Locus would believe the result because of its type. This RFC removes the belief rather than the type. Locus tracks proofs itself, and keeps track of what a value holds only through code whose meaning it knows. Code like `downcast`, whose meaning it does not know, is never given a proof at all, and a marker that such code makes or returns proves nothing.

## Design

### What a verified program means

Every rule in this RFC follows from one reading of a verified program. The RFC for the runtime semantics ([runtime-semantics](../../issues/runtime-semantics.md)) will define it precisely; this section fixes its shape, so that each rule can be held against it.

- **A verified function means a pure function of values.** This is Aeneas's reading of borrow-checked Rust. A place has a value at each point of the program, and a write gives it a new one, its next [version](#versions). A `&mut` parameter is a value at entry and a value given back, which a signature's result names with `final!`. A returned `&mut` is a way to compute the borrowed place's new value from the value the borrow ends with.
- **Only verified functions are read this way.** A function that contains `unsafe` code or handles a raw pointer is [unverified](#unverified-functions-and-unsafe-code): the reading gives a raw pointer's target no value, and a place's address is no part of its value. To verified code such a function is an unknown function of its Rust type.
- **A proposition is about values, never about places.** So it never changes its meaning: after `n = n + 1`, a proof about the old `n` is still a proof about that value. A type may read values, and a value whose type reads another value is rebuilt when that one changes, with its proofs proved again. That is why a field that an invariant reads changes only when the whole value is replaced.
- **Every type has two faces.** Its *erased* type is what rustc sees, with a marker for each logical part. Its *meaning* is a type in the logic, in which a proof type is its proposition.
- **Unverified code is never believed about logical data.** It never writes or returns a proof or a logical value in the open, and the most it is given is a loan of one by shared reference, which it reads and cannot change (`logic.boundary.loans`). So the only code that Locus believes about one is code whose meaning it knows: verified code, and Locus's own library. What unverified code may also hold is a value of a *sealed type*, whose proofs read only its own fields and whose fields it cannot reach. Such a value means the same wherever it goes, so what comes back is believed to be a value of its type, proofs included, and nothing more ([The boundary with unverified code](#the-boundary-with-unverified-code)).
- **Shared mutable state has no value in this reading.** What sits behind an `UnsafeCell` is part of no value, so Locus knows only its type. That type must therefore never change while the cell exists: a proposition under interior mutability reads only what is fixed where its type is written.
- **What the reading cannot express is rejected or opaque, never approximated.** A snapshot that cannot be shown finite is opaque; a type that would have to follow a place is rejected.

A rule that this reading contradicts is a defect in the rule. The three reviews before this section each found such rules one at a time; the fourth found that they shared causes, which is why the reading is now stated ([soundness-model](../../issues/soundness-model.md)).
### A first look

```rust
logic fn sorted(xs: Seq<Nat>) -> Prop {
    forall |i: Nat, j: Nat| i <= j && j < xs.len() ==> xs[i] <= xs[j]
}

fn binary_search(xs: &[u32], target: u32, ordered: @sorted(xs)) -> Option<usize> { ... }
fn linear_search(xs: &[u32], target: u32) -> Option<usize> { ... }

fn find(xs: &[u32], target: u32, ordered: Option<@sorted(xs)>) -> Option<usize> {
    match ordered {
        Some(proof) => binary_search(xs, target, proof),
        None => linear_search(xs, target),
    }
}

fn bump(counter: &mut u32, room: @(counter < 100)) -> @(final!(counter) == counter + 1) {
    *counter += 1;
    _
}
```

- `sorted` is a logic function that defines a proposition about a sequence of naturals. A `&[u32]` passed to it is seen through its model, a `Seq<Nat>`.
- `binary_search` needs a proof that `xs` is sorted. Calling it on unsorted data is a compile-time error.
- `find` takes a proof that may be absent. At run time it still knows whether one was passed.
- `bump` promises that the counter's final value is one more than its value at entry, given room below 100. `_` asks proof search for the proof, and the kernel checks it.

Exported to Rust ([RFC 0001](0001-compiler-pipeline.md)), the program is what remains, with `Erased` for the proof that `find` may hold:

```rust prose generated Rust
fn binary_search(xs: &[u32], target: u32) -> Option<usize> { ... }
fn linear_search(xs: &[u32], target: u32) -> Option<usize> { ... }

fn find(xs: &[u32], target: u32, ordered: Option<Erased>) -> Option<usize> {
    match ordered {
        Some(_) => binary_search(xs, target),
        None => linear_search(xs, target),
    }
}

fn bump(counter: &mut u32) {
    *counter += 1;
}
```

Every call of these functions is checked, so none can be called without its proof. Rust users could reach them only through wrappers, as [Export mode](#export-mode) shows. In build mode, the program that runs keeps every ghost parameter and value as a zero-sized marker instead, which no one reads ([Ghost values](#ghost-values)).

```rust prose the program that runs in build mode
fn binary_search(xs: &[u32], target: u32, ordered: Erased) -> Option<usize> { ... }
fn bump(counter: &mut u32, room: Erased) -> Erased { ... }
```

### Syntax and names

This RFC adds these forms to [RFC 0001's](0001-compiler-pipeline.md#locus-syntax-and-locus-names), which include named tuple components such as `(i: usize, @(i < n))`, under the same rule that Locus syntax appears only where rustc rejects the tokens:

| Form | Where | Example | Why Rust cannot mean it |
|---|---|---|---|
| Proof constructor | Expression | `Even::Step(2) @ h` | Rust has no binary `@` in expressions |
| Proof pattern | Pattern | `Even::Step(m) @ k` | Rust's `@` binding needs an identifier on its left, not a path |
| Final value | Expression | `final!(r)` | `final` is a reserved word, so Rust can neither define nor invoke a macro of that name |
| Logical function type | Type | `logic fn(Int) -> Prop` | Rust has no type beginning `logic fn` |
| Measure | Attribute of a logic function | `#[decreases(n)]` | It appears only on a logic function, which is itself Locus syntax |

Inside logical code, which Locus parses itself, it also uses `===`, `!==`, `==>`, `forall |x: T| P`, and `exists |x: T| P`. rustc rejects `===` as an invalid comparison operator (tested).

It adds these names to Locus's prelude, which programs shadow as they do std's (`compat.names.shadowable`):
- the attributes `#[verified]` and `#[unverified]`, which say which kind a function is ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code));
- the logic function `model`;
- the logical types `Int`, `Nat`, `Bool`, `Seq`, `Snap`, and the others of Locus's library, and the logic functions `snap` and `model`;
- the derive `Model`, beside the trait of that name;
- the propositions `True`, `False`, `And`, `Or`, and `Exists`.

**Naming.** By convention, logic functions are named in snake_case, as Rust's functions are, and `prop` declarations in CamelCase, as Rust's enums are. So a reader can tell how a proposition is used: a logic function by its definition, and a declared proposition by matching its arms.

How proofs are built beyond the forms this RFC fixes, including using a logic function's definition and rewriting along an equality, is for a third foundational RFC ([proof-construction](../../issues/proof-construction.md)).

### Logical types

Every type is either a runtime type or a logical type.

**Runtime types** are Rust's types:
- the program's own structs and enums, including those with logical fields;
- Rust's generic types, whatever their arguments.

**Logical types** have no runtime representation:
- `Int`, the integers, the one logical data type the language provides;
- `Prop`, the type of propositions;
- `@P`, the type of proofs of the proposition `P`;
- logical function types, written `logic fn(A) -> B`;
- tuples with at least one component, all of them logical. The unit type `()` is a runtime type;
- structs and enums declared with `#[derive(Logical)]`, which may be recursive without `Box` and describe finite values;
- `Snap<T>`, for each runtime type `T`: the type of `T`'s snapshots, each of which holds what a value of `T` is, field by field ([Snapshots and models](#snapshots-and-models));
- the model types that `#[derive(Model)]` generates, one for each type it is derived for;
- the logical types of Locus's library, which it builds from these: `Nat`, `Bool`, `Seq<T>`, `Set<T>`, `Map<K, V>`, `Maybe<T>`, and others. This RFC uses them in examples, and assumes only what it says about them; their definitions and operations are for the RFC for Locus's standard library ([standard-library](../../issues/standard-library.md)).

`Nat` is an `Int` with a proof that it is not negative:

```rust prose library definition
#[derive(Logical)]
struct Nat {
    value: Int,
    nonnegative: @(value >= 0),
}
```

**A logical type contains only logical types.** Its fields, payloads, and type arguments are logical. `Seq<Nat>` is a logical type, and `Seq<u32>` is an error. A runtime value enters logical data through its [model](#snapshots-and-models) or its snapshot. The one exception is `Snap<T>`, whose argument names the runtime type whose values it holds.

**A logical type refers to itself only positively.** A recursive `#[derive(Logical)]` type, or group of types, may contain itself as a field, a payload, a type argument of a positive logical type, or the result of a logical function type, but never as the parameter of a logical function type. Otherwise `struct Bad { f: logic fn(Bad) -> @False }` would prove `False` without recursion. This is Lean's strict positivity, the same rule as for [inductive propositions](#inductive-propositions).
- It counts the types a logical type reaches through snapshots and derived models. With `#[derive(Model)] struct Holder { b: Bad }`, the field `f: logic fn(Holder::Logic) -> @False` mentions `Bad` in a parameter, and is rejected, and so is `logic fn(Snap<Holder>) -> @False`.
- The parameters and results of a logical function type are logical types. A logic function may take a runtime value ([Logic functions](#logical-code)), but a logical function type may not, so no runtime type's fields hide a negative occurrence.
- The domain of `forall` or `exists` counts as a parameter of a logical function type. So `struct Bad { h: @(forall |b: Bad| False) }` is rejected too.
- Through a library type, a logical type may refer to itself only in the arguments that the library declares positive. `Seq`, `Maybe`, and tuples are positive in their arguments. A set, which is a predicate on its elements, is not ([standard-library](../../issues/standard-library.md)).

**Classification belongs to the type, not the binding.** There is no `ghost` keyword: a parameter, result, local, or field is ghost exactly when its type is logical.

### Ghost values

Values of logical types, proofs included, exist only for checking. They are *ghost*. They may appear wherever a value of their type can:
- in logical code;
- as parameters, results, locals, and fields of logical type;
- inside runtime types, as type arguments: `Option<@P>`, `Vec<Nat>`, `Result<u32, @P>`, or `Vec<(i: usize, @(i < n))>`.

**Representation.** No ghost value has a runtime representation. At run time, each is a zero-sized **marker**:
- **`Erased` for a proof type, and `LogicalData` for every other logical type.** Which crate defines them depends on the mode (RFC 0001), but each is one type, for every proposition or for every logical data type, and neither carries a proposition or a value. Their names are what rustc's errors and `Debug` output show, so `` `LogicalData: PartialEq` is not satisfied `` tells the reader that the value is logical.
- **In build mode, every ghost position keeps its marker** in both programs of RFC 0001: parameters, arguments, locals, results, and fields. So `fn halve(n: u32, even: @(n % 2 == 0)) -> u32` runs as `fn halve(n: u32, even: Erased) -> u32`, and a call keeps its proof argument as the marker. rustc then infers and resolves the program exactly as it is written, ghost values included.
- **An exported program removes ghost parameters and locals,** with every argument for them, and a result whose type is entirely logical becomes `()`, so that its Rust reads as a programmer would write it (RFC 0001).
- **Inside runtime types, and as the type of a field, logical types are markers** in both modes.
  - A tuple keeps its shape, with markers for its logical components, even when every component is logical, so its layout, and code that handles it generically or in macro input, are unchanged. A named result `(out: u8, @P)` is `(u8, Erased)`, and `(Nat, @P)` is `(LogicalData, Erased)`.
  - So `Option<@P>` is `Option<Erased>`, `Vec<Nat>` is `Vec<LogicalData>`, and `Vec<(usize, @P)>` is `Vec<(usize, Erased)>`.
  - Where Rust needs a value of such a type, the ghost value is its marker: `v.push(n)` pushes `n`'s marker.
  - The markers implement exactly the traits that their logical types implement (below). So rustc itself refuses a `Default` for `(u32, @P)`, and `==` on two `Vec<Nat>`s.
- **The runtime parts of a type are all kept.** An `Option` keeps its tag, and a `Vec` its length.
- **A reference to a logical value is that value.** Matching `&Option<@P>` with `Some(p)` binds `p` to the proof.
- **Logical types flow forward.** rustc infers types over the markers, so it knows that a value is a marker but not which logical type it stands for. Locus works that out itself, and never infers a logical type backward from a later use. A local whose type holds logical values gets that type from its annotation, or from what initializes it: a call's result, Locus syntax such as `logic! { 3 }`, or another value whose type is known. Write `let mut v: Vec<Nat> = Vec::new();`, not `let mut v = Vec::new();` followed by ghost pushes.
- **The same type means the same propositions.** rustc compares only markers, so `Option<@P>` and `Option<@Q>` are one type to it. Locus takes two types that hold logical values to be the same only when they are built the same way from the same propositions: the same formula, about the same values. It checks this wherever one type must match another: an argument against its parameter, an assigned value against its place, an initializer against its field, and every use of a type parameter against the others, as in `Cell::swap` or `Vec::append`. An equivalent proposition spelled differently, such as `0 < n` for `n > 0`, is a different type for now. A proof by value is proved again, and a holder is rebuilt from its parts. Converting a holder along a proved equivalence is for RFC 0003 ([proof-construction](../../issues/proof-construction.md)).
- **Types with interior mutability may hold logical values.** Their contents are part of no version, no snapshot, and no model ([Snapshots and models](#snapshots-and-models)), so Locus knows only their types. That is enough for proofs, which are irrelevant: a proof taken out of a `Cell<Option<@P>>` is a proof of `P`, because every value of type `@P` is one. A `Nat` taken out of a `Cell<Nat>` is some `Nat`. It is enough only while the cell's type cannot change. One cell can be seen through two shared references, each with the type it was given, so a proposition under interior mutability, at any depth, reads only what is fixed where its type is written: a local or a parameter at that point, a constant, or an immutable static. `let slot: Cell<Option<@(n > 0)>>` is about `n` as it is at the `let`, and stays so. In the type of a field or of a tuple component, such a proposition reads no other field and no other component, which an assignment could change under the cell ([Proofs in data](#proofs-in-data)). [The boundary with unverified code](#the-boundary-with-unverified-code) keeps code that Locus does not verify from writing them, and The cells and locks of Locus's library keep their contents' types ([standard-library](../../issues/standard-library.md)).

**Runtime code runs as written.** Erasure removes logical code and nothing else.
- A `let`, block, `if`, or `match` whose value is ghost keeps its runtime calls, conditions, and statements, and its patterns keep their runtime parts.
- So `let Some(p) = check(n) else { return 0 };` keeps the call and the early return, as `let Some(_) = check(n) else { return 0 };`.
- And `if n % 2 == 0 { _ } else { panic!("odd") }`, as a function's result of type `@P`, keeps its condition and its panic.
- A `const` or `static` of logical type is logical code, so its initializer never needs to run.

**Calls are checked, not guarded.** A function's proof parameters are its preconditions, and Locus checks the proof for each one at every call. No call escapes that check:
- within a crate, every file is checked, and a function whose parameters hold proofs never reaches code that Locus does not verify ([The boundary with unverified code](#the-boundary-with-unverified-code));
- across crates, contracts are visible only to code that Locus verifies: in build mode every crate that depends on a Locus crate is verified, and in export mode what other crates can reach states no contracts ([Export mode](#export-mode)).

**What follows a value.** Locus tracks ghost contents itself: which proofs and logical values a runtime value holds. It can, because a value that holds any is only ever handled by code whose meaning it knows:
- code that Locus verifies;
- Locus's own library, whose types keep what they are given ([Assumptions and the standard library](#assumptions-and-the-standard-library)).

No other code is given one in the open ([The boundary with unverified code](#the-boundary-with-unverified-code)):
- **Other generic code.**
  - Passing a value to a generic parameter of unverified code would instantiate that code with the erased type, where `Option<@P>` and `Option<@Q>` are one type. So the call is rejected.
  - Turning such a value into a trait object, such as `dyn Any`, is rejected too.
  - This is why the `convert` example above is rejected: it gives a proof to `Box<dyn Any>`.
  - The generic code of the standard library, and of other crates, is unverified in this sense.
- **Raw memory.** Reading through a raw pointer, giving a value another type by a pointer cast or by `transmute`, and making one with `mem::zeroed` or `MaybeUninit::assume_init` all need `unsafe`, so only an unverified function does them, and no unverified function handles a type that holds logical values or proofs ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)). To rustc, `Option<@P>` and `Option<@Q>` are the same type, so a cast between them changes nothing that rustc could reject; Locus rejects the function that names either.
- **Locus's library, and Rust's built-in types.**
  - Locus's `Vec`, `Cell`, and the other types it provides in place of the standard library's keep what their values hold, so `for (i, ok) in checked` keeps each element's proof.
  - Arrays, slices, tuples, `Option`, `Result`, and `Box` need no library. They are Rust's own, and the language knows what building one and taking one apart do.
  - Which types the library provides is [standard-library](../../issues/standard-library.md).
  - The `?` operator is built in on an `Option` or a `Result` that holds no proof ([RFC 0004](0004-runtime-code.md)). On a value that holds a proof it goes through the `Try` trait, and is for [traits-on-logical-types](../../issues/traits-on-logical-types.md).
- **The program's own verified generic code** is checked for each instantiation whose type arguments hold logical data: no value whose type mentions such a parameter may reach unverified code there. An unverified generic function of the program is other generic code, as above.
  - If checking those instantiations would not end, as with polymorphic recursion, the program is rejected.
  - Checking such code once for every instantiation is [traits-on-logical-types](../../issues/traits-on-logical-types.md).
- **Traits.**
  - Logical types implement `Clone`, `Copy`, and `Debug`, and nothing else, so `LogicalData` implements only these. rustc itself then rejects runtime code that would compare, order, or hash logical data: `==` on two `Vec<Nat>`s, which would compare only their lengths, or a derived `PartialEq` on a struct with a `Nat` field, which would ignore it.
  - Proof types also implement `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash`, and so does `Erased`. These are right for proofs, because any two proofs of a proposition are equal. So a derived `PartialEq` on a struct with proof fields works, and ignoring them is correct. That is why proofs and logical data have different markers.
  - No logical type implements a trait that makes a value from nothing, such as `Default`.
  - What the standard derives generate is checked like any other code: a derived `Clone` of a type with logical data fields copies them too ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)).

Further rules:
1. **Never a runtime decision.** Runtime `if`, `match`, `while`, and the other control flow never depend on a logical value. A match on an `Option<@P>` tests its runtime tag, not the proof.
2. **Arguments of ghost parameters are ghost values written as such.** An argument for a proof parameter, or for a parameter of logical type, is a binding that holds a ghost value, or logical code: a hole, a `logic!` block, a proof constructor, or a call of a logic function. The binding need not be [shown to be ghost](#logical-code): `let even = checked_even(n); halve(n, even)` passes `even` as it is.
   - A runtime value is passed there as logical code, which takes it to its model: `grow(n, logic! { x })`, not `grow(n, x)`. Lowering never looks up a callee's signature, which for a method would need the receiver's type, so the source says it ([Logical code](#logical-code)).
   - The same holds wherever a ghost value is expected outside logical code. A runtime value is never taken to its model implicitly: a logical field is initialized as `Tally { hits: 0, count: logic! { 0 } }`, not `count: 0`, because knowing that `count` is logical needs `Tally`'s definition, and a ghost local as `let k: Nat = logic! { n };`.
   - A runtime function that returns a proof is called in a statement or `let` of its own first: `let even = checked_even(n); halve(n, even)`, not `halve(n, checked_even(n))`. An exported program removes the argument with its parameter, so the call could not stay where it was.
   - Everywhere else a ghost value has a runtime slot, its marker, and a runtime call that produces it stays where it is: as a logical field's initializer, a logical tuple component, a payload such as the `h` of `Some(h)`, or an argument instantiating a generic parameter with a logical type. `Interval { lo, hi, ordered: checked_order(lo, hi) }` keeps the call. In build mode the call returns the marker itself, and an exported program writes `ordered: match checked_order(lo, hi) { _ => Erased }`, so its order and its temporaries are as in the source.
   - Moving a call out of an argument list instead would change when its temporaries are dropped, or conflict with a borrow of the receiver, so erasure never does that.
3. **A call is kept even when its result is ghost.** A call to a runtime function whose whole result is ghost stays in the runtime program, as a statement.
4. **No logical values in trait implementations, for now.**
   - Apart from `impl Model`, no logical type appears in an `impl`'s self type, in its trait's arguments, or in its associated types, and no trait method's signature holds logical values.
   - Otherwise unverified generic code could call such a method. With `impl From<Option<@P>> for Absurd`, generic code bounded by `From` could build an `Absurd` from any `Some(Erased)`.
   - Traits with logical values are [traits-on-logical-types](../../issues/traits-on-logical-types.md).
   - The rule applies to `impl`s as written. A runtime type that holds logical data, such as one with a proof field, may implement a trait whose signatures hold none: its methods take `Self`, so they are verified. When the type is sealed, unverified code that holds a value of it may call them, as the standard library's formatting code calls a `Display` implementation ([The boundary with unverified code](#the-boundary-with-unverified-code)). A generic `impl` of the program's own, used at an instantiation with logical types, is checked there ([What follows a value](#ghost-values)).
5. **No logical types in unions.** No field of a union has a type that holds a logical type, at any depth ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)). Reading a union's field reinterprets what another field wrote, as `transmute` does: with `union Pun { yes: Option<@(1 == 1)>, no: Option<@(0 != 0)> }`, writing `yes` and reading `no` would turn a proof of a true proposition into one of a false one, and a field of type `ManuallyDrop<Interval>`, or `ManuallyDrop<Vec<Interval>>`, would make intervals from whatever bytes another field wrote. `union Tagged { bits: u64, count: Nat }` is rejected too: reading a union needs `unsafe`, and an unverified function takes no type that holds logical data, so no code could ever read `Tagged`.

**Deciding what is logical.** Before rustc runs, Locus decides only what the source shows ([RFC 0001](0001-compiler-pipeline.md#the-lowered-program)): Locus syntax, the names of Locus's macros and their aliases, `derive(Logical)`, `derive(Model)`, `impl Model`, and paths to Locus's logical types such as `Nat`, and which bindings are shown to be ghost ([Logical code](#logical-code)). It never needs an expression's type or a callee's signature.
- Ghost values reach rustc as markers, so rustc infers and resolves the program as written: which method a call selects, which generic arguments it instantiates, and where markers flow.
- After rustc, Locus learns from its facts where each marker went, and works out which logical type it stands for, forward from signatures and annotations.
- Every assumption made before rustc is confirmed against the facts, by the identity of the item each name resolved to. A wrong one is an error or a second run, never a program with another meaning.
- A ghost value used in runtime code, such as a branch on a marker, is reported as Locus's error, such as "runtime code cannot branch on a logical value", with rustc's own error, if any, as a note.

### The boundary with unverified code

*Unverified code* is code that Locus does not verify:
- the crate's own [unverified functions](#unverified-functions-and-unsafe-code): those that contain `unsafe` code or are marked `#[unverified]`;
- the crate's plain-Rust dependencies, which are compiled as they are;
- the standard library, `Vec::push` as much as `black_box` and `Any::downcast`, apart from the built-in operations that the language defines ([Assumptions and the standard library](#assumptions-and-the-standard-library));
- code reached from any of these through trait objects or function pointers;
- foreign code, which is not compiled from Rust and is held to a stricter rule than the rest (decision 88, below);
- in export mode, every crate that uses the exported crate.

**What a verified claim promises.** Verified code is correct relative to the contracts of everything it does not verify. A contract is verified when Locus verified the code, assumed where the code states it with `unsafe assume!`, and otherwise the conservative default that this section gives. So Locus works over a verified subset of the program: code outside it still compiles and runs, and a claim that depends on it depends on its contract, which each result lists ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code), RFC 0001's manifests). The default rests on one assumption about that code, `assumed.unsafe.sound`: it is sound Rust, so it uses a value only as the value's type allows, and only while it owns the value or holds the borrow it was given. Programs write no new syntax for this: Locus infers what it needs. A program states a contract for code that Locus does not verify by wrapping it in a function of its own, whose signature is the contract and whose body assumes what it cannot prove ([Assumptions and the standard library](#assumptions-and-the-standard-library)).

**Which unverified code is taken to be sound.** `assumed.unsafe.sound` is made crate by crate, and inside a Locus crate function by function, and only where it is needed. Safe code needs no assumption: rustc keeps it from using a value beyond what the value's type allows. So the assumption is about crates that contain `unsafe` code.
- The standard library is always taken to be sound.
- An unverified crate with no `unsafe` code, built only from such crates and the standard library, needs nothing: it is sound because rustc checked it.
- **What foreign code can call counts as `unsafe` code** (decision 95). rustc checks a function for callers that are Rust, and foreign code is not one. In edition 2021, `#[no_mangle]` and `#[export_name]` need no `unsafe`, and in every edition a function whose ABI is not Rust's needs none, and neither does a `global_asm!` that names a function. So a crate that writes no `unsafe` can have a function that takes another crate's sealed type, which C then calls with any bytes (tested with rustc 1.97, in edition 2021). An unverified crate that has a function that foreign code can call, as `logic.boundary.foreign` defines one, is therefore recorded as a crate that contains `unsafe` code, whatever the function's signature. So is one that has a static that foreign code can reach (`logic.code.statics`), or a naked function, whose body is assembly. Such a crate is taken to be sound only when the project names it, as the next two points say of every crate with `unsafe` code.
- Any other unverified crate is taken to be sound only when the project names it as trusted, in its manifest. That list is what an audit reads, and each result's manifest names the crates on it that the result relied on. How the list is written is [distribution-and-export](../../issues/distribution-and-export.md).
- A crate that contains `unsafe` code, or is built from one, and is not named is treated as foreign code is: a function that calls into it is unverified ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)). So nothing that Locus believes is given to it or taken from it, and the function that calls it is what a result lists. The crate still compiles and runs.
- In a Locus crate, each unverified function that contains `unsafe` code is taken to be sound, and listed. One with no `unsafe` code, which calls no code that is not taken to be sound, needs nothing.

The driver records, for each crate it compiles, whether the crate contains `unsafe` code after macro expansion (RFC 0001), counting what decision 95 counts. An example of this needs two crates, which the litmus format cannot yet express ([multi-file-litmus](../../issues/multi-file-litmus.md)), so it has no spec rule yet. An example could not state decision 95 in one crate either: it is about what is recorded and listed, and an example states only whether a program is accepted and what it returns.

**One rule: no exposed logical data reaches unverified code.** Earlier revisions of this RFC let logical data cross into unverified code wherever an analysis showed it harmless: by value, behind a shared reference, when its type was completable, or inside a type with an invariant. Each case needed an argument of its own, and each review found a case the arguments had missed. The owner chose a line that needs none of them (decisions 61 to 64):

> After its type parameters are instantiated, no unverified code has a value, a type argument, a capture, or a callee whose type exposes logical data.

So unverified code never writes or returns a proof that could be mistaken for another, and nothing that it does with one is believed. Decision 76 has since let it be lent exposed logical data by shared reference, which it reads and cannot change, and from which nothing logical comes back (`logic.boundary.loans`, below).

- **A type *holds logical data*** when a logical type appears in it at any depth:
  - as the type itself, a type argument, or a tuple component: `@P`, `Option<@P>`, `Vec<Nat>`, `&mut Vec<(i: usize, @(i < n))>`;
  - in a field of a struct or an enum that it names, private fields and the types of other crates included: `Interval`, through its proof field, and so `&Interval` and `Vec<Interval>`;
  - in the parameters or the result of a function, a function pointer, or a closure, and in what a closure captures: `halve`, which takes a proof, and `move || found`, for a `found` that holds one;
  - in the type that an `impl Trait` hides.

- **A *sealed type* holds logical data and exposes none** (decision 73). It is a struct or an enum of a verified crate that holds logical data only in the declared types of its own fields, with none coming in through a type argument. `Interval` is one; so are `NonEmpty`, and `Sorted<u32>` for a `struct Sorted<T>` with a proof field about its items. `Bag<@P>` is not, because its logical data comes from its type argument.
  - **A type *exposes* logical data** when it holds some that is not inside the fields of a sealed type. `@P`, `Option<@P>`, `Vec<Nat>`, and `halve` expose logical data. `Interval`, `&mut Interval`, `Vec<Interval>`, `Option<Interval>`, and a closure that takes an `Interval` expose none.
  - **Why a sealed value may be held by code that Locus does not verify.** A marker is one Rust type for every proposition, and any code that can name it can make one; that is why exposed logical data never crosses. A sealed type is the opposite on both counts. It is a type of its own to rustc, and its propositions read only its own fields, so a value of it means the same wherever it goes. And nobody who cannot reach its fields can build one, or change what its proofs read: Rust's privacy keeps other crates out, and Locus keeps the crate's own unverified functions out ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)). So unverified code can move such a value, drop it, swap it for another value of the same type, and call the type's own functions on it, which are verified. Whatever it hands back is a value that verified code built.
  - **What is believed when one comes back.** That it is a value of its type, so its proof fields hold. Nothing else: after `std::hint::black_box(&mut iv)`, `iv.lo <= iv.hi` is known and `iv.lo == 0` is not, since the callee may have swapped in another interval.
  - **Three conditions keep this from failing.** A type that other crates can reach, when those crates are not verified, has no reachable field that exposes logical data or that a proof reads; Rust makes the fields of an enum's variants public, so such an enum has no proof field at all ([Export mode](#export-mode)). The crate's own unverified functions still name no type that holds logical data, sealed or not, because they could reach its fields. And `unsafe` code that is given such a value does not make one from other memory or write inside one: for generic code that is what being sound means, since it cannot know the type, and for the rest it is the assumption `assumed.unsafe.invariants`.
- **Downward, verified code gives unverified code nothing that exposes logical data.** No argument of a call into unverified code has such a type, and neither does a value inside an argument or a type argument of the call, whether the value is given or lent by `&mut`, and nothing that exposes logical data comes back:

  ```rust prose each call is rejected
  let found: Option<@(n > 0)> = check(n);
  std::hint::black_box(&found);      // a proof holder, which `black_box` would hand back
  std::hint::black_box(&mut counts); // a `Vec<Nat>`
  std::hint::black_box(halve);       // a function that takes a proof
  ```

  Plain values cross freely, and so does a sealed value: `std::hint::black_box(iv)`, for an `Interval`, is accepted.
- **Upward, unverified code calls a verified function only through a signature that exposes no logical data.** A verified function whose instantiated signature, its receiver included, exposes none may be called from anywhere, so the methods of a sealed type that take and return plain data and values of the type are open to every caller. That is the whole mechanism for entering verified code, and it needs no mark: such a function establishes inside, by computing or by checking at run time, whatever proofs it then uses.

  ```rust
  fn halve_even(n: u32, even: @(n % 2 == 0)) -> u32 {
      n / 2
  }

  // A plain signature: anyone may call it.
  pub fn halve(n: u32) -> Option<u32> {
      if n % 2 == 0 { Some(halve_even(n, _)) } else { None }
  }
  ```

  A function whose signature exposes logical data is called only by verified code, each call checked against its contract. It is never coerced to a function pointer, never turned into a trait object, never an `extern`, `#[no_mangle]`, or `#[export_name]` function, and it is passed as a value only within verified code: never to unverified code, and never to a library operation, which could call it with arguments that its contract does not allow. A function that foreign code can call is held to more: its signature holds no logical data at all (decision 88, below).
  - Otherwise other code could call it without its proof: `proofs.into_iter().fold(10, halve)` would call `halve` with whatever `proofs` holds, which proves something else.
  - A closure counts as such a function. `|v: Option<@(1 == 2)>| slot.set(v)`, given to code that calls it with an argument of its own, would store a proof of `1 == 1` as one of `1 == 2`.
  - In verified code such a function may be bound to a local and called, as in `let f = halve; f(10, _)`, and each call is checked like any other.
  - A closure that takes or returns only plain data and sealed values may be given to unverified code: `ivs.iter().map(|iv| iv.hi - iv.lo)` is accepted, since whatever `map` calls it with is an `Interval`.
  - Every precondition is a proof parameter. There is no separate `requires` clause, so a function's signature shows everything that a caller owes it, and a signature that exposes no logical data means that nothing is owed.
- **Exposed logical data is never hidden.** A value whose type exposes logical data is not converted to a trait object: `dyn Any` and `dyn Debug` show nothing of what they hold, and the rule above could no longer see it. A sealed value may be: it is safe in any hands. And a shared reference to exposed logical data may become a `&dyn Debug` as a loan, below, since whoever holds it can only read.
- **The rule is checked for each instantiation.** A generic function's signature may show no logical data while a caller passes some through a type parameter:

  ```rust prose rejected at the instantiation
  // Unverified, and safe: Locus does not verify it, and does not see inside it.
  fn launder<T: 'static>(mut x: T) -> T {
      if let Some(o) = (&mut x as &mut dyn Any).downcast_mut::<Option<Erased>>() {
          *o = Some(Erased);
      }
      x
  }

  fn pass<T: 'static>(x: T) -> T {
      launder(x) // nothing logical is in sight here
  }

  let forged = pass::<Option<@(0 != 0)>>(None); // rejected: `T` reaches `launder`
  ```

  So Locus checks the rule on instantiated code. An unverified generic function is never instantiated with a type that exposes logical data, except to be lent a value of it, below. A verified one is, when no value whose type mentions that parameter reaches unverified code in its body or in what it calls, other than as a loan. Its interface records, for each type parameter, how one does, so that a caller in another crate is checked without the body (decision 94): never; only as loans, with the type of each value that is lent, written in the function's own type parameters; or in some other way. One bit for each parameter would not do. For another crate's `pub fn show<T: Debug>(x: &T) -> String { format!("{:?}", x) }`, `show(&found)` lends an `Option<@P>` and is accepted, and `show(&slot)` lends a `Cell<Option<@P>>`, which can be written through a shared reference, and is rejected. The record says that a `&T` is lent, and the caller checks the loan's conditions at its own `T`. If checking the instantiations would not end, as with polymorphic recursion, the program is rejected.
- **Locus's own library is the exception.** The types it provides in place of the standard library's collections, reference-counted pointers, cells, and locks keep the values they are given in the standard library's own, and that is trusted, not checked (decision 83, `trusted.library`). So `v.push(entry)` keeps the entry's proof, and `slot.set(p)` stores a proof of the cell's proposition, when `v` and `slot` are Locus's, named with `use locus::std::…` ([Assumptions and the standard library](#assumptions-and-the-standard-library)). The standard library's own `Vec`, `Cell`, and `Option::take` are unverified code, as `black_box`, `Any::downcast`, and `mem::swap` are. An `Option<@P>` is taken apart with `match`.
- **Unverified code may be lent what it cannot change** (decision 76). A shared reference to a value whose type exposes logical data may be given to unverified code, when every logical type in that type lies in the structural part of the value's snapshot ([Snapshots and models](#snapshots-and-models)): none inside an `UnsafeCell`, behind a raw pointer, or in a type whose snapshot is opaque, and none in the signature or the captures of a function or a closure.

  ```rust
  let found: Option<@(n > 0)> = check(n);
  let text = format!("{:?}", found); // accepted: "Some(Erased)" or "None"
  let hidden: &dyn Debug = &found;   // accepted: a loan may be a trait object
  ```

  - **Why it is safe.** A marker in unverified hands could do harm in two ways, and a loan allows neither.
    - *By being written where verified code will read it.* A shared reference to data that holds no cell cannot be written through. That is the guarantee that already lets a snapshot leave out only cells and the targets of raw pointers (`models.frozen-data`).
    - *By being handed back.* An unverified function's result and its other parameters still expose no logical data, a function that takes a proof is never given to it, and it can build no sealed value. So a marker that it copies out of a loan, even one that it keeps in a static behind `dyn Any`, reaches no verified code.
  - **What it allows.** The standard library's `#[derive(Debug)]` on a type with a proof field, which lends each field to the formatter and prints `Interval { lo: 1, hi: 4, ordered: Erased }`; `{:?}` on an `Option<@P>`; and any other code that reads through `&` and returns plain data.
  - **What it does not.**
    - A `Cell<Option<@P>>` is not lent: a shared reference is enough to write to it.
    - `&halve`, for a `halve` that takes a proof, is not lent: whoever holds the reference could call it, with a marker copied from another loan.
    - `std::hint::black_box(&found)` is still rejected. `black_box` returns what it is given, and a reference that comes back from unverified code could be to another value.
    - Nothing is given by value or by `&mut`.
  - A loan reaches unverified code only through a type parameter or a trait object. A function of the crate's own that names `&Option<@P>` in its signature holds logical data, and so is verified.
  - It rests on two entries that the trust model already has: `assumed.unsafe.sound`, under which no `unsafe` code writes through a shared reference to data that holds no cell, and `trusted.library-specs`, under which the snapshots that Locus's library declares for standard types hold exactly what those types own.
- **What comes back is known only by its type.** A value that unverified code returns, and a place it was lent mutably, is known only by its Rust type, which for a sealed type includes its proof fields. A fact about it comes from a check in verified code:

  ```rust
  let n = parse(s); // `parse` is unverified
  if n < 100 {
      let small: @(n < 100) = _;
      // ...
  }
  ```

  Two calls are unrelated. What verified code knows about a call's result is what the callee's contract says, a proof in its result, and nothing else: it never assumes that a function gives the same answer twice. With `let a = s.as_ref();` and `let b = s.as_ref();`, a proof of `i < a.len()` says nothing about `b`. A trait implementation that a caller supplies can answer differently each time, and code that trusted it not to has been a recurring source of memory-safety bugs in Rust's own standard library ([Prior art](#prior-art)).
- **What is still awkward.** With sealed types, a value with an invariant can be printed, stored in any container, sorted, and shown to other crates. What remains:
  - A derive of another crate that hands every field to a trait of its own, such as `serde::Serialize`, needs the logical fields skipped, since a marker implements only the standard traits. The standard library's `#[derive(Debug)]` works, as a loan, and prints a proof field as `Erased`; a `Debug` written by hand over the plain fields leaves it out.
  - Exposed logical data still stays inside verified code and Locus's library: an `Option<@P>`, a `Vec<(usize, @P)>`, or a function that takes a proof is not given to `black_box`, to another crate's container, or to the standard library's iterator adapters. Other code can be lent it by shared reference, to read.
  - A proof about a value that the type does not hold cannot be sealed. `(i: usize, @(i < xs.len()))` exposes its proof; a struct that also held `xs` would not.
  - An enum with a proof field is not shown to unverified crates.
  - What a container that Locus does not verify gives back is some value of the type, not the one that went in.

  [opaque-certified-values](../../issues/opaque-certified-values.md) records these.

**Only plain data crosses the foreign boundary** (decision 88). *Foreign code* is code that is not compiled from Rust: what a program reaches through an `extern` block, and whatever calls the program's functions from outside it. Unverified Rust in another crate may hold a sealed value, because Rust's privacy keeps it from the fields. Nothing keeps foreign code from them. It sees a value as bytes, can write any of them, and can hand over bytes that no verified code built. So it stands where an unverified function of the crate itself stands, which handles no type that holds logical data, sealed or not ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)). That rule already covered the signature of a function declared in an `extern` block. It now holds at every point where a value can cross:

| Where a value crosses | The rule |
|---|---|
| A function declared in an `extern` block | No type that holds logical data in its signature (`logic.unverified.plain-rust`). Unchanged |
| A function with a body that foreign code can call: one whose ABI is not Rust's, or that is `#[no_mangle]` or `#[export_name]`, or that a `sym` operand of `asm!`, `global_asm!`, or `naked_asm!` names (decision 96) | The same. Before decision 88 only a signature that *exposes* logical data was refused here, so a sealed type passed |
| A function pointer type whose ABI is not Rust's, such as `extern "C" fn(u32) -> u32` | The same, of its parameters and its result, wherever the type appears. New |
| A static that foreign code can reach: `extern`, `#[no_mangle]`, or `#[export_name]`, or one that a `sym` operand names (decision 96) | No type that holds logical data (`logic.code.statics`). Unchanged by decision 88 |
| A naked function, `#[unsafe(naked)]`, whose body is assembly | It is unverified, so no type that holds logical data appears in its signature (`logic.unverified.functions`, decision 96) |
| A raw pointer | Nothing new: a function that handles one is unverified, and an unverified function handles no such type |

- **What it closes.** A sealed type in the signature of a function that C can call:

  ```rust prose rejected
  struct Positive {
      value: u32,
      nonzero: @(value > 0),
  }

  #[unsafe(no_mangle)]
  pub extern "C" fn reciprocal(p: Positive) -> u32 {
      100 / p.value
  }
  ```

  C calls `reciprocal` with whatever bytes it likes, and `p.nonzero` would then be a proof of `0 > 0`. Before decision 88 this was accepted: `Positive` is sealed, so the signature exposes nothing.
- **"Holds" is at any depth,** as everywhere in this section. `&Positive`, `Box<Positive>`, `*const Positive`, `Option<Positive>`, and a struct with a `Positive` field are refused where `Positive` is.
- **The function is otherwise as any other.** Whether it is verified follows from [the usual table](#unverified-functions-and-unsafe-code): it is verified unless it contains `unsafe` code or handles a raw pointer. The `unsafe` in `#[unsafe(no_mangle)]`, which edition 2024 requires, belongs to the attribute. It is not an `unsafe` block, and does not make the function unverified. So a safe shim takes plain data, checks it with a verified constructor, and goes on with what the check established:

  ```rust
  impl Positive {
      fn new(value: u32) -> Option<Positive> {
          if value > 0 { Some(Positive { value, nonzero: _ }) } else { None }
      }
  }

  fn divide(n: u32, d: u32, nonzero: @(d > 0)) -> u32 {
      n / d
  }

  #[unsafe(no_mangle)]
  pub extern "C" fn reciprocal(value: u32) -> u32 {
      match Positive::new(value) {
          Some(p) => divide(100, p.value, p.nonzero),
          None => 0,
      }
  }
  ```

  A shim that reads through a raw pointer, or contains an `unsafe` block, is unverified as any function is, and then names no `Positive` in its body either. It passes the plain values on to a verified function, as `sub` is passed `hi` and `lo` under [Unverified functions and unsafe code](#unverified-functions-and-unsafe-code).
- **The rule is checked for each instantiation,** as the rest of the boundary is. `extern "C" fn first<T>(x: T, y: T) -> T` is refused at `T = Positive`, and so is the function pointer type that it would coerce to there.
- **A `sym` operand gives a function to assembly** (decision 96). `asm!`, `global_asm!`, and `naked_asm!` take operands of the form `sym path`, which put the symbol of a function or of a static into the assembly. The assembly can then call the function with whatever the registers hold:

  ```rust prose rejected
  fn reciprocal(p: Positive) -> u32 {
      100 / p.value
  }

  global_asm!(
      ".globl reciprocal_entry",
      "reciprocal_entry:",
      "jmp {target}",
      target = sym reciprocal,
  );
  ```

  `reciprocal` has Rust's ABI and no attribute, and `global_asm!` needs no `unsafe`, so no row of decision 88's table named it (tested with rustc 1.97, which compiles this with a marker in place of the proof).
  - A function that a `sym` operand names is one that foreign code can call, in each of the three macros. No type that holds logical data appears in its signature, and a result lists it with the other functions that foreign code can call.
  - It is otherwise as any other function, as one with `#[no_mangle]` is. A verified `reciprocal` that takes a `u32` and builds the `Positive` inside may be named.
  - A static that a `sym` operand names is one that foreign code can reach, and has no type that holds logical data (`logic.code.statics`).
  - The assembly of a `global_asm!` is foreign code, and what is trusted of it is `assumed.foreign`. An `asm!` stands in an `unsafe` block, so its function is unverified, and the assembly is that function's `unsafe` code, which `assumed.unsafe.sound` covers as before.
- **A naked function is unverified** (decision 96). The body of a function marked `#[unsafe(naked)]` is one `naked_asm!`: assembly under a Rust signature, with nothing in it that Locus could verify.

  ```rust
  #[unsafe(naked)]
  pub extern "C" fn halve(n: u32) -> u32 {
      naked_asm!("mov eax, edi", "shr eax, 1", "ret")
  }
  ```

  - It is unverified as a function with an `unsafe` block is ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)). Its signature holds no logical data, sealed or not, `#[verified]` on it is rejected, and verified code knows it by its Rust type alone.
  - Its body is `unsafe` code of its crate. So it is taken to be sound, and listed, as every unverified function with `unsafe` code is (`assumed.unsafe.sound`). The trust model gains no entry for it.
  - rustc 1.97 accepts `#[unsafe(naked)]` only on a function whose ABI is not Rust's. So today a naked function is also one that foreign code can call, and its signature was already held to this rule. What decision 96 adds is its kind.
- **What is trusted of foreign code** is a new entry of the trust model, `assumed.foreign`: it keeps to the signatures that the program declares for it. It passes valid values of the Rust types declared, and does nothing that a sound Rust function of the same signature could not. Nothing checks a declaration against the code behind it. A function that foreign code can call need have no `unsafe` code in its body to mark it, so each result lists the functions of the build that foreign code can call. It also lists the foreign functions declared `safe` that verified code calls. A call of any other foreign function is inside an unverified function with `unsafe` code, which a result lists already.
- **An object that foreign code holds for a long time.** A C interface over a Rust object usually hands out an opaque pointer to it. Under this rule the shim cannot give that pointer the type `*mut Positive`, and a shim that casts it is unverified and names no `Positive`. The pattern that needs no `unsafe` code keeps the objects in a table on the Rust side and gives foreign code an integer key, which a verified function looks up. Whether Locus's library provides such a table is for the standard library's RFC ([standard-library](../../issues/standard-library.md)).

**A call keeps only the facts it cannot change.** Effects say which facts survive a call; contracts say what becomes true.
- **A verified function's effect is inferred.** Locus summarizes, from its body and the functions it calls, which of the places it is given it may write. The summary goes into the crate's interface (RFC 0001), so its callers use it without seeing its body, and the programmer writes nothing. After `reset(&mut x)`, for `fn reset(x: &mut u32) { *x = 0; }`, a fact about another local `y` still holds, because `reset` is given only `x`.
- **Code whose effect is unknown is assumed to do anything its access allows, while it has that access.** A call of unverified code may write every place that a value it is given can reach mutably, and no other. It keeps no access once the borrow it was given ends. That is the assumption `assumed.unsafe.sound`: a library that wrote through a reference after its lifetime would let safe code cause undefined behavior, so it would not be sound Rust. Implicit calls count too: drops, overloaded operators, and callbacks.
- **What a value can reach is decided from rustc's borrow facts, not from the value's visible type.** A place is reachable when a loan of it flows into a lifetime of the value's type: mutably for a mutable loan, and, for a shared loan, only where it reaches interior mutability. So a `&mut` held by a trait object, an opaque `impl Trait`, a closure, or a container counts. With `let bump: Box<dyn FnMut() + '_> = Box::new(|| n = 7);`, the box's type shows no `&mut`, but the closure holds a mutable loan of `n`, so after `black_box(bump)` Locus knows `n` only by its type (tested: rustc accepts the program, and unverified code that calls the closure leaves `n` at 7). The borrow checker already follows loans through reborrows, coercions, captures, and moves, so this over-approximates; an effect summary narrows it where the callee is known.
- **When the borrow ends, the place has a new version.** After `dep::touch(&mut n)`, Locus knows `n` only by its type. A proof about an earlier version stays a proof about that version, and says nothing about the new one ([Versions](#versions)). A write that Locus sees establishes a fact again: after `n = 3`, `n == 3` holds, and a later call such as `dep::tick()`, which is given nothing, leaves it alone. The same holds for a value that unverified code returns: after `let mut b = dep::make();`, `b` is known only by its type, and after `*b = 1`, `*b == 1` holds across `dep::tick()`. No place loses its facts at a later call: a raw pointer exists only inside an unverified function, which keeps no access once it returns, whatever it did with the address ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)).
- **A contract narrows this.** What a verified function may write is in its interface, so calling `push` on Locus's `Vec` changes only the vector, and retains nothing. A call of the standard library's own code is a call of unknown code.
- **Threads change nothing here.** Another thread's write becomes visible only through synchronization, which is a call; without it, the access is a data race, which is undefined behavior.
- How summaries are computed, and how precise they are, is the runtime semantics ([runtime-semantics](../../issues/runtime-semantics.md)). Where a summary is unknown, the call is treated as unknown code.

**Contracts at the boundary are later work.** A wrapper around code that Locus cannot verify, such as a call into C, would get precise reasoning only through a contract that states what the code does: what it requires, which places it may write, and what it returns, as in "writes only the given buffer, and returns at most its length". Each of these would be an assumption, which a result that relies on it lists, and stating one would have to be marked as `unsafe` is, since a false one could lead verified code to undefined behavior. `unsafe assume!(_)` is that form for a fact about a result: a verified function wraps the call, states the fact as a proof in its result, and assumes it, and what the code requires is a proof parameter of the wrapper (decision 80, [Assumptions and the standard library](#assumptions-and-the-standard-library)). The unverified function itself still has no contract and is known by its Rust type alone. What remains open, such as a proof that justifies skipping a runtime check, is [boundary-contracts](../../issues/boundary-contracts.md). An `unsafe` block or an `unsafe fn` authorizes no fact.

### Logical code

Logical code is:
- the bodies of logic functions and of proposition declarations;
- the contents of `logic! { ... }`, `prop!(...)`, and `prove!(...)`;
- propositions in types, written `@(...)`, and proof holes `_`;
- calls of logic functions, and proof constructors such as `And::Intro(h, k)`;
- an operator, a method call, or a field access whose operands are all logical code or bindings shown to be ghost.

**Bindings shown to be ghost.** Lowering decides what is logical code before rustc runs, from the source alone ([Deciding what is logical](#ghost-values)). So outside delimited logical code, a binding is *shown to be ghost* only when:
- a parameter of logical type, proof parameters included;
- a binding whose type annotation holds a logical type;
- a binding initialized by logical code, such as `logic! { ... }`, `prop!(...)`, a hole, or a call of a logic function;
- a binding in a proof pattern, such as the `h` of `Even::Step(m) @ h`.

A ghost value that the source does not show, such as `let k = v[0];` for `v: Vec<Nat>`, is still a ghost value, and can be passed on or stored. But an operator or method call on it is runtime code, which rustc rejects for a marker. Write `logic! { k + 1 }`, which reads `k` as logical code. So `let both = p && q;`, with `p` and `q` from `prop!`, combines two propositions, while `let m = k + 1;` is an error that suggests `logic!`.

Logical code is pure, total, and deterministic. It never calls runtime functions or methods, never mutates runtime state, and has no effects. It may read runtime places, which RFC 0001 checks as [observations](../../spec/observation.md), and it may call logic functions.

**Reading runtime values.**
- **Place expressions keep their Rust meaning.** A place is a local or parameter, a field or tuple field visible at that point, or the target of a reference or `Box`. Indexing and overloaded `Deref` do not form places in logical code.
- **Runtime values cross into logical code only where it reads a place,** and become logical values there, so logical code computes only with logical values.
- **Reading a place gives a snapshot, and using a runtime value as a value takes it to its model.** Using it as a value covers an operand, an argument of a logical parameter, a method receiver, and either side of `===` or `==`. A value whose type has no model cannot be used that way; `snap(x)` gives its snapshot instead ([Snapshots and models](#snapshots-and-models)). Its fields can still be read, and used through their own models.
- **Logical code never reads data that changes without a write it sees.** A snapshot holds no contents of an `UnsafeCell`, on which `Cell`, `RefCell`, `Mutex`, and the atomics are built, and no target of a raw pointer, which is not a place in logical code. And a raw pointer exists only in an unverified function, which is given a place only by a borrow, after which the place has a new version ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)). So no logical value depends on a write that Locus does not see. Only the trusted snapshots and models of Locus's library, such as `Vec`'s, look through raw pointers.
- **Methods and indexing work on models.** A method call or index on a runtime value works on its model, never on a runtime method, as RFC 0001 requires. For `v: &Vec<u32>`, `v.len()` is `Seq::len` of `v`'s model, and `v[i]` indexes that model. A logic function declared on the runtime type itself is found first, then those of its model's type. On a snapshot, they work on the snapshot: `snap(v).len()` for a `Vec` of values without a model.
- **Indexing a sequence,** as in `xs[i]`, is a placeholder until the RFC for Locus's standard library ([standard-library](../../issues/standard-library.md)) decides the index type and what an index outside the sequence means. Whatever it decides, indexing a sequence whose elements might have no values, such as a `Seq<@False>`, needs a proof that the index is in bounds, or indexing an empty one would prove `False`. [RFC 0005](0005-core-terms.md#sequences) proposes the answer: `xs[i]` with no proof only where the element type has a default.
- **Constants and const generic parameters** may appear in logical code, through their models. A constant of logical type is logical code ([Ghost values](#ghost-values)).
- **Statics** may appear only when they are immutable. No `static mut`, and no static that is `extern`, `#[no_mangle]`, or `#[export_name]`, or that a `sym` operand names (decision 96), holds logical data, because code outside the program can write it.
- **References are transparent.** A `&T`, `&mut T`, or `Box<T>` used as a value is the current value of its target. For a `&mut` parameter in a signature, "current" means at entry; see [Contracts](#contracts).

**Logical expressions.**
- **Arithmetic is exact:** it never overflows.
  - `Int` has `+`, `-`, and `*`. Division and remainder truncate toward zero, as Rust's do, so a machine integer's division agrees with its model's. Dividing by zero gives zero, and the remainder of a division by zero is the dividend, as in Lean, so `a === (a / b) * b + a % b` always holds.
  - Lean's own `/` on integers is Euclidean instead: its remainder is never negative, so `-7 / 2` is `-4` there and `-3` in Rust. Locus's library provides that as `div_euclid` and `rem_euclid`, the names Rust gives it ([standard-library](../../issues/standard-library.md)).
  - A `Nat` is used as an `Int` wherever an `Int` is expected.
  - `+`, `*`, `/`, and `%` on `Nat`s give a `Nat`, and `-` gives an `Int`. So for an empty `v`, `v.len() - 1` is `-1`.
  - Turning an `Int` into a `Nat` needs a proof that it is not negative.
  - An integer literal is an `Int`, or a `Nat` where a `Nat` is expected.
  - `as` in logical code converts between logical types, such as a `Nat` to an `Int`; a cast between machine integers is runtime code.
  - A machine integer's snapshot, and so its model, lies in its type's range, which proofs may use: for `x: u8`, `x <= 255`. For `usize` and `isize`, the range is that of the configuration being checked.
- **`===` is equality.** `a === b` is the proposition that `a` and `b` are equal: Lean's `Eq`, on two values of the same logical type. `a !== b` is its negation. It is built in, every logical type has it, and it cannot be overloaded.
  - On runtime values, `===` compares their models, and nothing else. Two runtime values whose models are equal need not be equal values, and their fields need not be equal: a hand-written model may forget fields entirely. So equality of models never gives equality of snapshots, or of fields read as places.
  - On snapshots, `===` is equality of values: two snapshots are equal exactly when they are the same variant with equal fields. Their opaque parts, such as a `Cell`'s, are values that Locus knows nothing about, except that a move or a copy keeps them: after `let before = logic! { snap(c) }; let d = c;`, `snap(d) === before` holds.
  - Any two proofs of one proposition are equal, so `h === k` always holds for them.
  - A type declared with `#[derive(Logical)]` gets the facts that its constructors are injective and distinct, as Lean's inductive types do.
- **`==`, `!=`, `<`, `<=`, `>`, and `>=` are comparisons, which give a `Bool`,** as Rust's give a `bool`.
  - For now they are built in only on `Int`, `Nat`, and `Bool`, where each agrees with the proposition it suggests: `a == b` is true exactly when `a === b`, and `a < b` exactly when `a` is less than `b`.
  - On other logical types, `==` waits for programmable comparisons, which are [traits-on-logical-types](../../issues/traits-on-logical-types.md), together with the laws that tie a comparison to `===` (Lean's `LawfulBEq`) and decidable equality.
  - On runtime values they compare models, like every operator in logical code. Runtime `PartialEq` and `PartialOrd` are never called in logical code; how they relate to models is a contract on the trait's methods ([traits-on-logical-types](../../issues/traits-on-logical-types.md)).
- **A `Bool` used as a proposition means that it is true:** `b` stands for `b === true`. So `@(n % 2 == 0)` is the type of proofs that `n % 2 == 0` is true, which for `Int` is the same as `n % 2 === 0`.
- **Connectives and quantifiers:** `&&`, `||`, `!`, `==>`, `forall |x: T| P`, and `exists |x: T| P`, over logical types `T`. See [Propositions](#propositions).
- **`if` and `match` choose between logical values.** `if` accepts a `Bool`, or any proposition, because classical logic decides every proposition (Lean's `Classical.propDecidable`).
- **`let`** binds a logical value.
- **Closures**, such as `|x: Int| x + 1`, are values of logical function types.

**Logic functions** are declared as `logic fn name(parameters) -> T { body }`.
- **Every logical computation produces a logical value.** A logic function's result type is logical, and so is the value of every `logic!`, `prop!`, and `prove!`, every proposition, and every proof. So erasing logical code never removes a value that runtime code uses.
- Each parameter is logical. A parameter written with a runtime type `T`, such as `xs: &[u32]`, receives a value of that type, whose visible fields the body can read as places, and which is taken to `T`'s model where the body uses it as a value. Only declared logic functions take runtime values this way; a logical function type's parameters are logical types.
- The body is logical code. Logic functions never run, so they need not be computable.
- **They are total.** Every recursive call is on a structurally smaller part of a parameter, exposed by a `match`, or decreases a measure declared with `#[decreases(e)]`, where `e` is an `Int`. At each recursive call, the measure at the call's arguments must be nonnegative and smaller than at the parameters, which is a proof obligation. For now, no logic function calls itself through another; mutual recursion is [mutual-recursion](../../issues/mutual-recursion.md). A body that proof search completes is held to the same rule, so search never fills a hole in `f` with a call of `f` that is not smaller.
- **Their type parameters range over logical types,** unless bounded by `Model`, which lets them stand for runtime types seen through their models.
- **They can belong to the program's own types,** in inherent `impl` blocks, and are then called as methods in logical code.
- **A logic function that returns a proof is a lemma:** `logic fn add_zero(n: Int) -> @(n + 0 === n)`.

**Logic blocks.** `logic! { e }` in a runtime body evaluates logical code at that point and gives a ghost value, usually bound to a name: `let before = logic! { v.len() };`.

### Snapshots and models

Logical code sees a runtime value in two ways: as what it is, its **snapshot**, and as what it means, its **model**.

```rust
struct Stack {
    items: Vec<u32>,
}

impl Model for Stack {
    type Logic = Seq<Nat>;
    logic fn model(stack: &Stack) -> Seq<Nat> {
        stack.items
    }
}

fn top(s: &Stack, nonempty: @(s.len() > 0)) -> u32 { ... }
```

Here `s.len()` uses `s` as a value, so it is `Seq::len` of `s`'s model. Inside `model`, `stack.items` reads a field of `stack`'s snapshot, and uses it as a value, so it means the vector's model, a `Seq<Nat>`.

**Snapshots.** Every runtime type `T` has a snapshot type `Snap<T>`, a logical type that holds what a value of `T` is, field by field. Locus derives it from `T`'s definition, with no declaration:

| `T` | `Snap<T>` |
|---|---|
| a struct or enum | a logical struct or enum with the same fields and variants, each field of its field type's snapshot type, proof fields included |
| `u8` … `u128`, `usize`, and the signed integers | a bounded integer: a `Nat` or `Int` within the type's range |
| `bool`, a tuple | `Bool`, the tuple of its components' snapshots |
| `Box<T>`, `&T`, `&mut T` | `Snap<T>` itself: a `Box` and a reference are transparent, and a `&mut`'s final value is `final!` ([Contracts](#contracts)). In a field that closes a cycle of references among types, a reference holds only its address, as a raw pointer does (below) |
| `[T; N]`, `[T]`, `Vec<T>` | `Seq<Snap<T>>`. Locus's library declares it for `Vec`, whose fields are raw pointers, and leaves out its capacity |
| `UnsafeCell<T>`, and so `Cell`, `RefCell`, `Mutex`, and the atomics | an opaque type: a snapshot never holds what is inside |
| a raw pointer | its address, not its target |
| a trait object, a closure, a function pointer | opaque |
| a type parameter `T` | `Snap<T>`, abstract, and substituted at each instantiation |
| a union, `MaybeUninit<T>` included | opaque: which field it holds is not part of its value |
| a field whose declared type names an associated type of one of its type's parameters, or an opaque type | opaque, for every type argument |
| any other type | opaque, unless Locus's library declares its snapshot |

- **A snapshot holds every field, private ones included,** so that two values whose fields differ have different snapshots, apart from what the table leaves out, such as a `Vec`'s capacity and an `UnsafeCell`'s contents. Rust's privacy decides who may name a field: logical code reads `s.f` only where Rust code could read `f`. Outside the crate that defines a type with private fields, its snapshots can be held, passed on, and compared, but not looked into; what they mean is known through the type's model and its contracts.
- **Snapshots never change unseen.** A snapshot belongs to a version ([Versions](#versions)). It holds no contents of an `UnsafeCell`, which change without a write, and no target of a raw pointer, only its address. So interior mutation leaves every snapshot as it was. A write through a raw pointer happens only in an unverified function, which changes only what it is lent mutably, and what was lent has a new version when the borrow ends ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)).
- **`UnsafeCell` is special because Rust makes it so.** It is a language item, and the only way Rust lets a value change through `&`. Every other type's snapshot is built from its fields, however much `unsafe` code the type uses. A type of the program's own built on raw pointers therefore has a snapshot that holds only addresses. Giving it a meaningful one, as the library does for `Vec`, needs an assumed specification or ghost permissions, which are for later.
- **Every snapshot is finite.** `Snap<T>` is a logical type, and so an inductive type in Lean, whose values are finite. A `Box` owns what it holds, so a chain of `Box`es ends. A chain of references need not: `static A: Node = Node { next: &A }` compiles, for `struct Node { next: &'static Node }` (tested), and a transparent `next` would make `Snap<Node>` a type with no values, of which `A` would give one, proving `False`. So a snapshot is cut at the field that closes such a cycle. A field is *cut* when its type reaches the type that declares it again through a reference, following type arguments, and a cut field's snapshot follows no reference: each reference in it holds only its address.
  - **The cut belongs to the type that closes the cycle, not to a generic type it uses.** With `struct Wrap<'a, T> { inner: &'a T }` and `struct Node<'a> { w: Wrap<'a, Node<'a>> }`, a self-referential `static N` compiles too (tested). `Wrap`'s own definition reaches nothing again, so `Snap<Wrap<T>>` holds `Snap<T>` for every `T`, and a fact proven about it in generic code holds at every instantiation, `Node` included. `Node`'s field `w` is cut instead: in `Snap<Node>`, `w.inner` is an address. So a type's snapshot is the same wherever it appears, a generic type's has one shape for every type argument, and borrowed views such as `Wrap<u32>`, or a slice view, keep their contents, so that their models can read them. [RFC 0005](0005-core-terms.md#where-a-snapshot-is-cut) gives a cut field's snapshot a type of its own, `CutSnap`: a `w` whose references are addresses is not a `Snap<Wrap<Node>>`, so "the same wherever it appears" holds only outside cut fields.
  - With `struct A<'a> { next: &'a B<'a> }` and `struct B<'a> { value: u32, prev: Option<&'a A<'a>> }`, `A`'s `next` reaches `A` again through `B`'s `prev`, and `B`'s `prev` reaches `B` again through `A`'s `next`, so both are cut, in `Snap<A>`, in `Snap<B>`, and inside any other snapshot. Only the types that close a cycle lose anything, and only in the fields that close it.
  - **A cycle can close where no field shows it.** With `trait Family { type Out; }`, `struct Node<F: Family> { next: F::Out }`, and `impl Family for K { type Out = &'static Node<K>; }`, the static `A: Node<K> = Node { next: &A }` compiles too (tested). `Node`'s declaration reaches nothing again, so no field is cut, and cutting at `K` alone would give `Node<F>`'s snapshot a shape that depends on `F`. So a field whose declared type names an associated type of one of its type's parameters, or an opaque type, is opaque in the snapshot, whatever the type arguments are.
  - Logical code that reads through a cut field's references as places still observes their targets: `a.next.value` in a proposition about `a: &A` reads the place `(*a.next).value`. Only a snapshot value, such as the argument of a logic function on `Snap<A>`, stops at the cut.
- **Opaque is the default.** A snapshot is structural only where the table above or Locus's library says how, and where the type's declaration shows the result to be finite and strictly positive. Everywhere else it is opaque: logical code can hold it, pass it on, and compare it, and a move or a copy keeps it, but nothing looks into it. So no Rust type is rejected for its snapshot's sake. The snapshot that Locus's library declares for a collection must be strictly positive in its element types: a set's or a map's is the sequence of its entries, and its model may be a `Set` or a `Map`. Otherwise `struct Group { subgroups: BTreeSet<Group> }`, which rustc accepts (tested), would have no snapshot type.
- **Two snapshots are equal exactly when they are the same variant with equal fields.** Their opaque parts are values that Locus knows nothing about, except that a move or a copy keeps them. So two `Cell`s built with the same contents need not have equal snapshots.
- **`snap(x)`** is a logic function in Locus's library that gives `x`'s snapshot, of type `Snap<T>`.

**Models.** A type's model is a logic function from its snapshots to a logical type, `T::Logic`. A type has one in exactly three ways:
- **declared by hand,** with `impl Model`, as `Stack` does;
- **derived,** with `#[derive(Model)]`, from the models of its fields;
- **declared by Locus's library,** for the standard library's types.

**A type has no model by default.** Using a value of a type without one as a value, as in `p === q` or `p.len()`, is an error that suggests deriving a model or writing `snap(p)`. Its fields can still be read, and used as values through their own models. This can be relaxed later, for example by making a type's snapshot its default model.

**How logical code uses runtime values.**
- **Reading a place gives a snapshot.** `p`, `p.x`, and `*r` read places, as RFC 0001 observes.
- **Using a runtime value as a value takes it to its model:** as an operand, either side of `===`, a method receiver, or an argument of a logical parameter, other than one of snapshot type.
- **Where a snapshot is expected, it stays a snapshot:** as the argument of a parameter of type `Snap<T>`, under an annotation of that type, or through `snap(x)`. A snapshot is a logical value, so it is never taken further.
- So `x + 1`, for `x: u32`, adds `x`'s model, a `Nat`, and never overflows. `sorted(xs)`, for `xs: &[u32]`, receives `xs`'s model, a `Seq<Nat>`. And `snap(a) === snap(b)` compares two whole values.

A derived model has the same fields and variants as the type, each taken to its model:

```rust
#[derive(Model)]
struct Point {
    x: u32,
    y: u32,
}

fn same(p: &Point, q: &Point, x: @(p.x === q.x), y: @(p.y === q.y)) -> @(p === q) {
    _
}
```

- `Point::Logic` is a logical struct with fields `x: Nat` and `y: Nat`. In logical code, `model(p).x` is `model(p.x)`.
- Two values have equal derived models exactly when they are the same variant and their fields' models are equal, as for any logical struct or enum. So `same` is accepted. Without the derive, `p === q` is an error, and `snap(p) === snap(q)` follows from the fields instead.
- **Deriving needs a model for every field.** A field of type `Cell<u32>`, which has none, makes the derive an error. A bare proof field, whose type is a proof type, is left out, because it carries no information. A field that holds proofs inside a runtime type keeps its runtime structure: `indices: Vec<(i: usize, @(i < 10))>` is modeled as a `Seq` of pairs, so the model keeps the indices and the length, and an `Option<@P>` field keeps its tag.
- **For now, only types without type parameters** derive a model. A generic derive would need a `Model` bound on its `impl`, which is ruled out below.
- **A type that contains itself through `Box`,** such as `enum List { Nil, Cons(u32, Box<List>) }`, may derive its model. `List::Logic` is then a recursive logical type, strictly positive as every logical type must be.
- **A type that reaches itself through a reference, following type arguments as snapshots do, may not derive its model.** For `struct Node { next: &'static Node }`, the model of `&Node` is `Node`'s, so `Node::Logic` would contain itself with no base case and have no values, while `static A: Node = Node { next: &A }` would give it one, proving `False`. The snapshot's cut at such a reference does not help, since the derived model takes each field to its model, not to its snapshot. A hand-written model can recur on snapshots instead, which the cut keeps finite.
- **A model may forget,** and a derived model forgets whatever its fields' models forget. So equal models never give equal snapshots, or equal fields: with `Inner { x }` modeled as `0` inside a derived `Outer { inner }`, two `Outer`s have equal models whatever their `x`.

The rest:
- **A type has at most one model.** A program that wants another interpretation of a type writes a logic function for it.
- **A model is declared or derived by the crate that defines the type,** following Rust's orphan rule, or declared by Locus's library for the standard library's types. The model of a type with private fields is how its crate tells other crates what its values mean: they cannot look into its snapshots, so they reason through its model and the contracts its functions state in terms of it. This is data refinement in Hoare's sense, with proof fields as the representation invariant and the model as the abstraction function.
- **`impl Model` is a real implementation** of the library's `Model` trait, with its logic function as a stub beside it, and `#[derive(Model)]` generates one.
  - So rustc checks `T: Model` bounds, and `T: Model` holds exactly for types with a model.
  - Its coherence errors, conflicting implementations (E0119) and the orphan rule (E0117), are reported as Locus's errors about models.
  - In build mode both programs keep it. An exported program has no `Model`: implementations and `T: Model` bounds are removed.
- **A `Model` bound appears only on a function's type parameters.** On an `impl` or a trait, removing it from an exported program could change which implementation a method call selects, so the exported program would behave differently from the one checked.
  - For example, take `impl<T: Model> Foo for T` beside `impl Bar for &Opq`, each with a method `go`.
  - In the verified program, `o.go()` calls `Bar`'s.
  - In the exported program, where the bound is gone, it calls `Foo`'s.
  - So such a bound is rejected, in both modes, so that a crate that builds also exports.
- **The models of standard types** are declared by Locus's library, and each is trusted:
  - unsigned integers, `Nat`; signed integers, `Int`; `bool`, `Bool`; each the integer or boolean its snapshot holds;
  - `&T`, `&mut T`, and `Box<T>`, the model of `T`;
  - `[T; N]`, `[T]`, and `Vec<T>`, a `Seq` of `T`'s model, for a `T` with a model;
  - `Option<T>`, a `Maybe` of `T`'s model, for a `T` with a model;
  - a tuple, the tuple of its components' models, when each has one;
  - `()`, the logical `Unit`;
  - a logical type, itself, so `Vec<Nat>`'s model is a `Seq<Nat>`.

  `Cell`, `RefCell`, `Mutex`, and the atomics have none. Others, such as `String`, `char`, maps, `Rc`, and `Arc`, are for the library's RFC ([standard-library](../../issues/standard-library.md)).
- **A container works for every element type through its snapshot.** `Vec<T>` has a model only when `T` has one, but `snap(v)` is a `Seq<Snap<T>>` for every `T`. So `snap(v).len()` works for a vector of `Point`s without a model, and generic code without a `Model` bound writes `snap(final!(v))[i] === snap(v)[j]` to say what a `swap` did.
- **In generic code, a value of a type parameter `T` without a `Model` bound cannot be used as a value;** its snapshot can. Facts proven about snapshots hold at every instantiation. With `T: Model`, logical code can use `T::Logic`.
- **A model is a function of the snapshot,** so it never reads the contents of an `UnsafeCell` or the target of a raw pointer, and a call that Locus knows nothing about, taking `&x`, never changes `x`'s model, as RFC 0001 relies on.
- **A model's definition never depends on itself,** directly or through other types' models. Inside a hand-written model, and inside every logic function it calls, a value of the type is read through its fields, never taken to the model being defined, and the models that derived and hand-written models use form no cycle. So a model cannot reach itself through a helper, nor through another type's model: a hand-written model of `Wrap<'a, Node<'a>>` that takes `inner` to `Node`'s model, beside a model of `Node` that takes `w` to `Wrap`'s, is rejected. A recursive type's hand-written model recurs on snapshots instead: the model calls a logic function on `Snap<List>`, which recurs structurally.
- **`model(x)`** gives `x`'s model explicitly. **In Rust's terms, `Model` has no methods,** as `compat.names.no-methods` requires. Its function takes the value as an ordinary argument, and is called either through `model(x)` or implicitly.
- **Models are interpretations.** Declaring one proves nothing about any operation. What runtime operations do to models is the runtime semantics ([runtime-semantics](../../issues/runtime-semantics.md)), together with the contracts of Locus's library.

### Versions

A place's value changes as the program runs, and logical code reads it at particular points. Locus's logic follows each binding through **versions**, as an SSA form does:

```rust
let mut n: u8 = 3;              // version n₀, with n₀ == 3
let positive: @(n > 0) = _;     // proves n₀ > 0
n = n + 1;                      // version n₁, with n₁ == 4
let stale: @(n > 0) = positive; // rejected: `positive` proves n₀ > 0, and this needs n₁ > 0
let fresh: @(n > 0) = _;        // accepted: search proves n₁ > 0 from what it knows of n₁
let before = logic! { n };      // names the value of n₁, which later writes leave alone
```

- **Bindings.** Each local and parameter is a binding with its own identity. Shadowing makes a new binding, and never changes an old one.
- **Versions.** A binding gets a new version at every write to it or to any part of it:
  - an assignment;
  - initialization after a move;
  - the end of a mutable borrow of it, when it receives the borrow's final value;
  - a call that changes it through `&mut`.

  A write through a reference, such as `*x = 2` for `x: &mut u32`, makes a new version of what `x` refers to, so propositions about `*x` follow it too.

  A write makes a new version even when it writes the value the binding already had.
- **What a version holds.**
  - The logical value of a place is that of its binding's current version.
  - A field is a projection of it.
  - Owned heap data, such as a `Box`'s target or a `Vec`'s elements, is part of it.
  - Data behind an `UnsafeCell` is part of no version (see [Snapshots and models](#snapshots-and-models)).
- **Propositions are about versions.**
  - Logical code reads the versions current where it is evaluated, and a proposition is a term over those versions.
  - A proposition never changes: later writes make new versions, and leave the old ones as they were.
- **A proof proves exactly its proposition.**
  - Where a proposition about other versions is required, the proof does not fit, even if the formula is spelled the same; it needs a new proof.
  - Proof search can find one from what the runtime semantics says relates the versions, such as `n₁ == n₀ + 1` after `n = n + 1`, when the sum is in range.
- **Versions are logical variables, not definitions.** The facts that relate them are hypotheses that proofs may use, so a proof about one version never fits another by computation alone.
- **In a signature,** parameters are at their entry versions, and `final!(x)` is the version that `x`'s referent has when the borrow ends.
- **What the runtime semantics must give at least.** How versions are formed from a function's typed body, at the joins of branches and in loops, is the runtime semantics ([runtime-semantics](../../issues/runtime-semantics.md)). [RFC 0004](0004-runtime-code.md) is its first part: the operations that Locus understands, arithmetic, joins, and loops. Whatever the rest decides, it gives at least these facts:
  - a `let` or an assignment, directly or through a reference, gives the new version the value written;
  - a version that no write separates from another, on every path between them, is the same version; on a later iteration of a loop, a write anywhere in the loop separates;
  - inside a branch, its condition or matched pattern holds;
  - after a call returns, its postconditions hold;
  - machine arithmetic gives the result of the same arithmetic on the models, wrapped to the type, which is the true result when that is in range, and machine comparisons agree with the models' ([RFC 0004](0004-runtime-code.md));
  - `len` and indexing of arrays and slices agree with their models, as the language defines them; a vector is read through the methods of Locus's `Vec` ([Assumptions and the standard library](#assumptions-and-the-standard-library));
  - a mutable borrow's final value is what its place holds when the borrow ends.

### Propositions

`Prop` is the type of propositions. Forming one proves nothing; `2 + 2 === 5` is a proposition.

- **Writing propositions.**
  - `prop!(e)` in a runtime body forms a proposition value.
  - `@(e)` is the type of proofs of `prop!(e)`.
  - `@P` is the type of proofs of `P`, for any expression `P` of type `Prop`, such as `@sorted(xs)`.
- **Three ways to define one:**
  - a formula, built with connectives and quantifiers;
  - a *defined predicate*, which is a logic function returning `Prop`, such as `sorted` above;
  - an *inductive proposition*, declared with `prop` ([below](#inductive-propositions)).
- **Snapshots.** A proposition in a body is formed where RFC 0001 places its observation, and is about the versions it read there. A proposition in a signature is about entry values. `let before = logic! { n };` names a snapshot explicitly.

**Connectives.** `&&`, `||`, `!`, and `==>` combine propositions into propositions, and `forall` and `exists` quantify them. The operands may be formulas, values of type `Prop`, or `Bool`s, each of which stands for the proposition that it is true:

```rust
logic fn between(low: Int, x: Int, high: Int) -> Prop {
    low <= x && x < high
}

fn check(x: u8, p: Prop, hp: @p, low: @(x > 1)) -> @(p && x > 1) {
    And::Intro(hp, low)
}
```

- **On runtime `bool`s, the same operators keep their Rust meaning.** Which meaning applies follows from the operands' types, as it does for `+` on `u8` and on `Int`.
- **On logical `Bool`s, `&&`, `||`, and `!` give a `Bool`,** except in a proposition: inside `@(...)`, `prop!`, or `prove!`, or where a `Prop` is expected. There they are connectives, so a proof of `@(x > 1 && x < 9)` is a conjunction of two proofs.
- **An operator whose operands are all logical is logical code,** wherever it appears. So `let both = p && q;` in a runtime body binds a ghost `Prop`, and `if p && q { ... }` is rejected, because runtime control flow never depends on a logical value.
- **Each connective has its proofs:**

  | Proposition | Proved by | Used by |
  |---|---|---|
  | `P && Q` | `And::Intro(hp, hq)` | `match h { And::Intro(hp, hq) => ... }` |
  | `P \|\| Q` | `Or::Left @ hp` or `Or::Right @ hq` | a `match` with an arm for each |
  | `P ==> Q` | a logical function `\|hp: @P\| proof` | applying it, `h(hp)` |
  | `!P` | the same, since `!P` is `P ==> False` | applying it to a proof of `P`, giving `False` |
  | `forall \|x: T\| P` | a logical function `\|x: T\| proof` | applying it, `h(v)` |
  | `exists \|x: T\| P` | `Exists::Witness(v) @ hp` | a `match`, whose witness stays inside the proof |

- **Where they come from.** `And`, `True`, and `False` are built in, because the arms of a `prop` are written with them. `Or` and `Exists` are inductive propositions of Locus's library. Implication and `forall` are logical function types, as in Lean, and equality is built in.

### Inductive propositions

A `prop` declaration defines a proposition by the ways it can be proven, as an `enum` defines a type by its variants. Each arm is one way to prove it:

```rust
prop Reachable(from: State, to: State) {
    Same => { from === to }
    Via(middle: State) => { transition(from, middle) && Reachable(middle, to) }
}
```

- **Declaration.**
  - `prop Name(parameters) { arms }`. Each arm has a name, then optional *witnesses*, then `=>` and a logical block ending in a `Prop`: the arm's *condition*.
  - Witnesses take the shapes of enum variants' fields: none, as in `Same`; positional, as in `Via(middle: State)`; or named, as in `At { index: Nat }`.
  - A witness's type may mention the parameters and earlier witnesses. The condition may mention all of them.
- **Meaning.**
  - `Reachable(a, b)` is a `Prop`. A proof of it shows that some arm's condition holds, for some witnesses.
  - Arms are alternatives. They may overlap, or leave cases uncovered, and none takes priority. A declaration with no arms can never be proven.
  - A recursive declaration means the least proposition closed under its arms: Lean's inductive propositions, in Rust's shape. Its references to itself must be strictly positive. They may appear in conjunctions, disjunctions, quantifiers, and the conclusion of an implication, but never under a negation or in an implication's premise. Its proofs are always finite.
- **Construction.**
  - `Name::Arm(witnesses) @ proof` proves `Name(arguments)`. It takes the witnesses, and after `@` a proof of the arm's condition, with the parameters and witnesses substituted: `Reachable::Via(m) @ And::Intro(step, rest)`.
  - The `@` slot is always written, even when the condition is trivial: `Reachable::Same @ _`.
  - The path is always qualified. A bare `Same @ h` would be Rust's binding pattern, which keeps its Rust meaning.
  - A nested construction is parenthesized: `Outer::Case(x) @ (Inner::Case(y) @ h)`.
  - The parameters are inferred from the proposition expected.
- **Matching.**
  - `match h { Reachable::Same @ eq => ..., Reachable::Via(m) @ both => ... }` must cover every arm. Each arm binds its witnesses, and a proof of its condition.
  - A match on a proof produces a proof (`proofs.elimination`), so witnesses never leave the reasoning.
  - A declaration with one arm can be destructured with `let`: `let Contains::At { index } @ found = membership;`.
- **Indices.**
  - Every parameter is shared by all arms. An arm that holds only at particular values says so with an equality in its condition, as `Same` does with `from === to`. This is how Lean's indexed families are written here.
  - Matching gives the equality as a proof, which later proofs can rewrite along ([proof-construction](../../issues/proof-construction.md)).
- **No definition to unfold.** A declared proposition has no defining equation. It is proved by an arm, and used by matching. Even a declaration with one arm is not the same proposition as that arm's condition.
- **Induction.**
  - A logic function that matches a proof of a recursive declaration, and calls itself on a proof found inside an arm, is a proof by induction.
  - The call is on a structurally smaller value, so it satisfies `logic.fn.total`:

    ```rust
    prop Even(n: Nat) {
        Zero => { n == 0 }
        Step(m: Nat) => { n == m + 2 && Even(m) }
    }

    logic fn even_mod_two(n: Nat, h: @Even(n)) -> @(n % 2 == 0) {
        match h {
            Even::Zero @ zero => _,
            Even::Step(m) @ And::Intro(step, previous) => {
                let hypothesis = even_mod_two(m, previous);
                _
            }
        }
    }
    ```

- **Generic declarations.** A `prop` may have type parameters, which range over logical types, as a logic function's do.

### Proofs

A proof is a value of a proof type. Proofs are irrelevant: any two proofs of a proposition are equal. They are ghost, so they are never moved or dropped, and can be used any number of times.

| Form | Meaning |
|---|---|
| `_` | A hole: a proof of the expected proposition, found by the three steps that [RFC 0003](0003-proof-construction.md) names |
| `prove!(P)` | A hole whose proposition is written out |
| `lemma(arguments)` | A call of a logic function that returns a proof |
| `Even::Step(m) @ h` | A constructor of an inductive proposition: its witnesses, then after `@` a proof of the arm's body |
| `match h { Even::Zero @ z => ..., Even::Step(m) @ k => ... }` | Reasoning by cases on a proof |
| `And::Intro(h, k)`, `Or::Left @ h`, `Or::Right @ k`, `True::Intro` | The connectives' constructors ([Connectives](#propositions)) |
| `Exists::Witness(v) @ h` | A proof of `exists \|x: T\| P`, from a witness `v` and a proof `h` of `P` at `v` |
| `\|x: T\| proof`, `\|h: @P\| proof` | A proof of `forall \|x: T\| Q`, or of `P ==> Q`: a logical function from each `x`, or each proof of `P`, to a proof of `Q` |
| `all(v)`, `imp(h)` | Using a proof `all` of a `forall` at `v`, or a proof `imp` of an implication on a proof `h` of its premise |

The connectives' constructors and eliminators are listed under [Propositions](#propositions). Forms for using a logic function's definition and for rewriting along an equality, the procedures `compute` and `linear`, and what a hole tries, are [RFC 0003](0003-proof-construction.md)'s.

- **A match on a proof produces a proof.** A witness inside a proof, such as an existential's, can be used to build other proofs, but never becomes a logical or runtime value. This is Lean's rule for eliminating propositions; Lean's exceptions for propositions with at most one proof can come later.
- **Search proposes, and the kernel decides.**
  - Holes, `prove!`, and the procedures of [RFC 0003](0003-proof-construction.md) are untrusted search. That RFC says exactly what each may try.
  - A candidate counts only once the kernel checks it.
  - Proofs that were found are stored with the crate, so that checking never depends on finding them again ([vision](../../VISION.md), commitment 3). The format is [proof-search-and-storage](../../issues/proof-search-and-storage.md).
- **The logic is Lean's.** The kernel checks proofs in Lean 4's type theory, which has:
  - an impredicative, proof-irrelevant `Prop`;
  - inductive types and propositions;
  - propositional extensionality, quotients, and classical choice.

  Each result can report the axioms it depends on (commitment 4).
- **The kernel RFC** defines the kernel, including how it provides `Int` and its arithmetic.

### Contracts

A runtime function states its contract in its signature.

- **Preconditions are proof parameters:** `fn halve(n: u32, even: @(n % 2 == 0)) -> u32`. A caller supplies a proof of the proposition with its own arguments substituted, about their values when the call is made. In build mode the parameter stays as a marker, an exported program removes it, and Locus checks the proof at every call ([Ghost values](#ghost-values)).
- **Postconditions are proofs in the result.**
  - A function that returns only a proof is written `-> @P`.
  - A named result is written `-> (out: u32, @(out <= n))`. Its names are in scope in the propositions that follow them.
- **Values in a signature are values at entry.** In a signature's propositions, before or after the arrow, every parameter means its value when the function is called. For a `&mut` parameter `x`, that is its referent's value at entry, whether written `x` or `*x`.
- **`final!(r)` is a mutable reference's final value.** For a `&mut` parameter `r`, or a `&mut` in the result, `final!(r)` is the value its referent has when the borrow ends, that is, when neither `r` nor any reborrow of it is used again. Mutable references stored in data are for later. In the reading of [What a verified program means](#what-a-verified-program-means) it is the value the function gives back for `r`: what RustHorn and Creusot call a *prophecy*, and what Aeneas computes with a backward function.
  - **When no reborrow of a `&mut` parameter can outlive the call,** the borrow ends when the function returns, after its locals are dropped, so `final!(x)` is the value at that point. That is how a function states what it did, as `bump` does above.
    - No reborrow can outlive the call when nothing else in the signature can hold `x`'s lifetime, as rustc decides from the signature: the lifetime appears in no other parameter, bound, `where` clause, or result, and no `impl Trait` result or `async fn` captures it without naming it. In edition 2024, `-> impl FnMut()` captures every lifetime in scope, so `fn stash(x: &mut u32) -> impl FnMut()` can return a closure that writes through `x` after the call (tested). A `&'static mut` parameter does not qualify.
    - Otherwise, as in `fn stash<'a>(x: &'a mut u32, slot: &mut Option<&'a mut u32>)`, the caller may write through the reborrow later, so `final!(x)` is not known at return.
    - Because drops can write through a borrow, the postcondition is checked after the locals' drops. That needs typed bodies with drops made explicit ([runtime-semantics](../../issues/runtime-semantics.md)).
  - **When a function returns a reborrow,** `final!` relates the two ends:

    ```rust
    fn first_mut(pair: &mut (u32, u32))
        -> (r: &mut u32, @(r == pair.0 && final!(pair) === (final!(r), pair.1)))
    ```

  - **Inside a body,** logical code reads current versions: `*x` at a point in the body is its value there. When a local borrow ends, the place it borrowed from gets a new version, which holds the value the borrow ended with, and the body reads it there.
  - **`final!(r)` reads no place.** It is not an observation in RFC 0001's sense, so it never extends a borrow.
  - **`final!` appears only in the propositions of a signature's result.** Not in a parameter's type, and not in a body. A body that could name a value before the program has produced it could store it: `t.count = logic! { final!(t).count + 1 }`, for a ghost field `count`, would make the final count one more than itself, and prove `False`. A `Prop` stored the same way, `t.claim = prop!(!final!(t).claim)`, is the liar. Creusot keeps its final-value operator out of ghost code and snapshots for the same reason. A caller still receives proofs whose propositions mention final values, such as `first_mut`'s above. It uses them once the borrow has ended, and never writes `final!` itself. To keep the value a local borrow ended with, a body names it when the borrow ends: `let after = logic! { n };`. [RFC 0005](0005-core-terms.md#a-borrows-final-value) replaces this rule about spelling with one about core terms, since inference can reach a final value that no body wrote.
  - **Entry values need no notation of their own.** A signature already means entry values. A body that needs one later names a snapshot: `let start = logic! { *x };`.
  - **Soundness.** RustHornBelt proved prophecies sound for safe Rust, and Creusot is built on them. That proof needs the value a prophecy resolves to not to depend on the prophecy, which holds for runtime values and, by the rule above, for ghost ones. Using prophecies needs to know where each borrow ends, which rustc's borrow checker computes on MIR. How Locus obtains that is [runtime-semantics](../../issues/runtime-semantics.md).
- **Partial correctness.** A postcondition holds when the function returns. A function that panics or loops forever proves nothing, so `fn never() -> @False { loop {} }` is accepted. A logic function cannot be written that way, because logic functions are total. That a runtime function terminates is a separate claim ([termination-claims](../../issues/termination-claims.md)).
- **Calls.** After a call returns, the caller has the proofs in its result, about the arguments it passed: `let (out, bounded) = clamp(n);`.
- **Ghost parameters.** A runtime function may take parameters of logical types, such as `n: Nat`, to state a contract in terms of a logical value that the caller supplies. Such a function is called only by verified code ([The boundary with unverified code](#the-boundary-with-unverified-code)).
- **Trait methods with contracts** are for [traits-on-logical-types](../../issues/traits-on-logical-types.md). Until then, no trait method's signature holds logical values, as the fourth of the further rules under [Ghost values](#ghost-values) says. When they arrive, an implementation in Rust cannot be checked, so in export mode such a trait will not be reachable from other crates.

### Proofs in data

**Invariants of the program's own types.** A struct or enum variant may have proof fields, which may read the type's other fields:

```rust
struct Interval {
    lo: u32,
    hi: u32,
    ordered: @(lo <= hi),
}
```

- **A proof field *reads* the fields that its proposition mentions.** In `Interval`, `ordered` reads `lo` and `hi`. A proposition in a field's type mentions only other fields of the same struct, variant, or tuple, and constants: never a field whose type is a proof type, and never the value as a whole. It reads a field through its owned parts, a `Box`, and a shared reference, and never through a `&mut` or into anything with interior mutability, so what it reads cannot change behind the value. When it reads a field whose own type has proof fields, it reads that whole field. In type theory's terms the struct is a dependent record, in which a later field's type mentions an earlier field; "reads" means nothing more.
- **Every value of the type satisfies its proof fields at every point of the program** (decision 66). It is never in a broken state, so nothing can observe one: not a destructor, not code that catches a panic, and not another thread.
  - Each construction must prove them, including constructions in code that macros and derives generate, and a struct update `..base`, which can reuse a proof from `base` when every field that proof reads comes from `base` too.
  - A field that a proof reads never changes in place. It is not assigned, nothing inside it is assigned, and it is not initialized again after its value was moved out. So `iv.lo = 0` is rejected, though `0 <= iv.hi` holds whatever `hi` is, and so is `*p.value = 0` for a `Box` field that a proof reads.
  - No place based on a proof field, or on a field that a proof field reads, is mutably borrowed, so neither `&mut iv.lo` nor `&mut cache.hint` is allowed, nor `let Interval { hi, .. } = iv` for `iv: &mut Interval`, which binds `hi` by mutable reference because it matches through a `&mut`, nor a closure that captures such a place mutably. So `self.items.push(x)` is rejected when a proof reads `items`: it would lend the field to `push`.
  - The value changes by being replaced whole, or by being taken apart and built again. Either way every proof field is proved for the new value:

    ```rust
    fn widen(iv: &mut Interval) {
        *iv = Interval { lo: 0, hi: iv.hi, ordered: _ };
    }

    // For `struct NonEmpty { items: Vec<u32>, ok: @(items.len() > 0) }`, with Locus's `Vec`,
    // whose `push` is taken here to return a proof that the vector grew by one.
    fn push(n: NonEmpty, x: u32) -> NonEmpty {
        let NonEmpty { mut items, .. } = n;
        let grew = items.push(x);
        NonEmpty { items, ok: _ }
    }
    ```

    Rust evaluates the right-hand side of an assignment first, and stores the new value even when dropping the old one panics (tested), so only the old value and the new one can ever be seen.
  - A field that no proof reads is assigned and borrowed like any other. A proof field itself, including one of type `Option<@P>`, may be read, and may be assigned a value whose proofs prove its proposition, unless another proof field reads it.
  - Dropping a value takes it apart for the last time. Its own destructor, if it has one, is given a value that satisfies its proof fields; the destructors of its fields run afterwards, and nothing observes the value again.
  - a proof field under interior mutability reads no other field: `hint: Cell<Option<@(x != 0)>>` is rejected, and `ready: Cell<Option<@(LIMIT > 0)>>`, for a constant `LIMIT`, is allowed. A shared reference to `hint` would keep the type it was given, `&Cell<Option<@(c.x != 0)>>` for the `c.x` of that moment, and rustc lets `c.x` be assigned while it is live, because the borrow covers only `c.hint`. A proof stored for the new `x` would then be read through it as a proof about the old one (tested: the lowered program compiles and divides by zero). The same holds for a cell that a field shares through `&` or `Rc`, and for a tuple component under interior mutability, which reads no other component;
  - an unverified function never constructs the type, since it handles no type that holds logical data.
- **Why nothing changes in place, and why there is no `open!` yet.** The previous revision accepted an assignment to a field that a proof reads when the proof could be proved again, and had an `open!` block in which such fields could be borrowed and the invariant was proved again at the end, with a panic inside the block aborting the process (decision 51). The owner chose to support neither for now (decision 67): "We will not support temporarily breaking an invariant while retaining the object's certified identity."
  - A value that is valid at every point needs no rule about unwinding, about calls that come back in, or about what a destructor may see, and it changes nothing about what a program does when it panics, so the [vision](../../VISION.md)'s commitment 2 has no exception.
  - The cost falls on a method that takes `&mut self`: rustc rejects moving `items` out of `*self` (E0507), so such a method cannot take the value apart where it stands. The three ways below are what it does instead.
- **A value with an invariant is changed in one of three ways, and each replaces it whole** (decisions 84 to 87). The owner, shown these beside a fourth that assigns the proof field again in place: "I like options 1-3." For `struct Stack { items: Vec<u32>, cap: usize, fits: @(items.len() <= cap) }`, each is the middle of a `push`, after the check that there is room:

  ```rust
  // 1. By value. The method takes the stack and returns it.
  let Stack { mut items, cap, .. } = self;
  let grew = items.push(x);
  Stack { items, cap, fits: _ }

  // 2. A stand-in. A valid value takes the place of `*self` while the new one is built.
  let (taken, was) = mem::replace(self, Stack::new(0));
  let Stack { mut items, cap, .. } = taken;
  let grew = items.push(x);
  *self = Stack { items, cap, fits: _ };

  // 3. A slot. The value is moved out of `*self`, and a new one is put back.
  let (slot, taken, was) = mem::take_out(self);
  let Stack { mut items, cap, .. } = taken;
  let grew = items.push(x);
  slot.put(Stack { items, cap, fits: _ });
  ```

  - **By value** needs nothing more than decision 66 gives.
  - **A stand-in** needs no rule either. `Stack` is sealed, so the standard library's `mem::replace` may be given one. Locus's library has a `replace` of its own, whose result says that it returns what was there:

    ```rust
    // In Locus's library, as `locus::std::mem::replace`.
    pub fn replace<T>(place: &mut T, new: T)
        -> (old: T, @(snap(old) === snap(place) && snap(final!(place)) === snap(new)))
    ```

    If `push` panics, the caller is left with the stand-in: a valid stack with the wrong contents, which is what `mem::take` leaves a Rust program with. The type needs a valid value that is cheap to build.
  - **A slot** is for a type that has none, such as a buffer that is never empty.

    ```rust
    // In Locus's library, under `locus::std::mem`.
    pub fn take_out<'a, T>(place: &'a mut T) -> (slot: Slot<'a, T>, old: T, @(snap(old) === snap(place)))

    impl<'a, T> Slot<'a, T> {
        pub fn put(self, new: T)
    }
    ```

    - `take_out` moves the value out of the place, and `put` writes a value there. Between the two the place holds nothing, and nothing can reach it: the slot holds the borrow, so rustc rejects every use of `*self` until `put` has ended it.
    - **A slot stays a local.** The `let` that calls `take_out` binds it to a variable, and that variable is used in one way only, as the receiver of `put`. It is not moved, borrowed, assigned, captured, stored, or returned, and no `.await` lies between the two calls. Every path that leaves the variable's scope without a panic has passed it to `put`.
    - **A panic that would unwind past a slot aborts the process.** The slot's destructor does it, so no destructor of the caller's, no code that catches the panic, and no other thread sees the empty place. The abort is the library function's own behavior, in a call that the program wrote. Erasing proofs changes nothing about it, so the [vision](../../VISION.md)'s commitment 2 has no exception.
    - **`put` is read as a write.** After `slot.put(v)` the place holds `v`, as after `*self = v`. A contract could not say so: it would have to speak of the value the place ends with, which nothing in a body may compute from ([`contracts.final`](../../spec/contracts.md)).
    - **The rule is what makes the guard sound.** A slot that leaked, in a cycle of reference-counted pointers or in a future that is forgotten, would never run its destructor, and the place would be used after its value was moved out. Rust's scoped threads take a closure for this reason. A slot that is only ever a local, passed once to `put`, cannot leak. Unverified code cannot call `take_out`, because its result holds a proof.
    - **Nothing is in a broken state at any point.** Inside the region the function owns the parts by value, as in the first way. So no value keeps "the object's certified identity" while its invariant does not hold, which is what the owner ruled out with `open!`.
  - **What the three do not cover:**
    - **A method that returns a `&mut` into a field that a proof reads,** such as a `top_mut` that lends out the last item of `items`. It stays rejected, even when the proof reads only the field's length. Such a type offers a method that takes the new item.
    - **A type with both an invariant and a destructor.** rustc does not let a type that implements `Drop` be taken apart (E0509), so such a value changes only by being replaced with one built anew. The destructor belongs on the type of a field.
  - **An invariant that the shape of a type can hold needs no proof field.** `struct NonEmpty { first: u32, rest: Vec<u32> }` is never empty, no proof reads `rest`, and `self.rest.push(x)` is allowed as it stands.
  - **Assigning the proof field again in place is left for later.** `self.items.push(x); self.fits = _;` reads best, and is how a loop keeps a fact ([RFC 0004](0004-runtime-code.md)). It leaves the value broken between the two statements, so it needs every operation between them to be unable to panic. The owner: "Maybe someday we will do option 4 too with no panic but not needed now" ([no-panic-claims](../../issues/no-panic-claims.md)).
- **Proof fields are not borrowed mutably either.** A proof field's type reads its sibling fields, and a `&mut` to it would carry that type away from the value it is about.
- **A field whose type holds a proof,** such as `Option<@P>`, counts as a proof field.
- **A proof field's runtime structure can carry an invariant too.** With `hints: Vec<@True>` and `nonempty: @(hints.len() > 0)`, `nonempty` reads `hints`, so `hints` is both a proof field and a field that a proof reads, and both rules hold. `s.hints.clear()` borrows it mutably and is rejected, and so is `s.hints = vec![h]`: the value is replaced whole.
- **Generated code that builds the type** must still prove the proof fields. What the standard derives generate is checked like any other code: a derived `Clone` copies logical data fields too.
- **No `unsafe impl` that a derive or another macro generates** is allowed on such a type, in either mode. A derive such as `bytemuck::Zeroable` checks only what it can see, and would let safe code build a value without its proof.
- **Code that Locus does not verify may hold such a value, and can neither build one nor open it.** After erasure, a value without its invariant can be valid Rust: an `Interval` with `lo > hi`, or a `Positive` holding zero, breaks no rule of the language, and Rust's own soundness, which is about undefined behavior, does not ask `unsafe` code to keep it. So the type is sealed ([The boundary with unverified code](#the-boundary-with-unverified-code)): no code but verified code reaches its fields, and a value that passes through unverified code is still one that verified code built. For `unsafe` code this is an obligation that Locus adds to Rust's, which the [trust model](../TRUST.md) lists as `assumed.unsafe.invariants` ([Unverified functions and unsafe code](#unverified-functions-and-unsafe-code)). A hand-written `unsafe impl` makes that promise for its type.
- **In export mode, and in the first stage, other crates can neither build such a type nor reach the fields that expose logical data or that its proofs read** ([Export mode](#export-mode)).
- **Reading a proof field is logical code:** `iv.ordered` is a proof of `iv.lo <= iv.hi`.
- **Invariants are the fourth way facts enter the logic,** in RFC 0001's sense. A value's type is the evidence for its proof fields, because only verified code builds one, nothing changes one in place, and no other code reaches its fields. So this holds for a value that has passed through unverified code, and such a type can instantiate any generic parameter.
- **Logical data fields,** such as `count: Nat`, are ghost too, and stay as fields of type `LogicalData`. Assigning one is a ghost statement.

**Runtime types holding logical values.** Any runtime type may hold logical values as type arguments, as described under [Ghost values](#ghost-values):
- `Option<@P>` keeps its tag at run time. `Some(h)` needs `h: @P` to build it, and matching `Some(h)` gives back `h: @P`.
- `Vec<(i: usize, @(i < xs.len()))>` is a `Vec<(usize, Erased)>` of indices, each with a proof that it is in bounds. `Vec<Nat>` keeps its length.
- In a tuple type, a name such as `i` binds the component for the propositions after it, as in a named result. A component that a later component's proposition reads follows the rules for invariants: it is never mutably borrowed and never assigned, and the tuple is replaced whole. So `v[0].0 += 1` is rejected, and `v[0] = (4, _)` is accepted.
- **A tuple whose later component reads an earlier one is one type.** It instantiates a type parameter only as a whole, and never matches a generic `(A, B)`. The program's own `fn set_first<A, B>(pair: &mut (A, B), first: A)`, given `&mut v[0]`, would replace the index and keep its proof (tested: the lowered program compiles and indexes out of bounds). A pattern takes such a tuple apart, as `for (i, ok) in checked` does.
- **A proposition written in a type is fixed where the type is written,** like any other proposition. In `let checked: Vec<(i: usize, @(i < items.len()))>`, it is about `items` as it is at the `let`. After `items.clear()`, the stored proofs still prove that, and nothing about the new `items`.
- What a value holds is known through verified code and Locus's own library, and no other code is given one.

### Unverified functions and unsafe code

Locus does not verify `unsafe` code, and undefined behavior voids every claim, as it voids the meaning of any Rust program: rustc optimizes on the assumption that there is none, so no rule of Locus's could contain it. What Locus decides is where the line runs between the code it verifies and the code it does not, and what it takes on trust about the second.

**The line is the function.** A function is verified as a whole or not at all, and which it is follows from the function itself (decision 65):

| Does its signature hold logical data? | Is it an `unsafe fn` or a naked function, or does its body contain `unsafe` or a raw pointer? | The function is |
|---|---|---|
| Yes | No | verified |
| Yes | Yes | rejected: it must be verified, and a verified function contains neither |
| No | No | verified |
| No | Yes | unverified |

- **The signature decides first.** A function whose signature, its receiver included, holds logical data can only be verified, since no unverified function handles any ([The boundary with unverified code](#the-boundary-with-unverified-code)). So a method of a type with proof fields is verified without anyone saying so, and so is what a derive generates for such a type.
- **Otherwise `unsafe` decides.** A function is *unverified* when it is an `unsafe fn`, when it is a naked function, whose body is assembly (decision 96), or when its body, after macro expansion and including its closures, contains an `unsafe` block or an expression whose type is a raw pointer. A foreign function is unverified. Every other function is *verified*. A function nested in another is judged by itself.
  - **`unsafe` marks itself.** A Rust function with an `unsafe` block is unverified without a mark, so a Rust file is still the same program when Locus compiles it (RFC 0001). `unsafe` that the compiler itself generates for an operation it has checked, as in the expansion of `format_args!`, does not count.
  - **A verified function handles no raw pointer.** Making one is safe Rust, but a place's address is no part of its value in [the reading](#what-a-verified-program-means): it would tell apart two places that hold equal values, and whoever is handed the pointer can write the place with no write that Locus sees. So `&mut n as *mut u32`, `&raw mut n`, and a call that returns a raw pointer, such as `as_mut_ptr`, make their function unverified, as an `unsafe` block does.
- **Two marks say it explicitly.** `#[verified]` requires the function to be verified, and `#[unverified]` makes it unverified:

  ```rust prose the two marks
  #[verified]
  fn process(input: Input) -> Output { ... } // adding `unsafe` here later is an error, not a silent change

  #[unverified]
  fn adapter(input: Input) -> Output { ... } // plain Rust that Locus leaves alone
  ```

  A mark chooses the kind and never waives a rule: `#[verified]` on a function with `unsafe` code is rejected, and so is `#[unverified]` on a function whose signature or body holds logical data. The marks are attributes, so they read like `#[inline]` or `#[must_use]`, and need no new keyword. There is no default for a module or a crate yet. If one comes, an explicit mark and a signature that holds logical data will still decide before it.
- **Failing to verify is an error.** Where Locus has something to verify and cannot, the function is rejected. It never becomes unverified instead.
- **Obligations come only from logical code.** A verified function is checked where it has a proof to check, logical data to follow, or a field that an invariant reads to protect. A function with none of these has nothing to prove, and is accepted whatever Rust it uses, including Rust that Locus cannot yet reason about. So every safe Rust function is already a verified function, and a Rust crate compiles unchanged.
- **An unverified function is plain Rust.** It holds no logical code, and no type that [holds logical data](#the-boundary-with-unverified-code) appears in the type of an expression in its body or in the signature of a function it names: not `Option<@P>`, not `Interval` or `&Vec<Interval>`, whose proof field is a logical type, and not a struct with a field of type `Vec<Nat>`. So adding an `unsafe` block to a function whose body states a proof is an error, which says that the function would be unverified, and `#[unverified]` on a function whose signature holds logical data is one too. The rule is the simple one on purpose: an unverified function never touches what Locus tracks. So it cannot build a value of a type with proof fields, write a field that an invariant reads, or call a function that needs a proof. Verified code does those, and passes it the plain values it needs:

  ```rust
  // Unverified: it contains `unsafe`. Its signature is Rust's.
  fn sub(hi: u32, lo: u32) -> u32 {
      if lo <= hi { unsafe { hi.unchecked_sub(lo) } } else { 0 }
  }

  // Verified.
  fn width(iv: &Interval) -> u32 {
      sub(iv.hi, iv.lo)
  }
  ```

  - Making a value from memory of another type needs `unsafe`: `transmute`, a pointer cast, a read of a union's field, `mem::zeroed`, `MaybeUninit::assume_init`. So only an unverified function can do it, and none handles a type whose values Locus believes. `transmute::<[u32; 2], Interval>([5, 1])` is not undefined behavior, but it is rejected, because its function is unverified and names `Interval`. No foreign function's signature names such a type either, and neither does the signature of a function that foreign code can call ([The boundary with unverified code](#the-boundary-with-unverified-code), decision 88).
  - Nor is a type parameter of an unverified function instantiated with a type that exposes logical data ([The boundary with unverified code](#the-boundary-with-unverified-code)). A sealed type may instantiate one, as it may for a generic function of another crate: the function as written names no such type, so it cannot reach the fields.
- **Verified code knows an unverified function by its Rust type alone,** as it knows a function of a Rust crate ([The boundary with unverified code](#the-boundary-with-unverified-code)). The call may write what it is lent mutably, and nothing else, and its result is known only by its type. A fact about the result comes from a runtime check:

  ```rust
  let n = parse(s); // `parse` is unverified
  if n < 100 {
      let small: @(n < 100) = _;
      // ...
  }
  ```

- **An unverified function with `unsafe` code is taken to be sound, and is listed.** `assumed.unsafe.sound` says of it what it says of a trusted crate: it causes no undefined behavior for any arguments its signature admits, changes only what it is lent, and keeps no access once a borrow ends. Rust already asks this of every safe function, so it is a promise the function's author has made, not one that Locus adds. A result that passes through such a function, or through an `unsafe impl`, lists each one in its manifest by location, as it lists a trusted crate (RFC 0001). A naked function is among them: its body is assembly, which is `unsafe` code. An unverified function with no `unsafe` code, which calls no such code outside the standard library and the trusted crates, needs no assumption and is not listed: rustc checked it, and Locus checked that it handles nothing Locus believes.
- **Why the function, and not the block.** An `unsafe` block shares its stack frame with the code around it. After `let p = v.as_mut_ptr(); unsafe { *p.add(1) = 3 }`, the block never mentions `v`, yet it has changed it. Reasoning about the code around a block therefore has to follow where every address went, which earlier revisions of this RFC called escape, and it rests on details of Rust's aliasing rules that Rust has not fixed. A function has none of this. What it can reach is what its signature is given, and Rust's own convention already makes a safe function answerable to every caller. So the escape rule, the ban on making values from raw memory, and the assumption `assumed.rust.aliasing` are gone: nothing in verified code needs them.
- **An unverified function has no contract.** Nothing proves a precondition for it, and nothing is believed about its result. A program that needs more wraps the call in a verified function, whose signature states the contract and whose body assumes what it cannot prove, with `unsafe assume!(_)` ([Assumptions and the standard library](#assumptions-and-the-standard-library)). A proof still cannot justify skipping a runtime check, as a proof that an index is in bounds would for `get_unchecked`: that takes `unsafe` code in a function with a proof in its signature, which waits for permissioned pointers.
- **Locus's own library will be verified, on permissioned pointers.** This is the owner's direction for the standard library's RFC, recorded here because it decides what `unsafe` code Locus's users ever depend on. It is planned work, and no longer an option (decision 71): the memory-safety bugs that are still being found under the safe interfaces of Rust's own standard library ([Prior art](#prior-art)) are the kind that it would rule out.
  - A *permissioned pointer* is an address together with a ghost permission: a value that only one holder can have, that says which memory the holder may use and what it holds, and that is erased like any ghost value. A permission is not a third kind of ghost value (decision 69). It is an ordinary runtime struct, zero-sized, that holds logical data and is not `Copy`: rustc keeps it to one holder, logical code can read and copy what it says but not the permission itself, and only the library's own primitives construct one. Locus's library will provide them, and will build `Vec` and the collections above it in verified Locus on top, with no `unsafe` code of their own. `Box` is not among them: it stays Rust's, with its allocation and its dereference defined by the language (decision 82).
  - Rust's standard library stays available to every Locus program, as unverified code: it can be called, and nothing is proved about it.
  - Locus's collections keep the layout and the allocator of the standard library's, so that a `Vec` passes between the two in constant time and without a copy (decision 75). A slice needs no conversion at all, and slices are what most Rust interfaces take.
  - What is trusted then shrinks from the assumptions in the library's first collections (`assumed.stated`), and their handling of the values they hold (`trusted.library`), to a few pointer primitives and the argument that they are sound.
  - A [spike](../../spikes/permissioned-pointers/README.md) wrote `Box` and `Vec` this way in plain Rust: about ten primitives with `unsafe` inside, and both collections with `unsafe` forbidden, at the size and cost of the standard ones. It also found that the pointers are not all it takes. Locus needs a value that holds logical data and is not `Copy`, which decision 69 gives, contracts on the primitives, a way to leave an invariant broken for good in a destructor, and contracts on trait methods; and shared ownership and interior mutability need more than permissions for one holder. [permissioned-pointers](../../issues/permissioned-pointers.md) has the list.
  - Until those collections exist, Locus's library provides the same types as wrappers around the standard library's, whose contracts are assumed ([Assumptions and the standard library](#assumptions-and-the-standard-library)). A program does not change when a wrapper is replaced by a verified collection: the signatures stay, and the assumptions go. Which types the library provides, and which methods of each carry a contract, is for the standard library's RFC ([standard-library](../../issues/standard-library.md)).
- **A claim that a function cannot panic is planned too** (decision 71). `#[no(panic)]` on a verified function would make an obligation of every operation in it that can panic: an index is proved in range, an `unwrap` is proved to hold a value, an addition is proved not to overflow, and every function it calls carries the claim. It is an attribute for one fixed claim, not an effect system. A proved index is then a place where the bounds check can be left out, which rests on the same contracts as the pointer primitives. Its design is [no-panic-claims](../../issues/no-panic-claims.md).
- **An invariant is an obligation that Locus adds.** Rust's soundness is about undefined behavior; `lo <= hi` in an `Interval` is a functional fact, which Rust does not ask `unsafe` code to keep. Locus asks it: every value of a type with proof fields satisfies them.
  - The crate's own unverified functions meet this by handling no type that holds logical data.
  - Generic `unsafe` code meets this for free, when it is sound. Code generic over `T`, such as `Vec`'s, cannot name `T`'s fields, so it can only move, copy, and drop the values it is given, and every value it hands back is one of them.
  - A hand-written `unsafe impl` of a trait such as `bytemuck::Zeroable` records the promise for its type, and those that derives and macros generate are rejected ([Proofs in data](#proofs-in-data)).
  - Other `unsafe` code that is given such a value, or that can name its type, in dependencies and, in export mode and the first stage, in the crates that use the crate, makes the promise as an assumption, which the trust model lists as `assumed.unsafe.invariants` and each result that relies on an invariant names with the type. It is the protection that every Rust type with private fields has: breaking it takes `unsafe` code that reaches into another crate's private fields.
- **The guarantee.** For a verified function, its postconditions hold whenever it returns normally, provided that its preconditions hold; that the assumptions it relies on hold, its own and those of the code it calls, each stated with `unsafe assume!` (`assumed.stated`), and that code without a contract does no more than this RFC's conservative defaults allow; that the code it does not verify is sound Rust, using each value only as its type allows and only while it holds it (`assumed.unsafe.sound`); that `unsafe` code keeps the invariants of sealed types (`assumed.unsafe.invariants`); that foreign code keeps to the signatures declared for it (`assumed.foreign`); and that the trust model's components do their part. Each result lists which of these it relies on.

### Assumptions and the standard library

**A program assumes a fact with `unsafe assume!(_)`** (decision 80). It is a proof, taken on trust, of the proposition that its position requires, as `_` is a proof that Locus finds. `unsafe assume!(P)` names the proposition.

```rust
// `parse` is another crate's. Its documentation promises a result below 100.
fn small(s: &str) -> (n: u32, @(n < 100)) {
    let n = dep::parse(s);
    (n, unsafe assume!(_))
}
```

- **A contract is a signature, and an assumption is a proof.** `small` states its contract as every function does, with a proof in its result, and its caller binds that proof like any other: `let (n, below) = small(s);`. Nothing is attached to a function from outside its signature, and no fact is merely in scope after a call. The owner: "prefer these to become native proofs in returns (like the way we use extern in rust - we just assume it behaves in a certain way)".
- **The function stays verified.** `unsafe assume!` is not an `unsafe` block, and performs no operation. Only its one proposition is trusted: every other proof in the function is checked, and so are its calls and the rule at the boundary.
- **It allows nothing that a proof would not.** A verified function still contains no `unsafe` code, so an assumed bound does not let an index go unchecked, and no logical data reaches unverified code because of an assumption.
- **It is unsafe in Rust's sense.** A false assumption makes every result that relies on it false, and once proofs justify `unsafe` operations it could cause undefined behavior. So it carries the word that a reader of Rust already searches for. Rust's `core::hint::assert_unchecked` is an `unsafe fn` for the same reason.
- **A lint forbids assumptions** (decision 93). `unsafe assume!` lowers to a macro of Locus's library, so the program that rustc compiles holds no `unsafe` block for it: `#![forbid(unsafe_code)]` does not see it, and neither does a tool that counts `unsafe`. So Locus has a lint of its own, `locus::assumptions`, set as Rust's lints are and allowed by default. With `#![forbid(locus::assumptions)]` at the root of a crate, an `unsafe assume!` anywhere in the crate is an error. The lint is about the code it is set on: what the crate's dependencies assume is in the list below.
- **Every result lists the assumptions it relies on,** in whichever crate they are written, each with its proposition and its place (`assumed.stated`). What it relies on depends on what the result is about (decision 90):
  - **A function's result is a theorem about valid arguments.** It takes each argument, and each value the function reads from a static, to be a valid value of its type, proof fields included. It lists the assumptions written in the function's own body and in the functions it calls.
  - **So it does not list an assumption made where a value was built.**

    ```rust
    struct Positive {
        n: u32,
        ok: @(n > 0),
    }

    fn trusted(n: u32) -> Positive {
        Positive { n, ok: unsafe assume!(_) }
    }

    fn recip(p: &Positive) -> u32 {
        divide(100, p.n, p.ok)
    }
    ```

    `trusted`'s result lists its assumption. `recip`'s lists none: what is proved of `recip` is that it divides by no zero for every valid `Positive`, and whether each `Positive` that reaches it is valid is the claim of the code that built it.
  - **The program's result lists every assumption in every crate the program links.** This is the list that is promised complete: a false assumption in it can make any result of the program false, and nothing outside it can.
  - Tracing which assumptions one stored value rests on, so that a result about one call of `recip` could name `trusted`'s, is a later refinement.
- **A precondition needs nothing new.** It is a proof parameter of the function that wraps the call.
- **An unverified function assumes nothing.** It holds no logical code.

**The standard library is reached in four ways** (decisions 81 and 82), according to what a type is:

| What | How Locus reaches it |
|---|---|
| Integers, `bool`, `char`, references, tuples, arrays, slices, `Box`, `Option`, `Result`, and ranges | They stay Rust's. The language defines what their built-in operations mean: operators and casts, the length and indexing of arrays and slices, building and matching an enum, `?` on a value that holds no proof, and `Box`'s allocation and dereference. Locus's library gives each a model |
| `Vec`, `String`, the maps and sets, `VecDeque`, `Rc`, `Arc`, the cells, the locks, and the atomics | Locus's library provides a type of its own around the standard library's, with contracts in the signatures of its methods |
| A method of the standard library on a type of the first row, when a program wants a fact from it: `Option::map`, `mem::swap` | A function of Locus's library, under a name of its own, since a method of a type that stays Rust's cannot be replaced. Most are a `match` or arithmetic, and so are verified, not assumed. `checked_add`, `checked_sub`, and `checked_mul` on the integer types are not in this row: the language understands those three as they are written ([RFC 0004](0004-runtime-code.md)) |
| Everything else: I/O, time, threads, formatting, and other crates | Called as plain Rust. Its results are known by their types |

- **A type is swapped only when it has private state,** so that its methods are the only way to learn about a value. An `Option` is taken apart with `match`; a `Vec` is not.
- **Locus's own type wraps the standard library's.**

  ```rust
  // In Locus's library.
  #[repr(transparent)]
  pub struct Vec<T>(std::vec::Vec<T>);

  impl<T> Vec<T> {
      /// The vector as a slice, with a proof that the two hold the same items.
      pub fn view(&self) -> (items: &[T], @(items === self)) {
          let items = self.0.as_slice();
          (items, unsafe assume!(_))
      }

      /// A precondition is a proof parameter. Nothing is assumed here.
      pub fn get(&self, i: usize, in_bounds: @(i < self.len())) -> &T {
          &self.0[i]
      }
  }
  ```

  - It dereferences to the standard library's type, so a method of that type that takes `&self` or `&mut self` can be called without being written again, with the standard library's signature and no contract. A method that the wrapper defines takes the place of the one it is named after.
  - **A forwarded method is the standard library's own** (decision 89). A call of it is a call of unverified code at the element type, so where that type exposes logical data the call is rejected, as every call of the standard library's `Vec` is. On a `Vec<Option<@P>>`, `v.pop()` is Locus's method and is accepted, and `v.swap_remove(0)` is the standard library's and is rejected. On a `Vec<u32>` both are accepted. The standard library's type at such an element type may still be named and borrowed, as `&*v` does: what is judged is each function that is given it.
  - **So each of Locus's types lists the methods that are its own.** These are the ones that this RFC's examples call, and the standard library's RFC completes the list:

    | Type | Its own |
    |---|---|
    | `Vec<T>` | `new`, `empty`, `len`, `push`, `pop`, `get`, `set`, `view`, `clear`, `iter`, and `iter_mut`; indexing by a `usize`; iteration by value and by reference; and the macro `vec!` |
    | `Cell<T>` | `new`, `get`, and `set` |
    | `Rc<T>` | `new`, and `clone` |
    | `Mutex<T>` | `new`, and `lock`, whose guard is a type of Locus's own |

    A method in the list keeps what it is given inside the library's own type, which is trusted (decision 83). A constructor, a method that takes `self` by value, and a trait's implementation are never forwarded, so each is in the list or is not there at all.
  - It holds the standard library's own type, so it has that type's layout and allocator, and converts to and from it in constant time, by value and by reference (decision 75).
  - Its contracts are assumed for now. When the type is rebuilt on permissioned pointers, the signatures stay and the assumptions go, and no program that uses it changes.
  - **Three signatures of Locus's `Vec` are proposed here,** because the examples of values with invariants need them (decision 87). A method that returns `()` in the standard library returns its proof in its place, so no call of it changes. A method that returns a value keeps the standard library's shape and gives no fact, since a program ported from Rust calls it for that value: `new`, `len`, and `pop` are such, and the length as a fact comes from `view`. A fact that such a method cannot give comes from a method under a name of its own.

    ```rust
    impl<T> Vec<T> {
        /// An empty vector, with a proof that it is empty. `Vec::new()` gives the vector alone.
        pub fn empty() -> (v: Vec<T>, @(v.len() == 0))

        /// The standard library's `push`, with a proof that the vector grew by one.
        pub fn push(&mut self, x: T) -> @(final!(self).len() == self.len() + 1)

        /// Writes one item. The standard library has only `v[i] = x`, a trait's method, which can return no proof.
        pub fn set(&mut self, i: usize, x: T) -> @(i < self.len() && final!(self).len() == self.len())
    }
    ```

    What each says about the items themselves, that the last one is `x` and the others are as they were, needs the operations on sequences that the standard library's RFC decides.
  - **`locus::std::mem` holds `replace`, `take_out`, and `Slot`** ([Proofs in data](#proofs-in-data)). The first is the standard library's function with a contract. The other two are Locus's own.
- **A program names Locus's types with an explicit import** (decision 81): `use locus::std::prelude::*;`, or one type at a time, as in `use locus::std::cell::Cell;`. `locus::std` mirrors the standard library's modules. Without the import, `Vec` is the standard library's, as in any Rust program. No rule of Locus's decides which is meant: an import shadows the prelude, by Rust's own rule. That rule treats a macro differently from a type. A glob import shadows the standard prelude's `Vec`, and not its `vec!`: a `vec!` that only a glob brings is ambiguous with the prelude's (rustc's E0659), and when the glob brings none, `vec!` is the standard library's and builds the standard library's `Vec`. So a program that wants Locus's `vec!` imports it by name beside the glob, with `use locus::std::vec;`. A list with `self`, as in `use locus::std::vec::{self, Vec};`, imports the module and the type and not the macro (tested with rustc 1.97.0). The owner: "I'd prefer to use explicit `use locus::prelude::...` infact, maybe something even more explicitly suggesting that we are shadowing rust stdlib".
- **The standard library's own collections are unverified code like any other.** `Vec::<Nat>::new()` is rejected. A `Vec<u32>` that another crate returns is some vector, and a call of its `len` tells Locus nothing about it. Converting it to Locus's `Vec` costs nothing, and from then on the contracts of that type's methods apply.
- **Until its collections are verified, Locus's library is trusted to keep what they are given** (decision 83). A wrapper's `push` hands its argument to the standard library's `push`. When the element type holds a proof, that is exposed logical data in unverified code, which no verified function may do. `trusted.library` covers it for the library's own types, and for nothing else, so `Vec<(i: usize, @(i < n))>` works from the start.

What this costs:

- **An operator or a trait method cannot return a proof,** because the standard library's trait fixes its signature. On Locus's own types, `v[i]`, `*rc`, `for x in &v`, and `v.clone()` run and tell Locus nothing; a named method, such as `v.get(i, p)` or `v.view()`, gives the fact. The types of the table's first row are not affected. Contracts on trait methods would lift this ([traits-on-logical-types](../../issues/traits-on-logical-types.md)).
- **A program has two `Vec` types.** Rust crates take and return the standard library's, so a conversion is written at those edges, and a trait of another crate that is implemented for the standard library's `Vec` is not implemented for Locus's.
- **The table's third row is where porting costs most.** `found.map(f)` is how Rust writes it, and a fact about the result comes only from Locus's function. `x.checked_add(y)`, `checked_sub`, and `checked_mul` cost nothing: the language understands those three, and gives the fact from the call as it is written ([RFC 0004](0004-runtime-code.md)).
- **What an exported crate does with Locus's own types is open** ([distribution-and-export](../../issues/distribution-and-export.md)). This RFC's export examples use none.

This replaces the specifications of standard functions that Locus's library had been trusted for, and the exception that let a specified operation take logical data (decisions 34, 56, 78, and 79).

### Export mode

In build mode, every crate that depends on a Locus crate is verified, so a crate's whole interface, contracts included, is available to the crates that use it. In export mode, Rust users build the crate, and Locus never sees their code ([RFC 0001](0001-compiler-pipeline.md#sources-and-modes)). So what other crates can reach must be plain Rust, and must not let them break what was proven. This restricts only the exported interface: inside the crate, everything above applies unchanged.

What other crates can reach is every item and field that the source declares plain `pub` and that other crates can reach from the crate's root, together with the public fields and methods of the types among them. Items and fields declared `pub(locus)` are not part of it ([RFC 0001](0001-compiler-pipeline.md#sources-and-modes)), and neither is a `pub` item that nothing outside the crate can reach. A plain `pub` item that breaks the rules below is an error, which suggests `pub(locus)`.

- **No contracts.** A function or method that other crates can reach has no proof parameters and no proofs in its result. A Rust caller cannot supply a proof for a precondition, and could not use one from a postcondition.
- **No exposed logical data.** No parameter, result, constant, static, or type alias that other crates can reach has a type that [exposes logical data](#the-boundary-with-unverified-code), such as `Nat`, `Option<@P>`, or `Vec<(usize, @P)>`. A sealed type may appear: `pub fn new(lo: u32, hi: u32) -> Option<Interval>` is exported as it is. Nor does such a function have a `Model` bound: the exported program removes it, so a Rust caller could use a type at which the function was never checked.
- **No public logical fields.** No field that other crates can reach exposes logical data: not a proof field, and not a field of a type such as `Nat` or `Option<@P>`. Such a field can be `pub(locus)`. Rust makes every field of an enum's variants public, so an enum that other crates can reach has no field that exposes logical data.
- **A field of a sealed type may be public** (decision 92). The rule says "exposes" throughout, so `pub struct Pair { pub first: Interval, pub second: Interval }` and `pub enum Shape { Span(Interval), Dot }` are exported as they are, as `Option<Interval>` and `(Interval, Interval)` already were. A Rust user builds a `Pair` from two intervals that the crate's own functions returned, and has no other value to put there.
- **Invariants stay closed.** Other crates can neither construct a struct that has a field which exposes logical data, nor reach a field that a proof field reads (decision 73).
  - Such a struct always has a field that other crates cannot reach, since a field that exposes logical data cannot be plain `pub`, so other crates cannot construct it.
  - Its other fields may be public, a field of a sealed type among them, so long as no proof field reads them.
  - A struct or an enum that holds logical data only inside fields of sealed types proves nothing of its own. Other crates may build one, and assign those fields, with values of those types.
  - So an exported crate can offer Rust users a type whose invariant is proved: they hold its values, pass them back, and call its methods, and cannot make a value that breaks it.
- **Ghost code only where erasure can remove it.** Every invocation of `logic!`, `prop!`, or `prove!`, and every call that passes a ghost argument, is written directly in the crate's own code: not in any macro's output, and not in the input of a function-like macro such as `vec!`. The exported program cannot depend on Locus's library, so erasure removes each of them as text, and text inside a function-like macro's input means what that macro makes of it (RFC 0001). An attribute macro or a derive may take an item that contains them, such as `#[test] fn t() { let h = prove!(4 % 2 == 0); ... }`: erasure edits the item before the macro sees it, and validation compares what the macro makes of it.
- **A function used as a value keeps its ghost parameters.** An exported program removes a function's ghost parameters, unless the function is used as a value, as `grow` is in `let f = grow;`, for `fn grow(n: u32, by: Nat) -> u32`. There they stay, as markers, with the markers as the arguments of its direct calls, so that the exported program compiles wherever the built one does. So does a ghost result: after `let f = witness;`, for `fn witness(n: u32) -> @(1 + 1 === 2)`, the function still returns the marker in the exported program, where its result would otherwise become `()`. Such a value stays within verified code ([The boundary with unverified code](#the-boundary-with-unverified-code)), and such a function is never plain `pub`, since its signature holds logical data.
- **What Rust still needs stays.** A result that is entirely logical becomes `()` in an exported program, unless it is a tuple, which keeps its shape with markers for its components, as everywhere else. A ghost local that runtime code names inside a function-like macro's input, as in `one![h]` for a macro of the crate's own, stays, bound to its marker, because erasure cannot edit what that macro makes of its input. Without these, the exported program would fail to compile, not silently do something else; validation and the export check would catch it, but the rules keep a crate that builds also exporting.
- **Wrappers.** What cannot be exported as it is stays `pub(locus)`, and gets a plain `pub` wrapper, which is checked like any other code:

  ```rust
  pub(locus) fn halve_even(n: u32, even: @(n % 2 == 0)) -> u32 {
      n / 2
  }

  pub fn halve(n: u32) -> Option<u32> {
      if n % 2 == 0 { Some(halve_even(n, _)) } else { None }
  }

  pub(locus) fn clamp_proven(n: u32) -> (out: u32, @(out <= 100)) {
      if n > 100 { (100, _) } else { (n, _) }
  }

  pub fn clamp(n: u32) -> u32 {
      let (out, _bounded) = clamp_proven(n);
      out
  }
  ```

  `halve` checks the precondition at run time, and `clamp` drops the postcondition. Rust users see two plain functions. Locus users, building in build mode, see all four.
- **In every declared configuration.** These rules hold in each configuration that the crate is checked in.
- **The first stage follows these rules too.** Until the crates that depend on a Locus crate are verified, what other crates can reach from any Locus crate follows this section, and `pub(locus)` means `pub(crate)` (RFC 0001). The rule behind both is one (decisions 68 and 73): an item that exposes logical data, a function by its signature and a static or a constant by its type, is visible only to crates that Locus verifies, and so is a field that exposes logical data or that a proof reads.

### Erasure, in summary

RFC 0001 defines erasure. For this RFC's constructs:

In the table, the marker is `Erased` for a proof and `LogicalData` for other logical data.

| In the source | In build mode, both programs | In an exported program |
|---|---|---|
| A parameter or local of logical type, proof parameters included, and every argument for one | Kept, with the marker's type | Removed, except the parameters of a function used as a value, which stay with the marker's type, as their arguments stay as the marker |
| A field of logical type | A field of the marker's type | The same |
| A result of logical type | The marker | `()`; a tuple keeps its shape, with markers; and the marker itself for a function used as a value |
| A logical type argument of a runtime type, a logical tuple component, and a ghost value where one is needed | The marker | The same |
| A call of a runtime function whose result is entirely ghost | Kept as written; it returns the marker | The call where it was: as a statement, or `match call { _ => Erased }` where a ghost value is needed |
| A named result `(out: T, @P)` | `(T, Erased)` | The same |
| Logic functions, proposition declarations, `#[derive(Logical)]` types, the model types that `#[derive(Model)]` generates, and `impl Model` | Stubs that resolve names, in both | Nothing |
| `logic!`, `prop!`, `prove!`, `_`, `final!` | A macro of Locus's library: observations and the marker in the checking program, the marker alone in the runtime program | Removed with the ghost value they produce, or the marker where one is needed |
| `pub(locus)` | `pub` | `pub(crate)` |

## Rules

The rules below go into five new chapters of the specification: [The logical layer](../../spec/logical-layer.md), [Models](../../spec/models.md), [Propositions and proofs](../../spec/proofs.md), [Contracts](../../spec/contracts.md), and [Exporting to Rust](../../spec/export.md).

| Rule | Examples |
|---|---|
| `logic.types.contents-logical` | `logical-layer/logical-fields.accept.lc`, `logical-layer/logical-type-with-runtime-field.reject.lc`, `logical-layer/sequence-of-runtime-type.reject.lc` |
| `logic.types.tuples` | `logical-layer/logical-tuple-keeps-shape.accept.lc`, `logical-layer/unit-is-runtime.accept.lc`, `logical-layer/logic-fn-unit-result.reject.lc` |
| `logic.types.positive` | `logical-layer/logical-fields.accept.lc`, `models/derive-model-for-recursive-type.accept.lc`, `proofs/positive-proposition.accept.lc`, `logical-layer/negative-logical-type.reject.lc`, `logical-layer/negative-through-derived-model.reject.lc`, `logical-layer/negative-through-forall.reject.lc`, `proofs/negative-proposition.reject.lc` |
| `logic.arith.nat-subtraction` | `logical-layer/length-minus-one.accept.lc`, `logical-layer/nat-difference-is-int.accept.lc`, `logical-layer/nat-difference-as-nat.reject.lc` |
| `logic.ghost.erased` | `logical-layer/ghost-parameters.accept.lc`, `logical-layer/logical-tuple-keeps-shape.accept.lc`, `logical-layer/logical-values-in-vec.accept.lc`, `logical-layer/runtime-call-bound-first.accept.lc` |
| `logic.ghost.marker-traits` | `logical-layer/derive-eq-with-proof-field.accept.lc`, `contracts/invariant-default-derive.reject.lc`, `logical-layer/compare-logical-vectors.reject.lc`, `logical-layer/derive-eq-with-logical-field.reject.lc` |
| `logic.ghost.types-forward` | `logical-layer/logical-values-in-vec.accept.lc`, `logical-layer/types-not-written.reject.lc` |
| `logic.ghost.type-identity` | `core/conversion-by-evaluation.accept.lc`, `logical-layer/proof-holder-same-proposition.accept.lc`, `core/conversion-is-not-arithmetic.reject.lc`, `core/value-name-per-version.reject.lc`, `logical-layer/proof-holder-other-proposition.reject.lc`, `runtime/proof-holder-keeps-its-proposition.reject.lc` |
| `logic.ghost.dependent-tuples` | `logical-layer/dependent-tuple-whole-through-generic.accept.lc`, `logical-layer/dependent-tuple-split-by-generic.reject.lc` |
| `logic.ghost.interior-mutability` | `contracts/proof-in-cell-closed.accept.lc`, `logical-layer/cell-of-invariant-type.accept.lc`, `logical-layer/cell-of-proof.accept.lc`, `contracts/proof-in-cell-constructed.reject.lc`, `contracts/proof-in-cell-read-field-kept.reject.lc`, `logical-layer/cell-in-tuple-reads-component.reject.lc`, `logical-layer/lend-proof-cell-to-unverified.reject.lc`, `logical-layer/rc-cell-to-unverified.reject.lc` |
| `logic.ghost.runtime-parts-kept` | `contracts/checked-indices.accept.lc`, `contracts/proof-in-option.accept.lc`, `logical-layer/logical-values-in-vec.accept.lc`, `contracts/some-without-proof.reject.lc` |
| `logic.ghost.runtime-code-kept` | `logical-layer/runtime-call-in-proof-field.accept.lc`, `logical-layer/runtime-check-kept.accept.lc` |
| `logic.ghost.generic-instantiations` | `logical-layer/dependent-tuple-whole-through-generic.accept.lc`, `logical-layer/own-generic-lends-proof-holder.accept.lc`, `logical-layer/proof-through-own-generic.accept.lc`, `models/generic-snapshot-at-cycle.accept.lc`, `logical-layer/generic-parameter-reaches-unverified.reject.lc`, `logical-layer/own-generic-lends-proof-cell.reject.lc`, `logical-layer/own-generic-with-any.reject.lc` |
| `logic.ghost.no-runtime-control` | `contracts/proof-in-option.accept.lc`, `logical-layer/branch-on-logical.reject.lc` |
| `logic.ghost.positions-logical` | `logical-layer/logical-field-from-logic-block.accept.lc`, `logical-layer/runtime-call-bound-first.accept.lc`, `logical-layer/runtime-call-in-proof-field.accept.lc`, `logical-layer/runtime-value-through-logic-block.accept.lc`, `logical-layer/runtime-call-in-proof-argument.reject.lc`, `logical-layer/runtime-value-to-logical-field.reject.lc`, `logical-layer/runtime-value-to-logical-parameter.reject.lc` |
| `logic.ghost.shown-by-source` | `logical-layer/operator-in-logic-block.accept.lc`, `logical-layer/operator-on-unshown-ghost.reject.lc` |
| `logic.ghost.no-trait-impls` | `logical-layer/trait-impl-on-invariant-type.accept.lc`, `logical-layer/proof-in-trait-impl.reject.lc` |
| `logic.ghost.no-unions` | `logical-layer/union-of-runtime-fields.accept.lc`, `logical-layer/union-with-completable-field.reject.lc`, `logical-layer/union-with-invariant-type.reject.lc`, `logical-layer/union-with-invariant-vector.reject.lc`, `logical-layer/union-with-proof-field.reject.lc` |
| `logic.boundary.unverified` | `logical-layer/proof-holder-taken-apart.accept.lc`, `logical-layer/lend-proof-holder-to-specified.reject.lc`, `logical-layer/lend-proof-holder-to-unverified.reject.lc`, `logical-layer/logical-values-in-std-vec.reject.lc` |
| `logic.boundary.logical-data` | `logical-layer/closure-over-sealed-value-to-unverified.accept.lc`, `logical-layer/debug-written-by-hand.accept.lc`, `logical-layer/derived-debug-on-type-with-proof-field.accept.lc`, `logical-layer/own-type-with-invariant-to-unverified.accept.lc`, `logical-layer/proof-holder-as-shared-trait-object.accept.lc`, `logical-layer/vec-of-invariant-type-to-unverified.accept.lc`, `logical-layer/closure-returning-proof-access.reject.lc`, `logical-layer/completable-function-to-unverified.reject.lc`, `logical-layer/own-generic-type-to-unverified.reject.lc`, `logical-layer/proof-access-as-trait-object.reject.lc`, `logical-layer/proof-cell-as-any.reject.lc`, `logical-layer/proof-through-any.reject.lc`, `logical-layer/shared-iterator-to-unverified.reject.lc` |
| `logic.boundary.plain-data` | `logical-layer/cell-of-invariant-type.accept.lc`, `logical-layer/debug-written-by-hand.accept.lc`, `logical-layer/forwarded-method-at-plain-data.accept.lc`, `logical-layer/own-method-at-proof-holder.accept.lc`, `logical-layer/own-type-with-invariant-to-unverified.accept.lc`, `logical-layer/proof-holder-taken-apart.accept.lc`, `logical-layer/proof-through-own-generic.accept.lc`, `logical-layer/unverified-function-given-fields.accept.lc`, `logical-layer/vec-of-invariant-type-to-unverified.accept.lc`, `logical-layer/verified-entered-through-plain-signature.accept.lc`, `contracts/slot-taken-by-unverified.reject.lc`, `logical-layer/closure-returning-proof-access.reject.lc`, `logical-layer/completable-holder-to-unverified.reject.lc`, `logical-layer/forwarded-method-at-proof-holder.reject.lc`, `logical-layer/generic-parameter-reaches-unverified.reject.lc`, `logical-layer/lend-proof-cell-to-unverified.reject.lc`, `logical-layer/lend-proof-holder-to-specified.reject.lc`, `logical-layer/lend-proof-holder-to-unverified.reject.lc`, `logical-layer/logical-values-in-std-vec.reject.lc`, `logical-layer/mut-reference-to-shared-proof-holder.reject.lc`, `logical-layer/mutable-iterator-to-unverified.reject.lc`, `logical-layer/mutex-guard-to-unverified.reject.lc`, `logical-layer/option-through-black-box.reject.lc`, `logical-layer/own-generic-type-to-unverified.reject.lc`, `logical-layer/own-generic-with-any.reject.lc`, `logical-layer/proof-free-logical-to-unverified.reject.lc`, `logical-layer/proof-only-logical-to-unverified.reject.lc`, `logical-layer/proof-sender-to-unverified.reject.lc`, `logical-layer/proof-through-any.reject.lc`, `logical-layer/proof-through-black-box.reject.lc`, `logical-layer/rc-cell-to-unverified.reject.lc`, `logical-layer/share-proof-holder-with-unverified.reject.lc`, `logical-layer/shared-iterator-to-unverified.reject.lc` |
| `logic.boundary.loans` | `logical-layer/derived-debug-on-type-with-proof-field.accept.lc`, `logical-layer/own-generic-lends-proof-holder.accept.lc`, `logical-layer/proof-holder-as-shared-trait-object.accept.lc`, `logical-layer/proof-holder-formatted.accept.lc`, `logical-layer/own-generic-lends-proof-cell.reject.lc`, `logical-layer/proof-cell-formatted.reject.lc`, `logical-layer/proof-function-lent-by-reference.reject.lc`, `logical-layer/share-proof-holder-with-unverified.reject.lc` |
| `logic.boundary.calls` | `logical-layer/closure-needing-proof-called.accept.lc`, `logical-layer/closure-over-sealed-value-to-unverified.accept.lc`, `logical-layer/ghost-function-as-value.accept.lc`, `logical-layer/proof-payload-function-as-value.accept.lc`, `logical-layer/closure-needing-proof-to-unverified.reject.lc`, `logical-layer/completable-function-to-unverified.reject.lc`, `logical-layer/proof-function-as-pointer.reject.lc`, `logical-layer/proof-function-as-trait-object.reject.lc`, `logical-layer/proof-function-lent-by-reference.reject.lc`, `logical-layer/proof-function-no-mangle.reject.lc`, `logical-layer/proof-function-to-specified-generic.reject.lc`, `logical-layer/proof-function-to-unverified.reject.lc` |
| `logic.boundary.foreign` | `logical-layer/foreign-abi-pointer-over-plain-data.accept.lc`, `logical-layer/foreign-result-checked-by-constructor.accept.lc`, `logical-layer/foreign-shim-checks-its-input.accept.lc`, `logical-layer/sym-function-over-plain-data.accept.lc`, `logical-layer/asm-sym-function-takes-sealed-type.reject.lc`, `logical-layer/export-name-function-takes-sealed-type.reject.lc`, `logical-layer/foreign-abi-function-takes-sealed-type.reject.lc`, `logical-layer/foreign-abi-generic-at-sealed-type.reject.lc`, `logical-layer/foreign-abi-pointer-in-a-field.reject.lc`, `logical-layer/foreign-abi-pointer-takes-sealed-type.reject.lc`, `logical-layer/foreign-callable-returns-sealed-type.reject.lc`, `logical-layer/foreign-callable-takes-sealed-reference.reject.lc`, `logical-layer/foreign-callable-takes-sealed-type.reject.lc`, `logical-layer/naked-asm-sym-function-takes-sealed-type.reject.lc`, `logical-layer/naked-function-takes-sealed-type.reject.lc`, `logical-layer/sym-function-takes-sealed-type.reject.lc` |
| `logic.boundary.escape` | `logical-layer/fact-after-unknown-call.accept.lc`, `logical-layer/fact-re-established.accept.lc`, `logical-layer/mut-behind-trait-object-rewritten.accept.lc`, `logical-layer/owned-value-from-unverified.accept.lc`, `logical-layer/raw-write-to-other-local.accept.lc`, `logical-layer/unverified-callee-keeps-no-access.accept.lc`, `logical-layer/verified-call-keeps-facts.accept.lc`, `contracts/invariant-type-values-forgotten.reject.lc`, `logical-layer/marked-unverified-may-write.reject.lc`, `logical-layer/mut-behind-trait-object.reject.lc`, `logical-layer/unverified-result-unknown.reject.lc` |
| `logic.unverified.functions` | `logical-layer/assumption-keeps-function-verified.accept.lc`, `logical-layer/foreign-shim-checks-its-input.accept.lc`, `logical-layer/marked-verified-plain-signature.accept.lc`, `logical-layer/naked-function-over-plain-data.accept.lc`, `logical-layer/transmute-between-runtime-types.accept.lc`, `logical-layer/union-of-runtime-fields.accept.lc`, `logical-layer/unsafe-in-plain-function.accept.lc`, `logical-layer/unverified-callee-keeps-no-access.accept.lc`, `logical-layer/unverified-result-checked-at-run-time.accept.lc`, `logical-layer/marked-unverified-may-write.reject.lc`, `logical-layer/marked-verified-with-unsafe.reject.lc`, `logical-layer/naked-function-marked-verified.reject.lc`, `logical-layer/naked-function-takes-sealed-type.reject.lc`, `logical-layer/raw-write-to-local.reject.lc`, `logical-layer/unsafe-function-with-proof-parameter.reject.lc` |
| `logic.verified.restrictions` | `logical-layer/marked-verified-plain-signature.accept.lc`, `logical-layer/plain-function-beyond-what-locus-reasons-about.accept.lc`, `logical-layer/assumption-beside-unsafe-block.reject.lc`, `logical-layer/marked-verified-with-unsafe.reject.lc`, `logical-layer/naked-function-marked-verified.reject.lc`, `logical-layer/naked-function-takes-sealed-type.reject.lc`, `logical-layer/raw-pointer-cast-to-proof-holder.reject.lc`, `logical-layer/raw-write-to-local.reject.lc`, `logical-layer/unsafe-function-with-proof-parameter.reject.lc` |
| `logic.unverified.plain-rust` | `logical-layer/foreign-result-checked-by-constructor.accept.lc`, `logical-layer/unsafe-in-plain-function.accept.lc`, `logical-layer/unverified-function-given-fields.accept.lc`, `logical-layer/extern-returning-invariant-type.reject.lc`, `logical-layer/marked-unverified-returns-proof.reject.lc`, `logical-layer/proof-through-raw-pointer.reject.lc`, `logical-layer/proof-through-transmute.reject.lc`, `logical-layer/raw-pointer-cast-to-proof-holder.reject.lc`, `logical-layer/transmute-into-invariant-type.reject.lc`, `logical-layer/unverified-function-calls-proof-function.reject.lc`, `logical-layer/unverified-function-takes-invariant-type.reject.lc`, `logical-layer/unverified-function-takes-logical-holder.reject.lc`, `logical-layer/zeroed-invariant-type.reject.lc`, `proofs/assumption-in-unverified-function.reject.lc` |
| `logic.unsafe.invariant` | `logical-layer/invariant-type-through-unverified.accept.lc`, `logical-layer/unverified-function-given-fields.accept.lc`, `logical-layer/transmute-into-invariant-type.reject.lc` |
| `logic.code.logical-values` | `logical-layer/logic-fn-model-result.accept.lc`, `logical-layer/logic-fn-runtime-result.reject.lc`, `logical-layer/logic-fn-unit-result.reject.lc` |
| `logic.eq.same` | `logical-layer/same-is-a-proposition.accept.lc`, `logical-layer/sameness-on-sequences.accept.lc`, `logical-layer/same-is-not-a-bool.reject.lc` |
| `logic.eq.runtime-values` | `logical-layer/equal-models-equal-field-models.accept.lc`, `models/derived-model-equal-fields.accept.lc`, `logical-layer/equal-models-not-equal-fields.reject.lc` |
| `logic.eq.snapshots` | `logical-layer/cell-snapshot-kept-by-move.accept.lc`, `models/snapshot-through-generic-view.accept.lc`, `models/snapshot-through-reference.accept.lc`, `models/snapshot-unchanged.accept.lc`, `models/snapshot-with-associated-type-field.accept.lc`, `models/snapshots-equal-fields.accept.lc`, `logical-layer/cell-contents-unknown.reject.lc` |
| `logic.eq.comparisons` | `logical-layer/comparison-is-a-bool.accept.lc`, `logical-layer/comparison-on-sequences.reject.lc`, `logical-layer/same-is-not-a-bool.reject.lc` |
| `logic.code.no-runtime-calls` | `logical-layer/logic-calls-logic-function.accept.lc`, `logical-layer/logic-calls-runtime-function.reject.lc` |
| `logic.fn.total` | `logical-layer/measure-recursion.accept.lc`, `logical-layer/structural-recursion.accept.lc`, `logical-layer/measure-increases.reject.lc`, `logical-layer/mutual-recursion.reject.lc`, `logical-layer/nonterminating-logic-fn.reject.lc`, `proofs/diverging-logic-proof.reject.lc` |
| `logic.const.well-founded` | `logical-layer/logical-constant-chain.accept.lc`, `logical-layer/logical-constant-cycle.reject.lc`, `logical-layer/static-justifies-itself.reject.lc`, `logical-layer/statics-justify-each-other.reject.lc` |
| `logic.code.statics` | `logical-layer/immutable-static-in-logic.accept.lc`, `logical-layer/sym-static-over-plain-data.accept.lc`, `logical-layer/no-mangle-static-holding-proof.reject.lc`, `logical-layer/static-mut-in-logic.reject.lc`, `logical-layer/sym-static-holding-proof.reject.lc` |
| `models.snapshots` | `models/generic-snapshot-at-cycle.accept.lc`, `models/read-through-cut-reference.accept.lc`, `models/snapshot-of-vector-without-model.accept.lc`, `models/snapshot-through-generic-view.accept.lc`, `models/snapshot-through-reference.accept.lc`, `models/snapshot-unchanged.accept.lc`, `models/snapshots-equal-fields.accept.lc`, `models/model-through-a-cut.reject.lc`, `models/snapshot-through-generic-cycle.reject.lc`, `models/snapshot-through-reference-cycle.reject.lc` |
| `models.snapshots.opaque` | `models/snapshot-with-associated-type-field.accept.lc`, `models/snapshot-of-union.reject.lc`, `models/snapshot-through-associated-type.reject.lc` |
| `models.snapshot-privacy` | `models/private-field-in-own-module.accept.lc`, `models/private-field-of-snapshot.reject.lc` |
| `models.frozen-data` | `logical-layer/cell-snapshot-kept-by-move.accept.lc`, `models/hand-written-model.accept.lc`, `logical-layer/cell-contents-unknown.reject.lc`, `models/model-reads-cell.reject.lc` |
| `models.observation` | `models/integer-model-no-overflow.accept.lc`, `observation/after-mutable-borrow-ends.accept.lc`, `observation/before-move.accept.lc`, `models/wrapping-is-not-the-model.reject.lc` |
| `models.no-default` | `models/snapshot-of-vector-without-model.accept.lc`, `models/snapshots-equal-fields.accept.lc`, `models/equal-fields-without-model.reject.lc`, `models/vector-without-element-model.reject.lc` |
| `models.integer-range` | `models/integer-range.accept.lc`, `models/integer-range-tighter.reject.lc` |
| `models.one-per-type` | `models/hand-written-model.accept.lc`, `models/two-models.reject.lc` |
| `models.derived` | `logical-layer/equal-models-equal-field-models.accept.lc`, `models/derive-model-for-recursive-type.accept.lc`, `models/derived-model-equal-fields.accept.lc`, `models/derived-model-keeps-proof-holder.accept.lc`, `logical-layer/equal-models-not-equal-fields.reject.lc`, `models/derive-model-for-generic.reject.lc`, `models/derive-model-through-generic-cycle.reject.lc`, `models/derive-model-through-reference-cycle.reject.lc`, `models/derive-model-with-cell-field.reject.lc` |
| `models.bounds-on-functions` | `models/model-bound-on-function.accept.lc`, `models/model-bound-on-impl.reject.lc` |
| `models.declared-by-owner` | `models/hand-written-model.accept.lc`, `models/model-for-foreign-type.reject.lc` |
| `models.own-definition` | `models/hand-written-model.accept.lc`, `models/model-helper-reads-fields.accept.lc`, `models/model-of-itself.reject.lc`, `models/model-through-helper.reject.lc`, `models/models-depend-on-each-other.reject.lc` |
| `proofs.versions` | `observation/proof-before-move.accept.lc`, `proofs/reproved-after-assignment.accept.lc`, `proofs/snapshot-survives-assignment.accept.lc`, `proofs/stale-proof-across-iterations.reject.lc`, `proofs/stale-proof-after-assignment.reject.lc`, `proofs/stale-proof-after-same-value.reject.lc`, `proofs/stale-proof-through-reference.reject.lc`, `proofs/stored-proof-after-clear.reject.lc` |
| `proofs.connectives` | `proofs/connectives.accept.lc`, `proofs/conjunction-needs-both.reject.lc` |
| `proofs.props.constructors` | `proofs/positive-proposition.accept.lc`, `proofs/wrong-arm.reject.lc` |
| `proofs.props.matching` | `proofs/match-every-arm.accept.lc`, `proofs/missing-arm.reject.lc` |
| `proofs.elimination` | `proofs/witness-stays-in-proof.accept.lc`, `proofs/witness-to-data.reject.lc` |
| `proofs.partial` | `proofs/diverging-runtime-proof.accept.lc`, `proofs/returns-on-some-path.reject.lc` |
| `proofs.assume.lint` | `proofs/proof-where-assumptions-forbidden.accept.lc`, `proofs/assumption-where-forbidden.reject.lc` |
| `proofs.assume` | `logical-layer/assumption-keeps-function-verified.accept.lc`, `proofs/assumed-proof-field-used-elsewhere.accept.lc`, `proofs/assumed-result.accept.lc`, `logical-layer/assumption-beside-unsafe-block.reject.lc`, `proofs/assumption-in-unverified-function.reject.lc`, `proofs/unproved-result-without-assumption.reject.lc` |
| `contracts.entry-values` | `contracts/final-value-at-return.accept.lc`, `contracts/entry-value-is-not-final.reject.lc` |
| `contracts.final` | `contracts/final-value-at-return.accept.lc`, `contracts/final-value-in-a-proof.accept.lc`, `contracts/local-borrow-final.accept.lc`, `contracts/returned-borrow.accept.lc`, `runtime/two-reborrows-returned.accept.lc`, `runtime/write-through-a-reborrow.accept.lc`, `contracts/final-escapes-through-parameter.reject.lc`, `contracts/final-in-ghost-write.reject.lc`, `contracts/final-reads-no-place.reject.lc`, `contracts/final-through-captured-lifetime.reject.lc`, `contracts/wrong-final.reject.lc`, `runtime/borrow-kept-by-another-parameter.reject.lc` |
| `contracts.calls.unrelated` | `contracts/call-once-result-kept.accept.lc`, `logical-layer/trait-result-validated.accept.lc`, `proofs/assumed-result.accept.lc`, `contracts/same-call-twice.reject.lc`, `logical-layer/trait-result-trusted.reject.lc`, `proofs/unproved-result-without-assumption.reject.lc` |
| `contracts.invariants` | `contracts/filled-before-built.accept.lc`, `contracts/invariant-constructed.accept.lc`, `contracts/invariant-replaced-whole.accept.lc`, `contracts/stack-push-by-value.accept.lc`, `contracts/struct-update-reuses-proof.accept.lc`, `export/public-invariant-field-when-built.accept.lc`, `contracts/built-before-filled.reject.lc`, `contracts/invariant-macro-construction.reject.lc`, `contracts/proof-field-taken-from-another-value.reject.lc`, `contracts/struct-update-changes-read-field.reject.lc` |
| `contracts.invariants.reads` | `contracts/invariant-reads-through-shared-reference.accept.lc`, `contracts/invariant-reads-behind-a-cut.reject.lc`, `contracts/invariant-reads-through-mut-reference.reject.lc`, `models/proof-field-inside-a-cut-field.reject.lc` |
| `contracts.invariants.no-mutable-borrow` | `contracts/unread-field-borrowed.accept.lc`, `contracts/element-of-read-field-lent.reject.lc`, `contracts/invariant-field-borrowed.reject.lc`, `contracts/proof-container-cleared.reject.lc`, `contracts/proof-field-borrow-outlives-assignment.reject.lc`, `contracts/proof-field-borrowed.reject.lc`, `contracts/proof-read-field-captured-mutably.reject.lc`, `contracts/proof-read-field-implicit-ref-mut.reject.lc`, `contracts/proof-read-field-ref-mut.reject.lc` |
| `contracts.invariants.assignment` | `contracts/cursor-replaced-whole.accept.lc`, `contracts/invariant-replaced-whole.accept.lc`, `contracts/proof-field-assigned.accept.lc`, `contracts/proof-in-cell-closed.accept.lc`, `contracts/ring-push-back.accept.lc`, `contracts/stack-push-by-value.accept.lc`, `contracts/stack-push-with-stand-in.accept.lc`, `contracts/assignment-needing-proof-in-closure.reject.lc`, `contracts/dependent-tuple-component-written.reject.lc`, `contracts/invariant-broken-by-assignment.reject.lc`, `contracts/invariant-field-assigned.reject.lc`, `contracts/option-proof-field.reject.lc`, `contracts/proof-container-replaced.reject.lc`, `contracts/proof-field-through-box.reject.lc` |
| `contracts.invariants.proof-holders` | `contracts/proof-field-assigned.accept.lc`, `contracts/dependent-tuple-component-written.reject.lc`, `contracts/option-proof-field.reject.lc`, `contracts/proof-container-cleared.reject.lc`, `contracts/proof-container-replaced.reject.lc`, `contracts/proof-field-borrowed.reject.lc` |
| `contracts.unsafe-impls` | `contracts/hand-written-unsafe-impl.accept.lc`, `contracts/unsafe-send-on-invariant-type.accept.lc`, `contracts/macro-generated-unsafe-impl.reject.lc` |
| `contracts.invariants.observed` | `contracts/filled-before-built.accept.lc`, `contracts/stack-push-with-stand-in.accept.lc`, `export/public-field-of-sealed-type.accept.lc`, `logical-layer/invariant-type-through-unverified.accept.lc`, `contracts/built-before-filled.reject.lc`, `contracts/invariant-type-values-forgotten.reject.lc` |
| `contracts.slots` | `contracts/ring-push-back.accept.lc`, `contracts/stack-push-through-slot.accept.lc` |
| `contracts.slots.local` | `contracts/stack-push-through-slot.accept.lc`, `contracts/slot-moved-away.reject.lc`, `contracts/slot-not-filled-on-a-path.reject.lc`, `contracts/slot-taken-by-unverified.reject.lc` |
| `export.plain-interface` | `export/dropped-postcondition.accept.lc`, `export/locus-logical-field.accept.lc`, `export/private-enum-with-invariant.accept.lc`, `export/private-logical-field.accept.lc`, `export/public-enum-with-sealed-payload.accept.lc`, `export/public-field-of-sealed-type.accept.lc`, `export/public-precondition-when-built.accept.lc`, `export/wrapped-precondition.accept.lc`, `export/model-bound.reject.lc`, `export/public-enum-with-invariant.reject.lc`, `export/public-logical-field.reject.lc`, `export/public-postcondition.reject.lc`, `export/public-precondition.reject.lc`, `export/public-proof-in-option.reject.lc` |
| `export.invariants-closed` | `export/private-invariant-fields.accept.lc`, `export/public-field-of-sealed-type.accept.lc`, `export/public-invariant-field-when-built.accept.lc`, `export/public-invariant-field.reject.lc`, `export/public-sealed-field-read-by-proof.reject.lc` |

The examples use this RFC's syntax. Where [proposition-syntax](../../issues/proposition-syntax.md) later respells something, their outcomes stay the same.

## Interactions

| Area | Effect |
|---|---|
| Types | Every type is runtime or logical, by its type. Logical types contain only logical types. Runtime types may hold logical types, which are markers at run time: `Erased` for proofs and `LogicalData` for other logical data. `Int` is built in, and `Nat` is a library type whose difference is an `Int`. Every runtime type has a snapshot type `Snap<T>`, and a model only when one is declared, derived, or given by the library. |
| Propositions and proofs | Propositions are Lean's, with Rust's syntax: inductive propositions with named arms, defined predicates, connectives, and quantifiers. `===` is Lean's equality, and comparisons give `Bool`s, as Rust's give `bool`s. Propositions are about versions. Proofs are irrelevant, ghost, and kernel-checked. |
| Ownership and mutation | Logical code reads places under RFC 0001's observation rules. Every write makes a new version. A `&mut` reference has a current value and a final value, `final!`. How runtime code changes values in the logic is [runtime-semantics](../../issues/runtime-semantics.md). |
| Erasure and runtime behavior | In build mode, every ghost value is a zero-sized marker in both programs, and nothing is erased. An exported program removes ghost parameters and locals, and keeps markers inside runtime types and fields. Runtime parts such as tags and lengths are kept, every call of a function with a precondition is checked, and calls of runtime functions are kept even when their results are ghost. |
| Rust interoperability | Rust code never sees a proof. In build mode, every crate that uses a Locus crate is verified. In export mode, and in the first stage, what other crates can reach exposes no logical data: no contracts, and no reachable field that exposes logical data or that a proof reads, so Rust users can hold a value with an invariant and cannot break it. `pub(locus)` keeps the rest for Locus crates. Unverified code is given plain data and sealed values, and is never instantiated with a type that exposes logical data. Foreign code is given plain data only, and gives back plain data only. Models are declared by a type's own crate. |
| Diagnostics | New errors: logical values where runtime ones are needed, runtime calls in logical code, non-terminating logic functions, proofs about other versions, logical data given to unverified code, a function that must be verified and contains `unsafe` code, a field that an invariant reads changed in place, invariants not established, and contracts or logical values in an exported interface. They are written as reasons until error codes exist. |
| Trust model | New entries: the snapshots and models that Locus's library declares for standard types; the propositions that a program or Locus's library states with `unsafe assume!`; that the types Locus's library provides around the standard library's keep what they are given; the consistency of Lean's axioms; that code Locus does not verify is sound Rust, which covers the standard library, the crates a project names, and each of a Locus crate's unverified functions with `unsafe` code; that `unsafe` code keeps the invariants of sealed types, an obligation Locus adds to Rust's; that foreign code keeps to the signatures the program declares for it; and that the system beneath the standard library behaves as it assumes. The earlier entry for Rust's aliasing rules is removed. The logical pass's ghost checks replace RFC 0001's checks on proofs in runtime code, and where borrows end joins rustc's borrow-checker entry. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Proofs as zero-sized values that Locus believes by their type.** This RFC keeps such a marker, `Erased`, but Locus never believes it.
- Proofs would flow through every generic function for free.
- Rejected. With one Rust type for every proposition, Rust code that can tell types apart, through `Any` or specialization, could turn one proof into another.
- Making that sound needs every Rust generic to be parametric, which neither specialization nor `unsafe` code guarantees.

**No logical types inside runtime types,** apart from proofs inside `Option` and `Result`.
- Rejected at the owner's direction. Containers of ghost values are useful, and forgetting ghost contents at unverified code makes them sound.

**Proof tokens.** Each proof parameter would become a parameter of a private zero-sized token type, which erasure supplies only at calls Locus has checked. Other crates would obtain tokens through an `unsafe` constructor, and a function whose precondition sits in its parameters' data would be `unsafe` to call.
- Rejected at the owner's direction for the two modes. Contracts are visible only to code that Locus verifies, so nothing needs a token.

**`unsafe` exports:** proof parameters removed, as here, with each exported function that has a precondition made `unsafe`. Rejected: export mode keeps contracts out of the exported interface instead, and a wrapper checks or drops them.

**`Nat` built in, with `Int` from the library.**
- Rejected at the owner's direction. With `Int` built in, `Nat` is a subtype, and subtracting two naturals gives an integer rather than truncating at zero, as Lean does. So `v.len() - 1` cannot silently become `0`.

**`()` instead of a named marker** for logical values inside runtime types, with `Vec<()>` for `Vec<Nat>`.
- Rejected at the owner's direction. Named markers read more clearly in generated code and in rustc's messages, and rustc itself refuses the traits they do not implement, such as `Default`.
- Neither adds safety inside the program: Locus never believes what a runtime value holds unless it tracked it, so a forged marker carries no more weight than a forged `()`.
- At the Rust boundary, the marker is never seen: build mode checks every crate that could see it, and export mode keeps it out of what other crates can reach.

**Distinct Rust types per proposition.** Not possible: propositions mention runtime values.

**`old(x)` for the entry value and bare `x` for the value at return,** as in Prusti.
- Rejected for one rule: a signature means entry values, and `final!` names the final value.
- That rule also covers borrows that are returned, kept in locals, or stored, which `old` cannot express.

**Several models per type, chosen with `x as M`.** Rejected: one canonical model makes `v.len()` in logical code unambiguous. Other interpretations are logic functions.

**Models before field access,** where logical code would take the receiver to its model before selecting a field.
- Rejected: keeping Rust's meaning for places makes logical code read like Rust, and works for types without models.
- Models apply where a value is used.

**One equality, `==`, meaning Lean's `Eq`,** with every comparison a proposition.
- Rejected at the owner's direction. In Rust, `==` is a programmable comparison that gives a `bool`, and a reader expects the same in logical code.
- Lean also separates `==` (`BEq`, a `Bool`) from `=` (`Eq`, a `Prop`), and ties them together with `LawfulBEq`. Here `===` is Lean's `=`.

**A view of every runtime value, its frozen part,** as the input to models. Replaced by snapshots, which are the same idea stated as values: what Rust's aliasing rules freeze, with containers built on raw pointers given their contents by Locus's library.

**A value without a model as an `Opaque<T>`, equal to another when their visible fields are equal.** This was the previous draft. Rejected: it is unsound when a field's model forgets something. With `Inner { x }` modeled as `0` inside `Outer { inner }`, two `Outer`s whose `x` differ have equal fields, so they would be equal, and reading `o.inner.x` from each would prove `0 === 1`. The flaw was building equality from fields' models. Snapshots are built from fields' snapshots, so their equality is the values' own.

**An opaque identity as the default model.** Rejected. It is consistent, because a write makes a new version, and interior mutation makes none and is never in a snapshot. But it tells almost nothing: two identities are equal only when they come from one version.

**An identity minted per object,** which a clone or a copy would change and a write would keep. Rejected for now. A Rust value has no identity of its own: a move changes its address, and a copy duplicates its bits, so the identity would have to live in the value, and a copy would carry it. An identity for reasoning about aliasing, such as which `Cell` two references share, can come later beside values.

**A type's snapshot as its default model.** Deferred at the owner's direction. It would let `p === q` compare whole values for any type without a model, and give `Vec<T>` a model for every `T`. But adding `impl Model` to a type later would silently change what `===` means in its clients' specifications. Explicit `snap(x)` keeps the two meanings apart, and the restriction can be lifted later.

**Snapshots by default in logical code, and models only through `model(x)`,** as Creusot's `View` and Verus's views work. Rejected at the owner's direction. Specifications are about what values mean more often than what they are, and models by default let `x + 1` and `v.len()` read as they do now.

**One marker for proofs and logical data.** Rejected. A marker that implements `PartialEq` and `Hash`, as proofs need, lets rustc accept `==` on two `Vec<Nat>`s, which would compare only their lengths, and a derived `PartialEq` that ignores a `Nat` field. The alternatives were for Locus to check every trait use that rustc satisfied through the marker, which makes it a second trait solver, or to give up `==` and `Hash` on values that hold proofs.

**Proof fields that can be borrowed mutably,** because whatever is written through the borrow must be a proof of its proposition. This was the previous draft. Rejected: the borrow outlives assignments to the fields the proposition reads. With `hint: Option<@(x != 0)>`, a borrow taken while `hint` was `None` could write a proof about the old `x` after `x = 0` was accepted. A container of proofs whose length another proof reads would also have been both borrowable and not.

**Exact completability,** deciding for each type whether every erased value is the erasure of some value. Rejected: that asks whether types are inhabited, which no compiler can decide in general. The conservative judgment can be widened later, for example with a declaration that a type is completable, proved by giving a value.

**Replacing a value with an invariant whole** for every change to a field that its proofs read. Rejected at first, because an assignment can only expose the old state or the new one, so proving the new one is enough. Adopted by decision 66: with it, one rule covers assignment and borrowing alike, and a value is valid at every point.

**No `unsafe impl` but `Send` and `Sync`** on a type with proof fields. Rejected: a hand-written `unsafe impl` is the programmer's own promise, which the trust model already covers. Only a generated one bypasses it.

**`mint`, an unspecified `Int` for each value of a type without a model.** Rejected: it tells logic nothing that a snapshot does not, and loses the type, so logic could do meaningless arithmetic on the integers.

**Inferring ghost bindings from types before rustc runs,** so that `let k = v[0]; let m = k + 1;` would work without `logic!`. Rejected: lowering would need `v`'s type, and so type inference and method resolution over the standard library, which is rustc's work. A wrong guess can compile with another meaning ([RFC 0001](0001-compiler-pipeline.md#alternatives)).

**Types with interior mutability holding no logical values.** Rejected at the owner's direction: proofs are irrelevant, so a cell's contents need only have the right type, and keeping unverified code from writing them is enough. The fourth review found the condition that argument needs, that the type cannot change under the cell (decision 47).

**Euclidean division for `/`,** as Lean's `/` on integers is. Rejected: a machine integer's `/` truncates, and its model's should agree. The Euclidean operations are `div_euclid` and `rem_euclid`, as in Rust.

**CamelCase for logic functions that give a `Prop`,** as Lean names propositions. Rejected at the owner's direction: they are functions, and Rust names functions in snake_case. CamelCase marks `prop` declarations, which are matched like enums.

**An anonymous constructor for one-arm propositions,** such as `(hp, hq)` for `And::Intro(hp, hq)`, as Lean's `⟨hp, hq⟩` is. Deferred by the owner to the RFC on proof construction ([proof-construction](../../issues/proof-construction.md)).

**`=>` for implication.** Rejected: inside a logical `match`, `=>` already separates an arm from its body, so `==>` is clearer.

**`forall (x: T) { F }` keywords.** Rejected for `forall |x: T| F`, which reads like Rust's closures and needs no new delimiters.

**A `ghost` keyword or a `Ghost<T>` wrapper,** as in Verus. Rejected: the type already says whether a value is ghost.

**Tracked evidence,** a `let mut h: @P` that follows the current version of what it reads. Taken up in a simpler form by [RFC 0004](0004-runtime-code.md): a `let mut` proof variable that the program assigns again, with nothing that follows a version. It serves as a loop's invariant there.

**Judging writability by variance.** A type would grant write access to a type argument exactly when it is not covariant in it, as rustc computes, in place of a list of writable types. Rejected: variance follows a type's definition and not what its methods do, so a covariant `Handle<T>` that holds an `Rc<dyn Any>` and a `PhantomData<T>` can still write a `T` through `&self` (tested). Variance can help describe known types, never admit unknown ones.

**Treating an invariant as one of Rust's safety invariants,** so that sound `unsafe` code would keep it by definition. Rejected as a framing: Rust's soundness is about undefined behavior, and `lo <= hi` is a functional fact that Rust does not ask anyone to keep. Locus states it as its own obligation, and lists it as an assumption.

**Knowing an exposed place only by its type, forever.** Rejected in an earlier revision: it is sound but needlessly weak, since a write that Locus sees fixes the place's value until the next call that may change it, and it says nothing about how escape crosses calls. Decision 54 has since removed escape: no place of verified code is exposed.

**A list of the standard types writable through a shared handle,** which earlier drafts trusted to be complete. Replaced by descriptions of what each standard type grants, where a type without one is assumed to grant everything, so that a missing entry makes Locus stricter rather than unsound.

**Judging what unknown code can reach from the visible type of what it is given.** Rejected after the third review: a `&mut` held by a trait object is invisible in the trait object's type, so `n` would keep a fact that unverified code had falsified. Rustc's borrow facts follow the loan through the coercion.

**Cutting a snapshot at the reference that lies on a cycle, per definition.** This was decision 18's first wording, and it misses a cycle through a generic wrapper: `Wrap<'a, T>`'s reference lies on no cycle in `Wrap`'s own definition, yet `Node<'a> { w: Wrap<'a, Node<'a>> }` closes one through it. Two repairs were considered and rejected:
- **Cutting per instantiated type,** so that `Snap<Wrap<u32>>` follows its reference and `Snap<Wrap<Node>>` does not. Then `Wrap<T>`'s snapshot has a shape that depends on `T`, and a fact proven about it in generic code need not hold at `Node`.
- **Always cutting a reference whose target involves a type parameter.** It is uniform, but every generic borrowed view, such as a slice view or an iterator, would hold only addresses, and its model, being a function of its snapshot, could not read its contents.

Cutting at the field of the type that closes the cycle keeps generic types uniform and views transparent, and costs only the fields that close a cycle.

**No forgeable proof under interior mutability at all,** nor in any type that Locus has no description for. The fourth review proposed this, and an earlier state of this revision had it. Rejected at the owner's direction for the narrower rule of decision 47. It lost `let slot: Cell<Option<@(n > 0)>>` for a local `n`, which is sound, since nothing that proposition reads can change under the cell. And what it would have removed, the description of what access standard types grant, already errs on the strict side for a type without one.

**Assigning a field that a proof under a `Cell` reads, when no shared reference to the cell is live.** This was the smallest repair of the stale reader, decided from rustc's borrow facts. Rejected: it does not cover a cell shared through `Rc` or a reference stored in a field, and it keeps the rest of the machinery.

**Prophetic values in ghost state, tracked by a mark on each binding,** as Creusot marks its prophetic logic functions. More programs would be accepted: a ghost local could name a final value, so long as it never reached a place the same borrow covers. Rejected for now: every ghost binding, field, and argument would carry the mark, and a signature's result already says what a caller needs.

**Types equal when their propositions are provably equivalent.** It is sound, since equivalent propositions are equal in Lean's logic, and it would accept `Option<@(0 < n)>` where `Option<@(n > 0)>` is needed. Deferred to RFC 0003, which decides how proofs are built and stored: each such match would be an obligation for proof search, at every place where two types meet.

**Treating a place lent to unverified code as escaped for good.** This was the previous draft: after `dep::touch(&mut n)`, every later call of unknown code made `n` unknown again. It defended against a library that keeps a pointer past the borrow. Such a library is not sound Rust, and the design needs unverified code to be sound Rust in any case: without that, a callee given `&Vec<T>` could write the vector's buffer, and a dependency could keep a pointer into a buffer it returned, which the previous draft missed (tested: the program runs and the buffer changes). So the rule protected one case of a failure that the others were open to, and cost every fact about data that a dependency had touched.

**Relying on Rust's aliasing rules alone,** with no assumption that unverified code is sound. Rejected: Stacked Borrows and Tree Borrows protect what a reference points to, not what a `Vec` or a `Box` behind it owns, so no fact about heap data would survive any call that was given a reference to its owner.
**No block: the invariant is open while a borrow lasts, and Locus reasons about unwinding.** `self.items.push(x)` would be accepted as written, with the invariant proved again where rustc's borrow facts end the borrow, and, for every operation in between that may panic, proved for the state that unwinding would leave. Rejected for now ([opening-invariants](../../issues/opening-invariants.md)): every specification would gain an outcome for unwinding, an update of two fields has no borrow that spans both, and where the invariant is open would be decided by borrow analysis instead of shown in the source. It can be added later for single specified calls, as a way to leave the block out.

**One assumption that every unverified crate is sound.** This was decision 45's first form. Narrowed by decision 52: safe crates need no assumption, and the others are named, so what was trusted is a list short enough to read.
**Verifying the code around an `unsafe` block.** This was the design through the fourth review: a function could hold both proofs and `unsafe` blocks, and Locus followed which places had escaped to raw pointers. Replaced by decision 54. A block shares a stack frame with the code around it, so following addresses leaned on the part of Rust's aliasing rules that is not settled, and making values from raw memory needed a list of operations to reject.

**A superset of safe Rust only,** in which a Locus crate has no `unsafe` code and all of it lives in other crates. This is decision 54 at the grain of the crate. Rejected: one `get_unchecked` would force a crate to be split, and a Rust crate with `unsafe` code would not be a Locus crate.

**Requiring the mark on every function with `unsafe` code.** Rejected by the owner: a Rust file with `unsafe` code would no longer compile unchanged, against the vision's first commitment. A function that states a proof and gains an `unsafe` block is rejected either way, with an error that says why.

**`assume!(P)` with no mark,** a proof hole that is trusted instead of proved. Rejected in that form, and taken with the mark, as `unsafe assume!(_)` (decision 80). The objection was to the missing mark. Proofs will come to justify `unsafe` operations, so a false assumption in code that looks safe could cause undefined behavior, which safe Locus must never do (vision, commitment 9). Rust makes `core::hint::assert_unchecked` an `unsafe fn` for the same reason.

**Contracts attached to functions from outside their signatures:** an attribute on an unverified function, or a declaration that mirrors another crate's function, each leaving the function's Rust type as it is and the fact in scope after the call. Rejected by the owner (decision 80): "I don't like just assuming facts in scope - prefer these to become native proofs in returns". A proof cannot be put in the result of a function whose Rust type is already fixed, so these forms could only add facts that no value holds. The specifications that Locus's library had for standard functions were of this kind, and decision 81 ends them. [boundary-contracts](../../issues/boundary-contracts.md) records the forms.

**A function whose whole body is trusted,** marked on the function, with a Locus signature and a plain Rust body. Rejected for `unsafe assume!(_)` at the owner's suggestion. A mark on the function trusts everything in it; a constructor of proofs trusts one proposition, and leaves the rest of the function checked. The mark could do two things that a proof cannot: let `unsafe` code sit behind a proof parameter, and hand logical data to unverified generic code. The first waits for permissioned pointers, and the second is trusted in Locus's own library alone (decision 83).

**Choosing Locus's types by context,** so that `Vec` meant Locus's inside Locus code with no import. Rejected by the owner for an explicit import (decision 81). An implicit switch would change what valid Rust means when it is moved into Locus code: `let n: usize = v.len();` could stop compiling. Adding a default later breaks nothing, and removing one would.

**Swapping every standard type,** `Option`, `Result`, and `Box` included. Rejected (decision 82). The language, and every signature of the standard library, uses its own `Option` and slices, so a second `Option` would meet the first at every call, and `match` already tells Locus everything about one.

**Verifying `unsafe` code with permissions,** as Verus does and separation logics do. Decision 59 takes the first step for Locus's own library: its collections are verified on permissioned pointers, which needs ghost values that are not `Copy`. Verifying a program's own `unsafe` code the same way, so that `assumed.unsafe.sound` is discharged for a function, comes after.

**Isolating unverified code** in another process or in WebAssembly, so that it cannot touch verified code's memory. Not pursued: it gives up calls that cost nothing, and the standard library cannot be isolated.

**Letting logical data cross where an analysis shows it harmless.** This was the design through the previous revision. A completable value, a value of a type with an invariant, and a proof lent by shared reference could all reach unverified code; Locus decided from a value's type what write access it granted, with a description for each standard type; and what came back was forgotten. Replaced by decision 61. Every review found a case the analysis had missed, and safe code can change a marker behind a generic parameter through `Any`, so each crossing needed its own argument. One rule that lets nothing cross needs none. The alternatives above that speak of completability, of write access, and of what is forgotten belong to that design, and are kept as its record.

**Checking the boundary where a generic function is written,** and not where it is instantiated. Rejected: the function's own text can show no logical data while a caller passes some through a type parameter and on into unverified code.

**No value with an invariant in unverified hands at all.** This was the first form of decision 62: a type with proof fields counted as exposed logical data. The rule was one sentence, and it cost too much: such a value could not be printed, stored in another crate's container, or shown to Rust users of an exported crate. Replaced by sealed types (decision 73), whose argument is short: the type is its own to rustc, and nobody who cannot reach its fields can build one.

**A derive of Locus's own for `Debug`, or a specification of the standard library's formatting code alone,** in place of the loan of decision 76. Rejected. Either would make a derived `Debug` work and nothing else: an `Option<@P>` could still not be printed, or read by any other code without a specification. The second needs the same exception for a `&dyn Debug` that the loan does, and the argument for a loan does not depend on which code holds the reference.

**Following a type parameter through the generic code of other crates,** so that an `Option<@P>` could be given to any generic function whose body only moves it. Rejected for now (decision 77). Whether a program compiled would depend on the private bodies of its dependencies, which a patch release may change. A body with no `unsafe` code is not enough, since safe code can swap one marker for another through `Any`. And the standard library's own generic code is built on `unsafe` code, and needs specifications in any case. So the boundary is judged at signatures.

**Assigning a field that a proof reads, when the proof can be proved again,** and **an `open!` block** for changing such a field through a borrow. These were decisions 14 and 51. Withdrawn by decisions 66 and 67: a value that is valid at every point needs no rule for unwinding and no abort.

**A closure in place of a slot,** `mem::replace_with(self, |old| new)`. It is sound as an ordinary library function. It needs logical code in the body of a runtime closure, which RFC 0001 rejects for now, and what is proved of the new value inside the closure does not reach the function's result without more machinery. A slot keeps the code in a straight line.

**A block in place of a slot.** It is the syntax of `open!` under another reading, and the owner withdrew that syntax (decision 67).

**A contract on `put`, in place of reading it as a write.** It would state that the place ends with the value given, and `take_out` would have to hand the caller that final value to state it with. A body that can compute from a value it has yet to produce can prove `False` (`contracts/final-in-ghost-write.reject.lc`).

**A slot that only aborts, with no rule about paths.** A path that forgot `put` would abort when it ran. The rule makes it an error when the program is checked. The rule that a slot is never moved is needed either way: without it a slot can leak.

**Every method of Locus's `Vec` returning its proof,** `new`, `len`, and `pop` included. A program ported from Rust calls those for their values, so each such call would change, also where no fact is wanted.

**Every function that foreign code can call unverified,** which the owner raised: "Maybe all extern functions are unverified and the same rules apply?" It is sound, and it is one rule fewer. Not taken (decision 88). An unverified function names no type that holds logical data in its body, so the shim could not call `Positive::new`, and each such function would need a second, verified function behind it that takes the plain values. Restricting the signature closes the same hole, because the signature is all that foreign code sees, and a shim with `unsafe` code or a raw pointer is unverified anyway.

**A mark on every verified function,** as `const fn` is marked. Rejected: every method of a type with proof fields would carry it, and what a derive generates could not.

**A mark only where a plain signature hides a proof,** with every unmarked function unverified unless its signature holds logical data. The marks would then list the functions that turn a runtime check into a proof. Considered, and set aside for verified by default (decision 65), once it was clear that a function with nothing to prove costs nothing to call verified. A lint can still ask for `#[verified]` on such functions.

**Keywords, `verified fn` and `unverified fn`.** Rejected for attributes: the marks are now optional statements of intent, which is what Rust uses attributes for, and they need no reserved word.

**A block, `verified { ... }`, inside an unverified function,** as `unsafe { ... }` is written inside a safe one. Not taken: a block would know nothing of the code around it, so a check written just outside it would not count, and it adds a second unit of verification.

**A default kind for a module or a crate.** Deferred at the owner's direction; the inference and the two marks come first.

**Compiling a function whose signature holds logical data as an `unsafe fn`,** so that rustc itself keeps safe Rust from calling it. Not part of the current plan, at the owner's direction.

**An effect system,** in which not panicking would be one effect among others. Rejected by the owner as more than is needed: `#[no(panic)]` states the one claim that is wanted ([no-panic-claims](../../issues/no-panic-claims.md)).

## Prior art

- **Lean 4** is the model for the logic, the kernel, inductive propositions, and proof irrelevance, and for separating a `Bool` comparison (`BEq`) from equality (`Eq`).
- **Aeneas** reads borrow-checked Rust as pure functions, with a backward function for each mutable borrow that is given back. That reading is what this RFC's rules are held against.
- **Creusot** has logic functions, a mutable reference's current value `*x` and final value `^x`, and models through its `View` trait. Its handling of `&mut` is the model for `final!`, and it keeps `^` out of ghost code and snapshots, as this RFC keeps `final!` out of bodies.
- **RustHorn and RustHornBelt** (Matsushita et al., ESOP 2020 and PLDI 2022) gave prophecies for Rust's mutable borrows and proved them sound.
- **Verus** separates spec, proof, and exec code, erases ghost code completely, and models types with `view()`. Its `external_body`, like Creusot's `#[trusted]`, marks a function whose body the tool does not verify; Locus draws the same line, at `unsafe` without being told, and gives such a function no contract yet.
- **Prusti** writes entry values with `old(...)`, the convention this RFC replaces with entry-by-default and `final!`.
- **Dafny** has ghost variables and fields, `decreases` clauses, and erasure of ghost code.
- **F\* with Low\* and KaRaMeL** erases proofs to produce C, as Locus erases them to produce Rust.
- **Rust's own function kinds,** `unsafe fn` and `const fn`, are the model for a kind that the signature shows and the compiler enforces at every call, including calls through function pointers and traits.
- **"Memory Safety Bugs in Rust Standard Library"** (Microsoft FORGE Lab, November 2026) groups soundness bugs found in `std` under three kinds of misplaced trust in `unsafe` code: that code will not panic, that an implementation supplied by the caller behaves correctly, and that the system beneath behaves as expected. The first two are what this RFC's rules refuse to trust: a value with an invariant is never half-built when something can panic, and a call's result is known only by its contract, however often the call is repeated. Its cases are among the examples: `contracts/built-before-filled`, `contracts/same-call-twice`, and `logical-layer/trait-result-trusted`. The third kind is outside what Locus reasons about, and the trust model lists it.

## Unresolved questions

**Decided by the owner while this RFC was drafted.** Recorded here; accepting the RFC confirms them. Where the owner set the direction, the mechanism is this RFC's proposal.
1. **Ghost proofs and forgetting.** Proofs and logical values are zero-sized markers at run time. In build mode both programs keep every ghost position as a marker; an exported program removes proof parameters and other ghost parameters and locals. Ghost contents are tracked only through verified code and specified standard library code, and contracts are visible only to code that Locus verifies, through the two modes of RFC 0001.
2. **Logical values in runtime types,** such as `Vec<Nat>` and `Option<@P>`.
3. **The `&mut` convention:** entry values in signatures, and `final!(r)` for the value when the borrow ends, with no separate notation for entry values.
4. **`Int` built in, and `Nat` from the library,** with `Nat - Nat` an `Int`. Integer division truncates toward zero, as Rust's does, division by zero gives zero, and Euclidean division is `div_euclid`.
5. **Inductive propositions with named arms,** built and matched as `Arm(witnesses) @ proof`, and `&&`, `||`, and `!` on propositions.
6. **Equality.** `===` is Lean's equality, a proposition. `==` and the orderings give a `Bool`, and are built in only on `Int`, `Nat`, and `Bool` until [traits-on-logical-types](../../issues/traits-on-logical-types.md).
7. **Naming.** Logic functions in snake_case, and `prop` declarations in CamelCase.
8. **Snapshots and models.** Every runtime type has a snapshot type `Snap<T>`, derived from its definition, which holds every field and nothing behind an `UnsafeCell` or a raw pointer. A type has a model only when it is declared, derived with `#[derive(Model)]` from its fields' models, or declared by Locus's library; there is no default, for now. Logical code reads places as snapshots and uses values through their models, and `snap(x)` gives a snapshot explicitly. On runtime values, `===` compares models; on snapshots, values. Every logical computation produces a logical value. A type's model has the type `T::Logic`; a machine integer's snapshot lies within its type's range; `()`'s model is `Unit`; and a derived model leaves out its proof fields and no others, with no way yet to skip a field.
9. **Interior mutability.** Types with interior mutability may hold logical values. Decision 47 adds a condition: a proposition under interior mutability reads no other field.
10. **The traits logical types implement:** `Clone`, `Copy`, and `Debug`, plus comparison and hashing for proofs. So proofs and logical data have separate markers, `Erased` and `LogicalData`, and rustc itself rejects comparing, ordering, or hashing logical data. The owner named the second `LogicalData`, so that rustc's errors say the value is logical.
11. **Export mode's rules.** What other crates can reach states no contracts, postconditions included, has no logical values in its types or fields, and has no `Model` bounds. Other crates can neither construct a struct with proof fields nor reach a field that its proofs read.
12. **Proof construction** beyond this RFC's forms is a third foundational RFC ([proof-construction](../../issues/proof-construction.md)). An anonymous constructor for one-arm propositions waits for it, and arms do not become constructors with proof fields.
13. **The boundary with unverified code,** defined once for both modes. Completable values are exempt, judged conservatively from types. A function needing a proof is never coerced to a pointer, made a trait object, or passed to generic code other than the program's own. No value that grants write access to a forgeable proof reaches unverified code, which Locus decides from types and the library's descriptions of standard types (decision 32).
14. **Invariants.** Neither a proof field nor a field that a proof reads is mutably borrowed; both may be assigned, with the proofs that read them proved for the new value. This reverses an earlier decision that let proof fields be borrowed. Only `unsafe impl`s that derives or macros generate are rejected, and keeping an invariant is an obligation Locus adds for `unsafe` code (decision 33).
15. **Runtime calls in ghost positions** stay where they are, except as arguments of ghost parameters, which an exported program removes.
16. **Logical types flow forward,** never backward from a later use.
17. **Bindings shown to be ghost.** Outside `logic!`, an operator or method call computes with ghost values only when the source shows them to be ghost, and a runtime value is never taken to its model implicitly: passed to a logical parameter or given to a logical field, it is written as `logic! { x }`. A binding that holds a ghost value may be passed to a ghost parameter whether or not the source shows it. This keeps every decision lowering makes before rustc to what the source shows ([RFC 0001](0001-compiler-pipeline.md)).

**Decided by the owner after the review of this revision.**

18. **Every snapshot is finite.** A reference that closes a cycle of references among types holds only its address in a snapshot, whatever snapshot it is part of; `Box` stays transparent. Decision 39 places the cut.
19. **No forgeable proofs or invariant types in unions,** at any depth, as decision 41 states it. Decision 60 widens this to every logical type.
20. **The rule for functions that need proofs covers closures,** and every value that can be called with arguments that hold logical values that are not completable.
21. **A proof field whose contents Locus cannot see,** such as one inside a `Cell`, is proved for a new value of a field it reads by proving its proposition. Decision 47 replaces this: a proof field inside a `Cell` reads no other field, so no assignment has to prove it again.
22. **The opaque parts of a snapshot are unknown values,** which moves and copies keep.
23. **Where a `&mut` parameter's borrow ends is decided from rustc's facts,** counting the lifetimes that an `impl Trait` result or an `async fn` captures without naming them.
24. **Statics.** Logical code reads only immutable statics, and no static that code outside the program can write holds logical values that are not completable.
25. **Totality.** No mutual recursion for now, and a body that proof search completes is held to the same rule. A type that contains itself through `Box` may derive its model.
26. **In export mode, a function used as a value keeps its ghost parameters, as markers,** and Locus's macros may appear in items that an attribute macro or a derive takes, but not in a function-like macro's input or in any macro's output.
27. **Escape.** A place whose address reaches a raw pointer, foreign code, or inline assembly, is exposed as an integer, or is given mutably to code whose effect is unknown, escapes; a fact about it holds only between a write Locus sees and the next call or `unsafe` operation that may write it. Values from raw memory hold no proofs. This rests on a new assumption, Rust's aliasing rules as Stacked Borrows and Tree Borrows agree on them. Decision 54 removes escape: a raw pointer exists only in an unverified function, so no place of verified code escapes, and the assumption about Rust's aliasing rules is no longer needed.
28. **The guarantee.** A verified function's postconditions hold on normal return, provided its preconditions hold, the contracts of the code it relies on and does not verify hold, whether assumed or the conservative defaults, `unsafe` code keeps the invariants of invariant types, Rust's rules on undefined behavior and aliasing hold, and the trusted components do their part. Commitment 9 of the vision says this. Decision 54 drops the clause on aliasing.
29. **No conjuring, for now.** Verified code never makes a value that carries a forgeable proof, or a value of an invariant type, from memory of another type, where the source names the type, and no `extern` signature names such a type. Decision 55 replaces the list of operations: each of them needs `unsafe`, and an unverified function handles no such type.

**Decided by the owner after the second review of this revision,** following the recommendations of both reviewers.

30. **Three concepts.** A value may carry a forgeable proof, be of an invariant type, or grant write access; each rule names the one it protects, so `Vec<Interval>` may be lent to a dependency while `&mut Option<@P>` may not.
31. **Effects are inferred.** Each verified function's summary of what it may write, what access it may retain, and whether it runs unknown code is inferred and kept in its interface, with no annotations. Unknown code is assumed to do anything its access allows. Facts are established again by writes Locus sees, and specifications such as `Vec::push`'s narrow the effect. Decision 45 changes what a summary records: what a function may write, whether it lets an address escape, and whether it may run unverified code or an unknown `unsafe` operation. Decision 54 changes it back: a summary records only what a function may write.
32. **Access is judged from types, and unknown types grant everything.** Locus's library describes what access standard types grant, and a missing description makes Locus stricter. A callable grants what calling it yields. A conversion that hides access, such as coercion to a trait object, is rejected. At first every transfer of access to a forgeable proof is rejected.
33. **An invariant is an obligation Locus adds,** not one of Rust's safety invariants. It holds wherever verified code can observe the value, including callbacks, drops, and unwinding. Opening an invariant in verified code is deferred, and so is a trusted form for zero-copy construction. Decision 51 has since added `open!`.
34. **Specifications at the boundary** state what unverified code writes, retains, initializes, and returns, each as an assumed specification. `unsafe` authorizes no fact by itself. Their syntax belongs to the standard library's RFC ([standard-library](../../issues/standard-library.md)). Decision 56 defers them for programs: for now only Locus's library states any ([boundary-contracts](../../issues/boundary-contracts.md)). Decisions 80 and 81 end them: a contract is a signature, and Locus's library has types of its own.
35. **Snapshot cuts are compositional,** and reading through a cut reference as a place is unaffected. Decision 39 places the cut.
36. **Derived models keep the runtime structure of fields that hold proofs,** leave out only bare proof fields, and are rejected for a type that reaches itself through a reference.
37. **Export keeps what Rust still needs:** a function used as a value keeps its ghost results as well as its parameters, a tuple result keeps its shape, and a ghost local named in a function-like macro's input stays as a marker.

**Decided by the owner after the third review,** following the recommendations.

38. **Escape follows loans, not visible types.** What a value given to unknown code can reach is decided from rustc's borrow facts, so a `&mut` held by a trait object, an opaque type, a closure, or a container counts. Every call or `unsafe` operation that may write a place, implicit calls included, makes its value unknown, and proofs about its earlier versions say nothing about the new one. Decision 45 keeps the loans and drops the escape: they decide what a call may write while it holds a borrow, and a place that was only lent is known again after a write Locus sees. Decision 54 removes the rest of escape.
39. **A snapshot is cut at the field that closes a cycle,** following type arguments, so a generic type's snapshot has one shape for every type argument, and borrowed views keep their contents. Derived and hand-written models form no cycle through one another.
40. **Access is a judgment over three modes,** owned, shared, and mutable. `&`, `Rc`, and `Arc` give shared access, which writes only through interior mutability, and a type of the program's own grants access only through its type arguments.
41. **Unions hold no forgeable proof and no value of an invariant type, at any depth.** Completable values are allowed. Decision 60 withdraws that allowance.
42. **An assignment that needs a proof is rejected inside a runtime closure or an async block,** as logical code there is, until [closures-async-packed-macros](../../issues/closures-async-packed-macros.md) decides how closures are checked.
43. **Closures that take proofs are not passed to the standard library's adapters, for now.** A specification that a function calls a closure only with values it was given, keeping their logical contents, would admit `Option::map` and the iterator adapters. It belongs to [standard-library](../../issues/standard-library.md), and is a priority, because carrying proofs through ordinary iterator code is much of what makes Locus usable. Since decision 82 it would be a function of Locus's library, not a specification of the standard library's.

**Decided by the owner after the fourth review.** The review found three ways to prove `False`, one accepted fact that was false at run time, and two missing rules. The owner asked for a design that is coherent rather than patched, and left the choices to the review, preferring a rule that rejects too much to one that proves too much.

44. **One reading.** A verified function means a pure function of values, and every rule is held against that reading ([What a verified program means](#what-a-verified-program-means)). What it cannot express is rejected or opaque. A model of the logical layer, with its soundness argument, is [soundness-model](../../issues/soundness-model.md).
45. **Unverified code is sound Rust** (`assumed.unsafe.sound`): it uses a value only as its type allows, and only while it owns the value or holds the borrow. The trust model had listed only Rust's aliasing rules, which do not freeze a `Vec`'s buffer behind a `&Vec`, and which let a dependency keep a pointer into a buffer it returned. So lending a reference to safe code is not an escape, and a place escapes only through a raw pointer. A verified function's summary says what it may write, whether it lets an address escape, and whether it may run unverified code or an `unsafe` operation whose effect is unknown. Decision 54 removes the last two: no place of verified code reaches a raw pointer, so a summary records only writes.
46. **`final!` appears only in a signature's result,** so no ghost value is computed from a value the program has yet to produce.
47. **A proposition under interior mutability reads only what is fixed where its type is written.** In the type of a field or of a tuple component, it reads no other field and no other component. A cell can be seen through two shared references, each with the type it was given, so its type must not change while it exists. The review first proposed allowing no forgeable proof under interior mutability at all, and the owner chose this narrower rule. It adds a condition to decision 9 and replaces decision 21.
48. **Snapshots are opaque by default,** and structural only where shown finite and strictly positive. A field typed by an associated type of its type's parameter is opaque, and so is a union.
49. **The same type means the same propositions,** with no conversion between equivalent ones until RFC 0003. Settled since by [RFC 0005](0005-core-terms.md): two propositions are the same when they convert to the same core term.
50. **A tuple whose later component reads an earlier one instantiates a type parameter only whole.**
51. **An `open!` block opens an invariant** ([opening-invariants](../../issues/opening-invariants.md)). Inside it, the fields that the value's proof fields read may be borrowed mutably and assigned without proof, and nothing observes the value. The invariant is proved again at the end of the block, and a panic that would unwind out of it aborts. The owner chose this over reasoning about unwinding with an outcome in every specification, and accepted the change to commitment 2 of the vision.
52. **Unverified code is taken to be sound crate by crate.** The standard library always is; a crate with no `unsafe` code, and none beneath it, needs no assumption; any other crate is trusted only when the project names it, and one that is not named is treated as foreign code. With decision 54, "treated as foreign code" means that a function that calls into such a crate is unverified, which is still for the owner to confirm (item 11 below).
53. **`unsafe` code in a verified crate is listed** in the manifest of each result that passes through it, as an assumed specification is. With decision 54 what is listed is each unverified function that contains `unsafe` code, and each `unsafe impl`.

**Decided by the owner on 2026-11-09,** after stepping back from the fourth review's repairs to ask whether the approach to `unsafe` code needed to change. The owner wanted a model in which nothing false can be proved, even if some true things cannot be proved yet.

54. **A function is verified whole, or not at all.** A function that contains `unsafe` code or handles a raw pointer is unverified, and so is one marked `#[unverified]`. The mark is optional where `unsafe` already shows it, so Rust code still compiles unchanged. A verified function contains neither. This replaces verifying the code around an `unsafe` block, and with it escape, the list of operations that make values from raw memory, and `assumed.rust.aliasing`.
55. **An unverified function is plain Rust.** It takes and returns no proof and holds no logical code. Verified code knows it by its Rust type alone, as it knows a function of a Rust crate.
56. **No program assumes a fact, for now.** There is no `assume!`, and an unverified function has no contract; a fact about what unverified code returns comes from a runtime check in verified code. The owner expects to do without preconditions and postconditions on unverified code for a while. When a form for them comes, stating an assumption will be marked as `unsafe` is ([boundary-contracts](../../issues/boundary-contracts.md)). Locus's library keeps its trusted specifications of the standard library. Decision 80 gives programs a way to assume, marked as this decision said it must be, and decision 81 ends the library's specifications.
57. **The words are *verified* and *unverified*,** for code, functions, and crates, in place of checked, unchecked, and opaque.
58. **An unverified function takes no type that holds logical data, at any depth,** through type arguments and through the fields of the program's own types, whether or not the data is completable. The owner confirmed this when it stood as a proposal about invariant types, and widened it to every logical type.
59. **Locus's own library will be verified Locus on permissioned pointers.** It will provide permissioned pointers and build `Box`, `Vec`, and the other collections on them, with no `unsafe` code of their own. Rust's standard library stays available, without proofs. The design is the standard library's RFC's, and [permissioned-pointers](../../issues/permissioned-pointers.md) records what a spike found it needs. Decision 82 narrows this for `Box`, which stays Rust's: Locus's library builds none of its own.
60. **No union field holds a logical type.** Since decision 58 no code could read such a union, so the allowance for completable data (decisions 19 and 41) is withdrawn.

**Decided by the owner on 2026-11-16,** in a session on how Locus should be built. The owner wanted a boundary under which nothing false can be proved, with a rule short enough to hold in mind, and was willing to change earlier decisions to get it.

61. **Unverified code handles only plain data.** In the owner's words about verified functions, "only these fns can take/return/use logical data of any kind". After instantiation, no unverified code has a value, a type argument, a capture, or a callee whose type holds logical data. The one exception was a library operation that Locus's library specified, which decision 81 replaces with Locus's own library. This replaces completability, the three kinds of protected value, and the analysis of what access a type grants: the rules `logic.boundary.completable`, `logic.boundary.payload`, `logic.boundary.access`, `logic.boundary.writes`, and `logic.ghost.forgotten` are withdrawn.
62. **A type with proof fields holds logical data.** The owner chose the strict reading at first, under which a value of one stayed within verified code. Decision 73 replaces that with sealed types.
63. **The boundary is checked for each instantiation.** A verified generic function's interface records which of its type parameters reach unverified code.
64. **Every precondition is a proof parameter, and a plain signature is the way in.** There is no `requires` clause. Unverified code calls a verified function exactly when its instantiated signature holds no logical data, so a function that turns a runtime check into a proof needs no mark. Two calls of one function are unrelated unless a proof relates them.
65. **A function's kind is inferred, and two attributes state it.** A signature that holds logical data requires verification. Otherwise `unsafe` code or an expression of raw-pointer type makes the function unverified, and every other function is verified. `#[verified]` and `#[unverified]` select a kind and never waive a rule, and failing to verify is an error, never a fallback. Obligations come only from logical code, so Rust code compiles unchanged. There is no default for a module or a crate for now: "we will just use these inference rules and per function coloring".
66. **A field that a proof reads changes only when the whole value is replaced,** or taken apart and built again. It is never assigned and never mutably borrowed, whoever would receive the borrow. A proof reads the fields its proposition mentions, through owned data, a `Box`, and shared references only, so a struct with proof fields is a dependent record and a value of one is valid at every point. This withdraws the assignment that proves the invariant again (decision 14, with decision 42 for closures), and settles item 8 of the list below as it stood.
67. **No `open!` for now.** "We will not support temporarily breaking an invariant while retaining the object's certified identity." This withdraws decision 51 and the rule `contracts.invariants.open`, and the vision's commitment 2 loses the exception it had gained. [opening-invariants](../../issues/opening-invariants.md) stays open.
68. **An item that holds logical data is visible only to crates that Locus verifies:** a function by its signature, a type by its fields, private ones included, and a static or a constant by its type. Decision 73 has since let a sealed struct be visible, when its logical fields and the fields its proofs read are not. The owner put it as a limit on exporting: public code that contains logical data is not exported to Rust users, while "it should be fine … to use pub code handling logical data in a native locus user". Export mode and the first stage of RFC 0001 follow it.
69. **There are two kinds of logical data, proofs and logical values.** A permission, when Locus's library has them, is "a private ZST with logical data inside": a runtime struct that is not `Copy`, not a third kind.
70. **`===` is the logic's equality, and the only one a proof rewrites with.** `==` in runtime code is a call that gives a `bool`. Decision 49 still says when two types are the same.
71. **Planned next: permissioned pointers, and a claim that a function cannot panic.** Locus's own library on permissioned pointers moves from a direction to planned work. `#[no(panic)]` joins the plan as an attribute for one claim, not an effect system ([no-panic-claims](../../issues/no-panic-claims.md)). How proofs are written, which the owner wants to read like ordinary Rust, stays with RFC 0003.
72. **Locus is written in `locus!` blocks first, with its full syntax inside them** (RFC 0001). `.lc` files come later, so that the macro is not needed for the syntax.

**Decided by the owner on 2026-11-16,** after seeing what decision 62 cost.

73. **Unverified code may hold a value of a sealed type, and can neither build one nor open it.** Asked whether a value with an invariant could be let into unverified hands safely, the owner said: "can we make it failproof while offering such a relaxation? If so, let's do it". A sealed type is a struct or enum of a verified crate that holds logical data only in the declared types of its own fields. It exposes no logical data, so the boundary's rule, decision 61, now speaks of what a type exposes. Three conditions hold it up: other crates reach no field that holds logical data or that a proof reads; the crate's own unverified functions still name no type that holds logical data; and `unsafe` code keeps such values intact, which is the assumption `assumed.unsafe.invariants`, kept from the previous revision.
74. **`assumed.platform` joins the trust model,** for the system beneath the standard library. The owner: "sure, can't hurt I guess?"
75. **Locus's own collections keep the layout of the standard library's,** so that a value passes between the two without being copied. The owner: "we should add this restriction regardless to keep zero copy possible". Locus's `Box`, `Vec`, and `String` have the layout and the allocator of Rust's, and convert to and from them in constant time. A collection that the standard library gives no way to take apart, such as a hash map, is outside this until it does. The design is the standard library's RFC's. Decision 82 narrows this for `Box`, which stays Rust's: there is no `Box` of Locus's to convert.
76. **Unverified code may be lent exposed logical data by shared reference, when it cannot change it.** Told that the standard library's derived `Debug` was rejected on a type with a proof field, the owner said: "This feels like a bad limitation - we should find a way to fix it I guess?", and agreed to the rule proposed in reply. A parameter of an unverified function may be a shared reference to exposed logical data that lies in the structural part of its value's snapshot, also as a trait object; the function's result and its other parameters still expose none. So a derived `Debug` prints a value with a proof field, and shows the field as `Erased`.
77. **The boundary is judged at signatures.** The owner asked why Locus does not check a standard library function such as `to_string`: "When locus compiles, won't it compile it all together?" Rust compiles each crate by itself, and the standard library arrives compiled. Locus could still follow a type parameter through the generic bodies it can see; the owner agreed not to, for now, for the reasons under [Alternatives](#alternatives).
78. **A specification is written in one form, by programs as by Locus's library, and the form says that it is assumed.** The owner: "The syntax for attaching post conditions should not be special cased to stdlib and should be available for user for their own code/crates too. And it should explicitly mention assume or unsafe or something like that." This binds the form that [boundary-contracts](../../issues/boundary-contracts.md) will design. Decision 56's rule stands until then, so no program in this RFC assumes a fact. Replaced by decision 80.
79. **A specification states postconditions only.** Asked why a specification should not state a precondition, the reply proposed allowing one on an `unsafe fn`, where it is the safety contract, and the owner decided against it: "I'm okay with no preconditions - let's keep it simple like that." So a specification says what is assumed of a function's result and obliges no caller, verified code still calls no `unsafe fn` (decision 65), and a proof does not yet remove a check. The form, and who sees a specification, are being designed in [boundary-contracts](../../issues/boundary-contracts.md). Replaced by decision 80, under which a precondition is a proof parameter of the function that states the contract.
80. **A program assumes a fact with `unsafe assume!(_)`, and a contract is always a signature.** The owner turned down a postcondition that would only be in scope after a call: "I don't like just assuming facts in scope - prefer these to become native proofs in returns (like the way we use extern in rust - we just assume it behaves in a certain way)". The owner then proposed the form: "why put unsafe assume as function decorator vs using 'unsafe assume!(_)' as the constructor for constructing the proof which can then be used anywhere, even in internal body of functions?" So nothing is attached to a function from outside it. A function states its contract in its signature, and where its body cannot prove a proof that it returns, it assumes it. This replaces decisions 78 and 79, and the part of decision 56 that left programs unable to assume anything.
81. **Locus's library reaches the standard library through types of its own, which a program names with an explicit import.** The owner: "What if we just did that by having locus stdblib exporting types or even modules and somehow played with library visibility rules that ensure for instance Option or Vec by default come from the right library depending on the right context?" And on how the context is chosen: "I'd prefer to use explicit `use locus::prelude::...` infact, maybe something even more explicitly suggesting that we are shadowing rust stdlib? `use locus::rust::std::prelude` or something like that?" The path proposed in reply, `locus::std`, mirrors the standard library's modules, and the owner accepted it with the rest ("Rest everything looks good"). This ends the trusted specifications of standard functions that decision 56 kept.
82. **A standard type is swapped only when it has private state.** The owner: "Re: Result/Option/Box etc - do we even need to swap rust stdlib versions?" Integers, references, tuples, arrays, slices, `Box`, `Option`, `Result`, and ranges stay Rust's, with built-in operations that the language defines. Collections, cells, and locks are Locus's own types around the standard library's. A standard method on a type that stays, when a program wants a fact from it, is a function of Locus's library. Everything else is called as plain Rust. The owner accepted this split as proposed. [RFC 0004](0004-runtime-code.md) has since made `checked_add`, `checked_sub`, and `checked_mul` operations that the language understands, so a fact from those three needs no function of Locus's library.
83. **Until its collections are verified, Locus's library is trusted to keep the values its types are given,** so that a `Vec` of Locus's may hold proofs from the start. Proposed in reply to the owner, and accepted with the rest.

**Decided by the owner on 2026-11-17,** in a session on how a value with an invariant is changed in place.

84. **A value with an invariant is changed in three ways, each of which replaces it whole:** a method that takes the value and returns it; a valid value that stands in for it while the new one is built; and a slot, which moves it out of a `&mut` and has a new one put back. The owner, shown these and a fourth: "I like options 1-3."
85. **Assigning the proof field again in place waits for `#[no(panic)]`.** The owner: "Maybe someday we will do option 4 too with no panic but not needed now".
86. **A slot is a guard that stays a local, and a panic that would unwind past one aborts the process.** Told that a guard which can leak is unsound, and offered a closure, the guard with a rule that it stays a local, or a block, the owner chose the guard. How the rule is stated is this RFC's.
87. **Locus's library has a `replace` whose result says that it returns what was there, and this RFC proposes the signatures of `Vec` that its examples need.** The owner, asked about each with decision 86: "yes to all".

**Decided by the owner on 2026-11-21,** in a session on the gaps left at the boundary.

88. **Only plain data crosses the foreign boundary.** Told that a function that C can call could take a sealed type, which C could fill with any bytes, the owner said: "shouldn't be possible for foreign code to work with logical data i.e. we need some sort of boundary. Maybe all extern functions are unverified and the same rules apply?" The reply proposed one rule at every point where a value crosses: no type that holds logical data, sealed or not, in the signature of a function declared in an `extern` block, of a function with a body that foreign code can call, or of a function pointer type whose ABI is not Rust's, and none in a static that foreign code can reach. A function that foreign code can call is verified or unverified as any other is, so that a safe shim may call a verified constructor. The trust model gains an assumption about foreign code, `assumed.foreign`, and tables of handles for long-lived objects are left to the library. The owner: "On extern, agree with your view."

**Confirmed tentatively by the owner on 2026-11-22.** A review of this RFC had left six rules unsettled. They were put to the owner as a table, one line each, and the owner said: "tentatively confirm the 6 open questions". Each is written into the text, the rules, and the examples, and stays open to be revisited (item 26 below).

89. **A method that one of Locus's own types only forwards is the standard library's.** The table's line: a method reached through `Deref` is the standard library's, so it is rejected at an element type that exposes logical data, and each wrapper type lists its own methods. Confirmed tentatively. The lists start from what this RFC's examples call.
90. **A function's result lists the assumptions in its own proof and in the functions it calls.** It is a theorem about valid arguments, so it does not list an assumption made where a value it is given was built. Only the program's list, of every assumption in every crate it links, is promised complete. Confirmed tentatively.
91. **An assignment to a field that a proof reads is rejected even when the proof could be proved again.** This is decision 66, which the owner's recorded words did not state: those were about not leaving an invariant broken for a time, and an assignment that proves the invariant for the new value never breaks it. Decisions 84 to 87, on the three ways to change such a value, build on the rule without stating it either. The owner now confirms it, tentatively. No rule and no example changes.
92. **What an exported crate shows may hold a sealed type wherever it exposes no logical data.** The table's line: a public struct with a public field of a sealed type is allowed, since the rule says "exposes" throughout. Its clauses had said "holds", which refused such a field while accepting an `Option` of the same type. Confirmed tentatively.
93. **`unsafe assume!` keeps its spelling, and a lint forbids assumptions in a crate.** The lowered program holds no `unsafe` for other tools to find, so the lint stands beside the list of assumptions in each result. Confirmed tentatively.
94. **A generic function's interface records more than one bit for each type parameter:** whether its values never reach unverified code, are only lent to it, or reach it otherwise, with enough of the signature to check a loan again at the caller's type. Confirmed tentatively.

**Confirmed tentatively by the owner on 2026-11-22,** on the crossing points that decision 88's table did not name. Two recommendations were put to the owner, each quoted below, and the owner's reply was to commit them with the six above: "we will try to commit all these changes including six boundary confirmations and extern rules". This RFC records them as it records the six. Each is written into the text, the rules, and the trust model, and stays open to be revisited (item 28 below).

95. **A function that foreign code can call counts as `unsafe` code when Locus records which unverified crates contain some.** The recommendation: "In edition 2021, an unverified crate can put `#[no_mangle]` on a function that takes another crate's sealed type, with no `unsafe` anywhere. C can then call it with any bytes, and the crate is not listed among the assumptions. I recommend counting a function that foreign code can call as `unsafe` code when Locus records which unverified crates contain some." Confirmed tentatively.
96. **A function named by a `sym` operand is one that foreign code can call, and a naked function is unverified.** The recommendation: "Two smaller crossing points are also unwritten: `global_asm!` naming a Rust function through `sym`, and naked functions. I recommend treating a function named by `sym` as one foreign code can call, and a naked function as unverified." Confirmed tentatively.

**Still for the owner,** in order of what they block. Each is already written into the spec and the examples as proposed, so accepting this RFC accepts it, and changing one changes its rule and examples:
1. **Versions,** following the owner's direction toward SSA: every write makes a new version, even of the same value, and a proof never carries over to another version without a new proof.
2. **One model per type,** declared by the type's own crate or by Locus's library.
3. **Places keep their Rust meaning in logical code,** and models apply to values.
4. **The program's own generic code is checked at each instantiation with logical types,** until [traits-on-logical-types](../../issues/traits-on-logical-types.md) allows checking it once.
5. **Syntax:** `===`, `==>`, `forall |x: T|`, `Arm(witnesses) @ proof`, `#[decreases(e)]`, and `final!`.
6. **No logical values in trait implementations,** apart from `impl Model`, until [traits-on-logical-types](../../issues/traits-on-logical-types.md); and `Model` bounds only on functions.
7. **Tuples keep their shape,** with markers for logical components, even when every component is logical, rather than losing them.
8. **A hand-written `unsafe impl` on a type with proof fields** stays allowed, as the programmer's promise, listed in each result that relies on it. Since decision 73 generic `unsafe` code can be instantiated with a sealed type, so a false promise, such as a `Zeroable` impl on a type whose invariant excludes zero, lets that code build a value that breaks the invariant. The alternative allows only `Send` and `Sync`. (The earlier item 8, the obligation at an assignment, is settled by decision 66.)
9. **Measures are `Int`s.**
10. **A runtime call as the argument of a proof parameter** stays "bind it first" in both modes. Build mode could allow `halve(n, checked_even(n))`, since its programs keep the argument; an exported program removes it. Keeping one rule means a crate that builds also exports.
11. **A function that calls into a crate with `unsafe` code that the project has not named is unverified.** This is what decision 52's "treated as foreign code" becomes without escape. The alternative takes every crate in the build to be sound, and has each result list the unnamed ones it relied on. Since decision 65 a function that states a proof cannot become unverified, so under this item such a call would be an error there.
12. **The marks are attributes,** `#[verified]` and `#[unverified]`, and not keywords.
13. **Which types Locus's library provides in place of the standard library's,** and which methods of each carry a contract. This RFC's examples assume `Vec`, `Cell`, `Rc`, and `Mutex` under `locus::std`, with the standard library's methods, which the litmus examples call only for what they do, and one method of Locus's own, `Vec::view`. Whether a method such as `len` or `push` should itself return a proof, as one example in [Proofs in data](#proofs-in-data) supposes of `push`, is for the standard library's RFC. Decision 89 lists, tentatively, the methods that are each type's own, starting from the ones the examples call.
14. **The conditions of decision 73.** The owner asked for the relaxation and for it to be failproof; the definition of a sealed type and its three conditions are this RFC's.
15. **The conditions of decision 76.** The owner asked for a derived `Debug` to work, and agreed to the loan. These are this RFC's: that what may be lent is decided from the snapshot; that a function's signature and a closure's captures are left out; that the function's result exposes nothing even when the caller discards it, so `black_box(&found)` stays rejected; and that a proof field prints as `Erased`.
16. **A function that assumes a fact stays verified.** The owner asked: "would adding 'unsafe assume' make the function unverified or can we keep it verified?" This RFC keeps it verified. `unsafe assume!` is a proof, not an `unsafe` block, and only its one proposition is trusted.
17. **The spelling `unsafe assume!(_)`.** The owner asked whether it should be "just 'assume' with assume implicitly being unsafe in this world". This RFC keeps `unsafe`: the owner's earlier ruling was that assuming is marked as `unsafe` is, and a reader who knows Rust and not Locus already searches for that word. The cost is one word at each assumption. Decision 93 keeps the spelling, tentatively, and adds a lint, since the word is not in the program that rustc compiles.
18. **Which built-in operations the language defines** on the types that stay Rust's. Decided since, in [RFC 0004](0004-runtime-code.md), whose table lists each with what it means.
19. **What an exported crate does with Locus's own types:** whether it depends on Locus's library as a Rust crate, or has them rewritten to the standard library's ([distribution-and-export](../../issues/distribution-and-export.md)). This RFC's export examples use none.
20. **How a slot's rule is stated** (decision 86): that every path that leaves the variable's scope has passed it to `put`, checked when the program is checked and not left to the abort; that no `.await` lies between the two calls; and that `put` is read as a write, where every other call is known by its signature.
21. **Three signatures of `Vec`, not six** (decision 87). The owner agreed to six, each returning its proof. This RFC proposes `push`, `set`, and `empty`, and keeps `new`, `len`, and `pop` in the standard library's shape: decided examples call `Vec::new()` and `len()` for their values, and one rejected example rests on `Vec::new()` giving no fact. `insert` waits for the example that needs it.
22. **The proofs of `replace` and `take_out` are stated on snapshots,** so that they hold for a type without a model. The three signatures of `Vec` are written on models, as `view` is, and so need an element type that has one. Which of the two a signature of Locus's library is stated on is the standard library's RFC's. It was put to the owner, and is an open question (item 27).
23. **What the three ways do not cover:** a `&mut` returned into a field that a proof reads, and a type with both an invariant and a destructor.
24. **The edges of decision 88,** which the owner saw as a table and not one by one:
    - The rule is checked after type parameters are instantiated, as the rest of the boundary is.
    - An ABI that is not Rust's is enough to make a function one that foreign code can call, with no `#[no_mangle]`, since such a function exists to be handed out as a pointer.
    - The `unsafe` of `#[unsafe(no_mangle)]` and `#[unsafe(export_name)]` does not make a function unverified. Decision 88 needs this: an unverified shim could not call the constructor.
    - The type of a function's result is held to the rule as a parameter's is, though a sealed value returned by value does no harm by itself: foreign code can do nothing with it but hand it back, which is refused where it comes back.
    - `assumed.foreign` also asks foreign code to keep the invariant of a sealed value that a crate Locus does not verify hands it. A Locus crate hands it none.
    - A manifest also lists the foreign functions declared `safe` that verified code calls, since no `unsafe` block marks such a call.
25. **Crossing points that are still not written.** Decisions 95 and 96 write the three that this item listed: a function that a `sym` operand names, a naked function, and whether a function that foreign code can call counts as `unsafe` code in an unverified crate. What they leave:
    - **A `global_asm!` that names nothing.** Its assembly is foreign code, under `assumed.foreign`, and no result lists it: a result lists the functions that foreign code can call, and not the assembly that calls them. Whether an unverified crate that has one is recorded as containing `unsafe` code is not written.
    - **What else the record of `unsafe` code counts.** Beyond decision 95, nothing says what it is for a crate to contain `unsafe` code. rustc's lint `unsafe_code` cannot be the definition: it reports `#[no_mangle]`, `#[export_name]`, `global_asm!`, and an `unsafe extern` block, and reports neither a function whose ABI is not Rust's nor a naked function (tested with rustc 1.97).
    - **Other attributes that place an item where code outside the program finds it,** such as `#[link_section]`, have not been examined.
26. **Decisions 89 to 94 are open to revisit.** The owner confirmed them tentatively, from a table of one line each. What this RFC chose in writing them, which the owner has not seen:
    - **The lists of decision 89.** `pop` and `clear` are among `Vec`'s own, since decision 87 names the first and an example's stated error needs the second. The iterators that `iter`, `iter_mut`, and iteration return would have to be types of Locus's own, since the standard library's `next` is a forwarded method too.
    - **The standard library's type at an element type that exposes logical data may be named and borrowed** in verified code, as `&*v` does. Only the functions that are given it are judged.
    - **Whether a forwarded method that only borrows, such as `capacity`, is a loan** at such an element type is not pinned by an example. `logic.boundary.loans` lends nothing that lies behind a raw pointer, which is where the standard library's `Vec` keeps its elements, while the snapshot that Locus's library declares for it is a sequence.
    - **A value read from a static is taken to be valid,** as an argument is (decision 90), so a function's result does not list what the static's initializer assumed. The program's result does.
    - **No example can show what a result lists** (decision 90). The format of the examples states whether a program is accepted and what it returns, and nothing about a result's list.
    - **One more rule changes a word for decision 92:** `contracts.invariants.observed` said that unverified code cannot reach the fields that "hold" logical data. And other crates may assign a public field of a sealed type, with another value of that type.
    - **A type that holds logical data only inside fields of sealed types is itself sealed, by the definition,** and under decision 92 other crates may build one and take it apart when those fields are public. So "unverified code can neither build a sealed type nor open it", which are decision 73's words, is exact only for a type that has a field which exposes logical data, such as a proof field. `export.invariants-closed` now says it that way.
    - **The lint of decision 93 is named `locus::assumptions`,** follows Rust's lint levels and scopes, is allowed by default, and is about the code it is set on only. How the attribute is kept from rustc, which does not know the name, is RFC 0001's.
    - **What decision 94 records for a loan** is the type of the value lent, written in the function's own type parameters. Its part across crates has no example, since an example is one file ([multi-file-litmus](../../issues/multi-file-litmus.md)); the two added show the same check within one crate.
27. **What the contracts of Locus's library are stated on is an open question.** `Vec::view` and the three signatures of decision 87 are written on models with no `Model` bound, which `models.no-default` does not allow (item 22). Two ways to close that gap were put to the owner. One states the contracts of the library's generic methods on snapshots, with no `Model` bound, and reads `v.len()` in logical code on a runtime array, slice, or `Vec` as the length of its snapshot, for every element type. The other keeps contracts on models, with a `Model` bound on each method. The first was recommended, and the owner said: "Re: library contracts - I'm not following but will defer to you." This RFC records no decision from that reply, because the recommendation cannot be written as it was put: two decided examples stand in its way, and one rule is missing.
    - `models/vector-without-element-model.reject.lc` rejects `points.len()` on a `Vec<Point>` whose `Point` has no model. With `v.len()` the length of the snapshot, that program is accepted. Changing a decided outcome needs the owner's word for that example.
    - `models/derived-model-keeps-proof-holder.accept.lc` proves that two vectors have one length from the equality of their models. With `v.len()` the length of the snapshot, that follows only from a rule that a library container's model has its snapshot's length. A hole calls no lemma, so the rule would have to be a step of conversion. No RFC has it, and [RFC 0005](0005-core-terms.md) leaves it to the library.
    - Two of the three examples of a stack, `contracts/stack-push-with-stand-in.accept.lc` and `contracts/stack-push-through-slot.accept.lc`, state their postcondition on `Stack`'s model, a `Seq<Nat>` taken from its vector's model. Once `push`'s proof speaks of snapshots they need the same rule.
    - Stating only the signatures on snapshots, and leaving `v.len()` on models, would leave every example with an invariant such as `items.len() <= cap` without the step from one length to the other.

    The ways out, and what each changes in the decided examples:
    - **a.** Add one step to conversion, that `len` of a library container's model is `len` of its snapshot. Then the recommendation can be written. `models/vector-without-element-model.reject.lc` becomes accepted, and the three accepted examples above stand as they are, by the new step.
    - **b.** Keep contracts on models, with a `Model` bound on each method. This is the alternative the owner was offered. None of the four examples above changes. `push`, `set`, `empty`, and `view` then need an element type that has a model, and whether every accepted example that calls one of them does so at such a type has not been checked.
    - **c.** State contracts on snapshots, and have a program call a lemma that takes one length to the other. The programs of the three accepted examples above change, each to call it. The rejected one becomes accepted if `v.len()` is then read as the snapshot's length, and stays as it is otherwise.

    Nothing is written for any of them, so the gap is still open: `Vec::view` and the three signatures stand as they were. The question is to be decided with the design of traits ([traits-on-logical-types](../../issues/traits-on-logical-types.md)), where what a contract on a snapshot can say is being worked out. That was the advice given to the owner after the reply above.
28. **Decisions 95 and 96 are open to revisit.** The owner saw the two recommendations quoted there, and not what this RFC chose in writing them:
    - **Each of the three macros counts** (decision 96). The recommendation named `global_asm!`. A `sym` operand of `asm!` or of `naked_asm!` gives a function to assembly in the same way, so `logic.boundary.foreign` names all three.
    - **A static that a `sym` operand names** is held to `logic.code.statics`, as a `#[no_mangle]` static is. The recommendation spoke of functions.
    - **The error is reported at the `sym` operand,** which is where the function or the static is given to assembly. Four examples pin that line.
    - **Being named by a `sym` operand does not make a function unverified,** as `#[no_mangle]` does not (item 24).
    - **Which assumption covers the assembly.** The assembly of a `global_asm!` is foreign code, under `assumed.foreign`, whose text now says so. The assembly of an `asm!`, and of a naked function's body, is `unsafe` code of its function, under `assumed.unsafe.sound`. No entry is added to the trust model.
    - **`logic.verified.restrictions` changes too:** a verified function is not a naked function. Without that clause, a naked function whose signature holds logical data, or one marked `#[verified]`, would be verified and would break no rule.
    - **A naked function is already one that foreign code can call,** for as long as rustc accepts `#[unsafe(naked)]` only with an ABI that is not Rust's (tested with rustc 1.97). So `logic.boundary.foreign` refuses the example of a naked function that takes a sealed type as well, and only the example of one marked `#[verified]` rests on the new clause alone.
    - **Decision 95 counts every function that foreign code can call,** whatever its signature, and not only one that takes a sealed type. It also counts a static that foreign code can reach, and a naked function. The recommendation spoke of a function. This is the reading under which fewer crates are taken to be sound without being named.
    - **What the record changes, and what it does not.** A crate so recorded is taken to be sound only when the project names it, and a function that calls into it while it is not named is unverified. A crate that no function of the program calls into, and whose function only foreign code calls, is named by no result on that account: its function is among those a result lists for `assumed.foreign`, as it was before decision 95. And in export mode Locus does not compile the crates that use the exported one, so nothing records them, and that case rests on `assumed.foreign` and `assumed.unsafe.invariants`, as before.
    - **Decision 95 has no rule and no example.** It is about what the driver records and what a result lists, which an example cannot state, and it takes two crates ([multi-file-litmus](../../issues/multi-file-litmus.md)). It is written in this RFC, in RFC 0001 as its decision 48, and in the trust model.
    - **The assembly in the examples is x86-64's,** and no `run` line executes it.

**Left to later work,** each with its issue:
- The rest of the runtime semantics, after [RFC 0004](0004-runtime-code.md): `&mut` parameters and reborrows, where borrows end, partial moves, and drops: [runtime-semantics](../../issues/runtime-semantics.md).
- Remaining syntax questions: [proposition-syntax](../../issues/proposition-syntax.md).
- Distribution, including how an exported package is laid out: [distribution-and-export](../../issues/distribution-and-export.md).
- Closures and async code, including closures that take or return proofs, closures that capture a reference through which a place holding logical values can be written, and assignments inside closures that need proofs: [closures-async-packed-macros](../../issues/closures-async-packed-macros.md).
- Locus's standard library: logical types, indexing of sequences, `div_euclid` and `rem_euclid`, models of standard types, its own collection, cell, and lock types and their contracts, and functions that carry proofs through closures as `Option::map` and the iterator adapters do: [standard-library](../../issues/standard-library.md).
- Traits and operators on logical types, generic predicates such as a `sorted` that applies to any ordered type, traits with logical values or contracts, programmable comparisons with their laws and decidable equality, and checking generic code once for logical instantiations: [traits-on-logical-types](../../issues/traits-on-logical-types.md).
- How proofs are constructed: using definitions, rewriting, procedures, and what a hole tries: [RFC 0003](0003-proof-construction.md), [proof-construction](../../issues/proof-construction.md).
- Storing proofs that were found, and what proof search may do: [proof-search-and-storage](../../issues/proof-search-and-storage.md).
- Claims that runtime functions terminate: [termination-claims](../../issues/termination-claims.md).
- How the kernel provides `Int`: the kernel RFC.
- Mutable references stored in data, whose model carries a current and a final value.
- A model of the logical layer, with the argument that its rules are sound: [soundness-model](../../issues/soundness-model.md).
- Assigning a proof field again in place, once a function can be claimed not to panic, and lending out a `&mut` into a field that a proof reads: [opening-invariants](../../issues/opening-invariants.md).
- What is still awkward at the boundary: [opaque-certified-values](../../issues/opaque-certified-values.md).
- A claim that a function cannot panic, `#[no(panic)]`, and leaving out the checks it makes unnecessary: [no-panic-claims](../../issues/no-panic-claims.md).
- A default kind for a module or a crate, and lints over the two marks.
- Contracts at the boundary: preconditions and postconditions for unverified functions and for other crates' functions, and how stating one is marked: [boundary-contracts](../../issues/boundary-contracts.md).
- Permissioned pointers, and Locus's own collections built on them: [permissioned-pointers](../../issues/permissioned-pointers.md).
