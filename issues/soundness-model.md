# A model of the logical layer, and why its rules are sound

- **Kind:** design
- **Status:** open
- **Blocks:** Treating RFC 0002's rules as settled; the RFC for the runtime semantics, which defines the translation this model is about
- **Rules:** `logic.boundary.plain-data`, `logic.boundary.escape`, `logic.unverified.plain-rust`, `logic.ghost.type-identity`, `logic.ghost.interior-mutability`, `contracts.final`, `contracts.invariants.assignment`, `models.snapshots.opaque`

## Scope

RFC 0002 now says in one section what a verified program means: a pure function of values, with propositions about values and never about places, an erased type and a meaning for every type, and erased values coming back from code that Locus does not verify. Its rules are held against that reading. But the reading is prose, and so is every argument that a rule follows from it.

Four reviews of [#10](https://github.com/nikhilgarg28/locus/pull/10) each found soundness holes, and the fourth found that its holes shared causes:
- a type that reads a value, with no one statement of what the type means when the value changes: the stale reader of a `Cell`, the missing rule for when two types are the same, and dependent tuples in generic code;
- a value computed from one that the program had yet to produce: `final!` in a ghost write;
- snapshots that were structural unless a counterexample had shown otherwise;
- boundary rules derived from two different pictures of what unverified code may do.

Reviewing prose again will find the next hole, not show that there is none. This issue is the work of writing the reading down as a model, small enough to check, and arguing that each rule follows from it.

The model covers a core, on paper or in Lean:
- locals, assignment, a struct with a proof field, a `&mut` parameter, a call of a verified function, `Option<@P>`, and one call of code that Locus does not verify;
- for each type, its erased type and its meaning in the logic;
- the statement to argue: a verified function's translation is well typed in the logic, and every run of its erased program that returns agrees with that translation, given the assumptions of [the trust model](../design/TRUST.md).

## Questions it must answer

1. **What a type that reads values means, and where it may appear.** One rule from which these follow, instead of four rules found one at a time: assigning a field that an invariant reads (`contracts.invariants.assignment`), when two types are the same (`logic.ghost.type-identity`), dependent tuples in generic code (`logic.ghost.dependent-tuples`), and what a proposition under interior mutability may read (`logic.ghost.interior-mutability`).
2. **What exactly is assumed of code that Locus does not verify.** `assumed.unsafe.sound` says it is sound Rust, read at the level of types. The model states that precisely enough for `logic.boundary.escape` to be a consequence, and says whether it is RustBelt's semantic typing or something weaker. Since decision 54 this code includes a crate's own unverified functions, each assumed sound as a function, and the model shows that keeping every logical type and invariant type out of them (`logic.unverified.plain-rust`) is enough for nothing they do to reach what Locus believes.
3. **Whether the logic needs prophecies at all.** `contracts.final` confines `final!` to a signature's result. The model says what a proof that mentions a final value means in a caller before the borrow ends, and whether backward functions, as in Aeneas, are enough.
4. **Which types have structural snapshots.** An argument that every structural snapshot type has only finite values, and that `models.snapshots.opaque` covers every other case.
5. **That an `open!` block is sound.** RFC 0002's decision 51 lets a field that an invariant reads be borrowed inside an `open!` block ([opening-invariants](opening-invariants.md)). The model shows that the value built at the end of the block is the only one that is ever observed, and that aborting on unwind is enough for that.
6. **Which features wait for the model.** Three that the owner chose carry most of the remaining risk, because each puts a logical value where the program can change it: proofs under interior mutability, which decision 47 keeps with a condition; ghost data in mutable runtime places, such as logical fields and `Vec<Nat>`; and dependent tuples inside containers. The model either covers them or says to hold them back.

## Log

### 2026-11-09, fourth review of PR #10

Raised by the fourth review of #10. It found three ways to prove `False` under the rules as written, one accepted fact that was false at run time, and two missing rules. The owner asked for a design that is coherent rather than patched. RFC 0002's decisions 44 to 50 fold the findings in and state the reading; this issue is what remains, a model that the reading can be checked against.

### 2026-11-09, conversation with the owner about PR #24

The owner asked whether the approach to `unsafe` code itself needed to change, and chose a smaller one: RFC 0002's decisions 54 to 57. A function is verified whole or not at all, an unverified function is plain Rust that takes and returns no proof, and no program assumes a fact. This removes the second picture of unverified code that the fourth review found: there is no escape, no list of operations that make values from raw memory, and no assumption about Rust's aliasing rules. What the model must now cover on this side is one assumption, `assumed.unsafe.sound`, stated for a function. Two points of the new text are the reviewer's proposals and still for the owner: that an unverified function handles no invariant type at all, and that a function that calls into an unnamed crate with `unsafe` code is unverified.

### 2026-11-09, conversation with the owner about PR #24, continued

Of the two proposals in the entry above, the owner confirmed the first and widened it: an unverified function takes no type that holds logical data at any depth, completable or not (RFC 0002, decision 58). The second, about crates that are not named, is still for the owner. Decision 59 adds a later part to this model: permissions, which are ghost values that are not `Copy`, and the argument that a layer of pointer primitives is sound ([permissioned-pointers](permissioned-pointers.md)).

### 2026-11-16, conversation with the owner on how to build Locus

The owner asked for a boundary whose rule is short enough to hold in mind, and RFC 0002's decisions 61 to 67 give one. What the model has to cover on this side is now:

- **One invariant at the boundary** (question 2): after instantiation, no unverified code has a value, a type argument, a capture, or a callee whose type holds logical data, except through a specified library operation. Completability, the three kinds of protected value, and the analysis of write access are gone, so the model no longer has to justify them. It has to show that the invariant is preserved, and that checking it for each instantiation is enough.
- **Values that are valid at every point** (questions 1 and 5): a field that a proof reads never changes in place, and there is no `open!`. A struct with proof fields is a dependent record, and the model need say nothing about unwinding through a broken value.
- **Two assumptions fewer.** `assumed.unsafe.invariants` is removed, since no `unsafe` code outside specified library operations holds a value with an invariant.

The conversation also listed the ways a boundary can fail, as classes to attack it with before its representation is fixed: fabricating evidence, duplicating a resource, invalidating a fact through a hidden alias, re-entering during an update, and leaving by a panic or a forgotten guard. The cases from "Memory Safety Bugs in Rust Standard Library" (Microsoft FORGE Lab, 2026) are examples of the second and fifth kinds, and are now in `litmus/`.

Two holes found in the same conversation, both closed by the rules as now written: a verified generic function that passes its own type parameter to unverified generic code (`logical-layer/generic-parameter-reaches-unverified.reject.lc`), and a crate that Locus does not verify calling a public function that takes a proof (decision 68).

### 2026-11-16, conversation with the owner on how to build Locus, continued

Decision 73 of RFC 0002 lets unverified code hold a value of a sealed type, so two things in the entry above change. The boundary's invariant now speaks of what a type exposes: logical data that is not inside the fields of a sealed type. And `assumed.unsafe.invariants` stays in the trust model, because `unsafe` code, such as a container's, does hold sealed values.

What the model has to show for a sealed type:
- that it means the same wherever it goes: its propositions read only its own fields, and none of its logical data comes in through a type argument, so one Rust type never stands for two meanings;
- that a value handed back by unverified code is one that verified code built: other crates cannot reach its fields, the crate's own unverified functions cannot name it, trait implementations for it are verified, and generic code can only move, copy, and drop it;
- what a hand-written `unsafe impl` on it promises, since generic `unsafe` code can now be instantiated with it.

### 2026-11-16, conversation with the owner on the standard library

RFC 0002's decision 76 adds loans, and with them two more things for the model to show:
- that a value lent by shared reference is unchanged when the loan ends, when all of its logical data lies in the structural part of its snapshot. This is `models.frozen-data` again, for unverified code that holds the reference;
- that nothing logical comes back. No operation open to verified code turns a type that exposes no logical data into one that does, other than building the value from proofs. A marker that unverified code copied out of a loan, and kept behind `dyn Any` or in a static, is then inert. The same fact keeps a `Box<dyn Any>` from any source harmless, so it is worth stating once, as the property the whole boundary rests on.
