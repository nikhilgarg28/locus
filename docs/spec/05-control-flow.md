+++
id = "language-control-flow"
title = "Control flow"
group = "Now"
spec_chapter = 1
order = 104
route = "specification/control-flow.html"
description = "Branches, loops, early returns, panics, and the facts each path establishes."
+++

# Control flow

<!-- spec: 1.0:6 informative -->
Branches and loops organize execution while also determining which facts a proof may use. This chapter explains the facts available inside a branch, the evidence carried through a loop, and the behavior of early exits and panic.

## if, else, loops, return

<!-- spec: 1.11:1 dynamic-semantics -->
In ordinary execution, `if c { a } else { b }` branches on a physical `bool`; each branch knows the outcome of the test it made, `a < b` as the fact `a < b` and its negation on the other side, and a condition `a != b` is the test `a == b` with the branches exchanged. An `if` in statement position still takes its `else`, which may be empty. `c && d`, `c || d`, and `!c` on `bool` are conditionals, so the right operand of `&&` knows the left was true, and the right operand of `||` knows it was false. Logical conditionals instead use Bool and must satisfy the [logical computation rules](08-logic.md#expressions-and-computation-modes).

<!-- spec: 1.11:2 dynamic-semantics -->
The loops are Rust's: `loop { ... }` with `break value` giving it its value and `continue` starting the next pass; `while c { ... }`, of type `()`, whose `break` carries nothing; and `for i in lo..hi { ... }` or `lo..=hi` over a range of any machine integer type, whose index is immutable and whose body knows `lo <= i` and `i < hi`, or `i <= hi`, afresh on every pass. Nothing is asked about the order of the bounds: a range whose start lies past its end runs no pass, and the bounds are evaluated once. `break` and `continue` belong to the innermost loop. A `loop` that never breaks fits any result type. A loop never promises to terminate: it is forbidden under `terminates` and in a proposition. A `for` over anything but a range, and `while let`, are not in Locus yet.

<!-- spec: 1.11:3 dynamic-semantics -->
`return`, or `return value`, ends the function from any depth, and the value is checked against the result type in what is known where the `return` stands. `return`, `break`, `continue`, the panic forms, and a call of a function declared `-> !` have the never type, which coerces to any type; a function declared `-> !` must not reach the end of its body.

## The forms that panic

<!-- spec: 1.10:1 dynamic-semantics -->
`panic!(msg)`, `todo!()`, `todo!(msg)`, `unreachable!()`, and `unreachable!(msg)` yield no value and stand where any type is expected; `assert!(c)`, `assert!(c, msg)`, `debug_assert!(c)`, and `debug_assert!(c, msg)` are checks of type `()`. A message is a string literal, written as it is; format arguments are not in Locus. After `assert!(c)` the condition is a fact. Without a `no_panic` promise, `todo!()` can stand for an unfinished body or the rest of one: it panics rather than establishing a successful return.

<!-- spec: 1.10:2 dynamic-semantics -->
A panic is a third way for a function to end, beside returning and never returning. Under `no_panic` every panic form must carry evidence of `false` at its point, found as a hole is: `assert!(x <= 3)` from a hypothesis or by arithmetic, `unreachable!()` from the facts of the arms around it. Code after a statement that transfers control or panics is unreachable, reported as a warning once per block, and not elaborated.
