# RFC 0002: The logical layer

- **Status:** proposed
- **Issue:** [#4](https://github.com/nikhilgarg28/locus/issues/4), and parts of [#5](https://github.com/nikhilgarg28/locus/issues/5) and [#6](https://github.com/nikhilgarg28/locus/issues/6)
- **Supersedes:** none

## Summary

This RFC defines what Locus adds to Rust:
- logical types, propositions, and proofs;
- logic functions and logic blocks;
- models, which say what runtime data means in the logic;
- contracts on runtime functions, including what a `&mut` parameter means in one.

The design in brief:
- **Logical types** describe values that exist only for checking:
  - `Nat`, the language's one built-in data type;
  - propositions, of type `Prop`, and proofs of them, of type `@P`;
  - logical functions;
  - data declared with `#[derive(Logical)]`.

  Locus's library builds `Int`, `Bool`, `Seq`, and the rest from these.
- **Proofs and logical values are ghost.** They never exist at run time, and they never reach rustc as values: both programs of [RFC 0001](0001-compiler-pipeline.md) leave them out. So no Rust code, generic or not, can copy, convert, or forge a proof.
- **Logical code is pure and total.** This covers logic functions, `logic!` blocks, propositions, and proofs. It reads runtime values through their **models**, and never calls runtime code.
- **A runtime function states its contract in its signature.** Proof parameters are preconditions, and proofs in the result are postconditions.
  - In a signature, every parameter means its value at entry.
  - For a `&mut` parameter `x`, `final!(x)` is the value its referent has when the borrow ends. This is a prophecy in the sense of RustHorn and Creusot.
- **Proofs can be carried in data** in two ways: as invariants of the program's own types, or as the payload of an `Option` or `Result`, whose tag stays at run time.

## Motivation

[RFC 0001](0001-compiler-pipeline.md) decides how Locus checks Rust and erases its own additions, but not what those additions mean. Three issues wait on a common core:
- logical types and models ([#4](https://github.com/nikhilgarg28/locus/issues/4));
- syntax ([#5](https://github.com/nikhilgarg28/locus/issues/5));
- runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)).

That core is what a proof is, how logical code sees runtime data, and how a function promises something about what it returns or changes.

A second reason is soundness. RFC 0001's previous draft gave every proof the same zero-sized Rust type. A review found that safe, ordinary Rust could then turn a proof of one proposition into a "proof" of another:

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

At run time both proof types were the same Rust type, so the downcast succeeded. The narrow fix was a rule about `'static` bounds, which still relied on Rust generics being parametric everywhere else. This RFC instead removes proofs from the programs rustc compiles altogether. That closes the hole by construction: no Rust value exists that could be converted.

The design follows [v0](https://github.com/nikhilgarg28/locus_v0) where v0 settled a question:
- logical types classified by type;
- snapshots;
- propositions with named arms;
- `Option` with a proof payload;
- canonical models;
- invariants in proof fields.

It departs from v0 where being a superset of Rust, or the ghost representation, requires it. Each departure is listed under [Alternatives](#alternatives).

## Design

### A first look

```rust
logic fn Sorted(xs: Seq<Nat>) -> Prop {
    forall |i: Nat, j: Nat| i <= j && j < xs.len() ==> xs[i] <= xs[j]
}

fn binary_search(xs: &[u32], target: u32, sorted: @Sorted(xs)) -> Option<usize> { ... }
fn linear_search(xs: &[u32], target: u32) -> Option<usize> { ... }

fn find(xs: &[u32], target: u32, sorted: Option<@Sorted(xs)>) -> Option<usize> {
    match sorted {
        Some(proof) => binary_search(xs, target, proof),
        None => linear_search(xs, target),
    }
}

fn bump(counter: &mut u32, room: @(counter < 100)) -> @(final!(counter) == counter + 1) {
    *counter += 1;
    _
}
```

- `Sorted` is a logic function that defines a proposition about a sequence of naturals. A `&[u32]` passed to it is seen through its model, a `Seq<Nat>`.
- `binary_search` needs a proof that `xs` is sorted. Calling it on unsorted data is a compile-time error.
- `find` takes a proof that may be absent. At run time it still knows whether one was passed.
- `bump` promises that the counter's final value is one more than its value at entry, given room below 100. `_` asks proof search for the proof, and the kernel checks it.

The runtime program is what remains:

```rust prose generated Rust
fn binary_search(xs: &[u32], target: u32) -> Option<usize> { ... }
fn linear_search(xs: &[u32], target: u32) -> Option<usize> { ... }

fn find(xs: &[u32], target: u32, sorted: Option<()>) -> Option<usize> {
    match sorted {
        Some(_) => binary_search(xs, target),
        None => linear_search(xs, target),
    }
}

fn bump(counter: &mut u32) {
    *counter += 1;
}
```

### Logical types

Every type is either a runtime type or a logical type.

**Runtime types** are Rust's types:
- the program's own structs and enums, including those with logical fields;
- Rust's generic types, whatever their arguments.

**Logical types** have no runtime representation:
- `Nat`, the natural numbers, the one logical data type the language provides;
- `Prop`, the type of propositions;
- `@P`, the type of proofs of the proposition `P`;
- logical function types, written `logic fn(A) -> B`;
- tuples whose components are all logical;
- structs and enums declared with `#[derive(Logical)]`, which may be recursive without `Box` and describe finite values;
- the logical types of Locus's library, which it builds from these: `Int`, `Bool`, `Seq<T>`, `Set<T>`, `Map<K, V>`, `Maybe<T>`, and others.

**A logical type contains only logical types.** Its fields, payloads, and type arguments are logical. `Seq<Nat>` is a logical type, and `Seq<u32>` is an error. A runtime value enters logical data through its [model](#models).

**Classification belongs to the type, not the binding.** There is no `ghost` keyword: a parameter, result, local, or field is ghost exactly when its type is logical.

### Ghost values

Values of logical types, proofs included, exist only for checking. They are *ghost*: both programs of RFC 0001 leave them out, and a program behaves as if they were absent.

They may appear:
- in logical code;
- as parameters, result components, locals, and fields of logical type, in runtime functions and in the program's own types;
- as the payload of an `Option` or `Result`, for proofs, in the positions described under [Proofs in data](#proofs-in-data).

Four rules keep them ghost:
1. **Never a type argument.** A logical type is never a type argument of a runtime type, and never instantiates a generic parameter of runtime code. That includes the program's own generic functions, whose bodies could call code that tells types apart. So `Vec<@P>`, `Vec<Nat>`, `identity::<@P>(h)`, and the `convert` call above are rejected. This keeps every proof out of every Rust generic, and so out of reach of `Any`, of specialization, and of any other way Rust code could tell types apart or make values.
2. **Never a runtime decision.** Runtime `if`, `match`, `while`, and the other control flow never depend on a logical value. A match on an `Option<@P>` tests its runtime tag, not the proof.
3. **Arguments of logical type are logical code.** A proof or logical argument is written in logical code, which does no runtime evaluation, so removing it removes nothing that runs. A runtime function that returns a proof is called in a statement or `let` of its own. Write `let even = checked_even(n); halve(n, even)`, not `halve(n, checked_even(n))`.
4. **A call is kept even when its result is ghost.** A call to a runtime function whose whole result is ghost stays in the runtime program, as a statement.

### Logical code

Logical code is:
- the bodies of logic functions and of proposition declarations;
- the contents of `logic! { ... }`, `prop!(...)`, and `prove!(...)`;
- propositions in types, written `@(...)`, and proof holes `_`;
- arguments for parameters of logical type.

Logical code is pure, total, and deterministic. It never calls runtime functions or methods, never mutates runtime state, and has no effects. It may read runtime places, which RFC 0001 checks as [observations](../../spec/observation.md), and it may call logic functions.

**Reading runtime values.**
- **Place expressions keep their Rust meaning.** A place is a local or parameter, a field or tuple field visible at that point, or the target of a reference or `Box`. Indexing and overloaded `Deref` do not form places in logical code.
- **A runtime value used as a logical value is its type's model.** That covers an operand, an argument of logical type, a method receiver, and either side of `==`. If the type has no model, the value is *opaque*: logical code can compare it for equality and pass it on, and nothing else.
- **Methods and indexing work on models.** A method call or index on a runtime value works on its model, never on a runtime method, as RFC 0001 requires. For `v: &Vec<u32>`, `v.len()` is `Seq::len` of `v`'s model, and `v[i]` indexes that model.
- **References are transparent.** A `&T`, `&mut T`, or `Box<T>` used as a value is the current value of its target. For a `&mut` parameter in a signature, "current" means at entry; see [Contracts](#contracts).

**Logical expressions.**
- **Arithmetic** on `Nat` and `Int` is exact: it never overflows.
  - `Nat` follows Lean: subtraction stops at zero, and division by zero gives zero.
  - An integer literal is a `Nat` unless it is negative or an `Int` is expected.
  - `n as Int` converts a `Nat` to an `Int`.
- **Comparisons and `==` are propositions.** `==` is equality of logical values, Lean's `Eq`.
- **Connectives:** `&&`, `||`, and `!` combine propositions, and `==>` is implication.
- **Quantifiers:** `forall |x: T| P` and `exists |x: T| P`, over logical types `T`.
- **`if` and `match` choose between logical values.** `if` accepts any proposition, because classical logic decides every proposition (Lean's `Classical.propDecidable`). A `Bool` used as a proposition means that it is true.
- **`let`** binds a logical value.
- **Closures**, such as `|x: Nat| x + 1`, are values of logical function types.

**Logic functions** are declared as `logic fn name(parameters) -> T { body }`.
- The result type is logical. Each parameter is logical, or a runtime type that the function observes.
- The body is logical code. Logic functions never run, so they need not be computable.
- **They are total.** Every recursive call is on a structurally smaller part of a parameter, exposed by a `match`, or decreases a measure declared with `#[decreases(e)]`, where `e` is a `Nat`. Each such decrease is a proof obligation. Mutual recursion is left for later.
- **Their type parameters range over logical types,** unless bounded by `Model`, which lets them stand for runtime types seen through their models.
- **They can belong to the program's own types,** in inherent `impl` blocks, and are then called as methods in logical code.
- **A logic function that returns a proof is a lemma:** `logic fn add_zero(n: Nat) -> @(n + 0 == n)`.

**Logic blocks.** `logic! { e }` in a runtime body evaluates logical code at that point and gives a ghost value, usually bound to a name: `let before = logic! { v.len() };`.

### Models

A model maps a runtime type to the logical type that describes it:

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
```

Here `stack.items` is a `Vec<u32>` used as a logical value, so it means the vector's model, a `Seq<Nat>`.

- **A type has at most one model.** A program that wants another view of a type writes a logic function for it.
- **A model is declared by the crate that defines the type,** following Rust's orphan rule, or by Locus's library for the standard library's types. Values of other types are opaque in logic, apart from their visible fields.
- **The models of standard types** are declared by Locus's library, and each is trusted:
  - unsigned integers, `Nat`; signed integers, `Int`; `bool`, `Bool`;
  - `&T`, `&mut T`, and `Box<T>`, the model of `T`;
  - `[T; N]`, `[T]`, and `Vec<T>`, a `Seq` of `T`'s model;
  - `Option<T>`, a `Maybe` of `T`'s model;
  - a tuple, the tuple of its components' models.

  Others, such as `String`, `char`, maps, `Rc`, and `Arc`, are for the library's RFC.
- **A model reads only what cannot change while the value is shared.** More generally, the *logical value* of a runtime value is the part of it that Rust freezes under a shared borrow: everything except the contents of `UnsafeCell`s and the targets of raw pointers.
  - Models are functions of logical values, and opaque values are logical values.
  - So `Cell`, `RefCell`, `Mutex`, and the atomics have no model of their contents.
  - So a call that Locus knows nothing about, taking `&x`, never changes `x`'s logical value, as RFC 0001 relies on.
- **`model(x)`** is a logic function in Locus's library that gives `x`'s model explicitly, for example to compare two values of a struct by their models.
- **In Rust's terms, `Model` has no methods,** as `compat.names.no-methods` requires. Its function takes the value as an ordinary argument, and is called either through `model(x)` or implicitly.
- **In generic code, a type parameter `T` without a `Model` bound is opaque.** Facts proven about its values hold at every instantiation, through the instantiating type's model when it has one. With `T: Model`, logical code can use `T::Logic`.
- **Models are interpretations.** Declaring one proves nothing about any operation. What runtime operations do to models is the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)), together with the specifications of the standard library that Locus's library declares.

### Propositions

`Prop` is the type of propositions. Forming one proves nothing; `2 + 2 == 5` is a proposition.

- **Writing propositions.**
  - `prop!(e)` in a runtime body forms a proposition value.
  - `@(e)` is the type of proofs of `prop!(e)`.
  - `@P` is the type of proofs of `P`, for any expression `P` of type `Prop`, such as `@Sorted(xs)`.
- **Defined predicates** are logic functions returning `Prop`, such as `Sorted` above. A proof uses the definition explicitly, through `unfold!` and `fold!`.
- **Inductive propositions** list the ways they can be proven, as named arms:

  ```rust
  prop Even(n: Nat) {
      Zero => { n == 0 }
      Step(m: Nat) => { n == m + 2 && Even(m) }
  }
  ```

  - Each arm is a constructor: `Even::Step(2) @ h` takes the witness `2` and a proof `h` of the arm's body, `4 == 2 + 2 && Even(2)`, and proves `Even(4)`.
  - Arms may have no witnesses, tuple witnesses, or named witnesses, as enum variants do.
  - A recursive reference must be strictly positive. It may appear in conjunctions, disjunctions, quantifiers, and the conclusion of an implication, but never under a negation or in an implication's premise.
  - This is Lean's inductive propositions, written in Rust's shape.
- **Snapshots.** A proposition reads runtime places where it is formed, and is about the values it read there. Later assignment, mutation, moves, or the end of a scope never change it. A proof of it therefore remains a proof about those values. To state the same formula about later values, prove it again:

  ```rust
  let mut n: u8 = 3;
  let positive: @(n > 0) = _;
  n = 0;
  let again: @(n > 0) = positive; // rejected: `positive` is about `n` before the assignment
  ```

  `let before = logic! { n };` names a snapshot explicitly. A proposition in a body is formed where RFC 0001 places its observation. A proposition in a signature is formed at entry.

### Proofs

A proof is a value of a proof type. Proofs are irrelevant: any two proofs of a proposition are equal. They are ghost, so they are never moved or dropped, and can be used any number of times.

| Form | Meaning |
|---|---|
| `_` | A hole: proof search finds a proof of the expected proposition |
| `prove!(P)` | A hole whose proposition is written out |
| `lemma(arguments)` | A call of a logic function that returns a proof |
| `Even::Step(m) @ h` | A constructor of an inductive proposition: its witnesses, then after `@` a proof of the arm's body |
| `match h { Even::Zero @ z => ..., Even::Step(m) @ k => ... }` | Reasoning by cases on a proof |
| `fold!(f, h)`, `unfold!(f, h)` | Using the definition of a logic function |
| `rewrite!(e, h)` | Transporting a proof along a proof `e` of an equality |

Locus's library defines `True`, `False`, `And`, `Or`, `Not`, `Exists`, `ForAll`, and `Eq` as inductive propositions. The connectives and quantifiers are spellings of them.

- **A match on a proof produces a proof.** A witness inside a proof, such as an existential's, can be used to build other proofs, but never becomes a logical or runtime value. This is Lean's rule for eliminating propositions; Lean's exceptions for propositions with at most one proof can come later.
- **Search proposes, and the kernel decides.**
  - Holes and `prove!` are filled by untrusted search: automation, decision procedures, solvers, or AI.
  - A candidate counts only once the kernel checks it.
  - Proofs that were found are stored with the crate, so that checking never depends on finding them again ([vision](../../VISION.md), commitment 3). The format is a later RFC.
- **The logic is Lean's.** The kernel checks proofs in Lean 4's type theory, which has:
  - an impredicative, proof-irrelevant `Prop`;
  - inductive types and propositions;
  - propositional extensionality, quotients, and classical choice.

  Each result can report the axioms it depends on (commitment 4). The kernel RFC defines the kernel.

### Contracts

A runtime function states its contract in its signature.

- **Preconditions are proof parameters:** `fn halve(n: u32, even: @(n % 2 == 0)) -> u32`. A caller supplies a proof of the proposition with its own arguments substituted, about their values when the call is made.
- **Postconditions are proofs in the result.**
  - A function that returns only a proof is written `-> @P`.
  - A named result is written `-> (out: u32, @(out <= n))`. Its names are in scope in the propositions that follow them.
- **Values in a signature are values at entry.** In a signature's propositions, before or after the arrow, every parameter means its value when the function is called. For a `&mut` parameter `x`, that is its referent's value at entry, whether written `x` or `*x`.
- **`final!(r)` is a mutable reference's final value.** For any mutable reference `r`, `final!(r)` is the value its referent has when the borrow ends, that is, when neither `r` nor any reborrow of it is used again. This holds wherever `r` comes from: a parameter, a local, a result, or a field. It is a *prophecy*: a logical value that is fixed when the borrow is created and becomes known when the borrow ends.
  - **When the function returns no reborrow of a `&mut` parameter,** the borrow ends when the function returns, so `final!(x)` is the value at return. That is how a function states what it did, as `bump` does above.
  - **When a function returns a reborrow,** `final!` relates the two ends:

    ```rust
    fn first_mut(pair: &mut (u32, u32))
        -> (r: &mut u32, @(r == pair.0 && final!(pair) == (final!(r), pair.1)))
    ```

  - **Inside a body,** logical code reads current values: `*x` at a point in the body is its value there. A local borrow has a final value too. When the borrow ends, the place it borrowed from holds `final!(r)`.
  - **`final!(r)` reads no place.** It is not an observation in RFC 0001's sense, so it never extends a borrow.
  - **There is no `old!`.** A signature already means entry values. A body that needs one later names a snapshot: `let start = logic! { *x };`.
  - **Soundness.** RustHornBelt proved prophecies sound for safe Rust, and Creusot is built on them. Using them needs to know where each borrow ends, which rustc's borrow checker computes on MIR. How Locus obtains that is [#6](https://github.com/nikhilgarg28/locus/issues/6).
- **Partial correctness.** A postcondition holds when the function returns. A function that panics or loops forever proves nothing, so `fn never() -> @False { loop {} }` is accepted. A logic function cannot be written that way, because logic functions are total. That a runtime function terminates is a separate claim, for a later RFC.
- **Calls.** After a call returns, the caller has the proofs in its result, about the arguments it passed: `let (out, bounded) = clamp(n);`.
- **Ghost parameters.** A runtime function may take parameters of logical types, such as `n: Nat`, to state a contract in terms of a logical value that the caller supplies.
- **Trait methods with contracts.** A trait's contract binds every implementation. An implementation outside the crate cannot be checked, so such a trait is `unsafe` to implement from Rust; the details are [#7](https://github.com/nikhilgarg28/locus/issues/7).

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
  - each construction must prove them, including constructions in code that macros and derives generate;
  - a field that a proof field reads is never assigned, or mutably borrowed, on its own; the whole value is replaced instead, as v0 required.
- **Reading a proof field is logical code:** `iv.ordered` is a proof of `iv.lo <= iv.hi`.
- **This holds even for values that pass through code Locus does not check.**
  - The fields such proofs read are not public in Rust (RFC 0001), and only checked code constructs the type.
  - So unchecked code can only move, copy, or drop a value, or call the type's own checked functions.
  - This is how invariants become the fourth way facts enter the logic, in RFC 0001's sense.
- **Logical data fields,** such as `count: Nat`, are ghost too. Assigning one is a ghost statement, which is erased.

**`Option` and `Result` with proof payloads.** The following keep their runtime tag and data, and lose their proofs:
- `Option<@P>`, which is `Option<()>` in Rust;
- `Result<T, @P>` and `Result<@P, E>`, which are `Result<T, ()>` and `Result<(), E>`;
- payloads that are named tuples with proofs, such as `Option<(i: usize, @(xs[i] == target))>`, which is `Option<usize>`.

These types are allowed only as the declared type of a parameter, result, local, or field. That is the exception to the first ghost rule.

- **A value of such a type carries a proposition:** if it is `Some`, then `P` holds. Building `Some(h)` needs `h: @P`, and matching `Some(h)` gives back `h: @P`.
- **Passed to a place that expects an ordinary type,** such as a generic parameter, it is only an `Option<()>`, and whatever comes back carries no proof.
- **Nested deeper,** as in `Vec<Option<@P>>`, it is rejected for now.

### Erasure, in summary

RFC 0001 defines erasure. For this RFC's constructs:

| In the source | In the runtime program |
|---|---|
| A parameter, result component, local, or field of logical type, and every argument for one | Removed |
| A call of a runtime function whose result is entirely ghost | The call, as a statement |
| `Option<@P>`, and `Some(h)` in an expression or a pattern | `Option<()>`, and `Some(())` or `Some(_)` |
| A named result `(out: T, @P)` | `T` |
| Logic functions, proposition declarations, `#[derive(Logical)]` types, `impl Model` | Removed |
| `logic!`, `prop!`, `prove!`, `_`, `final!` | Removed with the ghost value they produce |

### What changes in RFC 0001

Ghost proofs change parts of RFC 0001, which this PR revises alongside this RFC so that the two are decided together:
- **No stand-ins.** The checking and runtime programs contain no stand-in types. Logical items become stubs in the checking program only, for resolving names.
- **The fourth way facts enter the logic** is the invariants of the program's own types, as described above. This replaces the earlier `'static` rule.
- **The export surface follows from the runtime program.** Functions there already lack proofs, so no wrappers are needed. A public function with a precondition is `unsafe`.

## Rules

The rules below go into four new chapters of the specification: [The logical layer](../../spec/logical-layer.md), [Models](../../spec/models.md), [Propositions and proofs](../../spec/proofs.md), and [Contracts](../../spec/contracts.md).

| Rule | Examples |
|---|---|
| `logic.types.contents-logical` | `logical-layer/logical-fields.accept.lc`, `logical-layer/logical-type-with-runtime-field.reject.lc`, `logical-layer/sequence-of-runtime-type.reject.lc` |
| `logic.ghost.erased` | `logical-layer/ghost-parameters.accept.lc`, `logical-layer/runtime-call-bound-first.accept.lc` |
| `logic.ghost.not-type-argument` | `logical-layer/proof-in-vec.reject.lc`, `logical-layer/proof-through-generic.reject.lc`, `logical-layer/proof-through-any.reject.lc`, `contracts/proof-in-option.accept.lc` |
| `logic.ghost.no-runtime-control` | `logical-layer/branch-on-logical.reject.lc`, `contracts/proof-in-option.accept.lc` |
| `logic.ghost.arguments-logical` | `logical-layer/runtime-call-in-proof-argument.reject.lc`, `logical-layer/runtime-call-bound-first.accept.lc` |
| `logic.code.no-runtime-calls` | `logical-layer/logic-calls-runtime-function.reject.lc`, `logical-layer/logic-calls-logic-function.accept.lc` |
| `logic.fn.total` | `logical-layer/structural-recursion.accept.lc`, `logical-layer/measure-recursion.accept.lc`, `logical-layer/nonterminating-logic-fn.reject.lc`, `logical-layer/measure-increases.reject.lc`, `proofs/diverging-logic-proof.reject.lc` |
| `models.observation` | `models/integer-model-no-overflow.accept.lc`, `models/wrapping-is-not-the-model.reject.lc`, `observation/before-move.accept.lc`, `observation/after-mutable-borrow-ends.accept.lc` |
| `models.one-per-type` | `models/hand-written-model.accept.lc`, `models/two-models.reject.lc` |
| `models.declared-by-owner` | `models/hand-written-model.accept.lc`, `models/model-for-foreign-type.reject.lc` |
| `models.frozen-data` | `models/hand-written-model.accept.lc`, `models/model-reads-cell.reject.lc` |
| `proofs.snapshot` | `proofs/snapshot-survives-assignment.accept.lc`, `proofs/stale-proof-after-assignment.reject.lc` |
| `proofs.elimination` | `proofs/witness-stays-in-proof.accept.lc`, `proofs/witness-to-data.reject.lc` |
| `proofs.props.positive` | `proofs/positive-proposition.accept.lc`, `proofs/negative-proposition.reject.lc` |
| `proofs.partial` | `proofs/diverging-runtime-proof.accept.lc`, `proofs/diverging-logic-proof.reject.lc` |
| `contracts.entry-values` | `contracts/final-value-at-return.accept.lc`, `contracts/entry-value-is-not-final.reject.lc` |
| `contracts.final` | `contracts/final-value-at-return.accept.lc`, `contracts/returned-borrow.accept.lc`, `contracts/local-borrow-final.accept.lc`, `contracts/wrong-final.reject.lc` |
| `contracts.invariants` | `contracts/invariant-constructed.accept.lc`, `contracts/invariant-replaced-whole.accept.lc`, `contracts/invariant-field-assigned.reject.lc`, `contracts/invariant-default-derive.reject.lc` |
| `contracts.option-proof` | `contracts/proof-in-option.accept.lc`, `contracts/some-without-proof.reject.lc` |

The examples use this RFC's syntax. Where #5 later respells something, their outcomes stay the same.

## Interactions

| Area | Effect |
|---|---|
| Types | Every type is runtime or logical, by its type. Logical types contain only logical types, and never become type arguments of runtime types, except for proofs in `Option` and `Result`. |
| Propositions and proofs | Propositions are Lean's, with Rust's syntax: inductive propositions with named arms, defined predicates, connectives, and quantifiers. Proofs are irrelevant, ghost, and kernel-checked. |
| Ownership and mutation | Logical code reads places under RFC 0001's observation rules. Propositions are snapshots. A `&mut` reference has a current value and a final value, `final!`. How runtime code changes values in the logic is [#6](https://github.com/nikhilgarg28/locus/issues/6). |
| Erasure and runtime behavior | Ghost values are removed from both programs. Calls of runtime functions are kept even when their results are ghost. `Option` and `Result` keep their tags. |
| Rust interoperability | Rust code never sees a proof. Functions with preconditions are `unsafe` at the Rust boundary, fields that invariants read are private, and traits with contracts are `unsafe` to implement ([#7](https://github.com/nikhilgarg28/locus/issues/7)). Models are declared by a type's own crate. |
| Diagnostics | New errors: logical values where runtime ones are needed, runtime calls in logical code, non-terminating logic functions, stale proofs about earlier values, and invariants not established. They are written as reasons until error codes exist. |
| Trust model | New: the models and specifications that Locus's library declares for standard types, the consistency of Lean's axioms, and `unsafe` code preserving invariants. The logical pass's ghost checks replace RFC 0001's checks on proofs in runtime code. Where borrows end joins rustc's borrow-checker entry. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Proofs as zero-sized values in Rust types:** RFC 0001's previous draft, and v0's `Erased` marker. Proofs would flow through generic Rust code for free, through `Option::take`, `Vec`, and iterators.
- Rejected. With one Rust type for every proposition, Rust code that can tell types apart, through `Any` or specialization, could turn one proof into another.
- rustc would also compute types involving proofs, such as associated types and inferred arguments, without knowing their propositions, so Locus would have to redo that work itself.
- Ghost proofs avoid both problems, and leave signatures cleaner.

**Distinct Rust types per proposition.** Not possible: propositions mention runtime values.

**v0's `old!(x)` for the entry value and bare `x` for the value at return.** Rejected for one rule: a signature means entry values, and `final!` names the final value. This also covers borrows that are returned, kept in locals, or stored, which `old!` cannot express.

**Several models per type, chosen with `x as M`** (an earlier v0 decision). Rejected, as v0 later did: one canonical model makes `v.len()` in logical code unambiguous. Other views are logic functions.

**Models before field access**, as in v0, where logical code observed the receiver through its model before selecting a field. Rejected: keeping Rust's meaning for places makes logical code read like Rust and works for types without models. Models apply where a value is used.

**Boolean comparisons with `prop!` lifting them to propositions** (v0). Rejected in favor of Lean's propositional comparisons, with a classical `if`. This means there is one kind of comparison.

**Subtraction on `Nat` that needs a proof that the result is not negative** (v0). Rejected in favor of Lean's truncating subtraction, which keeps arithmetic total. Specifications must take care with `len() - 1`, as they must in Lean.

**`=>` for implication** (v0). Rejected: inside a logical `match`, `=>` already separates an arm from its body, so `==>` is clearer.

**`forall (x: T) { F }` keywords** (v0). Rejected for `forall |x: T| F`, which reads like Rust's closures and needs no new delimiters.

**A `ghost` keyword or a `Ghost<T>` wrapper**, as in Verus. Rejected: the type already says whether a value is ghost.

**Tracked evidence, a `let mut h: @P` that follows the current value of what it reads** (v0). Deferred to the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)), together with loops, where it serves as a loop invariant.

**Proofs through generic code by parametricity,** with a rule against `'static` bounds. Rejected: that relies on every Rust generic being parametric, which specialization and `unsafe` code do not guarantee.

## Prior art

- **Lean 4** is the model for the logic, the kernel, inductive propositions, and proof irrelevance.
- **Creusot** has logic functions, a mutable reference's current value `*x` and final value `^x`, and models through its `View` trait. Its handling of `&mut` is the model for `final!`.
- **RustHorn and RustHornBelt** (Matsushita et al., ESOP 2020 and PLDI 2022) gave prophecies for Rust's mutable borrows and proved them sound.
- **Verus** separates spec, proof, and exec code, erases ghost code completely, and models types with `view()`.
- **Prusti** writes entry values with `old(...)`, the convention this RFC replaces with entry-by-default and `final!`.
- **Dafny** has ghost variables and fields, `decreases` clauses, and erasure of ghost code.
- **F\* with Low\* and KaRaMeL** erases proofs to produce C, as Locus erases them to produce Rust.
- **v0** decided much of this design, as described above.

## Unresolved questions

**Decisions for the owner,** in order of what they block:
1. **Ghost proofs:** proofs and logical values never exist in either program, which closes the `Any` hole by construction. This was the owner's suggestion; this RFC works it out.
2. **The `&mut` convention:** entry values in signatures, and `final!(r)` for the value when the borrow ends, with no `old!`. Also the owner's.
3. **One model per type,** declared by the type's own crate or by Locus's library.
4. **Places keep their Rust meaning in logical code,** and models apply to values.
5. **Lean's propositional comparisons,** with a classical `if`, and truncating `Nat` subtraction.
6. **Syntax:** `==>`, `forall |x: T|`, `Arm(witnesses) @ proof`, `#[decreases(e)]`, and `final!`.
7. **A runtime call that returns a proof is written in its own statement or `let`,** rather than as an argument.
8. **No `#[derive(Model)]` for now.** A struct without a model is still readable field by field.

**Left to later work,** each with its issue:
- The runtime semantics: how runtime operations change values and models, loops and invariants, tracked evidence, where borrows end, and overflow: [#6](https://github.com/nikhilgarg28/locus/issues/6).
- Remaining syntax questions: [#5](https://github.com/nikhilgarg28/locus/issues/5).
- Distribution, unsafe exports, and traits with contracts: [#7](https://github.com/nikhilgarg28/locus/issues/7).
- Closures and async code, including closures that take or return proofs: [#8](https://github.com/nikhilgarg28/locus/issues/8).
- Locus's standard library: logical types, and models and specifications of standard types: [#11](https://github.com/nikhilgarg28/locus/issues/11).
- Traits and operators on logical types: [#12](https://github.com/nikhilgarg28/locus/issues/12).
- Storing proofs that were found, and what proof search may do: [#13](https://github.com/nikhilgarg28/locus/issues/13).
- Claims that runtime functions terminate: [#14](https://github.com/nikhilgarg28/locus/issues/14).
- Mutable references stored in data, whose model carries a current and a final value.
