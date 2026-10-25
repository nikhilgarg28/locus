---
name: implement
description: Implement a Locus task against accepted spec rules, within the task's scope. Use when asked to implement, build, or fix something in the compiler or tooling.
---

# Implementing a task

You build what the spec says; you do not decide what it says. Follow [AGENTS.md](../../../AGENTS.md) throughout.

## Before starting

1. Read the task issue. It names what authorizes the work (spec rules, an accepted RFC), the allowed scope, the required behavior, and when to stop. If it does not, ask for those before writing code.
2. Read the cited rules, the RFCs they came from, their litmus examples, and the README of each directory you will edit.
3. Check for open `decision-needed` issues that touch the same rules.

## Working

- Stay inside the allowed scope. Do not refactor or extend beyond it; record follow-up work as new issues.
- Test against the cited rules' litmus examples, and add focused tests for anything they do not cover. New tests may cite rules with `spec:` the way litmus examples do.
- **Stop if the spec does not answer a question you need answered, or if the accepted design does not work as written.** Open a `decision-needed` issue that shows the problem with a small program, and report it. Do not choose an interpretation or change the design to make the implementation work.
- Never change a decided expectation, weaken a test, or add an assumption to the trust model to get a passing build.

## Finishing

1. Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo xtask check`. Commit, then run `cargo xtask pr-check --base origin/main`.
2. Open a build PR using the template: what authorizes it, the expectation and trust changes (normally none), and the commands you ran with their results.
3. Report failures and skipped steps plainly.
