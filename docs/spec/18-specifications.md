+++
id = "language-specifications"
title = "Specifications and implementations"
group = "Now"
spec_chapter = 1
order = 113
route = "specification/specifications.html"
description = "Reviewed interfaces with checked local implementations."
+++

# Specifications and implementations

<!-- spec: 1.29:7 informative -->
A spec separates a reviewed interface from its checked implementation. Functions still require bodies and proofs still require evidence. The separation adds no assumptions to the kernel. Broader [interface](../vision/spec-interfaces.md) and [Rust import](../vision/rust-imports.md) designs describe future extensions.

## Declare a module interface

<!-- spec: 1.29:1 legality-rule -->
`spec mod Name { ... }` and `spec type Name { ... }` contain function, logical-function and constant headers ending in `;`. Members are public by default; member visibility qualifiers are rejected. Specs and their members are concrete, although function lifetime parameters are supported. Generic specs/methods, header bodies, nested items, transparent fields and spec attributes are not supported yet.

<!-- spec: 1.29:8 example -->
~~~rust check
spec mod arithmetic {
    fn next(n: u8) -> (out: u8, @(out == n.wrapping_add(1)));
    const FIRST: u8;
}
impl mod arithmetic {
    const FIRST: u8 = 0;
    fn next(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
        let out = n.wrapping_add(1);
        (out, _)
    }
}
fn use_interface() -> u8 {
    let (value, evidence) = arithmetic::next(arithmetic::FIRST);
    value
}
~~~

## Supply a representation and implementation

<!-- spec: 1.29:2 legality-rule -->
A module spec requires one `impl mod Name` in its lexical parent. A type spec requires one same-named, non-generic struct and one `impl Name` in its declaring module. Its representation has private fields and no visibility or derive attributes; visibility belongs on the spec. Matching members inherit public visibility; unlisted helpers must have no visibility qualifier. Ordinary module privacy applies to representation fields.

<!-- spec: 1.29:9 example -->
~~~rust check
mod percent {
    pub spec type Percent {
        fn zero() -> Percent;
        fn get(&self) -> u8;
    }
    struct Percent { value: u8, valid: @(value <= 100) }
    impl Percent {
        fn zero() -> Percent { Percent { value: 0, valid: _ } }
        fn get(&self) -> u8 { self.value }
    }
}
fn read() -> u8 {
    let value = percent::Percent::zero();
    value.get()
}
~~~

## Check completeness

<!-- spec: 1.29:3 legality-rule -->
All loaded specs require complete unique realizations, even when unused. Repeated spec blocks may add disjoint member names if their kind and visibility agree; duplicates never override. Missing/mismatched definitions and additional exposed members are errors. Additional inherent implementations of a spec type, including through aliases or from another module, are rejected.

<!-- spec: 1.29:10 example -->
~~~rust reject L0511
spec mod counter { fn read() -> u8; }
impl mod counter { }
~~~

## Preserve the exact contract

<!-- spec: 1.29:4 legality-rule -->
Signatures resolve in the completed ordinary module scope. Header and definition signatures must have identical tokens, apart from whitespace and comments. This includes parameter names, logical mode, receivers, lifetimes, input proofs and dependent result types. Header promises become implementation obligations; a decreasing measure belongs on the implementation. A constant header specifies its type, while its implementation supplies the checked value.

<!-- spec: 1.29:11 example -->
~~~rust check
spec mod arithmetic {
    #[no_panic]
    fn increment(n: u8, room: @(n < 255)) -> (out: u8, @(out == n + 1));
}
impl mod arithmetic {
    fn increment(n: u8, room: @(n < 255)) -> (out: u8, @(out == n + 1)) {
        let out = n + 1;
        (out, _)
    }
}
~~~

<!-- spec: 1.29:5 legality-rule -->
Manual implementations pass the ordinary body, effect and proof checks; trusted native bindings cannot realize them. Input proofs remain caller obligations and output proofs require evidence. Struct construction still establishes dependent proof fields. Logical definitions remain transparent, and recursion, mutation and snapshot rules are unchanged.

<!-- spec: 1.29:12 example -->
~~~rust reject L0230
spec mod claims { fn impossible() -> @(1 == 0); }
impl mod claims { fn impossible() -> @(1 == 0) { _ } }
~~~

## Split files and export

<!-- spec: 1.29:6 legality-rule -->
`impl mod Name;` loads `Name.lc` or `Name/mod.lc`, with ordinary missing/ambiguous-file rules. Only declared files participate. `use`, re-exports, package ownership and Rust export restrictions remain unchanged; proof-returning functions can receive data-only facades, while proof inputs or public logical fields remain unexportable.

<!-- spec: 1.29:13 informative -->
The [split-file fixture](../../tests/fixtures/specs/split/export.lc) declares its interface in `export.lc` and supplies definitions in [arithmetic.lc](../../tests/fixtures/specs/split/arithmetic.lc). Its test compiles generated Rust with warnings denied, runs the public functions and rejects access to the private helper.

<!-- spec: 1.29:14 informative -->
General traits, Rust `import` bindings and spec generation are future work in the [ordered plan](../roadmap/interop.md#specifications-and-native-imports).
