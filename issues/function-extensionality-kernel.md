# Kernel realization of logical function extensionality

- **Kind:** design
- **Status:** open
- **Blocks:** Implementing the adopted explicit `eq::funext` theorem
- **Rules:** `logic.eq.same`, `proofs.equality.funext`, `core.conversion`

## Scope

The owner adopted I07a: for pure logical `f, g: logic fn(A) -> B`, pointwise
`===` at every input implies `f === g`, with `A: Logical, B: Logical`. RFC 0002 decision 127 and RFC 0003 record
`eq::funext(f, g, pointwise)` as the explicit API. This issue is implementation
and metatheory work, not a reopening of that semantic decision.

The existing `assumed.lean` entry names Lean 4's logic with propositional
extensionality, quotients, and classical choice. Functional extensionality is
available as a derived theorem under that baseline. The kernel backend is not
selected, and no Locus kernel implementation has checked this theorem yet.

## Required evidence

- Supply the theorem's complete core proof, or a checked import with an explicit
  translation from Locus logical function application and equality. Enumerate
  its actual foundational dependencies. Do not add an independent unchecked
  axiom or use `unsafe assume!` to stand in for that proof.
- Preserve propositional equality versus conversion. A caller uses the theorem
  explicitly; conversion, `compute`, `linear`, and proof holes keep their
  current meanings. No equality reflection is introduced.
- Validate generic logical function values and logical closures, including
  captured logical values and higher-order observers. The interpretation of
  logical functions may not expose a code identity that distinguishes values
  proved equal by this theorem.
- Retain the ordinary runtime meaning of Rust functions, function pointers,
  closures and their effects. No logical theorem may erase or identify their
  executable representation, and no unsupported capture is created for them.
- Run the accepted and rejected `litmus/proofs/funext*` cases once a compiler
  exists. Metadata checks alone do not discharge these obligations.

## Out of scope

The I07b representation of finite `Map`, enumeration, finite support, quotienting
of map representations, dependent function surface syntax, and selection of a
kernel backend remain separate decisions. The nondependent theorem does not
choose any of them, and does not settle the I06 generic `!: Logical` gate. No change to capture defaults or the trait inventory is
needed here.

## Log

### 2026-11-26, owner I07a discussion and PR #26

The owner adopted explicit functional extensionality for pure logical functions
and requested that it be recorded before discussing I07b. This issue records the
proof and compiler validation work without silently selecting a backend or
adding a new trusted equality axiom.
