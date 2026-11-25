# Constants that rustc may never evaluate, read by logical code

- **Kind:** decision
- **Status:** open
- **Blocks:** reading a constant of a type with no values, or one that holds logical data other than proofs, in logical code; the claim that `logic.const.well-founded` closes the route of the review's finding F6
- **Rules:** `logic.const.well-founded`, `logic.ghost.positions-logical`, `core.conversion`

## Question

rustc evaluates an associated constant of a generic `impl` only for a use in code that it compiles, and a use in logical code is not one. The owner decided, on the review of 2026-11-25 (finding F6), that every proof in the initializer of a constant comes from logical code alone, so that no proof rests on a runtime call that nothing forces to return. That rule is in `logic.const.well-founded`.

It covers proofs. The same hazard has two more forms that it does not cover:

- **A constant of a type with no values.** `const NONE: Void = nothing();`, for `enum Void {}` and a `const fn nothing() -> Void { loop {} }`, holds no proof, and its type is a runtime type. Logical code that reads it has a value of `Snap<Void>`, a sum with no variants, and `match Holder::<u32>::NONE {}` proves anything ([`logical-layer/constant-of-a-type-without-values.open.lc`](../litmus/logical-layer/constant-of-a-type-without-values.open.lc)).
- **A logical field of a type with no values,** as a field of type `Never` for `#[derive(Logical)] enum Never {}`, in a runtime struct that a `const fn` returns. No proof is in it either.

Which values does logical code take a constant to have, when rustc may never have computed one?

## What it blocks

Whether logical code may read such a constant at all, and what the review's finding F6 is closed by.

## Options

1. **Logical code reads no constant whose initializer calls a runtime function,** or, narrower, no associated constant of a generic `impl` whose initializer does. A constant built from literals, constructors, and logical code stays readable.
2. **Locus forces the evaluation of every constant that logical code reads,** at the type arguments it reads it at: a new duty on the driver, as the review put it. What rustc does today varies: in this session, rustc 1.97 with a full build reported "constant evaluation is taking a long time" for an associated constant that only a closure that is never called mentioned, inside a function that is never called, and with `--emit=metadata` it reported nothing.
3. **Widen the rule to every value of a logical type in the initializer,** as the review's own sentence did. That covers the logical field of an empty type, not `Void`, and it rejects a constant of Locus's `Vec`, whose contents field is logical, built by `Vec::new()`.

## Log

### 2026-11-25, review fixes

Raised while writing the owner's decision on finding F6 of the adversarial review of 2026-11-25, as a variant that the decided rule does not reach. The open example is `logical-layer/constant-of-a-type-without-values.open.lc`.
