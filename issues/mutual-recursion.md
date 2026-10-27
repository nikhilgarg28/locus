# Logic functions: mutual recursion

- **Kind:** design
- **Status:** later
- **Blocks:** Nothing in RFC 0002. It limits which logic functions can be written until it is decided

## Question

How logic functions, and logical types declared with `#[derive(Logical)]`, may be mutually recursive.

RFC 0002 (`design/rfcs/0002-logical-layer.md`, *Logic functions*) requires every recursive call to be on a structurally smaller part of a parameter, or to decrease a declared measure. It leaves mutual recursion for later, and `logic.fn.total` speaks only of a function calling itself.

Mutual recursion is common in practice:
- `even` and `odd`;
- predicates over a syntax tree whose expressions and statements contain each other;
- a pair of `prop` declarations that refer to each other.

## Options

- **Groups with a shared measure,** as Lean checks a mutual block. Every call within the group decreases one measure, or a lexicographic tuple of measures.
- **Structural recursion across a mutual group of types,** where each call is on a part of a parameter whose type belongs to the group.
- **Encode as one function** over a sum of the arguments, with the user writing the measure. This is simple for the kernel, but awkward to write.

Strict positivity (`logic.types.positive`) already covers groups of types and propositions; this issue concerns totality.

## What it blocks

Nothing in RFC 0002. It limits which logic functions can be written until it is decided.

## Log

### 2026-10-26, opened on GitHub as #16

Moved into the repository from [GitHub issue #16](https://github.com/nikhilgarg28/locus/issues/16) on 2026-10-27. The text above is the issue's as it was then.
