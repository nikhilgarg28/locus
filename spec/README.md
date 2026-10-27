# Specification

The specification defines Locus. Each rule arrives through an [RFC](../design/rfcs/README.md), and the rule, not the RFC, is the authority on what it says. Rationale and rejected alternatives stay in the RFCs.

On `main`, the specification contains only accepted semantics. A design PR adds the rules its RFC proposes here, so that they are checked and reviewed with their examples; they bind only once the RFC is accepted, which the owner's merge does.

## Rules

A rule is a paragraph preceded by a marker on its own line:

```markdown
r[erasure.option.discriminant-kept]
When evidence inside an `Option` is erased, the `Some`/`None` tag is kept,
so a match on it behaves the same at run time.
```

- A rule says one thing that could be false: an example program can show whether the compiler follows it. Text that no example could test is explanation and has no marker.
- A rule ID is a dotted name of lowercase, hyphenated words, with at least two parts. Name rules by what they are about, not by where they sit; an ID stays the same when its paragraph moves.
- IDs are permanent. A rule that is removed leaves its ID unused forever, and removing one is an [expectation change](../AGENTS.md#rules).
- Unmarked text is explanation, and needs no tests.

## Provenance and status

Every rule is listed in the `## Rules` table of the RFC that introduced it, and of each later RFC that changes it. `cargo xtask check` fails on a rule that no RFC lists, and, for an RFC still being decided, on a row naming a rule that does not exist or an example that does not cite it. A rule's status is its RFC's: accepted if an accepted RFC lists it, otherwise proposed or draft. The website shows each rule's RFC, and its status unless it is accepted.

`cargo xtask context <id>` prints what the repository says about a rule: its text, its RFC and status, its examples, the open examples that cite it, and the trust entries its RFC added. Given a prefix such as `contracts`, it prints every rule under it.

## When sources disagree

A rule is the authority on what it says; an RFC records why; a decided litmus example is evidence of what a rule means. When a rule and a decided example disagree, or a rule and its accepted RFC do, neither wins: the disagreement is a defect. Add a [decision issue](../issues/README.md) that names both, and stop the work that depends on it until the owner decides.

## Examples and tests

Every rule has at least one decided [litmus example](../litmus/README.md) that cites it with `//~ spec: <id>`. `cargo xtask check` fails on a rule without one, and on a citation of a rule that does not exist. A rule about what is allowed should have an accepted example and a nearly identical rejected one on either side of its line; the check warns when one side is missing. Once the compiler exists, its tests cite rules the same way, and a rule counts as implemented when its examples pass.

On the website, each rule links to the examples that cite it.

## Code in the specification

Code blocks in Locus are fenced as `rust`, for highlighting, followed by how they are checked:

| Fence | Meaning |
|---|---|
| `rust check` | A complete program that must be accepted |
| `rust reject` or `rust reject Lnnnn` | A complete program that must be rejected, with that error code once codes exist |
| `rust prose <reason>` | An illustration that is not checked, such as a fragment or generated Rust, with the reason |

Prefer a litmus example to a code block when the code is evidence for a rule.

## Chapters

There are no chapters yet. Each chapter is a file in this directory, listed here in reading order.
