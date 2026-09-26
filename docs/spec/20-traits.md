+++
id = "language-traits"
title = "Traits"
group = "Now"
spec_chapter = 1
order = 114.5
route = "specification/traits.html"
description = "Reusable interfaces with concrete, checked implementations and explicit proof contracts."
+++

# Traits

<!-- spec: 1.31:1 legality-rule -->
A `trait` describes associated types, constants, functions and methods. Its name is not a concrete type. It may have no implementations or many; a concrete type may implement several traits. Members are implicitly public. Place helper methods in an inherent `impl`.

<!-- spec: 1.31:20 example -->
~~~rust check
trait Read {
    fn read(&self) -> u8;
}
struct Counter { value: u8 }
impl Read for Counter {
    fn read(&self) -> u8 { self.value }
}
fn example() -> u8 {
    let counter = Counter { value: 7 };
    counter.read()
}
~~~

## Associated items

<!-- spec: 1.31:2 legality-rule -->
An implementation supplies every required member, exactly once, with no extra members or visibility modifiers. `Self` names its concrete implementing type. Associated type bindings substitute for `Self::Item`; chained bindings are deferred. `type View: Logical;` requires a logical binding. Unconstrained associated slots impose no logical classification until instantiated.

<!-- spec: 1.31:21 example -->
~~~rust check
trait Source {
    type Item;
    const INITIAL: Self::Item;
    fn initial() -> Self::Item;
}
struct Bytes {}
impl Source for Bytes {
    type Item = u8;
    const INITIAL: u8 = 7;
    fn initial() -> u8 { Self::INITIAL }
}
fn example() -> <Bytes as Source>::Item {
    Bytes::initial()
}
~~~

## Contracts and logical computation

<!-- spec: 1.31:3 legality-rule -->
Resolved signatures must match after consistently renaming parameters. Receivers, logical mode, input evidence and output claims are part of that signature. Trait declarations introduce no evidence. Callers supply input proofs; implementation bodies must establish output proofs. Header promises become body obligations.

<!-- spec: 1.31:22 example -->
~~~rust check
trait Increment {
    #[no_panic]
    fn next(n: u8, room: @(n < 255)) -> (out: u8, @(out == n + 1));
}
struct Arithmetic {}
impl Increment for Arithmetic {
    fn next(value: u8, room: @(value < 255)) -> (out: u8, @(out == value + 1)) {
        let out = value + 1;
        (out, _)
    }
}
fn example() -> u8 {
    let (value, proof) = Arithmetic::next(7, prove!(7 < 255));
    value
}
~~~

<!-- spec: 1.31:4 legality-rule -->
`logic fn` members obey the existing purity, totality and erasure rules. Ordinary methods remain executable even when returning only evidence. Being a trait method, taking `&self`, or having an effect promise does not make a function logical. Logical observers are separate from the canonical `Model` implementation.

<!-- spec: 1.31:23 example -->
~~~rust check
trait Identity {
    type Value: Logical;
    logic fn identity(value: Self::Value) -> Self::Value;
}
struct Mathematics {}
impl Identity for Mathematics {
    type Value = Nat;
    logic fn identity(value: Nat) -> Nat { value }
}
fn identity_proof(n: Nat) -> @(Mathematics::identity(n) == n) {
    fold!(Mathematics::identity, prove!(n == n))
}
~~~

## Defaults and checking

<!-- spec: 1.31:5 legality-rule -->
Functions and constants may provide defaults. Each concrete implementation rechecks inherited bodies with its selected associated bindings and overrides. Calls to trait members through `self` or `Self` within those bodies select that implementation. Logical dependency cycles are rejected. An unused trait/default template is not a universally checked theorem; full body checking happens for concrete implementations.

<!-- spec: 1.31:24 example -->
~~~rust check
trait Amount {
    fn amount(&self) -> u8;
    fn doubled(&self) -> u8 { self.amount() + self.amount() }
}
struct Three {}
impl Amount for Three {
    fn amount(&self) -> u8 { 3 }
}
fn example() -> u8 {
    let value = Three {};
    value.doubled()
}
~~~

## Selection and ownership

<!-- spec: 1.31:6 legality-rule -->
Concrete calls use static dispatch. An inherent method takes precedence in the currently supported receiver tier; otherwise the trait must be bound in the caller's module. Multiple candidates require `<Type as Trait>::method(...)`. The same qualification selects associated constants and types. This version does not implement Rust's full autoderef candidate search.

