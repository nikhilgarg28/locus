+++
id = "principles"
title = "Principles"
group = "Vision"
created = "2026-09-21T21:03:06.000Z"
updated = "2026-09-22T23:44:20.000Z"
route = "vision/principles.html"
order = 8
+++

# Principles

## How the work is judged

The central promise is that someone can write familiar code, state meaningful facts about it, and get understandable help establishing them. The kernel is a foundation for that; the experience from source to executable is what decides whether the language succeeds. Until that has been tested, the core feature set holds steady.

**The test.** One polished end-to-end example: source code for a small bounded state machine; a result that carries evidence; a caller that reuses that evidence; useful diagnostics for an intentional mistake; and matching interpreted and generated behavior. Everything between here and there serves that example.

**Boundaries to protect.**

- Dependency is restricted to propositions. A value may change what a later proof field asserts and never the runtime layout. Dependent results, validated data, and loop invariants all rest on this one mechanism. A new feature must earn its place inside the boundary before the type system is widened.
- Loop invariants are evidence in loop state: initialization supplies it, and each iteration supplies it again.
- Logical classification belongs to types: Int, Nat, Bool, logical collections, Prop, and evidence erase; runtime types retain their representation. Checked logic fn and logical blocks are pure and total. Ordinary fn stays executable even when returning logical values. Model<T> constructs a logical observation of legally readable runtime data. There is no math fn, logic let, or implicit mode change through generics.
- Logical data is extensible through checked structs and inductive enums. Direct recursive logical types need no runtime indirection; Int, Seq, and finite maps can be library definitions. Native arithmetic support is an implementation choice with a specified correspondence. Box<T> always remains runtime, including when T is logical, and runtime recursive structures are related to separate logical models.
- Logical fields can occur in runtime aggregates, so two source values may share one runtime representation. Runtime equality, serialization, mutation, and foreign interfaces must respect that boundary. Model snapshots retain binding and heap-state versions; they cannot recover runtime data or storage permissions. Runtime branches cannot discriminate on logical contents.
- Ownership, aliasing, and mutation were tested with tier 0 of references in the core build (September 2026): let mut and assignment lowered to versions, tracked evidence carried by loops, &mut parameters as values in and new versions out, and a protected value that survives a panic. Which state a proof describes after another reference changes storage is answered for that tier by versions of bindings; heap-state versions, interior mutability, and references held in locals or fields are the questions the next tiers must answer before promising broad Rust compatibility.

**Acceptance criteria for the proof-writing experience.** The hand-built proofs so far show that the rules compose, not that the `let`-based surface is pleasant. Removing normalization from the kernel moved work into elaboration: projection, substitution, unfolding, and equality transport still have to happen somewhere, and if users must repair them by hand, friendly syntax will not save the experience.

- Routine rearrangements, such as introducing a local variable, destructuring a tuple, or extracting a helper, require little or no proof repair when the interfaces involved stay equivalent.
- An unsolved `_` shows the expected claim, the useful facts available, and the remaining gap.

Both are to be exercised by the first integrated examples, not deferred to a tooling milestone. The next design test, once generics and the narrow mutation tier exist, is a verified collection over a custom model with evidence reused across callers. [Open questions and deferred](open-questions.md) records its acceptance cases and the remaining concerns about proof elimination, model/trait contracts, erasure, and the logical foundation.

**Measure before large proofs are normal.** Predictable checking is not the same as fast compilation, and making every computation step explicit can produce a great deal of evidence. Track separately: time spent constructing proofs; the size of the resulting proof terms; time and memory spent checking them; and the cost of rechecking after a small edit. Checked lemma reuse and stable interfaces will matter early. Do not optimize the representation before there are numbers.

**Preferences, not constraints.**

- Generated Rust resembling the source is a strong preference. Temporary bindings and modest structural changes are acceptable where they make evaluation order or the correctness of erasure simpler.
- All logical types lower to one `Erased` marker. Generated Rust removes unused marker bindings by default, preserving effectful initializers and temporary/drop semantics, and names unused retained parameters appropriately. A debug dump may retain markers; no public flag is needed for clean output. The marker does not protect the Rust boundary by itself.
- The typed tree's two branches keep the output readable at a real cost in trust: a sound kernel cannot establish that the checked program and the executed program agree. Differential testing of the two branches is the next step for exactly that reason.

## Why Locus has its own kernel

Locus uses its own proof kernel, written in Rust. A Lean-backed adapter is no longer the plan. The decision rests on these commitments:

- The logic is classical. The planned soundness argument is a denotational model in the style of explicit refinement types (Ghalayini and Krishnaswami, ICFP 2023): propositions denote truth values, and types denote sets of logical values related to runtime representations. Locus keeps the two apart because logical data such as a `Prop` field belongs to the logical value but not to the representation; for runtime types without logical data fields this reduces to the paper's subset reading. Its mechanized Lean 4 development is a starting point; the two-layer values, divergence, and declared propositions are the parts Locus must add. Excluded middle is a single named lemma whose uses are recorded. Until the model is checked against every kernel rule, consistency is a goal. Exporting accepted terms to an independent implementation tests the kernel but does not replace that argument.
- The kernel has no conversion checker. It compares terms up to renaming and proof irrelevance; every computation step is an explicit equality axiom used through transport. The elaborator inserts the steps for immutable lets, projections, matches on known constructors, and literal arithmetic, so the programmer still sees the minimal type identity of the specification. Function bodies are unfolded only on request.
- Proof irrelevance holds, as part of conversion. `u8` is modelled as a natural number below 256 over a kernel-internal `Nat` with a recursor; its ordering lemmas are proved from that model. Native evaluation must agree with the model and can decide closed terms. Induction over that `Nat` is the proof-level recursion rule; bounded iteration is a separate term-level rule whose index-dependent state erases to a plain loop.
- Everything that constructs proofs is untrusted: the elaborator, the hole solver, and any later decision procedure, external solver, or model. Each must produce a kernel term, or a certificate for a checker that is explicitly added to the trusted base.
- Proofs are written declaratively, as stated intermediate facts joined by lemma calls and holes. There is no tactic language. The hole's search is fixed and bounded by step counts, never by time, and it does not search the prelude, so that its behavior is predictable; found proofs are recorded so rechecking does not repeat the search.
- The prelude is checked, not trusted. The main cost of not building on Lean is this lemma library; statements can be ported, proofs must be redone.

Regardless of kernel, the translation of programs into kernel statements, erasure, and execution initially remain trusted compiler components. Proof acceptance alone does not establish that generated machine code preserves the source semantics.
