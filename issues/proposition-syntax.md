# Syntax of propositions and proofs

- **Kind:** decision
- **Status:** open
- **Blocks:** Every litmus example beyond RFC 0001's. Those examples use provisional spellings

## Question

The syntax inside propositions and proofs, within the rules of [RFC 0001](https://github.com/nikhilgarg28/locus/blob/main/design/rfcs/0001-compiler-pipeline.md) (Locus syntax only where compiled Rust rejects the tokens; Locus names resolved like std's prelude). This covers:
- proof types `@P`;
- named results `(out: T, @P)`;
- the proof hole `_`;
- what goes inside `logic!`, `prop!` and `prove!`: connectives, quantifiers, implication;
- `logic fn` and `prop` declarations with named arms.

## What it blocks

Every litmus example beyond RFC 0001's. Those examples use provisional spellings.

## Options

To be laid out in the RFC.

## Example

The observation examples in RFC 0001 use provisional proposition syntax such as `@(v.len() > 0)`.

## Log

### 2026-10-25, opened on GitHub as #5

Moved into the repository from [GitHub issue #5](https://github.com/nikhilgarg28/locus/issues/5) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-26, comment on GitHub

> RFC 0002 (`design/rfcs/0002-logical-layer.md`, in [the RFC PR](https://github.com/nikhilgarg28/locus/pull/10)) proposes most of the syntax this issue covers:
> - `@P` and `@(e)`;
> - connectives `&&`, `||`, `!`, and `==>`;
> - quantifiers `forall |x: T| P` and `exists |x: T| P`;
> - `prop` declarations with named arms, and the constructor `Arm(witnesses) @ proof`;
> - the proof forms `_`, `prove!`, `fold!`, `unfold!`, and `rewrite!`;
> - `#[decreases(e)]`;
> - `final!(r)`.
>
> This issue stays open for what RFC 0002 leaves to it, and for any respelling the owner wants.
