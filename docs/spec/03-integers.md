+++
id = "language-integers"
title = "Integers and operators"
group = "Now"
spec_chapter = 1
order = 102
route = "specification/integers.html"
description = "Machine arithmetic has Rust semantics; mathematical observations use Int."
+++

# Integers and operators

<!-- spec: 1.0:4 informative -->
A machine operation computes a runtime result. A formula describes that result with mathematical integers. Keeping these separate lets a specification express an exact answer even when a careless implementation would overflow.

## Literals and casts

<!-- spec: 1.15:1 syntax -->
Integer literals support decimal, hexadecimal (`0x`), octal (`0o`), binary (`0b`), underscores, and type suffixes. They use an expected type, otherwise a suffix, otherwise `i32`; incompatible suffixes are rejected. A negative literal includes its minus sign. `T::MIN` and `T::MAX` have type `T`. Machine-to-machine `as` casts truncate or extend as in Rust.

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
Machine `+`, `-`, `*`, and signed unary minus panic on overflow when overflow checks are enabled and wrap when disabled. `/` and `%` panic on zero divisors and the signed `MIN / -1` case in either mode. Comparisons require operands of the same machine type. Bit operators and shifts are unsupported.

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
Under `#[no_panic]`, each potentially panicking operation needs evidence of its safety condition. For addition, the mathematical sum must fit the machine range. Once checked, its exact mathematical result is available. Without that promise, addition supplies only the modular result valid in both build modes. Normal continuation after division also establishes its nonzero divisor and excludes signed overflow.

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
In logical computation, machine integers are observed as `Int`; `as Int` makes this explicit. Logical arithmetic is total. Division truncates toward zero, with `a / 0 == 0` and `a % 0 == a`. These definitions do not change runtime division. A cast from Logical `Int` to a machine type is rejected, including inside propositions.

<!-- spec: 1.90:21 example -->
~~~locus check
logic fn division_laws() -> @(7 / 0 == 0 && 7 % 0 == 7) {
    And::Intro(prove!(7 / 0 == 0), prove!(7 % 0 == 7))
}
fn byte_model(n: u8) -> @(n + 1 > n) {
    // This claim uses Int addition, including when n is 255.
    prove!(n + 1 > n)
}
~~~
