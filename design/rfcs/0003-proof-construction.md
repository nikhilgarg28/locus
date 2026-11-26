# RFC 0003: Proof construction

- **Status:** draft
- **Issue:** [proof-construction](../../issues/proof-construction.md), and parts of [proof-search-and-storage](../../issues/proof-search-and-storage.md) and [naming-facts-from-control-flow](../../issues/naming-facts-from-control-flow.md)
- **Supersedes:** none

## Summary

A proof is a term: an expression in Rust's own syntax, built from calls, closures, constructors, `match`, `let`, and recursion. A small, closed list of *procedures*, `compute` and `linear`, may be called where a proof is expected, and each has a stated rule for what it proves and what it uses. A hole, `_`, tries three named things and nothing else. Whatever a procedure or a hole finds is a term that calls no procedure, and a command puts that term in the source when the author wants it kept. There is no tactic language.

**[RFC 0005](0005-core-terms.md) restates this RFC's rules on core terms,** after two reviews found gaps in them. Where the two differ, RFC 0005 is the later text. It changes five things here: what a named condition proves in runtime code; that the equality functions work on core terms, so that an equality of models rewrites no field; what "evaluation" is, which is now conversion, and is part of when two propositions are the same; the steps of a hole and the theory of `linear`; and how a value is inferred for `_`. It adds `given(P)`, the written form of a fact that a hole takes from the context. The sections below are not yet rewritten for it.

## Motivation

[RFC 0002](0002-logical-layer.md) defines what a proof is and leaves two things open: how a proof uses a definition or an equality, and what `_` may do. Without the first, a proof about a logic function cannot be written. Without the second, whether a program is accepted depends on a search that nothing describes.

```rust
#[decreases(n)]
logic fn double(n: Int) -> Int {
    if n <= 0 { 0 } else { double(n - 1) + 2 }
}

// What may stand in place of the `_`s, and what does each one do?
#[decreases(n)]
logic fn double_is_sum(n: Int, nonneg: @(n >= 0)) -> @(double(n) == n + n) {
    if n == 0 {
        _
    } else {
        let ih = double_is_sum(n - 1, _);
        _
    }
}
```

The owner set three requirements: that writing a proof should "feel like a library of regular functions, not an embedded DSL"; that the list of procedures be "relatively tightly scoped", with "clear rules on what each does + how to compose them with terms"; and that a call of one can always be replaced by what it found.

## Design

There are three layers, and only the last one searches.

### Terms

RFC 0002 has every form but the last two.

| Form | What it proves |
|---|---|
| A local or a parameter | What its type says |
| A call, `lemma(x, h)`, `all(v)`, `imp(h)` | A lemma's conclusion; a `forall` at a value; the conclusion of an implication |
| A closure, `\|x: T\| proof`, `\|h: @P\| proof` | A `forall`, or an implication |
| A constructor: `And::Intro(p, q)`, `Or::Left @ p`, `Exists::Witness(v) @ p`, `Name::Arm(w) @ p` | A conjunction, a disjunction, an `exists`, a declared proposition |
| `match` on a proof | By cases; it opens a witness |
| `let h = ...;` | Names a step |
| Recursion, with `#[decreases(e)]` | By induction |
| `if h: c { ... } else { ... }` | By cases on a condition, with the fact named |
| `_` where a logical value is expected | The value that the proofs beside it determine |

- **A lemma is a logic function, and a `forall` is one too.** `logic fn lt_trans(a: Int, b: Int, c: Int, ab: @(a < b), bc: @(b < c)) -> @(a < c)` states the same theorem as a proof of `forall |a: Int, b: Int, c: Int| a < b && b < c ==> a < c`, and is shorter to call. A lemma that is called takes its values as parameters. A statement that is itself quantified, such as the definition of `Sorted`, is written with `forall` and `exists`.
- **`_` for a value.** In `lt_trans(_, _, _, xy, yz)` the types of `xy` and `yz` determine the three values, so they are not written again. Where nothing determines a value, it is an error.
- **A named condition.** In `if low: n < floor { A } else { B }`, `low` proves `n < floor` in `A`, and `!(n < floor)` in `B`. It is in scope in the two branches and nowhere else. An unnamed condition still gives its fact to `_`, as before.

### Library functions that search for nothing

Each is a rule or a lemma, and is called like any function. The checker gives the methods on proofs and `rewrite` their types.

