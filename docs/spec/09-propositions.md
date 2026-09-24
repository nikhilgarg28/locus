+++
id = "language-propositions"
title = "Propositions"
group = "Now"
spec_chapter = 1
order = 108
route = "specification/propositions.html"
description = "Claims, predicates, named alternatives, and quantification."
+++

# Propositions

<!-- spec: 1.0:10 informative -->
A proposition describes a claim. A proof establishes it. Locus lets you name and combine claims before choosing how to prove them: a calculated predicate uses a logical definition; a named proposition lists sufficient reasons for it to hold.

## Proposition values

<!-- spec: 1.6:1 legality-rule -->
`prop!(expression)` creates a `Prop`: it accepts a proposition or turns a logical Boolean into the claim that it holds. It captures permitted observations at their current versions, with no runtime closure or lasting borrow. A captured proposition can outlive a local name if its dependencies are closed or explicitly packaged.

<!-- spec: 1.90:45 example -->
~~~locus check
fn claims() -> (Prop, Prop) {
    // Both are well-formed claims; only the first can be proved.
    (prop!(2 + 2 == 4), prop!(2 + 2 == 5))
}
struct Certificate { claim: Prop, evidence: @claim }
fn package() -> Certificate {
    let n: u8 = 3;
    Certificate { claim: prop!(n > 0), evidence: prove!(n > 0) }
}
~~~

<!-- spec: 1.6:2 legality-rule -->
Inside logical computation, unsigned machine integers default to `Nat`, signed machine integers to `Int`, and runtime booleans to `Bool`. Thus `prop!(x + 1 > x)` uses mathematical addition. `model!(x)` states a physical observation explicitly; `as Int` widens a Nat without changing its value. An explicitly wrapping operation keeps its machine-width meaning. Physical comparisons outside logic still require matching machine types.

## Calculated predicates

<!-- spec: 1.6:3 legality-rule -->
A `logic fn` returning `Prop` defines a predicate by calculation. Its body may use immutable lets, conditionals, matches, and total logical calls. Its claim is proved using its defining equation through `fold!` or `unfold!`; the definition does not assert that its body is true.

<!-- spec: 1.90:46 example -->
~~~locus check
logic fn magnitude_is_nonnegative(x: Int) -> Prop {
    let magnitude = if x < 0 { -x } else { x };
    prop!(magnitude >= 0)
}
logic fn always(x: Int) -> @magnitude_is_nonnegative(x) {
    if x < 0 {
        fold!(magnitude_is_nonnegative, prove!(-x >= 0))
    } else {
        fold!(magnitude_is_nonnegative, prove!(x >= 0))
    }
}
~~~

## Named proposition arms

<!-- spec: 1.6:4 informative -->
A named proposition lists alternative reasons it can hold. Each arm’s body computes a proposition to be proved. The following declaration allows zero, or a value below a supplied bound no greater than ten.

<!-- spec: 1.6:5 example -->
~~~locus check
prop Small(n: Int) {
    Zero => { prop!(n == 0) }
    Below { limit: Int } => { prop!(n < limit && limit <= 10) }
}
logic fn seven_is_small() -> @Small(7) {
    Small::Below { limit: 10 } @
        And::Intro(prove!(7 < 10), prove!(10 <= 10))
}
~~~

<!-- spec: 1.6:6 legality-rule -->
A named-arm constructor takes its declared witnesses, then `@` and evidence of its body, to establish the enclosing proposition. Arms may overlap or leave inputs uncovered. They are sufficient alternatives, not equations equating each body to the predicate. Matching evidence exposes an arm’s witnesses and body evidence, under the proof-elimination rules.

<!-- spec: 1.91:21 informative -->
Use `Arm => { ... }`, `Arm(witness: T) => { ... }`, or `Arm { witness: T } => { ... }`. The same unit, tuple, or named shape is used in construction and matching. Every arm has a name and an explicit evidence slot. Nested evidence constructions need parentheses or a block.

## Connectives and quantifiers

<!-- spec: 1.6:7 legality-rule -->
Inside a proposition formula, `P && Q`, `P || Q`, `!P`, and `P => Q` construct conjunction, disjunction, negation, and implication. `forall (x: T) { F }` and `exists (x: T) { F }` bind a Logical value and lower to the checked prelude propositions `ForAll<T>` and `Exists<T>`. Kernel-validated schemas authorize their use in recursive propositions; their names alone grant no authority.

<!-- spec: 1.90:47 example -->
~~~locus check
logic fn combine(p: Prop, q: Prop) -> Prop { prop!(p && (p => q)) }
logic fn a_larger_integer(n: Int) -> @(exists (m: Int) { m > n }) {
    Exists::<Int>::Witness(n + 1) @ prove!(n + 1 > n)
}
logic fn reflexivity() -> @(forall (n: Int) { n == n }) {
    ForAll::<Int>::Each(|n: Int| prove!(n == n)) @ True::Intro
}
~~~

<!-- spec: 1.25:4 legality-rule -->
`Exists<T>::Witness(value) @ proof` provides a witness and evidence. `ForAll<T>::Each(prove_each) @ True::Intro` provides a logical proof function for arbitrary inputs. Universal evidence can be applied to an argument. An existential witness is usable only inside further proof construction, not extractable as data.

## Recursive propositions

<!-- spec: 1.27:11 legality-rule -->
Recursive occurrences in arm conditions must be strictly positive and visible through supported logical forms: conjunction, disjunction, quantification, or an implication’s conclusion when its premise is independent. Negated, hidden, or premise-side recursive occurrences are rejected. A recursive claim does not run a recursive search. Constructed evidence is finite; proving facts about it uses checked induction.

<!-- spec: 1.90:48 example -->
~~~locus check
prop Reachable(from: Int, to: Int) {
    Same => { prop!(from == to) }
    Next(middle: Int) => {
        prop!(middle == from + 1) && Reachable(middle, to)
    }
}
logic fn one_step(n: Int) -> @Reachable(n, n + 1) {
    let rest: @Reachable(n + 1, n + 1) = Reachable::Same @ _;
    Reachable::Next(n + 1) @ And::Intro(prove!(n + 1 == n + 1), rest)
}
~~~
