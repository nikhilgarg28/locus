# Core surface grammar

This is the grammar implemented by the syntax frontend. It follows section 13 of the [core language specification](core-language-spec.md). Names, types, propositions, modes, and proofs still require the semantic checking that comes after parsing. `bool`, `u8`, and `Prop` are parsed as type names, not lexer keywords.

[rust-features.md](rust-features.md) lists the Rust features that the fragment does not support.

## Relationship to Rust tokens

Locus source is meant to tokenize as Rust, so that the same frontend can later run inside a `locus! { ... }` procedural macro as well as on `.loc` files. Every token here is a Rust token. Two places need care in that setting and none in the file-based frontend: Rust reads `pair.0.1` as a name, a dot, and the float `0.1`, which a macro driver has to split; and `=>` and `..` arrive as single punctuation tokens in both.

The words `struct`, `enum`, `match`, `loop`, `for`, `in`, `break`, and `continue` are reserved, as they are in Rust. `forall` and `exists` are reserved by Locus. `math` and `prop` are ordinary identifiers except where a declaration or a function type can begin: `math` directly before `fn`, and `prop` directly before a name at the start of a declaration.

## Retired syntax

The parser recognizes four retired forms in order to say what replaced them.

| Written | Diagnostic | Replacement |
|---|---|---|
| `def name(...)` | L0113, with an applicable fix; parsing continues as a `math fn` | `math fn name(...)` |
| `@{ command; ... }` | L0110 | Evidence is an ordinary expression: a stated intermediate fact, a lemma call, a `match`, or `_` |
| `a + b` | L0112 | `a.wrapping_add(b)`; arithmetic operators return with `Int` and `Nat` in a later milestone |
| `[]`, `[a, b]`, `[a; n]`, `[T]`, `[T; n]` | L0114 | None in the core; brackets hold exactly one proposition |

`Nat` is an ordinary name to the parser. Name resolution reports it.

## Notation

In this EBNF, brackets denote optional syntax and braces denote repetition. Quoted brackets and braces are source tokens. Comma-separated lists allow a trailing comma.

```ebnf
file        = { declaration } ;
declaration = [ "math" ] "fn" name "(" parameters ")" "->" type block
            | "struct" name "{" parameters "}"
            | "enum" name "{" [ variant { "," variant } [ "," ] ] "}"
            | "prop" name [ "(" parameters ")" ] "{" [ prop_variant { "," prop_variant } [ "," ] ] "}"
            | "const" name ":" type "=" expression ";" ;
parameters  = [ parameter { "," parameter } [ "," ] ] ;
parameter   = name ":" type ;
variant     = name [ "(" fields ")" ] ;
prop_variant = name [ "(" fields ")" ] [ ":" "@" proof_target ] ;
fields      = [ field { "," field } [ "," ] ] ;
field       = [ name ":" ] type ;

type        = name | "(" ")" | "(" type ")" | tuple_type
            | "@" proof_target
            | [ "math" ] "fn" "(" fields ")" "->" type ;
proof_target = (name | proposition | "(" expression ")") { postfix } ;
postfix     = "(" arguments ")" | "." name | "." integer ;
tuple_type  = "(" field "," ")"
            | "(" field "," field { "," field } [ "," ] ")" ;

block       = "{" { statement } [ expression ] "}" ;
statement   = "let" pattern [ ":" type ] "=" expression ";"
            | expression ";" ;
pattern     = name | "_" | "true" | "false" | integer
            | "(" ")" | "(" pattern ")"
            | "(" pattern "," [ pattern { "," pattern } [ "," ] ] ")"
            | name "{" [ pattern_field { "," pattern_field } [ "," ] ] "}"
            | path [ "(" [ pattern { "," pattern } [ "," ] ] ")" ] ;
pattern_field = [ name ":" ] pattern ;
path        = name "::" name ;

expression  = atom | "!" expression | expression binary_operator expression
            | expression postfix ;
arguments   = [ expression { "," expression } [ "," ] ] ;
atom        = name | path | integer | "true" | "false" | "_" | block
            | "(" ")" | "(" expression ")" | tuple_expression
            | name "{" [ value_field { "," value_field } [ "," ] ] "}"
            | proposition
            | "if" expression block "else" (block | if_expression)
            | "match" expression "{" { arm } "}"
            | "loop" "(" state ")" "->" type block
            | "for" name "in" expression ".." expression "(" state ")" block
            | "break" expression
            | "continue" "(" arguments ")"
            | ("forall" | "exists") "(" nonempty_parameters ")" block ;
tuple_expression = "(" expression "," [ expression { "," expression } [ "," ] ] ")" ;
value_field = [ name ":" ] expression ;
arm         = pattern "=>" expression [ "," ] ;
state       = [ state_parameter { "," state_parameter } [ "," ] ] ;
state_parameter = name ":" type "=" expression ;
proposition = "[" expression "]" ;
```

