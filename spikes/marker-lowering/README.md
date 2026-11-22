# Spike: an actual lowering with a Rust type for each logical type

A throwaway experiment for the draft of RFC 0006 on traits, and for [traits-on-logical-types](../../issues/traits-on-logical-types.md). It was built outside the repository, and depends on nothing in the repository when it is built: what it reuses from [`locus-macro`](../locus-macro/README.md) is copied. Nothing may depend on it. (The links assume it is placed under `spikes/`.)

Three variants were tried. This file describes the first, "a type for each logical type", from here to [Two more variants](#two-more-variants). That part describes two others, which were built afterwards beside the first, compares all three case by case, and says what each asks of a program.

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
- `run.sh [all|cases|corpus|ghostlets|typed|intrait|expansions] [name-prefix]` reproduces everything, with more sections for the two later variants. The runtime expansion is `cargo build`; the checking expansion is the same command with `RUSTFLAGS="--cfg __locus_check"`. Each lowering and each expansion has its own target directory. `run.log` is the output of the run that the results below come from.

## Results

Run on 2026-11-22. When the first mode was all there was, `./run.sh` took 81 seconds from an empty `target/` and left 1.8 GB there. Its summary lines:

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

---

# Two more variants

The first run's findings mostly come from putting a distinct Rust type into the type of every ghost value. Two more shapes were tried afterwards, each beside the first mode. The first mode's own results did not change: its part of `run.log` is the same text as before, apart from the time.

- **Second mode, "stub-self".** Program text keeps the one marker. A distinct type, a stub, stands only where an `impl` names a logical type, and questions about stubs go to rustc through a driver.
- **Third variant, "split".** The two expansions differ. The checking expansion is the first mode's, and the runtime expansion keeps one marker.

39 more cases were written for them, in `more/src/bin`: 21 for the second mode (`s`), and 18 from a reviewer's counterexamples and what followed from them (`r`). Every case, the first 58 included, was run under all three.

## Setup of the two variants

- `stub-self/` is a workspace of its own. Its library, `stub-self/locus`, turns on the macro crate's feature `stub-self`.
  - **Program text** is lowered as `locus-macro` lowers it: `Int`, `Nat`, `Bool`, `Prop`, `Seq<T>`, `Snap<T>` and every `#[derive(Logical)]` type are aliases of `LogicalData`, `logic!` and `prop!` give `LogicalData`, and a `logic!` whose `let` says `@P` gives `Erased`. There is no untyped marker and nothing to infer.
  - **Each logical type has a stub,** `__locus_Int`, `__locus_Seq<T>`, `__locus_Rational`: zero-sized, generic where the type is, no lifetime, and no value of it is ever made.
  - **`impl Tr for L`, for a logical type `L`,** is written for `L`'s stub, and every logical type its header names is a stub too. An associated type it gives is a stub. Its logic functions are not written: see the first guess below.
  - **A trait's logic functions stay in the lowered trait,** as `__logic_name`, each with a body that panics, so that an `impl` need not give them.
  - **A trait all of whose members are logical,** its supertraits included, gets one empty implementation for the marker beside its declaration: `impl<Rhs: Logical> AddLogic<Rhs> for LogicalData { type Output = LogicalData; }`. A trait with a method that runs, an associated constant, or an associated type not bounded by `Logical` gets none.
  - **Every probe is of a name:** `use Rational as _;` for `Rational::le(..)`, `use OrderSpec as _;` for `OrderSpec::le(..)`, both for `<Rational as OrderSpec>::le(..)`, and nothing for a type parameter.
  - **The library's exception** is `CloneEq`, with the method `clone_eq(&self) -> (Self, Erased)`, implemented once for `LogicalData`, once for `Erased`, and by one adapter for `Vec<T>`.
  - Three directives: `#![spike(logical(Rational))]` names a logical type that another invocation declares; `#![spike(always_marker_impl)]` writes the marker's implementation regardless; `#![spike(allow_reserved_names)]` turns off the rejection of `__logic_` and `__locus_` names, which is new in every mode.
- `stub-self/oracle/` is the driver, about 490 lines, adapted from the oracle of `logical-markers`. Cargo runs it in place of rustc (`RUSTC_WORKSPACE_WRAPPER`). For one crate, after rustc has checked the checking expansion, it reads the case's `//! ask:` lines, builds the stub types they name, and asks rustc's trait solver: whether a bound holds, which implementation rustc selects, what an associated type is, and each of those under a generic function's bounds. `Snap<X>` in a question is the library's type family `<X as SnapOf>::S`, normalized. It can also list, for each call the source wrote, the item rustc resolved it to.
- `split/` is a workspace of its own. Its library turns on the feature `split`. `locus!` lowers each item twice and writes both, one under each `cfg`.
  - **Checking expansion:** the first mode, with a trait's stubs kept in the lowered trait (the placement that passed in the first run).
  - **Runtime expansion:** the one-marker lowering. A trait all of whose items are logical is implemented for every type, by one blanket implementation written beside it, and each `impl` of such a trait expands to nothing.
  - The library is two files, one for each `cfg`.
- `locus-macros/src/lib.rs` is now about 3,050 lines. What it writes in the first mode is what it wrote before, but for the rejection of reserved names.
- `run.sh` runs all three. Its new sections are `stubcases`, `stubcorpus`, `stubmore`, `three`, `splitcorpus` and `splitcalls`. The driver needs `rustup component add rustc-dev llvm-tools`; without it no question is asked.

## Results of the two variants

From the same run as above (rustc 1.97.0, 2026-11-22). The whole of `./run.sh`, all three variants, took 325 seconds with `target/` already built, and `target/` is 4.9 GB. The new summary lines:

```text
cases: 58; outcome or output differs between the modes: 24
corpus: accepted litmus examples: 20; compile in both expansions: old 20, stub 20; files that differ: 1
corpus: rejected litmus examples that fail in the checking expansion alone: old 2, stub 2
ghost-lets: accepted litmus examples: 10; compile in both expansions: old 8, stub 8; files that differ: 1
its own cases: 39, as expected: 28, differ: 11
questions to the driver: 55, answered as expected: 55
corpus: accepted litmus examples: 20; compile in both expansions: old 20, split 20; files that differ: 2
ghost-lets: accepted litmus examples: 10; compile in both expansions: old 8, split 8; files that differ: 3
cases that both expansions accept: 48; with a call resolved differently: 5
```

### Second mode: the first mode's 58 cases

24 of the 58 change outcome or output. By group:

- **Ghost locals behave as with one marker.** `g06`, `g13`, `g14` and `g16` compile with no annotation. `g02` prints `locus::LogicalData` as the type rustc gave the local, and `g15` prints `"LogicalData"`. `g20` is E0308 again, ``expected `usize`, found `LogicalData` ``.
- **The failures of the hidden trait are gone:** `t13a`, `t13c`, `t13d`, `t17`. So are the probes that could not be written: `d06`, `t15`, `t16` compile, because nothing typed is probed.
- **rustc no longer rejects what only stubs tell apart.** `d03`, a `Vec<Nat>` used as a `Vec<Seq<Int>>`, compiles. `t05`, `Bool: IntoLogic<Rational>` with no conversion declared, compiles. Both are left to Locus and the driver.
- **`t21` is rejected,** where the first mode ran a method on a logical value. The `impl` is for the stub, and the value is the marker: E0599, ``no method named `hello` found for struct `LogicalData` ``. `h04` is rejected too, by accident: the alias of a snapshot type is the marker, and the `impl` collides with the marker's (E0119).
- **Two programs that the first mode compiles fail, and one prints something else:**

  ```text
  == t06-family: new runtime=ok check=ok; stub runtime=E0308 check=E0308; CHANGED
    stub runtime: cases/src/bin/t06-family.rs:28:33: error[E0308]: mismatched types: expected `LogicalData`, found `Erased`
  == t09-monoid: new runtime=ok check=ok; stub runtime=E0308 check=E0308; CHANGED
    stub runtime: cases/src/bin/t09-monoid.rs:42:10: error[E0308]: mismatched types: expected type parameter `M`, found `LogicalData`
  ```

  - `t06`: the associated type is a proof at `Nat`. The marker's implementation says `LogicalData`, and a proof is `Erased`.
  - `t09`: generic code that runs makes a ghost value of type `M` with `logic!`, which gives the one marker. `s19` is the same with `let r: M = logic! { .. };`: `locus!` sees that `M` is a type parameter and makes the value through its bound, and it compiles.
  - `h03`, the header rejection turned off: the `impl` written for `Snap<u32>` is for a stub, and program text gets the marker's, so it prints `LogicalData` where the first mode printed `Nat`.
- `d01` prints `LogicalData` four times for the `Debug` of four logical types. `t10` loses one of its two errors (E0046), because a trait's stubs now have bodies. The hand-written `g19` does not apply: this library has no `Untyped`.

Of the first run's 11 differences, 10 are gone. `d06`, `g14`, `t13a`, `t13c`, `t13d`, `t15`, `t16` and `t17` compile both ways, and `h04` and `t21` are rejected. The eleventh, `g15`, compiles and prints `"LogicalData"`: the first mode's header asks for "type annotations needed" there, a rule this mode does not have. Against that, `t06` and `t09` fail here and not in the first mode, and eight more cases do here what the first mode's rules say they should not: three ghost locals that needed a type and compile (`g06`, `g13`, `g16`), `d03` and `t05`, and the output of `d01`, `g02` and `h03`.

### Second mode: the corpus

The same counts as the one-marker lowering, and no source changed: 20 of 20 accepted litmus examples compile both ways, the 2 rejected ones fail in the checking expansion alone, and 8 of the 10 accepted ghost-let examples compile both ways. `x-logic-kind` and `x-split-logical-type` fail as they do with one marker (E0308, E0118): the two gaps that the first mode closed are open again. The one file that differs in each set is a different error code for the same failure, and the toy's fix for `_`.

### Second mode: its own cases, and the driver's answers

**Two implementations of one logic-only trait, and a generic logical self type** (`s01`, `s02`). Both expansions compile, with no E0119, and the driver tells the stubs apart:

```text
== s02-generic-self: expect runtime=ok check=ok; got runtime=ok check=ok; as expected
  both binaries print: run() => 3
  ask: bound Seq<Rational>: OrderSpec => holds
  ask: bound Seq<Seq<Int>>: OrderSpec => holds
  ask: bound Pair<Rational>: OrderSpec => holds
  ask: bound Pair<Seq<Int>>: OrderSpec => holds
  ask: bound Bool: OrderSpec => does not hold
  ask: bound Seq<Bool>: OrderSpec => does not hold
  ask: bound Pair<Bool>: OrderSpec => does not hold
  ask: impl Seq<Seq<Int>>: OrderSpec => the impl on line 40, at [Seq<Int>]
  ask: impl Pair<Rational>: OrderSpec => the impl on line 46, at [Rational]
  ask: in generic: bound Seq<T>: OrderSpec => holds
  ask: in generic: bound Pair<Seq<T>>: OrderSpec => holds
  ask: in generic: bound T: Monoid => does not hold
  ask: in generic: impl T: OrderSpec => the bound in scope
```

`Pair` is a runtime generic type, and `Pair<Rational>` in a question is that type over a stub.

**The division of labour** (`s03`). `Bool` does not implement `OrderSpec`. Program text passes a `Bool` to `show<T: OrderSpec>` and writes `is_max(flag, flag, flag)`. rustc accepts both, and the driver says no:

```text
== s03-bound-not-met: expect runtime=ok check=ok; got runtime=ok check=ok; as expected
  both binaries print: run() => 2
  ask: bound Bool: OrderSpec => does not hold
  ask: bound Int: OrderSpec => holds
```

`max::<Rational>`, for `CheckedOrder`, which has a method that runs, is still rejected by rustc: `t02` is E0277, ``the trait bound `LogicalData: CheckedOrder` is not satisfied``.

**An associated logical type** (`s04`, `s05`). The driver normalizes on stubs, and in program text `T::Out` is the marker, in a signature and in a runtime struct of 4 bytes:

```text
  ask: assoc Int: Family::Out => Int
  ask: assoc Nat: Family::Out => Seq<Nat>
  ask: assoc Seq<Int>: Family::Out => Seq<Int>
  ask: in keep_out: assoc T: Family::Out => <T as Family>::Out
  ask: assoc Nat: Family::Out => Erased
  ask: assoc Bool: Family::Out => (Nat, Int)
```

The last two are `s05`: a proof type and a tuple, declared and never used in program text. Used, they fail, because the marker's implementation can only say `LogicalData`:

```text
== s06-assoc-proof-used: expect runtime=ok check=ok; got runtime=E0308 check=E0308; DIFFERS
  runtime: more/src/bin/s06-assoc-proof-used.rs:28:33: error[E0308]: mismatched types: expected `LogicalData`, found `Erased`
== s07-assoc-tuple-used: expect runtime=ok check=ok; got runtime=E0308 check=E0308; DIFFERS
  runtime: more/src/bin/s07-assoc-tuple-used.rs:20:37: error[E0308]: mismatched types: expected `LogicalData`, found `(LogicalData, LogicalData)`
```

**The collision** (`s08` to `s13`, `r04`, `r05`). A program's `impl<T: Logical> Named for T` and the marker's implementation conflict, as expected:

```text
== s08-blanket-collision: expect runtime=E0119 check=E0119; got runtime=E0119 check=E0119; as expected
  runtime: more/src/bin/s08-blanket-collision.rs:14:5: error[E0119]: conflicting implementations of trait `Named` for type `LogicalData`: conflicting implementation for `LogicalData`
```

Four arrangements were tried:

- **A helper trait of the library that only stubs implement,** as one more bound on the program's implementation (`s11`, by hand): E0119, and rustc's note says why.

  ```text
  error[E0119]: conflicting implementations of trait `Named` for type `LogicalData`
     = note: upstream crates may add a new impl of trait `locus::__Stub` for type `locus::LogicalData` in future versions
  ```

  Inside the library itself the same arrangement compiles: `stub-self/locus/src/lib.rs` has those two implementations for a trait of its own, `__InLibrary`.
- **An associated type that is one type at every stub and another at the marker** (`s12`, by hand): E0119. rustc does not use it to tell them apart.
- **No implementation for the marker, for a trait that has an implementation for a bare type parameter in the same invocation.** This is what the mode does by default (`s09`). The program's own implementation serves the marker, which is `Logical`. It needs only the syntax of the crate that declares the trait, since the orphan rule puts such an implementation there. It keeps all three basic conversions, the identity, the blanket `Widen` over it, and `impl From2<Nat> for Int` (`r04`), and the driver answers them:

  ```text
    ask: bound Nat: Widen<Int> => holds
    ask: bound Int: Widen<Int> => holds
    ask: bound Bool: Widen<Int> => does not hold
    ask: bound Int: Widen<Nat> => does not hold
    ask: impl Int: From2<Nat> => the impl on line 28
    ask: impl Nat: Widen<Int> => the impl on line 25, at [Nat, Int]
  ```

  With the marker's implementation written regardless, the same program is E0119 twice (`r05`). The arrangement stops where the blanket implementation names a logical type as the trait's argument (`s10`): that is a stub in the header, and the bound in program text asks for the marker.

  ```text
  == s10-blanket-logical-argument: expect runtime=ok check=ok; got runtime=E0277 check=E0277; DIFFERS
    runtime: more/src/bin/s10-blanket-logical-argument.rs:20:36: error[E0277]: the trait bound `LogicalData: Tagged<LogicalData>` is not satisfied: the trait `Tagged<LogicalData>` is not implemented for `LogicalData`
  ```
- **Every stub is `Box<Tag>`,** a wrapper that the orphan rule sees through, over a tag of its own (`s13`, by hand, and not what the lowering does). An implementation for every logical type is then written over `Box<X>`, which the marker is not, and the library's trait can still be implemented for a program's stub. It compiles and prints `stub of Int: every stub | stub of Rational: every stub | marker: the marker | the library's trait at the program's stub: 0`.

**Snapshots** (`s14`, `r06`). `fn sequence(s: Seq<Snap<u32>>)` called from `fn bridge(s: Snap<Vec<u32>>)` compiles, and so does a `Vec<Nat>` used as a `Vec<Seq<Int>>`. For the driver's questions the library has a type family over stubs, and with it the answers are consistent:

```text
  ask: bound Snap<Vec<u32>>: OrderSpec => holds
  ask: bound Seq<Snap<u32>>: OrderSpec => holds
  ask: impl Snap<Vec<u32>>: OrderSpec => the impl on line 33, at [Snap<u32>]
  ask: impl Seq<Snap<u32>>: OrderSpec => the impl on line 33, at [Snap<u32>]
  ask: bound Snap<&mut Vec<u32>>: OrderSpec => holds
  ask: bound Snap<Vec<bool>>: OrderSpec => does not hold
  ask: bound Snap<Nat>: OrderSpec => holds
  ask: bound Nat: OrderSpec => holds
  ask: impl Snap<Nat>: OrderSpec => the impl on line 27
  ask: impl Nat: OrderSpec => the impl on line 27
```

One limit showed. The family cannot be written in a header (`r06b`, by hand): `impl Named for <u32 as SnapOf>::S` is E0119 against the marker's implementation, because rustc does not normalize that type when it checks for overlap.

```text
== r06b-hand-snapshot-family-in-header: expect runtime=E0119 check=E0119; got runtime=E0119 check=E0119; as expected
```

So the header in `r06` names the stub that the family gives for `u32`, which `locus!` can only know for a type with no structure.

**`Vec<Nat>: CloneEq`** through the library's one adapter and the marker's one implementation (`s15`): compiles and prints `(2, 3)`.

**Names** (`s16`, and `t12` to `t16`). With probes of names alone, all four spellings compile in the checking expansion, with the trait imported by name from another module: `OrderSpec::le(r, r)`, `Rational::le(r, r)`, `T::le(*a, *a)`, `<T as OrderSpec>::le(*a, *a)` and `<Rational as spec::OrderSpec>::le(r, r)`. The probe covers the name of the type or the trait. It does not cover the member: no probe says that `le` exists, or of which trait. The driver answers the typed part, the implementation at a type and the bound in scope under a parameter. A method call in logical code, `t.into()`, is still not probed at all.

### Second mode: how lowering knows that a self type is logical

To write the stub, lowering must know that the self type of an `impl` names a logical type.

- **In one invocation** it knows: the type is declared there.
- **For the library's types** the toy knows by name.
- **For a type of another invocation, the toy cannot know.** It sees a name, which is an alias of the marker. It writes the `impl` for the alias, so for `LogicalData`, and the program fails (`s17`):

  ```text
  == s17-other-invocation: expect runtime=ok check=ok; got runtime=E0119 check=E0119; DIFFERS
    runtime: more/src/bin/s17-other-invocation.rs:19:5: error[E0119]: conflicting implementations of trait `OrderSpec` for type `LogicalData`: conflicting implementation for `LogicalData`
  ```

  That error exists only because the trait is logic-only and the marker has it already. For a trait with a method that runs, the same mistake would compile, and give the method to every logical type. Told by `#![spike(logical(Rational))]`, the toy writes the stub (`s18`).
- **A lowering that sees the whole crate** has every `#[derive(Logical)]` of the crate, and needs two more things. From each dependency's interface, which of its exported names are logical types. And, for the path an `impl` writes, which item it names: Rust's name resolution, with imports, globs, re-exports and shadowing, before rustc has run. An alias written in the crate, `type R = Rational;`, needs the same. Nothing in the lowered program can stand in for that knowledge: every alias of every logical type is one type to rustc, so no trick on the alias gives back the stub.

### Second mode: where a reviewer's counterexamples break it

```text
== r01-runtime-wrapper: expect runtime=E0119 check=E0119; got runtime=E0119 check=E0119; as expected
  runtime: more/src/bin/r01-runtime-wrapper.rs:17:5: error[E0119]: conflicting implementations of trait `Tag` for type `Vec<LogicalData>`: conflicting implementation for `Vec<LogicalData>`
== r02-outside-self: expect runtime=ok check=ok; got runtime=E0119 check=E0119; DIFFERS
  runtime: more/src/bin/r02-outside-self.rs:19:5: error[E0119]: conflicting implementations of trait `Spec<LogicalData>` for type `u32`: conflicting implementation for `u32`
== r03-description: expect runtime=reject check=reject; got runtime=E0277 check=E0277; as expected
  runtime: more/src/bin/r03-description.rs:27:19: error[E0277]: the trait bound `LogicalData: Description` is not satisfied: the trait `Description` is not implemented for `LogicalData`
```

- **`r01`, a runtime wrapper.** `impl Tag for Vec<Int>` and `impl Tag for Vec<Nat>`, a trait with a method that runs. The first mode compiles it both ways and prints `(1, 2)`: what runs depends on an erased type. This mode rejects it, E0119. The values have the marker's type, so the two cannot move to stubs.
- **`r02`, a distinction outside `Self`.** `impl Spec<Int> for u32` and `impl Spec<Nat> for u32`, a logic-only trait. The self type is a runtime type, so the header is program text, and both are `Spec<LogicalData> for u32`: E0119. One other arrangement lets both exist and the bound be accepted (`r02b`, by hand): write the trait's argument as a stub in each header and in each bound, and give the marker the trait at every argument. It compiles and prints 4. It costs the blanket conversions (`r02c`, by hand): a logical value then meets `IntoLogic<Int>` only if the stub of `Int` is `FromLogic` of the marker, and it is E0277, ``the trait bound `locus::__locus_Int: FromLogic<LogicalData>` is not satisfied``. `s20` is the same collision in a self type: `impl OrderSpec for Pair<Rational>` and `for Pair<Int>`, for a runtime `Pair`, are E0119.
- **`r03`, content that runs and is not a method.** `Description` has an ordinary associated type and a constant, different at `Int` and `Nat`. The first mode compiles it and prints `((8, 1), (16, 2))`. This mode gives such a trait no implementation for the marker, the two implementations are for stubs, and the generic code that reads `T::WIDTH` at a logical value is E0277.
- **`r04`, `r05`, the blanket conversions:** above. The marker's implementation collides, and leaving it out for such a trait keeps all three.
- **`r06`, snapshot identity:** above.
- **`r07`, `r08`, a mangled name is writable.** An ordinary method `__logic_answer` in a trait with the logic function `answer`: the first mode fails both expansions with E0034, as the reviewer saw, and with the stub in the trait it is E0428, ``the name `__logic_answer` is defined multiple times``. `locus!` now rejects, in every mode, an identifier of the source that starts with `__logic_` or `__locus_`:

  ```text
  == r08-reserved-name: expect runtime=error check=error; got runtime=error check=error; as expected
    runtime: more/src/bin/r08-reserved-name.rs:9:12: error: locus: `__logic_answer` starts with a prefix that is reserved for the names Locus writes: `__logic_` and `__locus_`
  ```
- **`r10` to `r14`: an implementation for a runtime type at one logical argument applies at every one.** `impl Wrap<Int> { fn tag(&self) -> u8 { 1 } }`, beside a trait's `tag` for every `Wrap<T>`. Both expansions of this mode agree with each other and call the inherent method on a `Wrap<Nat>`: `r10` prints 1 where the first mode prints 2, and `r11` takes the branch that holds a proof of `False`. rustc reports no conflict. Locus would have to take the callee from rustc and check its header against the logical types.

### Third variant: the expectations

Each was checked against the run. They hold, with two things to say plainly.

- **Ghost locals.** Every ghost-local case compiles in the runtime expansion with no annotation, and "type annotations needed" appears in the checking expansion alone: `g06`, `g13`, `g14`, `g16` are `ok / E0283` or `ok / E0282`. `g20`, a `logic!` where a `usize` is wanted, fails in both, with a different error in each.
- **`Tag for Vec<Int>` and `Vec<Nat>`** (`r01`): the runtime expansion rejects it, E0119, and the checking expansion accepts it.
- **`Spec<Int>` and `Spec<Nat>` for `u32`** (`r02`): both expansions compile, and print 3.
- **`Description`** (`r03`): the runtime expansion rejects it, E0119, and the checking expansion accepts it.
- **The blanket conversions** (`r04`): both compile, and print 7. As first written the case also passed `logic! { 5 }` straight to a generic function, and the checking expansion asked for its type (E0283). That is the first mode's rule for ghost values, and nothing about conversions, so the case no longer does it.
- **Not as expected: a program the checking expansion accepts can fail in the runtime expansion for the one-marker reasons.** `t06`, `s06` and `s07`, an associated type that is a proof or a tuple: E0308 in the runtime expansion. `t09`, a ghost value of a type parameter's type: E0308 there. `x-logic-kind` and `x-split-logical-type` in the corpus: E0308 and E0118 there. And `t17`, `t18`: an `impl` with no members, of a logic-only trait declared in another invocation, does not expand to nothing, because the toy cannot tell that the trait is logic-only, and it collides with the blanket implementation (E0119).
- **Not as expected: the two expansions print different things without calling different code.** A marker's `Debug` output and `type_name` are its logical type's in one and `LogicalData` in the other: `d01`, `g02`, `g15`, `h03`, `s04`. `TypeId` tells `Int` from `Nat` in one and not in the other (`r15`).

The corpus: 20 of 20 accepted litmus examples compile both ways, and the 2 rejected ones fail in the checking expansion alone.

### Third variant: both expansions compile, and run different code

"Both expansions compile" is not enough. Five cases compile both ways and call a different item in each:

```text
== r10-inherent-beats-trait | runtime=ok check=ok prints [run() => 2] | runtime=ok check=ok prints [run() => 1] | runtime=ok check=ok RUNTIME PRINTS [run() => 1] CHECKING PRINTS [run() => 2]
== r13-autoref-order | runtime=ok check=ok prints [run() => (2, 2)] | runtime=ok check=ok prints [run() => (1, 1)] | runtime=ok check=ok RUNTIME PRINTS [run() => (1, 1)] CHECKING PRINTS [run() => (2, 2)]
== r14-associated-constant | runtime=ok check=ok prints [run() => 2] | runtime=ok check=ok prints [run() => 1] | runtime=ok check=ok RUNTIME PRINTS [run() => 1] CHECKING PRINTS [run() => 2]
```

Each line is the first mode, then the second, then the split variant.

- **`r10`, the reviewer's case, through the actual lowering.** A runtime `Wrap<T>` with an inherent `tag` at `Wrap<Int>` and a trait's `tag` at every `Wrap<T>`. On a `Wrap<Nat>` the checking expansion calls the trait's and prints 2, and the runtime expansion calls the inherent one and prints 1.
- **`r11`, with contracts.** The inherent `flag` returns `true` with a proof of `b`, and the trait's returns `false` with a proof of `!b`. The checking expansion prints `"the branch was not taken"`. The runtime expansion prints `"the branch that holds a proof of False was taken"`: the caller is inside `if b`, holding what Locus checked as a proof of `!b`.
- **`r09`, found while looking where the brief pointed.** `impl<T: Spec> Runs for T`, with `Spec` logic-only, beside `Fallback` for an `Inner` that does not have `Spec`. On a `Box<Inner>` the checking expansion finds nothing on the box, looks inside, and calls `Fallback::describe`. In the runtime expansion every type has `Spec`, so the box itself has `Runs::describe`. No inherent method and no logical type argument is involved.

**Can it arise other than by an inherent method beating a trait's?**

- **A trait implemented for `Wrap<Int>` only, and another trait with the same method name** (`r12`): not this way. With one marker both apply to `Wrap<Nat>` at the same step of rustc's lookup, and the runtime expansion is E0034.
- **The order of autoref** (`r13`, first pair): yes. One trait takes `self` and is implemented for `Wrap<Int>`; another takes `&self` and is implemented for every `Wrap<T>`. rustc tries the receiver by value first, so the runtime expansion calls the first and the checking expansion the second.
- **The order of deref** (`r13`, second pair, and `r09`): yes. A trait for `Box<Wrap<Int>>` against a trait for every `Wrap<T>`, on a `Box<Wrap<Nat>>`.
- **An associated constant** (`r14`): yes. An inherent constant at `Wrap<Int>` beats the trait's at every `Wrap<T>`, as an inherent method does.
- **The identity of a type argument** (`r15`): yes, and with no different callee. `TypeId::of::<T>() == TypeId::of::<Int>()` is `(true, false)` for an `Int` and a `Nat` in the checking expansion and `(true, true)` in the runtime expansion.

### Third variant: what a correspondence check needs

The driver can list, for each call, method call and use of an associated constant that the source wrote, the item rustc resolved it to and, where the type arguments say, the implementation it comes from. Run on both expansions of `r10`:

```text
main:034:031 => the definition at 29:5 | run with []
of_nat:027:039 => the definition at 18:9 | Wrap::<LogicalData>::tag with []
run:029:022 => the definition at 27:5 | of_nat with []
--
main:034:031 => the definition at 29:5 | run with []
of_nat:027:039 => the definition at 24:9 | Tag::tag -> <Wrap<T> as Tag>::tag with [Wrap<Nat>]
run:029:022 => the definition at 27:5 | of_nat with []
```

The listings differ at the call in `of_nat`: line 18 is the inherent method, and line 24 the trait's. Over every case that both expansions accept:

```text
== r09-two-expansions-two-impls: the two expansions resolve a call differently
  runtime:  run:036:015 => the definition at 23:9 | Runs::describe -> <T as Runs>::describe with [std::boxed::Box<Inner, std::alloc::Global>]
  checking: run:036:015 => the definition at 31:9 | Fallback::describe -> <Inner as Fallback>::describe with [Inner]
== r10-inherent-beats-trait: the two expansions resolve a call differently
  runtime:  of_nat:027:039 => the definition at 18:9 | Wrap::<LogicalData>::tag with []
  checking: of_nat:027:039 => the definition at 24:9 | Tag::tag -> <Wrap<T> as Tag>::tag with [Wrap<Nat>]
== r11-inherent-beats-trait-proof: the two expansions resolve a call differently
  runtime:  of_nat:029:028 => the definition at 15:9 | Wrap::<LogicalData>::flag with []
  checking: of_nat:029:028 => the definition at 21:9 | Flag::flag -> <Wrap<T> as Flag>::flag with [Wrap<Nat>]
== r13-autoref-order: the two expansions resolve a call differently
  runtime:  of_nat:042:069 => the definition at 20:9 | ByValue::tag -> <Wrap<LogicalData> as ByValue>::tag with [Wrap<LogicalData>]
  checking: of_nat:042:069 => the definition at 26:9 | ByReference::tag -> <Wrap<T> as ByReference>::tag with [Wrap<Nat>]
  runtime:  of_nat:042:082 => the definition at 33:9 | OnBox::mark -> <std::boxed::Box<Wrap<LogicalData>> as OnBox>::mark with [std::boxed::Box<Wrap<LogicalData>, std::alloc::Global>]
  checking: of_nat:042:082 => the definition at 39:9 | Inside::mark -> <Wrap<T> as Inside>::mark with [Wrap<Nat>]
== r14-associated-constant: the two expansions resolve a call differently
  runtime:  run:022:035 => the definition at 13:9 | Wrap::<LogicalData>::TAG with []
  checking: run:022:035 => the definition at 19:9 | HasTag::TAG -> <Wrap<T> as HasTag>::TAG with [Wrap<Nat>]
cases that both expansions accept: 48; with a call resolved differently: 5
```

No other case that both expansions accept resolves a call differently. What the listing took to be usable:

- **It compares where a definition is written,** not its path. A path can name a logical type, which the two expansions write differently.
- **It skips what lowering wrote:** probes, and the bodies of generated items, which the checking expansion alone has. They carry the span of a source token that is not their name, and the driver tells them by that.
- **It resolves a trait's item to its implementation** only where the type arguments have no parameter. In generic code it compares the trait's item, and a difference that appears only at an instantiation is not seen.
- **It does not compare type arguments,** which differ by design. So it does not see `r15`, nor the `Debug` and `type_name` differences: for those, the type arguments of the checking expansion would have to erase to the runtime expansion's, and the program would have to be kept from observing them.

## Every case under the three variants

`ok` is both expansions compiling. One code is that error in both, and `a / b` is the runtime expansion and then the checking expansion. Where two variants, or the two expansions of one, print different lines, the row is marked and each cell says what it prints. The `-hand-` cases are plain Rust against one variant's library, and fail to resolve its names under the others.

| Case | First mode | Stub-self | Split | |
|---|---|---|---|---|
| `d01-derive` | ok: `sizes: Rational 0, Tree 0, Pair<Tree> 0, List<Rational> 0, Ta...` | ok: `sizes: Rational 0, Tree 0, Pair<Tree> 0, List<Rational> 0, Ta...` | ok; runs `sizes: Rational 0, Tree 0, Pair<Tree> 0, List<Rational> 0, Ta...`, checked `sizes: Rational 0, Tree 0, Pair<Tree> 0, List<Rational> 0, Ta...` | **differs** |
| `d02-derive-lifetime` | error | error | error |  |
| `d03-vec-mismatch` | E0308 | ok | ok / E0308 |  |
| `d04-no-equality` | E0369 | E0369 | E0369 |  |
| `d05-field-type-unknown` | ok / E0425 | ok / E0425 | ok / E0425 |  |
| `d06-bounded-impl-probe` | ok / E0599 | ok | ok / E0599 |  |
| `g01-only-logical` | ok | ok | ok |  |
| `g02-takes-nat` | ok: `("Nat", 1)` | ok: `("LogicalData", 1)` | ok; runs `("LogicalData", 1)`, checked `("Nat", 1)` | **differs** |
| `g03-two-borrows` | ok | ok | ok |  |
| `g04-returned-reference` | ok | ok | ok |  |
| `g05-mutable-borrow` | ok | ok | ok |  |
| `g06-generic-only` | E0283 | ok | ok / E0283 |  |
| `g07-generic-annotated` | ok | ok | ok |  |
| `g08-shadowing` | ok | ok | ok |  |
| `g09-closure-capture` | ok | ok | ok |  |
| `g10-pattern` | ok | ok | ok |  |
| `g11-loop-assignment` | ok | ok | ok |  |
| `g12-snapshot-of-mut` | ok | ok | ok |  |
| `g13-copied` | E0282 | ok | ok / E0282 |  |
| `g14-tuple-component` | E0282 | ok | ok / E0282 |  |
| `g15-format-capture` | ok: `"Untyped"` | ok: `"LogicalData"` | ok; runs `"LogicalData"`, checked `"Untyped"` | **differs** |
| `g16-single-implementation` | E0283 | ok | ok / E0283 |  |
| `g17-in-another-macro` | ok | ok | ok |  |
| `g18-snap-mismatch` | ok | ok | ok |  |
| `g19-hand-untyped-passed` | E0308 | E0432 | E0432 / E0308 |  |
| `g20-logic-where-runtime` | E0277 | E0308 | E0308 / E0277 |  |
| `h01-snap-header` | error | error | error |  |
| `h02-proof-header` | error | error | error |  |
| `h03-snap-header-allowed` | ok: `<Snap<bool> as Describe>::Width is Nat; <Snap<&mut Vec<String...` | ok: `<Snap<bool> as Describe>::Width is LogicalData; <Snap<&mut Ve...` | ok; runs `<Snap<bool> as Describe>::Width is LogicalData; <Snap<&mut Ve...`, checked `<Snap<bool> as Describe>::Width is Nat; <Snap<&mut Vec<String...` | **differs** |
| `h04-snap-header-alias` | ok | E0119 | ok |  |
| `h05-hand-lifetime-marker` | E0478,E0803 | E0478,E0803 | E0119,E0407 / E0478,E0803 |  |
| `p01-hand-generic-value-probe` | E0283 | E0283 | E0283 |  |
| `p02-hand-use-trait-member` | E0658 | E0658 | E0658 |  |
| `t01-two-traits` | ok | ok | ok |  |
| `t02-max-rational` | E0277 | E0277 | E0277 |  |
| `t03-logical-impl-runs` | error | error | error |  |
| `t04-library-traits` | ok | ok | ok |  |
| `t05-into-missing` | E0277 | ok | ok / E0277 |  |
| `t06-family` | ok | E0308 | E0308 / ok |  |
| `t07-one-byte` | ok | ok | ok |  |
| `t08-weaker-contract` | ok | ok | ok |  |
| `t09-monoid` | ok | E0308 | E0308 / ok |  |
| `t10-kind-fn-for-logic` | E0046,E0407 | E0407 | ok / E0046,E0407 |  |
| `t11-kind-logic-for-fn` | E0046,E0407 | E0046,E0407 | E0046,E0407 |  |
| `t12-names-same-module` | ok | ok | ok |  |
| `t13a-imported-impl` | E0277,E0405 | ok | ok |  |
| `t13b-imported-parameter` | ok | ok | ok |  |
| `t13c-imported-type` | ok / E0599 | ok | ok |  |
| `t13d-imported-qualified` | ok / E0405 | ok | ok |  |
| `t14-imported-in-trait` | ok | ok | ok |  |
| `t15-names-trait-path` | ok / E0782 | ok | ok / E0790 |  |
| `t16-names-trait-path-in-trait` | ok / E0790 | ok | ok / E0790 |  |
| `t17-default-member-split` | E0277 / E0277,E0599 | ok | E0119 / ok |  |
| `t18-default-member-split-in-trait` | ok | ok | E0119 / ok |  |
| `t19-runtime-name-clash` | ok | ok | ok |  |
| `t20-hand-unmangled-clash` | E0034 | E0034 | E0034 |  |
| `t21-provided-method-split` | ok | E0599 | ok |  |
| `t22-provided-method-joined` | error | error | error |  |
| `r01-runtime-wrapper` | ok | E0119 | E0119 / ok |  |
| `r02-outside-self` | ok | E0119 | ok |  |
| `r02b-hand-outside-self-stub-arguments` | E0432 | ok | E0432 |  |
| `r02c-hand-stub-arguments-and-blanket` | E0432 | E0277 | E0432 |  |
| `r03-description` | ok | E0277 | E0119 / ok |  |
| `r04-blanket-conversions` | ok | ok | ok |  |
| `r05-blanket-conversions-marker` | ok | E0119 | ok |  |
| `r06-snapshot-identity` | E0119 | ok | ok / E0119 |  |
| `r06b-hand-snapshot-family-in-header` | E0432 | E0119 | E0432 |  |
| `r07-reserved-name-allowed` | E0034 | E0308,E0428 | E0308,E0428 |  |
| `r08-reserved-name` | error | error | error |  |
| `r09-two-expansions-two-impls` | ok: `"the fallback, for Inner"` | ok: `"the fallback, for Inner"` | ok; runs `"the blanket implementation, for a type that meets Spec"`, checked `"the fallback, for Inner"` | **differs** |
| `r10-inherent-beats-trait` | ok: `2` | ok: `1` | ok; runs `1`, checked `2` | **differs** |
| `r11-inherent-beats-trait-proof` | ok: `"the branch was not taken"` | ok: `"the branch that holds a proof of False was taken"` | ok; runs `"the branch that holds a proof of False was taken"`, checked `"the branch was not taken"` | **differs** |
| `r12-trait-for-one-argument` | ok | E0034 | E0034 / ok |  |
| `r13-autoref-order` | ok: `(2, 2)` | ok: `(1, 1)` | ok; runs `(1, 1)`, checked `(2, 2)` | **differs** |
| `r14-associated-constant` | ok: `2` | ok: `1` | ok; runs `1`, checked `2` | **differs** |
| `r15-type-identity` | ok: `(true, false)` | ok: `(true, true)` | ok; runs `(true, true)`, checked `(true, false)` | **differs** |
| `s01-two-impls` | E0277 | ok | ok / E0277 |  |
| `s02-generic-self` | ok | ok | ok |  |
| `s03-bound-not-met` | E0277 | ok | ok / E0277 |  |
| `s04-assoc-type` | ok: `(15, 4, 4, "Seq<Nat>")` | ok: `(15, 4, 4, "LogicalData")` | ok; runs `(15, 4, 4, "LogicalData")`, checked `(15, 4, 4, "Seq<Nat>")` | **differs** |
| `s05-assoc-proof-declared` | ok | ok | ok |  |
| `s06-assoc-proof-used` | ok | E0308 | E0308 / ok |  |
| `s07-assoc-tuple-used` | ok | E0308 | E0308 / ok |  |
| `s08-blanket-collision` | ok | E0119 | ok |  |
| `s09-blanket-serves-marker` | ok | ok | ok |  |
| `s10-blanket-logical-argument` | ok | E0277 | ok |  |
| `s11-hand-blanket-helper` | E0432 | E0119 | E0432 |  |
| `s12-hand-blanket-kind` | E0432 | E0119 | E0432 |  |
| `s13-hand-blanket-boxed` | E0432 | ok | ok / E0432 |  |
| `s14-snapshot-passage` | E0308 | ok | ok / E0308 |  |
| `s15-clone-eq` | E0405 | ok | E0405 |  |
| `s16-names` | E0277,E0405 | ok | ok / E0790 |  |
| `s17-other-invocation` | ok | E0119 | ok |  |
| `s18-other-invocation-told` | ok | ok | ok |  |
| `s19-value-of-parameter-type` | ok | ok | ok |  |
| `s20-runtime-type-over-logical` | ok | E0119 | ok |  |
| `s21-logic-argument-generic` | E0283 | ok | ok / E0283 |  |

## What each variant asks of a program

Stated as rules a programmer could be told. Each is tied to the case that showed it. A rule marked "not enforced" is one that rustc does not give in that variant, and that Locus would have to check.

**First mode: a type for each logical type, in both expansions.**

1. Write the type of a ghost local that runtime code mentions, wherever rustc cannot find it: where every use is generic (`g06`, `g16`), and where it is only copied (`g13`). A `logic!` passed straight to a generic function has no place to write it, and must be bound first (`s21`). A ghost value inside a larger runtime value needs the whole type written (`g14`).
2. Write no snapshot type, proof type or dependent tuple in the header of an `impl` (`h01`, `h02`), nor an alias of one (`h04`, not enforced).
3. A logical type implements only traits with no method that runs (`t03`, `t22`; not enforced where the method is provided and the trait is out of view, `t21`).
4. A logical type has no lifetime parameter (`d02`).
5. Write no name that starts with `__logic_` or `__locus_` (`r08`).
6. Not enforced, and needed if nothing that runs may depend on an erased type: no implementation of a trait with content that runs for a runtime type that differs from another only in a logical argument (`r01` prints `(1, 2)`; `r10` to `r14`); no associated constant or ordinary associated type in a trait that a logical type implements (`r03`); no `TypeId`, `type_name` or `Debug` of a ghost value's type (`r15`, `g02`, `d01`).

**Second mode: stub-self.**

1. No annotation on a ghost local, ever, except one: a ghost value whose type is a type parameter is written `let r: M = logic! { .. };` (`t09`, `s19`).
2. An associated logical type is a logical type whose values are the marker: not a proof type, not a tuple (`t06`, `s06`, `s07`).
3. A logical type implements only traits with no content that runs: no method, no associated constant, no ordinary associated type (`t02`, `t21`, `r03`). The library's `Clone`, `Copy`, `Debug` and `CloneEq` are the exception.
4. No two implementations that differ only in a logical type inside a runtime type, as the self type or as a trait's argument (`r01`, `r02`, `s20`).
5. An implementation for every logical type does not name a logical type as its trait's argument (`s10`).
6. An `impl` for a logical type is written where lowering knows the type is logical (`s17`, `s18`).
7. Rules 2, 4 and 5 of the first mode.
8. Not enforced: no implementation, inherent or of a trait, for a runtime type at one logical argument. rustc applies it at every logical argument (`r10` prints 1, `r11` takes the absurd branch, `r13`, `r14`), and reports nothing.
9. Not enforced by rustc, by design: that a bound on a logic-only trait holds at a logical type (`s03`, `t05`), that two logical types are the same (`d03`, `s14`), and which implementation a logic function comes from. The driver answers these, and Locus must ask at every such use.

**Third variant: split.**

1. Rule 1 of the first mode: the checking expansion asks for the same annotations.
2. Rules 1 and 2 of the second mode: the runtime expansion fails the same way (`t09`, `t06`).
3. Rules 2 to 5 of the first mode.
4. Enforced by the runtime expansion: no two implementations of a trait with content that runs that differ only in a logical type (`r01`, `r03`: E0119).
5. Not enforced, and needed: nothing that rustc resolves may depend on a logical type, or on whether a logic-only bound holds. That rules out an inherent method or constant for a runtime type at one logical argument (`r10`, `r11`, `r14`), a trait with content that runs implemented for such a type (`r13`), and an implementation of a trait with content that runs bounded by a logic-only trait (`r09`). The correspondence listing above finds these five. A rule would have to be stated on implementations, and `r09` shows it cannot stop at logical type arguments.
6. Not enforced: no `TypeId`, `type_name` or `Debug` of a ghost value's type (`r15`, `g02`, `d01`, `s04`). The listing does not find these.

## Guesses in the two variants

Beside those of the first mode, which the checking expansion of the split variant keeps:

| What it decides | How | What it would need |
|---|---|---|
| Whether the self type of an `impl` is a logical type, to write its stub (stub-self) | The library's names, the `#[derive(Logical)]` types of this invocation, and a directive | Each dependency's list of logical types, and name resolution of the path before rustc runs (`s17`) |
| The signature of a logic function in an `impl` for a logical type (stub-self) | Not written at all: the trait's stub has a body | The trait's declaration. The trait says `Self` and `Bool` where the `impl` says `Rational` and `Bool`, and only position tells a stub from the marker |
| Whether a trait is logic-only, for the marker's implementation and the blanket one | From its declaration in this invocation; a supertrait of another invocation by a list of names | The supertrait's declaration |
| Whether a trait has an implementation for a bare type parameter (stub-self) | The `impl`s of this invocation | The crate that declares the trait, which the orphan rule makes enough |
| The stub of a snapshot type in a header (stub-self, rejection off) | The opaque stub, right only for a type with no structure | The snapshot family, which rustc will not normalize in a header (`r06`) |
| Whether an `impl` expands to nothing in the runtime expansion (split) | The trait is logic-only in this invocation, or one of the library's by name, or the `impl` gives a logic function and nothing that runs | The trait's declaration (`t17`, `t18`) |
| Which calls the source wrote, in the driver's listing | The source text at the call is the item's name | A mark from lowering on what it generates |

## Not tested, in the two variants

- Locus's side of the second mode: nothing asks the driver at each use, and nothing checks a selected implementation's header against the logical types. The questions are written by hand in each case.
- A method call or an operator in logical code, `t.into()` and `x + x`: carried as tokens, and never asked of the driver.
- Another crate. Every case is one crate, and "another invocation" stands in for it. The marker's implementation and the blanket implementation of a trait from a dependency, and the driver finding stubs there, were not tried beyond the library.
- The `Box<Tag>` arrangement of `s13` and the stub arguments of `r02b`, in the lowering. Both are hand-written Rust.
- The split variant with the hidden-trait placement in its checking expansion, and the split variant's own corpus of ghost-let annotations.
- A correspondence check that compares type arguments, or that sees a difference inside generic code at an instantiation.
- Whether the split variant's two libraries can be one crate in a real build, where a dependency is compiled once for each `cfg`: here `RUSTFLAGS` rebuilds everything.
