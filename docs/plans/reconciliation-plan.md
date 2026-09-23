+++
group = "Plan"
created = "2026-09-21T22:14:21.000Z"
id = "reconciliation-plan"
title = "Reconciliation plan"
updated = "2026-09-23T02:44:04.000Z"
route = "plans/reconciliation-plan.html"
order = 11
+++

# Reconciliation plan

Reconciliation is complete. All three tiers have passed tools/check.sh --extended, including certificate replay, hostile inputs, generic collections and three-way execution. All four preview features are stabilized. The fast suite measured 211 seconds against its advisory 120-second target; the extended suite measured 826 seconds. Process follows, including generated measurements and specification traceability.

## What done means

**Tier one, the logical split.** The three target examples check, run, and compile exactly as Target examples writes them. A type is Logical or runtime by its type alone, and a Logical value never reaches a runtime position: the erased check judges it independently of the elaborator. Only a logic fn or a logic block enters the logic; an ordinary fn never does, whatever it promises. prop declarations have named arms and are used by construction and matching; a one-arm predicate is opened by a let pattern in ordinary code. The default model rule holds: prop!(x <= 3) on a u32 and its cast form are the same term. The generated Rust has no marker binding a reader would delete and carries no blanket lint allowance. Every migration diagnostic carries a fix that is tested by applying it. Language, as built is rewritten, and the interim rules of the core build (LOC-193, Ghost, logical-by-promises) are gone from the code.

**Tier two, logical data.** Seq, Maybe, Nat, Exists, and ForAll are library declarations written in Locus and checked by the kernel; a logical enum may be recursive, with positivity checked and induction supplied; a prop may be inductive; a user type may have several models selected by as M; Option is a prelude enum and Percent returns it. The acceptance examples of Library-defined logical data and Design concern 2 are corpus files.

**Tier three, references and heap state.** Slices as parameter views with a proof precondition on indexing; shared references held, stored, and returned under tier one of references; trusted declarations of Rust collections with models, each with its reason and listed by the audit; and the generic verified collection of Design concerns, with its wrong proof, its refactoring, and its rejected witness extractor.

## Milestones

| Milestone | Reached at | What a user can do |
|---|---|---|
| 1. One language | T2 | Write the Vision's spelling for everything the core build did, with logic fn, named arms, and Bool |
| 2. Logical data | D10 | Specify with sequences, maps, and user-defined logical types and predicates, with induction |
| 3. Collections | B4 | Verify a generic collection with a model, over slices and trusted Rust containers |

## Order and what can run in parallel

The waves below are computed from the tasks by python3 tools/atlas.py plan Reconciliation. Within a wave the commits are independent. Q1, Q2, and C1 wait for P6 of the Process project, the preview gates, which is not a task here.

| Wave | Syntax | Kernel | Elaborator | Generated | Data | References | Robustness |
|---|---|---|---|---|---|---|---|
| 1 | Q1, Q2 | L1, L2 |  | G2 |  |  |  |
| 2 |  | D1 | C1 |  |  |  |  |
| 3 |  |  | C2, C3, C5 | G1 |  |  |  |
| 4 |  |  | C4, D2 |  |  |  |  |
| 5 |  |  | D3, D7, D8 |  |  |  | T1 |
| 6 |  | D4 |  |  |  |  | T2 |
| 7 |  | D5 |  |  | D6 |  |  |
| 8 |  |  |  |  | D9 | B1 |  |
| 9 |  |  |  |  |  | B2, B3 | D10 |
| 10 |  |  |  |  |  |  | B4 |

- **The elaborator lane is the long chain** of tier one: C1, then C2, C3, C4, then T1. The syntax and kernel work of the tier are small and land first.
- **Tier two is kernel-heavy**: D1 (type parameters), D4 (recursion and induction), and D5 (inductive predicates) each add to the trusted base and each is a commit with its contract text written first, as K6 was.
- **Tier three waits on tier two** for Seq and Model, and B1 (views) comes before B3 (stored references) on Rue's evidence that views that never escape a call carry a long way.

### Who owns which files

| Lane | Owns | Shares |
|---|---|---|
| Syntax | src/lexer.rs, src/parser.rs, src/ast.rs, tests/frontend.rs | none |
| Kernel | src/kernel, tests/kernel_*.rs, the Kernel contract | tests/kernel_soundness.rs with Robustness |
| Elaborator | src/elab, tests/elaborate*.rs | src/typed/tree.rs with Generated |
| Generated | src/erased, src/typed/lower.rs, src/build.rs | src/typed/tree.rs with Elaborator |
| Data | the prelude files in Locus, their lemma tests | none |
| References | src/elab/references.rs, src/elab/moves.rs, the trusted-declaration files | src/erased/rust.rs with Generated |
| Robustness | tests/corpus, tests/acceptance.rs, tools | none |

