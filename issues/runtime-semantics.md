# Runtime semantics of checked code

- **Kind:** decision
- **Status:** open
- **Blocks:** Any proof about what runtime code computes; freezing the facts format and the choice of THIR or MIR in RFC 0001; the planned semantic slice: verifying one small mutable function with a reborrow and a destructor end to end through the pipeline, with its certificate and a replay of the stored proof. It is meant to run alongside this RFC, to expose the pipeline's constraints early
- **Rules:** `proofs.versions`, `contracts.final`, `logic.boundary.escape`, `logic.unsafe.escape`

## Question

How rustc's typed bodies become statements in the logic: the **runtime semantics** of checked code. RFCs 0001 and 0002 ([#10](https://github.com/nikhilgarg28/locus/pull/10)) fix the interface, and leave this to its own RFC.

An external review asked for this RFC early, before the pipeline's representations are frozen, because it decides whether several of their commitments can be implemented as stated. It needs to answer:

- **Versions.** RFC 0002 says every write makes a new version, as in SSA, and that propositions are about versions. This RFC decides how versions are formed from typed bodies:
  - at branch joins and in loops;
  - for partial moves and reborrows;
  - which facts relate one version to another.
- **Mutation and `&mut`,** for example through Aeneas's functional reading.
  - RFC 0002 fixes the meaning in signatures: a parameter means its value at entry, and `final!(r)` is a mutable borrow's final value, a prophecy.
  - This RFC decides how bodies are translated, including where each borrow ends, which is where prophecies are resolved.
  - rustc computes where borrows end on MIR. Drops, unwinding, and borrows pull toward MIR; source-shaped typed expressions pull toward THIR.
- **What remains true after an opaque call,** and what is forgotten:
  - a completable value lent mutably to unchecked code comes back as any value of its type;
  - values read through raw pointers, or cast or transmuted, are forgotten.
- **Drops and unwinding.**
  - Which obligations hold on unwind.
  - How explicit and implicit drops affect postconditions, which RFC 0002 checks after the locals' drops.
  - Whether a type's invariant may be broken temporarily, inside an operation whose contract restores it on return and on unwind ([#18](external-review-ideas.md)).
- **Loops and loop invariants,** including tracked evidence (`let mut h: @P`), which RFC 0002 defers here.
- **Integer overflow,** where proofs must hold under every build profile. The `overflow-checks` setting is not a stable `cfg`, so no configuration guard can test it ([#9](cfg-corner-cases.md)).
- **Panics, and calls Locus cannot translate.**
- **Narrower observations:** whether reading a proof field can observe only the fields its proposition reads, rather than the whole value.
- **Obligation generation.** Turning claims and typed bodies into the obligations the kernel checks is trusted (`trusted.obligations` in `design/TRUST.md`). This RFC decides how much of it is, and how that trust is reduced. The kernel establishes the theorem it is given; the translation must establish that the theorem describes the program.

## What it blocks

- Any proof about what runtime code computes.
- Freezing the facts format and the choice of THIR or MIR in RFC 0001.
- The planned semantic slice: verifying one small mutable function with a reborrow and a destructor end to end through the pipeline, with its certificate and a replay of the stored proof. It is meant to run alongside this RFC, to expose the pipeline's constraints early.

## Options

To be laid out in the RFC. The choice between THIR and MIR as the source is part of it.

## Example

None yet. The stale-proof examples in `litmus/proofs/` and `litmus/contracts/`, and `litmus/observation/proof-before-move.accept.lc`, rely on the minimum facts that RFC 0002 requires of whatever this RFC decides.

## Log

### 2026-10-25, opened on GitHub as #6

Moved into the repository from [GitHub issue #6](https://github.com/nikhilgarg28/locus/issues/6) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.
