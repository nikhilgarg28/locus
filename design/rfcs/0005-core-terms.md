# RFC 0005: Core terms

- **Status:** draft
- **Issue:** [proof-construction](../../issues/proof-construction.md), [runtime-semantics](../../issues/runtime-semantics.md)
- **Supersedes:** none. It restates rules of [RFC 0002](0002-logical-layer.md), [RFC 0003](0003-proof-construction.md), and [RFC 0004](0004-runtime-code.md), which are still being decided.

## Summary

Every logical expression is translated to a term of a small core language before any rule about proofs applies, and the rules for equality, rewriting, inferred values, and evaluation are stated on those terms. The core makes explicit what source leaves implicit: which version of a place is meant, whether a value is read as its snapshot or its model, that machine arithmetic wraps, and what a runtime condition evaluated to. This RFC also says what the context holds at each point of a function, when two propositions are the same, and what a proof may cite.

## Motivation

Two reviews of [PR #25](https://github.com/nikhilgarg28/locus/pull/25) found, between them, six programs that the rule text accepts and that prove something false, and about forty smaller gaps. Most have one cause: the rules of RFC 0003 and RFC 0004 speak of source text.

```rust prose accepted by the rules as RFC 0003 stated them
fn small_sum(a: u32, b: u32) -> Option<@(a + b < 10)> {
    if small: a + b < 10 { Some(small) } else { None }
}
```

In a release build `small_sum(u32::MAX, 5)` returns a proof. The program compared the wrapped sum, 4, and the proposition is about the true one. The rule said that `small` proves "`c`", and `c` means one thing as runtime code and another as a proposition.

```rust prose accepted by the rules as RFC 0003 stated them
logic fn x_of(o: &Outer) -> Nat { o.inner.x }

fn read(a: &Outer, b: &Outer, same: @(a === b)) -> @(x_of(a) === x_of(b)) {
    eq::congr(x_of, same)
}
```

`a === b` compares models, and `Outer`'s model forgets `x`. The rule for `eq::congr` spoke of "`a`" and "`b`", and did not say that the `a` in `a === b` and the `a` in `x_of(a)` are different things.

A third gap is not an attack. RFC 0003 used "evaluation" for what `compute` does and for what a hole allows of a known proof, and never defined it. Its own examples need two different definitions, and RFC 0002's decision 49 had left the question of when two propositions are the same to it.

## Design

### Terms

| Core term | What it is |
|---|---|
| A value name, written `n₁` here | The snapshot of one version of a runtime place, of type `Snap<T>`. A variable: nothing replaces it by another term |
| `model(t)` | The model of a snapshot, for a type that has one |
| `wrap` at a machine integer type | An integer brought into the type's range, exactly as the machine does |
| The value a borrow ends with | What `final!(r)` denotes, for one borrow |
| Literals, constructors, fields, `if`, `match`, logical closures, calls of logic functions, `forall`, `exists`, `===`, comparisons | As in RFC 0002. `<` and `>` are separate, and neither is defined from the other |
| `s.len()`, `s.get(i)`, `s.at(i, p)`, `s.update(i, v)`, `s[i]` | Sequences, [below](#sequences) |

A program never writes a value name. Diagnostics show one as the place and where it got that value, such as "`n`, after line 12".

### From source to core

1. **A place becomes the value name of its current version,** with its projections: `p.x` is `p₁.x`.
2. **A runtime value used as a value becomes the model of that.** An operand, a method's receiver, either side of `===`, and an argument of a logical parameter are such uses. `snap(x)` is the value name itself.
3. **A logic function with a runtime-typed parameter receives the snapshot.**

So `same: @(a === b)` is a proof of `model(a₁) === model(b₁)`, and `x_of(a)` is `x_of(a₁)`. `eq::congr(x_of, same)` asks for `x_of` of a model, which is a type error: `x_of` takes a snapshot. `big.rewrite(same)`, for a proof `big` of `a.inner.x > 5`, finds no `model(a₁)` in `a₁.inner.x`, and is an error too.

### Wrapping

For a machine integer type of `w` bits, with `M = 2^w`, `wrap` of an integer `x` is:

- unsigned: the Euclidean remainder of `x` by `M`;
- signed: the Euclidean remainder of `x + M/2` by `M`, less `M/2`.

The Euclidean remainder is never negative, which Locus's `%` is not: `%` truncates toward zero, as Rust's does. A program writes `wrap` as a logic function of the type, `u32::wrap(a + b)`.

- `a + b`, `a - b`, `a * b`, and `-a` in runtime code have the value `wrap` of the true result, at the operation's type.
- `x as U` between integer types has the value `wrap` of `x` at `U`. So `(-1i32) as u32` is 4294967295, and nothing follows about the sign of `x`.
- This definition is exact. What `linear` can do with it is a separate matter and is less: it knows that `wrap` gives a value in range, and that it changes nothing in range.

### The context

What Locus knows at a point is the proofs in scope there and the *context*: facts about value names. **Every fact has one source,** a rule of the runtime semantics, and is in the context only where execution has passed that source. No rule gives a fact because a proof asks for one. That these rules say what the program does is trusted (`trusted.obligations`), and is to be tested rule by rule against rustc.

| Source | What it gives |
|---|---|
| An understood operation, once it has returned | A value name for its result, the fact that this equals the operation's value over its operands' names, and the condition under which it returned. Operands are read in Rust's order of evaluation |
| A call, once it has returned | A value name for its result, and the proofs its signature returns |
| A condition | One `Bool` term. Where it held the term is a fact, and where it did not, its negation is |
| A write | A new value name for the place. For a field or an element, the value that holds it has a new name too, equal to the old value updated there |
| `for i in a..b` | `lo <= i && i < hi`, for the values `lo` and `hi` that the bounds had when the range was built |
| A join | For a place that some branch wrote, a new name equal to the `if` of the branches' values |
| The end of a mutable borrow | A new name for the place, equal to the value the borrow ended with |
| The top of a loop | A new name, and no fact, for each place the loop writes |

**A condition is what the program checked.** In `if small: a + b < 10`, the term is `u32::wrap(a + b) < 10`, so that is what `small` proves, and `Some(small)` is rejected where an `@(a + b < 10)` is expected. A condition with a call in it can be named too: the call's result has a value name, and the proof says that this one result was `true`. It says nothing about another call.

**A join keeps values and drops one-sided facts.**

```rust
fn clamp(mut n: u32) -> (out: u32, @(out <= 100)) {
    if n > 100 {
        n = 100;
    }
    (n, _)
}
```

After the `if`, `n` has a new name `n₃`, with `n₃ === if n₁ > 100 { 100 } else { n₁ }`. `linear` splits on the `if`, so the hole is filled. A statement and an `if` expression that do the same thing now mean the same. A fact learned in one branch only, such as an index having returned there, is not in the context afterwards.

**A mutable borrow.**

```rust
struct Pair {
    a: u32,
    b: u32,
}

fn bump(p: &mut Pair, room: @(p.a < 100)) -> @(final!(p).a == p.a + 1 && final!(p).b == p.b) {
    let r = &mut p.a;
    *r += 1;
    _
}
```

- While a mutable borrow lives, the place it borrows has no value name. The reference's target has names of its own: the first equals the value the place had, and each write through the reference gives a new one. Here the target is `t₁ === p₁.a`, and after the write `t₂ === u32::wrap(t₁ + 1)`.
- When the borrow ends, the value it ends with equals the target's name at that point, and the place gets a new name equal to it. Without this the final value would be a symbol related to nothing.
- For a borrow of a part, the whole value's new name is its old value with that part replaced: `p₂ === Pair { a: t₂, b: p₁.b }`. A reborrow is a borrow of the target, and ends the same way.
- At return, the borrow that `p` holds ends at `p₂`, so `final!(p)` is `p₂`. Both fields of `p₂` come from the constructed value by conversion, `room` puts `t₁ + 1` in range, and `linear` fills the hole.
- Where a reborrow outlives the function that made it, as when `first_mut` returns `&mut pair.0`, the same equation is known there between the two final values: the pair ends as its old value with that part replaced by whatever the returned reference ends with. Both are still to come, which is why a proof may speak of them ([below](#a-borrows-final-value)).
- A borrow given to a call ends in the callee or when the call returns, and the caller then knows of the place what the callee's proofs say of the value the borrow ended with.

**A loop is an induction.**

```rust
fn count_up() -> (n: u32, @(n <= 10)) {
    let mut n = 0;
    let mut bounded: @(n <= 10) = _;
    while n < 10 {
        n += 1;
        bounded = _;
    }
    (n, bounded)
}
```

- At the top, each place the loop writes gets a new name, about which the context holds nothing.
- A proof variable that the loop assigns is an *invariant* of the loop when its version proves its proposition at the names of the path into the loop, and at those of every path from the body back to the top, a `continue` included. Locus checks each of those.
- It then checks the body with a version that proves the proposition at the top's new names. That is the induction step, and it is the only thing assumed.
- A proof variable that the loop assigns and that is not an invariant has no version at the top. Reading it before it is assigned again is an error, as reading a variable that some path left uninitialized is in Rust.
- A proof variable that the loop does not assign keeps its version: a proof about the values it was made from.
- Leaving the loop because the condition did not hold carries what is known at the top, with the negated condition. A `break` carries what is known at the `break`.

**A `for` range is built once.** `for i in a..b` evaluates `a` and then `b` before the first iteration, and its fact is about those two values. `for i in 0..xs.len()` gives `i < xs.len()` for the current `xs` only while the loop does not write `xs`.

### Kept proof variables

`let mut ok: @(n < 10)` declares a template over the places it reads. Each assignment makes a new *version* of the variable's contents, whose type is the template at the value names of that moment: `@(n₁ < 10)`, and later `@(n₂ < 10)`.

- **No version is ever given another type.** After `n` is written, `ok` still holds a proof about the earlier value. This is what RFC 0004 meant by "a proof is about the values it was made from", and it is the one place where a variable's declared type is a template and not one proposition.
- **Only a local of type `@P` is such a variable.** A `let mut` of type `Option<@P>` keeps the proposition of its declaration, as every other type that holds a proof does.
- **It is not mutably borrowed, and no closure assigns it.** A `&mut` to it would need a type that follows the template, which no reference has.
- **At a join, Locus builds a new version by cases** on the condition, carrying each branch's proof along the join's equation. It proves the proposition at the joined names when every branch's version did at its own, and the `if` of the branches' propositions otherwise. The joined name is a variable, so this is a proof that elaboration constructs, not a type that it relabels.

### A borrow's final value

`contracts.final` kept a body from writing `final!`. That was a rule about spelling, and inference went around it:

```rust
struct Tally {
    hits: u32,
    count: Nat,
}

fn lend(t: &mut Tally) -> (r: &mut Tally, @(final!(r).count === final!(r).count)) {
    (t, _)
}

logic fn recover(n: Nat, same: @(n === n)) -> Nat {
    n
}

fn absurd(t: &mut Tally) {
    let (r, about_the_end) = lend(t);
    let predicted = logic! { recover(_, about_the_end) };
    r.count = logic! { predicted + 1 };
}
```

`recover(_, about_the_end)` infers its first argument from the proof's type, and so hands the body the count that `r` will end with, before the body writes it. The count would then be one more than itself.

**Until a mutable borrow ends, the value it ends with occurs only inside propositions and proofs.** An expression whose type is neither a proof type nor `Prop` is rejected when its core term mentions that value, however the value got there: written, inferred for a `_`, or returned by a logic function. The check is on core terms.

- A proof about a final value may be bound, combined with other proofs, and returned before the borrow ends. `first_mut`, which returns a `&mut` into a pair with a proof that relates the pair's final value to the reference's, needs exactly that to prove its own result.
- This is sound because a proof cannot become a value: a match on a proof produces a proof (`proofs.elimination`), so nothing computed before the borrow ends, and written through it, can depend on what it ends with.
- Once the borrow has ended, the value is the one the place holds, and nothing restricts it.

### When two propositions are the same

Two types that hold logical values are the same when they are built the same way from the same types, and their propositions *convert* to the same core term. Conversion is five reductions, and nothing else:

1. arithmetic and comparisons on literals;
2. an `if` on a literal, a `match` on a constructor, and a field of a constructed value;
3. a call of a logic function that does not call itself, a model included, becomes its body, where every name in that body could be written at the point of the call;
4. a call of a logic function that calls itself becomes its body only when the argument it recurs on is a constructor, or every argument its measure reads is a literal;
5. a constant, and a ghost `let` that is not `mut`, become their values.

| Pair | Converts? | Why |
|---|---|---|
| `double(3)` and `6` | Yes | Reduction 4, on a literal |
| `double(n - 1)` and its body | No | `n` is not a literal; `eq::unfold(double)(n - 1)` proves the equation |
| `sorted(xs)` and its `forall` | Yes | Reduction 3 |
| A model and its body, outside the module of a private field it reads | No | The body names what cannot be written there, so the model is known by the type's contracts |
| `LIMIT > 0` and `true`, for `const LIMIT: u32 = 8` | Yes | Reductions 5 and 1 |
| `a + b` and `b + a` | No | A law of arithmetic is a proof, by `linear` |
| `0 < n` and `n > 0` | No | The same, so `logical-layer/proof-holder-other-proposition.reject.lc` stays rejected |
| `n₁ < 10` and `n₂ < 10` | No | A value name never unfolds |

- **This is what Rust does with types.** `[u8; 2 + 2]` and `[u8; 4]` are one type, and a type alias is transparent. Rust evaluates what it can and proves nothing, and so does conversion.
- **Reduction is lazy and bounded.** It is tried only as far as a comparison needs, within a fixed number of steps that the rule will state. A comparison that does not finish is an error, and not a finding that the two differ. The bound is about getting the same answer on every machine. That conversion proves nothing false rests on logic functions being total and on the kernel.
- **`compute()` takes its term from the goal.** For `a === b` it is `eq::refl(a)`, accepted when `a` and `b` convert. For a `Bool` used as a proposition it is `eq::refl(true)`. So `compute` is still a procedure, and what it finds is a term, which RFC 0003 had promised and could not give.

### Sequences

- `s.len()`; `s.get(i)`, a `Maybe` that is empty outside the sequence; `s.at(i, p)`, for a proof `p` that `i` is inside it; and `s.update(i, v)`, which a write to an element needs. The index is an `Int`.
- **`s[i]` with no proof exists only where the element type has a default,** a term that Locus derives from the type's structure: `Int`, `Nat`, `Bool`, a sequence, a `Maybe`, a tuple or a logical struct without proof fields whose parts all have one, and the snapshot of a machine integer, a `bool`, or a struct, tuple, array, or slice built from those. A proof type, a type parameter, and an opaque snapshot have none.
- Outside the sequence, `s[i]` is that default, and the default never converts to anything, so nothing is known of it.
- An unrestricted `s[i]` would be unsound: the element at 0 of an empty `Seq<@False>` would be a proof of `False`. RFC 0002 had warned of this and left the operation as a placeholder.
- A `Nat` is made from an `Int` with a proof that it is not negative.

### Where a snapshot is cut

RFC 0002 cuts a field that closes a cycle of references, so that every snapshot is finite, and said both that a type's snapshot is the same wherever it appears and that the cut field's references hold only addresses. Those two do not fit: with `struct Wrap<'a, T> { inner: &'a T }` and `struct Node<'a> { w: Wrap<'a, Node<'a>> }`, the `w` inside `Snap<Node>` is not a `Snap<Wrap<Node>>`.

- **A cut field has the type `CutSnap<F>`,** for its declared type `F`: the snapshot of `F` with every reference in it held as its address. It is defined for every `F`, has no model, and is not `Snap<F>`, so it is not accepted where a `Snap<F>` is.
- **A model or a logic function reads only snapshots,** so it reads nothing behind such an address. Otherwise a model would not be a function of the snapshot: two values with equal snapshots can have different referents there.
- **A function body may read the place behind a cut reference,** as it reads any place. The read gives the value name of that place's current version, and two reads with no write between give the same name.

### What a proof may cite

| Kind of fact | Written as |
|---|---|
| A definition | Nothing: conversion |
| A fact in the context | `given(P)`, accepted only when `P` converts to a fact that is in the context at that point |
| A fact from a type | A lemma of Locus's library: `u8::range(x)`, `Nat::nonneg(n)` |

- **`given(P)` searches for nothing.** It is the written form of what a hole takes from the context, and is what lets a hole expand to a term (`proofs.procedures.expand`).
- **A hole tries three things, in order:** a proof in scope or a fact in the context of the proposition required; `compute()`; and `linear` over all of them.
- **`linear` has a fixed theory:**
  - equality, with equal arguments giving equal results;
  - linear arithmetic on integers;
  - the connectives, and an `if` between two terms;
  - that the constructors of a data type are injective and distinct, and that two structs, tuples, or snapshots are equal exactly when their fields are. An opaque part of a snapshot counts as a field;
  - that a `Nat` is not negative, that a machine integer is in its range, and the two facts about `wrap`.
- **It never applies constructor reasoning to a proposition.** Any two proofs of one proposition are equal, so if `Exists::Witness` were injective, a proof with witness 0 and one with witness 1 would give `0 === 1`.
- It compares terms up to conversion, after a name that a fact says equals a literal is replaced by it. It instantiates no `forall`, multiplies no two variables, and knows nothing about sequences.

### Constants and statics

```rust
struct Certified {
    proof: @False,
}

static BAD: Certified = Certified { proof: logic! { BAD.proof } };
```

The logical data in the initializer of a constant or a static does not depend on the item itself: not directly, not through other constants and statics, not through logic functions, and not through a field of the item. While an initializer is checked, the item's own proof fields are not available. Statics whose addresses form a cycle at run time are still allowed.

## Rules

A new chapter, [Core terms](../../spec/core.md), and changes to five others.

| Rule | Examples |
|---|---|
| `core.elaboration` | `core/congruence-at-the-snapshot.accept.lc`, `core/congruence-through-a-model.reject.lc`, `core/rewrite-misses-a-field-read.reject.lc` |
| `core.values` | `core/value-known-by-a-fact.accept.lc`, `core/value-name-per-version.reject.lc` |
| `core.wrap` | `core/cast-in-range.accept.lc`, `core/wrap-written-out.accept.lc`, `core/cast-wraps.reject.lc`, `core/difference-may-wrap.reject.lc` |
| `core.conversion` | `core/constant-unfolds.accept.lc`, `core/conversion-by-evaluation.accept.lc`, `core/predicate-used-as-its-body.accept.lc`, `core/unproved-index-of-integers.accept.lc`, `core/conversion-is-not-arithmetic.reject.lc`, `core/private-body-stays-folded.reject.lc`, `core/recursive-call-stays-folded.reject.lc` |
| `core.sequences` | `core/index-with-a-proof.accept.lc`, `core/index-without-bounds.reject.lc` |
| `core.sequences.default` | `core/predicate-used-as-its-body.accept.lc`, `core/unproved-index-of-integers.accept.lc`, `core/index-outside-is-unknown.reject.lc`, `core/unproved-index-of-proofs.reject.lc` |
| `models.snapshots` | `models/generic-snapshot-at-cycle.accept.lc`, `models/read-through-cut-reference.accept.lc`, `models/snapshot-of-vector-without-model.accept.lc`, `models/snapshot-through-generic-view.accept.lc`, `models/snapshot-through-reference.accept.lc`, `models/snapshot-unchanged.accept.lc`, `models/snapshots-equal-fields.accept.lc`, `models/model-through-a-cut.reject.lc`, `models/snapshot-through-generic-cycle.reject.lc`, `models/snapshot-through-reference-cycle.reject.lc` |
| `models.snapshots.cut` | `models/body-reads-behind-a-cut.accept.lc`, `models/model-through-a-cut.reject.lc` |
| `logic.ghost.type-identity` | `core/conversion-by-evaluation.accept.lc`, `logical-layer/proof-holder-same-proposition.accept.lc`, `core/conversion-is-not-arithmetic.reject.lc`, `core/value-name-per-version.reject.lc`, `logical-layer/proof-holder-other-proposition.reject.lc`, `runtime/proof-holder-keeps-its-proposition.reject.lc` |
| `logic.const.well-founded` | `logical-layer/logical-constant-chain.accept.lc`, `logical-layer/logical-constant-cycle.reject.lc`, `logical-layer/static-justifies-itself.reject.lc`, `logical-layer/statics-justify-each-other.reject.lc` |
| `contracts.final` | `contracts/final-value-at-return.accept.lc`, `contracts/final-value-in-a-proof.accept.lc`, `contracts/local-borrow-final.accept.lc`, `contracts/returned-borrow.accept.lc`, `runtime/write-through-a-reborrow.accept.lc`, `contracts/final-escapes-through-parameter.reject.lc`, `contracts/final-in-ghost-write.reject.lc`, `contracts/final-reads-no-place.reject.lc`, `contracts/final-through-captured-lifetime.reject.lc`, `contracts/wrong-final.reject.lc` |
| `contracts.final.pending` | `contracts/final-value-in-a-proof.accept.lc`, `contracts/final-value-recovered-by-inference.reject.lc` |
| `proofs.holes` | `proofs/forall-used-by-call.accept.lc`, `proofs/hole-from-branch-and-arithmetic.accept.lc`, `proofs/forall-not-used-by-hole.reject.lc`, `proofs/hole-needs-a-lemma.reject.lc` |
| `proofs.procedures.compute` | `core/constant-unfolds.accept.lc`, `proofs/compute-closed-term.accept.lc`, `proofs/compute-open-term.reject.lc` |
| `proofs.procedures.linear` | `proofs/data-constructors-are-distinct.accept.lc`, `proofs/equality-chain-by-procedure.accept.lc`, `proofs/hole-from-branch-and-arithmetic.accept.lc`, `proofs/linear-with-division.accept.lc`, `proofs/procedure-given-its-facts.accept.lc`, `proofs/hole-needs-a-lemma.reject.lc`, `proofs/linear-product-of-variables.reject.lc`, `proofs/proof-constructors-are-not-distinct.reject.lc` |
| `proofs.equality` | `core/congruence-at-the-snapshot.accept.lc`, `proofs/equality-chain-as-term.accept.lc`, `proofs/rewrite-with-equality.accept.lc`, `core/congruence-through-a-model.reject.lc`, `core/rewrite-misses-a-field-read.reject.lc`, `proofs/equality-chain-mismatched.reject.lc` |
| `proofs.values.inferred` | `proofs/inferred-values.accept.lc`, `contracts/final-value-recovered-by-inference.reject.lc`, `proofs/inferred-value-from-a-procedure.reject.lc`, `proofs/inferred-value-undetermined.reject.lc` |
| `proofs.facts.named` | `proofs/named-condition.accept.lc`, `runtime/named-sum-as-checked.accept.lc`, `proofs/named-condition-after-branch.reject.lc`, `runtime/named-sum-may-wrap.reject.lc` |
| `proofs.facts.types` | `proofs/range-of-a-machine-integer.accept.lc` |
| `proofs.given` | `proofs/given-a-branch-condition.accept.lc`, `proofs/given-what-an-index-returned.accept.lc`, `proofs/given-a-fact-that-is-not-there.reject.lc` |
| `runtime.facts` | `core/value-known-by-a-fact.accept.lc`, `proofs/given-a-fact-that-is-not-there.reject.lc`, `runtime/fact-from-one-branch.reject.lc` |
| `runtime.conditions` | `proofs/given-a-branch-condition.accept.lc`, `runtime/named-call-result.accept.lc`, `runtime/named-sum-as-checked.accept.lc`, `runtime/named-sum-may-wrap.reject.lc` |
| `runtime.arith.wrapping` | `contracts/cursor-replaced-whole.accept.lc`, `core/cast-in-range.accept.lc`, `core/wrap-written-out.accept.lc`, `runtime/binary-search-found.accept.lc`, `runtime/midpoint-in-range.accept.lc`, `runtime/sum-in-range.accept.lc`, `core/cast-wraps.reject.lc`, `core/difference-may-wrap.reject.lc`, `runtime/sum-may-overflow.reject.lc` |
| `runtime.writes` | `runtime/element-write-keeps-length.accept.lc`, `runtime/field-write-updates-the-value.accept.lc` |
| `runtime.borrows` | `runtime/write-through-a-reborrow.accept.lc`, `runtime/reborrow-ends-with-its-last-write.reject.lc` |
| `runtime.proofs.assigned` | `runtime/kept-proof-assigned-again.accept.lc`, `runtime/kept-proof-on-every-path.accept.lc`, `runtime/loop-invariant-by-induction.accept.lc`, `runtime/kept-proof-borrowed.reject.lc`, `runtime/kept-proof-left-behind.reject.lc`, `runtime/proof-holder-keeps-its-proposition.reject.lc` |
| `runtime.joins` | `runtime/assert-gives-its-condition.accept.lc`, `runtime/clamp-after-a-branch.accept.lc`, `runtime/kept-proof-on-every-path.accept.lc`, `runtime/loop-keeps-a-fact.accept.lc`, `runtime/debug-assert-gives-nothing.reject.lc`, `runtime/fact-from-one-branch.reject.lc`, `runtime/kept-proof-on-one-path.reject.lc` |
| `runtime.loops` | `runtime/binary-search-found.accept.lc`, `runtime/element-write-keeps-length.accept.lc`, `runtime/loop-invariant-by-induction.accept.lc`, `runtime/loop-keeps-a-fact.accept.lc`, `runtime/loop-invariant-missed-at-continue.reject.lc`, `runtime/loop-loses-a-fact.reject.lc` |
| `runtime.loops.for-range` | `runtime/for-over-a-range.accept.lc`, `runtime/for-over-an-iterator.reject.lc`, `runtime/range-bound-written-while-built.reject.lc`, `runtime/range-bounds-built-once.reject.lc` |

## Interactions

| Area | Effect |
|---|---|
| Types | Two types that hold logical values are the same up to conversion of their propositions. `CutSnap<F>` is a new snapshot type. A kept proof variable's declared type is a template |
| Propositions and proofs | Every rule about equality, rewriting, inferred values, and conversion is stated on core terms. `given` and the type lemmas are new term forms. `linear`'s theory is a closed list |
| Ownership and mutation | A mutable borrow suspends the borrowed place's name, and its end is tied to the target's last write. A write to a part gives the whole a new name. Joins and loop tops say what each place is afterwards |
| Erasure and runtime behavior | None. Nothing here runs: `u32::wrap` is a logic function, and `given` is a proof |
| Rust interoperability | None. No Rust program is rejected that was accepted, and plain Rust still owes no proof |
| Diagnostics | A value name is shown as its place and where it got that value. An error from `linear` can say which fact about `wrap` was missing. Overrunning the step bound has its own error |
| Trust model | None added. The context's rules are part of `trusted.obligations`, whose entry now says so, with tests of each rule against rustc in a debug and a release build |

## Alternatives

**Conversion only where a proof writes it.** Type identity stays "the same core term", and a proof says `conv` each time it crosses a definition. Then a proof of `sorted(xs)` cannot be called as its `forall` without a step, and every model crossing is written. The owner chose conversion in type identity.

**`compute()` as a primitive form of proof.** The first draft of this proposal had it. A second review pointed out that conversion, how a proof says "by conversion", and search are three things. With conversion in type identity, `eq::refl` is the form, and `compute` stays a procedure.

**A condition with a call cannot be named.** Stricter than needed: the proof says only that this one result was `true`.

**A pending proof that can only be bound, moved, and returned.** This was the wording the owner and the second review saw. A function such as `first_mut` could not then prove its own result, which relates two final values, and `contracts/returned-borrow.accept.lc` would be rejected. What must not happen is a *value* computed from a final value, and the rule now says that.

**Every stale proof read is an error.** A proof about earlier values is still a proof, and RFC 0004 keeps it usable. Only a proof variable that a loop assigns and does not keep on every path has no version, because there is no one proposition it would prove.

**A total `s[i]` at every type, with an unspecified value outside.** Unsound for an element type with no values.

**Making the whole cut field opaque.** Simpler, and it loses the field's parts that are not references.

**Widening conversion with laws of arithmetic or with facts about value names.** Then whether two types are the same would depend on a proof search, and RFC 0002's rule that a proof about one version never fits another by computation alone would go.

## Prior art

- **Lean 4** identifies types up to definitional equality, with proof irrelevance, and keeps `Eq` apart from `BEq`. Locus's conversion is a small part of Lean's: it never unfolds a recursive function on a variable, which is where Lean's unfolding needs heuristics.
- **Rust** evaluates constant expressions in types and treats aliases as transparent, and proves no equations between generic constant expressions.
- **RustHornBelt and Creusot** make a mutable borrow's final value a prophecy, and keep it out of ghost code and snapshots for the reason given above.
- **Dafny and Verus** treat indexing a sequence outside its length as an unspecified value. Both can, because every type in their logics has a value. Locus's proof types do not.
- **Compilers in SSA form** give each assignment a new name and join names at control-flow merges, which is what value names and joins are.

## Unresolved questions

**Decided by the owner on 2026-11-21,** in two rounds, after the reviews of PR #25.

1. **Logical code is elaborated to core terms, and conversion is part of when two types are the same.** Shown the core, five reductions, `u32::wrap`, and a choice between conversion in type identity and conversion only where written: "I'm okay with your recommendation and the rest of the idea looks reasonable." The recommendation was conversion in type identity, the five reductions, a constant and a ghost `let` as transparent while a runtime value stays a variable, and `<` and `>` kept separate.
2. **Joins, kept proof variables, cut snapshots, indexing, `given`, and `linear`'s theory,** as this RFC has them, and four rules put for confirmation: a borrow's final value, `for` ranges, initializers, and writes. "I'm okay with this", with a request for a summary to put to a second reviewer before anything was written.
3. **Written as a new RFC,** after that review and a revised summary: "Go for it please."

**Still for the owner.** Each is written into the rules and examples as proposed.

1. **A final value is kept out of values, not out of proofs.** The summary said that a proof about one could only be bound, moved, and returned. See [Alternatives](#alternatives) for why that was wrong.
2. **What the second review changed,** which the owner saw in the revised summary: constructor reasoning for data types only; a kept proof at a join built by cases; the induction rule for loops, and that a proof variable which is not an invariant has no version at the top; a borrow's end tied to the last write; `CutSnap` cutting every reference; a derived default for unproved indexing; and `compute` taking its term from the goal.
3. **The list of types that have a default.**
4. **The names** `given`, `u8::range`, `Nat::nonneg`, and `CutSnap`.
5. **That overrunning the step bound is an error,** and the bound's value, which is not chosen.
6. **That a `let mut` of type `Option<@P>` is not a kept proof variable.**

**Left to later work:**

- Which operations are understood, and on which types. The reviews found that comparisons must be limited to integers and `bool`; RFC 0004 is not yet revised for that.
- What a verified function's summary of the places it may write records ([runtime-semantics](../../issues/runtime-semantics.md)).
- How the measure obligation of a recursive call is written out.
- Shifts and bitwise operations, and whether `debug_assert!` gives a fact in a debug build.
- Whether the contracts of Locus's library are stated on snapshots, and the lemma that ties a library container's model to its snapshot's length ([standard-library](../../issues/standard-library.md)).
- A small checked slice of this core, which the second review recommends: arithmetic, a join, one loop, and one mutable reborrow.
- The remaining findings of the reviews, which this RFC does not touch.
