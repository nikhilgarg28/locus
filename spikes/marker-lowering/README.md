# Spike: an actual lowering with a Rust type for each logical type

A throwaway experiment for the draft of RFC 0006 on traits, and for [traits-on-logical-types](../../issues/traits-on-logical-types.md). It was built outside the repository, and depends on nothing in the repository when it is built: what it reuses from [`locus-macro`](../locus-macro/README.md) is copied. Nothing may depend on it. (The links assume it is placed under `spikes/`.)

## Question

The first spike, [`logical-markers`](../logical-markers/README.md), showed with Rust written by hand that rustc resolves traits on logical types once each logical type is a zero-sized Rust type of its own. A reviewer's objection: hand-written lowered Rust cannot show that an actual lowering works.

**Can an actual syntactic lowering produce one Rust type for each logical type, by the draft's rules, so that the lowered program compiles in both expansions and rustc resolves traits on logical types? Where does a syntactic lowering have to guess, and where does it fail?**

1. Do the programs that today's one-marker lowering compiles still compile, both ways?
2. The draft's rule for ghost locals: one with no logical type for rustc where only logical code mentions it, and otherwise one binding whose type rustc infers. Can lowering tell the two apart from the tokens?
3. A trait's logic functions are not items of the lowered trait. Where do their stubs go, so that rustc still resolves the names that logical code uses?
4. Which of the draft's rejections can lowering make from syntax, and which need more?

## Setup

A Cargo workspace of its own, stable rustc 1.97.0 and cargo 1.97.0, proc-macro2 1.0.107. All outputs go under `target/`, which `.gitignore` excludes.

- `locus/` is the library, adapted from the copy in `locus-macro`.
  - `Int`, `Nat`, `Bool`, `Prop` and a generic `Seq<T>` are zero-sized types of their own. `Seq<T>` holds a `PhantomData<fn() -> T>` and is `Clone` and `Copy` with no bound on `T`. Every proof is `Erased`. `Snap<T>` is `<T as HasSnap>::Marker`, one marker for every `T`.
  - `Logical` is implemented for each of them, for `Erased`, for the snapshot marker, and for pairs and triples. Unlike the draft's, it has a hidden member, `__locus_marker()`, and the supertraits `Copy + 'static`.
  - `ghost::<G: Logical>()` gives a marker of the type rustc infers. `Untyped` is a marker that is not `Logical`.
  - `AddLogic`, `FromLogic`, and `IntoLogic` with its blanket implementation are written in Locus, inside a `locus!` block of the library itself, and lowered by the same macro as a program's traits.
  - The macros that carry lowered logical code take `[borrows of places] [items probed as values] [names probed] code`, and put their observations and probes under `#[cfg(__locus_check)]` in their own output, as before.
- `locus-macros/` is `locus!`, about 2,400 lines after rustfmt, adapted from the 1,400 of `locus-macro`. It is still a token rewriter with heuristics, and it still writes one text, the same under both `cfg`s. What changed:

  | Source | What `locus!` writes |
  |---|---|
  | `#[derive(Logical)] struct Pair<T> { .. }` | `struct Pair<T> { __locus: PhantomData<fn() -> (T,)> }` under its own name, with `Clone`, `Copy` and `Debug` for every `T`, and `impl<T: Logical> Logical for Pair<T>`. The fields are not written. The checking expansion has a function beside it that names the fields' types. A lifetime parameter is an error of `locus!` |
  | `let x = logic! { .. };`, where only logical code mentions `x` in the rest of its scope | `let x = ::locus::untyped!(..);`, which is `Untyped` |
  | Any other `logic! { .. }` | `::locus::data!(..)`, which is `::locus::ghost()` |
  | `x = logic! { .. };` | A ghost statement: `untyped!` or `data!`, as `x` was bound |
  | `prove!(..)`, a hole; `prop!(..)` | `Erased`; `Prop` |
  | `logic fn name` in a trait | A stub `__logic_name` in a hidden trait `__locus_logic_Trait`, declared beside the trait with its parameters and supertraits, and made one more supertrait of it |
  | `logic fn name` in an `impl` of a trait | A stub `__logic_name` in an `impl` of the hidden trait, written beside the `impl` |
  | `logic fn name` in an inherent `impl` | A stub `__logic_name` there |
  | The body of any `logic fn` | Probes of the names it calls, then a panic |
  | A name that logical code calls: `double(..)`, `geometry::area(..)` | The probe `use double as _;` |
  | `T::le(..)`, `Self::le(..)`, `Rational::le(..)`, `<T as OrderSpec>::le(..)` | The probe `T::__logic_le`, as a value; the last through the hidden trait |
  | An `impl` whose header writes `@P`, `Snap`, `CutSnap`, or a named tuple component | An error of `locus!` |
  | An `impl` of a trait for a logical type that gives a `fn`, or of a trait of the same invocation that declares one | An error of `locus!` |

  Two spike-only directives: `#![spike(logic_members_in_trait)]` keeps the stubs as items of the lowered trait, under their mangled names, and `#![spike(allow_ghost_headers)]` turns the header rejection off. Two environment variables: with `LOCUS_SPIKE_ALL_TYPED=1`, every `let x = logic! { .. };` is a typed binding, and `LOCUS_SPIKE_IN_TRAIT=1` is the first directive in every invocation, the library's included.
