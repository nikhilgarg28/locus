# Propositions and proofs

A proposition is a claim, of type `Prop`; forming one proves nothing. A proof is a value of a proof type `@P`, checked by the kernel. Proofs are irrelevant and ghost. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

## Propositions

Each local and parameter is a binding, and every write to a binding or to any part of it gives the binding a new version, as in SSA form.

r[proofs.versions]
A proposition is about the versions of the places it reads where it is formed. A write gives a binding a new version even when it writes the value the binding already had, and a write through a reference gives what it refers to a new version. Two points share a version only if no write separates them on any path, including a later iteration of a loop. A proof of a proposition about one version is not a proof of the same formula about another version.

r[proofs.connectives]
In a proposition, `&&`, `||`, `!`, and `==>` are connectives: they combine propositions, including values of type `Prop`, and a `Bool` operand stands for the proposition that it is true. A proof of `P && Q` needs proofs of both `P` and `Q`, and a proof of `P || Q` a proof of either.

r[proofs.props.constructors]
A proof of a proposition declared with `prop` is built only by one of its arms, from that arm's witnesses and a proof of its condition, with the parameters and witnesses substituted.

r[proofs.props.matching]
A match on a proof of a declared proposition covers every one of its arms, and each arm of the match receives that arm's witnesses and a proof of its condition.

## Proofs

r[proofs.elimination]
A match on a proof produces a proof. A witness inside a proof never becomes a logical or runtime value.

r[proofs.partial]
A proof that a runtime function returns needs to hold only when the function returns. A runtime function that never returns needs no proof of its result.

