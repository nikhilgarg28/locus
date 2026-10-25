# Trust model

What a user has to believe for a checked Locus program to mean what its specification says. This page grows only through accepted RFCs.

## Trusted components

Parts of the implementation, Locus's or another project's, whose bugs could make a false claim pass.

| Component | What it must get right | Why it is trusted | What would detect a violation | Added by |
|---|---|---|---|---|
| rustc's type checker and trait resolution | The types of runtime values and places, and the items that paths and method calls resolve to | Locus reasons with rustc's answers rather than computing its own, so that proofs are about the program rustc compiles | Tests comparing the facts Locus receives with the behavior of the compiled runtime program | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| rustc's borrow checker | Ownership in runtime code, and that every place a proof or logical block observes is readable where it is observed | Locus has no permission checker of its own; observations are checked as borrows | Litmus examples that observe moved and mutably borrowed places, which must be rejected | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| `locus-driver` | Reporting rustc's facts faithfully: types, resolutions, and typed bodies | It is the only path from rustc to Locus | Tests that compare its reports with rustc's own output for the same program | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| The parser | Where Locus syntax begins and ends | Lowering and erasure both start from its syntax trees; a misplaced boundary could check one program and emit another | Byte-for-byte round trips, and parsing every file rustc's test suite accepts | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| Lowering to the checking program | Observing every place a proof reads, at the point where it reads it, and otherwise matching the source | rustc can only check what the checking program shows it | Tests of lowering, including litmus examples that must be rejected | [RFC 0001](rfcs/0001-compiler-pipeline.md) |
| Erasure | A runtime program that behaves like the source, apart from the erased logical parts | The runtime program is what runs | Building and running the runtime program against litmus `run` expectations | [RFC 0001](rfcs/0001-compiler-pipeline.md) |

## Assumptions

Facts taken without proof: axioms, specifications of external code, and properties of the platform. None yet.

## Rules

- Every entry has a permanent ID in the first column of its table: `trusted.<name>` for a component, `assumed.<name>` for an assumption, in dotted kebab-case. Like a rule ID, it never changes, and it is never reused once an entry is removed.
- Every entry names the accepted RFC that added it.
- Every entry says what it covers, why it is needed, and what would detect a violation.
- A change that adds to this page, or depends on something not listed here, is a design change and needs an RFC.