- `cases/` has 58 programs, one binary each. The first line of each says what the two compilations must do, by the draft's rules and not by what this toy was seen to do: `ok`, an error code, `error` for an error of `locus!`, `reject` for any error, `annotate` for rustc's "type annotations needed" (E0282 or E0283). `//! prints:` lines are what both binaries must print. Five of them, named `-hand-`, are plain Rust with no `locus!`.
- `corpus/` is a copy of the 26 files that `locus-macro`'s `run.sh` builds for its question 2: 22 litmus examples and 4 others.
- `ghost-lets/` holds the 17 litmus examples, of 454 at commit `a8f2db2`, that have a `let x = logic! { .. };` with no annotation (19 such `let`s), each copied inside `locus! { }` by `gen-ghost-lets.py`. Four of them are also in `corpus/`.
- `baseline/` is the library and macro of `locus-macro`, copied unchanged, as a workspace of its own. Its packages' `src/bin` are links to the directories above, so both lowerings compile the same files.
- `run.sh [all|cases|corpus|ghostlets|typed|intrait|expansions] [name-prefix]` reproduces everything. The runtime expansion is `cargo build`; the checking expansion is the same command with `RUSTFLAGS="--cfg __locus_check"`. Each lowering and each expansion has its own target directory. `run.log` is the output of the run that the results below come from.

## Results

Run on 2026-11-22. From an empty `target/`, `./run.sh` took 81 seconds and left 1.8 GB there. Its summary lines:

```text
cases: 58, as expected: 47, differ: 11
accepted litmus examples: 20; compile in both expansions: old 20, new 20
rejected litmus examples that fail in the checking expansion alone: old 2, new 2
accepted litmus examples: 10; compile in both expansions: old 8, new 8
corpus: 8 such `let`s; 6 hold the untyped marker, 2 are typed by rustc's inference
ghost-lets: 19 such `let`s; 17 hold the untyped marker, 2 are typed by rustc's inference
cases: 40 such `let`s; 18 hold the untyped marker, 22 are typed by rustc's inference
corpus: 23 programs that compile both ways with the draft's rule; with every ghost local typed, 4 fail, at 4 `let`s
ghost-lets: 12 programs that compile both ways with the draft's rule; with every ghost local typed, 12 fail, at 12 `let`s
cases: 29 programs that compile both ways with the draft's rule; with every ghost local typed, 10 fail, at 15 `let`s
cases: 58, as expected: 51, differ: 7
```

The last line is every case again with the other placement of a trait's logic functions, [below](#where-the-stubs-of-a-traits-logic-functions-go).

Every case that compiled both ways printed `expansion: runtime` from one binary and `expansion: checking` from the other, and the same lines after that.

### What today's lowering compiles

**All 20 accepted litmus examples of the corpus still compile both ways,** and the 16 with `run` lines print them from both binaries. The 2 rejected examples still fail in the checking expansion alone, with the same errors (E0382, E0502). No source was changed, and no annotation was added.

Of the 4 other files, two changed, both from failing to compiling (lines of the run, some left out):

```text
== x-logic-kind: old runtime=E0308 check=E0308; new runtime=ok check=ok;
  old runtime: corpus/src/bin/x-logic-kind.rs:30:21: error[E0308]: mismatched types: expected `Erased`, found `LogicalData`
  new: both binaries print: annotated() => 2|unannotated() => 2
== x-split-logical-type: old runtime=E0118 check=E0118,E0599; new runtime=ok check=ok;
  old runtime: corpus/src/bin/x-split-logical-type.rs:20:5: error[E0118]: no nominal type found for inherent implementation: impl requires a nominal type
  new: both binaries print: use_them() => 1
```

- `x-logic-kind`: `logic! { add_zero(3) }` gives a proof, and the old toy had to choose between the proof marker and the data marker without knowing. Now `logic!` gives the type rustc infers, and `Erased` is one of them.
- `x-split-logical-type`: a logical type declared in one `locus!` and given an `impl` in another. The type is now a struct under its own name, so the second invocation needs to know nothing.
- `x-locus-name-collision` fails as before (E0659), for a reason that has nothing to do with markers.

Of the 17 litmus examples with an unannotated ghost `let`, 10 are accepted examples. 8 compile both ways under both lowerings. The other 2 fail under both, for what the toy lacks: `locus::std` (`operator-in-logic-block`), and `given(..)` with `===` outside a macro (`hole-written-out-with-an-earlier-value`). Three files changed outcome:

- `branch-on-logical` (rejected): it failed and still fails in both expansions, but rustc's error moved. It was E0369 at the operator, ``binary operation `>` cannot be applied to type `LogicalData` ``. It is E0283 at the `let`, `type annotations needed`. The toy takes `if count > 0` for a mention of the ghost `count` by runtime code, so `count` is a typed binding with nothing to infer from. Whether RFC 0002 makes `count > 0` logical code, as an operator on a binding shown to be ghost, the toy does not decide.
- `final-value-recovered-by-inference` (rejected): the old toy took `_` for a place, and this one does not. Unrelated to markers.
- `hole-written-out-with-an-earlier-value`: the same errors less one.

### Ghost locals

**In the litmus corpus, 17 of the 19 ghost `let`s with no annotation are mentioned by logical code alone,** as the toy found, and hold the untyped marker. The other two are in `branch-on-logical` and `hole-written-out-with-an-earlier-value`, above: the mention is `count > 0` in one and `given(..)` in the other, forms the toy does not take for logical code. No annotation was added to any litmus example, and none of the examples that compile needed one.

