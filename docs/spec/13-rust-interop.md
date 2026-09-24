+++
id = "language-rust-interop"
title = "Rust interoperability"
group = "Now"
spec_chapter = 1
order = 112
route = "specification/rust-interop.html"
description = "Readable generated Rust and the privacy boundary that protects verified interfaces."
+++

# Rust interoperability

<!-- spec: 1.0:14 informative -->
Generated Rust contains executable data and code, with erased markers where logical positions remain. Because Rust cannot distinguish proof claims, a verified API must protect the ways a Rust caller can construct or mutate invariant-bearing values.

## Visibility and the boundary with Rust

<!-- spec: 1.17:1 legality-rule -->
Items and fields are private by default; supported Rust visibility forms retain their spelling. A plain `pub` function cannot accept raw evidence, including through a caller-constructible aggregate. Use restricted visibility for Locus-only helpers, or export a validated type with private fields and checked constructors. Rust callers must not be able to replay one erased proof marker as evidence of another claim.

<!-- spec: 1.90:62 example -->
~~~locus run
pub struct Percent { value: u32, valid: @(value <= 100) }
impl Percent {
    pub fn checked(value: u32) -> Option<Percent> {
        if value <= 100 {
            Some(Percent { value, valid: prove!(value <= 100) })
        } else { None }
    }
    pub fn get(&self) -> u32 { self.value }
}
fn demo() -> u32 {
    match Percent::checked(75) {
        Option::Some(percent) => percent.get(),
        Option::None => 0,
    }
}
//~ run: demo() => 75
~~~

<!-- spec: 1.91:29 informative -->
A safe Rust caller can request a checked percentage and read it through the method. It cannot initialize the private fields or obtain unrestricted mutable access to them. Whole-value replacement preserves that boundary even if a later operation panics. This guarantee assumes safe callers and the documented [trusted base](15-scope.md#the-trusted-base).

## What is generated

<!-- spec: 1.19:1 dynamic-semantics -->
`locus rust file.lc` prints a module. `locus build files... --out directory [--name crate]` creates a dependency-free Rust 2024 crate, with a generated root and one public module per input. Logical positions share a private marker. Source control flow, names, methods, visibility, and declaration order are retained where possible; unnecessary `mut` and dead marker bindings are removed.

## Checking agreement with Rust

<!-- spec: 1.19:2 dynamic-semantics -->
The test harness compares the check-IR interpreter, erased interpreter, and compiled Rust with overflow checks both enabled and disabled. Outcomes include results, panic messages and completed mutable updates. Exhausted interpreter fuel is inconclusive, never a passing result. Random generated programs exercise arithmetic, branches, loops, mutation, and borrowed calls.

<!-- spec: 1.91:30 informative -->
See [correctness](../correctness.md) for the testing strategy and remaining preservation obligations. Generated code is ordinary Rust, but Locus is not a Rust superset: native adapters and generic ABI compatibility remain limited to the documented forms.
