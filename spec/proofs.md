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
A proof hole, `_` or `prove!(P)`, is filled by trying three things in this order, and nothing else: a proof that is known at that point of the proposition required, up to evaluation; `compute()`; and `linear` given everything that is known at that point. What is known at a point is the proofs in scope there and the facts that the language gives there, such as the condition of an enclosing branch and the value that a place was given. A hole that none of the three fills is an error.

r[proofs.procedures]
A procedure is a function of Locus's library that finds a proof when the program is checked. The procedures are `compute` and `linear`, and a program defines none. A call of one is written where a proof is expected and proves the proposition expected there; where no proposition is expected, the call is an error. A procedure uses the proofs it is given as arguments and the definitions of logic functions, and nothing else: it reads no proof in scope, instantiates no `forall`, and opens no `exists`. The same call gives the same result on every machine.

r[proofs.procedures.compute]
`compute()` proves an equality whose two sides evaluate to the same value, and a proposition that evaluates to `true`. It takes no proofs.

r[proofs.procedures.linear]
`linear(facts)` proves a proposition that follows from the proofs in the tuple `facts` by equality and linear integer arithmetic. The proposition and the facts are first reduced by evaluation. What it then understands is `===` and the comparisons; `+`, `-`, and multiplication, division, and remainder by a literal, on `Int` and `Nat`; `&&`, `||`, `!`, and `==>`; an `if` between two terms that it understands; and that equal arguments give equal results. Every other subterm is an atom, about which it knows only what the facts say. When the facts contradict each other, it proves any proposition.

r[proofs.procedures.expand]
What a hole or a procedure finds is a proof term with no hole and no procedure call in it. Writing that term in place of the hole or the call gives a program that is accepted, and that means the same.

r[proofs.equality]
For a proof `e` of `a === b`: `e.symm()` proves `b === a`; `e.then(f)`, for a proof `f` of `b === c`, proves `a === c`; `eq::congr(g, e)`, for a logic function `g`, proves `g(a) === g(b)`; and `h.rewrite(e)`, for a proof `h`, proves the proposition of `h` with every occurrence of `a` replaced by `b`. `eq::refl(a)` proves `a === a`. Each takes proofs of `===`, and of no other equality.

r[proofs.unfold]
`eq::unfold(f)` proves the defining equation of the logic function `f`: for every argument, `f` of it is the body of `f` with the parameter replaced by it. It unfolds one layer. A declared proposition has no defining equation.

r[proofs.values.inferred]
In logical code, where an argument is a logical value, `_` stands for the value that the types of the proofs passed beside it determine. When they determine no value, it is an error.

r[proofs.facts.named]
In `if h: c { A } else { B }`, `h` is a proof of `c` in `A`, and a proof of `!c` in `B`, and is in scope nowhere else. The program runs as `if c { A } else { B }` does.

