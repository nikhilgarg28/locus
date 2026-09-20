# Core surface grammar

This is the grammar implemented by the syntax frontend. Names, types, propositions, effects, and proof commands still require subsequent semantic checking. `Nat`, `Bool`, `Prop`, and `u8` are parsed as type names, not lexer keywords. There is no separate `prop` declaration.

## Differences from the target specification

The frontend predates the current [core language specification](core-language-spec.md). This document describes what the parser accepts today. Batch 0.3b of [the core plan](core-plan.md) closes the gap. Until then:

| Implemented here | Specification |
|---|---|
| `def` declares a logical definition. | `math fn`: pure, total, and callable from both logic and executable code. `def` is retired. |
| `@{ command; ... }` proof blocks with tactic-like commands. | Retired. Proofs are ordinary expressions: stated intermediate facts, lemma calls, `match`, and `_`. |
| `Nat` and binary `+` in examples and tests. | The core has `u8` with `wrapping_add` and `wrapping_sub` only. `Int`/`Nat` and arithmetic operators are a later milestone. |
| No `struct`, `enum`, or `prop` declarations. | All three are in the core. |
| No `match`, `Enum::Variant` paths, or literal patterns. | In the core. `if` is defined as `match` on `bool`. |
| No `loop`, `break`, `continue(...)`, or bounded `for`. | In the core. |
| Member access `.name` only. | Positional projection `.0` as well. |
| No function types. | `fn(A) -> B` and `math fn(A) -> B`. |
| `forall (...) { ... }` parsed as an expression atom; no `exists`. | Both are formula forms. |
| Array and slice types and array literals are parsed; `[e]` is kept neutral. | Arrays are outside the core, and `[e]` is always a proposition literal. |

[rust-features.md](rust-features.md) lists the Rust features that the target fragment does not support.

## Notation

In this EBNF, brackets denote optional syntax and braces denote repetition. Quoted brackets/braces are source tokens. Comma-separated parameter and argument lists allow a trailing comma.

```ebnf
file        = { declaration } ;
declaration = ("fn" | "def") name "(" parameters ")" "->" type block
            | "const" name ":" type "=" expression ";" ;
parameters  = [ parameter { "," parameter } [ "," ] ] ;
parameter   = name ":" type ;

type        = name | "(" ")" | "(" type ")" | tuple_type
            | "@" proof_target
            | "[" type [ ";" expression ] "]" ;
proof_target = (name | bracket_expression | "(" expression ")") { postfix } ;
postfix     = "(" arguments ")" | "." name ;
tuple_type  = "(" field "," ")"
            | "(" field "," field { "," field } [ "," ] ")" ;
field       = [ name ":" ] type ;

block       = "{" { statement } [ expression ] "}" ;
statement   = "let" pattern [ ":" type ] "=" expression ";"
            | expression ";" ;
pattern     = name | "_" | "(" ")" | "(" pattern ")"
            | "(" pattern "," [ pattern { "," pattern } [ "," ] ] ")" ;

expression  = atom | prefix_expression | binary_expression
            | expression postfix ;
arguments   = [ expression { "," expression } [ "," ] ] ;
atom        = name | integer | "true" | "false" | block
            | "(" ")" | "(" expression ")" | tuple_expression
            | bracket_expression
            | "if" expression block "else" (block | if_expression)
            | "forall" "(" nonempty_parameters ")" block
            | "_" | proof_block ;
tuple_expression = "(" expression "," [ expression { "," expression } [ "," ] ] ")" ;
bracket_expression = "[" "]"
                   | "[" expression "]"
                   | "[" expression "," [ expression { "," expression } [ "," ] ] "]"
                   | "[" expression ";" expression "]" ;
proof_block = "@" "{" { command } "}" ;
command     = name { expression } ";" ;
```

`if_expression` has the `if` form above. `nonempty_parameters` follows `parameters` but requires at least one parameter. Prefix expressions use `!`. Binary precedence, highest first: `+`; comparisons `== != < <= > >=`; `&&`; `||`; implication `=>`. Addition, conjunction, and disjunction associate left; implication associates right. Unparenthesized comparison chains are errors. Calls and member access bind tighter than prefix `!`.

## Executable functions and logical definitions

`fn` and `def` share parameter, result, and body grammar; the AST records `FunctionMode::Runtime` or `FunctionMode::Logical`. `def` is the implemented spelling of what the specification now calls `math fn`. The intended semantic rules, as revised by the specification, are:

- `fn` accepts/returns executable data and erased proofs, including data/proof tuples. It may diverge, and is checked for partial correctness. Proposition and proof values in its signature are ghost: they are erased and cannot determine executable data or control flow. A proof type such as `@[same(x, y)]` can mention propositions without passing a proposition object at runtime.
- `def` (`math fn`) is a pure, total function. It can accept/return propositions, proofs, and data. It is callable from logic, and also from executable code; it is compiled when its signature contains executable data and erased entirely when its signature is wholly ghost.
- Local `Prop` bindings and logical calls are allowed in `fn` bodies as ghost bindings. A ghost value cannot determine executable data, runtime branching, or runtime layout. Proofs can discharge statically checked obligations.
- Unknown runtime inputs can be used symbolically in logic. Compile-time resolution means checking the expression and its dependencies, not knowing every input value or deciding every proposition.
- A runtime `fn` cannot be used in a proposition. A function used in logic must be a `math fn`, whose totality is guaranteed syntactically: no general loops and, in the core, no recursion.

