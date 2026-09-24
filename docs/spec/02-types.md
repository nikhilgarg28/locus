+++
id = "language-types"
title = "Values and types"
group = "Now"
spec_chapter = 1
order = 101
route = "specification/types.html"
description = "Physical data, logical data, products, enums, and dependent evidence fields."
+++

# Values and types

<!-- spec: 1.0:3 informative -->
A type describes the values a program can hold and the operations it permits. Start with runtime scalars and aggregates, then add logical values to describe and prove their properties. In this chapter, **physical** means represented in the running program; **logical** means erased after checking.

## Type classification

<!-- spec: 1.3:1 legality-rule -->
Every type is either physical or Logical. Machine integers, `bool`, references, and runtime containers are physical. `Int`, `Bool`, `Prop`, proof types, logical callables, and checked `#[derive(Logical)]` declarations are Logical. An ordinary struct or enum remains physical even when it contains logical fields. Tuples retain a product of their field representations, including erased positions; the current frontend does not accept a tuple as a `logic fn` result.

<!-- spec: 1.3:4 legality-rule -->
Classification belongs to the type, not the binding. Use ordinary `let` for logical values and `x as M` to observe a runtime value through a logical [model](11-models.md). A logical value has no runtime contents to recover.

## Machine integers

<!-- spec: 1.91:1 informative -->
The unsigned types `u8`, `u16`, `u32`, and `u64` hold nonnegative integers; `i8`, `i16`, `i32`, and `i64` also hold negative integers. The number is the width in bits. For example, `u8` ranges from 0 to 255 and `i8` from −128 to 127. Each type has `MIN` and `MAX`. See [integer arithmetic](03-integers.md) for literals, casts, and overflow.

<!-- spec: 1.90:1 example -->
~~~locus run
fn packet_fields() -> (u8, u16, i32) {
    let kind: u8 = 0x2a;
    let length = 1_024u16;
    let offset = -12; // i32 when no other type is expected
    (kind, length, offset)
}
//~ run: packet_fields() => (42, 1024, -12)
~~~

## Runtime booleans: bool

<!-- spec: 1.91:2 informative -->
`bool` has the values `true` and `false`. Runtime comparisons produce `bool`; `if` uses it to choose which code executes. `!`, `&&`, and `||` provide negation and short-circuit conjunction and disjunction.

<!-- spec: 1.90:2 example -->
~~~locus run
fn eligible(age: u8, has_ticket: bool) -> bool {
    age >= 16 && has_ticket
}
//~ run: eligible(18, true) => true
//~ run: eligible(12, true) => false
~~~

## Unit and never: () and !

