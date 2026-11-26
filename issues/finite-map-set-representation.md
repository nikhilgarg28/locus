# Finite logical maps and sets: representation and proof interfaces

- **Kind:** decision
- **Status:** open
- **Blocks:** Completing the logical Set/Map library after the chosen key-set API
- **Rules:** `logic.eq.same`, `logic.types.positive`, `proofs.elimination`, `proofs.equality.funext`

## Decided API

The owner chose logical `Set<K,E=Same>` and `Map<K,V,E=Same>`, with
`E: Equiv<K>` and `keys() -> Set<K,E>`. The membership condition is
`iff(m.keys().contains(k), m.get(k) !== Maybe::None)`. Membership and lookup must
respect `E`; map values use built-in `===` for the extensional-content goal.
RFC 0006's `PartialEquiv<T=Self>` / `Equiv<T=Self>` interface serves both the
type's own logical `==` and an explicitly selected policy. `Same` defaults to
`===`; `E=K` selects the type's own lawful equivalence. The separate name
`Equivalence` is dropped. Policy arguments do not alter logical `==` dispatch.

RFC 0005's [API record](../design/rfcs/0005-core-terms.md#finite-collection-sets-and-map-keys)
keeps these choices separate from implementation. `keys()` itself is unordered;
keys of a finite collection map form a finite set. Different nominal policies
remain different collection types; explicit conversion can merge classes and
requires a treatment of conflicting map values.

The owner also settled the direction of methods returning a sorted `Seq` for
appropriately ordered keys. Their exact bounds, signatures, representation and construction remain
open under the constraints below. Extensional contents sameness must be proved
from the selected representation, not supplied by overriding `===` or assuming
a set axiom.

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
- What data, exact-key or class-order laws, and construction implement the desired
  sorted enumeration API? `keys()` itself does not choose an order.
- Does a future general set abstraction admit infinite sets? This API decision
  concerns finite collection key sets and does not decide that separate policy.

## Ordered enumeration constraints

For `Set<K,Same>`, a unique sorted duplicate-free enumeration needs a total order
that distinguishes exact keys: comparison is Equal iff its operands are `===`.
A bare `OrdLogic` relation may be total only modulo the type's chosen equivalence,
so it can tie distinct exact keys. Finiteness plus those stronger ordering laws
makes uniqueness provable; it still does not extract an enumeration from a
proof of existence under `proofs.elimination`.

For nontrivial `E`, a raw-key order does not in general choose a canonical
representative for each class. Even/odd classes of integers have no least
member under the ordinary order. A `Seq<K>` result needs canonical
representatives/normalization with appropriate laws and order; alternatively
return a sequence of classes with a class order. Both need explicit API and
implementation work. Retained insertion order or remembered representatives
remain logical data if stored, even privately, and no custom `===` may erase
them. A conversion between policies also needs explicit class merging and,
for maps, value-conflict semantics. No mechanism is selected by this record.

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
- `map-key-set-enumeration`: determine the evidence and construction needed for
  generic enumeration. The additional sorted-exact and sorted-equivalence
  cases isolate exact ordering from canonical representative selection.

No new axioms, choice function, proof-to-data elimination, or runtime contract is
introduced by this record. Runtime Eq/Hash/Ord bridges must align with the selected `E` without changing
Rust dispatch. I08a now fixes ordinary runtime insertion as preserving an
existing equivalent key; a separate snapshot-derived observer describes the
retained entries. Its representation must contain enough information: `Unit`
plus only the class-to-value map cannot recover key representatives. A wrapper
may enrich existing logical contents, without a mandated redundant field.
Actual Eq/Hash/Ord/Borrow compatibility and complete bridge/observer contracts
remain I08b work; this issue does not implement them. The existing stdlib study remains an
unverified candidate, not an implementation of the adopted API.

## Log

### 2026-11-26, owner I07b discussion and PR #26

After adopting explicit functional extensionality in I07a, the owner chose a
logical `Set<K>` and requested that `Map<K,V>::keys()` return it rather than a
sequence. The owner accepted this unordered API, not a quotient, witness
extraction, enumeration order or complete representation. This issue records
the remaining choices and their proof obligations while RFC 0005 records the
adopted API subset.

### 2026-11-26, owner unifies equality domains and requests ordered enumeration

The owner adopted `PartialEquiv<T: Logical = Self>` with symmetry/transitivity,
`Equiv<T: Logical = Self>` adding reflexivity, and `Same` as the default policy
for `Set<K,E>` / `Map<K,V,E>`. `keys()` preserves `E`. Values retain `===` in
map content sameness; another collection `==` is independent. Logical operators
use the key type's own relation, not the collection's policy. Native bridges
must match the selected relation without changing Rust dispatch.

The owner additionally settled sorted sequences for appropriately ordered
sets. The record retains exact-key ordering and canonical-class requirements;
it does not assume that the current `OrdLogic` bound supplies them. Quotient,
choice/finite-support construction, representative normalization, policy
conversion conflicts and exact enumeration interfaces remain open. No float
representation, capture-default or broader trait inventory change is recorded.

### 2026-11-26, owner settles I08a runtime key retention

The owner approved ordinary insertion retaining the existing equivalent key and
replacing its value. Changing the representative is explicit. Logical Map stays
classes-to-values; actual retained keys/entries are a separate snapshot-derived
observation used by key-returning APIs. RFC 0005 and the conventions chapter
record the cases and snapshot-adequacy constraint. A wrapper may enrich existing
logical contents, but no second field, representation, or restoration protocol
is mandated. The actual I08b equality/hash/order/borrowing bridges remain open.
