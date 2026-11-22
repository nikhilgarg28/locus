# Syntax of propositions and proofs

- **Kind:** decision
- **Status:** open
- **Blocks:** Every litmus example beyond RFC 0001's. Those examples use provisional spellings
- **Rules:** `proofs.connectives`, `compat.syntax.not-rust`

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

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.

### 2026-11-22, comparisons are propositions, and there is no `<==>`

RFC 0002's decisions 97 and 98 record two answers of the owner's that are about what is written inside a proposition. The owner's words are quoted there and in [traits-on-logical-types](traits-on-logical-types.md).

- `==`, `!=`, `<`, `<=`, `>`, and `>=` in logical code give a `Prop`, and a `Bool` is used as a proposition, with nothing written, at the positions where the form of the code asks for one. `&&`, `||`, and `!` are connectives on propositions, and operators on `Bool`s when every operand is a `Bool`. The rule `proofs.connectives` is reworded for this.
- There is no connective for "exactly when". Offered `<==>` or a function of the library, the owner chose the function: a contract writes `r === decide(P)`, or two implications.

The list of positions, and that `==` is a proposition as the orderings are, are RFC 0002's proposal, and are item 29 of what is still for the owner there.

### 2026-11-22, forms that the decisions on snapshots and models use

RFC 0002's decisions 99 to 108 write four forms that this issue may respell: the attribute `#[opaque]` on a field; `T::valid(s)`, the proposition that a snapshot is valid, as a logic function of the type; `T::model(s)`, a type's model applied to a snapshot value; and a snapshot value built with its type's own constructor, as in `Inner { n: 0 }`. The owner saw none of the four spellings one by one. They are in item 30 of what is still for the owner there.
