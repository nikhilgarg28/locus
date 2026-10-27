# Issues

An issue records work that is not done yet: a question only the owner can answer, a piece of design work, a task, or a soundness problem. Issues live here, one file each, and change only through pull requests. So an issue raised by a PR is reviewed with that PR and lands with it, and it disappears if the PR is abandoned. GitHub's issue tracker is for reports from outside the project: an agent does not create, edit, comment on, or close a GitHub issue unless the owner asks, and records the issue here instead.

The [website](../site/README.md) lists every issue, with the open ones first, and shows the open issues about a spec rule next to the rule.

## Layout

```text
issues/
  runtime-semantics.md
  proof-construction.md
```

A file is `<name>.md`, where the name is lowercase, hyphenated words. The name is the issue's permanent identity: litmus examples link an issue by it, and it is never renamed.

## Format

```markdown
# Runtime semantics of checked code

- **Kind:** decision
- **Status:** open
- **Blocks:** the first build task, and RFC 0002's effect summaries
- **Rules:** `logic.boundary.calls`, `logic.boundary.writes`

## Question

...

## Log

### 2026-10-27, PR #10

Raised while reviewing RFC 0002.
```

The title comes first, then the header lines:

| Line | Required | Meaning |
|---|---|---|
| `Kind` | Yes | What closes it, from the table below |
| `Status` | Yes | `open`; `later`, recorded so that it is not forgotten and not scheduled; or `closed` |
| `Blocks` | No | The work or decisions waiting on it |
| `Rules` | No | The spec rules it is about, in backticks. `cargo xtask context` lists the issue under each of them. |
| `Closed by` | When closed | What closed it, as [below](#closing-an-issue) |

| Kind | Meaning | Body |
|---|---|---|
| `decision` | A question only the owner can answer, because the spec and accepted RFCs do not | Question, What it blocks, Options, and an Example if one helps |
| `design` | Design work: an RFC, spec rules, litmus examples, or a spike | Scope, and the questions it must answer |
| `task` | Implementation or tooling work, written as a contract | Goal, Authorized by, Allowed scope, Required behavior, Forbidden, Verification, Stop if |
| `soundness` | Something accepted that should not be, or a hole in [the trust model](../design/TRUST.md) | What is accepted, Why it should not be, Reproducer, Impact |

The body's sections are a guide, not checked. The last section is always `## Log`.

## The log

The log is the issue's history: what was found, asked, answered, and decided, and where. Each entry starts with `### YYYY-MM-DD, <where>`, where `<where>` names the PR, review, or conversation it came from. A comment someone made is quoted with `>`.

A PR may add entries to the end of the log. It never changes or removes what the log already says: the guard treats that, like deleting or renaming an issue, as a change to the record, which needs [the `expectation-change` label](../AGENTS.md#rules). Everything above the log may be edited, so that the question, its options, and what it blocks stay current; the log keeps what they used to say.

## Closing an issue

A closed issue says what closed it on its `Closed by` line, and its log says how.

- **A decision closes only when the owner decides it.** Its `Closed by` line names the accepted RFC that decided it, as `RFC 0003`. A question that changes no semantics, such as one about process, may instead be closed by `the owner`, with the owner's words quoted in the last log entry. Setting an RFC to accepted, like recording any decision, happens only when the owner has decided and asked for it.
- **Other issues** name the PR, RFC, or issue that closed them, or say why they were dropped. A duplicate is closed by the issue it duplicates.

An `.open` litmus example links the decision that will decide it. Closing that decision means deciding the example in the same PR, as accept or reject, or linking it to the decision that still will.

`cargo xtask check` checks each issue's header and log, that its rules exist, that what closed it exists, that a closed decision names an accepted RFC or quotes the owner, and that each example links an issue that exists.
