# Storing found proofs, and what proof search may do

- **Kind:** decision
- **Status:** open
- **Blocks:** Implementing holes, reproducible checking, and the manifest's certificate

## Question

RFC 0002 (`design/rfcs/0002-logical-layer.md`, proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10)) lets holes `_` and `prove!(P)` be filled by untrusted search. The kernel checks every result, and found proofs are stored with the crate (vision, commitment 3). RFC 0001 adds a **manifest** for each result: what it relied on, including its certificate. Open:

- **Storage.** The format, where it lives, and how it relates to each result's manifest.
- **Matching a stored proof to its hole after an edit.** The external review suggested matching by the hole's normalized goal and a fingerprint of what it depends on, rather than by source location alone, and invalidating a certificate when a dependency changes ([#18](external-review-ideas.md), item 4). Since the kernel re-checks every stored proof, this is a question of robustness and speed, not of trust. The identity must include the hypotheses in context.
- **Search:** which procedures run by default, how far they may go, and how failures are reported.
- **Proof language:** whether there are tactics beyond proof terms, and how solvers and AI propose candidates. This is shared with RFC 0003 ([#15](proof-construction.md)).
- **Determinism:** a crate checks the same way on any machine once its proofs are stored.

## What it blocks

Implementing holes, reproducible checking, and the manifest's certificate.

## Options

To be laid out in an RFC.

## Log

### 2026-10-25, opened on GitHub as #13

Moved into the repository from [GitHub issue #13](https://github.com/nikhilgarg28/locus/issues/13) on 2026-10-27. The text above is the issue's as it was then.
