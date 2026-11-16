# Seeing what a proof has to work with

- **Kind:** decision
- **Status:** open
- **Blocks:** Writing proofs in a loop with the checker, by hand or by an AI: without it, a failed `_` says only that it failed

## Question

How a programmer, or an AI writing proofs, finds out what Locus knows at a point in a program and what it is asked to prove there; and whether any of it may be printed when the program runs. The owner raised it:

> how do we make it easy to debug proofs/facts - should we permit printing some, maybe through special macro? Does that break any invariants? What about inspecting all facts in the scope? Any other helpers?

## What it blocks

Writing any proof that `_` does not find at once. Lean's users work in a view that shows the goal and the hypotheses at the cursor, and an AI that writes proofs needs the same information as text.

## What can be shown, and when

- **Nothing logical exists when the program runs.** Proofs and logical values are erased, and a marker carries no proposition and no value. So `{:?}` on a proof prints `Erased` (RFC 0002, decision 76), and there is nothing more to print. A build that kept logical values so that they could be printed would be another program, and the vision's commitment 2 says that erasure changes nothing about what a program does.
- **Everything logical exists when the program is checked.** So seeing it is a matter of diagnostics, not of output.

## Options

1. **Inspection when the program is checked, and only then.** A few forms that produce diagnostics, change nothing about the program or about what is accepted, and are erased.
2. **A debugging build that evaluates logical code when the program runs,** so that a logical value can be printed beside runtime ones. It needs every logical function to be executable, a second meaning for erasure, and a build whose behavior is not the released one's.
3. **Nothing in the language, and an editor view alone.** It serves a person at a keyboard, and not an AI working through the command line, a test log, or a review.

## Proposal

Option 1, with the editor view built on the same queries. The names are provisional.

| Form | Written | What it reports |
|---|---|---|
| `show!()` | where a proof is expected | The proposition required there, and every proof in scope with its proposition. It is not a proof: the check stops there |
| `facts!()` | as a statement | The proofs in scope, and what is known of each local: its value where the language defines it, and which version of each place a proof is about |
| `explain!(h)` | as a statement | The proposition that `h` proves, with the version of each place it reads. This is what shows why a proof about `items` before a `clear` does not apply after it |
| `eval!(e)` | as a statement | The value of a closed logical expression, computed by the checker, as Lean's `#eval` does |
| `todo_proof!()` | where a proof is expected | Nothing. It stands for a proof that is not written yet, so that the rest of a function can be checked first |

Each form keeps five things true:

1. **No effect on the program.** It is erased in every build, and an exported crate contains none.
2. **No effect on what is accepted.** A program is accepted or rejected the same with the reporting forms removed, and proof search never reads them.
3. **No view past privacy.** A report shows a field of a snapshot only where the code at that point could read it (`models.snapshot-privacy`). So a diagnostic does not show another crate's private representation, and no program comes to depend on it.
4. **The same report on every machine,** as acceptance is (vision, commitment 3).
5. **`todo_proof!()` is never part of a result.** A result that relies on one is reported as unfinished, a build refuses it without a flag that says so, and it cannot be exported. It is not `unsafe assume!`, which is a claim meant to stay.

Other helpers, in the order they would help most:

- **A failed `_` reports what `show!()` would,** and then the facts that came closest: a proof of the same proposition about another version, or a lemma whose conclusion matches.
- **A trace of proof search,** on request, for the hole it is asked about.
- **A search for a counterexample.** A proposition that can be evaluated is tried on small values before anyone tries to prove it, which is the cheapest way to find a wrong contract.
- **A runtime check that gives a proof:** `let h = ensure!(i < n);` panics if the condition fails, and otherwise binds its proof. It is the honest way past a proof that will not go through, where `unsafe assume!` is the other. It is `if h: i < n { ... } else { panic!() }` in one line ([naming-facts-from-control-flow](naming-facts-from-control-flow.md)).
- **A list of what a result rests on:** every `unsafe assume!` and every `todo_proof!()`, with its proposition and its place.
- **The goal and the facts at the cursor,** in an editor.

The strongest argument against the proposal is that every form is one more name in the prelude, and that an editor view would cover most of them. The answer is that a proof written by an AI is debugged in text.

## Questions it must answer

1. The forms and their names, and whether they are Locus's macros, with what that means inside another macro's input.
2. Whether `show!()` stops the check or only warns.
3. Whether `todo_proof!()` exists at all, or whether an unfinished proof is always an error.
4. How a version is written in a report, so that a person can tell two versions of one place apart.
5. Whether `ensure!` belongs to the language's library, or is left to `if` with a named condition.
6. Whether a build that evaluates logical code at run time, option 2, is ever wanted.

## Log

### 2026-11-16, conversation with the owner on specifications

Raised by the owner, in the words quoted above, while asking for an audit of the decisions that are still open. The proposal is the reply's, and the owner has not decided on it.
