+++
id = "open-questions"
title = "Open questions and deferred"
group = "Vision"
created = "2026-09-21T21:03:06.000Z"
updated = "2026-09-22T23:44:20.000Z"
route = "vision/open-questions.html"
order = 9
+++

# Open questions and deferred features

Undecided, or decided against for now. Section numbers refer to [Language, as built](language.md). When one of these is settled it moves into the [Target language](target-language.md) and gets tasks.

## Open questions

These are not decided. Each lists the current behavior of this document first. None needs to be settled before the kernel spike.

1. Settled in the target language: prop!(e) lifts a logical Bool through holds; @P is evidence of a proposition. Quantifiers are library propositions with constructors, not literal keywords. Brackets remain available for arrays; a lighter literal spelling can be considered later.
2. Settled: implication is spelled =>, and no longer collides with the match arm separator, because it appears only inside prop!, prove!, @(...), and the braced body of a predicate arm.
3. Settled in the target language: evidence must be of exactly the claim wanted, after computing. The bridging the elaborator does today is to be removed.
4. Branch and arm evidence is anonymous. A naming form, such as if h: n != 0 { ... }, would let hand-written steps refer to it without a hole.
5. A hole does not search the prelude (section 12.3). A mechanism for marking lemmas that a hole may apply, with its own step budget, would shorten proofs at some cost in predictability.
6. The spelling of the chain form (section 8.4). One candidate is the bracketed form trans[a =(p) b =(q) c] used by the explicit refinement calculus.
7. Settled in the core build: the bounded for admits break, and a reversed or empty range runs no passes, with no proof of order asked.
8. Adopted, as The rule about Rust in the Target language; the earlier violations were removed in the core build.

## Design concerns

Review recorded 22 September 2026, following the logical/runtime separation and library-data decisions. The items below distinguish unresolved rules and validation work from already agreed restrictions. They do not authorize additional Rust features or claim that the compiler implements the Vision. Alternative mechanisms mentioned here remain options until their semantics are specified.

1. **Library abstractions and generic models.** Fixed logical classification requires corresponding runtime/logical operations, including bool/Bool, comparison, and optional values. Test a generic verified collection over a user-defined runtime element T with a logical model M, rather than only over machine integers. Define how model selection, generic bounds, and related comparison laws are expressed. Different models can forget different information; equality of content models must not imply allocation identity, capacity equality, or full runtime equality. A finite-map library must state whether an operation/equality concerns representation or extensional contents. The source language needs general inductive data and logical functions, not a growing list of special collection primitives.

2. **Proof elimination within logic.** Preserve proof irrelevance when extending matching to logical functions, closures, and library quantifiers. Two proofs of Exists(n: Nat, True), built with witnesses 0 and 1, are interchangeable; a function that extracts the stored witness and reduces respectively to 0 and 1 would contradict that principle. Erasure of both values does not help. The existing restriction allowing the witness only inside a proof-producing elimination must carry forward. State exact rules for permitted proof projections, specialization of ForAll, and any proposed subsingleton exceptions. Include a rejected witness extractor returning Nat and an accepted proof using the witness internally. Arbitrary logical result types are not a sufficient check. The rule to preserve is the standard one and is already how the kernel works: evidence is eliminated only into a Prop or a proof type, never into data; a match on evidence yields a proof, and the witness of an existential is usable inside that proof and nowhere else.

3. **Snapshots, heap state, and resources.** Specify which storage an observation depends on, how a mutation invalidates facts about current state, and how facts about disjoint unchanged storage are retained. Historical snapshots remain valid. Reborrowing, split slices, captured/stored references, and especially returning &mut require more than entry/return SSA versions: subsequent caller writes determine the owner's later state. The existing tier limits remain. Interior mutation, locks, atomics, and concurrency need dedicated permission and observation rules. Freely reusable snapshot evidence does not itself grant exclusive storage access; tracked resources with restricted duplication, or another ownership logic, may be needed. That is future design work, not an implicit extension of @P.

4. **Erasure preserves execution, not just returned values.** Formalize the correspondence between checked execution and generated Rust, including effects, divergence, panic, unwinding, partial initialization/moves, temporary lifetimes, and Drop timing. In derive(mutate_and_prove(&mut x)), preserve the ordinary call without moving its effects across a logical branch or changing its borrow/drop scope. Generic/trait dispatch must be resolved and retained when many source types collapse to Erased; generated impls and specializations must not collide. Audit callable captures, auto traits such as Send/Sync, variance/drop checking, and ABI/layout promises before enabling the relevant Rust features. Box<Nat> remains a runtime box throughout, not a newly erased ownership operation. Default dead-marker cleanup is subject to this same preservation contract.

5. **Behavioral trait contracts and external implementations.** A method signature alone cannot supply ordering laws, iterator relationships, callback effects, or destructor behavior. Decide how traits express contracts and how implementations establish them, including model relations, effects, trait objects, and closures when supported. External Rust implementations need a verified adapter or an explicitly trusted specification; a matching signature grants no theorem. Keep erased-proof entry points protected at the Rust boundary. Opaque types and wrappers must also account for public mutation and construction paths such as Default, conversion, and deserialization. One Erased marker cannot enforce the original proof index in handwritten Rust.

