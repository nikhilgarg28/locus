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
  - Both programs of [RFC 0001](0001-compiler-pipeline.md) remove them, or replace them with `()` where a runtime type holds them. So `Vec<Nat>` is a `Vec<()>` at run time, and keeps its length.
  - A proof parameter becomes a *token* that only code Locus checked can supply. So a function cannot be called without meeting its precondition.
  - Locus believes what a value holds only through code it checked, or through standard library code whose specification it has. Everywhere else ghost contents are forgotten, so no Rust code can forge or convert a proof.
- **Logical code is pure and total.** This covers logic functions, `logic!` blocks, propositions, and proofs. It reads runtime values through their **models**, and never calls runtime code.
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

A second reason is soundness. RFC 0001's previous draft gave every proof the same zero-sized Rust type, and believed a value's proposition from its type. A review found that safe, ordinary Rust could then turn a proof of one proposition into a "proof" of another:

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

At run time both proof types were the same Rust type, so the downcast succeeded, and Locus believed the result because of its type. This RFC removes the belief rather than the type. Locus tracks proofs itself, and keeps track of what a value holds only through code whose meaning it knows. Code like `downcast`, whose meaning it does not know, gives back `()`, and `()` proves nothing.

The design follows [v0](https://github.com/nikhilgarg28/locus_v0) where v0 settled a question:
- logical types classified by type;
- versions and snapshots;
- propositions with named arms;
- logical values held in runtime containers;
- canonical models;
- invariants in proof fields;
- checking generic code at each logical instantiation.

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

The runtime program is what remains. `Proof` and `PROOF` are the token type and its value, in the crate's generated module:

```rust prose generated Rust
fn binary_search(xs: &[u32], target: u32, _sorted: crate::locus_erased::Proof) -> Option<usize> { ... }
fn linear_search(xs: &[u32], target: u32) -> Option<usize> { ... }

fn find(xs: &[u32], target: u32, sorted: Option<()>) -> Option<usize> {
    match sorted {
        Some(_) => binary_search(xs, target, crate::locus_erased::PROOF),
        None => linear_search(xs, target),
    }
}

fn bump(counter: &mut u32, _room: crate::locus_erased::Proof) {
    *counter += 1;
}
```

### Syntax and names

This RFC adds these forms to [RFC 0001's](0001-compiler-pipeline.md#locus-syntax-and-locus-names), under the same rule that Locus syntax appears only where rustc rejects the tokens:

| Form | Where | Example | Why Rust cannot mean it |
|---|---|---|---|
| Proof constructor | Expression | `Even::Step(2) @ h` | Rust has no binary `@` in expressions |
| Proof pattern | Pattern | `Even::Step(m) @ k` | Rust's `@` binding needs an identifier on its left, not a path |
| Final value | Expression | `final!(r)` | `final` is a reserved word, so Rust can neither define nor invoke a macro of that name |
| Logical function type | Type | `logic fn(Int) -> Prop` | Rust has no type beginning `logic fn` |
| Measure | Attribute of a logic function | `#[decreases(n)]` | It appears only on a logic function, which is itself Locus syntax |

Inside logical code, which Locus parses itself, it also uses `==>`, `forall |x: T| P`, and `exists |x: T| P`.

It adds these names to Locus's prelude, which programs shadow as they do std's (`compat.names.shadowable`):
- the macros `fold!`, `unfold!`, and `rewrite!`;
- the logic function `model`;
- the logical types `Int`, `Nat`, `Bool`, `Seq`, `Opaque`, and the others of Locus's library;
- the propositions `True`, `False`, `And`, `Or`, and `Exists`.

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
- tuples whose components are all logical;
- structs and enums declared with `#[derive(Logical)]`, which may be recursive without `Box` and describe finite values;
- `Opaque<T>`, for each runtime type `T` without a model: the values of `T`, which logical code can compare for equality and pass on, and whose visible fields it can read;
- the logical types of Locus's library, which it builds from these: `Nat`, `Bool`, `Seq<T>`, `Set<T>`, `Map<K, V>`, `Maybe<T>`, and others ([#11](https://github.com/nikhilgarg28/locus/issues/11)).

`Nat` is an `Int` with a proof that it is not negative:

```rust prose library definition
#[derive(Logical)]
struct Nat {
    value: Int,
    nonnegative: @(value >= 0),
}
```

**A logical type contains only logical types.** Its fields, payloads, and type arguments are logical. `Seq<Nat>` is a logical type, and `Seq<u32>` is an error. A runtime value enters logical data through its [model](#models).

**A logical type refers to itself only positively.** A recursive `#[derive(Logical)]` type, or group of types, may contain itself as a field, a payload, a type argument of a positive logical type, or the result of a logical function type, but never as the parameter of a logical function type. Otherwise `struct Bad { f: logic fn(Bad) -> @False }` would prove `False` without recursion. This is Lean's strict positivity, the same rule as for [inductive propositions](#propositions).

**Classification belongs to the type, not the binding.** There is no `ghost` keyword: a parameter, result, local, or field is ghost exactly when its type is logical.

### Ghost values

Values of logical types, proofs included, exist only for checking. They are *ghost*. They may appear wherever a value of their type can:
- in logical code;
- as parameters, result components, locals, and fields of logical type;
- inside runtime types, as type arguments: `Option<@P>`, `Vec<Nat>`, `Result<u32, @P>`, or `Vec<(i: usize, @(i < n))>`.

**Representation.** No ghost value has a runtime representation. Both programs of RFC 0001 change them the same way:
- **Ghost positions are removed.** A parameter, result component, local, or field of logical type is removed, with every argument for it. The one exception is a proof parameter, which becomes a token (below).
- **Inside runtime types, logical types become `()`.**
  - A tuple loses its logical components, and a tuple left with one component becomes that component.
  - So `Option<@P>` is `Option<()>`, `Vec<Nat>` is `Vec<()>`, and `Vec<(usize, @P)>` is `Vec<usize>`.
  - Where Rust needs a value of such a type, the ghost value is `()`: `v.push(n)` becomes `v.push(())`.
- **The runtime parts of a type are all kept.** An `Option` keeps its tag, and a `Vec` its length.
- **A reference to a logical value is that value.** Matching `&Option<@P>` with `Some(p)` binds `p` to the proof.

**Runtime code runs as written.** Erasure removes logical code and nothing else.
- A `let`, block, `if`, or `match` whose value is ghost keeps its runtime calls, conditions, and statements, and its patterns keep their runtime parts.
- So `let Some(p) = check(n) else { return 0 };` keeps the call and the early return, as `let Some(_) = check(n) else { return 0 };`.
- And `if n % 2 == 0 { _ } else { panic!("odd") }`, as a function's result of type `@P`, keeps its condition and its panic.

**Proof tokens.** A function's proof parameters are its preconditions. Code that Locus has not checked must not be able to call the function without meeting them, so each proof parameter becomes a parameter of the crate's *proof token* type.
- **The token type** is a zero-sized type in the crate's generated module. Its constructor is not public, and its one value is `PROOF`.
- **Erasure writes the token at each call that Locus checked, and nowhere else.** Code that Locus did not check cannot make one, so it cannot call the function. That includes another crate, and a plain `cargo build` of an edited file.
- **A token means nothing.** It is never stored, returned, or passed on, and Locus never draws a fact from one. It only records that the call was checked, so it cannot be converted into a proof of anything.
- **Other crates** obtain tokens only through an `unsafe` constructor, by which the caller takes responsibility for the precondition. Rust callers use it directly; Locus crates use it at calls that Locus has checked. The details are [#7](https://github.com/nikhilgarg28/locus/issues/7).
- **Proofs anywhere else are erased as above.** A public function whose precondition sits inside a parameter's data, as in `Option<@P>`, is `unsafe` at the Rust boundary (RFC 0001).

**What survives where.** Locus tracks ghost contents itself: which proofs and logical values a runtime value holds. It tracks them only through code whose meaning it knows:
- code that Locus checks;
- standard library code whose specification Locus's library declares, such as `Vec::push`, `Option::take`, and indexing.

Everywhere else, ghost contents are forgotten:
- **Other generic code.** Passing a value to a generic parameter of any other code instantiates it with the erased type, exactly as rustc does. What comes back holds `()`, not proofs or logical values. This is why the `convert` example above is rejected: `downcast` gives back a `Box<()>`, and `()` is not a proof.
- **Specified code.** Locus's library specifies at least `Option`, `Result`, `Box`, `Vec`, arrays, and slices as code that keeps what its values hold. Their exact specifications are [#11](https://github.com/nikhilgarg28/locus/issues/11). The `?` operator on a value that holds a proof goes through the `Try` trait, and is for [#12](https://github.com/nikhilgarg28/locus/issues/12).
- **The program's own generic code** is checked again for each instantiation whose type arguments contain logical types, as v0 did. Checking it once for every such instantiation is [#12](https://github.com/nikhilgarg28/locus/issues/12).
- **Traits.** Logical types implement `Clone`, `Copy`, and `Debug`. Proof types also implement `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash`; these are right for proofs, because any two proofs of a proposition are equal. No logical type implements a trait that makes a value from nothing, such as `Default`. Locus checks these bounds where it tracks ghost contents; elsewhere the contents are forgotten anyway.

Three further rules:
1. **Never a runtime decision.** Runtime `if`, `match`, `while`, and the other control flow never depend on a logical value. A match on an `Option<@P>` tests its runtime tag, not the proof.
2. **Ghost positions hold logical code.** These are ghost positions:
   - an argument for a parameter of logical type, for a proof parameter, or for a generic parameter instantiated with a logical type;
   - the logical component of a tuple;
   - the initializer of a logical field;
   - the logical payload of a variant, such as `h` in `Some(h)`.

   Logical code does no runtime evaluation, so erasing it removes nothing that runs, and evaluation order is unchanged. A runtime function that returns a proof is called in a statement or `let` of its own. Write `let even = checked_even(n); halve(n, even)`, not `halve(n, checked_even(n))`.
3. **A call is kept even when its result is ghost.** A call to a runtime function whose whole result is ghost stays in the runtime program, as a statement.
4. **Places that hold logical values are lent mutably only where Locus can follow them.** A mutable borrow of a place whose type holds logical values may be passed only to code that Locus checks or has a specification for. Otherwise that code could write a value whose ghost contents Locus has forgotten, such as `None` turned into `Some(())` by `mem::swap`, into a place whose type promises a proof. Shared borrows are harmless, because the code cannot change the place.
5. **Functions with ghost parameters are called directly.** A function with proof or logical parameters is called by name in a `.lc` file, outside any macro's input or output. It is not used as a value, coerced to a function pointer, or passed to generic code, and it is not `async`. Calls through function values with contracts are [#12](https://github.com/nikhilgarg28/locus/issues/12), and macros are [#8](https://github.com/nikhilgarg28/locus/issues/8).

**Deciding what is logical.** Lowering must know which types are logical before rustc runs, because it removes ghost positions from the checking program.
- It resolves type paths the way rustc would, from the crate's items, imports, and aliases, so `type Count = Nat;` makes `Count` logical, and a program's own `struct Nat` is not.
- The driver then confirms each decision by the identity of the item the path resolved to, as for Locus's macros (RFC 0001). A wrong decision means lowering again.
- Lowering itself reports a ghost binding used in runtime code. So such an error is Locus's, such as "runtime code cannot branch on a logical value", rather than a rustc error about a name that lowering removed.

### Logical code

Logical code is:
- the bodies of logic functions and of proposition declarations;
- the contents of `logic! { ... }`, `prop!(...)`, and `prove!(...)`;
- propositions in types, written `@(...)`, and proof holes `_`;
- ghost arguments;
- an operator, or a call of a logic function, whose operands are all logical.

Logical code is pure, total, and deterministic. It never calls runtime functions or methods, never mutates runtime state, and has no effects. It may read runtime places, which RFC 0001 checks as [observations](../../spec/observation.md), and it may call logic functions.

**Reading runtime values.**
- **Place expressions keep their Rust meaning.** A place is a local or parameter, a field or tuple field visible at that point, or the target of a reference or `Box`. Indexing and overloaded `Deref` do not form places in logical code.
- **A runtime value used as a logical value is its type's model.** That covers an operand, an argument of logical type, a method receiver, and either side of `==`. If the type has no model, the value is *opaque*: logical code can compare it for equality and pass it on, and nothing else.
- **Methods and indexing work on models.** A method call or index on a runtime value works on its model, never on a runtime method, as RFC 0001 requires. For `v: &Vec<u32>`, `v.len()` is `Seq::len` of `v`'s model, and `v[i]` indexes that model. A logic function declared on the runtime type itself is found first, then those of its model's type.
- **Constants and const generic parameters** may appear in logical code, through their models.
- **References are transparent.** A `&T`, `&mut T`, or `Box<T>` used as a value is the current value of its target. For a `&mut` parameter in a signature, "current" means at entry; see [Contracts](#contracts).

**Logical expressions.**
- **Arithmetic is exact:** it never overflows.
  - `Int` has `+`, `-`, and `*`. Division and remainder truncate toward zero, as Rust's do, and dividing by zero gives zero.
  - A `Nat` is used as an `Int` wherever an `Int` is expected.
  - `+`, `*`, `/`, and `%` on `Nat`s give a `Nat`, and `-` gives an `Int`. So for an empty `v`, `v.len() - 1` is `-1`.
  - Turning an `Int` into a `Nat` needs a proof that it is not negative.
  - An integer literal is an `Int`, or a `Nat` where a `Nat` is expected.
  - The model of a machine integer lies in its type's range, which proofs may use: for `x: u8`, `x <= 255`. For `usize` and `isize`, the range is that of the configuration being checked.
- **Comparisons and `==` are propositions.** `==` is equality of logical values, Lean's `Eq`.
- **Connectives and quantifiers:** `&&`, `||`, `!`, `==>`, `forall |x: T| P`, and `exists |x: T| P`, over logical types `T`. See [Propositions](#propositions).
- **`if` and `match` choose between logical values.** `if` accepts any proposition, because classical logic decides every proposition (Lean's `Classical.propDecidable`). A `Bool` used as a proposition means that it is true.
- **`let`** binds a logical value.
- **Closures**, such as `|x: Int| x + 1`, are values of logical function types.

**Logic functions** are declared as `logic fn name(parameters) -> T { body }`.
- The result type is logical. Each parameter is logical, or a runtime type that the function observes.
- The body is logical code. Logic functions never run, so they need not be computable.
- **They are total.** Every recursive call is on a structurally smaller part of a parameter, exposed by a `match`, or decreases a measure declared with `#[decreases(e)]`, where `e` is a `Nat`. Each such decrease is a proof obligation. Mutual recursion is left for later.
- **Their type parameters range over logical types,** unless bounded by `Model`, which lets them stand for runtime types seen through their models.
- **They can belong to the program's own types,** in inherent `impl` blocks, and are then called as methods in logical code.
- **A logic function that returns a proof is a lemma:** `logic fn add_zero(n: Int) -> @(n + 0 == n)`.

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
- **In the checking program, `impl Model` is a real implementation** of the library's `Model` trait, with its logic function as a stub beside it. So rustc checks `T: Model` bounds, and its coherence errors, conflicting implementations (E0119) and the orphan rule (E0117), are reported as Locus's errors about models. The runtime program has no `Model`: implementations and `T: Model` bounds are removed.
- **The models of standard types** are declared by Locus's library, and each is trusted:
  - unsigned integers, `Nat`; signed integers, `Int`; `bool`, `Bool`;
  - `&T`, `&mut T`, and `Box<T>`, the model of `T`;
  - `[T; N]`, `[T]`, and `Vec<T>`, a `Seq` of `T`'s model;
  - `Option<T>`, a `Maybe` of `T`'s model;
  - a tuple, the tuple of its components' models;
  - a logical type, itself, so `Vec<Nat>` is a `Seq<Nat>`;
  - any other type `T` without a model, `Opaque<T>`, so `Vec<T>` for a generic `T` is a `Seq<Opaque<T>>`.

  Others, such as `String`, `char`, maps, `Rc`, and `Arc`, are for the library's RFC ([#11](https://github.com/nikhilgarg28/locus/issues/11)).
- **A model reads only what cannot change while the value is shared.** More generally, the *logical value* of a runtime value is the part of it that Rust freezes under a shared borrow: everything except the contents of `UnsafeCell`s and the targets of raw pointers.
  - Models are functions of logical values, and opaque values are logical values.
  - So `Cell`, `RefCell`, `Mutex`, and the atomics have no model of their contents.
  - So a call that Locus knows nothing about, taking `&x`, never changes `x`'s logical value, as RFC 0001 relies on.
- **`model(x)`** is a logic function in Locus's library that gives `x`'s model explicitly, for example to compare two values of a struct by their models.
- **In Rust's terms, `Model` has no methods,** as `compat.names.no-methods` requires. Its function takes the value as an ordinary argument, and is called either through `model(x)` or implicitly.
- **In generic code, a type parameter `T` without a `Model` bound is opaque:** its values are `Opaque<T>`. Facts proven about them hold at every instantiation, through the instantiating type's model when it has one. With `T: Model`, logical code can use `T::Logic`.
- **Models are interpretations.** Declaring one proves nothing about any operation. What runtime operations do to models is the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)), together with the specifications of the standard library that Locus's library declares.

### Versions

A place's value changes as the program runs, and logical code reads it at particular points. Locus's logic follows each binding through **versions**, as an SSA form does:

```rust
let mut n: u8 = 3;              // version n₀, with n₀ == 3
let positive: @(n > 0) = _;     // proves n₀ > 0
n = n + 1;                      // version n₁, with n₁ == n₀ + 1
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

  A write makes a new version even when it writes the value the binding already had.
- **What a version holds.**
  - The logical value of a place is that of its binding's current version.
  - A field is a projection of it.
  - Owned heap data, such as a `Box`'s target or a `Vec`'s elements, is part of it.
  - Data behind an `UnsafeCell` is part of no version (see [Models](#models)).
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
  - a version that no write separates from another is the same version;
  - inside a branch, its condition or matched pattern holds;
  - after a call returns, its postconditions hold;
  - a mutable borrow's final value is what its place holds when the borrow ends.

### Propositions

`Prop` is the type of propositions. Forming one proves nothing; `2 + 2 == 5` is a proposition.

- **Writing propositions.**
  - `prop!(e)` in a runtime body forms a proposition value.
  - `@(e)` is the type of proofs of `prop!(e)`.
  - `@P` is the type of proofs of `P`, for any expression `P` of type `Prop`, such as `@Sorted(xs)`.
- **Three ways to define one:**
  - a formula, built with connectives and quantifiers;
  - a *defined predicate*, which is a logic function returning `Prop`, such as `Sorted` above;
  - an *inductive proposition*, declared with `prop` ([below](#inductive-propositions)).
- **Snapshots.** A proposition in a body is formed where RFC 0001 places its observation, and is about the versions it read there. A proposition in a signature is about entry values. `let before = logic! { n };` names a snapshot explicitly.

**Connectives.** `&&`, `||`, `!`, and `==>` combine propositions into propositions, and `forall` and `exists` quantify them. The operands may be formulas or values of type `Prop`:

```rust
logic fn Between(low: Int, x: Int, high: Int) -> Prop {
    low <= x && x < high
}

fn check(x: u8, p: Prop, hp: @p, low: @(x > 1)) -> @(p && x > 1) {
    And::Intro(hp, low)
}
```

- **On runtime `bool`s, the same operators keep their Rust meaning.** Which meaning applies follows from the operands' types, as it does for `+` on `u8` and on `Int`.
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

A `prop` declaration defines a proposition by the ways it can be proven, as an `enum` defines a type by its variants. This is how v0 wrote predicates with named arms:

```rust
prop Reachable(from: State, to: State) {
    Same => { from == to }
    Via(middle: State) => { Transition(from, middle) && Reachable(middle, to) }
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
  - Every parameter is shared by all arms. An arm that holds only at particular values says so with an equality in its condition, as `Same` does with `from == to`. This is how Lean's indexed families are written here.
  - Matching gives the equality as a proof, which `rewrite!` uses.
- **No definition to unfold.** A declared proposition has no defining equation, so `fold!` and `unfold!` do not apply. It is proved by an arm, and used by matching. Even a declaration with one arm is not the same proposition as that arm's condition.
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
| `fold!(f, h)`, `unfold!(f, h)` | Using the definition of a logic function |
| `rewrite!(e, h)` | Transporting a proof along a proof `e` of an equality |
| `And::Intro(h, k)`, `Or::Left @ h`, `Or::Right @ k`, `True::Intro` | The connectives' constructors ([Connectives](#propositions)) |
| `Exists::Witness(v) @ h` | A proof of `exists \|x: T\| P`, from a witness `v` and a proof `h` of `P` at `v` |
| `\|x: T\| proof`, `\|h: @P\| proof` | A proof of `forall \|x: T\| Q`, or of `P ==> Q`: a logical function from each `x`, or each proof of `P`, to a proof of `Q` |
| `all(v)`, `imp(h)` | Using a proof `all` of a `forall` at `v`, or a proof `imp` of an implication on a proof `h` of its premise |

The connectives' constructors and eliminators are listed under [Propositions](#propositions).

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

- **Preconditions are proof parameters:** `fn halve(n: u32, even: @(n % 2 == 0)) -> u32`. A caller supplies a proof of the proposition with its own arguments substituted, about their values when the call is made. In both programs, the parameter is a [proof token](#ghost-values).
- **Postconditions are proofs in the result.**
  - A function that returns only a proof is written `-> @P`.
  - A named result is written `-> (out: u32, @(out <= n))`. Its names are in scope in the propositions that follow them.
- **Values in a signature are values at entry.** In a signature's propositions, before or after the arrow, every parameter means its value when the function is called. For a `&mut` parameter `x`, that is its referent's value at entry, whether written `x` or `*x`.
- **`final!(r)` is a mutable reference's final value.** For any mutable reference `r`, `final!(r)` is the value its referent has when the borrow ends, that is, when neither `r` nor any reborrow of it is used again. This holds wherever `r` comes from: a parameter, a local, or a result. Mutable references stored in data are for later. It is a *prophecy*: a logical value that is fixed when the borrow is created and becomes known when the borrow ends.
  - **When no reborrow of a `&mut` parameter can outlive the call,** the borrow ends when the function returns, after its locals are dropped, so `final!(x)` is the value at that point. That is how a function states what it did, as `bump` does above.
    - No reborrow can outlive the call when `x`'s lifetime appears in no other parameter and not in the result.
    - Otherwise, as in `fn stash<'a>(x: &'a mut u32, slot: &mut Option<&'a mut u32>)`, the caller may write through the reborrow later, so `final!(x)` is not known at return.
    - Because drops can write through a borrow, the postcondition is checked after the locals' drops. That needs typed bodies with drops made explicit ([#6](https://github.com/nikhilgarg28/locus/issues/6)).
  - **When a function returns a reborrow,** `final!` relates the two ends:

    ```rust
    fn first_mut(pair: &mut (u32, u32))
        -> (r: &mut u32, @(r == pair.0 && final!(pair) == (final!(r), pair.1)))
    ```

  - **Inside a body,** logical code reads current versions: `*x` at a point in the body is its value there. A local borrow has a final value too. When the borrow ends, the place it borrowed from gets a new version, which holds `final!(r)`.
  - **`final!(r)` reads no place.** It is not an observation in RFC 0001's sense, so it never extends a borrow.
  - **`final!` appears in postconditions and in bodies,** not in preconditions.
  - **There is no `old!`.** A signature already means entry values. A body that needs one later names a snapshot: `let start = logic! { *x };`.
  - **Soundness.** RustHornBelt proved prophecies sound for safe Rust, and Creusot is built on them. Using them needs to know where each borrow ends, which rustc's borrow checker computes on MIR. How Locus obtains that is [#6](https://github.com/nikhilgarg28/locus/issues/6).
- **Partial correctness.** A postcondition holds when the function returns. A function that panics or loops forever proves nothing, so `fn never() -> @False { loop {} }` is accepted. A logic function cannot be written that way, because logic functions are total. That a runtime function terminates is a separate claim ([#14](https://github.com/nikhilgarg28/locus/issues/14)).
- **Calls.** After a call returns, the caller has the proofs in its result, about the arguments it passed: `let (out, bounded) = clamp(n);`.
- **Ghost parameters.** A runtime function may take parameters of logical types, such as `n: Nat`, to state a contract in terms of a logical value that the caller supplies. Such a function is called directly, as rule 5 under [Ghost values](#ghost-values) says.
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
  - no place based on a field that a proof field reads is assigned or mutably borrowed, so neither `iv.lo = 0` nor `*s.b = 0` through a `Box` field is allowed; the whole value is replaced instead, as v0 required;
  - a struct update `..base` never supplies a proof field;
  - code that Locus cannot translate, such as inline assembly, never constructs the type.
- **A field whose type holds a proof,** such as `Option<@P>`, counts as a proof field.
- **Generated code that omits a logical data field,** such as a derived `Clone`, leaves that field's value unknown, and must still prove the proof fields.
- **The runtime program keeps such types closed to Rust.**
  - A struct or variant with proof fields is `#[non_exhaustive]` there, so that other crates cannot build it. Without that, Rust could build a public enum variant, or a struct whose proof reads no field, without its proof.
  - Its proof fields, and the fields they read, are not public.
  - No `unsafe impl` other than `Send` and `Sync` is allowed on it, written or derived. A derive such as `bytemuck::Zeroable` sees only the runtime fields, and would let safe code build a value without its proof.
- **Changing such a value in place costs a whole replacement.** For `NonEmpty { items: Vec<u32>, ok: @(items.len() > 0) }`, `self.items.push(x)` is rejected. This keeps the invariant even when a panic interrupts an update. Updating in place when nothing can panic is [#6](https://github.com/nikhilgarg28/locus/issues/6).
- **Reading a proof field is logical code:** `iv.ordered` is a proof of `iv.lo <= iv.hi`.
- **This holds even for values that pass through code Locus does not check.**
  - The fields such proofs read are not public in Rust (RFC 0001), and only checked code constructs the type.
  - So unchecked code can only move, copy, or drop a value, or call the type's own checked functions.
  - This is how invariants become the fourth way facts enter the logic, in RFC 0001's sense.
  - Such a type is a distinct Rust type, so it can instantiate any generic parameter. Unlike `()`, it cannot be confused with another type.
- **Logical data fields,** such as `count: Nat`, are ghost too. Assigning one is a ghost statement, which is erased.

**Runtime types holding logical values.** Any runtime type may hold logical values as type arguments, as described under [Ghost values](#ghost-values):
- `Option<@P>` keeps its tag at run time. `Some(h)` needs `h: @P` to build it, and matching `Some(h)` gives back `h: @P`.
- `Vec<(i: usize, @(i < xs.len()))>` is a `Vec<usize>` of indices, each with a proof that it is in bounds. `Vec<Nat>` keeps its length.
- What a value holds is known through checked code and specified standard library code, and is forgotten elsewhere.

### Erasure, in summary

RFC 0001 defines erasure. For this RFC's constructs:

| In the source | In both programs |
|---|---|
| A parameter, result component, local, or field of logical type, and every argument for one | Removed |
| A proof parameter, and the argument for it | A parameter of the crate's token type, and the token `PROOF` |
| A logical type argument of a runtime type, and a ghost value where one is needed | `()`, and a tuple without its logical components |
| A call of a runtime function whose result is entirely ghost | The call, as a statement |
| A named result `(out: T, @P)` | `T` |
| Logic functions, proposition declarations, `#[derive(Logical)]` types, `impl Model` | Stubs in the checking program; nothing in the runtime program |
| `logic!`, `prop!`, `prove!`, `_`, `final!` | Removed with the ghost value they produce, or `()` where one is needed |

### What changes in RFC 0001

These changes are made in RFC 0001 in this PR, so that the two RFCs are decided together:
- **No stand-ins for proofs or logical types.** Logical items become stubs in the checking program only, for resolving names.
- **Tokens.** The crate's generated module holds the proof token type, beside the configuration guard.
- **The fourth way facts enter the logic** is the invariants of the program's own types, as described above. This replaces the earlier `'static` rule. Ghost contents that pass through unchecked code are forgotten.
- **The export surface follows from the runtime program.**
  - Results there already lack proofs, so no wrappers are needed.
  - A function with proof parameters cannot be called from Rust without a token.

## Rules

The rules below go into four new chapters of the specification: [The logical layer](../../spec/logical-layer.md), [Models](../../spec/models.md), [Propositions and proofs](../../spec/proofs.md), and [Contracts](../../spec/contracts.md).

| Rule | Examples |
|---|---|
| `logic.types.contents-logical` | `logical-layer/logical-fields.accept.lc`, `logical-layer/logical-type-with-runtime-field.reject.lc`, `logical-layer/sequence-of-runtime-type.reject.lc` |
| `logic.types.positive` | `logical-layer/logical-fields.accept.lc`, `logical-layer/negative-logical-type.reject.lc` |
| `logic.arith.nat-subtraction` | `logical-layer/length-minus-one.accept.lc`, `logical-layer/nat-difference-is-int.accept.lc`, `logical-layer/nat-difference-as-nat.reject.lc` |
| `logic.ghost.erased` | `logical-layer/ghost-parameters.accept.lc`, `logical-layer/logical-values-in-vec.accept.lc`, `logical-layer/runtime-call-bound-first.accept.lc` |
| `logic.ghost.runtime-parts-kept` | `contracts/checked-indices.accept.lc`, `contracts/proof-in-option.accept.lc`, `logical-layer/logical-values-in-vec.accept.lc`, `contracts/some-without-proof.reject.lc` |
| `logic.ghost.runtime-code-kept` | `logical-layer/runtime-check-kept.accept.lc` |
| `logic.ghost.forgotten` | `logical-layer/proof-through-own-generic.accept.lc`, `logical-layer/option-through-black-box.reject.lc`, `logical-layer/proof-through-any.reject.lc`, `logical-layer/proof-through-black-box.reject.lc` |
| `logic.ghost.no-runtime-control` | `contracts/proof-in-option.accept.lc`, `logical-layer/branch-on-logical.reject.lc` |
| `logic.ghost.positions-logical` | `logical-layer/runtime-call-bound-first.accept.lc`, `logical-layer/runtime-call-in-proof-argument.reject.lc`, `logical-layer/runtime-call-in-proof-field.reject.lc` |
| `logic.ghost.mutable-lending` | `logical-layer/share-proof-holder-with-unchecked.accept.lc`, `logical-layer/lend-proof-holder-to-unchecked.reject.lc` |
| `logic.ghost.direct-calls` | `logical-layer/ghost-parameters.accept.lc`, `logical-layer/ghost-function-as-value.reject.lc` |
| `logic.code.no-runtime-calls` | `logical-layer/logic-calls-logic-function.accept.lc`, `logical-layer/logic-calls-runtime-function.reject.lc` |
| `logic.fn.total` | `logical-layer/measure-recursion.accept.lc`, `logical-layer/structural-recursion.accept.lc`, `logical-layer/measure-increases.reject.lc`, `logical-layer/nonterminating-logic-fn.reject.lc`, `proofs/diverging-logic-proof.reject.lc` |
| `models.observation` | `models/integer-model-no-overflow.accept.lc`, `observation/after-mutable-borrow-ends.accept.lc`, `observation/before-move.accept.lc`, `models/wrapping-is-not-the-model.reject.lc` |
| `models.integer-range` | `models/integer-range.accept.lc`, `models/integer-range-tighter.reject.lc` |
| `models.one-per-type` | `models/hand-written-model.accept.lc`, `models/two-models.reject.lc` |
| `models.declared-by-owner` | `models/hand-written-model.accept.lc`, `models/model-for-foreign-type.reject.lc` |
| `models.frozen-data` | `models/hand-written-model.accept.lc`, `models/model-reads-cell.reject.lc` |
| `proofs.versions` | `observation/proof-before-move.accept.lc`, `proofs/reproved-after-assignment.accept.lc`, `proofs/snapshot-survives-assignment.accept.lc`, `proofs/stale-proof-after-assignment.reject.lc`, `proofs/stale-proof-after-same-value.reject.lc` |
| `proofs.connectives` | `proofs/connectives.accept.lc`, `proofs/conjunction-needs-both.reject.lc` |
| `proofs.props.constructors` | `proofs/positive-proposition.accept.lc`, `proofs/wrong-arm.reject.lc` |
| `proofs.props.matching` | `proofs/match-every-arm.accept.lc`, `proofs/missing-arm.reject.lc` |
| `proofs.props.positive` | `proofs/positive-proposition.accept.lc`, `proofs/negative-proposition.reject.lc` |
| `proofs.elimination` | `proofs/witness-stays-in-proof.accept.lc`, `proofs/witness-to-data.reject.lc` |
| `proofs.partial` | `proofs/diverging-runtime-proof.accept.lc`, `proofs/returns-on-some-path.reject.lc` |
| `contracts.entry-values` | `contracts/final-value-at-return.accept.lc`, `contracts/entry-value-is-not-final.reject.lc` |
| `contracts.final` | `contracts/final-reads-no-place.accept.lc`, `contracts/final-value-at-return.accept.lc`, `contracts/local-borrow-final.accept.lc`, `contracts/returned-borrow.accept.lc`, `contracts/wrong-final.reject.lc` |
| `contracts.invariants` | `contracts/invariant-constructed.accept.lc`, `contracts/invariant-replaced-whole.accept.lc`, `contracts/invariant-default-derive.reject.lc`, `contracts/invariant-field-assigned.reject.lc`, `contracts/option-proof-field.reject.lc`, `contracts/proof-field-through-box.reject.lc` |

The examples use this RFC's syntax. Where [#5](https://github.com/nikhilgarg28/locus/issues/5) later respells something, their outcomes stay the same.

## Interactions

| Area | Effect |
|---|---|
| Types | Every type is runtime or logical, by its type. Logical types contain only logical types. Runtime types may hold logical types, which become `()`. `Int` is built in, and `Nat` is a library type whose difference is an `Int`. |
| Propositions and proofs | Propositions are Lean's, with Rust's syntax: inductive propositions with named arms, defined predicates, connectives, and quantifiers. They are about versions. Proofs are irrelevant, ghost, and kernel-checked. |
| Ownership and mutation | Logical code reads places under RFC 0001's observation rules. Every write makes a new version. A `&mut` reference has a current value and a final value, `final!`. How runtime code changes values in the logic is [#6](https://github.com/nikhilgarg28/locus/issues/6). |
| Erasure and runtime behavior | Ghost values are removed, or become `()` inside runtime types, in both programs alike, and runtime parts such as tags and lengths are kept. Proof parameters become tokens. Calls of runtime functions are kept even when their results are ghost. |
| Rust interoperability | Rust code never sees a proof. A function with proof parameters needs a token, which Rust can get only through `unsafe`. Preconditions inside data make a function `unsafe`. Fields that invariants read are private, and traits with contracts are `unsafe` to implement ([#7](https://github.com/nikhilgarg28/locus/issues/7)). What a value holds is forgotten when it passes through unchecked generic code. Models are declared by a type's own crate. |
| Diagnostics | New errors: logical values where runtime ones are needed, runtime calls in logical code, non-terminating logic functions, proofs about other versions, values that carry no proof after passing through unchecked code, and invariants not established. They are written as reasons until error codes exist. |
| Trust model | New entries: the models and specifications that Locus's library declares for standard types and generic code; the consistency of Lean's axioms; and `unsafe` code preserving invariants. The logical pass's ghost checks replace RFC 0001's checks on proofs in runtime code, and where borrows end joins rustc's borrow-checker entry. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Proofs as zero-sized values that Locus believes by their type:** RFC 0001's previous draft, and v0's `Erased` marker.
- Proofs would flow through every generic function for free.
- Rejected. With one Rust type for every proposition, Rust code that can tell types apart, through `Any` or specialization, could turn one proof into another.
- Making that sound needs every Rust generic to be parametric, which neither specialization nor `unsafe` code guarantees.

**No logical types inside runtime types:** an earlier draft of this RFC, which allowed only `Option` and `Result` to hold proofs.
- Rejected at the owner's direction. Containers of ghost values are useful, as v0 found, and forgetting ghost contents at unchecked code makes them sound.

**Proof parameters removed entirely, with `unsafe` functions at the Rust boundary:** an earlier draft of this RFC.
- Rejected at the owner's direction. Inside the crate and outside it, nothing but the type checker stopped a call that skipped Locus.
- Tokens make rustc itself reject such a call.

**`Nat` built in, with `Int` from the library,** as an earlier draft of this RFC, and the vision, had it.
- Rejected at the owner's direction. With `Int` built in, `Nat` is a subtype, and subtracting two naturals gives an integer rather than truncating at zero, as Lean does. So `v.len() - 1` cannot silently become `0`.

**A named zero-sized type instead of `()`** for logical values inside runtime types, such as `Vec<Ghost>` for `Vec<Nat>`.
- Inside the program, it would add no safety: Locus never believes what a runtime value holds unless it tracked it, so a forged marker would carry no more weight than a forged `()`.
- It would read more clearly, and rustc itself would refuse traits the marker does not implement, such as `Default`.
- At the Rust boundary, a private marker in a *parameter* would stop Rust callers from passing a proof they do not have. `find(xs, t, None)` could then stay safe, rather than `find` being `unsafe`.
- But one marker type stands for every proposition. So it must never reach Rust callers in a *result*, or they could pass a marker from one function's result as another function's precondition. That needs a conversion at each boundary: cheap for `Option`, but a copy of the whole vector for `Vec<(usize, @P)>`.
- This RFC keeps `()`, and leaves a private marker for `Option` and `Result` parameters to [#7](https://github.com/nikhilgarg28/locus/issues/7).

**Distinct Rust types per proposition.** Not possible: propositions mention runtime values.

**v0's `old!(x)` for the entry value and bare `x` for the value at return.**
- Rejected for one rule: a signature means entry values, and `final!` names the final value.
- That rule also covers borrows that are returned, kept in locals, or stored, which `old!` cannot express.

**Several models per type, chosen with `x as M`** (an earlier v0 decision). Rejected, as v0 later did: one canonical model makes `v.len()` in logical code unambiguous. Other views are logic functions.

**Models before field access,** as in v0, where logical code observed the receiver through its model before selecting a field.
- Rejected: keeping Rust's meaning for places makes logical code read like Rust, and works for types without models.
- Models apply where a value is used.

**Boolean comparisons with `prop!` lifting them to propositions** (v0). Rejected in favor of Lean's propositional comparisons, with a classical `if`. This means there is one kind of comparison.

**`=>` for implication** (v0). Rejected: inside a logical `match`, `=>` already separates an arm from its body, so `==>` is clearer.

**`forall (x: T) { F }` keywords** (v0). Rejected for `forall |x: T| F`, which reads like Rust's closures and needs no new delimiters.

**A `ghost` keyword or a `Ghost<T>` wrapper,** as in Verus. Rejected: the type already says whether a value is ghost.

**Tracked evidence, a `let mut h: @P` that follows the current version of what it reads** (v0). Deferred to the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)), together with loops, where it serves as a loop invariant.

## Prior art

- **Lean 4** is the model for the logic, the kernel, inductive propositions, and proof irrelevance.
- **Creusot** has logic functions, a mutable reference's current value `*x` and final value `^x`, and models through its `View` trait. Its handling of `&mut` is the model for `final!`.
- **RustHorn and RustHornBelt** (Matsushita et al., ESOP 2020 and PLDI 2022) gave prophecies for Rust's mutable borrows and proved them sound.
- **Verus** separates spec, proof, and exec code, erases ghost code completely, and models types with `view()`.
- **Prusti** writes entry values with `old(...)`, the convention this RFC replaces with entry-by-default and `final!`.
- **Dafny** has ghost variables and fields, `decreases` clauses, and erasure of ghost code.
- **F\* with Low\* and KaRaMeL** erases proofs to produce C, as Locus erases them to produce Rust.
- **v0** decided much of this design, including its versions, as described above.

## Unresolved questions

**Decisions for the owner,** in order of what they block:
1. **Ghost proofs with tokens and forgetting:**
   - proof parameters become tokens;
   - other proofs become nothing, or `()` inside runtime types;
   - ghost contents are tracked only through checked code and specified standard library code.

   These follow the owner's direction; the mechanisms are this RFC's proposal.
2. **The `&mut` convention:** entry values in signatures, and `final!(r)` for the value when the borrow ends, with no `old!`. This is the owner's.
3. **Versions:** every write makes a new version, even of the same value, and a proof never carries over to another version without a new proof.
4. **`Int` built in, and `Nat` from the library,** with `Nat - Nat` an `Int`. This is the owner's. Integer division truncates toward zero, as Rust's does, and division by zero gives zero.
5. **One model per type,** declared by the type's own crate or by Locus's library.
6. **Places keep their Rust meaning in logical code,** and models apply to values.
7. **Lean's propositional comparisons,** with a classical `if`.
8. **The traits logical types implement:** `Clone`, `Copy`, and `Debug`, plus comparison and hashing for proofs.
9. **The program's own generic code is checked at each instantiation with logical types,** until [#12](https://github.com/nikhilgarg28/locus/issues/12) allows checking it once.
10. **Syntax:** `==>`, `forall |x: T|`, `Arm(witnesses) @ proof`, `#[decreases(e)]`, and `final!`.
11. **Ghost positions hold logical code:** a runtime call that returns a proof is written in its own statement or `let`, rather than as an argument, a tuple component, a field initializer, or a payload.
12. **A place holding logical values is lent mutably only to checked or specified code,** and a function with ghost parameters is only called directly, never used as a value.
13. **Types with proof fields are closed to Rust:** `#[non_exhaustive]`, with private proof fields and the fields they read, and no `unsafe impl` other than `Send` and `Sync`.
14. **Changing a value with an invariant means replacing it whole,** as v0 required, until [#6](https://github.com/nikhilgarg28/locus/issues/6) allows updates in place where nothing can panic.
15. **`Opaque<T>`** is the logical type of values of a type without a model.
16. **No `#[derive(Model)]` for now.** A struct without a model is still readable field by field.

**Left to later work,** each with its issue:
- The runtime semantics: how runtime operations change versions and models, loops and invariants, tracked evidence, where borrows end, and overflow: [#6](https://github.com/nikhilgarg28/locus/issues/6).
- Remaining syntax questions: [#5](https://github.com/nikhilgarg28/locus/issues/5).
- Distribution: how other crates obtain tokens, `unsafe` exports, and traits with contracts: [#7](https://github.com/nikhilgarg28/locus/issues/7).
- Closures and async code, including closures that take or return proofs: [#8](https://github.com/nikhilgarg28/locus/issues/8).
- Locus's standard library: logical types, and models and specifications of standard types and generic code: [#11](https://github.com/nikhilgarg28/locus/issues/11).
- Traits and operators on logical types, and checking generic code once for logical instantiations: [#12](https://github.com/nikhilgarg28/locus/issues/12).
- Storing proofs that were found, and what proof search may do: [#13](https://github.com/nikhilgarg28/locus/issues/13).
- Claims that runtime functions terminate: [#14](https://github.com/nikhilgarg28/locus/issues/14).
- How the kernel provides `Int`: the kernel RFC.
- Mutable references stored in data, whose model carries a current and a final value.