The syntax parser does not enforce these semantic restrictions. Both declaration kinds currently require explicit parameter/result types, and nested declarations are outside the initial grammar.

## Propositions and bracket expressions

Propositions are erased logical values of type `Prop`, not executable Booleans. Logical expressions may refer symbolically to unknown runtime inputs. Their meaning and well-formedness are checked at compile time; neither those inputs nor the truth of every claim must be known at compile time. Merely defining a proposition does not establish it.

A proposition literal uses brackets, even when its annotation already says `Prop`:

```rust
def positive(n: Nat) -> Prop {
    [n > 0]
}

const equality_is_reflexive: Prop = [
    forall (n: Nat) {
        n == n
    }
];
```

The intended elaboration rules are:

- In the core fragment, which has no arrays, `[e]` always denotes a proposition literal, with or without an expected type: `let claim = [n != 0];` binds a `Prop`. The earlier rule, under which `[e]` defaulted to a singleton array unless a `Prop` was expected, is withdrawn. How brackets are shared with arrays once arrays exist is an open question in the specification.
- `[]`, `[e,]`, `[a, b]`, and `[e; count]` are exclusively array forms, parsed but outside the core. A multiline proposition is one formula, not a comma-separated list of claims.
- `let claim: Prop = n > 0;` is a type error: there is no implicit Boolean-to-proposition conversion. Write `let claim: Prop = [n > 0];`.
- Existing proposition values do not need wrapping: `let another: Prop = claim;` and a call returning `Prop` are valid.
- For `p: Prop` and `q: Prop`, `!p`, `p && q`, `p || q`, and `p => q` construct new propositions directly, without extra brackets. They do not prove, decide, or branch on those claims. Boolean `!`, `&&`, and `||` retain their executable meanings; Boolean conjunction/disjunction retain short-circuit behavior. Implication is a logical operation. Mixed Boolean/proposition operands require an explicit logical formula.
- Logical literals interpret comparisons and connectives logically and permit quantification. They do not execute arbitrary code or make effectful/nonterminating calls valid in logic.
- A local proposition refers to logical snapshots at its definition; later mutation will not change what that proposition means.

The parser deliberately retains a single-element bracket expression as `ExprKind::Bracket`, regardless of any visible annotation. Selecting its meaning and enforcing these rules require the later elaborator. Explicit array forms have separate AST nodes. Array and slice types (`[T; count]`, `[T]`) are preserved syntactically; runtime array support remains a later milestone.

## Proof types and construction

`@claim` denotes evidence for a named proposition. `@[condition]` applies the same type constructor to a proposition literal. Calls to proposition-returning `def` declarations can be used directly, such as `@same(x, y)`, or within literals, such as `@[same(x, y)]`. Calls and member access belong to the proof target; ungrouped binary operators do not. The target must have type `Prop` after elaboration.

An expression `_` requests checked evidence for its expected proposition. It cannot assume a claim, stand for unspecified runtime data, or select an arbitrary proposition when no goal is known. Unsolved goals are compilation errors. `_` in a binding/destructuring pattern continues to mean ignoring a value; this is a different AST form.

`@{ ... }` supplies explicit proof commands. This form is retired by the specification and will be removed in batch 0.3b; it is still parsed today. A proof type (`@claim`, `@[condition]`) is not also a proof-producing expression. Bare `@` is an error; use `_`. Use an annotation to state an explicit goal, such as `let evidence: @[n == n] = _;`.

Tactic names are ordinary identifiers. The parser neither executes nor validates commands. Member access supports syntax such as `x.wrapping_add(1)`; typing and execution follow in later batches. Hash-based proof forms are no longer accepted. `#[...]` and `#![...]` remain reserved for future attributes and currently receive an unsupported-feature diagnostic.

## Scope, lexical rules, and recovery

The final expression supplies a block's result. Every preceding expression statement requires `;`, including an `if` or block expression. Early `return` is outside this initial grammar. `(x)` is grouping, `(x,)` is a tuple, and `(out: Nat,)` is a named one-field tuple type. Named result fields in either declaration kind will bind only in subsequent fields; parameter names and local bindings have lexical scope. Constants require an explicit type and are currently top-level declarations; local names use `let`.

Identifiers currently use ASCII letters/digits and underscores and cannot begin with a digit. Proposition names follow a lowercase snake_case convention, not a capitalization rule. Integers are decimal digit sequences with optional single underscores between digits; their text is retained without a machine-integer conversion. Numeric suffixes, strings, and raw identifiers are not supported yet. Whitespace, `//` comments, and nested `/* ... */` comments are accepted. Spans use UTF-8 byte offsets, including on invalid Unicode input.

Parsing has bounded nesting and expression-chain depth. Recovery can retain a partial AST for diagnostics; a file with any lexical or syntax error is never reported as successfully parsed. Conversely, syntax success does not establish semantic validity: the parser still accepts false proof goals, unbracketed Booleans in `Prop` contexts, and misplaced proof holes, for subsequent checking to reject. Proposition-valued `fn` signatures are no longer among the things to reject: the specification allows them as ghost positions.
