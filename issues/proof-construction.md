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

### 2026-11-16, conversation with the owner on the proof library

The owner set a requirement for the library that proofs are written with:

> For proof writing, can you propose how the library functions may look like? I want it to feel like a library of regular functions, not an embedded DSL.

The reply revises the proposal of the previous entry in that direction. It is still a proposal.

- **Everything is a function, a method, an enum, or a closure.** The macros that the previous entry and RFC 0002 mention for proofs, `calc!`, `rewrite!`, `fold!`, and `unfold!`, become methods and functions: `ab.then(bc)` chains two equalities, `h.rewrite(e)` rewrites, and `eq::by_def()` proves an equality that holds by unfolding definitions.
- **A lemma is a logic function** in a module named for what it is about: `int::add_comm(a, b)`, `seq::len_push(s, x)`.
- **A procedure is a function whose result is generic in the proposition,** which is inferred from where the call stands, as the result of `collect()` or `Default::default()` is: `let h: @(lo <= m) = arith::linear();`. Its signature is ordinary. Its body is the checker's, as the body of `size_of::<T>()` is the compiler's.
- **Three things need the language, and nothing else does:**
  1. procedures, as above, which run when the program is checked;
  2. inferring the predicate that `rewrite` and `congr` are generic in;
  3. values that a lemma's caller does not write, such as the `a`, `b`, and `c` of a transitivity lemma. The proposal is to write them as generic parameters, `logic fn lt_trans<a: Int, b: Int, c: Int>(h1: @(a < b), h2: @(b < c)) -> @(a < c)`, inferred as type parameters are and given with a turbofish when they cannot be.

### 2026-11-17, conversation with the owner on terms and procedures

The owner compared two ways of writing a proof: terms built by calling functions, and tactics that change a goal. After seeing a proof written at each level, down to a pure term with no search in it, the owner decided how the two meet:

> I can do option 3 but I want to keep a relatively tightly scoped list of procedures and have clear rules on what each does + how to compose them with terms.

Option 3 was: a procedure call may stand in the source, and a command replaces any call by the term it found, when the author wants it kept. So:

- **Decided:** proofs are terms; a small, closed list of procedures may be called where a proof is expected; each has a stated rule; and every call can be expanded into a term that calls no procedure.
- **Not decided:** the list, the rules, and the spellings. The owner asked for a proposal of the whole machinery, which follows.

**Proposal.** Three layers, of which only the last searches.

*1. Term forms, which are the language's.* RFC 0002 has all but the last two.

| Form | Proves |
|---|---|
| a local or a parameter | what its type says |
| a call, `lemma(x, h)` | a lemma's conclusion; a `forall` at a value; the conclusion of an implication |
| a closure, `\|x: T, h: @P\| ...` | a `forall`, or an implication |
| a constructor, `And::Intro(p, q)`, `Or::Left(p)`, `Exists::Witness(v) @ p`, `Name::Arm(w) @ p` | a conjunction, a disjunction, an `exists`, a declared proposition |
| `match` on a proof, and `let` for a proposition with one arm | by cases; opens a witness |
| `let h = ...;` | names a step |
| recursion, with `#[decreases(e)]` | by induction |
| `if h: c { ... } else { ... }` | by cases on a condition, with the fact named ([naming-facts-from-control-flow](naming-facts-from-control-flow.md)) |
| `_` where a value is expected | a value that the checker infers from the proofs beside it, as in `lt_trans(_, _, _, ab, bc)` |

*2. Library functions that search for nothing.* Each is a lemma or a rule, with a predictable name.

- **`logic`:** `p.left()`, `p.right()`, `absurd(f)`, `contradiction(p, not_p)`, `by_contra(...)`, `excluded_middle()`, `if_true(h)`, `if_false(h)`.
- **`eq`:** `refl(x)`, `e.symm()`, `e.then(f)`, `congr(f, e)`, `h.rewrite(e)`, which replaces every occurrence, `h.rewrite_at(e, pattern)` for one, and `unfold(f)`, the defining equation of a logic function.
- **`int`, `nat`:** the laws of arithmetic and order, each one lemma, named as Lean names them: `add_comm`, `add_assoc`, `mul_add`, `le_trans`, `lt_of_lt_of_le`, `le_antisymm`, `lt_or_ge`, `add_lt_add_left`, `sub_pos`, `div_add_mod`, `mod_lt`.
- **`seq`, `set`, `map`:** what each operation does to length and contents, and equality by contents.
- **Induction principles** as functions that take closures: `nat::induction(base, step)`.

*3. Procedures.* The list is closed: a program cannot add one, and writes a lemma instead.

| Procedure | Proves | Uses | Fails when |
|---|---|---|---|
| `compute()` | A proposition whose two sides evaluate to the same thing, or a closed proposition that evaluates to true | Definitions only | The sides do not reduce to the same form |
| `linear(facts)` | A goal built from `+`, `-`, multiplication, division and remainder by a literal, comparisons, `&&`, `\|\|`, `!`, and `==>`, over `Int` and `Nat` | The facts it is given, and nothing else. Any other subterm is an atom, and two atoms are the same when a given equality makes them so | The goal does not follow from those facts by linear arithmetic. It reports values for which it fails |
| `ring()` | An equality between polynomials over `Int` | Nothing | The two sides are different polynomials |
| `_` | Tries, in this order and nothing more: a proof in scope of this proposition; `compute()`; `linear` with every proof in scope | The scope | None of the three succeeds |

