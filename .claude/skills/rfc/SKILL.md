---
name: rfc
description: Draft or revise a Locus RFC, with its spec rules and litmus examples. Use when asked to propose a language change, write up a design question, or turn a decision into an RFC.
---

# Drafting an RFC

An RFC proposes what Locus should mean and why. The owner decides; your job is to make the decision easy to make well. Follow [AGENTS.md](../../../AGENTS.md) throughout.

## Before writing

1. Read [VISION.md](../../../VISION.md) and [the RFC process](../../../design/rfcs/README.md).
2. Find what the proposal touches: the spec chapters and rules, the accepted RFCs they came from, and their litmus examples. Search open issues for the same question.
3. If the proposal contradicts the vision or an accepted RFC, say so at the top of the RFC rather than working around it.

## Writing

1. Copy [the template](../../../design/rfcs/0000-template.md) to `design/rfcs/NNNN-slug.md`, using the next unused number. Set the status to `proposed` once it is complete; never to `accepted` unless the owner has decided and asked you to record it.
2. Start from programs a user would write. Show what is accepted, what is rejected, and what accepted programs do when they run, before stating the semantics precisely.
3. Fill in every row of the interactions table. "None" means you checked.
4. Give real alternatives, each with the reason it loses. If you favor one, say which and why, but present the others fairly.
5. Write the rules the RFC adds into the spec (`r[id]` markers; see [spec/README.md](../../../spec/README.md)), and add a litmus example for each one: for a rule about what is allowed, an accepted example and a nearly identical rejected one. Use the [litmus playbook](../litmus/SKILL.md) to probe each rule's boundary.
6. Turn each unresolved question into an `.open` litmus example or a `decision-needed` issue, and list it under "Unresolved questions".

## Finishing

1. Run `cargo xtask check`, commit, then run `cargo xtask pr-check --base origin/main`. Report what they printed.
2. Open a design PR using the template. List the decisions the owner needs to make, in order of what they block.
3. Ask for an adversarial review in a fresh session ([review playbook](../review/SKILL.md)), and fold in its findings or answer them.

If the owner asked for independent drafts, do not read the other drafts or their PRs before finishing your own.