**Without the draft's rule, each of them would need a type.** With `LOCUS_SPIKE_ALL_TYPED=1`, every one of the 12 ghost-let programs that otherwise compile both ways fails, and so do 4 of the corpus's 23:

```text
== gl-before-move: runtime=E0282
  ghost-lets/src/bin/gl-before-move.rs:17:9: error[E0282]: type annotations needed
      let n = logic! { v.len() };
== snapshot-survives-assignment: runtime=E0282
  corpus/src/bin/snapshot-survives-assignment.rs:14:9: error[E0282]: type annotations needed
      let before = logic! { n };
```

rustc reports one `let` for each function, so the count of `let`s is a floor: two of those programs have two.

**One typed binding is one place.** `g02` prints the type rustc inferred, backward from the call:

```text
== g02-takes-nat: expect runtime=ok check=ok; got runtime=ok check=ok; as expected
  both binaries print: run(&vec![4, 5, 6]) => ("locus::Nat", 1)
== g03-two-borrows: expect runtime=ok check=ok; got runtime=ok check=ok; as expected
  both binaries print: run(&vec![4, 5, 6]) => (true, true)
== g04-returned-reference: expect runtime=ok check=ok; got runtime=ok check=ok; as expected
  both binaries print: run(&vec![4, 5, 6]) => (true, 1)
```

Two borrows of a ghost local are equal addresses, a reference to one outlives the call that returned it, and a mutable borrow is assigned through (`g05`). A closure that captures one and passes it on gives rustc the type through the capture (`g09`).

**Where every use is generic, rustc asks for the type, and points at the user's `let`** (`g06`). One implementation of the bound is not enough for it to choose (`g16`):

```text
error[E0283]: type annotations needed
  --> cases/src/bin/g16-single-implementation.rs:18:13
   |
18 |         let before = logic! { v.len() };
   |             ^^^^^^
19 |         only(before)
   |         ---- ------ type must be known at this point
   |         |
   |         required by a bound introduced by this call
   |
   = note: the type must implement `Monoid`
help: the trait `Monoid` is implemented for `locus::Int`
  --> cases/src/bin/g16-single-implementation.rs:11:5
   |
11 |     impl Monoid for Int {
   |     ^^^^^^^^^^^^^^^^^^^
note: required by a bound in `only`
  --> cases/src/bin/g16-single-implementation.rs:15:16
   |
15 |     fn only<M: Monoid>(m: M) -> &'static str { std::any::type_name::<M>() }
   |                ^^^^^^ required by this bound in `only`
help: consider giving `before` an explicit type, where the type for type parameter `M` is specified
   |
18 |         let before: /* Type */ = logic! { v.len() };
   |                   ++++++++++++
```

**The toy classified every case of shadowing, capture, pattern and loop correctly.** What it wrote for `g11`, where only the first function's `seen` is left untyped (from `./run.sh expansions g11`, spacing tidied and lint attributes removed):

```rust
fn plain(v: &Vec<u32>) -> u32 {
    let mut seen = ::locus::untyped!([] [] [] 0);
    let mut total = 0;
    for x in v {
        total += *x;
        seen = ::locus::untyped!([&seen] [] [] seen + 1);
    }
    let all = ::locus::proof!([&seen, &v] [] [] seen == v.len());
    total
}
fn passed(v: &Vec<u32>) -> u32 {
    let mut seen = ::locus::data!([] [] [] 0);
    let mut total = 0;
    for x in v {
        total += *x;
        seen = ::locus::data!([&seen] [] [] seen + 1);
    }
    total + takes_nat(seen)
}
```

In `g08`, the ghost `x` is left untyped under a later `let x`, an inner block's `let x`, a closure's parameter `x`, a `match` arm and an `if let` that bind `x`, and a `for` and a `while let` that bind `x`; and it is typed where a ghost `x` shadows a runtime one and is passed on, and where an arm with the pattern `Point { x: here }` passes the ghost `x` on. In `g10`, `let (count, before, passed) = (v.len(), logic! { .. }, logic! { .. });` leaves `before` untyped and types `passed`.

**Three programs did not behave as the draft says, or as today:**

```text
== g13-copied: expect runtime=annotate check=annotate; got runtime=E0282 check=E0282; as expected
== g14-tuple-component: expect runtime=ok check=ok; got runtime=E0282 check=E0282; DIFFERS
  runtime: cases/src/bin/g14-tuple-component.rs:11:13: error[E0282]: type annotations needed for `(usize, _)`
== g15-format-capture: expect runtime=annotate check=annotate; got runtime=ok check=ok; DIFFERS
  both binaries print: run(&vec![4, 5, 6]) => "Untyped"
### Three of them with the existing spike's lowering (one marker), for comparison
== g13-copied: old runtime=ok check=ok
== g14-tuple-component: old runtime=ok check=ok
```

- `g13`: `let before = logic! { v.len() }; let again = before;`, and only logical code reads either. `let again = before;` is a mention by runtime code, so `before` is a typed binding, and nothing gives its type. The draft says this "happens where every use is generic". Here no use is generic. Today's lowering compiles it.
- `g14`: `let pair = (v.len(), logic! { v.len() });`. The ghost value is a component of a runtime local, so the rule for ghost locals does not apply to it, and nothing gives its type. Today's lowering compiles it.
- `g15`: `format!("{before:?}")`. The mention is inside a string literal, the toy sees none, and the local is left untyped. rustc accepts, because `format!` takes any `Debug`, and the program prints `Untyped` where a typed binding would print `Nat`.

