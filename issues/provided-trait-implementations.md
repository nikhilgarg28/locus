# Provided trait implementations and bridge evidence

- **Kind:** decision
- **Status:** open
- **Blocks:** RFC 0007 becoming a reviewed proposal, ordinary Rust calls using checked trait guarantees

## Question

[RFC 0007](../design/rfcs/0007-provided-trait-implementations.md) records the owner's
nested-implementation design as a draft, with the I04 hashing architecture
adopted by the owner on 2026-11-25. What evidence connects a
provided ordinary Rust method to its checked operation, and how is that evidence
available to callers under the current proof-in-result discipline?

The owner's chosen scoping is fixed for this draft: `Self` in a nested `impl`
header names the outer implementor; inside its body it names the inner target.
An outer-type alias such as `as Source` is not part of this proposal.

## What it blocks

The draft cannot become a complete proposal until it explains call-fact
transport, non-`Self` member signatures and bounds, snapshot validity and effect
premises, concrete hashing interfaces and trust registrations, and cross-crate
enforcement. These are separate from rustc's existing coherence checks.

## Options

- Keep ordinary methods proof-free and give no call facts; proofs remain
  available only through the checked methods.
- Add an explicit checked bridge certificate that transports the checked
  operation's contract to the selected ordinary implementation, without
  introducing arbitrary executable functions into pure logic.
- Introduce a more general verified or assumed contract attachment mechanism,
  with precise implementation identity, effects and trust accounting.

For non-`Self` destinations, begin with expressible cases and explicit bounds,
or add a naming/bound-elaboration mechanism in a later proposal. Hashing's
architecture is selected: `HashChecked` emits a trace through a write-only sink,
its generated `Hash` retains Rust's original generic signature, and designated
standard leaf adapters delegate directly to their actual implementation.
RFC 0007 names the obligations and the remaining implementation gates.
[Checked-hashing followups](checked-hashing-followups.md) retain the deferred
borrowed-lookup, batch, container and user-hasher work. The general provider
options above remain open.

## Log

### 2026-11-23, provided implementation design conversation

The owner approved nested `impl Trait for Type` syntax, chose the `Self`
scoping above, and requested: "push this as unreviewed RFC - we will come
review/critique it later as PR". RFC 0007 preserves that scope. It changes no
spec rules, decided examples, or trust entries.

### 2026-11-24, snapshot/view reconciliation proposed

[RFC 0008](../design/rfcs/0008-snapshot-capture-and-views.md) replaces implicit models with compiler-generated snapshot capture and ordinary explicit views. `Opaque<T>` and `UnsafeCell<T>` snapshot to `Unit`; field-erasure attributes are removed. Collection wrappers keep logical contents with generated conditional storage and offer native-shaped and checked methods. Existing unresolved lowering, loan, coherence, and bridge-fact questions remain open; [snapshot-view-reconciliation](snapshot-view-reconciliation.md) links the validation gates. This update does not rewrite the earlier log or mark an RFC accepted.

### 2026-11-25, owner adopts I04; PR #26

After the successive Codex/Claude reviews, the owner said: "Yes please adopt
this, update the RFCs and update the PR in background." RFC 0007 now records
the selected trace architecture, direct standard adapters, scoped checked
hashing, explicit failure restriction, and trusted-fact inventory. The provider
and optional structural derive remain distinct. Automatic HashEq derivation
requires lawful fields and the same field equality; enum derivation prefixes
the zero-based declaration-order variant index as a Usize event. Borrow
compatibility is owed for the actual projection, equality and traces. Its API,
standard containers/tuples, slice_trace, and user-proved hashers remain later.
Nine Rust probes are published, not represented as Locus verification. This
adopts I04 without accepting all of RFC 0007 or closing the general bridge issue.
