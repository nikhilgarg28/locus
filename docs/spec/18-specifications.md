+++
id = "language-specifications"
title = "Specifications and implementations"
group = "Now"
spec_chapter = 1
order = 113
route = "specification/specifications.html"
description = "Opaque type interfaces with checked local implementations."
+++

# Specifications and implementations

<!-- spec: 1.29:7 informative -->
A spec separates a type's reviewed interface from its checked implementation. Its name denotes a concrete, opaque type. The backing type has a different identity. Specs add no assumptions to the proof kernel.

## Declare the interface

<!-- spec: 1.29:1 legality-rule -->
`spec type Name<T> { ... }` contains function, logical-function, constant and associated-type headers. Members are implicitly public; visibility belongs on the spec. Headers end in `;` and have no bodies. Module specs, generic methods, associated types in generic families and default bodies are deferred.

<!-- spec: 1.29:8 example -->
~~~rust check
spec type Arithmetic {
    fn next(n: u8) -> (out: u8, @(out == n.wrapping_add(1)));
    const FIRST: u8;
}
struct ArithmeticImpl {}
impl Arithmetic for ArithmeticImpl {
    const FIRST: u8 = 0;
    fn next(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
        let out = n.wrapping_add(1);
        (out, _)
    }
}
fn use_interface() -> u8 {
    let (value, evidence) = Arithmetic::next(Arithmetic::FIRST);
    value
}
~~~

## Supply a backing type

<!-- spec: 1.29:2 legality-rule -->
`impl Spec for Representation { ... }` supplies exactly the declared members. In the implementation, `Self` and `self` refer to the backing representation. Through the spec, they refer to the opaque type. An explicit spec name in an implementation still denotes the public type; it is not an alias for the representation. Checked adapters wrap owned `Self` results, including tuple components, and unwrap owned or borrowed `Self` inputs. Borrowed/container `Self` results and nested `Self` inputs are rejected. Backing fields and additional inherent methods are inaccessible through the spec. The backing type remains independently usable under ordinary privacy. There is no implicit conversion between it and the spec; put helpers in inherent `impl Representation` blocks.

<!-- spec: 1.29:9 example -->
~~~rust check
mod percent {
    pub spec type Percent {
        fn zero() -> Self;
        fn get(&self) -> u8;
    }
    struct Representation { value: u8, valid: @(value <= 100) }
    impl Percent for Representation {
        fn zero() -> Self { Self { value: 0, valid: _ } }
        fn get(&self) -> u8 { self.value }
    }
}
fn read() -> u8 {
    let value = percent::Percent::zero();
    value.get()
}
~~~

## Check the whole family

<!-- spec: 1.29:3 legality-rule -->
Every loaded spec needs exactly one complete implementation in its owning package, even if unused. Repeated spec declarations and additional inherent spec implementations are rejected. A generic implementation repeats the entire family, with the same bounds and argument order; specialization and stronger bounds are rejected. Generic bodies are checked at concrete instantiation, **not universally when unused**.

<!-- spec: 1.29:10 example -->
~~~rust reject L0511
spec type Counter { fn read() -> u8; }
struct Representation {}
impl Counter for Representation {}
~~~

<!-- spec: 1.29:15 example -->
~~~rust check
spec type Cell<T> {
    fn new(value: T) -> Self;
    fn take(self) -> T;
}
struct Storage<T> { value: T }
impl<T> Cell<T> for Storage<T> {
    fn new(value: T) -> Self { Self { value } }
    fn take(self) -> T { self.value }
}
fn example() -> u8 {
    let cell: Cell<u8> = Cell::new(7);
    cell.take()
}
~~~

## Match the contract

<!-- spec: 1.29:4 legality-rule -->
Matching uses resolved types and consistently renamed binders, ignoring grouping and source locations. Receivers, logical mode, lifetime relationships, input proofs and result propositions must agree. Header effect promises become body obligations. Constants specify a type and supply checked initializers. A concrete associated-type binding replaces `Self::Item` and `Spec::Item`; chained/recursive associated aliases are deferred.

<!-- spec: 1.29:11 example -->
~~~rust check
spec type Arithmetic {
    #[no_panic]
    fn increment(n: u8, room: @(n < 255)) -> (out: u8, @(out == n + 1));
}
struct Implementation {}
impl Arithmetic for Implementation {
    fn increment(value: u8, room: @(value < 255)) -> (out: u8, @(out == value + 1)) {
        let out = value + 1;
        (out, _)
    }
}
~~~

<!-- spec: 1.29:16 example -->
~~~rust check
spec type Counter {
    type Value;
    const INITIAL: Self::Value;
    fn initial() -> Self::Value;
}
struct Implementation {}
impl Counter for Implementation {
    type Value = u8;
    const INITIAL: u8 = 0;
    fn initial() -> u8 { Self::INITIAL }
}
fn read() -> Counter::Value { Counter::initial() }
~~~

<!-- spec: 1.29:5 legality-rule -->
Both manual bodies and generated adapters pass ordinary ownership, effect, proof and IR checks. Native trusted bindings cannot satisfy manual specs. Input proofs remain caller obligations; output proofs require checked evidence. Logical definitions remain transparent. Spec declarations never grant a recursive proof axiom or manufacture representation invariants.

<!-- spec: 1.29:12 example -->
~~~rust reject L0230
spec type Claims { fn impossible() -> @(1 == 0); }
struct Implementation {}
impl Claims for Implementation {
    fn impossible() -> @(1 == 0) { _ }
}
~~~

## Split files and export

<!-- spec: 1.29:6 legality-rule -->
Use ordinary `mod` and `use` to separate a spec from its implementation. Both must belong to the same package; normal name visibility applies. Only referenced source files participate. Rust exports preserve the opaque wrapper and expose its declared methods. Proof outputs receive the usual data-only facade; proof inputs and public logical fields remain unexportable. Generic Rust exports remain deferred.

<!-- spec: 1.29:13 informative -->
The [split-file fixture](../../tests/fixtures/specs/split/export.lc) declares its interface in `export.lc` and implements it in [arithmetic.lc](../../tests/fixtures/specs/split/arithmetic.lc). Tests compile the generated Rust with warnings denied and reject access to hidden members.

<!-- spec: 1.29:14 informative -->
Module specs, broader associated types, traits, Rust imports and assumed native realizations are tracked in the [interoperability roadmap](../roadmap/interop.md#specifications-and-native-imports). The [spec vision](../vision/spec-interfaces.md) separates those extensions from this checked subset.