`g19`, written by hand, shows the other outcome of a wrong pick: the untyped marker passed where a `Nat` is wanted is E0308, ``expected `Nat`, found `Untyped` ``.

**`Snap` as one marker keeps no borrow.** `g12` takes a snapshot of a `&mut Vec<u32>` three ways, annotated as `Snap<&mut Vec<u32>>`, inferred from a parameter of type `Snap<Vec<u32>>`, and untyped, then pushes through the reference and through the vector, then uses all three. It compiles both ways and prints `[1, 2, 3]`. The cost is `g18`: rustc accepts a `Snap<Vec<u32>>` where a `Snap<bool>` is wanted.

### Traits

**The two-trait pattern lowers and resolves.** What `locus!` wrote for part of `t01` (from `./run.sh expansions t01`, spacing tidied and lint attributes removed):

```rust
trait __locus_logic_OrderSpec {
    fn __logic_le(a: Self, b: Self) -> Bool
    where
        Self: ::core::marker::Sized + OrderSpec;
    fn __logic_le_total(a: Self, b: Self) -> ::locus::Proof![Self::le(a, b) || Self::le(b, a)]
    where
        Self: ::core::marker::Sized + OrderSpec;
}
trait OrderSpec: __locus_logic_OrderSpec {}
trait CheckedOrder: OrderSpec {
    fn le_checked(&self, other: &Self) -> (bool, ::locus::Proof![r === Self::le(*self, *other)]);
}
impl __locus_logic_OrderSpec for Rational {
    fn __logic_le(a: Rational, b: Rational) -> Bool {
        ::locus::stub_body!([] [])
    }
    ..
}
impl OrderSpec for Rational {}
fn is_max<T: OrderSpec>(m: T, a: T, b: T) -> Prop {
    ::locus::stub_body!([T::__logic_le] [])
}
fn max<'a, T: CheckedOrder>(a: &'a T, b: &'a T) -> (&'a T, ::locus::Proof![is_max(*m, *a, *b)]) {
    let (a_le_b, why) = a.le_checked(b);
    let total = ::locus::untyped!([&a, &b] [T::__logic_le_total] [] T::le_total(*a, *b));
    if a_le_b {
        (b, ::locus::proof!([&b, &a] [] [is_max] is_max(*b, *a, *b)))
    } else {
        (a, ::locus::proof!([&a, &b] [] [is_max] is_max(*a, *a, *b)))
    }
}
```

```text
== t01-two-traits: expect runtime=ok check=ok; got runtime=ok check=ok; as expected
  both binaries print: newest() => (2, 5)|spec_only() => 3
== t02-max-rational: expect runtime=E0277 check=E0277; got runtime=E0277 check=E0277; as expected
  runtime: cases/src/bin/t02-max-rational.rs:61:35: error[E0277]: the trait bound `Rational: CheckedOrder` is not satisfied: unsatisfied trait bound
== t03-logical-impl-runs: expect runtime=error check=error; got runtime=error check=error; as expected
  runtime: cases/src/bin/t03-logical-impl-runs.rs:61:12: error: locus: a logical type implements only traits with no method that runs, and this implementation of `CheckedOrder` gives the method `le_checked`
```

Also as the draft says:

- **The library's traits** (`t04`). rustc finds `Nat: IntoLogic<Rational>` through the blanket implementation and the program's `impl FromLogic<Nat> for Rational`, `Rational: IntoLogic<Int>`, `Nat: IntoLogic<Int>` through the library's own conversion, the reflexive conversion beside it, `Rational: AddLogic<Output = Rational>`, and `<Rational as AddLogic>::Output` as a parameter's type. `Bool: IntoLogic<Rational>` is E0277 (`t05`). In the first spike, with one marker, a conversion of the library's own overlapped the reflexive one (E0119).
- **An associated logical type** that is `Int` at `Int` and a proof at `Nat`, in a signature and in a runtime struct, which stays 4 bytes (`t06`).
- **A proof in a method's signature,** with a default body and generic code over the bound (`t07`), and the draft's `Monoid`, for the library's `Int` and a program's logical type, with a lemma and a defaulted logic function (`t09`).
- **A weaker contract in an `impl`** is accepted by rustc, which sees `(u8, Erased)` on both sides (`t08`). The draft leaves that to Locus's own check, which this spike does not have.
- **rustc rejects `Vec<Nat>` used as `Vec<Seq<Int>>`** (`d03`, E0308), and a marker still has no `==` (`d04`, E0369).

**The kind of a member is checked by rustc, as a side effect of the mangled names.** An `impl` that gives a logic function as a `fn`, or the reverse, is rejected in both expansions, in rustc's terms:

```text
== t10-kind-fn-for-logic: expect runtime=reject check=reject; got runtime=E0046,E0407 check=E0046,E0407; as expected
  runtime: cases/src/bin/t10-kind-fn-for-logic.rs:15:9: error[E0407]: method `le` is not a member of trait `OrderSpec`: not a member of trait `OrderSpec`
  runtime: cases/src/bin/t10-kind-fn-for-logic.rs:14:5: error[E0046]: not all trait items implemented, missing: `__logic_le`: missing `__logic_le` in implementation
```

