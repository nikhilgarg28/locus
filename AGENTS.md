# Working on Locus

Locus is a superset of Rust with explicit, kernel-checked proofs that are erased before the program runs. It is being designed in the open, starting from a vision and a small set of decisions, before most of the compiler exists. This file is the map, the rules, and the commands. Read it in full; read other files only as your task needs them.

## The map

| Path | What it holds | Authority |
|---|---|---|
| [VISION.md](VISION.md) | Goals, non-goals, the experience Locus is for | The owner's. Agents may propose edits; the owner decides. |
| [spec/](spec/README.md) | The accepted semantics, as rules with IDs | Normative |
| [design/rfcs/](design/rfcs/README.md) | Proposals and the reasons for each decision | Binding once accepted |
| [design/TRUST.md](design/TRUST.md) | What must be trusted for a checked program to mean what it claims | Normative |
| [litmus/](litmus/README.md) | Example programs that must be accepted, must be rejected, or are open questions | Decided outcomes are normative |
| [diagnostics/](diagnostics/README.md) | One page per error code | Describes the compiler |
| `crates/` | The compiler. Created by the first implementation RFC. | Implements the spec |
| `spikes/` | Throwaway experiments. Nothing may depend on them. | None |
| [xtask/](xtask/src/main.rs) | Repository checks and the site builder | Tooling |
| [site/](site/README.md) | Website configuration and landing page | Presentation |

Each directory's README explains its format. Before editing files in a directory, read its README.

## Read only what the task needs

Read this file, the task (its issue or the request), and then only what the task touches: the spec chapters and rules it cites, the RFCs those rules came from, the litmus examples for them, and the README of each directory you edit. Do not read the whole repository to get started.

Start from `cargo xtask context <rule-id>` for each rule the task cites. It prints the rule, the RFC that introduced it and whether that RFC is accepted, the examples on either side of it, the open examples that cite it, and the trust entries its RFC added.

## Every PR is one of three kinds

- **Design**: RFCs, spec rules, litmus examples, the vision, and the trust model. No compiler code.
- **Build**: compiler code and tests that implement accepted spec rules. Never changes the spec, the RFCs, the vision, or the trust model.
- **Review**: no commits. Comments on a PR, or new issues.

Tooling, documentation, and process changes that change no semantics are ordinary PRs. `cargo xtask pr-check` rejects a branch that changes both the design (`VISION.md`, `spec/`, `design/`) and `crates/`.

## Rules

1. **Only the owner decides what Locus means.** When you meet a question the spec and accepted RFCs do not answer, do not choose the convenient answer. Open a `decision-needed` issue that states the question, what it blocks, and the options, add an `.open` litmus example if one helps, and stop that part of the work. Do the same when a spec rule, its RFC, and a decided example disagree: none of them wins ([spec/README.md](spec/README.md#when-sources-disagree)).
2. **An RFC binds only once accepted.** Write new RFCs as `proposed`. Set an RFC to `accepted` or `rejected` only when the owner has decided and asked you to record it. The owner's merge is the acceptance.
3. **Do not weaken decided expectations.** Do not change a decided litmus outcome, remove or change an expected error code or run result, delete a decided example, or remove a spec rule unless the owner asked for that change. Such a PR must carry the `expectation-change` label and justify each change in its description; CI rejects it otherwise.
4. **Every spec rule has examples.** A rule (`r[id]`) needs at least one decided litmus example citing it, and a rule about what is allowed should have an accepted and a rejected example on either side of its line. CI enforces the first and warns about the second.
5. **No new trust without an RFC.** Do not add axioms, trusted declarations, unchecked shortcuts, or anything else that extends [the trust model](design/TRUST.md) unless an accepted RFC names it there.
6. **Do not merge, and do not push to `main`.** Open a PR from a branch; the owner merges.
7. **Report what actually happened.** Say which commands ran and what they printed, what failed, and what you skipped. Never invent results, measurements, or test counts.
8. **Spikes stay spikes.** Code in `spikes/` answers one question and may be deleted at any time. Nothing outside `spikes/` may depend on it; to keep something from a spike, rewrite it under a task.

## Commands

```sh
cargo xtask check                          # every repository check; run before each PR
cargo xtask context <rule-id>              # one rule, with its RFC, status, examples, and trust entries
cargo xtask pr-check --base origin/main    # the PR guards that CI runs
cargo xtask site                           # build the website into target/site
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`pr-check` compares committed work with its base, so commit before running it.

## Issues and pull requests

Work is tracked in GitHub issues. Labels:

| Label | Meaning |
|---|---|
| `decision-needed` | A question only the owner can answer; say what it blocks |
| `design` | Design work: an RFC, spec rules, or litmus examples |
| `task` | Implementation work, written as a contract ([template](.github/ISSUE_TEMPLATE/task.md)) |
| `soundness` | Something accepted that should not be, or a hole in the trust model |
| `expectation-change` | A PR that changes or removes decided expectations, with the owner's agreement |
| `later` | Recorded so that it is not forgotten; not scheduled |

PRs follow the [template](.github/pull_request_template.md): their kind, what authorizes them, expectation and trust changes, and the verification that ran.

## Playbooks

Step-by-step instructions for recurring work live in [.claude/skills/](.claude/skills/). They are plain Markdown and apply to any agent:

- [rfc](.claude/skills/rfc/SKILL.md): draft or revise an RFC
- [litmus](.claude/skills/litmus/SKILL.md): write examples, and probe a rule with adversarial variants
- [implement](.claude/skills/implement/SKILL.md): implement a task against accepted rules
- [review](.claude/skills/review/SKILL.md): review a PR adversarially
- [decision-brief](.claude/skills/decision-brief/SKILL.md): prepare the owner's open decisions