`if_expression` has the `if` form above. `nonempty_parameters` follows `parameters` but requires at least one parameter. The comma after a match arm may be omitted only when the arm's body is a block, and after the last arm. Binary precedence, highest first: comparisons `== != < <= > >=`; `&&`; `||`; implication `=>`. Conjunction and disjunction associate left; implication associates right. Unparenthesized comparison chains are errors. Calls and projections bind tighter than prefix `!`.

The parser is more permissive than the language in three places, and the elaborator narrows them: a `let` pattern must be irrefutable; struct fields and patterns are checked against the declaration; and the body of a quantifier is a block whose only meaningful content is its final formula.

## Headers and blocks

Three conventions keep a `{` unambiguous. They are parsing conventions and do not change typing.

- In the condition of an `if`, the scrutinee of a `match`, and the bounds of a `for`, `name {` begins the following block and is not a struct literal. This is Rust's rule. Parentheses, brackets, call arguments, and nested blocks lift the restriction: `match (Unit {}) { ... }`.
- The proposition of a proof type follows the same rule, because a proof type is usually followed by a body: in `-> @claim { given }` the braces are the function body.
- In the bounds of a `for`, the parenthesized group directly before the body is the state list, not a call on the upper bound. `for i in 0..limit(a) (acc: u8 = 0) { ... }` calls `limit` and then lists the state. A `for` with no state writes `()`.

`=>` separates a match arm's pattern from its body and is also implication. A pattern never contains an expression and there are no guards, so the first `=>` after a pattern is the separator and any later one belongs to the body.

## Executable and math functions

`fn` and `math fn` share parameter, result, and body grammar; the AST records `FunctionMode::Runtime` or `FunctionMode::Math`. The semantic rules, from the specification, are:

- `fn` accepts/returns executable data and erased proofs, including data/proof tuples. It may diverge, and is checked for partial correctness. Proposition and proof values in its signature are ghost: they are erased and cannot determine executable data or control flow. A proof type such as `@[same(x, y)]` can mention propositions without passing a proposition object at runtime.
- `math fn` is a pure, total function. It can accept/return propositions, proofs, and data. It is callable from logic, and also from executable code; it is compiled when its signature contains executable data and erased entirely when its signature is wholly ghost.
- Local `Prop` bindings and logical calls are allowed in `fn` bodies as ghost bindings. A ghost value cannot determine executable data, runtime branching, or runtime layout. Proofs can discharge statically checked obligations.
- Unknown runtime inputs can be used symbolically in logic. Compile-time resolution means checking the expression and its dependencies, not knowing every input value or deciding every proposition.
- A runtime `fn` cannot be used in a proposition. A function used in logic must be a `math fn`, whose totality is guaranteed syntactically: no general loops and, in the core, no recursion.

The syntax parser does not enforce these semantic restrictions. Both declaration kinds currently require explicit parameter/result types, and nested declarations are outside the initial grammar.

## Propositions and bracket expressions

Propositions are erased logical values of type `Prop`, not executable Booleans. Logical expressions may refer symbolically to unknown runtime inputs. Their meaning and well-formedness are checked at compile time; neither those inputs nor the truth of every claim must be known at compile time. Merely defining a proposition does not establish it.

A proposition literal uses brackets, even when its annotation already says `Prop`:

