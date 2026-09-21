# The elaborator

The elaborator turns a parsed file into typed trees and hands each item to the `Session`, which lowers it, has the kernel check it, and erases it ([IR architecture](ir-architecture.md)). It lives in `src/elab/`. Nothing in it is trusted: a mistake here produces a program the kernel rejects, never an accepted wrong one.

It does four jobs that the plan lists separately: name resolution, type elaboration, filling each `_` with an explicit proof, and the diagnostics for all three. There is no separate resolved syntax tree. Names are resolved against a scope as expressions are elaborated, and declarations are ordered first by what they mention.

## Order of declarations

An item is checked when it is declared, so everything it mentions must already exist. `order.rs` reads dependencies off the syntax: every name an item writes that is also the name of an item. Items are elaborated dependencies first, whatever order the file lists them in. A cycle is an error (`L0203`), because the core has no recursion. A local that shadows an item's name counts as a mention of it, which can only add an edge.

When an item is rejected, the items that mention it are skipped without a second report.

## The mirrored context

Proofs in a typed tree refer to identities: the binder of each `let`, the fact of each branch and arm, the result of each call. The checker binds those identities as it walks the lowered program. While it works, the elaborator keeps a kernel context of its own with the same identities, bound in the same order, so that it can:

- ask the kernel what a term's type is, instead of recomputing it (a projection's type, a `let` binder's type);
- test every proof it finds with the kernel before using it, so that a failure is reported at the `_` and not as a rejection of the whole function.

The mirrored context must never hold more than the checker's will. Three rules keep it so. A branch, an arm, a loop body, and a quantifier end with a rollback. A block used as an expression keeps its declarations, because the checker splices its statements into the enclosing sequence, and forgets its names and facts. The term that stands for an expression's value is computed by `typed::value_term`, which is lowering's own rule without the lowering.

## Bidirectional elaboration

An expression is elaborated against the type expected of it when there is one. That is how `_` learns what to prove, how a tuple checked against `(out: u8, @[out == n])` learns that its second field speaks of its first, and how `Or::Left(h)` learns which disjunction it proves. Arguments, struct fields, variant payloads, loop state, and `continue` are all checked against a telescope: each value's term replaces its binder in the types that follow.

Inside `[ ... ]` and proof types, `logic.rs` builds kernel propositions directly: a comparison is a claim, and `&&`, `||`, `!`, `=>` are connectives. Outside, `a && b` on booleans is an `if`, which is also how the right operand comes to know the left one held.

Evidence of one claim is accepted where another is wanted when the solver can bridge them. This is what lets `still`, which is evidence about `step(...).0`, be passed where evidence about `next` is wanted after `let (next, still) = step(...)`.

## Filling a hole

The search is fixed and bounded. Every limit counts steps.

1. **A fact in scope.** Facts are: branch and arm evidence; the equation of each `let`; loop bounds; every evidence-typed name; and the evidence fields of every tuple and struct in scope, each stated about that value's own fields.
2. **The same after normalizing** the claim and every fact. Names are replaced by what they stand for, using the equations in scope whose left side is a variable or a field of one, read left to right. Projections of written tuples and structs, matches on written constructors, and arithmetic on literals are computed. Calls of the program's own math functions are unfolded; the prelude's orderings stay folded. Conjunctions among the facts are taken apart.
3. **Reflexivity; structure.** `&&` by proving both sides, `||` by proving one, `=>` by assuming the premise, `forall` by generalizing, `false` from a refuted fact whose claim can be shown. Depth at most 8.
4. **Closed evaluation.** A comparison of known values is run by the kernel.
5. **All 256 cases.** A comparison that speaks of exactly one unknown byte is decided by evaluating it at every byte, under the facts that speak of that byte alone. The kernel's `EvaluateAll` rule does the evaluation; the elaborator assembles the facts into the evaluated claim and discharges them afterwards. When a case fails, the failing byte is reported.

Ordering claims are compared in one form, as the outcome of the runtime test (`a < b` is `u8_lt(a, b) == true`), using the kernel's reflection axioms in both directions.

What this does not reach: a claim relating two unknown bytes that is not already a fact after normalizing, such as `x <= limit` and `limit <= 9` giving `x <= 9`. That step is a lemma call. The checked ordering lemmas are callable by name: `u8_le_refl`, `u8_zero_le`, `u8_le_trans`, `u8_lt_of_le_of_ne`, `u8_succ_le_of_lt`. Rewriting is oriented, from names to values; it is not a congruence closure.

Relative to specification section 12.3, tier 5 is an addition and the congruence closure of its tier 3 is not built. The specification has been updated to say so.

## When a hole cannot be filled

The report states, in source syntax:

- the claim, with written values put in their fields;
- the claim after computing, when that differs;
- a failing case, when evaluation found one: the byte, and that the known facts allow it;
- the facts that speak of a value the claim speaks of, normalized the same way, at most six.

~~~
error[L0230]: cannot show `within_limit(lock.failures.wrapping_add(1))`
  --> lock.loc:27:77
   = note: after computing, the claim is `lock.failures.wrapping_add(1) <= 3`
   = note: it fails when `lock.failures` is 3, which the facts known here allow
   = note: known here: `lock.failures <= 3`
~~~

A `let` whose value fails poisons the names it would have bound, so that their uses are not reported as unknown.

## Evidence written out

`proofs.rs` elaborates the explicit forms of specification section 8 into kernel proofs: proof constructors, with `And::Intro`, `Or::Left`, `Or::Right`, and `True::Intro` for the built-in connectives; `match` on evidence, where each arm receives its variant's index equations as facts; `match h {}` on evidence of `false`, at any type; `rewrite`, `unfold`, `fold`; evidence of a `forall` or an implication applied to an argument; and a `math fn` that returns evidence, named as evidence of its general claim.

## Measurements

`locus check file.loc --holes` lists every hole: where, which tier filled it, the size of the proof in nodes, and the time to find and check it once. `locus check file.loc --stats` lists, per function, the time to elaborate it, which includes the search, and the time to lower, check, and erase it.

For `examples/lock.loc` in a debug build: 8 proofs, about 800 proof nodes, about 8 ms elaborating, of which 6 ms is search, and about 5 ms checking. The largest proof is a 256-case one, at about 250 nodes; its size does not grow with the number of cases, because the kernel evaluates them. There is no cache of found proofs yet, so checking a file again after an edit costs the same as checking it the first time. Memory is not measured.

## Not yet elaborated

- Patterns beyond a name, `_`, and a tuple in `let`; beyond `Enum::Variant(names)` and `_` in `match`. Literal and struct patterns parse and are rejected here.
- `let And::Intro(a, b) = h;`. Use `match`.
- `exists`: formulas elaborate, and there is no source form yet for introducing or opening one.
- Values of `fn` type. A `math fn` type elaborates; no expression has one except a function named as evidence.
- Field access by name on a tuple with named fields. Use `.0` or `let`.
- A `for` with one state parameter yields a one-field tuple; the specification says it yields the value.
- In generated Rust, evidence that needed bridging prints as `Proved` and not as the name that was written, and `match` arms print in declaration order.
- In a `math fn`, a `for` whose body is not a sequence of `let`s ending in `continue` is rejected by lowering with an internal error, not with a message of its own.