r[proofs.assume]
`unsafe assume!(P)` is a proof of `P` that is taken on trust, and `unsafe assume!(_)` is one of the proposition that its position requires. It is the only way a program takes a fact that it does not prove. It is written where a proof is expected, in verified code, and nowhere else: an unverified function holds no logical code ([`logic.unverified.plain-rust`](logical-layer.md#unverified-functions-and-unsafe-code)). It performs no operation and is not an `unsafe` block, so a function that contains it stays verified, and everything else in that function is checked as before. An assumption allows nothing that a proof would not: a verified function still contains no `unsafe` code ([`logic.verified.restrictions`](logical-layer.md#unverified-functions-and-unsafe-code)), and no exposed logical data reaches unverified code because of one. Every result lists the assumptions it relies on, in whichever crate they are written, each with its proposition and its place ([`assumed.stated`](../design/TRUST.md)).

## Constructing proofs

A proof is a term: an expression built from the forms of this chapter. [RFC 0003](../design/rfcs/0003-proof-construction.md) explains the design. Two things find a proof when the program is checked, a hole and a procedure, and each finds a term that the kernel checks.

r[proofs.holes]
A proof hole, `_` or `prove!(P)`, is filled by trying three things in this order, and nothing else: a proof in scope, or a fact in the context ([`runtime.facts`](runtime.md)), of the proposition required; `compute()`; and `linear` given every proof in scope and every fact in the context. A hole that none of the three fills is an error.

r[proofs.procedures]
A procedure is a function of Locus's library that finds a proof when the program is checked. The procedures are `compute` and `linear`, and a program defines none. A call of one is written where a proof is expected and proves the proposition expected there; where no proposition is expected, the call is an error. A procedure uses the proofs it is given as arguments and the definitions of logic functions, and nothing else: it reads no proof in scope, instantiates no `forall`, and opens no `exists`. The same call gives the same result on every machine.

r[proofs.procedures.compute]
`compute()` takes its term from the proposition required. For `a === b` it is `eq::refl(a)`, accepted when `a` and `b` convert ([`core.conversion`](core.md)). For a `Bool` used as a proposition it is `eq::refl(true)`, accepted when the `Bool` converts to `true`. It takes no proofs.

r[proofs.procedures.linear]
`linear(facts)` proves a proposition that follows from the proofs in the tuple `facts` in a fixed theory. The theory is: equality, with equal arguments giving equal results; linear arithmetic on `Int` and `Nat`, which is `+`, `-`, the comparisons, and multiplication, division, and remainder by a literal; `&&`, `||`, `!`, and `==>`, and an `if` between two terms; that the constructors of a logical data type or of a snapshot are injective and distinct, and that two structs, tuples, or snapshots are equal exactly when their fields are, an opaque part of a snapshot counting as a field; that a `Nat` is not negative and that a machine integer's value lies in its type's range; and that `wrap` gives a value in the type's range, equal to its argument when the argument is in range ([`core.wrap`](core.md)). It applies none of this to the constructors of a proposition, since any two proofs of one proposition are equal. Terms are compared up to conversion, after a name that the facts say equals a literal is replaced by it. Every other subterm is an atom, about which it knows only what the facts say: it instantiates no `forall`, multiplies no two variables, and knows nothing about sequences. When the facts contradict each other, it proves any proposition.

r[proofs.procedures.expand]
What a hole or a procedure finds is a proof term with no hole and no procedure call in it. Writing that term in place of the hole or the call gives a program that is accepted, and that means the same. Where the term speaks of a value that the program can no longer spell at that point, such as the value a variable had before a write, the expansion also adds a ghost `let` that names the value, where it was current. Where the term would have to write out the value that a borrow has yet to end with, and nothing determines that value for a `_`, the proof has no form in source. Like every accepted proof, it has a complete core term that the kernel checks.

r[proofs.equality]
The equality functions take proofs of `===` between core terms of one type, and of no other equality. For a proof `e` of `a === b`: `e.symm()` proves `b === a`; `e.then(f)`, for a proof `f` of `b === c`, proves `a === c`; `eq::congr(g, e)`, for a logic function `g` whose parameter has the type of `a`, proves `g(a) === g(b)`; and `h.rewrite(e)`, for a proof `h`, proves the proposition of `h` with every occurrence of the core term `a` replaced by `b`, without capturing a bound variable. A `rewrite` that finds no occurrence is an error. `eq::refl(a)` proves `a === a`. On runtime values, `a === b` is an equality of models, so it rewrites the model of `a`, and nothing read from the fields of `a` ([`core.elaboration`](core.md)).

r[proofs.unfold]
`eq::unfold(f)` proves the defining equation of the logic function `f`: for every argument, `f` of it is the body of `f` with the parameter replaced by it. It unfolds one layer. A declared proposition has no defining equation. `eq::unfold(f)` is available where conversion would unfold a body like `f`'s: where every name in the body could be written at the point of the call ([`core.conversion`](core.md)). Elsewhere `f` is known by the lemmas and the contracts that its module provides.

r[proofs.values.inferred]
In logical code, `_` in an argument whose parameter is a proof is a proof hole. `_` in an argument whose parameter is any other logical value stands for the value that matching determines, on core terms and before conversion: the parameters' declared propositions against the propositions of the proofs passed beside it, and, when the call's result is a proof, the declared result against the proposition required of the call. An argument that is itself a hole or a procedure call determines nothing. When no value is determined, or more than one is, it is an error.

r[proofs.facts.named]
In `if h: c { A } else { B }`, `h` is in scope in the two branches and nowhere else, and the program runs as `if c { A } else { B }` does. In logical code, `h` is a proof of `c` in `A`, and of `!c` in `B`. In runtime code, `c` is evaluated once to a `Bool` term ([`runtime.conditions`](runtime.md)), and `h` is a proof of that term in `A` and of its negation in `B`: of what the program checked, and of nothing more.

r[proofs.given]
`given(P)` is a proof of `P` when `P` converts to a fact that is in the context at that point ([`runtime.facts`](runtime.md)). It searches for nothing, and it is an error when no fact there is `P`.

r[proofs.facts.types]
Locus's library has a lemma for each fact that a type gives: for each machine integer type, that a value of the type lies in its range, as in `u8::range(x)`, and for `Nat`, that one is not negative, `Nat::nonneg(n)`. `linear` and a hole use these facts without being given them.

