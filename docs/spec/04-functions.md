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
A function signature describes the data and evidence exchanged with its caller. Function promises add checked requirements such as avoiding panic; methods use the same model with a receiver. This chapter gives the supported declaration forms and their boundaries.

## Functions and promises

<!-- spec: 1.9:1 syntax -->
A function is `fn name(params) -> R { body }`, with a result type that may name its value, `-> (out: u8, @(out <= 3))`, so that the evidence returned speaks of the data returned. A parameter may be `mut`, `&T`, or `&mut T`. Every function has a result type; `-> ()` is written out. Ordinary runtime recursion is not yet supported; logical recursion is checked as described in [Logical data](08-logic.md#logical-data).

<!-- spec: 1.9:2 syntax -->
The promises are four attributes, written on the function or once for the file as `#![no_panic]`, never inferred, and each checked in the check IR against the body:

<!-- spec: 1.9:3 syntax -->
| Promise | What is checked |
|---|---|
| `#[terminates]` | no `loop`, `while`, or `for` anywhere in the body, and every function called promises it |
| `#[no_panic]` | every operator that may panic carries evidence that it does not ([Integers and operators](03-integers.md#machine-arithmetic)), every panic form carries evidence of `false` ([The forms that panic](05-control-flow.md#the-forms-that-panic)), and every function called promises it |
| `#[no_io]` | every function called promises it; the core has no primitive that performs I/O |
| `#[no_alloc]` | every callee promises it; allocating native collection and Box operations are rejected |

<!-- spec: 1.9:4 syntax -->
Only `logic fn` declares a function of the logic ([Expressions and computation modes](08-logic.md#expressions-and-computation-modes)). Runtime promises do not change this classification. A promise not kept is reported at the construct that breaks it. `#[terminates(decreases = e)]` is parsed but rejected. Use checked structural recursion or `recurse!(evidence, call)` in a logical function; this attribute does not introduce ordinary runtime recursion.

<!-- spec: 1.9:5 syntax -->
A `const` with a runtime form is a Rust `const`: its value is a literal, a cast, a comparison, a wrapping method, or a tuple, struct, or variant of those, naming other constants, and it is used by name; in the logic it is a function of no parameters with a defining equation. A constant of type `Prop` has no runtime form and may mention any function of the logic.

## Methods

<!-- spec: 1.14:1 syntax -->
`impl T { ... }` declares functions under `T`. One with no `self` is an associated function, `T::name(args)`; one whose first parameter is `self`, `mut self`, `&self`, or `&mut self` is a method, called `x.name(args)` or by path with the receiver written as a lend, `T::name(&x, args)`. The receiver is a parameter named `self` of type `T`, passed as written: `self` moves `x` unless it is `Copy`, `&self` lends it, `&mut self` lends it mutably and needs a `let mut x`. `Self` is `T` inside the block, in types, literals, and variant paths; `*self` is the value behind a reference receiver, read, matched, or for `&mut self` replaced whole. In the logic a method is the function `T::name`, which a proposition calls as `c.small()` or `Counter::small(&c)`, and which `unfold!` and `fold!` take by path. An inherent `impl` block holds functions only. General trait declarations and implementations are unsupported; `impl Model<Runtime> for LogicalDestination` is the dedicated checked exception.
