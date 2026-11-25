# Provided trait implementations and bridge evidence

- **Kind:** decision
- **Status:** open
- **Blocks:** RFC 0007 becoming a reviewed proposal, ordinary Rust calls using checked trait guarantees

## Question

[RFC 0007](../design/rfcs/0007-provided-trait-implementations.md) records the owner's
nested-implementation design as an unreviewed draft. What evidence connects a
provided ordinary Rust method to its checked operation, and how is that evidence
available to callers under the current proof-in-result discipline?

The owner's chosen scoping is fixed for this draft: `Self` in a nested `impl`
header names the outer implementor; inside its body it names the inner target.
An outer-type alias such as `as Source` is not part of this proposal.

## What it blocks

The draft cannot become a complete proposal until it explains call-fact
transport, non-`Self` member signatures and bounds, standard hashing's generic
hasher parameter, snapshot validity and effect premises, and cross-crate
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
or add a naming/bound-elaboration mechanism in a later proposal. For hashing,
begin with `HashChecked<H>` for checked hashers, or design a protocol that can
retain Rust `Hash`'s original generic signature. No option is selected here.

## Log

### 2026-11-23, provided implementation design conversation

The owner approved nested `impl Trait for Type` syntax, chose the `Self`
scoping above, and requested: "push this as unreviewed RFC - we will come
review/critique it later as PR". RFC 0007 preserves that scope. It changes no
spec rules, decided examples, or trust entries.

### 2026-11-24, snapshot/view reconciliation proposed

[RFC 0008](../design/rfcs/0008-snapshot-capture-and-views.md) replaces implicit models with compiler-generated snapshot capture and ordinary explicit views. `Opaque<T>` and `UnsafeCell<T>` snapshot to `Unit`; field-erasure attributes are removed. Collection wrappers keep logical contents with generated conditional storage and offer native-shaped and checked methods. Existing unresolved lowering, loan, coherence, and bridge-fact questions remain open; [snapshot-view-reconciliation](snapshot-view-reconciliation.md) links the validation gates. This update does not rewrite the earlier log or mark an RFC accepted.