6. **Proof ergonomics and cost.** Keep explicit proof slots while supplying certificate-producing automation for routine arithmetic and equality steps. Introducing a let, destructuring a tuple, or extracting a helper with an equivalent contract should not expose a cascade of manual transports/unfolding repairs. Exercise failed holes and version-mismatch diagnostics. Measure annotation burden, proof construction time, certificate size, checking time/memory, and rechecking after an edit. Bounded deterministic search and a small kernel do not by themselves imply fast builds or understandable failures. The own-kernel route also requires building and maintaining the lemma library.

7. **Logical foundation and source/kernel correspondence.** Consolidate the formation, equality, positivity, elimination, and recursion rules for library-defined logical types and propositions. Generics, higher-order predicates, and dependent proof-returning callables need an explicit account of permissible type/proposition quantification, including any levels required to keep it sound. Library Exists/ForAll shift work into these general rules; they do not remove it. Native numerals, arithmetic certificates, primitive observations, and any future extensionality/choice principles need stated interpretations and trust boundaries. Kernel acceptance alone does not prove that source lowering described the intended program or that erasure implements it; test both translations and develop the soundness argument. No unchecked recursive equation or library declaration may introduce facts on its own.

8. **What a returned proof promises.** An ordinary function's output guarantee applies on successful return. An ordinary fn returning @False can diverge without making False a theorem; it remains inadmissible in logic. Termination and no-panic promises are separate. State this distinction consistently in headers, diagnostics, and correctness claims. Logical fn remains total and pure independently of its result type.

**Validation before broadening the Rust subset.** After the existing core acceptance example, build one generic verified collection with a custom element model, explicit evidence propagated through two or three callers, and a mutable operation within the supported borrowing tier. Include an intentional wrong proof, a harmless refactoring, a rejected existential witness extractor, a runtime Box containing a logical payload, and an executable call inside an otherwise erased expression. Check both proof diagnostics and retained runtime behavior. This is a follow-up design test, not a claim that the current immutable core already supports it.

## To settle before the core is built

Raised in the last review before implementation, and tracked as tasks under Open decisions: LOC-129, LOC-130, LOC-131, LOC-132, LOC-133, LOC-134, LOC-135, LOC-136. In short: the checked IR for mutation, early exit, and panics, with the flow rules the target language defers; how moves meet the logic; acceptance examples in the target syntax, and the automation they show to be needed; whether evidence of one claim is accepted for another; how Int enters the kernel and how the machine models are tested; what a panic is to the interpreter and to erasure; what locus build emits; and the order of implementation.

## Deferred

The README milestones give the intended order. Each item below is outside this fragment.

- Logical lambdas and dependent proof-returning callables, capturing immutable logical observations. Their design is decided in Logical computation and erasure; their implementation supports library Exists/ForAll and removes hand lifting of proof helpers.
- Logical classification, logic fn/blocks, model observations, and their lowering. Int/Nat/Seq have no runtime representation; the earlier executable-mathematics plan is superseded. Runtime arbitrary-precision types, if ever wanted, would be distinct runtime types with models. The existing arithmetic kernel and build tasks are not automatically migrated by this decision.
- Generics over types and propositions, with Logical bounds on logical stored elements and mode-specific callable/comparison bounds. Exists and ForAll become library propositions; runtime Option and logical optional types remain distinct.
- Recursion, as one feature: recursive logical enums, recursive prop declarations, and checked structural recursion/induction, followed by supported well-founded recursion. Logical enums can directly contain recursive occurrences with no Box because they have no runtime layout. Int, Seq, finite maps, and eventually Nat can be library types once these mechanisms exist; Target language contains schematic examples. Box<T> is always runtime, even for logical T. A runtime boxed structure can have a separate logical model; neither it nor Rc/Arc/shared references become logical by ignoring indirection. Cycles, aliasing, and interior mutation need dedicated observation rules. Supported mutual and nested recursive forms, and migration of native arithmetic support to library definitions, remain implementation decisions.
- Model fields are decided: a field of logical type M erases, even in a runtime struct. Model<T> is implemented on M with a logical observer taking &T; a type may have several models. Snapshot access permissions and heap-state versions need implementation. Runtime equality cannot recover distinctions stored only in logical data fields.
- Dependent conjunction and implication, where the right operand is well formed only under the left, as in [i < len && a.get(i) > 0] once indexing demands a proof. This arrives with the first operation that takes a proof precondition, and is a reason to make && and => kernel formers at that point, not instances of the prelude And.
- Requires/ensures/invariant/assert as sugar over proof parameters, dependent results, and loop proof state.
- Mutation, references, ownership, and surface loops that elaborate to the state-passing loops of section 10.
- Termination measures, for total correctness of executable loops and for non-structural logical recursion. Until then the workaround for the latter is an explicit fuel parameter.
- Trusted and external declarations, with the rest of Rust interoperability. Every such declaration is to be syntactically marked.
- Runtime closures.


- Default cleanup of unused Erased bindings is decided under What is generated. A pre-cleanup diagnostic dump may be useful later; a public flag and its spelling are deferred, not required for clean generated Rust.
