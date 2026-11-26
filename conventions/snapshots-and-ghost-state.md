# Snapshots, views, and stored ghost state

**Prefer deriving observations and views from snapshots wherever practical.**
Store additional ghost state when it supplies information that cannot conveniently
be derived, such as history, an ownership protocol, or the abstract contents of
opaque external storage. This is library-design guidance, not a restriction on
logical fields.

## Derive what the snapshot already describes

A logical observer computes from a frozen snapshot. If a collection's supported
storage snapshot already describes its elements, an `elements` observer can
extract that sequence. Other observers can compute its length, sum, or membership
without maintaining another field containing the same summary.

This avoids a second representation that every mutation must synchronize. A new
snapshot after mutation determines a new observation; an earlier snapshot and
its observations remain historical values. Capture does not update a previously
saved snapshot or run an observer at runtime.

For example, a logical sum derived from the current element sequence needs no
stored ghost sum. Proving how a mutation changes that sum can still require
sequence lemmas. Deriving a view removes synchronization work, not the mathematical
proofs about the operations.

The advice applies only to information the snapshot actually contains and the
observer may access. A view cannot recover an `Opaque<T>` or `UnsafeCell<T>`
payload, bypass privacy, follow a cut reference, or make an unsupported foreign
type capturable. Permissioned storage needs its own specified observation and
access rules; this convention does not provide them.

## Keep ghost state when it adds useful information

Useful cases include:

- **History:** previous operations, an event log, or a chosen witness that is not
  determined by the current runtime representation.
- **Ownership and protocols:** logical facts needed to describe an ownership or
  state-transition protocol, subject to the language's permission rules. Ordinary
  copyable ghost data does not itself create exclusive authority.
- **Opaque external storage:** a wrapper's logical contents when its native
  payload has no contents snapshot. Its operations must establish correspondence
  with that payload using supported checked or trusted interfaces.
- **Proof convenience:** a stored summary or witness whose derivation is possible
  but awkward or expensive for proofs. Its benefit may justify the extra
  consistency obligations.

A logical field is not self-justifying. Write down what it means, which operations
may change the state it describes, and what establishes the relationship after
those operations. A name such as `model`, `view`, or `contents` supplies no
correspondence guarantee by itself.

## Account for every mutation route

A redundant ghost summary must stay synchronized whenever the owner can be
observed. Ordinary setters, callbacks, native handoff, returned mutable references,
and other escape routes all matter. If a method returns `&mut T`, the final value
may be chosen after the method returns, including through reborrows. Updating a
summary only when the reference is first handed out does not track that later
value. A forgettable guard's destructor is not a sufficient restoration protocol.

Deriving an observation from storage can avoid that duplicated-state problem.
It does not make every invariant insensitive to mutation: a sortedness proof can
still fail when an arbitrary element changes. Follow the applicable loan,
preservation, and observation rules rather than treating a derived view as
permission to lend any field.

A native implementation alone does not solve synchronization. A native collection
with a separately stored ghost sum has the same additional obligation as a
wrapper with that sum. Conversely, a wrapper may derive some observations from
what its own supported snapshot contains. The useful distinction is whether a
fact is derived from observed state or separately maintained.

| Representation of a summary | What a mutation requires |
|---|---|
| Pure logical observer over a snapshot | Prove the observer's new result from the new snapshot and operation contract. |
| Separately stored ghost summary | Establish its relationship with the changed state before the owner is observable again. |
| Runtime cached aggregate, such as an executable cached sum | Execute an appropriate cache update and prove consistency; ghost computation cannot perform that runtime write. |

For a runtime cache, an API may need to control mutation, recalculate the cache,
or use an explicitly supported mutation protocol. A future-value proof does not
perform that update, and the pending-final-value rules do not allow storing an
unresolved future value as ghost data either. The general protocol for restoring
a wrapped collection's separately stored contents across escaped loans remains a
separate design issue; this advice does not assume that it has been solved.

## Related semantics and workloads

- [Snapshots and explicit views](../spec/models.md): capture, visibility,
  observation barriers, and ordinary library views.
- [Contracts](../spec/contracts.md): invariant observation, named acquisition,
  and proof-only pending final values.
- [Collection contracts](../spec/collections.md): the specified contents and
  element-loan interface, including its restoration obligation.
- [Standard-library workload](../studies/stdlib/README.md) and
  [mutable-loan workload](../studies/mutable-loans/README.md): unverified studies
  exposing these tradeoffs, not implementations certified by this convention.
