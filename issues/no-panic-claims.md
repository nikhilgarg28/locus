# A claim that a function cannot panic

- **Kind:** decision
- **Status:** open
- **Blocks:** Code for targets where a panic is not acceptable; leaving out the runtime checks that a proof makes unnecessary
- **Rules:** `proofs.partial`

## Question

A proof that a runtime function returns holds when the function returns, and a panic proves nothing (`proofs.partial`). The vision's commitment 9 says that stronger claims are separate and explicit. This issue is the design of one of them: that a function cannot panic.

The owner put it in the plan as an attribute for this one claim (RFC 0002, decision 71):

```rust prose proposed, not yet Locus syntax
#[no(panic)]
fn get(xs: &[u8], i: usize, in_bounds: @(i < xs.len())) -> u8 {
    xs[i]
}
```

In such a function, every operation that can panic is an obligation that it will not: an index is proved in range, an `unwrap` is proved to hold a value, a division is proved to have a nonzero divisor, an addition is proved not to overflow, and an explicit `panic!` is proved unreachable. A failed obligation is an error at that operation. The function calls only functions that carry the claim, or library functions with a specification that says as much.

It is not an effect system. One fixed claim, checked function by function, needs no effect variables, as `const fn` needs none.

## What must be decided

1. **What counts as a panic.** Indexing, slicing, `unwrap` and `expect`, division, overflow under every build profile, `RefCell` borrows, and capacity overflow. Whether an abort counts: running out of memory or stack is not a panic.
2. **How an obligation is discharged.** Whether search proves it from the facts in scope with nothing written, as it does for a struct built with a hole, or whether the source writes a proof at each operation.
3. **Calls.** Which library functions get a specification that they cannot panic, perhaps under a precondition, and what a destructor that runs in such a function must show.
4. **Generic code, closures, and trait methods.** A generic function cannot panic only if what it is given cannot. That needs a way to say "if the argument has the claim", which is where a single built-in effect starts to need more. Whether a trait method can carry the claim for every implementation.
5. **Leaving out the check.** A proved index is a place where the bounds check can be left out, as the vision's commitment 2 promises "over time". The guarantee wanted is that the check is gone because of the proof, not because an optimizer happened to remove it. It rests on a contract for the unchecked operation ([boundary-contracts](boundary-contracts.md)), as the pointer primitives of [permissioned-pointers](permissioned-pointers.md) do.
6. **The spelling,** `#[no(panic)]`, and whether the same form states termination, as `#[no(diverge)]` ([termination-claims](termination-claims.md)).
7. **What it removes elsewhere.** Code that cannot unwind needs no rule about what a destructor sees after a panic.

## Example

[`contracts/loop-over-stepped-range.open.lc`](../litmus/contracts/loop-over-stepped-range.open.lc): a loop that is plainly free of panics and was offered, in a discussion among Rust users, as tedious to prove so.

## Prior art

- **SPARK** has absence of run-time errors as its own level of assurance, below functional correctness, and it is the level most projects aim for.
- **WUFFS** proves bounds and overflow safety at compile time, with no runtime checks, for parsers of untrusted data.
- **`no-panic`,** a Rust crate, fails the build when the optimizer does not remove every panic path, so its answer changes with the compiler and the optimization level.
- **Verus** requires such obligations of every verified function.

## Log

### 2026-11-16, conversation with the owner on how to build Locus

Raised after reading a discussion among Rust users of "Memory Safety Bugs in Rust Standard Library" (Microsoft FORGE Lab, 2026), in which the most common request was a way to guarantee that a panicking branch does not exist, without trusting the optimizer to remove it, especially for embedded and kernel code. The owner asked:

> Should we have '#[no(panic)]' or some macro like that on methods which the compiler will complain about unless it can be proven? I initially thought about having full effect system but that feels like an overkill when all we need is panic guarantees.

and then decided: "Yes to #no(panic) in plan". The questions above are what was said in reply, and none is decided. On how hard the claim is to prove: it is undecidable in general, which does not matter for a claim that a function opts into; it suits self-contained code such as parsers, drivers, and cryptography, and is not a sensible default.
