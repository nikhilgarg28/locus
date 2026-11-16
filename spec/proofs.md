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