**A mangled name is what keeps a logic function out of rustc's resolution of Rust code.** In `t19` a trait has `logic fn eq`, the type has `PartialEq`, and `Version::eq(&a, &b)` in runtime code is `PartialEq::eq`. In `t20`, the same lowered by hand with the stub named `eq`, it is E0034, ``multiple `eq` found``.

### Where the stubs of a trait's logic functions go

The default here follows the draft: the stubs are not items of the lowered trait. They are in a hidden trait that the lowered trait has as a supertrait, so a bound `T: OrderSpec` gives rustc the stubs for `T`. That works in the module that declares the trait (`t12`: `Self::le`, `T::le`, `Rational::le`, `<T as OrderSpec>::le`) and, under a bound, anywhere (`t13b`). It fails for a trait that another module imports by name, as Rust asks:

```text
== t13a-imported-impl: expect runtime=ok check=ok; got runtime=E0277,E0405 check=E0277,E0405; DIFFERS
  runtime: cases/src/bin/t13a-imported-impl.rs:20:10: error[E0405]: cannot find trait `__locus_logic_OrderSpec` in this scope
== t13c-imported-type: expect runtime=ok check=ok; got runtime=ok check=E0599; DIFFERS
  check:   cases/src/bin/t13c-imported-type.rs:25:33: error[E0599]: no associated function or constant named `__logic_le` found for struct `Rational` in the current scope: associated function or constant not found in `Rational`
== t13d-imported-qualified: expect runtime=ok check=ok; got runtime=ok check=E0405; DIFFERS
  check:   cases/src/bin/t13d-imported-qualified.rs:25:29: error[E0405]: cannot find trait `__locus_logic_OrderSpec` in this scope
== t17-default-member-split: expect runtime=ok check=ok; got runtime=E0277 check=E0277,E0599; DIFFERS
  runtime: cases/src/bin/t17-default-member-split.rs:17:20: error[E0277]: the trait bound `Thing: __locus_logic_Named` is not satisfied: unsatisfied trait bound
```

- `use spec::OrderSpec;` does not import the hidden trait. So `impl OrderSpec for Rational` cannot name the hidden trait it must also implement (`t13a`, both expansions), `Rational::le(..)` in logical code does not find the stub (`t13c`), and neither does `<T as OrderSpec>::le(..)` (`t13d`).
- An `impl` that gives no logic function does not show whether its trait has a hidden trait (`t17`: every logic function of the trait has a default body, and the trait is in another invocation).

With `#![spike(logic_members_in_trait)]`, the stubs are items of the lowered trait, under their mangled names, and each of these compiles both ways (`t14`, `t18`). With that placement in every invocation, the library's included (`./run.sh intrait`), no case that the default passes fails:

```text
## Every case with the stubs of a trait's logic functions kept in the lowered trait
cases: 58, as expected: 51, differ: 7
```

The 51 are the default's 47 and `t13a`, `t13c`, `t13d` and `t17`. The 7 are the rest of the default's 11: `d06`, `g14`, `g15`, `h04`, `t15`, `t16` and `t21`.

**Neither placement can probe `OrderSpec::le(r, r)`,** the member named through its trait:

```text
== t15-names-trait-path: expect runtime=ok check=ok; got runtime=ok check=E0782; DIFFERS
  check:   cases/src/bin/t15-names-trait-path.rs:20:23: error[E0782]: expected a type, found a trait
== t16-names-trait-path-in-trait: expect runtime=ok check=ok; got runtime=ok check=E0790; DIFFERS
  check:   cases/src/bin/t16-names-trait-path-in-trait.rs:19:23: error[E0790]: cannot call associated function on trait without specifying the corresponding `impl` type: cannot call associated function of trait
```

`locus!` cannot tell that path from `Rational::le(r, r)`. With the stubs in the trait, rustc finds the member and then needs the type of `Self`, which only the arguments' types give.

### Probes

RFC 0001 probes a stub as a value and writes each of its type arguments as the marker. There is no longer one marker to write:

```text
== p01-hand-generic-value-probe: expect runtime=E0283 check=E0283; got runtime=E0283 check=E0283; as expected
  runtime: cases/src/bin/p01-hand-generic-value-probe.rs:20:11: error[E0283]: type annotations needed: cannot infer type of the type parameter `T` declared on the function `is_max`
== d06-bounded-impl-probe: expect runtime=ok check=ok; got runtime=ok check=E0599; DIFFERS
  check:   cases/src/bin/d06-bounded-impl-probe.rs:27:34: error[E0599]: the associated function or constant `__logic_least` exists for struct `Pair<Untyped>`, but its trait bounds were not satisfied: associated function or constant cannot be called on `Pair<Untyped>` due to unsatisfied trait bounds
```

- **A function is probed by `use is_max as _;`,** which resolves the name and infers nothing. It worked for every function that logical code calls in the cases and the corpus, generic or not, from a runtime body and from a stub's, and by a path through a module (`geometry::area` in `d01`). It cannot reach a member of a trait: `use OrderSpec::le as _;` is E0658, unstable (`p02`).
- **A logic function of a generic logical type needs the type's arguments.** `Pair::swap(both)` is probed as `Pair::<Untyped>::__logic_swap`, which works where the `impl` has no bound (`d01`) and fails where it has one (`d06`).

### Headers

