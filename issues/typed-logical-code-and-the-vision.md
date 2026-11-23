# Logical code that rustc types, and the vision

- **Kind:** decision
- **Status:** open
- **Blocks:** Accepting RFC 0001 as it now reads, since two sentences of the vision say otherwise, and with it the RFC on traits, which rests on that lowering
- **Rules:** `compat.superset.same-meaning`, `compat.names.reserved`, `logic.ghost.erased`

## Question

RFC 0001's decisions 49 to 52 change how logical code reaches rustc. Two sentences of the vision no longer describe the design, and only the owner edits the vision. What should they say?

**Commitment 6.** It says:

> rustc never runs logical code or holds a logical value: in the program it compiles, proofs and logical values are zero-sized markers, logical code reaches it only as input to Locus's macros, and logical items only as stubs that resolve names.

Under decision 49, rustc still never runs logical code, and still holds only zero-sized markers. Two clauses are no longer true.

- **"logical code reaches it only as input to Locus's macros".** Each piece of logical code now also reaches rustc as a Rust expression over stubs, its typing skeleton, in a closure that is never called. rustc types it, resolves its names and its traits, and borrow-checks it.
- **"logical items only as stubs that resolve names".** A logic function's stub now has the function's real signature, and logical code is typed against it. Each logical type is a zero-sized Rust type of its own, where it was an alias of one marker.

**Commitment 1.** It says:

> The one exception is a program that tests `__locus_check`, the `cfg` name Locus reserves for its checking compile.

RFC 0001 now has a second exception. A program may not write an identifier that begins with `__logic_` or `__locus_` (`compat.names.reserved`):

```rust prose valid Rust, which the rule rejects
fn __logic_answer() -> u32 {
    42
}
```

The owner adopted a principle on the names that lowering writes, and did not see a reserved prefix. It comes from where the stubs of a trait's logic functions live: in the lowered trait, under a name that every crate must agree on.

## What it blocks

Nothing is blocked from being written. RFC 0001, its rules, and its examples are as decisions 49 to 52 have them, as proposed. Accepting RFC 0001 while the vision says otherwise would leave two normative sources in disagreement, so the acceptance waits for the owner's words. The RFC on traits rests on this lowering.

## Options

**For commitment 6,** proposed words, for the owner to take or change:

> rustc never runs logical code or holds a logical value: in the program it compiles, proofs and logical values are zero-sized markers, logical code reaches it only as code that it types and never runs, and logical items only as stubs that are never called.

Or, to say less about how:

> rustc never runs logical code or holds a logical value: in the program it compiles, proofs and logical values are zero-sized markers, and nothing of the logical layer is ever executed.

**For commitment 1:**

- **Name the reserved names as the one exception.** Proposed words: "The one exception is a program that writes a name that Locus reserves: `__locus_check`, the `cfg` name of its checking compile, or an identifier that begins with `__locus_` or `__logic_`."
- **Reserve nothing more, and keep the stubs out of a program's way by another route.** [marker-lowering](../spikes/marker-lowering/README.md) tried one, a hidden trait beside the lowered one, and an import of the trait broke it. With mangled names and no rule, a program's own method of the same name is rustc's ambiguity (E0034) or a duplicate definition (E0428), about a name that the program never saw in its own source (`r07`).
- **Reject only a name that meets one of Locus's.** A program may then write `__logic_x` where no stub has that name, and whether it is an error depends on what a trait of another crate declares.

Commitment 7 says that Locus builds programs "by lowering them to Rust, in which proofs are zero-sized markers". That still holds.

**A third point is not a disagreement in words.** Commitment 2 says that proofs cost nothing at run time. In build mode a typing skeleton is a closure that is built, dropped, and never called, so no logical code is evaluated. No spike measured whether an unoptimized build keeps anything of those closures, or the size of a binary with them. It is recorded here so that the measurement is not forgotten.

## Example

`litmus/compatibility/reserved-prefix-in-a-name.reject.lc` is decided as rejected, as `compat.names.reserved` has it, and `litmus/compatibility/name-resembling-reserved-prefix.accept.lc` is its accepted neighbor. The first changes its outcome if the owner chooses the second or third option for commitment 1.

## Log

### 2026-11-23, raised while writing RFC 0001's decisions 49 to 52

The owner chose the lowering in which each logical type is a Rust type of its own and rustc types logical code, and adopted the principle that every name lowering writes and rustc can show reads as the programmer thinks. Neither answer spoke of the vision. Writing them into RFC 0001 found the two sentences above. `VISION.md` is not edited: RFC 0001 lists both points under what is still for the owner, as items 6 and 10.