## Rules for every commit

The Build plan's rules, and:

- A cross-cutting change lands behind a preview gate with its corpus files marked, and the built language's tests stay untouched until the tier's acceptance stabilises the gate.
- A commit that adds to the trusted base (L1, L2, D1, D4, D5) writes its contract text first and lands it in the same commit.
- A commit that removes a spelling gives it a migration diagnostic with a fix, tested by applying the fix.
- Anything learned that contradicts the Vision is recorded there or as an open decision before the commit lands.

## The commits

One task each, in the Reconciliation project, where the scope, the tests, and the condition for done are written. This list and the table of waves are written by python3 tools/atlas.py plan from the tasks, which are the only record of the order.

### Syntax

- **Q1** LOC-205. logic as a keyword: logic fn, logic blocks, and logical callable types parsed. After: nothing.
- **Q2** LOC-206. Named-arm propositions: declarations, constructors with @, and proof patterns. After: nothing.

### Kernel

- **L1** LOC-207. Bool-valued comparisons on Int and the lifting of Bool to Prop in the kernel. After: nothing.
- **L2** LOC-208. Constructors of a declared proposition take witnesses and one proof of a computed body. After: nothing.
- **D1** LOC-218. Type parameters in the kernel, instantiated by substitution. After: L2.
- **D4** LOC-221. Recursive logical types, structural recursion, and induction. After: D1, D3.
- **D5** LOC-222. Inductive predicates: recursive prop declarations with positivity and induction over proofs. After: D4, C4.

### Elaborator

- **C1** LOC-209. Every type is Logical or runtime; Ghost<T> and snapshot! are replaced by model observations. After: Q1, L1.
- **C2** LOC-210. logic fn and logic blocks; an ordinary fn never enters the logic. After: Q1, C1.
- **C3** LOC-211. The default model rule, and prop! and prove! over proposition expressions. After: C1, L1.
- **C4** LOC-212. Named-arm propositions in the elaborator, with proof patterns in ordinary code. After: Q2, L2, C2.
- **C5** LOC-213. Scope escape, stored claims, and versions named in diagnostics. After: C1.
- **D2** LOC-219. Generics in the surface language, with mode fixed by the declaration. After: D1, C2.
- **D3** LOC-220. derive(Logical): logical structs and enums, with no runtime tag. After: D2.
- **D7** LOC-224. The Model trait: x as M for user models, with the default models registered. After: D2, C3.
- **D8** LOC-225. Logical closures and dependent proof-returning callables. After: D2.

### Generated

- **G1** LOC-214. Default cleanup of markers in the generated Rust. After: C1.
- **G2** LOC-215. Dereference on writes through a mutable reference, and the printer's remaining lies. After: nothing.

### Data

- **D6** LOC-223. Library logical data: Seq, Maybe, and Nat with its correspondence to Int. After: D4.
- **D9** LOC-226. Quantifiers as library propositions, with the keywords as sugar. After: D8, D5.

### References

- **B1** LOC-228. Parameter-only views: slices as &[T] and &mut [T], indexing with a proof precondition. After: D6, D7.
- **B2** LOC-229. Trusted declarations of Rust collections with logical models, listed by the audit. After: B1.
- **B3** LOC-230. Shared references in locals, fields, and results, with lifetimes and observation permissions. After: B1.

### Robustness

- **T1** LOC-216. The target examples in the Vision's spelling, and the tier-one acceptance corpus. After: C3, C4, C5, G1, G2.
- **T2** LOC-217. Acceptance of tier one: Now rewritten, the interim rules gone, the criteria as tests. After: T1.
- **D10** LOC-227. Acceptance of tier two: the finite map and the examples of logical data. After: D6, D7, D9.
- **B4** LOC-231. Acceptance of tier three: the generic verified collection. After: B2, B3, D9.

## Decisions still needed

- **Nat as native or library.** D6 makes Nat a library type with a checked correspondence to non-negative Int; until then measures over Nat wait, and decreases stays parsed.
- **Quantifier keywords** stay through tier one; D9 decides whether they remain as sugar.
- **Option and Result** arrive as prelude enums in D2, so the Percent target keeps its own enum through tier one.
- **Traits.** D7 accepts impl only for the compiler-known Model trait; general traits are a later project.
- **Mutual recursion** in D4 and D5 is decided in those commits.
