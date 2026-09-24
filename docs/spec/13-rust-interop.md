+++
id = "language-rust-interop"
title = "Rust interoperability"
group = "Now"
spec_chapter = 1
order = 113
route = "specification/rust-interop.html"
description = "Readable generated Rust and the privacy boundary that protects verified interfaces."
+++

# Rust interoperability

<!-- spec: 1.0:14 informative -->
Generated Rust contains executable data and code, with erased markers where logical positions remain. Because Rust cannot distinguish proof claims, a verified API must protect the ways a Rust caller can construct or mutate invariant-bearing values.

## Visibility and the boundary with Rust

<!-- spec: 1.17:1 legality-rule -->
Source visibility controls Locus access. The selected [export entry](17-modules.md#rust-export-entries) grants Rust access only to exportable interfaces. Logical inputs, non-proof logical outputs, and logical public fields or enum payloads are rejected. Ordinary functions may export proof results through the data-only facade described below. Export validated physical types with private invariant-bearing fields and checked methods. Legacy flat-file emission retains its older evidence-input restriction; use the project build path for the complete same-crate boundary.

<!-- spec: 1.90:62 example -->
~~~rust run
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

## Returning proofs to Locus and data to Rust

<!-- spec: 1.93:2 informative -->
An ordinary function can serve both callers without a handwritten wrapper:

<!-- spec: 1.93:3 example -->
~~~rust check
pub fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    let out = n.wrapping_add(1);
    (out, _)
}
fn locus_caller(n: u8) -> u8 {
    let (out, evidence) = increment(n);
    out
}
~~~

<!-- spec: 1.93:4 informative -->
When selected by an export entry, its Rust API is equivalent to:

<!-- spec: 1.93:5 informative -->
~~~rust prose projected-signature-covered-by-tests-export-facades
pub fn increment(n: u8) -> u8;
~~~

<!-- spec: 1.93:6 informative -->
Locus retains the complete signature and checks the returned evidence. Generated internal calls use the implementation returning `(u8, Erased)`. Rust calls the second entry, which invokes that implementation once and returns the byte. A proof-only ordinary result becomes `()`, even when its function mutates data or panics. [The return projection rules](17-modules.md#proof-returning-functions) specify tuple shapes and restrictions.

<!-- spec: 1.93:7 informative -->
Neither a public proof-taking parameter nor a public proof field becomes legal. A physical enum with a proof payload also remains unexportable; removing its payload would change a nominal interface. Non-proof logical results such as Nat and Prop are outside this convenience.

## What is generated

<!-- spec: 1.19:1 dynamic-semantics -->
`locus build ENTRY --out-dir DIR` writes an includable Rust 2024 component and receipt. The public facade and private backend preserve the checked export boundary. `locus rust` prints the selected component for module/export entries. The legacy flat-file `build files... --out DIR` path still creates a crate. Both preserve executable control flow and remove dead marker bindings without discarding runtime effects.

## Checking agreement with Rust

<!-- spec: 1.19:2 dynamic-semantics -->
The test harness compares the check-IR interpreter, erased interpreter, and compiled Rust with overflow checks both enabled and disabled. Outcomes include results, panic messages and completed mutable updates. Exhausted interpreter fuel is inconclusive, never a passing result. Random generated programs exercise arithmetic, branches, loops, mutation, and borrowed calls.

<!-- spec: 1.91:30 informative -->
See [correctness](../correctness.md) for the testing strategy and remaining preservation obligations. Generated code is ordinary Rust, but Locus is not a Rust superset: native adapters and generic ABI compatibility remain limited to the documented forms.
