+++
id = "language-integers"
title = "Integers and operators"
group = "Now"
spec_chapter = 1
order = 102
route = "specification/integers.html"
description = "Checked machine arithmetic, natural and integer models, and proof-directed check elimination."
+++

# Integers and operators

<!-- spec: 1.0:4 informative -->
A machine operation computes a runtime result. A formula describes that result with mathematical integers. Keeping these separate lets a specification express an exact answer even when a careless implementation would overflow.

## Literals and casts

<!-- spec: 1.15:1 syntax -->
Integer literals support decimal, hexadecimal (`0x`), octal (`0o`), binary (`0b`), underscores, and type suffixes. They use an expected type, otherwise a suffix, otherwise `i32`; incompatible suffixes are rejected. A negative literal includes its minus sign. `T::MIN` and `T::MAX` have physical type `T` and its canonical model type in logic; see [constants](04-functions.md#constants). Machine-to-machine `as` casts truncate or extend as in Rust.

<!-- spec: 1.90:17 example -->
~~~locus run
fn narrow() -> (u8, i16) {
    let low = 0x1234u16 as u8;
    let signed = -1i8 as i16;
    (low, signed)
}
//~ run: narrow() => (52, -1)
~~~

## Machine arithmetic

<!-- spec: 1.15:2 syntax -->
Machine `+`, `-`, `*`, and signed unary minus return the exact mathematical result when it fits their type, and panic otherwise. This rule is independent of Rust overflow-check settings. `/` and `%` panic on zero divisors and the signed `MIN / -1` case in either mode. Comparisons require operands of the same machine type. Bit operators and shifts are unsupported.

<!-- spec: 1.27:2 dynamic-semantics -->
`wrapping_add`, `wrapping_sub`, `wrapping_mul`, and signed `wrapping_neg` wrap explicitly and never panic. Their result equations retain that width-dependent meaning inside propositions.

<!-- spec: 1.90:18 example -->
~~~locus run
fn rollover(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    let out = n.wrapping_add(1);
    (out, _)
}
//~ run: rollover(255) => (0, Erased)
~~~

## Proving that arithmetic cannot panic

<!-- spec: 1.27:3 dynamic-semantics -->
The compiler tries to prove an operation’s safety from facts available before it. Checked evidence permits a plain Rust operator; otherwise it emits `checked_add`, `checked_sub`, `checked_mul`, or `checked_neg` followed by `expect`. Under `#[no_panic]`, missing safety evidence is an error. Optional safety certificates use the proof store; a missing certificate under `--locked` retains the runtime check. Successful arithmetic establishes its exact result; division also establishes a nonzero divisor and excludes signed overflow.

<!-- spec: 1.90:19 example -->
~~~locus run
#[no_panic]
fn increment(n: u8, room: @(n < u8::MAX))
    -> (out: u8, @(out == n + 1))
{
    let out = n + 1;
    (out, _)
}
//~ run: increment(254, Erased) => (255, Erased)
~~~

<!-- spec: 1.91:15 informative -->
Removing the precondition makes `255 + 1` possible. The compiler reports an unmet safety obligation at the addition; it does not invent a runtime guard to satisfy `no_panic`.

<!-- spec: 1.90:20 example -->
~~~locus reject L0235
#[no_panic]
fn increment(n: u8) -> u8 { n + 1 }
~~~

## Logical arithmetic

<!-- spec: 1.15:3 syntax -->
Unsigned machine values are observed as `Nat`, and signed values as `Int`. `Nat as Int` preserves the value; unchecked `Int as Nat` is rejected. Nat widens to Int when an Int is expected or the other typed operand is Int. Natural subtraction requires evidence that the right operand does not exceed the left. Logical arithmetic is total on these admitted inputs. Division truncates toward zero, with `a / 0 == 0` and `a % 0 == a`. These definitions do not change runtime division. A cast from Logical `Int` to a machine type is rejected, including inside propositions.

<!-- spec: 1.90:21 example -->
~~~locus check
logic fn division_laws() -> @(7 / 0 == 0 && 7 % 0 == 7) {
    And::Intro(prove!(7 / 0 == 0), prove!(7 % 0 == 7))
}
fn byte_model(n: u8) -> @(n + 1 > n) {
    // This claim uses Nat addition, including when n is 255.
    prove!(n + 1 > n)
}
~~~

<!-- spec: 1.92:8 legality-rule -->
Safety evidence is checked before introducing the operation’s result or successful-return facts. Operand effects execute once, in source order, even when a runtime check fails. A postcondition describes normal return; it does not imply that the call cannot panic.

<!-- spec: 1.92:9 example -->
~~~locus run
fn sum(a: u8, b: u8) -> (out: u8, @(out == a + b)) {
    let out = a + b;
    (out, _)
}
//~ run: sum(40, 2) => (42, Erased)
//~ run: sum(255, 1) => panic: attempt to add with overflow
~~~