<!-- spec: 1.91:3 informative -->
Unit, written `()`, is the result when there is no useful value to return. The never type, `!`, describes an expression that cannot return normally. A panic or an endless loop can stand where another type is expected because neither supplies a result. See [early exits](05-control-flow.md#early-return).

<!-- spec: 1.90:3 example -->
~~~locus run
fn do_nothing() -> () { () }
fn unfinished() -> ! { todo!("not implemented") }
fn value_or_fail(ready: bool) -> u8 {
    if ready { 7 } else { unfinished() }
}
//~ run: value_or_fail(true) => 7
~~~

## Tuples

<!-- spec: 1.4:1 legality-rule -->
Construct a tuple with `(a, b)` and a one-field tuple with `(a,)`. Read fields by position, such as `pair.0` or `pair.1.0`. Optional names in tuple **types** bind values for later field types; they do not create named accessors. Construction and destructuring substitute each earlier field into the types of the following fields.

<!-- spec: 1.90:4 example -->
~~~locus run
fn successor(n: u8) -> (value: u8, @(value == n.wrapping_add(1))) {
    let value = n.wrapping_add(1);
    (value, _)
}
fn client(n: u8) -> u8 {
    let (value, correct) = successor(n);
    let same_claim: @(value == n.wrapping_add(1)) = correct;
    value
}
//~ run: client(255) => 0
~~~

<!-- spec: 1.4:3 legality-rule -->
A `let` pattern may be a name, `_`, or a tuple of those patterns. A bound name may be `mut`. Runtime structs and enum variants are opened through field access or `match`, not a `let` pattern.

## Structs and dependent proof fields

<!-- spec: 1.3:2 legality-rule -->
Types may annotate parameters, bindings, fields, constants, and results. Later fields and function results may refer to earlier values inside propositions and proof types. These dependencies never select a runtime layout. Named structs are nominal: separately declared structs are different types even when their fields match.

<!-- spec: 1.27:1 legality-rule -->
Construct `S { field: value, other }` with every declared field exactly once, in any order. `other` abbreviates `other: other`. Read a field with `value.field`. A later proof field is checked against the values supplied for the earlier fields.

<!-- spec: 1.90:5 example -->
~~~locus run
pub struct NonZero {
    value: u32,
    valid: @(value != 0),
}
fn checked(value: u32) -> Option<NonZero> {
    if value != 0 {
        Some(NonZero { value, valid: prove!(value != 0) })
    } else {
        None
    }
}
//~ run: checked(7) => Some(NonZero { value: 7, valid: Erased })
//~ run: checked(0) => None
~~~

<!-- spec: 1.91:4 informative -->
`NonZero` ties its evidence to its own `value` field. A struct can instead contain `claim: Prop` followed by `evidence: @claim`, packaging a claim and its proof. A mutable value must preserve these field dependencies; [whole-value replacement](07-mutation.md#assignment) supplies fresh evidence together with new data.

## Enums, Option, and Result

<!-- spec: 1.4:2 legality-rule -->
Enum variants have no fields, positional fields, or named fields. Construct them with `E::V`, `E::V(value)`, or `E::V { field: value }`; `Self::V` works inside the enum’s `impl`. Later payload types may refer to earlier payloads in propositions. A runtime enum keeps its discriminant even when a payload is logical. [Matching](05-control-flow.md#matching-enums) handles its alternatives.

<!-- spec: 1.90:6 example -->
~~~locus run
enum ReadOutcome { End, Byte(u8), Failed { code: u8 } }
fn to_option(outcome: ReadOutcome) -> Option<u8> {
    match outcome {
        ReadOutcome::End => None,
        ReadOutcome::Byte(value) => Some(value),
        ReadOutcome::Failed { code: _ } => None,
    }
}
fn demo() -> Option<u8> { to_option(ReadOutcome::Byte(42)) }
//~ run: demo() => Some(42)
~~~

<!-- spec: 1.91:5 informative -->
The prelude supplies `Option<T>` (`Some`, `None`) and `Result<T, E>` (`Ok`, `Err`). They describe runtime alternatives. For example, `Option<@True>` stores a runtime choice with erased evidence. Generic arguments must be closed: a proof type capturing a local `n` cannot yet be an Option argument. Use a struct or enum with a value field and a dependent proof field instead. These are Locus templates emitted as specialized enums; they are not yet Rust’s standard-library generic ABI.

## Arrays: [T; n]

<!-- spec: 1.91:6 informative -->
An array has a fixed, literal length and one element type. `[u8; 3]` is a type; `[10, 20, 30]` constructs a value. Lengths and runtime indices use `u64`. An access needs a proof that the index is in bounds, often supplied by a surrounding branch. [Collections](11-models.md#collections) specify the bounds and update rules.

<!-- spec: 1.90:7 example -->
~~~locus run
fn sample(index: u64) -> Option<u8> {
    let bytes: [u8; 3] = [10, 20, 30];
    if index < 3 { Some(bytes[index]) } else { None }
}
//~ run: sample(2) => Some(30)
//~ run: sample(3) => None
~~~

## Vectors: Vec<T>

<!-- spec: 1.91:7 informative -->
A `Vec<T>` owns growable runtime storage. Construct an empty vector with `Vec::new()` or initialize one with `Vec::from(array)`. `push` appends an element. Both its length and its contents have logical observations, so a result can state how an operation changed them.

<!-- spec: 1.90:8 example -->
~~~locus run
fn append_byte(value: u8) -> u64 {
    let mut bytes: Vec<u8> = Vec::from([1, 2]);
    bytes.push(value);
    bytes.len()
}
//~ run: append_byte(9) => 3
~~~

## References and slices: &T, &mut T, &[T]

<!-- spec: 1.91:8 informative -->
A reference lends access without transferring ownership. `&T` permits reads; a call-scoped `&mut T` permits updates. A slice borrows an array or vector without fixing its length in the type. Stored and returned shared references need the [lifetime rules](06-ownership.md#shared-references); mutable references cannot yet be stored or returned.

<!-- spec: 1.90:9 example -->
~~~locus run
fn first(items: &[u8]) -> Option<u8> {
    if items.len() > 0 { Some(items[0]) } else { None }
}
fn demo() -> Option<u8> {
    let items: [u8; 2] = [8, 9];
    first(&items)
}
//~ run: demo() => Some(8)
~~~

## Owned indirection: Box<T>

<!-- spec: 1.91:9 informative -->
`Box<T>` owns an indirect runtime value. Use `Box::new(value)` and `*box_value` to construct and dereference it. A recursive runtime enum needs this indirection to have a finite layout. `Box<T>` is always physical, including when `T` is Logical; logical recursive enums use direct recursion instead.

<!-- spec: 1.90:10 example -->
~~~locus run
enum Chain { End, Link(u8, Box<Chain>) }
fn singleton(value: u8) -> Chain {
    Chain::Link(value, Box::new(Chain::End))
}
fn boxed_byte() -> u8 {
    let value = Box::new(17u8);
    *value
}
//~ run: boxed_byte() => 17
~~~

## Logical integers: Int

<!-- spec: 1.91:10 informative -->
`Int` denotes mathematical integers without a machine-width bound. Its arithmetic is erased. A runtime integer can be observed as an `Int`, but an `Int` cannot be converted back into runtime data. The [operators chapter](03-integers.md#logical-arithmetic) defines its total division and remainder operations.

<!-- spec: 1.90:11 example -->
~~~locus check
logic fn distance_squared(x: Int, y: Int) -> Int {
    let delta = x - y;
    delta * delta
}
logic fn concrete_distance() -> @(distance_squared(3, 7) == 16) {
    fold!(distance_squared, prove!((3 - 7) * (3 - 7) == 16))
}
~~~

## Logical booleans: Bool

<!-- spec: 1.91:11 informative -->
`Bool` has logical true and false values. Comparisons inside logical computation produce it; a logical `if` can select between logical results. It cannot control an executable branch. A runtime `bool` can be observed through its `Bool` model.

<!-- spec: 1.90:12 example -->
~~~locus check
logic fn nonnegative(n: Int) -> Bool { n >= 0 }
logic fn magnitude(n: Int) -> Int {
    if n < 0 { -n } else { n }
}
~~~

## Claims and evidence: Prop and @P

<!-- spec: 1.91:12 informative -->
`Prop` holds a claim, such as `prop!(n != 0)`. `@P` holds evidence for the particular claim `P`. Both erase. A false claim is a valid proposition value; producing its evidence is what the checker must refuse. See [propositions](09-propositions.md) and [proofs](10-proofs.md).

<!-- spec: 1.90:13 example -->
~~~locus check
fn claim_and_evidence(n: Int) -> (claim: Prop, @claim) {
    let claim = prop!(n == n);
    (claim, prove!(n == n))
}
~~~

## User-defined logical data

<!-- spec: 1.91:13 informative -->
`#[derive(Logical)]` checks that all stored fields are Logical. Logical enums may be directly recursive, with finite values and checked positive recursion. The checked library defines `Nat` (natural numbers), `Maybe<T>` (an optional logical value), and `Seq<T>` (a finite sequence). These are ordinary declarations, not additional compiler primitives. See [logical data and recursion](08-logic.md#logical-data).

<!-- spec: 1.90:14 example -->
~~~locus check
#[derive(Logical)]
struct Bounds { lower: Int, upper: Int }
#[derive(Logical)]
enum Nat { Zero, Succ(Nat) }
logic fn two() -> Nat { Nat::Succ(Nat::Succ(Nat::Zero)) }
~~~

## Logical function values

<!-- spec: 1.91:14 informative -->
`logic Fn(x: T) -> U` is the type of an erased callable. Its parameters and result are Logical; a result may contain evidence about its parameters. A closure such as `|x: Int| x + 1` constructs one. Named logical functions can additionally observe runtime inputs; [closures](08-logic.md#logical-closures) have the narrower signature rule.

<!-- spec: 1.90:15 example -->
~~~locus check
logic fn twice(f: logic Fn(x: Int) -> Int, value: Int) -> Int {
    f(f(value))
}
logic fn add_two(n: Int) -> Int { twice(|x: Int| x + 1, n) }
~~~

## Generic types and functions

<!-- spec: 1.3:3 legality-rule -->
Generic structs, enums, propositions, and functions are checked templates. Each concrete instantiation is elaborated and kernel-checked; an unused body is not universally verified. `T: Logical` is the supported bound. Generic methods in inherent `impl` blocks and general trait definitions are unsupported; [Model](11-models.md#defining-a-model) is a dedicated built-in interface.

<!-- spec: 1.90:16 example -->
~~~locus check
#[derive(Logical)]
struct Pair<T: Logical> { first: T, second: T }
logic fn duplicate<T: Logical>(value: T) -> Pair<T> {
    Pair { first: value, second: value }
}
logic fn example() -> Pair<Int> { duplicate(3) }
~~~

<!-- spec: 1.27:13 legality-rule -->
Generic type arguments must be closed with respect to local value bindings. `Option<@True>` is supported; `Option<@(n > 0)>` for a parameter `n` is not. A named aggregate can bind its own value field and use that field in a later proof type.
