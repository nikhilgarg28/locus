+++
id = "language-proofs"
title = "Proofs and evidence"
group = "Now"
spec_chapter = 1
order = 109
route = "specification/proofs.html"
description = "Constructing, transporting, composing, and checking evidence."
+++

# Proofs and evidence

<!-- spec: 1.0:11 informative -->
A proof is a value in an explicit evidence slot. Start with a fact already available, use a lemma to derive another, or request bounded proof construction. The kernel checks the resulting evidence against the exact required claim.

## Proofs and their forms

<!-- spec: 1.7:1 syntax -->
Write `@claim`, `@predicate(args)`, or `@(formula)` for a proof type. Leading `@` marks the type; in `Predicate::Arm @ evidence`, it separates a constructor from its body evidence. Proof values themselves are ordinary names, field projections, calls, constructors, or expressions.

<!-- spec: 1.7:2 syntax -->
| Form | Purpose |
|---|---|
| `_` | Request evidence for the expected claim. |
| `prove!(P)` | State the goal explicitly and retain the established fact. |
| `And::Intro(h, k)` | Prove both conjuncts. |
| `Or::Left(h)` / `Or::Right(k)` | Choose a disjunct. |
| `True::Intro` | Establish truth. |
| `Name::Arm(args) @ h` | Establish a named proposition. |
| `match h { ... }` | Reason by the evidence’s alternatives. |
| `rewrite!(equality, h)` | Transport evidence across an equality. |
| `fold!(definition, h)` / `unfold!(definition, h)` | Close or open a definition. |

<!-- spec: 1.90:49 example -->
~~~rust check
logic fn combine(p: Prop, q: Prop, hp: @p, hq: @q) -> @(p && q) {
    And::Intro(hp, hq)
}
fn bounded(n: u8, small: @(n < 10)) -> @(n <= 10) {
    let result: @(n <= 10) = _;
    result
}
~~~

<!-- spec: 1.7:3 syntax -->
Built-in proof forms use `!`, as in `prove!` and `rewrite!`. The compiler interprets their arguments as proof syntax; they are not ordinary functions or an extensible macro system. A bare `rewrite(...)` is rejected with a suggested correction.

## Opening and closing definitions

<!-- spec: 1.91:22 informative -->
`unfold!(f, h)` turns evidence about a call of `f` into evidence about its body. `fold!(f, h)` goes in the opposite direction. Neither operation proves the body for you. Give the intended result type when closing a definition so the checker knows which call to reconstruct.

<!-- spec: 1.90:50 example -->
~~~rust check
logic fn nonzero(n: Int) -> Prop { prop!(n != 0) }
logic fn named(n: Int, known: @(n != 0)) -> @nonzero(n) {
    fold!(nonzero, known)
}
logic fn opened(n: Int, known: @nonzero(n)) -> @(n != 0) {
    unfold!(nonzero, known)
}
~~~

<!-- spec: 1.91:23 informative -->
A path such as `Counter::small` selects a logical method. `fold!(value as ModelType, h)` and `unfold!(value as ModelType, h)` select a checked model definition. A named `prop` is opened by matching its evidence, not by unfolding one of its arms.

## Rewriting evidence

<!-- spec: 1.7:5 syntax -->
Supplied evidence must establish the expected claim after the permitted computation steps. A related claim is not automatically substituted: if evidence mentions `x` but the goal mentions `y`, use explicit transport when an equation is needed.

<!-- spec: 1.90:51 example -->
~~~rust check
fn transport(x: u8, y: u8, same: @(x == y), small: @(x <= 10))
    -> @(y <= 10)
{
    rewrite!(same, small)
}
~~~

<!-- spec: 1.91:24 informative -->
Here `same` proves `x == y`. Rewriting replaces every occurrence of the left side in `small`’s claim with the right side, yielding `y <= 10`. The direction matters; use a symmetry lemma when the available equation points the other way.

## Calling lemmas

<!-- spec: 1.7:4 syntax -->
Theory lemmas are called by name with explicit premises, such as `u32_le_trans(a, b, c, ab, bc)`. The built-in family covers integer order, equality, machine models, and selected arithmetic laws. The [kernel lemma reference](../reference/kernel.md#lemmas) lists their exact signatures; application grants only the lemma’s stated conclusion.

<!-- spec: 1.90:52 example -->
~~~rust check
fn ordered(a: u32, b: u32, c: u32, ab: @(a <= b), bc: @(b <= c))
    -> @(a <= c)
{
    u32_le_trans(a, b, c, ab, bc)
}
~~~

## Matching proofs

<!-- spec: 1.26:5 dynamic-semantics -->
A nonempty match on evidence must return evidence, not arbitrary data, including Logical data. Witnesses stay inside the proof derivation. A witness-free single-arm proof permits `let` destructuring; witness-bearing proofs require a proof-producing match. An empty match on falsity remains valid. These restrictions preserve proof irrelevance.

<!-- spec: 1.90:53 example -->
~~~rust check
prop Within(n: Int) { Bounds => { prop!(0 <= n && n <= 100) } }
logic fn upper(n: Int, bounded: @Within(n)) -> @(n <= 100) {
    let Within::Bounds @ both = bounded;
    match both { And::Intro(lower, upper) => upper }
}
~~~

## General proofs and specialization

<!-- spec: 1.91:25 informative -->
A logical proof function can establish a general claim; universal evidence specializes it to one argument. Implication evidence similarly accepts evidence of its premise. This is proof composition, with no runtime function pointer or proof inspection.

<!-- spec: 1.90:54 example -->
~~~rust check
logic fn reflexive(n: Int) -> @(n == n) { _ }
logic fn at_seven() -> @(7 == 7) {
    let all: @(forall (n: Int) { n == n }) = reflexive;
    all(7)
}
~~~

## What a hole finds

<!-- spec: 1.8:1 legality-rule -->
Holes, `prove!`, and implicit safety obligations use bounded search. Limits count work, not elapsed time. Every result is checked by the kernel; failure to find a proof does not establish that a proposition is false.

<!-- spec: 1.8:2 legality-rule -->
| Tier | Evidence source |
|---|---|
| Stored | A certificate from `Locus.lock`, checked again. |
| Exact | A matching fact, proof binding, or proof field. |
| Computed | Fixed let, projection, known-constructor, and literal computation steps. |
| Evaluation | A closed comparison checked by evaluation. |
| Arithmetic | A checked linear-arithmetic certificate, including machine ranges and supported division facts. |

<!-- spec: 1.8:3 legality-rule -->
A hole does not automatically unfold definitions, rewrite by arbitrary equations, select connective constructors, or enumerate all machine values. Use an explicit proof step for those tasks. Available conjunction facts can contribute arithmetic premises; that does not construct a new conjunction goal. `check --holes` reports chosen tiers and proof sizes; `--stats` summarizes obligations.

<!-- spec: 1.90:55 example -->
~~~rust check
logic fn bounded(n: Int, low: @(0 <= n), high: @(n <= 10))
    -> @(0 <= n && n <= 10)
{
    And::Intro(low, high)
}
~~~