<!-- spec: 1.31:25 example -->
~~~rust check
trait Left { fn value(&self) -> u8; }
trait Right { fn value(&self) -> u8; }
struct Choice {}
impl Left for Choice { fn value(&self) -> u8 { 1 } }
impl Right for Choice { fn value(&self) -> u8 { 2 } }
fn example() -> u8 {
    let choice = Choice {};
    <Choice as Right>::value(&choice)
}
~~~

<!-- spec: 1.31:7 legality-rule -->
Implementations currently target named Locus structs, enums and opaque spec types. A trait/type pair has at most one implementation in the loaded package graph. The implementation's package must own the trait or its concrete type. An imported Rust trait retains its original identity across aliases. Implementations for another Locus package's runtime types remain deferred until cross-package method ABI support is available. A spec's backing representation and its public opaque type are different types: implementing a trait for one does not implement it for the other.

<!-- spec: 1.31:8 dynamic-semantics -->
Selected methods lower to ordinary checked functions before erasure. Arguments and runtime effects are preserved once, including mutation in proof-returning methods. Logical results erase without changing the selected implementation. Existing ownership, historical snapshots and tracked-proof invalidation rules apply unchanged.

## Generic bounds

<!-- spec: 1.31:30 legality-rule -->
`T: Trait` requires an implementation for each concrete type argument. `where T: Trait` states the same requirement; `+` combines requirements. Bounds use normal module visibility and imported trait identity. Functions, logical functions, structs, enums and propositions may declare them.

<!-- spec: 1.31:40 example -->
~~~rust check
trait Read { fn read(&self) -> u8; }
struct Counter { value: u8 }
impl Read for Counter {
    fn read(&self) -> u8 { self.value }
}
fn read<T>(value: &T) -> u8 where T: Read {
    value.read()
}
fn example() -> u8 {
    let value = Counter { value: 7 };
    read(&value)
}
~~~

<!-- spec: 1.31:31 legality-rule -->
Generic member selection uses declared bounds before substitution. An inherent method on the eventual concrete type cannot replace that selection. Multiple applicable bounds require qualification. Locals, destructuring and returned values retain the type information used for this selection.

<!-- spec: 1.31:41 example -->
~~~rust check
trait Left { fn value(&self) -> u8; }
trait Right { fn value(&self) -> u8; }
struct Choice {}
impl Left for Choice { fn value(&self) -> u8 { 1 } }
impl Right for Choice { fn value(&self) -> u8 { 2 } }
fn choose<T: Left + Right>(value: &T) -> u8 {
    <T as Right>::value(&*value)
}
fn example() -> u8 { let c = Choice {}; choose(&c) }
~~~

<!-- spec: 1.31:32 legality-rule -->
`T: Source<Item = u8>` fixes an associated type. Without an equality, use `T::Item` or `<T as Source>::Item`. A `where T::Item: Read` predicate constrains that projection. Equality constraints compare types, not propositions. Missing, ambiguous, conflicting or cyclic obligations fail checking.

<!-- spec: 1.31:42 example -->
~~~rust check
trait Source {
    type Item;
    fn item(&self) -> Self::Item;
}
struct Bytes {}
impl Source for Bytes {
    type Item = u8;
    fn item(&self) -> u8 { 9 }
}
fn item<S: Source<Item = u8>>(source: &S) -> S::Item {
    source.item()
}
fn example() -> u8 { let source = Bytes {}; item(&source) }
~~~

<!-- spec: 1.31:33 legality-rule -->
A generic trait implementation covers a complete named family: `impl<T: Read> Read for Wrapper<T>`. Its conditions must hold before that implementation is available. There is at most one implementation per trait and named family, including implementations with different bounds. Partial patterns, blanket implementations over a bare parameter and specialization are deferred.

<!-- spec: 1.31:43 example -->
~~~rust check
trait Read { fn read(&self) -> u8; }
struct Byte { value: u8 }
impl Read for Byte { fn read(&self) -> u8 { self.value } }
struct Wrapper<T> { inner: T }
impl<T: Read> Read for Wrapper<T> {
    fn read(&self) -> u8 { self.inner.read() }
}
fn read<T: Read>(value: &T) -> u8 { value.read() }
fn example() -> u8 {
    let wrapped: Wrapper<Byte> = Wrapper { inner: Byte { value: 4 } };
    read(&wrapped)
}
~~~

<!-- spec: 1.31:34 legality-rule -->
A generic inherent implementation may add conditions on its block or individual methods. A method is available only when its conditions hold. This does not add conditions to construction of the containing type or change the method's result shape. Method-local type parameters remain deferred.

