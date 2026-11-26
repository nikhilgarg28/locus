# Finite logical maps and sets: representation and proof interfaces

- **Kind:** decision
- **Status:** open
- **Blocks:** Completing the logical Set/Map library after the chosen key-set API
- **Rules:** `logic.eq.same`, `logic.types.positive`, `proofs.elimination`, `proofs.equality.funext`

## Decided API

The owner chose logical `Set<K>` and `Map<K,V>::keys() -> Set<K>`, with membership
specified by `iff(m.keys().contains(k), m.get(k) !== Maybe::None)`. RFC 0005's
[API record](../design/rfcs/0005-core-terms.md#finite-collection-sets-and-map-keys)
keeps this choice separate from its implementation. `keys()` is an unordered
observation, not a sequence, iterator, or ordering policy. Keys of a finite
collection map form a finite set.

Extensional contents semantics is a library goal to prove from the selected
representation. It is not permission to override `===` or assume a set axiom.
The public key-set result and membership obligation are adopted; this issue
holds only the unsettled representation and remaining API/proof choices.

## Question

Which representation supports finite `Map` and `Set`, checked membership and
extensionality lemmas, and any later enumeration API under Locus's existing
rules? What bounds and proof interfaces does it require?

In particular:

- What finiteness evidence is stored, and how are cardinality and finite support
  proved without extracting logical data from an existential proof?
- Which functions and data are part of `===`? Function extensionality settles
  logical function fields, not unrelated retained data such as an enumeration.
- What strict-positivity/variance obligations follow from that representation?
- Is there an enumeration API at all, and what extra data, bounds or explicitly
  chosen construction could implement it? `keys()` does not choose an order.
- Does a future general set abstraction admit infinite sets? This API decision
  concerns finite collection key sets and does not decide that separate policy.

## Options to examine, none selected

1. **Membership/lookup functions with checked finiteness evidence.** A key set
   could expose a membership predicate and a proof of finite support. No data
   enumeration follows merely from a proof that an enumeration exists. Prove
   extensional equality using the actual function/proposition representation
   and its evidence; I07a's theorem alone does not settle every field or the
   logical API needed to relate equivalent propositions.
2. **Finite data modulo a stated equivalence.** A quotient could identify order
   and duplicate differences, but Locus's source quotient construction,
   eliminators and checked realization are not chosen. The current trust
   baseline's Lean quotients do not by themselves expose such a source API.
3. **Normalized finite data under sufficient explicit bounds.** Prove that the
   representation is canonical under a selected normalization policy. This
   can require ordering or other evidence and may restrict the generic API.
   No canonical ordering of arbitrary `K` is adopted here.

An ordinary record retaining `Seq<K>` is not made extensional by making its
field private. Two distinct orderings remain distinguishable by logical
projections available inside its module. A lemma equating them would have to
be proved under ordinary equality, not asserted because clients cannot see it.

## Source obligations and boundaries

The following `.open` cases record future requirements; their provisional
library paths and lemma spellings are not an added API:

- `map-key-set-membership`: return the chosen set type and prove the membership
  condition with a checked library theorem.
- `set-extensional-contents`: determine a representation for which sameness of
  members establishes ordinary `===`, then check that theorem.
- `map-key-set-enumeration`: decide whether generic key sets may be enumerated
  and which evidence/order policy such a function requires.

No new axioms, choice function, proof-to-data elimination, or runtime contract is
introduced by this record. Runtime maps' selected key equivalence, retained
representatives and `Borrow` compatibility remain I08/bridge work; this issue
must not silently select those policies. The existing stdlib study remains an
unverified candidate, not an implementation of the adopted API.

## Log

### 2026-11-26, owner I07b discussion and PR #26

After adopting explicit functional extensionality in I07a, the owner chose a
logical `Set<K>` and requested that `Map<K,V>::keys()` return it rather than a
sequence. The owner accepted this unordered API, not a quotient, witness
extraction, enumeration order or complete representation. This issue records
the remaining choices and their proof obligations while RFC 0005 records the
adopted API subset.