Candidates for a second wave, each to be argued for: rewriting to a normal form with a given list of equations, and facts about bits and wrapping arithmetic.

*Rules for composing them.*

1. **A procedure call is an expression whose type is a proof.** It stands wherever a proof is expected.
2. **Its goal is the type expected there:** an annotation, a parameter, a field, a result. With no expected type it is an error. A lemma needs none, since its result says what it proves.
3. **A procedure uses its arguments, and never the scope.** `_` is the one form that reads the scope.
4. **Facts are passed as a tuple of proofs,** `linear((step, ih))`, and any proof may be one, a lemma's result included.
5. **Procedures do not instantiate a `forall` or open an `exists`.** The term does that first: `linear((sorted(i, j, bounds),))`.
6. **Cases and induction belong to terms.** `linear` splits only on what is inside its own fragment.
7. **Every call expands.** A command replaces a call by the term it found, which uses only the forms of layer 1 and the functions of layer 2. A check does not search again for a call whose stored proof still matches ([proof-search-and-storage](proof-search-and-storage.md)).
8. **The same call gives the same answer on every machine,** within a fixed bound.
9. **A failed call reports** what `debug::goal()` would ([proof-debugging](proof-debugging.md)), and the procedure's own reason.

*What is left out on purpose:* tactic blocks, procedures written by programs, solvers, default sets of rewriting rules, and facts that a procedure takes without being given them. The macros `calc!`, `rewrite!`, `fold!`, and `unfold!` are replaced by the methods and functions of layer 2.

### 2026-11-17, conversation with the owner on terms and procedures, concluded

The owner accepted the proposal of the previous entry with one change, and asked for it to be written into the open PR:

> We can drop ring for now - rest is all good. Let's please add it to the PR.

[RFC 0003](../design/rfcs/0003-proof-construction.md) is the result, as a draft: it has had no adversarial review. It adds nine rules to the specification, under "Constructing proofs", each with examples. What the owner decided is listed in that RFC, and so is what the RFC proposes beyond it: what `else` binds in `if h: c`, that `linear` includes equality and reduces by evaluation first, that contradictory facts prove anything, the names, and that facts are passed as a tuple.

This issue stays open until the owner decides on RFC 0003.

### 2026-11-21, conversation with the owner on the reviews of PR #25

RFC 0003 was reviewed adversarially at `f891be3`, and a second review by another tool followed at `e3659e8`. The reviews agree on one cause for most of what they found in this RFC: its rules are stated on source text, and "evaluation" means two things in it. Among the findings: a named condition proves more than the program checked; `eq::congr` and `rewrite` on an equality of runtime values reach fields that a model forgets; what `compute` finds cannot be written as a term; and about eleven accepted examples are not filled by the three steps a hole tries. None of this is repaired in the RFC yet.

Proposed in reply: logical code is translated to a core language of typed terms before any rule applies, and the rules for equality, rewriting, inference, and evaluation are stated on those terms. The owner:

> I'm okay with your recommendation and the rest of the idea looks reasonable.

What that accepts:

1. **Elaboration to core terms.** A place becomes a value name for one of its versions. A runtime value used as a value becomes `model(...)` of its snapshot. Arithmetic in runtime code has the value `wrap<T>(...)`. A runtime condition becomes one `Bool` term, and naming it gives a proof that this term is `true`.
2. **Conversion is part of when two types are the same.** Two proof types are the same when their propositions convert to the same term. This is the choice that RFC 0002's decision 49 left to this RFC. `compute()` stays a procedure, and expands to `eq::refl(_)`.
3. **Conversion is five reductions, and no others:**
   - arithmetic and comparisons on literals;
   - `if` on a literal, `match` on a constructor, and a field of a constructed value;
   - a call of a logic function that does not recur, a model included, becomes its body, where every name in that body could be written at the point of use;
   - a call of a recursive logic function becomes its body only when the argument it recurs on is a literal or a constructor;
   - a constant, and a ghost `let` that is not `mut`, become their values.
4. **A value name never unfolds.** A runtime value stays a variable with a fact about it, as RFC 0002 says of versions. Reduction is lazy, and stops at a fixed number of steps that the rule states.
5. **`wrap` is exact,** defined with the Euclidean remainder, and a program writes it as `u32::wrap(e)`.
6. **`<` and `>` stay separate,** neither defined from the other, so that conversion never makes two spellings of a proposition the same. `logical-layer/proof-holder-other-proposition.reject.lc` stays rejected.

None of this is written into an RFC, a rule, or an example yet. Asked whether to write it out first, the owner chose to settle the questions that add forms to the same core before it is written once: what is known where paths join, how a kept proof variable is typed, a borrow's final value, where a snapshot is cut, and indexing a sequence.

This issue stays open.
