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
A proposition describes values at a particular program point. Reassignment does not change a proposition already formed. When a proof should follow a mutable variable’s current value, declare it as tracked evidence and refresh it after updates.

## Names and snapshots

<!-- spec: 1.2:1 legality-rule -->
Each binding has its own identity. Shadowing creates another identity; assignment gives the same mutable binding a new version. A proposition captures the versions current when it is formed. Reusing a source spelling never retargets an old claim.

<!-- spec: 1.90:36 example -->
~~~rust check
fn snapshot() -> @True {
    let mut n: u8 = 3;
    let before = n as Int;
    let saved: @(before > 0) = _;
    n = 0;
    let still_about_before: @(before > 0) = saved;
    True::Intro
}
~~~

## What the compiler remembers

<!-- spec: 1.2:3 legality-rule -->
Available facts include typed parameters, justified let equations, branch and match facts, range bounds, proof bindings, and proof fields of aggregates. An ordinary call contributes only its declared result and evidence. Its implementation is not unfolded into a logical equation.

## Assignment

<!-- spec: 1.12:1 dynamic-semantics -->
`let mut` and mutable parameters permit assignment to the whole value or a field path. Assignment is a statement, not a value. Each write creates a new logical version; field writes rebuild the containing value. A field used by another field’s proof cannot be changed alone: replace the whole aggregate with fresh evidence.

<!-- spec: 1.90:37 example -->
~~~rust check
struct Limited { value: u8, valid: @(value <= 10) }
fn reset(item: &mut Limited) -> () {
    item = Limited { value: 0, valid: prove!(0 <= 10) };
    ()
}
~~~

## Mutation, versions, and tracked evidence

<!-- spec: 1.12:3 dynamic-semantics -->
An immutable proof keeps its snapshot. A `let mut` proof tracks the bindings named in its declared type. Assignment or mutable lending of a dependency makes it stale, even if the claim remains true. Assign fresh evidence to restore availability. After a branch, tracked evidence is available only if every reaching arm leaves it valid. The checker verifies the claim about the current versions independently of this availability analysis.

<!-- spec: 1.90:38 example -->
~~~rust run
fn refreshed() -> (n: u8, @(n <= 10)) {
    let mut n: u8 = 3;
    let mut bounded: @(n <= 10) = _;
    n = 4;             // bounded is unavailable now
    bounded = _;       // evidence about the new n
    (n, bounded)
}
//~ run: refreshed() => (4, Erased)
~~~

<!-- spec: 1.91:18 informative -->
An immutable `let claim = prop!(n <= 10)` forms a snapshot boundary. A mutable proof of `@claim` follows that fixed claim, not future values of `n`. Refresh is required only for dependencies actually named by the tracked type.

## Branch joins

<!-- spec: 1.12:2 dynamic-semantics -->
A branch joins the new versions of outer bindings assigned by any reaching arm. A loop carries outer bindings written in its body or condition, including nested writes and mutable calls; a field write counts for its root. Local or shadowing bindings are separate. After a join or loop, changed values retain only their types and explicitly carried evidence; initial-value equations are not retained.

## Loop invariants

<!-- spec: 1.27:10 dynamic-semantics -->
Tracked evidence carried by a loop must hold at entry, every `continue`, and the body’s end. It is available after the loop. Evidence not carried retains only its old snapshot; if it tracks changed data, it must be re-established before use. A fact needed after a `break` must be valid on that exit too.

<!-- spec: 1.90:39 example -->
~~~rust run
#[no_panic]
fn count_to(limit: u8) -> (count: u8, @(count <= limit)) {
    let mut count: u8 = 0;
    let mut bounded: @(count <= limit) = _;
    while count < limit {
        count = count + 1;
        bounded = _;
    }
    (count, bounded)
}
//~ run: count_to(0) => (0, Erased)
//~ run: count_to(255) => (255, Erased)
~~~

<!-- spec: 1.91:19 informative -->
At entry, `0 <= limit`. Each iteration increments only while `count < limit`, so the addition fits and the bound can be refreshed. After the loop, `bounded` supplies the promised result property. The contract deliberately promises the maintained bound; it does not certify this loop’s termination or rely on an automatically exported exit condition.

## Item scope

<!-- spec: 1.2:2 legality-rule -->
Items are checked in dependency order, independent of textual order. Types and values have separate namespaces. Supported logical recursion and mutually referring logical enum groups are checked specially; other dependency cycles, including ordinary runtime recursion, are rejected.
