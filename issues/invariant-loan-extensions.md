# Later extensions to invariant-preserving loans

- **Kind:** decision
- **Status:** later
- **Rules:** `contracts.invariants.acquisition`, `contracts.invariants.acquisition-unwind`, `contracts.final.pending`

## Question

The owner adopted the narrow named-acquisition rule. Three possible extensions
remain deliberately unscheduled:

1. Pledges or another constrained mutation API, so a client can promise a final
   value preserving an invariant such as sortedness.
2. Unwind contracts, so a call may unwind while supplying enough evidence to
   restore the invariant instead of aborting.
3. A proved or trusted primitive no-unwind guarantee that permits omitting the
   acquisition guard. A normal-return contract is not that guarantee, and none
   is assumed by the current design.

No syntax, new trust entry, or proof rule for these is adopted. The independent
[no-panic claims](no-panic-claims.md) issue records broader effect questions.

## What it blocks

Only these extensions. Native initialized-prefix element loans can use the
adopted rule. A wrapped collection's separate ghost-sequence restoration remains
in [snapshot-view-reconciliation](snapshot-view-reconciliation.md).

## Options

Retain the current restriction; design checked pledges with their own loan-end
protocol; or design explicit exceptional-return/effect contracts. Compare their
unwind, cancellation, forget and multiple-loan semantics before choosing.

## Log

### 2026-11-24, owner decision after the mutable-loan spike

The owner approved recording pledges, unwind contracts and primitive no-unwind
guarantees as later work alongside RFC 0002 decision 125.
