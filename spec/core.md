# Core terms

Logical code is not checked as it is spelled. It is translated to a small language of typed terms, and the rules about proofs are stated on those. [RFC 0005](../design/rfcs/0005-core-terms.md) explains the design.

## Elaboration

r[core.elaboration]
Logical code is elaborated to *core terms* before any rule of this specification about equality, rewriting, inferred values, or conversion applies, and those rules speak of core terms, not of how a program spells them. In a core term, a place is the value name of the version it has at that point, with its projections; a runtime value used as a value is the model of that ([`models.observation`](models.md)); `snap(x)` is the value name itself; and a logic function whose parameter has a runtime type is applied to the snapshot.

r[core.values]
A value name stands for the snapshot of one version of a runtime place ([`proofs.versions`](proofs.md)). It is a variable: conversion never replaces it by another term, and what is known of it is the facts in the context ([`runtime.facts`](runtime.md)). Two versions of one place have two value names, whatever values they hold.

r[core.wrap]
For a machine integer type `T` of `w` bits, with `M` the `w`th power of two, `wrap` at `T` of an integer `x` is the Euclidean remainder of `x` by `M` when `T` is unsigned, and the Euclidean remainder of `x + M/2` by `M`, less `M/2`, when `T` is signed. The Euclidean remainder is never negative. A program writes it as a logic function of the type, as in `u32::wrap(a + b)`.

## Conversion

r[core.conversion]
Two core terms *convert* when these reductions bring them to the same term, up to the names of bound variables: arithmetic and comparisons on literals; an `if` on a literal, a `match` on a constructor, and a field of a constructed value; a call of a logic function that does not call itself, a model included, which becomes its body; a call of a logic function that calls itself, which becomes its body only when the argument it recurs on is a constructor or, for a declared measure, every argument the measure reads is a literal; either only where every name in the body could be written at the point of the call; and a constant or a ghost `let` that is not `mut`, which becomes its value. Nothing else converts: no law of arithmetic, and no fact about a value name. Reduction is tried only as far as a comparison needs, within a fixed number of steps, and a comparison that does not finish within it is an error, not a finding that the terms differ.

## Sequences

r[core.sequences]
A sequence has a length, `s.len()`, and is indexed by an `Int`. `s.get(i)` is a `Maybe` of the element at `i`, which is empty when `i` is negative or not less than the length. `s.at(i, p)` is the element at `i`, for a proof `p` of `0 <= i && i < s.len()`. `s.update(i, v)` is the sequence with `v` at `i`, of the same length, and is `s` itself when `i` is outside it.

r[core.sequences.default]
`s[i]`, with no proof, is allowed only when the element type has a *default*, a term that Locus derives from the type's structure: `Int`, `Nat`, `Bool`, a sequence, a `Maybe`, a tuple or a logical struct without proof fields whose parts all have one, and the snapshot of a machine integer, of a `bool`, or of a struct, tuple, array, or slice built from those. A proof type, a type parameter, and an opaque snapshot have none. `s[i]` is the element at `i` when `i` is inside the sequence, and otherwise the default, which converts to nothing else: nothing is known of it.