<!-- spec: 1.31:44 example -->
~~~rust check
trait Read { fn read(&self) -> u8; }
struct Byte { value: u8 }
impl Read for Byte { fn read(&self) -> u8 { self.value } }
struct Holder<T> { inner: T }
impl<T> Holder<T> {
    fn read(&self) -> u8 where T: Read { self.inner.read() }
    fn tag(&self) -> u8 { 1 }
}
fn example() -> u8 {
    let holder: Holder<Byte> = Holder { inner: Byte { value: 7 } };
    holder.read()
}
~~~

<!-- spec: 1.31:35 legality-rule -->
Bounds supply callable signatures, not evidence. Callers still supply input proofs; implementations establish output proofs on normal return. `logic fn` retains purity, totality and erasure requirements. Use a declared proof-bearing law to reason about an abstract operation; explicitly folding or unfolding that operation in a generic template is rejected.

<!-- spec: 1.31:45 example -->
~~~rust check
trait Step {
    fn next(n: u8, room: @(n < 255)) -> (out: u8, @(out == n + 1));
}
struct Arithmetic {}
impl Step for Arithmetic {
    fn next(n: u8, room: @(n < 255)) -> (out: u8, @(out == n + 1)) {
        let out = n + 1;
        (out, _)
    }
}
fn advance<S: Step>(n: u8, room: @(n < 255))
    -> (out: u8, @(out == n + 1)) {
    S::next(n, room)
}
fn example() -> u8 {
    let (out, evidence) = advance::<Arithmetic>(7, prove!(7 < 255));
    out
}
~~~

<!-- spec: 1.31:36 legality-rule -->
Body, ownership and proof checking still run for each concrete instantiation. An unused generic body is not a universal theorem. Logical classification belongs to the selected source types; an associated `Logical` requirement is checked even when no associated method is called. Distinct source types remain distinct when both erase to the same marker.

<!-- spec: 1.31:37 legality-rule -->
Trait-bound calls preserve argument evaluation, runtime effects and snapshot dependencies through specialization and erasure. Open generic Rust exports remain unsupported: export must not remove a bound and thereby admit unchecked implementations. Trait method `where` clauses and generic trait-implementation forwarding are also rejected at the Rust export boundary. Concrete checked wrappers may be exported under the existing physical-interface rules.

## Rust interoperability

<!-- spec: 1.31:9 legality-rule -->
A Rust export rejects the whole trait if its interface contains proofs, logical functions/types, logical associated requirements or promises. Actual associated bindings and implementation signatures must also be exportable. Trait proof results are never projected away. Physical implementations forward to their checked bodies. Rust sees required method signatures; Locus defaults are supplied in each generated implementation, not exported as unchecked defaults for arbitrary Rust implementors.

<!-- spec: 1.31:10 legality-rule -->
The supported imported Rust trait subset uses the same concrete implementation and call machinery. It admits safe, non-generic traits with scalar/tuple types, `Self`, associated types/constants and ordinary receivers/references. Every member, including Rust defaults, needs a local body. Imported signatures add no proof laws, logical functions or effect promises. A native compiler probe validates used interfaces against the selected Cargo metadata; unsupported traits remain inspectable and report L0514 when used.

<!-- spec: 1.31:26 example -->
~~~rust prose cargo-fixture-tests-native-imports
import renamed::Surface;
struct Counter { value: u8 }
impl Surface for Counter {
    type Item = u8;
    const LIMIT: u8 = 10;
    fn read(&self) -> u8 { self.value }
}
~~~

<!-- spec: 1.31:11 informative -->
Implementations for primitives, references and built-in containers are deferred under [LOC-262](../roadmap/generics.md#LOC-262). Generic trait parameters, method-local type parameters, supertraits, blanket implementations, specialization, `dyn`, and compiler-integrated trait bounds other than `Logical` remain deferred. Rust named-type instantiation and arbitrary native implementations remain outside this slice. See [language abstractions](../roadmap/generics.md#LOC-22) and its explicit follow-ups and [native interoperability](../roadmap/interop.md#LOC-44).

<!-- spec: 1.31:12 informative -->
Interior mutability needs a broader observation and aliasing model; shared receivers must not be treated as proof of state preservation. That work stays in [LOC-47](../roadmap/memory-layout.md#LOC-47). Trait promises and the export restriction are tracked in [LOC-261](../roadmap/generics.md#LOC-261).
