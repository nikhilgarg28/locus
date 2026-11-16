# Proof construction (RFC 0003)

- **Kind:** decision
- **Status:** open
- **Blocks:** Writing nontrivial proofs, the format of stored proofs ([#13](proof-search-and-storage.md)), and the kernel RFC's term language

## Question

How proofs are built, beyond the forms [RFC 0002](https://github.com/nikhilgarg28/locus/blob/claude/beautiful-mccarthy-cmtyao/design/rfcs/0002-logical-layer.md) fixes. Those forms are holes (`_` and `prove!`), calls of lemmas, the constructors and matches of `prop` declarations, and the connectives' constructors.

The RFC would cover:
- **Using definitions.** Unfolding and folding logic functions, which RFC 0002 used to spell `fold!` and `unfold!`, and which computation the kernel performs on its own.
- **Rules:** `proofs.elimination`
- **Equalities.** Rewriting a proof along an equality, which RFC 0002 used to spell `rewrite!`. Also how `===` is used, and how the built-in agreement of `==` with `===` is applied: by search, or by bridges the elaborator inserts.
- **Tactics.** How much of Lean's tactic language Locus adopts, if any, as against proof terms plus search.
- **Search.** The interface between a hole and untrusted search: the goal and context a hole gives, what search may return, and how found proofs are stored ([#13](proof-search-and-storage.md)).
- **Shorter forms.** For example, an anonymous constructor for one-arm propositions, `(hp, hq)` for `And::Intro(hp, hq)`. The owner deferred this as polish.
- **Libraries of lemmas.** How they are organized and found.

## What it blocks

Writing nontrivial proofs, the format of stored proofs ([#13](proof-search-and-storage.md)), and the kernel RFC's term language.

## Options

To be laid out in the RFC. Lean's split between proof terms and tactics is the main reference point.

## Log

### 2026-10-26, opened on GitHub as #15

Moved into the repository from [GitHub issue #15](https://github.com/nikhilgarg28/locus/issues/15) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.

### 2026-11-16, conversation with the owner on how to build Locus

Two things the owner said about this RFC's subject, for when it is drafted:

> I do want proofs as local variables. To me a more interesting question is [...] what's the grammar for defining proofs and what are its atoms? Basically, what is the equivalent of lean tactics here? I'd really like it to feel regular Rust code too - definitely something to design later.

And `===` is "the only thing which kernel could use for rewriting/checking" (RFC 0002, decision 70).

A benchmark for the design, from a discussion of panic-freedom among Rust users, is [`contracts/loop-over-stepped-range.open.lc`](../litmus/contracts/loop-over-stepped-range.open.lc): a loop over `(0..buf.len()).step_by(2)` that indexes `buf`. It is plainly in bounds and was offered as tedious to prove so. It needs a specification of what `step_by` yields and a fact carried through the loop. The same discussion asked that proofs that tools find be kept apart from the code people read, which is what a hole and a stored proof already do. WUFFS, which proves bounds and overflow safety with a few named proof steps and no runtime checks, is prior art for a small proof language.

### 2026-11-16, proposal made at the owner's request

The owner asked for an audit of the decisions that are still open, with a proposal for each. This is the proposal for this issue. Nothing in it is decided.

The owner's question: "what library/syntax etc to use for writing proofs - how to make it feel like Rust even though we may be writing equivalent of lean tactics".

**A proof is a Rust expression, and a tactic is a function that returns a proof.** Lean has two ways to write a proof, a term and a tactic script. Locus would have one: the term, in Rust's own expression syntax, with holes, and with a library of procedures that return proofs. Each thing a Lean tactic does has a form that Rust already has. The spellings are illustrative.

| In Lean | In Locus |
|---|---|
| `have h : P := ...` | `let h: @P = ...;` |
| `exact e` | the expression `e` |
| `apply f` | a call with holes: `f(_, _)` |
| `intro x h` | a logical closure: `\|x: Int, h: @(x > 0)\| ...` proves a `forall` with a hypothesis |
| `cases h` | `match h { Or::Left(a) => ..., Or::Right(b) => ... }` |
| `constructor`, `exists 3` | a constructor: `And(p, q)`, `Exists(3) @ p` |
| `induction n` | a recursive `logic fn` with `#[decreases(n)]` |
| `rw [e]` | `h.rewrite(e)`, for an `e: @(a === b)` |
| `calc` | `calc! { a; == b by lemma(x); <= c by _ }` |
| `simp [l1, l2]`, `omega`, `decide` | library functions that return proofs: `simp(&[l1, l2])`, `linear()`, `decide()` |
| `assumption`, `trivial` | `_`, which runs the default search |
| `if h : c then ... else ...` | `if h: c { ... } else { ... }` ([naming-facts-from-control-flow](naming-facts-from-control-flow.md)) |
| `sorry` | `todo_proof!()` ([proof-debugging](proof-debugging.md)) |

- **Why terms.** A term is checked where it stands, reads like the code around it, and uses the scoping that `let` and `match` already have. It needs no second language and no goal state that lives outside the source. An AI writes a term as easily as a script.
- **Where tactics are better, and the answer.** A script survives small changes to a statement, and one `simp` closes many goals. The procedures keep that: `_` and `simp(..)` run again when the statement changes, and what they find is stored ([proof-search-and-storage](proof-search-and-storage.md)), so checking never depends on finding it again.
- **Nothing here is trusted.** Every procedure returns a term that the kernel checks.
- **Options.** (1) Terms and procedures, as above. (2) A tactic block as well, `by { ... }`, whose statements transform a goal. (3) Proofs in Lean files beside the program. The proposal is option 1, with option 2 kept for later as sugar that expands to a term, if long proofs prove too verbose. Option 3 takes the proofs out of the program, against the vision.
- **The strongest argument against option 1** is that a long proof is easier to write against a goal that the tool displays after each step. `show!()` gives that display for a term with a hole in it.
- **Lemmas** are written in Locus first. Bringing a theorem in from Lean's libraries by name is possible in principle, since the logic is Lean's, and is left for the kernel RFC.