`locus!` rejects a snapshot type or a proof type written in a header (`h01`, `h02`). With the rejection off, `h03` shows what the rule prevents:

```text
== h03-snap-header-allowed: expect runtime=ok check=ok; got runtime=ok check=ok; as expected
  both binaries print: <Snap<bool> as Describe>::Width is locus::Nat|<Snap<&mut Vec<String>> as Describe>::Width is locus::Nat|from a proof of 2 == 2: written for a proof of 1 == 2
```

The implementation written for `Snap<u32>` is the one rustc selects at `Snap<bool>` and at `Snap<&mut Vec<String>>`, and `impl From<Option<@(1 == 2)>> for Absurd` is the one it selects for a proof of `2 == 2`.

Two rejections are beyond the tokens of one invocation:

```text
== h04-snap-header-alias: expect runtime=reject check=reject; got runtime=ok check=ok; DIFFERS
== t21-provided-method-split: expect runtime=reject check=reject; got runtime=ok check=ok; DIFFERS
  both binaries print: run() => 7
```

- `h04`: `type Shot = Snap<u32>; impl Describe for Shot`. `locus!` sees the name `Shot`.
- `t21`: a logical type implements a trait whose only method that runs has a default body, and the trait is declared in another invocation. The `impl` gives nothing that runs, `locus!` accepts it, and `half.hello()` runs on a logical value. In one invocation `locus!` has the declaration and rejects it (`t22`).

### All cases

`Expected` and `Got` are runtime / checking where the two differ.

| Case | What it does | Expected | Got | |
|---|---|---|---|---|
| `d01-derive` | `#[derive(Logical)]`: one with no parameters, a generic one, two that contain themselves; a logical field in a runtime struct; `Copy`, `Debug`, sizes | ok | ok |  |
| `d02-derive-lifetime` | A logical type with a lifetime parameter: `locus!` rejects it | error | error |  |
| `d03-vec-mismatch` | A `Vec<Nat>` used as a `Vec<Seq<Int>>` | E0308 | E0308 |  |
| `d04-no-equality` | `#[derive(PartialEq)]` on a runtime struct with a logical field | E0369 | E0369 |  |
| `d05-field-type-unknown` | A field of a logical type whose type does not exist: seen only by the probe of the checking expansion | ok / E0425 | ok / E0425 |  |
| `d06-bounded-impl-probe` | A logic function in `impl<T: OrderSpec> Pair<T>`, called from logical code: the probe has no type to write for `T` | ok | ok / E0599 | **differs** |
| `g01-only-logical` | A ghost local that only later logical code mentions | ok | ok |  |
| `g02-takes-nat` | One passed to a function that takes a `Nat`; prints the type rustc inferred | ok | ok |  |
| `g03-two-borrows` | One borrowed twice; a function compares the two addresses | ok | ok |  |
| `g04-returned-reference` | A reference to one, returned from a function and used later | ok | ok |  |
| `g05-mutable-borrow` | A mutable borrow of one, assigned through | ok | ok |  |
| `g06-generic-only` | One passed only to a generic function | annotate | E0283 |  |
| `g07-generic-annotated` | The same with `let before: Nat` | ok | ok |  |
| `g08-shadowing` | Seven kinds of shadowing: a later `let`, an inner block, a ghost local over a runtime one, a closure's parameter, `match` and `if let`, a field label, `for` and `while let` | ok | ok |  |
| `g09-closure-capture` | One captured by a closure that passes it to runtime code | ok | ok |  |
| `g10-pattern` | Bound in patterns: from a call, from a tuple that holds a `logic!`, by `if let` | ok | ok |  |
| `g11-loop-assignment` | Assigned in a loop by `seen = logic! { .. };`, with and without a runtime use | ok | ok |  |
| `g12-snapshot-of-mut` | A ghost snapshot of a `&mut`, then a new mutable use of the place: annotated, inferred, and untyped | ok | ok |  |
| `g13-copied` | `let again = before;`, both read only by logical code | annotate | E0282 |  |
| `g14-tuple-component` | `let pair = (v.len(), logic! { v.len() });`: a ghost component of a runtime local | ok | E0282 | **differs** |
| `g15-format-capture` | `format!("{before:?}")`: a mention by runtime code that the tokens do not show | annotate | ok | **differs** |
| `g16-single-implementation` | One passed only to `fn only<M: Monoid>`, where `Monoid` has one implementation | annotate | E0283 |  |
| `g17-in-another-macro` | One named in `vec![..]`: inside a `prove!` there, and as a value there | ok | ok |  |
| `g18-snap-mismatch` | A `Snap<Vec<u32>>` passed where a `Snap<bool>` is wanted: Locus's typing must reject it, rustc cannot | ok | ok |  |
| `g19-hand-untyped-passed` | Hand-written: the untyped marker passed where a `Nat` is wanted | E0308 | E0308 |  |
| `g20-logic-where-runtime` | A `logic!` where a `usize` is wanted | reject | E0277 |  |
| `h01-snap-header` | `impl Describe for Snap<u32>`: `locus!` rejects it | error | error |  |
| `h02-proof-header` | `impl From<Option<@(1 == 2)>> for Absurd`: `locus!` rejects it | error | error |  |
| `h03-snap-header-allowed` | Both with the rejection off: which implementation rustc selects at `Snap<bool>`, and for a proof of `2 == 2` | ok | ok |  |
| `h04-snap-header-alias` | `type Shot = Snap<u32>; impl Describe for Shot` | reject | ok | **differs** |
| `h05-hand-lifetime-marker` | Hand-written: a marker with a lifetime parameter, against `Logical: 'static` | reject | E0478,E0803 |  |
| `p01-hand-generic-value-probe` | Hand-written: a generic stub probed as a value, with no marker to write for its type argument | E0283 | E0283 |  |
| `p02-hand-use-trait-member` | Hand-written: `use Trait::member as _;` | E0658 | E0658 |  |
| `t01-two-traits` | `OrderSpec` for a logical `Rational` and a runtime `Version`, `CheckedOrder: OrderSpec` for `Version`, `is_max<T: OrderSpec>`, `max<T: CheckedOrder>` | ok | ok |  |
| `t02-max-rational` | `max::<Rational>` | E0277 | E0277 |  |
| `t03-logical-impl-runs` | `impl CheckedOrder for Rational`: `locus!` rejects the method that runs | error | error |  |
| `t04-library-traits` | `AddLogic`, `FromLogic`, and `IntoLogic` with its blanket implementation, from the library, at a program's type | ok | ok |  |
| `t05-into-missing` | `Bool: IntoLogic<Rational>`, with no conversion declared | E0277 | E0277 |  |
| `t06-family` | An associated logical type that is `Int` at `Int` and a proof at `Nat`, stored in a runtime struct | ok | ok |  |
| `t07-one-byte` | A proof in a trait method's signature, a default body, generic code that uses it | ok | ok |  |
| `t08-weaker-contract` | An `impl` that promises `b < 200` where the trait says `b < 128`: Locus must reject it, rustc cannot | ok | ok |  |
| `t09-monoid` | `Monoid` for `Int` and for a program's logical type, a lemma, a default logic function, generic code | ok | ok |  |
| `t10-kind-fn-for-logic` | The trait says `logic fn le`, the `impl` gives `fn le` | reject | E0046,E0407 |  |
| `t11-kind-logic-for-fn` | The trait says `fn byte`, the `impl` gives `logic fn byte` | reject | E0046,E0407 |  |
| `t12-names-same-module` | `Self::le`, `T::le`, `Rational::le`, `<T as OrderSpec>::le` in logical code, in the module that declares the trait | ok | ok |  |
| `t13a-imported-impl` | The trait imported by name from another module: an `impl` of it | ok | E0277,E0405 | **differs** |
| `t13b-imported-parameter` | The same: `T::le(..)` under `T: OrderSpec` | ok | ok |  |
| `t13c-imported-type` | The same: `Rational::le(..)` | ok | ok / E0599 | **differs** |
| `t13d-imported-qualified` | The same: `<T as OrderSpec>::le(..)` | ok | ok / E0405 | **differs** |
| `t14-imported-in-trait` | t13a to t13d with the stubs kept in the lowered trait | ok | ok |  |
| `t15-names-trait-path` | `OrderSpec::le(r, r)`: the member named through its trait | ok | ok / E0782 | **differs** |
| `t16-names-trait-path-in-trait` | The same with the stubs kept in the lowered trait | ok | ok / E0790 | **differs** |
| `t17-default-member-split` | A defaulted logic function; an `impl` with no members, in another invocation than the trait | ok | E0277 / E0277,E0599 | **differs** |
| `t18-default-member-split-in-trait` | The same with the stubs kept in the lowered trait | ok | ok |  |
| `t19-runtime-name-clash` | A logic function named `eq` on a type with `PartialEq`; Rust code calls `Version::eq` | ok | ok |  |
| `t20-hand-unmangled-clash` | Hand-written: the same if the stub kept the name `eq` | E0034 | E0034 |  |
| `t21-provided-method-split` | A logical type implements a trait whose method that runs has a default body, declared in another invocation | reject | ok | **differs** |
| `t22-provided-method-joined` | The same in one invocation: `locus!` rejects it | error | error |  |

