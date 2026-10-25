# Trust model

What a user has to believe for a checked Locus program to mean what its specification says. This page grows only through accepted RFCs.

## Trusted components

Parts of the implementation, Locus's or another project's, whose bugs could make a false claim pass.

| Component | What it must get right | Why it is trusted | What would detect a violation | Added by |
|---|---|---|---|---|
| rustc's type checker and trait resolution | The types of runtime values and places, and the items that names and method calls resolve to | Locus reasons with rustc's answers rather than computing its own, so that proofs are about the program rustc compiles | Tests comparing the facts Locus receives with the behavior of the compiled runtime program | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| rustc's borrow checker | Ownership in runtime code, and that every place logical code reads is readable where it is read | Locus has no permission checker of its own; observations are checked as borrows | Litmus examples that read moved and mutably borrowed places, which must be rejected | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| `locus-driver` | Reporting rustc's facts faithfully, for the program it compiled, and failing when it cannot | It is the only path from rustc to Locus | Tests comparing its reports with rustc's own output for the same program | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| The facts format | Describing types, items, resolutions, and typed bodies without losing what the logic depends on | Everything Locus knows about runtime code passes through it | Round-trip tests, and tests that each fact matches the compiled program | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| Locus's library | Probes and stand-ins that mean what the lowering assumes | The driver recognizes probes by their definitions there | Tests of each probe against rustc | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| The parser | Where Locus syntax begins and ends | Lowering and erasure both start from its syntax trees; a misplaced boundary could check one program and emit another | Byte-for-byte round trips of rustc's test suite parsed as `.lc` files | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| Lowering to the checking program | Observing every place logical code reads, where it reads it; probing every name it uses; and otherwise matching the source and the runtime program | rustc can only check what the checking program shows it | Tests of lowering, including litmus examples that must be rejected | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| Erasure | A runtime program with the same types as the checking program, that behaves like the source apart from erased logical code and source locations, with a guard for every unchecked configuration | The runtime program is what runs | Building and running the runtime program against litmus `run` expectations | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| The `locus` orchestration | Pairing facts with the program they describe, checking every declared configuration, and stopping when rustc rejects the checking program | It decides what a successful check means | Tests that stale or mismatched facts, and failing configurations, fail the check | [RFC 0001](rfcs/0001-compiler-pipeline.md) |

## Assumptions

Facts taken without proof: axioms, specifications of external code, and properties of the platform and the build.

| Assumption | Why it is needed | What would narrow it | Added by |
|---|---|---|---|
| The compiler that builds the runtime program resolves names and types as the pinned rustc did | The runtime program may be built by a different release than the one that checked it | Following stable releases closely; fully qualified paths where resolution could change ([#7](https://github.com/nikhilgarg28/locus/issues/7)) | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| Dependencies resolve to versions that behave as the checked ones did, for everything the logic relies on | A crate is checked against one resolution of its dependencies | Exact versions for Locus dependencies ([#7](https://github.com/nikhilgarg28/locus/issues/7)) | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| Macros that read the environment or files (`env!`, `include!`, procedural macros) produce the same code when the runtime program is built as when it was checked | A configuration guard cannot test what a macro will read | Recording the inputs a check saw ([#7](https://github.com/nikhilgarg28/locus/issues/7)) | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| rustc compiles the runtime program as Rust specifies | The runtime program is ordinary Rust | Nothing in Locus; this is the trust every Rust program places in its compiler | [RFC 0001](rfcs/0001-compiler-pipeline.md) |

## Rules

- Every entry has a permanent ID in the first column of its table: `trusted.<name>` for a component, `assumed.<name>` for an assumption, in dotted kebab-case. Like a rule ID, it never changes, and it is never reused once an entry is removed.
- Every entry names the accepted RFC that added it.
- Every entry says what it covers, why it is needed, and what would detect a violation or narrow it.
- A change that adds to this page, or depends on something not listed here, is a design change and needs an RFC.
