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

### 2026-11-16, proposal made at the owner's request

The owner asked for an audit of the decisions that are still open, with a proposal for each. This is the proposal for this issue. Nothing in it is decided.

In the order of the questions above:
1. **A panic** is anything that unwinds or aborts through Rust's panic machinery: an explicit `panic!`, a failed index, overflow where it is checked, `unwrap` on `None`. A call of `process::exit`, a stack overflow, and running out of memory are not; a collection's methods that can fail to allocate offer a fallible form.
2. **Each obligation is a proof like any other.** `_` finds it where it can, silently; where it cannot, the operation is an error, and the program writes the proof or a check. No new way to prove anything.
3. **A call** is allowed when the callee carries the claim, or is one of the operations the language defines, whose panic condition the language knows. Since RFC 0002's decision 80 there are no specifications, so a standard function without the claim is wrapped, or checked before the call.
4. **Generic code, closures, and trait methods** wait for contracts on trait methods ([traits-on-logical-types](traits-on-logical-types.md)). Until then a `#[no(panic)]` function calls none through a type parameter.
5. **Leaving out the check** waits for [permissioned-pointers](permissioned-pointers.md).
6. **The spelling** `#[no(panic)]`, and `#[no(diverge)]` for termination, so that the two claims read as one family.

The open example [`contracts/loop-over-stepped-range.open.lc`](../litmus/contracts/loop-over-stepped-range.open.lc) is decided by questions 2 and 3.

### 2026-11-17, conversation with the owner on what runtime code tells Locus

[RFC 0004](../design/rfcs/0004-runtime-code.md) settles what arithmetic means outside a `#[no(panic)]` function: the wrapped result, whichever way the crate is built, with nothing owed where it is written. The owner placed the strict rule here: in a `#[no(panic)]` function, each `+`, `-`, and `*` is proved to be in range, since a build with overflow checks could panic there. `wrapping_add` and its kin are then how a function says that wrapping is meant, and would join the understood operations with this claim.

The same RFC gives each understood operation the condition under which it returns. Under this claim, that condition is what the function must prove before the operation.

### 2026-11-17, conversation with the owner on changing a value with an invariant

Assigning a field that a proof reads, and then the proof field again, waits for this claim. The value is broken between the two statements, so nothing between them may panic. The owner: "Maybe someday we will do option 4 too with no panic but not needed now" (RFC 0002, decision 85).