## What it means

1. **An actual lowering can do it.** One text, two expansions, a Rust type for each logical type: the 20 accepted litmus examples that today's lowering compiles still compile both ways with no source changed, and rustc resolves the two-trait pattern, the library's logical traits with a blanket implementation, an associated logical type, and bounds on ghost parameters. rustc now rejects `Vec<Nat>` used as `Vec<Seq<Int>>`.
2. **Two gaps of the one-marker lowering close.** Lowering no longer has to know whether a `logic!` gives a proof: rustc infers the marker. A logical type no longer needs state shared between invocations to be given an `impl`: it is a struct under its own name.
3. **The draft's rule for ghost locals is decidable from syntax for what the corpus writes, and it is needed.** Of the 19 unannotated ghost `let`s in the litmus corpus, the toy found 17 mentioned by logical code alone, and the other two are mentioned in forms it does not parse. With the rule, no example that compiles needed an annotation. Without it, every one of them does.
4. **"That happens where every use is generic" is too narrow.** A typed binding also needs a written type where its runtime uses are not generic and still say nothing: a copy to another local (`g13`). And the rule does not cover a ghost value inside a runtime local (`g14`). Both compile today with no annotation.
5. **A wrong pick can be silent.** Where runtime code mentions a ghost local and the tokens do not show it, lowering leaves it untyped. If the use is generic, rustc accepts, and the local's Rust type is `Untyped`. The draft's check that the type rustc inferred is the type Locus has would have to read `Untyped` as a failure. The pick is also visible when the program runs: a marker's `Debug` output and `type_name` now name its logical type.
6. **Probes change.** With no marker that stands for every logical type, a probe cannot write a stub's type arguments. A probe by `use` covers functions. For a logic function of a generic logical type under a bound, and for a member named through its trait, this spike found no probe that a syntactic lowering can write.
7. **"Not an item of the lowered trait" has a cost that mangling alone avoids.** A hidden trait beside the lowered one breaks for a trait imported by name, in three ways, and needs the trait's declaration to lower an `impl`. Stubs kept in the lowered trait, under names that no Rust code can write, pass those four cases and every case the hidden trait passes, are no candidate for a name Rust code wrote (`t19`), and make rustc check that each `impl` gives each member in its declared kind. The draft's reason for the rule is that no logic function becomes a candidate in rustc's resolution of Rust code. The mangled name gives that by itself.
8. **Lowering cannot make two of the draft's rejections.** A snapshot type behind an alias in a header, and a logical type implementing a trait whose method that runs is provided by a trait it cannot see. The first needs what rustc knows of the alias. The second needs the trait's declaration, which one invocation does not have, and which for a trait of another crate only rustc has. The draft already checks a selected header against the logical types after rustc. The rule on methods that run needs a check there too.
9. **`Logical` cannot be memberless in the lowered program,** if `logic!` is to give a marker whose type rustc infers. Here it has a hidden constructor.
10. **rustc's messages change.** A `logic!` where a runtime type is wanted is E0277, ``the trait bound `usize: Logical` is not satisfied``, where it was E0308, ``expected `usize`, found `LogicalData` `` (`g20`). A ghost local with nothing to infer from is "type annotations needed" at its `let`, with a suggestion to write the type. Errors about traits name `__logic_le` and `__locus_logic_OrderSpec`.