- **Equality.** For a proof `e` of `a === b`:

  | Call | Proves |
  |---|---|
  | `eq::refl(a)` | `a === a` |
  | `e.symm()` | `b === a` |
  | `e.then(f)`, for `f` proving `b === c` | `a === c` |
  | `eq::congr(g, e)`, for a logic function `g` | `g(a) === g(b)` |
  | `h.rewrite(e)`, for any proof `h` | What `h` proves, with every `a` replaced by `b` |

  `===` is the only equality these take (RFC 0002, decision 70).
- **Definitions.** `eq::unfold(f)` proves the defining equation of a logic function `f`: `eq::unfold(double)(n)` proves `double(n) === if n <= 0 { 0 } else { double(n - 1) + 2 }`. It unfolds one layer. A declared proposition has no defining equation, and is used by matching.
- **Everything else is a lemma** of Locus's library: the laws of arithmetic and order, what each operation on a sequence does, and the like. Their names and their list are [standard-library](../../issues/standard-library.md)'s.

### Procedures

A procedure is a function of Locus's library whose result is a proof of whatever proposition is expected where the call stands, in the way that the result of `collect()` is whatever collection is expected. It is called like any function. Its body is the checker's, and so is the type of `linear`.

| Procedure | Proves | Uses | Fails when |
|---|---|---|---|
| `compute()` | An equality whose two sides evaluate to the same value; a proposition that evaluates to `true` | The definitions of logic functions, and nothing else | The sides do not reduce to the same form |
| `linear(facts)` | What follows from `facts` by equality and linear integer arithmetic | The proofs in the tuple `facts`, and nothing else | The goal does not follow from those facts in that theory. It reports values for which the goal fails |

- **What `linear` understands.** After reducing the goal and the facts by evaluation, as `compute` does: `===` and the comparisons; `+`, `-`, and multiplication, division, and remainder by a literal, on `Int` and `Nat`; `&&`, `||`, `!`, and `==>`; an `if` between two terms it understands; and that equal arguments give equal results. Every other subterm is an atom, about which it knows only what the facts say. `x * y` is an atom. Facts that contradict each other prove anything.
- **The list is closed.** A program cannot add a procedure. It writes a lemma.

### What `_` does

A proof hole tries three things, in this order, and nothing else:

1. a proof that is known at that point of the proposition required, up to evaluation;
2. `compute()`;
3. `linear`, given everything known at that point.

