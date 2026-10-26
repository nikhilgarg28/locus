# Propositions and proofs

A proposition is a claim, of type `Prop`; forming one proves nothing. A proof is a value of a proof type `@P`, checked by the kernel. Proofs are irrelevant and ghost. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

## Propositions

r[proofs.snapshot]
A proposition is about the values that the runtime places it reads hold where it is formed. A proof of it remains a proof about those values, and is not a proof of the same formula about later values.

r[proofs.props.positive]
A proposition declaration refers to itself only in strictly positive positions: not under a negation, and not in the premise of an implication.

## Proofs

r[proofs.elimination]
A match on a proof produces a proof. A witness inside a proof never becomes a logical or runtime value.

r[proofs.partial]
A proof that a runtime function returns needs to hold only when the function returns. A runtime function that never returns needs no proof of its result.
