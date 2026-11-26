# Map key representatives and snapshot observers

**Ordinary insertion preserves the existing equivalent key and replaces its
value.** This is the owner-adopted standard-library convention (I08a), recorded
in [RFC 0005](../design/rfcs/0005-core-terms.md#runtime-map-key-representatives).
It guides the map library's contracts; this chapter adds no compiler rule or
permission to assume an unproved contract.

## Keep abstract contents separate from stored keys

A logical `Map<K,V,E>` describes equivalence classes of keys mapped to values.
It should not change merely because a runtime map stores a different
representative of the same class. APIs exposing concrete keys need a separate
snapshot-derived observer of the retained keys or entries.

For discussion, write `stored_key(s, q)` for the retained key's snapshot at
logical query `q`, or absence. This is an illustrative observer name, not a
chosen method signature. If the runtime key type is `R`, its result describes
`Snap<R>`, rather than inventing a new representative from the abstract class.
The key-observation map into `K` and the implementation's equality bridge must
show that this is the queried class. Exact capture and bridge interfaces remain
separate work.

The actual retained object is preserved on an equivalent-key insertion. A
snapshot contract states equality only of what the key snapshot observes; it
does not claim bit identity, pointer identity, or equality of omitted payloads.
Ordinary insertion of a previously absent class retains the incoming key.
Replacing a representative is explicit: remove the entry and insert the desired
key, or use a separately specified replacement operation. No such new operation
or atomicity guarantee is introduced here.

## An equivalent key can look different

Consider runtime keys with observable fields `{ id, label }`, and a separately
validated key equivalence comparing `id` only. Let `A = { id: 7, label: 10 }`
and `B = { id: 7, label: 20 }`. Their class is the same while their snapshots
differ. This example presupposes the bridge; it does not certify arbitrary
handwritten Eq/Hash/Ord implementations.

| Operation, on normal return | Abstract contents | Retained-key observation |
|---|---|---|
| Insert `(A, 1)` into an empty map | Class 7 maps to 1 | A |
| Then insert `(B, 2)` | Class 7 maps to 2; previous value is 1 | Still A |
| Query `get_key_value` using B | Finds value 2 | Returns a reference to the stored A, not B |
| Remove the entry using B | Class 7 becomes absent | Returned entry contains A and 2 |
| After removal, insert `(B, 3)` | Class 7 maps to 3 | B |
| Independently build `(B, 1)` in another empty map | Same abstract contents as the first row | B, so representative observations can differ |

An inequivalent insertion creates another class and preserves the previous
class's representative. Rebalancing or rehashing can change layout and iteration
order without replacing retained keys. A key iterator exposes retained keys;
it does not return canonical representatives of the abstract classes. Any
ordering guarantee needs its own contract. The desired sorted enumeration of
logical `Set` is a different API with the conditions in RFC 0005.

`get_key_value`, removal returning an entry, and key iteration must therefore
refer to the retained-entry observation, not only to `Map<K,V,E>`. Equality of
the abstract map alone does not prove equality of those observations. Equality
of whole snapshots still supports ordinary logical congruence for every pure
observer of them.

## Derive observers from information the snapshot retains

For native permissioned storage, derive the retained-entry observation from
its supported storage snapshot. This needs no redundant ghost ledger merely
to restate data the snapshot already contains.

For wrapped native storage, the library's checked or explicitly trusted
contracts must establish correspondence with the real entries. A pure observer
cannot recover representatives from `Unit` plus only a class-to-value map:
those observations have discarded the very distinction between A and B. Trust
cannot consistently add a pure observer that distinguishes identical snapshots.

A wrapper may enrich its existing logical contents so that both the abstract
map and representative observations derive from that information. Other
representations can work if they retain equivalent information and establish
correspondence. This convention mandates no second ghost field, exact storage
representation, custom snapshot behavior, or new restoration protocol. Merely
naming an observer establishes no relationship to the opaque payload. Any
assumed correspondence or retention contract must be recorded under the existing
[trust inventory](../design/TRUST.md), rather than supplied implicitly by this
convention.

All mutations and native handoffs must preserve that correspondence whenever
the wrapper is observable, including the already applicable unwind and loan
boundaries. These normal-return cases introduce no stronger unwind guarantee.
See [snapshots and ghost state](snapshots-and-ghost-state.md) for the general
pattern. Actual Eq/Hash/Ord/Borrow compatibility and complete executable map
contracts remain open implementation/design work, not facts supplied by this
convention.
