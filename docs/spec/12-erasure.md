+++
id = "language-erasure"
title = "Erasure"
group = "Now"
spec_chapter = 1
order = 111
route = "specification/erasure.html"
description = "What disappears, what remains, and why erasure must preserve observable effects."
+++

# Erasure

<!-- spec: 1.0:13 informative -->
Verification happens before logical distinctions disappear. Erasure removes logical values and computation while preserving runtime control flow, evaluation order, mutation, and panic. The result type alone never decides whether an expression executes.

## Values and effects

<!-- spec: 1.18:1 dynamic-semantics -->
Logical positions lower to one private zero-sized `Erased` marker. Logical declarations and computations have no runtime implementation. Physical aggregates keep their positions with logical fields replaced by markers. Runtime enum tags and physical storage remain; a Box stays physical even with a Logical payload.

<!-- spec: 1.18:2 dynamic-semantics -->
A logical operation erases after evaluating any ordinary argument computations in source order. An ordinary function remains executable even if every argument and result is logical. In particular, a function taking `&mut` and returning only evidence still performs its mutation.

<!-- spec: 1.90:60 example -->
~~~rust run
fn clear(value: &mut u8) -> @(value == 0) {
    value = 0;
    _
}
logic fn reuse(p: Prop, evidence: @p) -> @p { evidence }
fn demo() -> u8 {
    let mut value: u8 = 9;
    let ignored = reuse(prop!(true), {
        clear(&mut value);
        True::Intro
    });
    value
}
//~ run: demo() => 0
~~~

<!-- spec: 1.91:28 informative -->
The ordinary block passed as an argument runs and clears `value`. The subsequent logical `reuse` call disappears. Putting the same `clear` call inside `logic { ... }` would be rejected, because explicit logical computation cannot perform the mutation.

## Removing unused markers

<!-- spec: 1.18:3 dynamic-semantics -->
Generated Rust removes unused marker bindings and unused erased pattern components. A logical local borrowed by runtime code retains zero-sized marker storage for that borrow. An initializer with runtime effects remains as a statement. An initializer that transfers control remains the block’s terminal expression. Cleanup must preserve behavior and produce warning-clean Rust, rather than suppressing unused-variable warnings globally.

<!-- spec: 1.93:1 informative -->
The [Rust export facade](17-modules.md#proof-returning-functions) is a separate, one-way return projection. Internal erased signatures and layouts remain intact; only the additional public entry omits the supported proof result positions.

## Runtime choices with proof payloads

<!-- spec: 1.18:4 dynamic-semantics -->
Runtime code cannot observe proof contents. Proof matches establish further evidence; they do not extract witnesses as data. Physical enums keep their discriminants while proof payloads erase. This includes `Option<@P>` when `P` mentions an in-scope snapshot. Only the payload erases: `Some` and `None` remain distinct. Logical type arguments add no runtime storage or computation; ordinary calls inside payload construction still run.

<!-- spec: 1.90:61 example -->
~~~rust run
enum Checked {
    Zero,
    Positive { value: u8, evidence: @(value > 0) },
}
fn certify(n: u8) -> Checked {
    if n > 0 {
        Checked::Positive { value: n, evidence: prove!(n > 0) }
    } else { Checked::Zero }
}
fn accepted(n: u8) -> bool {
    match certify(n) {
        Checked::Positive { value: _, evidence: _ } => true,
        Checked::Zero => false,
    }
}
//~ run: accepted(0) => false
//~ run: accepted(8) => true
~~~
