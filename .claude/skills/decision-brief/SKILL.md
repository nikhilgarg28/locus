---
name: decision-brief
description: Prepare a brief of the owner's open Locus decisions, ordered by what each blocks. Use when the owner asks what needs deciding, or before a design session.
---

# Preparing a decision brief

The owner's time is the scarcest resource in the project. A brief turns open questions into a few decisions that can each be made in minutes. Follow [AGENTS.md](../../../AGENTS.md) throughout.

## Gathering

1. Collect the `decision` issues in `issues/` that are open or for later, `proposed` RFCs, and `.open` litmus examples.
2. Merge duplicates, and note which questions depend on others.
3. Order them by what they block: work waiting on the answer, and other decisions that depend on it.

## Writing

For each decision, in order:

- **Question**: one sentence.
- **Why now**: what it blocks.
- **Options**: two to four, each with its main consequence, and with the program that tells them apart (a litmus example, if there is one).
- **Recommendation**: which option, and the strongest argument against it.
- **Links**: the issue, the RFC, the examples.

Keep the brief to what fits in one sitting. Put minor or blocked decisions in a short list at the end. Do not decide anything yourself, and do not present a recommendation as settled.
