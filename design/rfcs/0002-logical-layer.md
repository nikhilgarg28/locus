# RFC 0002: The logical layer

- **Status:** proposed
- **Issue:** [#4](https://github.com/nikhilgarg28/locus/issues/4), and parts of [#5](https://github.com/nikhilgarg28/locus/issues/5) and [#6](https://github.com/nikhilgarg28/locus/issues/6)
- **Supersedes:** none

## Summary

This RFC defines what Locus adds to Rust:
- logical types, propositions, and proofs;
- logic functions and logic blocks;
- models, which say what runtime data means in the logic;
- versions, which say which values a proposition is about;
- contracts on runtime functions, including what a `&mut` parameter means in one.

The design in brief:
- **Logical types** describe values that exist only for checking:
  - `Int`, the language's one built-in data type;
  - propositions, of type `Prop`, and proofs of them, of type `@P`;
  - logical functions;
  - data declared with `#[derive(Logical)]`.

  Locus's library builds `Nat`, `Bool`, `Seq`, and the rest from these.
- **Proofs and logical values are ghost.** They have no runtime representation, and Locus tracks them itself.
  - At run time each is a zero-sized marker: `Erased` for a proof, and `LogicalData` for any other logical value. So `Vec<Nat>` is a `Vec<LogicalData>` at run time, and keeps its length. An exported program removes ghost parameters and locals altogether ([RFC 0001](0001-compiler-pipeline.md)).
  - The two markers implement different traits, so rustc itself rejects `==` on two `Vec<Nat>`s, and still accepts it on values that hold proofs, which are all equal.
  - Locus believes what a value holds only through code it checked, or through standard library code whose specification it has. Everywhere else ghost contents are forgotten, so no Rust code can forge or convert a proof.
  - Contracts are visible only to code that Locus checks. In build mode, every crate that uses a Locus crate is checked. In export mode, what other crates can reach is plain Rust, with no logical values and no contracts.
- **Logical code is pure and total.** This covers logic functions, `logic!` blocks, propositions, and proofs. It computes only with logical values, reads runtime values through their **models**, and never calls runtime code.
- **Snapshots and models.** Every runtime value has a **snapshot**, `Snap<T>`, which is what it is, field by field. A type may also have a **model**, which is what it means: declared by hand, derived with `#[derive(Model)]` from its fields' models, or declared by Locus's library. There is no default model. Logical code reads places as snapshots, uses values through their models, and writes `snap(x)` to compare whole values. Equal models never imply equal snapshots.
- **A proposition is about versions.** Every write to a place makes a new version of it, as in SSA. A proposition is about the versions it read, so a proof never silently carries over to a later value.
- **A runtime function states its contract in its signature.** Proof parameters are preconditions, and proofs in the result are postconditions.
  - In a signature, every parameter means its value at entry.
  - For a `&mut` parameter `x`, `final!(x)` is the value its referent has when the borrow ends. This is a prophecy in the sense of RustHorn and Creusot.
- **Types can carry proofs** as invariants of the program's own types, and inside any runtime type, such as `Option<@P>` or `Vec<(i: usize, @(i < n))>`.

## Motivation

[RFC 0001](0001-compiler-pipeline.md) decides how Locus checks Rust and erases its own additions, but not what those additions mean. Three issues wait on a common core:
- logical types and models ([#4](https://github.com/nikhilgarg28/locus/issues/4));
- syntax ([#5](https://github.com/nikhilgarg28/locus/issues/5));
- runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)).

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

At run time both proof types would be the same Rust type, so the downcast would succeed, and Locus would believe the result because of its type. This RFC removes the belief rather than the type. Locus tracks proofs itself, and keeps track of what a value holds only through code whose meaning it knows. Code like `downcast`, whose meaning it does not know, gives back a marker, and a marker proves nothing.

## Design

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
- the logic function `model`;
- the logical types `Int`, `Nat`, `Bool`, `Seq`, `Snap`, and the others of Locus's library, and the logic functions `snap` and `model`;
- the derive `Model`, beside the trait of that name;
- the propositions `True`, `False`, `And`, `Or`, and `Exists`.

**Naming.** By convention, logic functions are named in snake_case, as Rust's functions are, and `prop` declarations in CamelCase, as Rust's enums are. So a reader can tell how a proposition is used: a logic function by its definition, and a declared proposition by matching its arms.

How proofs are built beyond the forms this RFC fixes, including using a logic function's definition and rewriting along an equality, is for a third foundational RFC ([#15](https://github.com/nikhilgarg28/locus/issues/15)).

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
- `Snap<T>`, for each runtime type `T`: the type of `T`'s snapshots, whose values are exactly the values `T` can have ([Snapshots and models](#snapshots-and-models));
- the model types that `#[derive(Model)]` generates, one for each type it is derived for;
- the logical types of Locus's library, which it builds from these: `Nat`, `Bool`, `Seq<T>`, `Set<T>`, `Map<K, V>`, `Maybe<T>`, and others. This RFC uses them in examples, and assumes only what it says about them; their definitions and operations are for the RFC for Locus's standard library ([#11](https://github.com/nikhilgarg28/locus/issues/11)).

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
- Through a library type, a logical type may refer to itself only in the arguments that the library declares positive. `Seq`, `Maybe`, and tuples are positive in their arguments. A set, which is a predicate on its elements, is not ([#11](https://github.com/nikhilgarg28/locus/issues/11)).

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
- **Types with interior mutability may hold logical values.** Their contents are part of no version, no snapshot, and no model ([Snapshots and models](#snapshots-and-models)), so Locus knows only their types. That is enough for proofs, which are irrelevant: a proof taken out of a `Cell<Option<@P>>` is a proof of `P`, because every value of type `@P` is one. A `Nat` taken out of a `Cell<Nat>` is some `Nat`. [The boundary with unchecked code](#the-boundary-with-unchecked-code) keeps code that Locus does not check from writing them, and Locus's library specifies that the methods of `Cell`, `RefCell`, and `Mutex` keep their contents' types ([#11](https://github.com/nikhilgarg28/locus/issues/11)).

**Runtime code runs as written.** Erasure removes logical code and nothing else.
- A `let`, block, `if`, or `match` whose value is ghost keeps its runtime calls, conditions, and statements, and its patterns keep their runtime parts.
- So `let Some(p) = check(n) else { return 0 };` keeps the call and the early return, as `let Some(_) = check(n) else { return 0 };`.
- And `if n % 2 == 0 { _ } else { panic!("odd") }`, as a function's result of type `@P`, keeps its condition and its panic.
- A `const` or `static` of logical type is logical code, so its initializer never needs to run.

**Calls are checked, not guarded.** A function's proof parameters are its preconditions, and Locus checks the proof for each one at every call. No call escapes that check:
- within a crate, every file is checked, and a function whose parameters hold proofs never reaches code that Locus does not check ([The boundary with unchecked code](#the-boundary-with-unchecked-code));
- across crates, contracts are visible only to code that Locus checks: in build mode every crate that depends on a Locus crate is checked, and in export mode what other crates can reach states no contracts ([Export mode](#export-mode)).

**What survives where.** Locus tracks ghost contents itself: which proofs and logical values a runtime value holds. It tracks them only through code whose meaning it knows:
- code that Locus checks;
- standard library code whose specification Locus's library declares.

Everywhere else, ghost contents are forgotten:
- **Other generic code.**
  - Passing a value to a generic parameter of any other code instantiates it with the erased type, exactly as rustc does. What comes back holds markers, not proofs or logical values.
  - Turning such a value into a trait object of a trait that Locus has no specification for, such as `dyn Any`, forgets too.
  - This is why the `convert` example above is rejected: `downcast` gives back a `Box<Erased>`, and `Erased` is not a proof.
  - For now, the generic code of other crates is unspecified in this sense.
- **Raw pointers, casts, and `transmute`.** A value read through a raw pointer, or given another type by a pointer cast or by `transmute`, is forgotten the same way: it holds markers and no proofs. To rustc, `Option<@P>` and `Option<@Q>` are the same type, so a cast between them changes nothing that rustc could reject.
- **Specified code.**
  - Locus's library specifies at least `Option`, `Result`, `Box`, `Vec`, arrays, slices, and their iterators as code that keeps what its values hold, so `for (i, ok) in checked` keeps each element's proof.
  - Their exact specifications are [#11](https://github.com/nikhilgarg28/locus/issues/11).
  - The `?` operator on a value that holds a proof goes through the `Try` trait, and is for [#12](https://github.com/nikhilgarg28/locus/issues/12).
- **The program's own generic code** is checked again for each instantiation whose type arguments contain logical types.
  - If checking those instantiations would not end, as with polymorphic recursion, the program is rejected.
  - Checking such code once for every instantiation is [#12](https://github.com/nikhilgarg28/locus/issues/12).
- **Traits.**
  - Logical types implement `Clone`, `Copy`, and `Debug`, and nothing else, so `LogicalData` implements only these. rustc itself then rejects runtime code that would compare, order, or hash logical data: `==` on two `Vec<Nat>`s, which would compare only their lengths, or a derived `PartialEq` on a struct with a `Nat` field, which would ignore it.
  - Proof types also implement `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash`, and so does `Erased`. These are right for proofs, because any two proofs of a proposition are equal. So a derived `PartialEq` on a struct with proof fields works, and ignoring them is correct. That is why proofs and logical data have different markers.
  - No logical type implements a trait that makes a value from nothing, such as `Default`.
  - Locus understands the standard derives: a derived `Clone` of a type with logical data fields copies them too.

Further rules:
1. **Never a runtime decision.** Runtime `if`, `match`, `while`, and the other control flow never depend on a logical value. A match on an `Option<@P>` tests its runtime tag, not the proof.
2. **Arguments of ghost parameters are ghost values written as such.** An argument for a proof parameter, or for a parameter of logical type, is a ghost binding or logical code: a hole, a `logic!` block, a proof constructor, or a call of a logic function.
   - A runtime value is passed there as logical code, which takes it to its model: `grow(n, logic! { x })`, not `grow(n, x)`. Lowering never looks up a callee's signature, which for a method would need the receiver's type, so the source says it ([Logical code](#logical-code)).
   - A runtime function that returns a proof is called in a statement or `let` of its own first: `let even = checked_even(n); halve(n, even)`, not `halve(n, checked_even(n))`. An exported program removes the argument with its parameter, so the call could not stay where it was.
   - Everywhere else a ghost value has a runtime slot, its marker, and a runtime call that produces it stays where it is: as a logical field's initializer, a logical tuple component, a payload such as the `h` of `Some(h)`, or an argument instantiating a generic parameter with a logical type. `Interval { lo, hi, ordered: checked_order(lo, hi) }` keeps the call. In build mode the call returns the marker itself, and an exported program writes `ordered: match checked_order(lo, hi) { _ => Erased }`, so its order and its temporaries are as in the source.
   - Moving a call out of an argument list instead would change when its temporaries are dropped, or conflict with a borrow of the receiver, so erasure never does that.
3. **A call is kept even when its result is ghost.** A call to a runtime function whose whole result is ghost stays in the runtime program, as a statement.
4. **No logical values in trait implementations, for now.**
   - Apart from `impl Model`, no logical type appears in an `impl`'s self type, in its trait's arguments, or in its associated types, and no trait method's signature holds logical values.
   - Otherwise unchecked generic code could call such a method. With `impl From<Option<@P>> for Absurd`, generic code bounded by `From` could build an `Absurd` from any `Some(Erased)`.
   - Traits with logical values are [#12](https://github.com/nikhilgarg28/locus/issues/12).

**Deciding what is logical.** Before rustc runs, Locus decides only what the source shows ([RFC 0001](0001-compiler-pipeline.md#the-lowered-program)): Locus syntax, the names of Locus's macros and their aliases, `derive(Logical)`, `derive(Model)`, and `impl Model`, and which bindings the source shows to be ghost ([Logical code](#logical-code)). It never needs an expression's type or a callee's signature.
- Ghost values reach rustc as markers, so rustc infers and resolves the program as written: which method a call selects, which generic arguments it instantiates, and where markers flow.
- After rustc, Locus learns from its facts where each marker went, and works out which logical type it stands for, forward from signatures and annotations.
- Every assumption made before rustc is confirmed against the facts, by the identity of the item each name resolved to. A wrong one is an error or a second run, never a program with another meaning.
- A ghost value used in runtime code, such as a branch on a marker, is reported as Locus's error, such as "runtime code cannot branch on a logical value", with rustc's own error, if any, as a note.

### The boundary with unchecked code

*Unchecked code* is code that Locus neither checks nor has a specification for:
- the crate's plain-Rust dependencies, which are compiled as they are;
- standard library code that Locus's library does not specify, such as `black_box` and `Any::downcast`;
- code reached from either of these through trait objects or function pointers;
- in export mode, every crate that uses the exported crate.

A type holding logical values is *completable* when every value of its erased form is the erasure of one of its values. Unchecked code can do no harm with completable values, so the rules below concern the others.
- `Nat` and `Vec<Nat>` are completable: any vector of markers is the erasure of some vector of naturals.
- A proof type `@P` is not, and neither is `Option<@P>`, because `Some(Erased)` is the erasure of nothing unless `P` holds.
- **Locus decides completability conservatively, from types alone**, because the property itself asks whether types are inhabited, which no compiler can decide in general:
  - `Int`, and the logical types that Locus's library declares completable, such as `Nat`, `Bool`, and `Seq<T>` for a completable `T`, are;
  - a `#[derive(Logical)]` type is completable when one of its constructors has no proof fields and only completable fields, counting a recursive type as completable through a constructor that does not recur;
  - a runtime type, or a tuple, is completable when every logical type it holds is;
  - proof types and logical function types are not, even `@True`, and neither is anything else.
- **Lent mutably, a completable value comes back as any value of its type.** Locus forgets everything it knew about it, as it does for any place that opaque code can write (RFC 0001). So after `black_box(&mut counts)`, Locus knows that `counts` is a `Vec<Nat>`, and not its length.

Three rules hold at the boundary, the same in both modes. The modes differ only in which code is unchecked.
1. **What comes back is forgotten,** as [above](#ghost-values).
2. **Nothing calls a function that needs a proof without supplying one.** A function whose parameters hold logical values that are not completable is never coerced to a function pointer or turned into a trait object, and is passed to a generic parameter only of the program's own code, which is checked again at that instantiation. Nor is it an `extern`, `#[no_mangle]`, or `#[export_name]` function. So it never reaches unchecked code, or specified code such as `Option::map`.
   - Otherwise other code could call it without its proof: `proofs.into_iter().fold(10, halve)` would call `halve` with whatever `proofs` holds, which proves something else, and `let f: fn(u32, @(10 % 2 == 0)) -> u32 = halve;` would make a pointer that skips its precondition.
   - In checked code such a function may be bound to a local and called, as in `let f = halve; f(10, _)`, and each call is checked like any other.
3. **Unchecked code never gets a way to write a place that holds logical values that are not completable.** Locus decides this from a value's type and its type arguments, at any depth. Such a place is behind a `&mut` or `*mut`, or inside a type that Locus's library specifies as writable through a shared handle: `Cell`, `RefCell`, `Mutex`, `RwLock`, `OnceCell`, `OnceLock`, and the sending halves of channels, such as `mpsc::Sender`, through which one holder writes what another reads. `&`, `Rc`, and `Arc` share them. So `&mut found`, `&cell`, `Rc<Cell<Option<@P>>>`, and a `Sender<Option<@P>>` never reach unchecked code, and no raw borrow `&raw mut` of such a place is taken.
   - Otherwise unchecked code could write a marker it took from a proof of another proposition, through `Any`, into a place that Locus still trusts.
   - Locus needs this only for types it trusts. What comes back through a type that Locus's library does not specify is forgotten anyway, so such a type cannot carry a forged proof back, whatever its API does.
   - A closure, or a value of the program's own type, does not count for what it holds in fields whose types its own definition fixes: unchecked code can only call it, move it, or drop it, and the code that runs is checked. Its type arguments always count. A `&mut Bag<(usize, @P)>` would let unchecked code overwrite the whole `Bag`, through `Any`, with a `Bag<(usize, @Q)>`.
   - Reading is harmless: a shared reference to `Option<@P>` gives nothing to write through.

### Logical code

Logical code is:
- the bodies of logic functions and of proposition declarations;
- the contents of `logic! { ... }`, `prop!(...)`, and `prove!(...)`;
- propositions in types, written `@(...)`, and proof holes `_`;
- calls of logic functions, and proof constructors such as `And::Intro(h, k)`;
- an operator, a method call, or a field access whose operands are all logical code or ghost bindings that the source shows.

**Ghost bindings that the source shows.** Lowering decides what is logical code before rustc runs, from the source alone ([Deciding what is logical](#ghost-values)). So outside delimited logical code, a binding counts as ghost only when the source shows it:
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
- **Logical code never reads data that changes without a write it sees.** A snapshot holds no contents of an `UnsafeCell`, on which `Cell`, `RefCell`, `Mutex`, and the atomics are built, and no target of a raw pointer, which is not a place in logical code. So no logical value changes while Locus sees no write. Only the trusted snapshots and models of Locus's library, such as `Vec`'s, look through raw pointers.
- **Methods and indexing work on models.** A method call or index on a runtime value works on its model, never on a runtime method, as RFC 0001 requires. For `v: &Vec<u32>`, `v.len()` is `Seq::len` of `v`'s model, and `v[i]` indexes that model. A logic function declared on the runtime type itself is found first, then those of its model's type. On a snapshot, they work on the snapshot: `snap(v).len()` for a `Vec` of values without a model.
- **Indexing a sequence,** as in `xs[i]`, is a placeholder until the RFC for Locus's standard library ([#11](https://github.com/nikhilgarg28/locus/issues/11)) decides the index type and what an index outside the sequence means. Whatever it decides, indexing a sequence whose elements might have no values, such as a `Seq<@False>`, needs a proof that the index is in bounds, or indexing an empty one would prove `False`.
- **Constants and const generic parameters** may appear in logical code, through their models. A constant of logical type is logical code ([Ghost values](#ghost-values)).
- **References are transparent.** A `&T`, `&mut T`, or `Box<T>` used as a value is the current value of its target. For a `&mut` parameter in a signature, "current" means at entry; see [Contracts](#contracts).

**Logical expressions.**
- **Arithmetic is exact:** it never overflows.
  - `Int` has `+`, `-`, and `*`. Division and remainder truncate toward zero, as Rust's do, so a machine integer's division agrees with its model's. Dividing by zero gives zero, and the remainder of a division by zero is the dividend, as in Lean, so `a === (a / b) * b + a % b` always holds.
  - Lean's own `/` on integers is Euclidean instead: its remainder is never negative, so `-7 / 2` is `-4` there and `-3` in Rust. Locus's library provides that as `div_euclid` and `rem_euclid`, the names Rust gives it ([#11](https://github.com/nikhilgarg28/locus/issues/11)).
  - A `Nat` is used as an `Int` wherever an `Int` is expected.
  - `+`, `*`, `/`, and `%` on `Nat`s give a `Nat`, and `-` gives an `Int`. So for an empty `v`, `v.len() - 1` is `-1`.
  - Turning an `Int` into a `Nat` needs a proof that it is not negative.
  - An integer literal is an `Int`, or a `Nat` where a `Nat` is expected.
  - `as` in logical code converts between logical types, such as a `Nat` to an `Int`; a cast between machine integers is runtime code.
  - A machine integer's snapshot, and so its model, lies in its type's range, which proofs may use: for `x: u8`, `x <= 255`. For `usize` and `isize`, the range is that of the configuration being checked.
- **`===` is equality.** `a === b` is the proposition that `a` and `b` are equal: Lean's `Eq`, on two values of the same logical type. `a !== b` is its negation. It is built in, every logical type has it, and it cannot be overloaded.
  - On runtime values, `===` compares their models, and nothing else. Two runtime values whose models are equal need not be equal values, and their fields need not be equal: a hand-written model may forget fields entirely. So equality of models never gives equality of snapshots, or of fields read as places.
  - On snapshots, `===` is equality of values: two snapshots are equal exactly when they are the same variant with equal fields, and their opaque parts, such as a `Cell`'s, are equal only when they come from one version.
  - Any two proofs of one proposition are equal, so `h === k` always holds for them.
  - A type declared with `#[derive(Logical)]` gets the facts that its constructors are injective and distinct, as Lean's inductive types do.
- **`==`, `!=`, `<`, `<=`, `>`, and `>=` are comparisons, which give a `Bool`,** as Rust's give a `bool`.
  - For now they are built in only on `Int`, `Nat`, and `Bool`, where each agrees with the proposition it suggests: `a == b` is true exactly when `a === b`, and `a < b` exactly when `a` is less than `b`.
  - On other logical types, `==` waits for programmable comparisons, which are [#12](https://github.com/nikhilgarg28/locus/issues/12), together with the laws that tie a comparison to `===` (Lean's `LawfulBEq`) and decidable equality.
  - On runtime values they compare models, like every operator in logical code. Runtime `PartialEq` and `PartialOrd` are never called in logical code; how they relate to models is a specification ([#12](https://github.com/nikhilgarg28/locus/issues/12)).
- **A `Bool` used as a proposition means that it is true:** `b` stands for `b === true`. So `@(n % 2 == 0)` is the type of proofs that `n % 2 == 0` is true, which for `Int` is the same as `n % 2 === 0`.
- **Connectives and quantifiers:** `&&`, `||`, `!`, `==>`, `forall |x: T| P`, and `exists |x: T| P`, over logical types `T`. See [Propositions](#propositions).
- **`if` and `match` choose between logical values.** `if` accepts a `Bool`, or any proposition, because classical logic decides every proposition (Lean's `Classical.propDecidable`).
- **`let`** binds a logical value.
- **Closures**, such as `|x: Int| x + 1`, are values of logical function types.

**Logic functions** are declared as `logic fn name(parameters) -> T { body }`.
- **Every logical computation produces a logical value.** A logic function's result type is logical, and so is the value of every `logic!`, `prop!`, and `prove!`, every proposition, and every proof. So erasing logical code never removes a value that runtime code uses.
- Each parameter is logical. A parameter written with a runtime type `T`, such as `xs: &[u32]`, receives a value of that type, whose visible fields the body can read as places, and which is taken to `T`'s model where the body uses it as a value. Only declared logic functions take runtime values this way; a logical function type's parameters are logical types.
- The body is logical code. Logic functions never run, so they need not be computable.
- **They are total.** Every recursive call is on a structurally smaller part of a parameter, exposed by a `match`, or decreases a measure declared with `#[decreases(e)]`, where `e` is an `Int`. At each recursive call, the measure at the call's arguments must be nonnegative and smaller than at the parameters, which is a proof obligation. Mutual recursion is [#16](https://github.com/nikhilgarg28/locus/issues/16).
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

**Snapshots.** Every runtime type `T` has a snapshot type `Snap<T>`, a logical type whose values are exactly the values that `T` can have. Locus derives it from `T`'s definition, with no declaration:

| `T` | `Snap<T>` |
|---|---|
| a struct or enum | a logical struct or enum with the same fields and variants, each field of its field type's snapshot type, proof fields included |
| `u8` … `u128`, `usize`, and the signed integers | a bounded integer: a `Nat` or `Int` within the type's range |
| `bool`, a tuple | `Bool`, the tuple of its components' snapshots |
| `&T`, `&mut T`, `Box<T>` | `Snap<T>` itself: references are transparent, and a `&mut`'s final value is `final!` ([Contracts](#contracts)) |
| `[T; N]`, `[T]`, `Vec<T>` | `Seq<Snap<T>>`. Locus's library declares it for `Vec`, whose fields are raw pointers, and leaves out its capacity |
| `UnsafeCell<T>`, and so `Cell`, `RefCell`, `Mutex`, and the atomics | an opaque type: a snapshot never holds what is inside |
| a raw pointer | its address, not its target |
| a trait object, a closure, a function pointer | opaque |
| a type parameter `T` | `Snap<T>`, abstract, and substituted at each instantiation |

- **A snapshot holds every field, private ones included,** so that two values that differ anywhere have different snapshots. Rust's privacy decides who may name a field: logical code reads `s.f` only where Rust code could read `f`. Outside the crate that defines a type with private fields, its snapshots can be held, passed on, and compared, but not looked into; what they mean is known through the type's model and its contracts.
- **Snapshots never change unseen.** A snapshot belongs to a version ([Versions](#versions)). It holds no contents of an `UnsafeCell`, which change without a write, and no target of a raw pointer, only its address. So interior mutation, and any write through a raw pointer, leaves every snapshot as it was, by the aliasing rules that `unsafe` code must already obey.
- **`UnsafeCell` is special because Rust makes it so.** It is a language item, and the only way Rust lets a value change through `&`. Every other type's snapshot is built from its fields, however much `unsafe` code the type uses. A type of the program's own built on raw pointers therefore has a snapshot that holds only addresses. Giving it a meaningful one, as the library does for `Vec`, needs an assumed specification or ghost permissions, which are for later.
- **Two snapshots are equal exactly when they are the same variant with equal fields.** Their opaque parts are equal only when they come from one version.
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
- **Deriving needs a model for every field.** A field of type `Cell<u32>`, which has none, makes the derive an error. Proof fields are left out, because they carry no information.
- **For now, only types without type parameters** derive a model. A generic derive would need a `Model` bound on its `impl`, which is ruled out below.
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
  - In the checked program, `o.go()` calls `Bar`'s.
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

  `Cell`, `RefCell`, `Mutex`, and the atomics have none. Others, such as `String`, `char`, maps, `Rc`, and `Arc`, are for the library's RFC ([#11](https://github.com/nikhilgarg28/locus/issues/11)).
- **A container works for every element type through its snapshot.** `Vec<T>` has a model only when `T` has one, but `snap(v)` is a `Seq<Snap<T>>` for every `T`. So `snap(v).len()` works for a vector of `Point`s without a model, and generic code without a `Model` bound writes `snap(final!(v))[i] === snap(v)[j]` to say what a `swap` did.
- **In generic code, a value of a type parameter `T` without a `Model` bound cannot be used as a value;** its snapshot can. Facts proven about snapshots hold at every instantiation. With `T: Model`, logical code can use `T::Logic`.
- **A model is a function of the snapshot,** so it never reads the contents of an `UnsafeCell` or the target of a raw pointer, and a call that Locus knows nothing about, taking `&x`, never changes `x`'s model, as RFC 0001 relies on.
- **A model's definition never depends on itself.** Inside a hand-written model, and inside every logic function it calls, a value of the type is read through its fields, never taken to the model being defined. So a model cannot reach itself through a helper.
- **`model(x)`** gives `x`'s model explicitly. **In Rust's terms, `Model` has no methods,** as `compat.names.no-methods` requires. Its function takes the value as an ordinary argument, and is called either through `model(x)` or implicitly.
- **Models are interpretations.** Declaring one proves nothing about any operation. What runtime operations do to models is the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)), together with the specifications of the standard library that Locus's library declares.

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
  - Proof search can find one from what the runtime semantics says relates the versions, such as `n₁ == n₀ + 1` after `n = n + 1`.
- **Versions are logical variables, not definitions.** The facts that relate them are hypotheses that proofs may use, so a proof about one version never fits another by computation alone.
- **In a signature,** parameters are at their entry versions, and `final!(x)` is the version that `x`'s referent has when the borrow ends.
- **What the runtime semantics must give at least.** How versions are formed from a function's typed body, at the joins of branches and in loops, is the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)). Whatever it decides, it gives at least these facts:
  - a `let` or an assignment, directly or through a reference, gives the new version the value written;
  - a version that no write separates from another, on every path between them, is the same version; on a later iteration of a loop, a write anywhere in the loop separates;
  - inside a branch, its condition or matched pattern holds;
  - after a call returns, its postconditions hold;
  - machine arithmetic that does not overflow gives the result of the same arithmetic on the models, and machine comparisons agree with the models';
  - `len` and indexing of slices and vectors agree with their models, as Locus's library specifies ([#11](https://github.com/nikhilgarg28/locus/issues/11));
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
  - Matching gives the equality as a proof, which later proofs can rewrite along ([#15](https://github.com/nikhilgarg28/locus/issues/15)).
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
| `_` | A hole: proof search finds a proof of the expected proposition |
| `prove!(P)` | A hole whose proposition is written out |
| `lemma(arguments)` | A call of a logic function that returns a proof |
| `Even::Step(m) @ h` | A constructor of an inductive proposition: its witnesses, then after `@` a proof of the arm's body |
| `match h { Even::Zero @ z => ..., Even::Step(m) @ k => ... }` | Reasoning by cases on a proof |
| `And::Intro(h, k)`, `Or::Left @ h`, `Or::Right @ k`, `True::Intro` | The connectives' constructors ([Connectives](#propositions)) |
| `Exists::Witness(v) @ h` | A proof of `exists \|x: T\| P`, from a witness `v` and a proof `h` of `P` at `v` |
| `\|x: T\| proof`, `\|h: @P\| proof` | A proof of `forall \|x: T\| Q`, or of `P ==> Q`: a logical function from each `x`, or each proof of `P`, to a proof of `Q` |
| `all(v)`, `imp(h)` | Using a proof `all` of a `forall` at `v`, or a proof `imp` of an implication on a proof `h` of its premise |

The connectives' constructors and eliminators are listed under [Propositions](#propositions). Forms for using a logic function's definition and for rewriting along an equality are for RFC 0003 ([#15](https://github.com/nikhilgarg28/locus/issues/15)).

- **A match on a proof produces a proof.** A witness inside a proof, such as an existential's, can be used to build other proofs, but never becomes a logical or runtime value. This is Lean's rule for eliminating propositions; Lean's exceptions for propositions with at most one proof can come later.
- **Search proposes, and the kernel decides.**
  - Holes and `prove!` are filled by untrusted search: automation, decision procedures, solvers, or AI.
  - A candidate counts only once the kernel checks it.
  - Proofs that were found are stored with the crate, so that checking never depends on finding them again ([vision](../../VISION.md), commitment 3). The format is [#13](https://github.com/nikhilgarg28/locus/issues/13).
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
- **`final!(r)` is a mutable reference's final value.** For any mutable reference `r`, `final!(r)` is the value its referent has when the borrow ends, that is, when neither `r` nor any reborrow of it is used again. This holds wherever `r` comes from: a parameter, a local, or a result. Mutable references stored in data are for later. It is a *prophecy*: a logical value that is fixed when the borrow is created and becomes known when the borrow ends.
  - **When no reborrow of a `&mut` parameter can outlive the call,** the borrow ends when the function returns, after its locals are dropped, so `final!(x)` is the value at that point. That is how a function states what it did, as `bump` does above.
    - No reborrow can outlive the call when `x`'s lifetime is a lifetime parameter of the function that appears nowhere else in its signature, bounds and `where` clauses included. A `&'static mut` parameter does not qualify.
    - Otherwise, as in `fn stash<'a>(x: &'a mut u32, slot: &mut Option<&'a mut u32>)`, the caller may write through the reborrow later, so `final!(x)` is not known at return.
    - Because drops can write through a borrow, the postcondition is checked after the locals' drops. That needs typed bodies with drops made explicit ([#6](https://github.com/nikhilgarg28/locus/issues/6)).
  - **When a function returns a reborrow,** `final!` relates the two ends:

    ```rust
    fn first_mut(pair: &mut (u32, u32))
        -> (r: &mut u32, @(r == pair.0 && final!(pair) === (final!(r), pair.1)))
    ```

  - **Inside a body,** logical code reads current versions: `*x` at a point in the body is its value there. A local borrow has a final value too. When the borrow ends, the place it borrowed from gets a new version, which holds `final!(r)`.
  - **`final!(r)` reads no place.** It is not an observation in RFC 0001's sense, so it never extends a borrow.
  - **`final!` appears in postconditions and in bodies,** not in preconditions.
  - **Entry values need no notation of their own.** A signature already means entry values. A body that needs one later names a snapshot: `let start = logic! { *x };`.
  - **Soundness.** RustHornBelt proved prophecies sound for safe Rust, and Creusot is built on them. Using them needs to know where each borrow ends, which rustc's borrow checker computes on MIR. How Locus obtains that is [#6](https://github.com/nikhilgarg28/locus/issues/6).
- **Partial correctness.** A postcondition holds when the function returns. A function that panics or loops forever proves nothing, so `fn never() -> @False { loop {} }` is accepted. A logic function cannot be written that way, because logic functions are total. That a runtime function terminates is a separate claim ([#14](https://github.com/nikhilgarg28/locus/issues/14)).
- **Calls.** After a call returns, the caller has the proofs in its result, about the arguments it passed: `let (out, bounded) = clamp(n);`.
- **Ghost parameters.** A runtime function may take parameters of logical types, such as `n: Nat`, to state a contract in terms of a logical value that the caller supplies. Such a function never reaches unchecked code unless its ghost parameters are completable ([The boundary with unchecked code](#the-boundary-with-unchecked-code)).
- **Trait methods with contracts** are for [#12](https://github.com/nikhilgarg28/locus/issues/12). Until then, no trait method's signature holds logical values, as rule 4 under [Ghost values](#ghost-values) says. When they arrive, an implementation in Rust cannot be checked, so in export mode such a trait will not be reachable from other crates.

### Proofs in data

**Invariants of the program's own types.** A struct or enum variant may have proof fields, which may read the type's other fields:

```rust
struct Interval {
    lo: u32,
    hi: u32,
    ordered: @(lo <= hi),
}
```

- **Every value of the type satisfies its proof fields, at every point:**
  - each construction must prove them, including constructions in code that macros and derives generate, and a struct update `..base`, which can reuse a proof from `base` when every field that proof reads comes from `base` too;
  - no place based on a proof field, or on a field that a proof field reads, is mutably borrowed, so neither `&mut iv.lo` nor `&mut cache.hint` is allowed;
  - such a place may be assigned, when each proof field that reads it is proved for the new value, as if it were reassigned a hole `_`. So `iv.lo = 0` is accepted, because `0 <= iv.hi` holds whatever `hi` is, and `iv.lo = 10` is rejected unless `10 <= iv.hi` is known. Rust evaluates the right-hand side first, and stores the new value even when dropping the old one panics (tested), so only the old state and the new one can ever be seen;
  - a proof field itself, including one of type `Option<@P>`, may be read, and assigned a value whose proofs prove its proposition. Like any field, if another proof field reads it, assigning it re-proves that one too;
  - code that Locus cannot translate, such as inline assembly, never constructs the type.
- **Why proof fields are not borrowed mutably.** A borrow of a proof field would outlive the assignments it overlaps. Through `let h = &mut cache.hint;`, a proof proved for `cache.x` as it was could be written after `cache.x = 0` was accepted, because `hint` was `None` when `x` was assigned.
- **A field whose type holds a proof,** such as `Option<@P>`, counts as a proof field.
- **A proof field's runtime structure can carry an invariant too.** With `hints: Vec<@True>` and `nonempty: @(hints.len() > 0)`, `nonempty` reads `hints`, so `hints` is both a proof field and a field that a proof reads, and both rules hold. `s.hints.clear()` borrows it mutably and is rejected. `s.hints = vec![h]` is accepted, because `nonempty` holds for the new vector.
- **Generated code that builds the type** must still prove the proof fields. The standard derives are understood: a derived `Clone` copies logical data fields too.
- **No `unsafe impl` that a derive or another macro generates** is allowed on such a type, in either mode. A derive such as `bytemuck::Zeroable` checks only what it can see, and would let safe code build a value without its proof.
- **`unsafe` code makes a promise beyond Rust's.** After erasure, a value without its invariant can be perfectly valid Rust: an `Interval` with `lo > hi`, or a `Positive` holding zero, breaks no rule of Rust's. So `mem::zeroed`, `transmute`, or a raw write can break an invariant while keeping Rust's safety contract. The [trust model](../TRUST.md) therefore lists a Locus-specific assumption: `unsafe` code, in the crate and in its dependencies, never creates or changes a value of a type with proof fields except through code that Locus checks. A hand-written `unsafe impl` makes that promise for its type. A proof result that relies on an invariant lists this assumption among its own (RFC 0001).
- **In export mode, other crates can neither construct such a type nor reach the fields its proofs read** ([Export mode](#export-mode)).
- **Changing such a field through a borrow waits.** For `NonEmpty { items: Vec<u32>, ok: @(items.len() > 0) }`, `self.items.push(x)` mutably borrows `items`, and is rejected even though it keeps the invariant: a panic inside `push` could leave a broken value in view. Assigning `self.items` a new vector, with the proof for it, works. Updating through a borrow when nothing can panic is [#6](https://github.com/nikhilgarg28/locus/issues/6), and so is an operation whose contract restores the invariant on return and on unwind ([#18](https://github.com/nikhilgarg28/locus/issues/18)).
- **Reading a proof field is logical code:** `iv.ordered` is a proof of `iv.lo <= iv.hi`.
- **This holds even for values that pass through code Locus does not check.**
  - Only checked code constructs the type or writes the fields its proofs read. In build mode, every crate that can name the type is checked; in export mode, other crates can do neither.
  - So unchecked code can only move, copy, or drop a value, or call the type's own checked functions.
  - This is how invariants become the fourth way facts enter the logic, in RFC 0001's sense.
  - Such a type is a distinct Rust type, so it can instantiate any generic parameter. Unlike a marker, it cannot be confused with another type.
- **Logical data fields,** such as `count: Nat`, are ghost too, and stay as fields of type `LogicalData`. Assigning one is a ghost statement.

**Runtime types holding logical values.** Any runtime type may hold logical values as type arguments, as described under [Ghost values](#ghost-values):
- `Option<@P>` keeps its tag at run time. `Some(h)` needs `h: @P` to build it, and matching `Some(h)` gives back `h: @P`.
- `Vec<(i: usize, @(i < xs.len()))>` is a `Vec<(usize, Erased)>` of indices, each with a proof that it is in bounds. `Vec<Nat>` keeps its length.
- In a tuple type, a name such as `i` binds the component for the propositions after it, as in a named result. A component that a later component's proposition reads follows the rules for invariants: it is never mutably borrowed, and an assignment to it must re-prove the later component. So `v[0].0 += 1` is rejected unless `v[0].0 + 1 < 10` is known.
- **A proposition written in a type is fixed where the type is written,** like any other proposition. In `let checked: Vec<(i: usize, @(i < items.len()))>`, it is about `items` as it is at the `let`. After `items.clear()`, the stored proofs still prove that, and nothing about the new `items`.
- What a value holds is known through checked code and specified standard library code, and is forgotten elsewhere.

### Export mode

In build mode, every crate that depends on a Locus crate is checked, so a crate's whole interface, contracts included, is available to the crates that use it. In export mode, Rust users build the crate, and Locus never sees their code ([RFC 0001](0001-compiler-pipeline.md#sources-and-modes)). So what other crates can reach must be plain Rust, and must not let them break what was proven. This restricts only the exported interface: inside the crate, everything above applies unchanged.

What other crates can reach is every item and field that the source declares plain `pub` and that other crates can reach from the crate's root, together with the public fields and methods of the types among them. Items and fields declared `pub(locus)` are not part of it ([RFC 0001](0001-compiler-pipeline.md#sources-and-modes)), and neither is a `pub` item that nothing outside the crate can reach. A plain `pub` item that breaks the rules below is an error, which suggests `pub(locus)`.

- **No contracts.** A function or method that other crates can reach has no proof parameters and no proofs in its result. A Rust caller cannot supply a proof for a precondition, and could not use one from a postcondition.
- **No logical values.** No parameter, result, constant, static, or type alias that other crates can reach has a type that holds logical values, such as `Nat`, `Option<@P>`, or `Vec<(usize, @P)>`. Nor does such a function have a `Model` bound: the exported program removes it, so a Rust caller could use a type at which the function was never checked.
- **No public logical fields.** No field that other crates can reach holds logical values, proof fields included. Such a field can be `pub(locus)`. Rust makes every field of an enum's variants public, so an enum that other crates can reach has no logical or proof fields.
- **Invariants stay closed.** Other crates can neither construct a struct that has proof fields, nor reach a field that a proof field reads.
  - A struct with proof fields always has a field that other crates cannot reach, since its proof fields cannot be plain `pub`, so other crates cannot construct it.
  - Its other fields may be public, so long as no proof field reads them.
- **Ghost code only where erasure can remove it.** Every invocation of `logic!`, `prop!`, or `prove!`, and every call that passes a ghost argument, is written directly in the crate's own code, and not inside another macro's input or output. The exported program cannot depend on Locus's library, so erasure removes each of them as text, and text inside another macro's input means what that macro makes of it (RFC 0001).
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

### Erasure, in summary

RFC 0001 defines erasure. For this RFC's constructs:

In the table, the marker is `Erased` for a proof and `LogicalData` for other logical data.

| In the source | In build mode, both programs | In an exported program |
|---|---|---|
| A parameter or local of logical type, proof parameters included, and every argument for one | Kept, with the marker's type | Removed |
| A field of logical type | A field of the marker's type | The same |
| A result of logical type | The marker | `()` |
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
| `logic.types.positive` | `logical-layer/logical-fields.accept.lc`, `proofs/positive-proposition.accept.lc`, `logical-layer/negative-logical-type.reject.lc`, `logical-layer/negative-through-derived-model.reject.lc`, `logical-layer/negative-through-forall.reject.lc`, `proofs/negative-proposition.reject.lc` |
| `logic.arith.nat-subtraction` | `logical-layer/length-minus-one.accept.lc`, `logical-layer/nat-difference-is-int.accept.lc`, `logical-layer/nat-difference-as-nat.reject.lc` |
| `logic.ghost.erased` | `logical-layer/ghost-parameters.accept.lc`, `logical-layer/logical-tuple-keeps-shape.accept.lc`, `logical-layer/logical-values-in-vec.accept.lc`, `logical-layer/runtime-call-bound-first.accept.lc` |
| `logic.ghost.marker-traits` | `logical-layer/derive-eq-with-proof-field.accept.lc`, `logical-layer/compare-logical-vectors.reject.lc`, `logical-layer/derive-eq-with-logical-field.reject.lc` |
| `logic.ghost.types-forward` | `logical-layer/logical-values-in-vec.accept.lc`, `logical-layer/types-not-written.reject.lc` |
| `logic.ghost.interior-mutability` | `logical-layer/cell-of-proof.accept.lc`, `logical-layer/lend-proof-cell-to-unchecked.reject.lc`, `logical-layer/rc-cell-to-unchecked.reject.lc` |
| `logic.ghost.runtime-parts-kept` | `contracts/checked-indices.accept.lc`, `contracts/proof-in-option.accept.lc`, `logical-layer/logical-values-in-vec.accept.lc`, `contracts/some-without-proof.reject.lc` |
| `logic.ghost.runtime-code-kept` | `logical-layer/runtime-call-in-proof-field.accept.lc`, `logical-layer/runtime-check-kept.accept.lc` |
| `logic.ghost.forgotten` | `logical-layer/completable-holder-to-unchecked.accept.lc`, `logical-layer/proof-through-own-generic.accept.lc`, `logical-layer/completable-holder-forgotten-after-lending.reject.lc`, `logical-layer/option-through-black-box.reject.lc`, `logical-layer/own-generic-with-any.reject.lc`, `logical-layer/proof-through-any.reject.lc`, `logical-layer/proof-through-black-box.reject.lc`, `logical-layer/proof-through-raw-pointer.reject.lc`, `logical-layer/proof-through-transmute.reject.lc` |
| `logic.ghost.generic-instantiations` | `logical-layer/proof-through-own-generic.accept.lc`, `logical-layer/own-generic-with-any.reject.lc` |
| `logic.ghost.no-runtime-control` | `contracts/proof-in-option.accept.lc`, `logical-layer/branch-on-logical.reject.lc` |
| `logic.ghost.positions-logical` | `logical-layer/runtime-call-bound-first.accept.lc`, `logical-layer/runtime-call-in-proof-field.accept.lc`, `logical-layer/runtime-value-through-logic-block.accept.lc`, `logical-layer/runtime-call-in-proof-argument.reject.lc`, `logical-layer/runtime-value-to-logical-parameter.reject.lc` |
| `logic.ghost.shown-by-source` | `logical-layer/operator-in-logic-block.accept.lc`, `logical-layer/operator-on-unshown-ghost.reject.lc` |
| `logic.ghost.no-trait-impls` | `logical-layer/trait-impl-on-invariant-type.accept.lc`, `logical-layer/proof-in-trait-impl.reject.lc` |
| `logic.boundary.unchecked` | `logical-layer/lend-proof-holder-to-specified.accept.lc`, `logical-layer/lend-proof-holder-to-unchecked.reject.lc` |
| `logic.boundary.completable` | `logical-layer/proof-free-logical-to-unchecked.accept.lc`, `logical-layer/proof-only-logical-to-unchecked.reject.lc` |
| `logic.boundary.calls` | `logical-layer/completable-function-to-unchecked.accept.lc`, `logical-layer/ghost-function-as-value.accept.lc`, `logical-layer/ghost-parameters.accept.lc`, `logical-layer/proof-payload-function-as-value.accept.lc`, `logical-layer/proof-function-as-pointer.reject.lc`, `logical-layer/proof-function-to-specified-generic.reject.lc`, `logical-layer/proof-function-to-unchecked.reject.lc` |
| `logic.boundary.writable` | `logical-layer/own-type-with-invariant-to-unchecked.accept.lc`, `logical-layer/own-generic-type-to-unchecked.reject.lc`, `logical-layer/proof-sender-to-unchecked.reject.lc` |
| `logic.boundary.writes` | `logical-layer/completable-holder-to-unchecked.accept.lc`, `logical-layer/lend-proof-holder-to-specified.accept.lc`, `logical-layer/own-type-with-invariant-to-unchecked.accept.lc`, `logical-layer/proof-free-logical-to-unchecked.accept.lc`, `logical-layer/share-proof-holder-with-unchecked.accept.lc`, `logical-layer/completable-holder-forgotten-after-lending.reject.lc`, `logical-layer/lend-proof-cell-to-unchecked.reject.lc`, `logical-layer/lend-proof-holder-to-unchecked.reject.lc`, `logical-layer/own-generic-type-to-unchecked.reject.lc`, `logical-layer/proof-only-logical-to-unchecked.reject.lc`, `logical-layer/proof-sender-to-unchecked.reject.lc`, `logical-layer/rc-cell-to-unchecked.reject.lc` |
| `logic.code.logical-values` | `logical-layer/logic-fn-model-result.accept.lc`, `logical-layer/logic-fn-runtime-result.reject.lc`, `logical-layer/logic-fn-unit-result.reject.lc` |
| `logic.eq.same` | `logical-layer/same-is-a-proposition.accept.lc`, `logical-layer/sameness-on-sequences.accept.lc`, `logical-layer/same-is-not-a-bool.reject.lc` |
| `logic.eq.runtime-values` | `logical-layer/equal-models-equal-field-models.accept.lc`, `models/derived-model-equal-fields.accept.lc`, `logical-layer/equal-models-not-equal-fields.reject.lc` |
| `logic.eq.snapshots` | `models/snapshot-unchanged.accept.lc`, `models/snapshots-equal-fields.accept.lc`, `logical-layer/cell-contents-unknown.reject.lc` |
| `logic.eq.comparisons` | `logical-layer/comparison-is-a-bool.accept.lc`, `logical-layer/comparison-on-sequences.reject.lc`, `logical-layer/same-is-not-a-bool.reject.lc` |
| `logic.code.no-runtime-calls` | `logical-layer/logic-calls-logic-function.accept.lc`, `logical-layer/logic-calls-runtime-function.reject.lc` |
| `logic.fn.total` | `logical-layer/measure-recursion.accept.lc`, `logical-layer/structural-recursion.accept.lc`, `logical-layer/measure-increases.reject.lc`, `logical-layer/nonterminating-logic-fn.reject.lc`, `proofs/diverging-logic-proof.reject.lc` |
| `logic.const.well-founded` | `logical-layer/logical-constant-chain.accept.lc`, `logical-layer/logical-constant-cycle.reject.lc` |
| `models.snapshots` | `models/snapshot-of-vector-without-model.accept.lc`, `models/snapshot-unchanged.accept.lc`, `models/snapshots-equal-fields.accept.lc` |
| `models.snapshot-privacy` | `models/private-field-in-own-module.accept.lc`, `models/private-field-of-snapshot.reject.lc` |
| `models.frozen-data` | `models/hand-written-model.accept.lc`, `logical-layer/cell-contents-unknown.reject.lc`, `models/model-reads-cell.reject.lc` |
| `models.observation` | `models/integer-model-no-overflow.accept.lc`, `observation/after-mutable-borrow-ends.accept.lc`, `observation/before-move.accept.lc`, `models/wrapping-is-not-the-model.reject.lc` |
| `models.no-default` | `models/snapshot-of-vector-without-model.accept.lc`, `models/snapshots-equal-fields.accept.lc`, `models/equal-fields-without-model.reject.lc`, `models/vector-without-element-model.reject.lc` |
| `models.integer-range` | `models/integer-range.accept.lc`, `models/integer-range-tighter.reject.lc` |
| `models.one-per-type` | `models/hand-written-model.accept.lc`, `models/two-models.reject.lc` |
| `models.derived` | `logical-layer/equal-models-equal-field-models.accept.lc`, `models/derived-model-equal-fields.accept.lc`, `logical-layer/equal-models-not-equal-fields.reject.lc`, `models/derive-model-for-generic.reject.lc`, `models/derive-model-with-cell-field.reject.lc` |
| `models.bounds-on-functions` | `models/model-bound-on-function.accept.lc`, `models/model-bound-on-impl.reject.lc` |
| `models.declared-by-owner` | `models/hand-written-model.accept.lc`, `models/model-for-foreign-type.reject.lc` |
| `models.own-definition` | `models/hand-written-model.accept.lc`, `models/model-helper-reads-fields.accept.lc`, `models/model-of-itself.reject.lc`, `models/model-through-helper.reject.lc` |
| `proofs.versions` | `observation/proof-before-move.accept.lc`, `proofs/reproved-after-assignment.accept.lc`, `proofs/snapshot-survives-assignment.accept.lc`, `proofs/stale-proof-across-iterations.reject.lc`, `proofs/stale-proof-after-assignment.reject.lc`, `proofs/stale-proof-after-same-value.reject.lc`, `proofs/stale-proof-through-reference.reject.lc`, `proofs/stored-proof-after-clear.reject.lc` |
| `proofs.connectives` | `proofs/connectives.accept.lc`, `proofs/conjunction-needs-both.reject.lc` |
| `proofs.props.constructors` | `proofs/positive-proposition.accept.lc`, `proofs/wrong-arm.reject.lc` |
| `proofs.props.matching` | `proofs/match-every-arm.accept.lc`, `proofs/missing-arm.reject.lc` |
| `proofs.elimination` | `proofs/witness-stays-in-proof.accept.lc`, `proofs/witness-to-data.reject.lc` |
| `proofs.partial` | `proofs/diverging-runtime-proof.accept.lc`, `proofs/returns-on-some-path.reject.lc` |
| `contracts.entry-values` | `contracts/final-value-at-return.accept.lc`, `contracts/entry-value-is-not-final.reject.lc` |
| `contracts.final` | `contracts/final-reads-no-place.accept.lc`, `contracts/final-value-at-return.accept.lc`, `contracts/local-borrow-final.accept.lc`, `contracts/returned-borrow.accept.lc`, `contracts/final-escapes-through-parameter.reject.lc`, `contracts/wrong-final.reject.lc` |
| `contracts.invariants` | `contracts/invariant-constructed.accept.lc`, `contracts/invariant-replaced-whole.accept.lc`, `export/public-invariant-field-when-built.accept.lc`, `contracts/invariant-default-derive.reject.lc` |
| `contracts.invariants.no-mutable-borrow` | `contracts/unread-field-borrowed.accept.lc`, `contracts/invariant-field-borrowed.reject.lc`, `contracts/proof-container-cleared.reject.lc`, `contracts/proof-field-borrow-outlives-assignment.reject.lc`, `contracts/proof-field-borrowed.reject.lc` |
| `contracts.invariants.assignment` | `contracts/invariant-field-assigned.accept.lc`, `contracts/invariant-replaced-whole.accept.lc`, `contracts/proof-container-replaced.accept.lc`, `contracts/proof-field-assigned.accept.lc`, `contracts/dependent-tuple-component-written.reject.lc`, `contracts/invariant-broken-by-assignment.reject.lc`, `contracts/option-proof-field.reject.lc`, `contracts/proof-field-through-box.reject.lc` |
| `contracts.invariants.proof-holders` | `contracts/proof-container-replaced.accept.lc`, `contracts/proof-field-assigned.accept.lc`, `contracts/dependent-tuple-component-written.reject.lc`, `contracts/option-proof-field.reject.lc`, `contracts/proof-container-cleared.reject.lc`, `contracts/proof-field-borrowed.reject.lc` |
| `contracts.unsafe-impls` | `contracts/hand-written-unsafe-impl.accept.lc`, `contracts/unsafe-send-on-invariant-type.accept.lc`, `contracts/macro-generated-unsafe-impl.reject.lc` |
| `export.plain-interface` | `export/dropped-postcondition.accept.lc`, `export/locus-logical-field.accept.lc`, `export/private-enum-with-invariant.accept.lc`, `export/private-logical-field.accept.lc`, `export/public-precondition-when-built.accept.lc`, `export/wrapped-precondition.accept.lc`, `export/model-bound.reject.lc`, `export/public-enum-with-invariant.reject.lc`, `export/public-logical-field.reject.lc`, `export/public-postcondition.reject.lc`, `export/public-precondition.reject.lc`, `export/public-proof-in-option.reject.lc` |
| `export.invariants-closed` | `export/private-invariant-fields.accept.lc`, `export/public-invariant-field-when-built.accept.lc`, `export/public-invariant-field.reject.lc` |

The examples use this RFC's syntax. Where [#5](https://github.com/nikhilgarg28/locus/issues/5) later respells something, their outcomes stay the same.

## Interactions

| Area | Effect |
|---|---|
| Types | Every type is runtime or logical, by its type. Logical types contain only logical types. Runtime types may hold logical types, which are markers at run time: `Erased` for proofs and `LogicalData` for other logical data. `Int` is built in, and `Nat` is a library type whose difference is an `Int`. Every runtime type has a snapshot type `Snap<T>`, and a model only when one is declared, derived, or given by the library. |
| Propositions and proofs | Propositions are Lean's, with Rust's syntax: inductive propositions with named arms, defined predicates, connectives, and quantifiers. `===` is Lean's equality, and comparisons give `Bool`s, as Rust's give `bool`s. Propositions are about versions. Proofs are irrelevant, ghost, and kernel-checked. |
| Ownership and mutation | Logical code reads places under RFC 0001's observation rules. Every write makes a new version. A `&mut` reference has a current value and a final value, `final!`. How runtime code changes values in the logic is [#6](https://github.com/nikhilgarg28/locus/issues/6). |
| Erasure and runtime behavior | In build mode, every ghost value is a zero-sized marker in both programs, and nothing is erased. An exported program removes ghost parameters and locals, and keeps markers inside runtime types and fields. Runtime parts such as tags and lengths are kept, every call of a function with a precondition is checked, and calls of runtime functions are kept even when their results are ghost. |
| Rust interoperability | Rust code never sees a proof. In build mode, every crate that uses a Locus crate is checked. In export mode, what other crates can reach has no contracts and no logical values, and lets them neither build a value without its invariant nor reach a field that one reads. `pub(locus)` keeps the rest for Locus crates. What a value holds is forgotten when it passes through unchecked generic code. Models are declared by a type's own crate. |
| Diagnostics | New errors: logical values where runtime ones are needed, runtime calls in logical code, non-terminating logic functions, proofs about other versions, values that carry no proof after passing through unchecked code, invariants not established, and contracts or logical values in an exported interface. They are written as reasons until error codes exist. |
| Trust model | New entries: the models and specifications that Locus's library declares for standard types and generic code, with its list of types writable through a shared handle and of completable logical types; the consistency of Lean's axioms; and `unsafe` code preserving invariants, a promise beyond Rust's safety contract. The logical pass's ghost checks replace RFC 0001's checks on proofs in runtime code, and where borrows end joins rustc's borrow-checker entry. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Proofs as zero-sized values that Locus believes by their type.** This RFC keeps such a marker, `Erased`, but Locus never believes it.
- Proofs would flow through every generic function for free.
- Rejected. With one Rust type for every proposition, Rust code that can tell types apart, through `Any` or specialization, could turn one proof into another.
- Making that sound needs every Rust generic to be parametric, which neither specialization nor `unsafe` code guarantees.

**No logical types inside runtime types,** apart from proofs inside `Option` and `Result`.
- Rejected at the owner's direction. Containers of ghost values are useful, and forgetting ghost contents at unchecked code makes them sound.

**Proof tokens.** Each proof parameter would become a parameter of a private zero-sized token type, which erasure supplies only at calls Locus has checked. Other crates would obtain tokens through an `unsafe` constructor, and a function whose precondition sits in its parameters' data would be `unsafe` to call.
- Rejected at the owner's direction for the two modes. Contracts are visible only to code that Locus checks, so nothing needs a token.

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

**Replacing a value with an invariant whole** for every change to a field that its proofs read. Rejected: an assignment can only expose the old state or the new one, so proving the new one is enough.

**No `unsafe impl` but `Send` and `Sync`** on a type with proof fields. Rejected: a hand-written `unsafe impl` is the programmer's own promise, which the trust model already covers. Only a generated one bypasses it.

**`mint`, an unspecified `Int` for each value of a type without a model.** Rejected: it tells logic nothing that a snapshot does not, and loses the type, so logic could do meaningless arithmetic on the integers.

**Inferring ghost bindings from types before rustc runs,** so that `let k = v[0]; let m = k + 1;` would work without `logic!`. Rejected: lowering would need `v`'s type, and so type inference and method resolution over the standard library, which is rustc's work. A wrong guess can compile with another meaning ([RFC 0001](0001-compiler-pipeline.md#alternatives)).

**Types with interior mutability holding no logical values.** Rejected at the owner's direction: proofs are irrelevant, so a cell's contents need only have the right type, and keeping unchecked code from writing them is enough.

**Euclidean division for `/`,** as Lean's `/` on integers is. Rejected: a machine integer's `/` truncates, and its model's should agree. The Euclidean operations are `div_euclid` and `rem_euclid`, as in Rust.

**CamelCase for logic functions that give a `Prop`,** as Lean names propositions. Rejected at the owner's direction: they are functions, and Rust names functions in snake_case. CamelCase marks `prop` declarations, which are matched like enums.

**An anonymous constructor for one-arm propositions,** such as `(hp, hq)` for `And::Intro(hp, hq)`, as Lean's `⟨hp, hq⟩` is. Deferred by the owner to the RFC on proof construction ([#15](https://github.com/nikhilgarg28/locus/issues/15)).

**`=>` for implication.** Rejected: inside a logical `match`, `=>` already separates an arm from its body, so `==>` is clearer.

**`forall (x: T) { F }` keywords.** Rejected for `forall |x: T| F`, which reads like Rust's closures and needs no new delimiters.

**A `ghost` keyword or a `Ghost<T>` wrapper,** as in Verus. Rejected: the type already says whether a value is ghost.

**Tracked evidence,** a `let mut h: @P` that follows the current version of what it reads. Deferred to the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)), together with loops, where it serves as a loop invariant.

## Prior art

- **Lean 4** is the model for the logic, the kernel, inductive propositions, and proof irrelevance, and for separating a `Bool` comparison (`BEq`) from equality (`Eq`).
- **Creusot** has logic functions, a mutable reference's current value `*x` and final value `^x`, and models through its `View` trait. Its handling of `&mut` is the model for `final!`.
- **RustHorn and RustHornBelt** (Matsushita et al., ESOP 2020 and PLDI 2022) gave prophecies for Rust's mutable borrows and proved them sound.
- **Verus** separates spec, proof, and exec code, erases ghost code completely, and models types with `view()`.
- **Prusti** writes entry values with `old(...)`, the convention this RFC replaces with entry-by-default and `final!`.
- **Dafny** has ghost variables and fields, `decreases` clauses, and erasure of ghost code.
- **F\* with Low\* and KaRaMeL** erases proofs to produce C, as Locus erases them to produce Rust.

## Unresolved questions

**Decided by the owner while this RFC was drafted.** Recorded here; accepting the RFC confirms them. Where the owner set the direction, the mechanism is this RFC's proposal.
1. **Ghost proofs and forgetting.** Proofs and logical values are zero-sized markers at run time. In build mode both programs keep every ghost position as a marker; an exported program removes proof parameters and other ghost parameters and locals. Ghost contents are tracked only through checked code and specified standard library code, and contracts are visible only to code that Locus checks, through the two modes of RFC 0001.
2. **Logical values in runtime types,** such as `Vec<Nat>` and `Option<@P>`.
3. **The `&mut` convention:** entry values in signatures, and `final!(r)` for the value when the borrow ends, with no separate notation for entry values.
4. **`Int` built in, and `Nat` from the library,** with `Nat - Nat` an `Int`. Integer division truncates toward zero, as Rust's does, division by zero gives zero, and Euclidean division is `div_euclid`.
5. **Inductive propositions with named arms,** built and matched as `Arm(witnesses) @ proof`, and `&&`, `||`, and `!` on propositions.
6. **Equality.** `===` is Lean's equality, a proposition. `==` and the orderings give a `Bool`, and are built in only on `Int`, `Nat`, and `Bool` until [#12](https://github.com/nikhilgarg28/locus/issues/12).
7. **Naming.** Logic functions in snake_case, and `prop` declarations in CamelCase.
8. **Snapshots and models.** Every runtime type has a snapshot type `Snap<T>`, derived from its definition, which holds every field and nothing behind an `UnsafeCell` or a raw pointer. A type has a model only when it is declared, derived with `#[derive(Model)]` from its fields' models, or declared by Locus's library; there is no default, for now. Logical code reads places as snapshots and uses values through their models, and `snap(x)` gives a snapshot explicitly. On runtime values, `===` compares models; on snapshots, values. Every logical computation produces a logical value.
9. **Interior mutability.** Types with interior mutability may hold logical values.
10. **The traits logical types implement:** `Clone`, `Copy`, and `Debug`, plus comparison and hashing for proofs. So proofs and logical data have separate markers, `Erased` and `LogicalData`, and rustc itself rejects comparing, ordering, or hashing logical data. The owner named the second `LogicalData`, so that rustc's errors say the value is logical.
11. **Export mode's rules.** What other crates can reach states no contracts, postconditions included, has no logical values in its types or fields, and has no `Model` bounds. Other crates can neither construct a struct with proof fields nor reach a field that its proofs read.
12. **Proof construction** beyond this RFC's forms is a third foundational RFC ([#15](https://github.com/nikhilgarg28/locus/issues/15)). An anonymous constructor for one-arm propositions waits for it, and arms do not become constructors with proof fields.
13. **The boundary with unchecked code,** defined once for both modes. Completable values are exempt, judged conservatively from types. A function needing a proof is never coerced to a pointer, made a trait object, or passed to generic code other than the program's own. No way to write a place holding logical values reaches unchecked code, which Locus decides from types, type arguments included, and from its library's list of types writable through a shared handle, channels included.
14. **Invariants.** Neither a proof field nor a field that a proof reads is mutably borrowed; both may be assigned, with the proofs that read them proved for the new value. This reverses an earlier decision that let proof fields be borrowed. Only `unsafe impl`s that derives or macros generate are rejected, and the trust model states the promise that `unsafe` code makes as one beyond Rust's safety contract.
15. **Runtime calls in ghost positions** stay where they are, except as arguments of ghost parameters, which an exported program removes.
16. **Logical types flow forward,** never backward from a later use.
17. **Ghost bindings that the source shows.** Outside `logic!`, an operator or method call computes with ghost values only when the source shows them to be ghost, and a runtime value passed to a logical parameter is written as `logic! { x }`. This keeps every decision lowering makes before rustc to what the source shows ([RFC 0001](0001-compiler-pipeline.md)).

**Still for the owner,** in order of what they block:
1. **Versions,** following the owner's direction toward SSA: every write makes a new version, even of the same value, and a proof never carries over to another version without a new proof.
2. **One model per type,** declared by the type's own crate or by Locus's library.
3. **Places keep their Rust meaning in logical code,** and models apply to values.
4. **The program's own generic code is checked at each instantiation with logical types,** until [#12](https://github.com/nikhilgarg28/locus/issues/12) allows checking it once.
5. **Syntax:** `===`, `==>`, `forall |x: T|`, `Arm(witnesses) @ proof`, `#[decreases(e)]`, and `final!`.
6. **No logical values in trait implementations,** apart from `impl Model`, until [#12](https://github.com/nikhilgarg28/locus/issues/12); and `Model` bounds only on functions.
7. **Tuples keep their shape,** with markers for logical components, even when every component is logical, rather than losing them.
8. **The obligation at an assignment** to a field that a proof reads is the first proof that the source does not write as a hole. The alternative is to require replacing the value whole, or a destructuring assignment such as `(iv.lo, iv.ordered) = (0, _)`.
9. **Measures are `Int`s.**
10. **A runtime call as the argument of a proof parameter** stays "bind it first" in both modes. Build mode could allow `halve(n, checked_even(n))`, since its programs keep the argument; an exported program removes it. Keeping one rule means a crate that builds also exports.

**Left to later work,** each with its issue:
- The runtime semantics: how runtime operations change versions and models, loops and invariants, tracked evidence, where borrows end, and overflow: [#6](https://github.com/nikhilgarg28/locus/issues/6).
- Remaining syntax questions: [#5](https://github.com/nikhilgarg28/locus/issues/5).
- Distribution, including how an exported package is laid out: [#7](https://github.com/nikhilgarg28/locus/issues/7).
- Closures and async code, including closures that take or return proofs, and closures that capture a reference through which a place holding logical values can be written: [#8](https://github.com/nikhilgarg28/locus/issues/8).
- Locus's standard library: logical types, indexing of sequences, `div_euclid` and `rem_euclid`, and models and specifications of standard types and generic code, including `Cell`, `RefCell`, and `Mutex`: [#11](https://github.com/nikhilgarg28/locus/issues/11).
- Traits and operators on logical types, generic predicates such as a `sorted` that applies to any ordered type, traits with logical values or contracts, programmable comparisons with their laws and decidable equality, and checking generic code once for logical instantiations: [#12](https://github.com/nikhilgarg28/locus/issues/12).
- How proofs are constructed: using definitions, rewriting, tactics, the interface to search, and shorter forms: RFC 0003, [#15](https://github.com/nikhilgarg28/locus/issues/15).
- Storing proofs that were found, and what proof search may do: [#13](https://github.com/nikhilgarg28/locus/issues/13).
- Claims that runtime functions terminate: [#14](https://github.com/nikhilgarg28/locus/issues/14).
- How the kernel provides `Int`: the kernel RFC.
- Mutable references stored in data, whose model carries a current and a final value.
