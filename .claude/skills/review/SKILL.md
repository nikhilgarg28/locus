---
name: review
description: Review a Locus PR adversarially, from the diff and the spec alone. Use when asked to review a PR, an RFC, or a change, or to check that a change is sound.
---

# Reviewing a PR

You are the check that the author is not. Review from the diff, the spec, and the accepted RFCs, not from the author's explanation of what they meant. Do not commit; leave review comments on the PR, and open issues for anything outside its scope. Follow [AGENTS.md](../../../AGENTS.md) throughout.

## For every PR

1. **Authorization.** Does an accepted RFC, a spec rule, or an issue authorize the change? Is it one kind of PR (design or build), not both?
2. **Expectations.** Does it change or remove any decided litmus outcome, mode, program, expected error, run result, or spec rule? If so, is it labeled `expectation-change`, and is each change justified? `cargo xtask pr-check` lists what it finds, but read the diff too.
3. **Trust.** Does it add an axiom, a trusted declaration, an unchecked path, or a dependency the trust model does not list?
4. **Evidence.** Do the cited examples actually exercise the rules they cite? A citation on an example that would pass anyway is not coverage.

## For a design PR

- Look for contradictions with the vision, with accepted RFCs, and within the RFC itself.
- Try to break each new rule: write variants with the [litmus playbook](../litmus/SKILL.md), and look for a program the rules accept that should be rejected, or the reverse.
- Check the interactions table against the language as it is. Name interactions it misses.
- Say what is unspecified: questions the RFC answers only implicitly, or not at all.

## For a build PR

- Check the behavior against the cited rules, not against the PR's description.
- Look for cases the tests miss, especially rejected programs that the change might now accept.
- Check that the commands in the PR were run and their results reported.

## Reporting

List findings most serious first. For each, give the evidence (a program, a line of the diff, a rule) and what would resolve it. Say plainly when you found nothing in an area you checked.
