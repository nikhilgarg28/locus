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
A runtime branch both chooses code to execute and gives the proof checker a fact about that path. Loops need evidence that remains valid from one iteration to the next. A result guarantee describes normal return; panic and divergence are separate outcomes.

## if and else

<!-- spec: 1.11:1 dynamic-semantics -->
A runtime `if` tests a physical `bool`. Its branches receive the condition and its negation as facts. `else` is required, even for a statement; it may be empty. Boolean `&&` and `||` short-circuit, so their right operands know the left operand’s outcome. Logical conditionals instead use `Bool` and produce logical results.

<!-- spec: 1.90:26 example -->
~~~rust run
fn at_least(a: u8, b: u8) -> (out: u8, @(a <= out && b <= out)) {
    if a >= b {
        (a, And::Intro(prove!(a <= a), prove!(b <= a)))
    } else {
        (b, And::Intro(prove!(a <= b), prove!(b <= b)))
    }
}
//~ run: at_least(4, 9) => (9, Erased)
~~~

## Matching enums

<!-- spec: 1.27:5 legality-rule -->
A runtime `match` covers each enum variant, or supplies a `_` arm. Patterns use the variant’s unit, tuple, or named-field shape, with names or `_` for fields; named patterns allow renaming and `..`. Nested enum patterns are unsupported; irrefutable tuple/unit struct patterns may nest. Each arm knows its constructor equation. Use `if` for booleans and comparisons for integers.

<!-- spec: 1.90:27 example -->
~~~rust check
enum Message { Stop, Payload { byte: u8, channel: u8 } }
fn payload(message: Message) -> Option<u8> {
    match message {
        Message::Stop => None,
        Message::Payload { byte: value, .. } => Some(value),
    }
}
~~~

## Loops

<!-- spec: 1.11:2 dynamic-semantics -->
`loop` repeats until `break`, which may supply its result. `while condition` and range-based `for` have unit results and accept only valueless `break`. `continue` starts the next iteration; both transfers target the innermost loop. Loops are currently forbidden in logical computation and under `#[terminates]`, including finite range loops. Iterator-based `for` and `while let` are unsupported.

<!-- spec: 1.27:6 dynamic-semantics -->
A range loop evaluates its bounds once. `lo..hi` supplies `lo <= i && i < hi`; `lo..=hi` supplies `lo <= i && i <= hi`. The immutable index has the bounds’ machine type. Reversed ranges are empty. A loop that never breaks can satisfy any expected result type by never producing a result.

<!-- spec: 1.90:28 example -->
~~~rust run
fn first_attempt(ready: bool) -> u8 {
    loop {
        if ready { break 1; } else { break 0; }
    }
}
fn range_sum() -> u8 {
    let mut total: u8 = 0;
    for i in 1u8..=3u8 { total = total.wrapping_add(i); }
    total
}
//~ run: first_attempt(true) => 1
//~ run: range_sum() => 6
~~~

<!-- spec: 1.91:17 informative -->
A loop does not retain the initial value of a variable it changes. Carry the needed property as [tracked evidence](07-mutation.md#loop-invariants). This is the induction step: establish the property before the loop, then re-establish it after each update.

## Early return

<!-- spec: 1.11:3 dynamic-semantics -->
`return value` ends the function at any nesting depth; `return` supplies unit. The value must satisfy the declared result type using facts available at that point. Return, break, continue, panic, and calls declared `-> !` do not produce a normal value and coerce to any expected type. A `-> !` function must not reach its body’s end.

<!-- spec: 1.90:29 example -->
~~~rust run
fn bounded(value: u8) -> (out: u8, @(out <= 10)) {
    if value <= 10 { return (value, prove!(value <= 10)); } else { }
    (10, prove!(10 <= 10))
}
//~ run: bounded(3) => (3, Erased)
//~ run: bounded(20) => (10, Erased)
~~~

## The forms that panic

<!-- spec: 1.10:1 dynamic-semantics -->
`panic!(message)`, `todo!()`, and `unreachable!()` panic; the latter two also accept a message. `assert!(condition)` and `debug_assert!(condition)` return unit, with optional messages. Messages are string literals, not format arguments. Normal continuation after `assert!` supplies its condition as a fact; `debug_assert!` supplies no such fact because it can be disabled.

<!-- spec: 1.90:30 example -->
~~~rust run
fn divide(n: u32, divisor: u32) -> u32 {
    assert!(divisor != 0, "zero divisor");
    n / divisor
}
//~ run: divide(12, 3) => 4
~~~

<!-- spec: 1.10:2 dynamic-semantics -->
Under `no_panic`, every possible panic path needs evidence that it is unreachable. For `assert!(condition)`, this means proving its condition. Without that promise, `todo!()` may stand for unfinished code and simply panics. Code following an unconditional control transfer is warned about once per block and is not elaborated.
