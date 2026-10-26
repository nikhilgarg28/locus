# Propositions and proofs

A proposition is a claim, of type `Prop`; forming one proves nothing. A proof is a value of a proof type `@P`, checked by the kernel. Proofs are irrelevant and ghost. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

## Propositions

Each local and parameter is a binding, and every write to a binding or to any part of it gives the binding a new version, as in SSA form.

r[proofs.versions]
A proposition is about the versions of the places it reads where it is formed. A write gives a binding a new version even when it writes the value the binding already had. A proof of a proposition about one version is not a proof of the same formula about another version.

r[proofs.connectives]
In logical code, `&&`, `||`, `!`, and `==>` combine propositions, including values of type `Prop`, into propositions. A proof of `P && Q` needs proofs of both `P` and `Q`, and a proof of `P || Q` a proof of either.

r[proofs.props.constructors]
A proof of a proposition declared with `prop` is built only by one of its arms, from that arm's witnesses and a proof of its condition, with the parameters and witnesses substituted.

r[proofs.props.matching]
A match on a proof of a declared proposition covers every one of its arms, and each arm of the match receives that arm's witnesses and a proof of its condition.

r[proofs.props.positive]
A proposition declaration refers to itself only in strictly positive positions: not under a negation, and not in the premise of an implication.

## Proofs

r[proofs.elimination]
A match on a proof produces a proof. A witness inside a proof never becomes a logical or runtime value.

r[proofs.partial]
A proof that a runtime function returns needs to hold only when the function returns. A runtime function that never returns needs no proof of its result.
