# Logical code in closures, async blocks, and packed fields, and how expanded macros observe

- **Kind:** decision
- **Status:** open
- **Blocks:** Proofs inside iterator adapters, callbacks, and async code; proofs about packed structs; and the final form of RFC 0001's section on Locus's macros

## Question

Whether and how logical code can appear in places that RFC 0001 (`design/rfcs/0001-compiler-pipeline.md`, proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10)) rejects for now, and one mechanism it still has to settle:

- **Closure bodies and async blocks.**
- **Rules:** `observe.captures.rejected`, `compat.names.macros-expand`, `logic.boundary.calls`
  - An observation inside one would become a capture, and change what the closure or future captures.
  - Under build mode's single lowered text, a ghost local named inside a closure is also a real capture of a zero-sized marker: by reference, unless the closure is `move`. That changes the closure's borrows, though not its layout.
  - RFC 0001 rejects logical code there for now (`observe.captures.rejected`).
- **Packed fields.** A shared borrow of a packed field is an error in Rust, so logical code cannot observe one.
- **How Locus's macros observe places when rustc expands them.**
  - Locus's macros are now real macros that rustc expands wherever it expands macros: inside another macro's input such as `vec[prove!(...)]`, in a macro's output, and under an alias. The examples that used to be rejected are accepted in [#10](https://github.com/nikhilgarg28/locus/pull/10): `litmus/compatibility/locus-macro-inside-rust-macro.accept.lc`, `prove-in-macro-output.accept.lc`, `prove-through-alias.accept.lc`, and `litmus/observation/macro-in-vec-reads-sibling-field.accept.lc`.
  - What remains open is how such an expansion observes precisely. It must not take `&before.a` when `before` is a logical snapshot, which is a marker, but must observe `pair.a` alone when `pair` is a runtime value and `pair.b` is mutably borrowed. A macro sees only its tokens.
  - A spike running now tests:
    - observing whole roots;
    - a side table of the ghost bindings in scope, keyed by source position;
    - rewriting Locus invocations inside other macros' input.
- **Export mode** accepts only Locus macros and ghost-argument calls written directly in the crate's own code (`export.macros-direct`), because erasure removes them as text. Lifting that waits on this issue.

## What it blocks

Proofs inside iterator adapters, callbacks, and async code; proofs about packed structs; and the final form of RFC 0001's section on Locus's macros.

## Options

To be laid out in an RFC. The spike's findings will be linked here.

## Log

### 2026-10-25, opened on GitHub as #8

Moved into the repository from [GitHub issue #8](https://github.com/nikhilgarg28/locus/issues/8) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, comment on GitHub

> Recorded from the review of [#10](https://github.com/nikhilgarg28/locus/pull/10). `logic.boundary.calls` keeps a closure whose parameter carries a forgeable proof from reaching even *specified* standard library code. So `found.map(|p| divide(100, n, p))`, `filter_map`, and `fold` with such closures are rejected today, and users will write these often.
>
> A sound relaxation, for when this issue settles closures: allow passing such a closure to specified generic code whose specification names the values it calls the closure with. `Option::map` calls it with the contained value. Locus would then check each indirect call like a direct one, against that specification. Unspecified code would stay rejected.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.
