# Snapshot capture and explicit view reconciliation

- **Kind:** design
- **Status:** open
- **Blocks:** Accepting RFC 0008 and implementing generated storage, unsupported-field deferral, and loan restoration
- **Rules:** `models.observation`, `models.snapshots.opaque`, `models.capture.available`, `models.capture.footprint`, `models.storage`, `models.stamps`, `collections.vec.element-loan`

## Work

[RFC 0008](../design/rfcs/0008-snapshot-capture-and-views.md) records the owner's snapshot-only capture, explicit library views, type-based Unit barriers, ordinary contract-hidden stamps, and collection API reconciliation. RFCs 0001, 0002, 0005, 0006 and the unreviewed provider RFC 0007 are updated alongside the spec and examples. Historical decision logs remain historical.

## Remaining gates

- Generate deferred logical storage without a bound on the runtime struct. Check exact read-after-write value preservation, assignment through aliases, helper calls, destructuring, and lifetime erasure, in addition to rustc typing.
- Preserve ordinary Rust declarations with unsupported foreign fields. The higher-ranked lifetime deferral experiment is not evidence of a stable compiler guarantee; [unmarked-fields-and-the-superset](unmarked-fields-and-the-superset.md) remains open.
- Represent recursive snapshots and cuts uniformly through generic and associated types. Never switch a snapshot family after monomorphization.
- Restore collection contents at the actual end of a mutable element loan, across reborrows, callbacks, panic and recoverable unwinding. A forgettable destructor is insufficient. Multiple loans need a separate protocol.
- Enumerate supported native snapshots and wrappers. Unsupported third-party values require explicit `Opaque<T>` or another wrapper; no universal foreign fallback is promised.
- Review RFC 0007's providers, generic Clone overlap, and whether any ordinary call may expose a bridge fact without a result proof. Those remain independent open decisions.

## Log

### 2026-11-24, owner-requested reconciliation

The owner asked to add the revised snapshot/view specification and Claude's latest review to the PR, reconcile other RFCs, and push it with the previously saved provided-implementation RFC. This authorizes the expectation changes enumerated in RFC 0008. Snapshot/view selection and the two type-based barriers follow that discussion; `mint` uses existing `contracts.calls.unrelated` and introduces no intrinsic or axiom. The owner also restored GitHub authentication. No RFC is marked accepted and no implementation gate is claimed passed by this documentation change.
