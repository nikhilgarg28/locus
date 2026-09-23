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
A proposition states a claim; evidence establishes that claim. Keeping those separate lets a program construct, combine and pass claims without asserting that they are true. Named proposition arms describe the evidence needed to establish each case.

## Proposition values

<!-- spec: 1.6:1 legality-rule -->
A proposition is a Logical value of `Prop`. `prop!(expression)` accepts a proposition directly or turns a logical Bool test into the claim that it holds. Constructing that claim does not prove it. Runtime captures are observed through their models at their current SSA versions; no runtime closure or retained borrow is created. Capturing still requires that the value is available and that the observation is permitted. A proposition may be stored or returned after the original local name leaves scope, provided its logical dependencies are closed or explicitly carried in the returned value.

<!-- spec: 1.6:2 legality-rule -->
Logical machine observations default to mathematical `Int` (`Bool` for physical booleans), so `prop!(x + 1 > x)` uses logical integer arithmetic rather than executing a possibly overflowing machine add. `x as Int` makes the model explicit. Explicit wrapping operations retain their machine-width meaning and then expose that result to the logic. Physical comparisons outside logical contexts still require matching machine types.

<!-- spec: 1.6:3 legality-rule -->
`logic fn predicate(x: Int) -> Prop { prop!(x >= 0) }` is a computed predicate. Its immutable local bindings, conditionals, matches and checked total logical calls are ordinary logical computation. Proofs use its defining equation through `fold!` and `unfold!`; the compiler does not assume the body true.

<!-- spec: 1.6:4 informative -->
A named proposition instead declares introduction alternatives:

<!-- spec: 1.6:5 example -->
~~~locus check
prop Small(n: Int) {
    Zero => { prop!(n == 0) }
    Below { limit: Int } => { prop!(n < limit && limit <= 10) }
}
~~~

<!-- spec: 1.6:6 legality-rule -->
`Small::Zero @ evidence` requires evidence of its arm body and establishes `Small(n)`. `Small::Below { limit: 10 } @ evidence` passes the witness separately from the evidence of the resulting body. The alternatives need not be mutually consistent or exclusive: they are sufficient ways to establish the named proposition. They are not definitional equalities between that proposition and each individual body. Matching a proof recovers the chosen witnesses and its body evidence.

<!-- spec: 1.6:7 legality-rule -->
`P && Q`, `P || Q`, `!P` and `P => Q` compose propositions. `forall (x: T) { F }` and `exists (x: T) { F }` expand to the ordinary checked library propositions ForAll<T> and Exists<T>, using a logical predicate closure over Logical T. The compiler supplies their checked source definitions; user libraries such as Seq are included separately. The kernel validates their registered schemas before permitting recursive occurrences beneath them in inductive declarations. Native quantifier terms remain internal compatibility forms for certificates and dependent proof transport, not another source lowering. No constructor gains authority from its spelling.