## Where the lowering guesses

Each is marked `GUESS` in `locus-macros/src/lib.rs`.

| What it decides | How | What it would need |
|---|---|---|
| Whether a type is logical | By the name it starts with: the library's, a `#[derive(Logical)]` type of this invocation, `Snap`, or a type parameter whose bound in the parameter list names `Logical` | The crate's items and imports for a type of another module or crate; rustc for an alias, `T::Out`, `Vec<Nat>`, a bound in a `where` clause, or a bound on a trait that has `Logical` as a supertrait |
| Whether runtime code mentions a ghost local | A walk of the rest of its scope that skips `prove!`, `logic!`, `prop!`, propositions in annotations, and ghost statements | What another macro does with its input; names inside string literals (`g15`); which macros are Locus's, by resolution and not by name; which operators and calls outside a macro are logical code (RFC 0002), for which it needs the names of logic functions |
| What a pattern binds | Every lowercase identifier that is not a field label, a path segment, or a call | Name resolution: `limit` in a pattern is a binding, or a constant in scope |
| Where a closure's body ends | A block, or up to the next `,` or `;` | A parser |
| Which component of a tuple a `logic!` initializes | Only for a tuple pattern over a tuple expression of the same length | Nothing more for that form; for any other pattern or expression, the type |
| Whether a path names a type or a module | A capital on the segment before the last | Name resolution |
| Whether `Name::le(..)` names a type or a trait | Not decided: probed as a type (`t15`, `t16`) | Name resolution, and then the type of `Self` |
| The type arguments of a generic logical type in a probe | The untyped marker, for a type whose number of parameters it knows | The logical type of the logical code, which only Locus's own typing has (`d06`) |
| Whether a trait has a hidden trait for its stubs, when lowering an `impl` | Exactly when the `impl` gives a logic function, unless the trait is in this invocation | The trait's declaration (`t17`) |
| Whether a trait has a method that runs, for the rule on logical types | From the methods the `impl` gives, and the trait's declaration if it is in this invocation | The trait's declaration, supertraits included (`t21`) |
| Whether a header writes a snapshot type | The names `Snap` and `CutSnap` | Resolution of aliases (`h04`) |
| Which names are traits and logical types of this invocation | Unqualified names | Paths |

## Not tested

- Locus's own checks. Nothing here reads the checking expansion. That an `impl`'s signatures are its trait's (`t08`), that a `Snap<A>` is not used as a `Snap<B>` (`g18`), and that the type rustc inferred for a ghost local is the type Locus has are not built. So no case shows a program in which rustc infers a logical type that Locus's forward typing would not give.
- The driver. Which implementation a member comes from, whether a bound holds, and what an associated type is, for logical code, were asked of rustc in the first spike and not here. Operators and method calls in logical code are not probed: `x + x` under `T: AddLogic` and `t.into()` are carried as tokens.
- Operators on bindings shown to be ghost, and calls of logic functions, outside a macro. RFC 0002 makes them logical code, and the toy takes them for runtime code.
- A logical type or a Locus trait from another crate. Only another invocation in the same crate was tried.
- `Model` as a trait, law traits and their provided implementations, `#[derive(CloneEq)]`, evidence, `dyn`, export, and unverified crates.
- A bound in a `where` clause, `const fn` stubs, lints of either expansion, rust-analyzer, and rustfmt.
- The rest of the litmus corpus: 35 distinct examples of 454 were lowered.
- Whether the hidden constructor on `Logical`, or `Logical: 'static`, costs anything elsewhere. `h05` shows only that rustc refuses a hand-written marker with a lifetime parameter.
