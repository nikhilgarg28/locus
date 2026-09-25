+++
id = "language-functions"
title = "Functions and methods"
group = "Now"
spec_chapter = 1
order = 103
route = "specification/functions.html"
description = "Signatures, result scope, methods, constants, and checked effect promises."
+++

# Functions and methods

<!-- spec: 1.0:5 informative -->
A signature tells the caller what to supply and what a successful return establishes. Evidence parameters express preconditions; evidence in the result expresses postconditions. An ordinary function may still panic or diverge unless it makes a stronger promise.

## Parameters and results

<!-- spec: 1.9:1 syntax -->
Declare an ordinary function as `fn name(parameters) -> Result { body }`. Omitting the result annotation means `-> ()`; it does not infer the result from the body. This applies to methods and spec headers too. Parameters may be mutable or borrowed. A result type can mention input values, and named tuple fields bind returned values for later evidence fields. Ordinary runtime recursion is not yet supported.

<!-- spec: 1.90:22 example -->
~~~rust run
#[no_panic]
fn difference(lo: u32, hi: u32, ordered: @(lo <= hi))
    -> (gap: u32, @(gap == hi - lo))
{
    let gap = hi - lo;
    (gap, _)
}
fn distance() -> u32 {
    let (gap, correct) = difference(5, 12, prove!(5 <= 12));
    gap
}
//~ run: distance() => 7
~~~

<!-- spec: 1.91:16 informative -->
The result’s `lo` and `hi` refer to this call’s inputs. `gap` names the returned tuple’s first component. The caller receives evidence about its own destructured value, not a reference to a vanished callee local. Mutable references use the [entry/return convention](06-ownership.md#entry-and-return-values).

<!-- spec: 1.96:8 example -->
~~~rust run
fn clear(value: &mut u8) { value = 0; }
fn demo() -> u8 { let mut value: u8 = 8; clear(&mut value); value }
//~ run: demo() => 0
~~~

## Functions and promises

<!-- spec: 1.9:2 syntax -->
Promises are explicit attributes on a function, or file defaults such as `#![no_panic]`. The compiler checks each promise against the body and callees; it does not infer a missing promise.

<!-- spec: 1.9:3 syntax -->
| Promise | Requirement |
|---|---|
| `#[terminates]` | No loops; every runtime callee promises termination. |
| `#[no_panic]` | Prove primitive safety conditions and unreachable panic paths; runtime callees promise no panic. |
| `#[no_alloc]` | No allocating Box/collection operations; runtime callees promise no allocation. |
| `#[no_io]` | Runtime callees promise no I/O; the current core has no I/O primitive. |

<!-- spec: 1.90:23 example -->
~~~rust check
#![no_panic]
#[terminates] #[no_alloc] #[no_io]
fn bounded_sum(a: u8, b: u8, fits: @(a + b <= u8::MAX)) -> u8 {
    a + b
}
~~~

<!-- spec: 1.9:4 syntax -->
Runtime promises do not make a function callable in logic. Only `logic fn` does that. A broken promise is reported at the offending construct. A runtime `terminates(decreases = ...)` annotation is unsupported; logical recursion has its own [checked descent rules](08-logic.md#logical-data).

## Constants

<!-- spec: 1.9:5 syntax -->
A runtime `const` accepts literals, references to constants, casts, comparisons, integer arithmetic, and products or variants built from them. Machine arithmetic is checked during compilation; overflow and division by zero are errors unless explicit wrapping operations are used. It emits a Rust constant. A logical constant, including one of type `Prop`, is erased and may use logical functions. Constant definitions have logical defining equations.

<!-- spec: 1.90:24 example -->
~~~rust check
const RETRIES: u8 = 3;
const retries_fit: Prop = prop!(RETRIES < u8::MAX);
fn allowed() -> @retries_fit { fold!(retries_fit, prove!(RETRIES < u8::MAX)) }
~~~

<!-- spec: 1.92:13 legality-rule -->
Observe a physical constant's checked value through its canonical model. Its initializer keeps physical semantics; it is not reinterpreted as logical arithmetic. Machine `MIN`/`MAX` constants follow this rule. `const fn` declarations are not yet supported. A call in logic still requires `logic fn`; runtime effect promises do not suffice.

<!-- spec: 1.92:15 legality-rule -->
Declare an associated constant inside `impl T` as `const NAME: Type = value;` and read it as `T::NAME`, without call parentheses. Inside the implementation, `Self` names `T`. Associated constants follow the same initializer and visibility rules as free constants. Forward references are allowed; dependency cycles and name collisions with other constants, functions or variants are rejected.

<!-- spec: 1.92:14 example -->
~~~rust run
struct Limits {}
impl Limits {
    const LIMIT: u32 = u32::MAX;
    const ROLLED: u8 = 255u8.wrapping_add(1);
}
logic fn constants() -> @(Limits::ROLLED == 0 && Limits::LIMIT + 1 == 4294967296) {
    And::Intro(prove!(Limits::ROLLED == 0), prove!(Limits::LIMIT + 1 == 4294967296))
}
fn value() -> u8 { Limits::ROLLED }
//~ run: value() => 0
~~~

## Methods

<!-- spec: 1.14:1 syntax -->
`impl T` contains associated constants, functions and methods. Call an associated function as `T::name(args)` and a method as `value.name(args)`. `Self` denotes `T` in types, literals, and variant paths. A logical method remains a logical function, selectable by path in `fold!` and `unfold!`. Concrete [trait implementations](20-traits.md) share these receiver and proof rules. The compiler-owned Model interface remains separate.

<!-- spec: 1.27:4 syntax -->
A receiver may be `self`, `mut self`, `&self`, or `&mut self`. For runtime methods, by-value receivers move unless `Copy`; shared receivers lend for reading; mutable receivers require mutable storage. Logical methods instead [observe their receivers](08-logic.md#observing-arguments), whether declared with `self` or `&self`. `*self` reads a reference receiver, or replaces it whole through `&mut self`. The equivalent path call supplies the receiver explicitly.

<!-- spec: 1.90:25 example -->
~~~rust run
struct Counter { value: u8 }
impl Counter {
    fn new(value: u8) -> Self { Self { value } }
    fn read(&self) -> u8 { self.value }
    fn clear(&mut self) -> () { self.value = 0; () }
}
fn demo() -> u8 {
    let mut counter = Counter::new(7);
    counter.clear();
    counter.read()
}
//~ run: demo() => 0
~~~
