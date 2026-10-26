---
name: litmus
description: Write Locus litmus examples, or probe a rule or design with adversarial variants and classify each as accept, reject, or open. Use when asked for examples, edge cases, counterexamples, or to stress a design.
---

# Writing litmus examples

A litmus example asks one question with a small program, and its file name gives the answer. The format is in [litmus/README.md](../../../litmus/README.md). Follow [AGENTS.md](../../../AGENTS.md) throughout.

## One example

1. Choose the topic directory, or create one for a new idea. Name the file after the question, not the answer: `stale-after-mutation`, not `bad-proof`.
2. Write the smallest program that asks the question. Remove everything that does not bear on it.
3. Header: one `//~ question:`, the `//~ spec:` rules it tests, `//~ rfc:` if an RFC introduced it, and `//~ mode: export` if it is about export mode. `cargo xtask context <rule-id>` shows the examples a rule already has.
4. For `reject`, mark each expected error on its line. For `accept`, add `//~ run:` lines when the run-time behavior is part of the question.
5. Classify it from the spec and accepted RFCs only. If they do not decide it, it is `open`: open a `decision-needed` issue and link it with `//~ issue:`. Never pick an outcome because it is convenient or because a draft implementation does it.

## Probing a boundary

Given a rule, an RFC, or a seed example, write variants that try to break it. For each variant, change one thing, and try in particular:

- mutation after evidence is formed, and through aliases, references, and loops
- moves, partial moves, and values used after a panic is caught
- overflow, division, and edge values of each integer type
- evidence carried in data (`Option`, structs, collections) and erased at run time
- generics, traits, and code reached only through a call
- recursion and non-termination
- calls into Rust and values coming back from it
- anything that would make a false claim checkable, or change run-time behavior through erased data

Report the variants in a table (variant, outcome, the rule that decides it or the issue that will), and add the decided and open ones as files. A variant the spec accepts but that looks wrong is a `soundness` issue, not an example to quietly classify.
