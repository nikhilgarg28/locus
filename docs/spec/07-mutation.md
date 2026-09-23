+++
id = "language-mutation"
title = "Bindings, snapshots, and mutation"
group = "Now"
spec_chapter = 1
order = 106
route = "specification/mutation.html"
description = "How names, immutable observations, and tracked evidence behave as data changes."
+++

# Bindings, snapshots, and mutation

<!-- spec: 1.0:8 informative -->
A claim describes the value observed at a particular point in the program. Reassigning a variable creates a new version rather than changing that earlier claim. This chapter explains snapshots, branch and loop state, and tracked evidence that must be refreshed after mutation.

## What the compiler remembers

<!-- spec: 1.2:1 legality-rule -->
A source name is resolved to a binding identity, and every claim is written over identities, never over spellings. Shadowing makes a new identity, so a proposition formed before a `let` that reuses a name still speaks of the old value. A `let mut` binding has one identity and a sequence of versions, one more at each assignment; a claim speaks of the version current where it was formed, and the type of tracked evidence is read again at each use over the versions current there ([Mutation, versions, and tracked evidence](#mutation-versions-and-tracked-evidence)).

<!-- spec: 1.2:2 legality-rule -->
Items are elaborated in dependency order, whatever order the file lists them in: every name an item mentions that is also an item's name is an edge, and an unsupported cycle is an error. Logical functions admit the checked recursion described in [Logical data](08-logic.md#logical-data), and mutually referring logical enums are checked as one declaration group. Ordinary recursive calls remain rejected. Types and values are two namespaces, as in Rust: a struct, enum, or proposition may share its name with a function or a constant.

<!-- spec: 1.2:3 legality-rule -->
What is known at a point of a function is a context of typed bindings and facts: the parameters; each `let` with its defining equation, when its right side is a term of the logic; the fact of each branch and arm; the bounds of a `for` index; every binding of proof type; and the evidence fields of every struct and tuple in scope, stated about that value's own fields. A call to an ordinary function gives its result the declared type, including any evidence carried in that type. Its body is never unfolded, and the result has no defining equation derived from the body.

## Mutation, versions, and tracked evidence

<!-- spec: 1.12:1 dynamic-semantics -->
`let mut x = e;` declares a binding that may be assigned, whole, `x = e`, or by a field path, `x.f.g = e`, `t.0 = e`; a `mut` parameter is assigned the same way. Assignment is a statement and has no value. The checker sees no assignment: lowering gives the binding a new version at each one, an assignment to a field rebuilding the whole value around the new field, and every later mention means the current version. A field that evidence in the same struct depends on cannot be assigned alone, because the rebuilt value would carry evidence about the old field; the whole value is replaced.

<!-- spec: 1.12:2 dynamic-semantics -->
A branch some arm of which assigns a binding declared outside it joins: after it, each such binding is the version the arm taken produced, and nothing else is known of it beyond what evidence says; an arm that returns, breaks, continues, or panics contributes nothing to the join. A loop carries the bindings declared before it that its body, or the condition of a `while`, assigns, over binding identities: a field write counts for its root, a `let mut` inside the body is local, a shadowing name is another binding, and an assignment inside a nested block or branch counts. After a loop, what it carried is known only by its type and by the evidence carried with it.

<!-- spec: 1.12:3 dynamic-semantics -->
Evidence bound with `let` is a snapshot: it keeps its claim about the versions captured where it was written, whatever is assigned later, and `let p = prop!(x <= 3)` likewise captures the `x` of that point. Evidence bound with `let mut` is tracked: its declared type is read over the versions current wherever it is used, an assignment to a binding it mentions, or a `&mut` lend of one, leaves it stale, and a use of stale evidence is an error that names the assignment. `ok = _`, `ok = prove!(F)`, or `ok = still` establishes it again over the current versions. A branch keeps tracked evidence valid after the join when every arm that reaches the join left it valid; tracked evidence included in a loop state must be valid at entry, at every `continue`, and at the end of the body, and is valid after the loop. Evidence omitted from that state remains a fact only about its old snapshots; if it tracks data the loop changes, it is stale and must be re-established before a subsequent use. The flow analysis that decides validity is not trusted: lowering types every version, so stale evidence passed where the current claim is wanted is rejected by the checker whatever the analysis said. Logical type classification is independent of whether a binding is mutable.
