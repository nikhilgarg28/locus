# RFC 0000: Title

- **Status:** draft
- **Issue:** #
- **Supersedes:** none

## Summary

One paragraph: what changes, and for whom.

## Motivation

The problem, shown with a program a user would write. What cannot be said or checked today, and why it matters to the [vision](../../VISION.md).

## Design

Examples first, then the precise meaning. Say what is accepted, what is rejected, and what each accepted program does when it runs.

## Rules

The spec rules this RFC adds or changes, each with its ID and the litmus examples that test it. Rules about what is allowed need an accepted and a rejected example on either side of the line. Every rule the RFC adds must appear here: `cargo xtask check` fails on a spec rule that no RFC lists, and on a row whose rule or examples do not exist.

| Rule | Examples |
|---|---|
| `topic.rule-name` | `litmus/topic/name.accept.lc`, `litmus/topic/near-miss.reject.lc` |

## Interactions

How the design affects each part of the language. Write "none" only after checking.

| Area | Effect |
|---|---|
| Types | |
| Propositions and proofs | |
| Ownership and mutation | |
| Erasure and runtime behavior | |
| Rust interoperability | |
| Diagnostics | |
| Trust model | |

## Alternatives

Each alternative considered, and why it lost.

## Prior art

How other languages and systems handle this, with references.

## Unresolved questions

Each one a [decision issue](../../issues/README.md), with an `.open` litmus example where one helps.
