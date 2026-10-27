# Proof construction (RFC 0003)

- **Kind:** decision
- **Status:** open
- **Blocks:** Writing nontrivial proofs, the format of stored proofs ([#13](proof-search-and-storage.md)), and the kernel RFC's term language

## Question

How proofs are built, beyond the forms [RFC 0002](https://github.com/nikhilgarg28/locus/blob/claude/beautiful-mccarthy-cmtyao/design/rfcs/0002-logical-layer.md) fixes. Those forms are holes (`_` and `prove!`), calls of lemmas, the constructors and matches of `prop` declarations, and the connectives' constructors.

The RFC would cover:
- **Using definitions.** Unfolding and folding logic functions, which RFC 0002 used to spell `fold!` and `unfold!`, and which computation the kernel performs on its own.
- **Rules:** `proofs.elimination`
- **Equalities.** Rewriting a proof along an equality, which RFC 0002 used to spell `rewrite!`. Also how `===` is used, and how the built-in agreement of `==` with `===` is applied: by search, or by bridges the elaborator inserts.
- **Tactics.** How much of Lean's tactic language Locus adopts, if any, as against proof terms plus search.
- **Search.** The interface between a hole and untrusted search: the goal and context a hole gives, what search may return, and how found proofs are stored ([#13](proof-search-and-storage.md)).
- **Shorter forms.** For example, an anonymous constructor for one-arm propositions, `(hp, hq)` for `And::Intro(hp, hq)`. The owner deferred this as polish.
- **Libraries of lemmas.** How they are organized and found.

## What it blocks

Writing nontrivial proofs, the format of stored proofs ([#13](proof-search-and-storage.md)), and the kernel RFC's term language.

## Options

To be laid out in the RFC. Lean's split between proof terms and tactics is the main reference point.

## Log

### 2026-10-26, opened on GitHub as #15

Moved into the repository from [GitHub issue #15](https://github.com/nikhilgarg28/locus/issues/15) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.
