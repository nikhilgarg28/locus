# Logical never: generic kind and faithful lowering

- **Kind:** decision
- **Status:** open
- **Blocks:** Generic never-type uses and the checking/export implementation of logical impossibility
- **Rules:** `logic.types.never`, `logic.ghost.erased`, `proofs.elimination`, `logic.fn.total`

## Question

RFC 0002 decision 126 records the owner-chosen interface:

```rust
pub logic fn unreachable(p: @False) -> ! { match p {} }
```

The direct logical result and its coercion to an expected logical type are
decided. The same spelling in executable Rust must retain genuine non-return.
The owner explicitly left its treatment in `Logical` bounds and lowering for
validation. What is the exact generic kind/type representation that admits these
uses without turning executable `!` into an inhabited ghost marker?

## What it blocks

- Whether an explicit generic argument `!` satisfies `T: Logical`, and how
  marker trait/type identity rules apply to it.
- Whether a logical struct or generic container may use `!` as a field/type
  argument, and what `Snap<!>` means. Direct result support settles none of
  these cases.
- A faithful typing skeleton, erasure and export: no erased logical call may
  initialize executable data, hide a live runtime path, or change Rust drop,
  divergence, inference or trait selection.

Ordinary Rust uses of `!`, including type arguments accepted by the selected Rust toolchain, remain ordinary Rust. These gates concern the new logical uses and their representation.

The admitted `False` elimination is not waiting on another proof-search feature.
The compiler and kernel implementation must establish these representation
properties before claiming support. The unrelated
[unevaluated-constant problem](constants-rustc-may-not-evaluate.md) remains open:
a diverging Rust constant does not become a supplied value of an empty type.

## Options

1. Keep direct logical result/coercion support special, and restrict the generic
   and aggregate uses until a checked encoding has been demonstrated.
2. Give the existing empty type explicit kind-sensitive generic evidence and
   lower every occurrence according to its phase. Demonstrate that ordinary
   Rust `!` remains uninhabited in both build programs and in exported Rust.
3. Internally distinguish a logical empty representation from executable Rust
   `!`, while preserving the approved source interface. Specify identity and
   normalization before permitting generic instantiation across that boundary.

None of these options permits runtime loops or panic to establish logical
termination, witness extraction from an existential proof, or a forged proof
of `False`. The direct semantic interface remains the one the owner adopted.

## Verification gates

Kernel-check the empty eliminator and ordinary inductive definitions; add
accept/reject compiler tests for the decided `litmus/proofs/*never*` cases.
Compare typing, control flow and erasure in debug/release and exported Rust.
Decide the linked generic/aggregate `.open` examples only with owner approval.
Do not report metadata/litmus validation as executing a Locus program.

## Log

### 2026-11-25, owner I06 discussion and PR #26

The owner chose ordinary inductive `Seq` and `Maybe`, then asked for a function
taking a proof of false and returning never. After selecting the name
`unreachable`, the owner said "Let's do that" and asked whether never is both
logical and runtime. The explanation retained runtime non-return and identified
generic kind/lowering as a gate. The owner then requested a commit and PR update.
This issue retains that gate without undoing the selected logical interface.
