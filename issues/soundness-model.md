# A model of the logical layer, and why its rules are sound

- **Kind:** design
- **Status:** open
- **Blocks:** Treating RFC 0002's rules as settled; the RFC for the runtime semantics, which defines the translation this model is about
- **Rules:** `logic.boundary.escape`, `logic.ghost.type-identity`, `logic.ghost.interior-mutability`, `contracts.final`, `models.snapshots.opaque`

## Scope

RFC 0002 now says in one section what a checked program means: a pure function of values, with propositions about values and never about places, an erased type and a meaning for every type, and erased values coming back from code that Locus does not check. Its rules are held against that reading. But the reading is prose, and so is every argument that a rule follows from it.

Four reviews of [#10](https://github.com/nikhilgarg28/locus/pull/10) each found soundness holes, and the fourth found that its holes shared causes:
- a type that reads a value, with no one statement of what the type means when the value changes: the stale reader of a `Cell`, the missing rule for when two types are the same, and dependent tuples in generic code;
- a value computed from one that the program had yet to produce: `final!` in a ghost write;
- snapshots that were structural unless a counterexample had shown otherwise;
- boundary rules derived from two different pictures of what unchecked code may do.

Reviewing prose again will find the next hole, not show that there is none. This issue is the work of writing the reading down as a model, small enough to check, and arguing that each rule follows from it.

The model covers a core, on paper or in Lean:
- locals, assignment, a struct with a proof field, a `&mut` parameter, a call of a checked function, `Option<@P>`, and one call of code that Locus does not check;
- for each type, its erased type and its meaning in the logic;
- the statement to argue: a checked function's translation is well typed in the logic, and every run of its erased program that returns agrees with that translation, given the assumptions of [the trust model](../design/TRUST.md).

## Questions it must answer

1. **What a type that reads values means, and where it may appear.** One rule from which these follow, instead of four rules found one at a time: assigning a field that an invariant reads (`contracts.invariants.assignment`), when two types are the same (`logic.ghost.type-identity`), dependent tuples in generic code (`logic.ghost.dependent-tuples`), and what a proposition under interior mutability may read (`logic.ghost.interior-mutability`).
2. **What exactly is assumed of code that Locus does not check.** `assumed.unsafe.sound` says it is sound Rust, read at the level of types. The model states that precisely enough for `logic.boundary.escape` to be a consequence, and says whether it is RustBelt's semantic typing or something weaker.
3. **Whether the logic needs prophecies at all.** `contracts.final` confines `final!` to a signature's result. The model says what a proof that mentions a final value means in a caller before the borrow ends, and whether backward functions, as in Aeneas, are enough.
4. **Which types have structural snapshots.** An argument that every structural snapshot type has only finite values, and that `models.snapshots.opaque` covers every other case.
5. **That an `open!` block is sound.** RFC 0002's decision 51 lets a field that an invariant reads be borrowed inside an `open!` block ([opening-invariants](opening-invariants.md)). The model shows that the value built at the end of the block is the only one that is ever observed, and that aborting on unwind is enough for that.
6. **Which features wait for the model.** Three that the owner chose carry most of the remaining risk, because each puts a logical value where the program can change it: proofs under interior mutability, which decision 47 keeps with a condition; ghost data in mutable runtime places, such as logical fields and `Vec<Nat>`; and dependent tuples inside containers. The model either covers them or says to hold them back.

## Log

### 2026-11-09, fourth review of PR #10

Raised by the fourth review of #10. It found three ways to prove `False` under the rules as written, one accepted fact that was false at run time, and two missing rules. The owner asked for a design that is coherent rather than patched. RFC 0002's decisions 44 to 50 fold the findings in and state the reading; this issue is what remains, a model that the reading can be checked against.
