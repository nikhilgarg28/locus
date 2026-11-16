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

### 2026-11-16, proposal made at the owner's request

The owner asked for an audit of the decisions that are still open, with a proposal for each. This is the proposal for this issue. Nothing in it is decided.

- **What runs behind `_` by default:** a fixed, bounded search with no solver and no randomness: the proofs in scope, the conditions of the branches around it, the definitions of locals, rewriting with `===` proofs in scope, and a decision procedure for linear arithmetic. The same program gives the same answer on any machine (vision, commitment 3). More is asked for by name ([proof-construction](proof-construction.md)).
- **Where found proofs live:** in one file per crate beside `Cargo.lock`, committed, and read by every check. A check never searches when the file has a proof for a hole; a separate command searches and updates the file, and continuous integration runs with the file locked, as it does with `Cargo.lock`.
- **How a stored proof finds its hole after an edit:** by the goal after normalization together with the hypotheses in scope, and a fingerprint of the definitions it uses, as the external review suggested. A proof whose key no longer matches is searched for again, never reused.
- **When search fails,** the report is the one [proof-debugging](proof-debugging.md) proposes.
- **The strongest argument against a committed file** is noise in reviews. The answer is that it is generated, like a lock file, and that without it a check depends on search succeeding again.
