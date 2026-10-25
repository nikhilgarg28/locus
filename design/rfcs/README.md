# RFCs

An RFC proposes a change to what Locus means, and records the decision and its reasons. Accepted RFCs explain *why*; the [specification](../../spec/README.md) states *what*.

## What needs an RFC

Anything that changes what a program means or whether it is accepted: syntax, typing, propositions and proofs, ownership and mutation, erasure, Rust interoperability, and [the trust model](../TRUST.md). Changes to the [vision](../../VISION.md) and to this process also go through RFCs.

Diagnostic wording, refactoring, performance, and tooling that change no semantics do not need one.

## Statuses

| Status | Meaning |
|---|---|
| `draft` | Being written; not ready for a decision |
| `proposed` | Ready for the owner's decision; it has had an adversarial review |
| `accepted` | Decided. Its rules are in the spec, each with litmus examples. |
| `rejected` | Decided against. Kept for its reasoning. |
| `superseded` | Replaced by a later RFC, named on its `Superseded by` line |

## How an RFC lands

1. A design PR adds `design/rfcs/NNNN-slug.md`, copied from [the template](0000-template.md), with status `proposed`. Use the next unused number; if two PRs collide, the later one renumbers.
2. The same PR adds the spec rules the RFC introduces and the litmus examples that test them, written as the proposal would decide them. Questions the RFC leaves open become `.open` examples with `decision-needed` issues.
3. A fresh session reviews the PR adversarially ([playbook](../../.claude/skills/review/SKILL.md)), looking for contradictions, unsoundness, and interactions the RFC missed.
4. The owner decides. To accept, the status becomes `accepted` and the owner merges. To reject, the status becomes `rejected` and the PR is merged for the record (with its spec rules and examples removed) or closed.

For a foundational decision, the owner may ask for two independent drafts, each written without seeing the other, and a third session to compare them.

Once accepted, an RFC is not edited except to mark it superseded. A change of mind is a new RFC.