What is known at a point is the proofs in scope, and the facts that the runtime semantics gives there: the condition of an enclosing branch, the value that a place was given, the range of a machine integer ([RFC 0004](0004-runtime-code.md), and RFC 0002's [Logical code](0002-logical-layer.md#logical-code)). `prove!(P)` is the same hole with its proposition written out. A hole that none of the three fills is an error, and the program writes a term.

### Composing procedures with terms

1. **A procedure call is an expression whose type is a proof.** It stands wherever a proof is expected: an initializer, an argument, a field, a result.
2. **Its goal is the proposition expected there.** `let h = linear((a,));` expects none, and is an error. A lemma needs no expected proposition, since its result says what it proves.
3. **A procedure uses its arguments, and never the scope.** `_` is the one form that reads the scope.
4. **Facts are a tuple of proofs,** and any proof may be one: `linear((step, ih))`, `linear((lemma(x),))`, `linear(())`.
5. **A procedure instantiates no `forall` and opens no `exists`.** The term does that first: `linear((sorted(i, j, bounds),))`.
6. **Cases and induction belong to terms.**
7. **The same call gives the same result on every machine,** within a bound that is part of the procedure's definition.

The proof of the motivating example, with nothing left to a hole:

```rust
#[decreases(n)]
logic fn double_is_sum(n: Int, nonneg: @(n >= 0)) -> @(double(n) == n + n) {
    let def = eq::unfold(double)(n);
    if zero: n == 0 {
        linear((def, zero))
    } else {
        let ih = double_is_sum(n - 1, linear((nonneg, zero)));
        linear((def, ih, nonneg, zero))
    }
}
```

`def` says what `double(n)` is, as an `if` on `n <= 0`. `linear` reduces that `if` once the facts settle its condition, and treats `double(n)` and `double(n - 1)` as atoms.

### Expanding a call

Whatever a procedure or a hole finds is a term built from the forms and functions above, with no procedure call and no hole in it, and the kernel checks that term. A command replaces a call in the source by its term. So a program can be written at any level between `_` and a term that names every rule, and moved from one to the other without changing what it means. A check does not search again for a call whose stored proof still matches; the store is [proof-search-and-storage](../../issues/proof-search-and-storage.md). [RFC 0005](0005-core-terms.md#what-a-proof-may-cite) qualifies this in two ways. The expansion may add a ghost `let` that names an earlier value. And a proof that would have to write out a value that a borrow has yet to end with has no form in source, though it has, like every accepted proof, a complete core term that the kernel checks.

### What is left out

- **A tactic language.** No block of commands that change a goal.
- **Procedures that a program defines.**
- **Solvers, and default sets of rewriting rules.**
- **Facts that a procedure takes without being given them.**
- **Polynomial identities,** `ring`, which the owner left out for now; a multiplication law is a lemma.
- **The macros `calc!`, `rewrite!`, `fold!`, and `unfold!`,** which earlier drafts had sketched. The functions above take their place.

### Functional extensionality

RFC 0002 decision 127 adopts extensional sameness for pure logical functions.
The explicit API fits the existing equality module:

```rust
pub logic fn funext<A: Logical, B: Logical>(
    f: logic fn(A) -> B,
    g: logic fn(A) -> B,
    pointwise: @(forall |x: A| f(x) === g(x)),
) -> @(f === g);
// Called as eq::funext(f, g, pointwise).
```

This is a library theorem signature, not an unchecked declaration a program may
add. Its type parameters have the stated `Logical` bounds; whether `!` meets
those bounds remains the I06 lowering/kind gate. A caller supplies the proof at
every input. Equality at one input, a finite
sample, or a restricted domain does not suffice; nor does an overridable relation
on the outputs. The result is the existing `===`: it can be passed to `eq::congr`
and used to rewrite, including under a higher-order logical observer. Ordinary
congruence gives the converse, equal functions have equal outputs.

The theorem does not make function equality definitionally decidable. A bare
`compute()` does not use a pointwise proof, and neither `linear` nor `_` calls
`funext` automatically. Logical closures obey this rule as logical functions;
runtime closures and function pointers gain no new equality or effect law.

The trust baseline already names Lean 4's type theory with propositional
extensionality, quotients and classical choice (`assumed.lean`). Lean derives
functional extensionality in that foundation. Locus must supply and kernel-check
the corresponding theorem or a faithful checked realization under that baseline,
with its dependencies recorded. This is not an independent `funext` axiom or an
`unsafe assume!` adapter. The backend and proof import/encoding remain explicit
work in [function-extensionality-kernel](../../issues/function-extensionality-kernel.md);
no kernel execution is claimed here. Choosing a weaker foundation would require
revisiting its ability to realize this adopted theorem, not silently trusting it.
Finite `Map` representation and enumeration remain separate, unresolved choices.

### Empty cases in logical data definitions

RFC 0002 decision 126 admits the library function
`pub logic fn unreachable(p: @False) -> ! { match p {} }`.
An existing `prove!(False)` or `_` may supply its argument only when the ordinary
proof-hole rules produce a kernel-checked contradiction. There is no new search
procedure or assumption. Logical never then eliminates into the expected logical
result, so a total data function can type an impossible branch. Extracting a
witness from an existential proof remains rejected, and a runtime panic or loop
cannot replace the logical elimination. The function and Rust's `unreachable!`
macro have separate namespaces and different meanings.

## Rules

The rules below go into [Propositions and proofs](../../spec/proofs.md).

| Rule | Examples |
|---|---|
| `proofs.holes` | `proofs/hole-from-branch-and-arithmetic.accept.lc`, `proofs/forall-not-used-by-hole.reject.lc`, `proofs/hole-needs-a-lemma.reject.lc` |
| `proofs.procedures` | `proofs/named-condition.accept.lc`, `proofs/procedure-given-its-facts.accept.lc`, `proofs/forall-not-used-by-hole.reject.lc`, `proofs/procedure-not-given-a-fact-in-scope.reject.lc`, `proofs/procedure-without-a-goal.reject.lc` |
| `proofs.procedures.compute` | `core/constant-unfolds.accept.lc`, `proofs/compute-closed-term.accept.lc`, `proofs/compute-open-term.reject.lc` |
| `proofs.procedures.linear` | `proofs/data-constructors-are-distinct.accept.lc`, `proofs/equality-chain-by-procedure.accept.lc`, `proofs/hole-from-branch-and-arithmetic.accept.lc`, `proofs/linear-with-division.accept.lc`, `proofs/procedure-given-its-facts.accept.lc`, `proofs/hole-needs-a-lemma.reject.lc`, `proofs/linear-product-of-variables.reject.lc`, `proofs/proof-constructors-are-not-distinct.reject.lc` |
| `proofs.procedures.expand` | `contracts/final-value-by-the-goal.accept.lc`, `proofs/equality-chain-as-term.accept.lc`, `proofs/equality-chain-by-procedure.accept.lc`, `proofs/hole-written-out-with-an-earlier-value.accept.lc` |
| `proofs.equality` | `core/congruence-at-the-snapshot.accept.lc`, `proofs/equality-chain-as-term.accept.lc`, `proofs/rewrite-with-equality.accept.lc`, `core/congruence-through-a-model.reject.lc`, `core/rewrite-misses-a-field-read.reject.lc`, `proofs/equality-chain-mismatched.reject.lc`, `proofs/reflexivity-of-two-values.reject.lc` |
| `proofs.equality.funext` | `proofs/funext.accept.lc`, `proofs/funext-through-observer.accept.lc`, `proofs/funext-logical-closures.accept.lc`, `proofs/funext-at-one-input.reject.lc`, `proofs/funext-output-relation.reject.lc`, `proofs/funext-not-a-hole-rule.reject.lc`, `proofs/funext-not-conversion.reject.lc`, `proofs/funext-runtime-function.reject.lc` |
| `proofs.unfold` | `proofs/unfold-logic-function.accept.lc`, `proofs/unfold-declared-proposition.reject.lc`, `proofs/unfold-private-body.reject.lc` |
| `proofs.values.inferred` | `contracts/final-value-by-the-goal.accept.lc`, `contracts/lemma-at-a-final-value.accept.lc`, `proofs/inferred-value-from-the-goal.accept.lc`, `proofs/inferred-values.accept.lc`, `contracts/final-value-recovered-by-inference.reject.lc`, `proofs/inferred-value-determined-twice.reject.lc`, `proofs/inferred-value-from-a-procedure.reject.lc`, `proofs/inferred-value-undetermined.reject.lc`, `proofs/reflexivity-of-two-values.reject.lc` |
| `proofs.facts.named` | `proofs/named-condition.accept.lc`, `runtime/named-sum-as-checked.accept.lc`, `proofs/named-condition-after-branch.reject.lc`, `runtime/named-sum-may-wrap.reject.lc` |
| `proofs.forall` | `proofs/forall-proved-by-a-closure.accept.lc`, `proofs/forall-used-by-call.accept.lc`, `proofs/implication-used-without-its-premise.reject.lc` |

## Interactions

| Area | Effect |
|---|---|
| Types | A procedure's result type is the proof type expected where it is called. `_` in a value position stands for a logical value. |
| Propositions and proofs | Adds the rules for equality and definitions that RFC 0002 left to this RFC, and fixes what a hole tries. Every accepted example of RFC 0002 with a hole stays accepted: each is closed by a known proof, by evaluation, or by equality and linear arithmetic. |
| Ownership and mutation | None. Proofs are ghost, and none of these forms reads a place that RFC 0002's forms do not. |
| Erasure and runtime behavior | None. Every form here is logical code and is erased. `if h: c` runs as `if c`. |
| Rust interoperability | `if h: c` is new syntax, which rustc rejects. `compute`, `linear`, and `eq` are names of Locus's library, which a program's own definitions shadow, as RFC 0001 says of every Locus name. |
| Diagnostics | A failed hole or procedure reports the proposition required and the proofs it was given or found in scope; `linear` adds values for which the goal fails. The forms for asking for that report are [proof-debugging](../../issues/proof-debugging.md). |
| Trust model | No new axiom or trust category. A procedure's result is a term that the kernel checks, so no procedure is trusted (`trusted.kernel`). `eq::funext` must be realized as a checked theorem under the already declared Lean foundation (`assumed.lean`); the kernel backend and proof encoding remain open. |

## Alternatives

**A tactic language,** as in Lean and Coq: commands that transform a goal, with the proof state held outside the source. It is concise, and it computes the statements of intermediate steps. Rejected for now. A script cannot be read without replaying it, breaks in the middle when a statement changes, and is a second language beside Rust. Most proofs about programs are shallow, and one call of `linear` closes them. A `by { ... }` block that expands to a term can be added later without changing anything written here.

**Terms alone, with procedures only as a command that inserts their result.** Fully explicit. Rejected by the owner for calls that may stand in the source: a chain of arithmetic lemmas in the middle of a function hides the program. Expansion keeps the explicit term one command away.

**Procedures that read every proof in scope.** Shorter: `linear()`. Rejected. A call's result would depend on what happens to be in scope, an unrelated fact could change it, and a stored proof would name things the source does not. `_` is the short form.

**`ring`, for polynomial identities.** Left out by the owner for now, to keep the list small.

**Values that the caller does not write, as generic parameters:** `logic fn lt_trans<a: Int, b: Int, c: Int>(...)`. Rejected for ordinary parameters with `_`. It was a new feature for what `forall` and parameters already say. It can be added later without changing what any lemma means.

**Macros for proof steps:** `calc!`, `rewrite!`, `unfold!`. Rejected for methods and functions, at the owner's direction that the library be regular functions.

**Proofs in Lean files beside the program.** Rejected: the vision keeps proofs next to the code they are about.

## Prior art

- **LCF** (Milner, 1970s) introduced tactics, and the idea that a theorem can only be made by a small set of trusted rules. Locus keeps the second.
- **Lean** has both terms and tactics, and they nest in each other. Its library's style has moved toward structured proofs with automation at the leaves, and its `simp?` and `exact?` print an explicit replacement for a search. Expansion is that, as a rule of the language.
- **Isabelle's Isar** (Wenzel, 1999), after **Mizar**, replaced tactic scripts with proofs that state their intermediate steps.
- **Agda and Idris** have no tactic language in ordinary use. A proof is a term with holes, and the tool reports what each hole needs. That is the way of working this RFC assumes.
- **Lean's `omega`** decides a fragment close to `linear`'s arithmetic, and returns a term.

## Unresolved questions

**Decided by the owner,** in the session of 2026-11-17:

1. **Proofs are terms, a closed list of procedures may be called where a proof is expected, and every call expands.** "I can do option 3 but I want to keep a relatively tightly scoped list of procedures and have clear rules on what each does + how to compose them with terms."
2. **The proof library is regular functions.** "I want it to feel like a library of regular functions, not an embedded DSL."
3. **The procedures are `compute` and `linear`.** Of the proposal, which also had `ring`: "We can drop ring for now - rest is all good."
4. **A procedure uses only the facts it is given, and `_` tries three named things.** Proposed, and accepted with the rest.
5. **A lemma takes its values as parameters, a quantified statement uses `forall` and `exists`, and `_` stands for a value that the checker can infer.** The owner asked of a lemma written with generic parameters: "can we use forall and exists in stdlib for writing this".
6. **A branch's fact is named where the branch is written.** The spelling is the owner's: "Maybe we can use if h: x < 0: { here h refers to proof} or something like that."

**Decided by the owner in I07a on 2026-11-26:** pure logical functions have extensional `===`, obtained by an explicit theorem application from pointwise `===`. RFC 0002 decision 127 records the authorization; this RFC records the `eq::funext` API and its boundary. The kernel realization is still work, not a claim that an implementation exists.

**Still for the owner.** Each is written into the rules and examples as proposed:

1. **What `else` binds:** the same name, at the negated condition.
2. **`linear` includes equality,** with equal arguments giving equal results, and reduces by evaluation first. RFC 0002's accepted examples need this of a hole: several prove that two snapshots are equal from their fields.
3. **Contradictory facts prove anything,** in `linear`.
4. **The names:** `compute`, `linear`, `eq::refl`, `eq::congr`, `eq::unfold`, and the methods `symm`, `then`, and `rewrite`.
5. **Facts are passed as a tuple.**

**Left to later work:**

- Naming the fact of a `while`, a `match` guard, and an `if let`: [naming-facts-from-control-flow](../../issues/naming-facts-from-control-flow.md).
- More procedures, each to be argued for: rewriting to a normal form with a given list of equations, facts about bits and wrapping arithmetic, and `ring`.
- Where found proofs are stored, how a stored proof finds its call after an edit, and the command that expands one: [proof-search-and-storage](../../issues/proof-search-and-storage.md).
- The forms that report a goal and a value: [proof-debugging](../../issues/proof-debugging.md).
- The lemmas of Locus's library and their names: [standard-library](../../issues/standard-library.md).
- Rewriting one occurrence and not all.
