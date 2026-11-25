# Implement named invariant-preserving acquisition

- **Kind:** task
- **Status:** open
- **Blocks:** Native collection element loans whose invariants read their storage
- **Rules:** `contracts.final`, `contracts.final.pending`, `contracts.invariants.acquisition`, `contracts.invariants.acquisition-unwind`, `contracts.invariants.observed`

## Goal

Implement RFC 0002 decision 125 after its design PR is accepted, with checked
certificates and exact Rust lowering. The owner has decided the rule; the compiler
and its proof of correspondence do not yet exist.

## Authorized by

The owner's request to lock I01 after the mutable-loan spike, recorded in RFC 0002,
decision 125. This task does not expand the supported semantics beyond that rule.

## Allowed scope

Named acquisition recognition, loan-identity association, frozen proof evidence,
proof-field suspension, preservation obligation generation, final-value typing
tokens and acquisition-only unwind protection in build and export.

## Required behavior

- Check the immutable binding's single runtime use on elaborated code. Proof-only
  final mentions do not count. Reborrows and output-parameter escape retain their
  actual loan identities and lifetimes; do not equate all endpoint values.
- Derive every affected enclosing invariant obligation from entry evidence and
  the returned contract. Never use an affected suspended proof field as evidence,
  including changed/future invariant evidence extracted before acquisition or
  indirectly through a snapshot, lemma or type. Keep suspension until both loan
  end and preservation discharge; call return without an escaping loan is not
  permission to reuse the old proof field.
- Identify the proof-only region immediately after acquisition and require a
  certificate before any runtime operation, drop, return, or other exit.
- Generate final-value tokens without reading a suspended place, keeping a source
  lifetime alive, changing type/trait resolution or exposing future data.
- Evaluate each argument exactly once in Rust's order; protect only the call's
  unwind boundary. Preserve cleanup order in both build programs and exports.
- Replay the sequence-preservation lemma and generated obligations in the kernel.
  An SMT success is not a Locus proof certificate.

## Forbidden

A general open-invariant region, pending final terms in independently stored data,
a trusted no-unwind guarantee, silent changes to runtime argument evaluation,
accepting a missing restoration certificate, or claiming the wrapped collection's
separate ghost-state restoration has been implemented by this work.

## Verification

Use the decided `litmus/contracts/acquisition-*` and `named-acquisition` cases,
`spikes/mutable-loans`, and the crate-shaped `studies/mutable-loans` workload.
The spike has Rust/ASan/Miri and hand-translated SMT results; none establishes
compiler acceptance. Add actual compiler/kernel tests as the implementation exists.

## Stop if

A new semantic choice is needed or a generated program differs from the adopted
call boundary. Record the choice rather than weakening an accepted expectation.

## Log

### 2026-11-24, owner decision after the mutable-loan spike

The owner approved the named-binding design and requested a commit and PR. These
are the remaining implementation/validation gates, not a request to re-decide I01.