```rust
math fn positive(n: u8) -> Prop {
    [n > 0]
}

const equality_is_reflexive: Prop = [
    forall (n: u8) {
        n == n
    }
];
```

The intended elaboration rules are:

- The core has no arrays, so `[e]` always denotes a proposition literal, with or without an expected type: `let claim = [n != 0];` binds a `Prop`. How brackets are shared with arrays once arrays exist is an open question in the specification.
- `[]`, `[e,]`, `[a, b]`, and `[e; count]` are rejected. A multiline proposition is one formula, not a comma-separated list of claims.
- `let claim: Prop = n > 0;` is a type error: there is no implicit Boolean-to-proposition conversion. Write `let claim: Prop = [n > 0];`.
- Existing proposition values do not need wrapping: `let another: Prop = claim;` and a call returning `Prop` are valid.
- For `p: Prop` and `q: Prop`, `!p`, `p && q`, `p || q`, and `p => q` construct new propositions directly, without extra brackets. They do not prove, decide, or branch on those claims. Boolean `!`, `&&`, and `||` retain their executable meanings; Boolean conjunction/disjunction retain short-circuit behavior. Implication is a logical operation. Mixed Boolean/proposition operands require an explicit logical formula.
- Logical literals interpret comparisons and connectives logically and permit quantification. They do not execute arbitrary code or make effectful/nonterminating calls valid in logic.
- A local proposition refers to logical snapshots at its definition; later mutation will not change what that proposition means.

The parser records a bracket expression as `ExprKind::Proposition`. Enforcing the rules above is the elaborator's work.

## Proof types and construction

`@claim` denotes evidence for a named proposition. `@[condition]` applies the same type constructor to a proposition literal. Calls to proposition-returning `math fn` declarations can be used directly, such as `@same(x, y)`, or within literals, such as `@[same(x, y)]`. Calls and projections belong to the proof target; ungrouped binary operators do not. The target must have type `Prop` after elaboration.

An expression `_` requests checked evidence for its expected proposition. It cannot assume a claim, stand for unspecified runtime data, or select an arbitrary proposition when no goal is known. Unsolved goals are compilation errors. `_` in a binding/destructuring pattern continues to mean ignoring a value; this is a different AST form.

A proof type (`@claim`, `@[condition]`) is not also a proof-producing expression, and `@` never begins an expression. Use an annotation to state an explicit goal, such as `let evidence: @[n == n] = _;`.

Evidence is built with ordinary expressions. A declared proposition is proved by applying one of its constructors, `Small::Below(bound)`, and used by `match`. `rewrite`, `unfold`, and `fold` use call syntax with reserved names and need no grammar of their own. Hash-based proof forms are no longer accepted. `#[...]` and `#![...]` remain reserved for future attributes and currently receive an unsupported-feature diagnostic.

## Scope, lexical rules, and recovery

The final expression supplies a block's result. Every preceding expression statement requires `;`, including an `if` or block expression. Early `return` is outside this initial grammar. `(x)` is grouping, `(x,)` is a tuple, and `(out: u8,)` is a named one-field tuple type. Named result fields in either kind of function will bind only in subsequent fields; parameter names and local bindings have lexical scope. Constants require an explicit type and are currently top-level declarations; local names use `let`.

Identifiers currently use ASCII letters/digits and underscores and cannot begin with a digit. Proposition names follow a lowercase snake_case convention, not a capitalization rule. Integers are decimal digit sequences with optional single underscores between digits; their text is retained without a machine-integer conversion. Numeric suffixes, strings, and raw identifiers are not supported yet. Whitespace, `//` comments, and nested `/* ... */` comments are accepted. Spans use UTF-8 byte offsets, including on invalid Unicode input.

Parsing has bounded nesting (64 levels) and expression-chain length (128). Recovery can retain a partial AST for diagnostics; a file with any lexical or syntax error is never reported as successfully parsed. Conversely, syntax success does not establish semantic validity: the parser still accepts false proof goals, unbracketed Booleans in `Prop` contexts, misplaced proof holes, refutable `let` patterns, and a `break` outside a loop, for subsequent checking to reject.
