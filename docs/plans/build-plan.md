+++
id = "build-plan"
title = "Build plan"
group = "Plan"
created = "2026-09-21T22:14:21.000Z"
updated = "2026-09-22T23:44:20.000Z"
route = "plans/build-plan.html"
order = 10
+++

# Build plan

How the code gets from what is built (Language, as built) to the core language of Target language: 41 commits in seven lanes, each with its tests and the condition under which it is done. Each commit is a task in the Core build project, where its status is tracked; this document holds the order, the reasons, and the exit criteria for the whole. It is a plan and changes as commits land and teach us things.

## What done means

The body of work is finished when all of these hold, each as a test or a command and not as a judgement.

- **The target examples run.** The 32-bit lock, the midpoint, and the protected type from Target examples check, run in both interpreters, compile under rustc with warnings denied, and the three agree on every listed input, panics included. Percent uses an enum of its own in place of Option.
- **The proofs are the ones predicted.** check --stats classifies the obligations of those files as exact, computed, or arithmetic, and the counts match Target examples (three, four, seven) or the difference is recorded there as a finding.
- **A Rust caller is held at the boundary.** A hand-written Rust crate compiled against the generated module can call what is plain pub and nothing that takes evidence, cannot name a marker constructor, cannot build a protected struct, and the marker replay attack fails to compile. rustc gives the verdict in each case. A protected value that a Rust caller still holds after catching a panic satisfies its invariant (LOC-191).
- **An author is told why.** Removing a hypothesis from each target example gives a diagnostic with the obligation, the claim after computing, the facts considered, and a counterexample where the arithmetic procedure finds one. Every error code has a rejected file and a golden rendering.
- **A crate can be checked by the kernel alone.** With its stored proofs, locus check --locked runs no search, and proofs written by a weaker search still check.
- **Checking is deterministic.** Checking a file twice, on any machine, gives byte-identical proofs, diagnostics, and Rust. No hole is filled by search that bridges claims, and every limit in the checker is a count of work and never a clock.
- **Locus is never more permissive than rustc.** Every accepted corpus program compiles without warnings. Every program rejected for a move or an aliasing error in a runtime use is also rejected by rustc when printed without that check.
- **What is checked is what runs.** Under the extended suite the random program generator agrees three ways (check-IR interpreter, erased interpreter, compiled Rust) on at least 10,000 programs covering arithmetic in both overflow configurations, panics, mutation, bounded loops, and &mut calls, with no more than a handful inconclusive.
- **The trusted base is written down and tested as such.** Every kernel axiom and rule is named in the Kernel contract (a test compares them), has a test that uses it and a near miss that is rejected, and the machine models and the operator table agree with Rust exhaustively at 8 bits and at the boundaries above. Each untrusted part (the solver, the arithmetic procedure, the flow analysis) has a test in which its wrong answer, a fabricated constraint included, is rejected by the trusted part behind it. No proof is accepted for a claim known to be false.
- **The parser is robust, and total for stated reasons.** The fuzz tests pass at a hundred times their default counts, which shows robustness on what was tried. Totality rests on two guarantees stated in Architecture and tested on their own: every loop in the parser consumes a token or stops, and recursion is bounded.
- **The legacy is gone.** No math fn, no [P] or @[P], no state-passing loop or bounded for, no u8 model over Nat, no proof by 256 cases. Language, as built is rewritten from the code.
- **The suites are usable.** tools/check.sh, the fast suite with fixed seeds, passes in under two minutes and gates every commit. tools/check.sh --extended passes before a milestone is declared.

## Milestones

| Milestone | Reached at | What a user can do |
|---|---|---|
| 1. The new surface | S5 (LOC-158), E1 (LOC-167) | Write the target syntax for everything the language already does, with evidence that matches exactly |
| 2. Arithmetic | E7 (LOC-173) | Write 32-bit programs whose overflow is shown impossible: midpoint, step, remaining |
| 3. Mutation | M4 (LOC-180) | Write let mut and Rust loops that carry evidence: the whole lock file |
| 4. The boundary | O4 (LOC-185) | Ship a protected type to a Rust caller: Percent |
| 5. Acceptance | R4 (LOC-189) | Everything under What done means |

## Order and what can run in parallel

A commit can start when the commits it depends on have landed. Grouping by that rule gives the waves below, which are computed from the tasks. Within a wave the commits are independent of one another.

| Wave | Harness and robustness | Syntax | Kernel | Elaborator | Mutation | Ownership |
|---|---|---|---|---|---|---|
| 1 | H1, H2, H3 |  | K1 |  | M0 |  |
| 2 | R1, R2, R3 | S1 | K2 | E4 |  |  |
| 3 |  | S2 | K3, K6 |  | M1 |  |
| 4 |  | S3 | K4 | E1 |  |  |
| 5 |  | S4 | K5, K7, K8 | E5 |  |  |
| 6 |  | S5 |  | E2, E9 |  |  |
| 7 |  |  |  | E3, E6, E8, E10 | M2 | O2 |
| 8 |  |  |  | E7 | M3 | O1 |
| 9 |  |  |  | E11 | M4, M5 |  |
| 10 |  |  |  |  |  | O3 |
| 11 |  |  |  |  |  | O4 |
| 12 | R4 |  |  |  |  |  |

- **The longest chain** runs through the syntax lane into mutation and ownership; python3 tools/atlas.py plan prints it. The syntax lane has one owner, so it starts first and nothing should interrupt it.
- **The kernel lane is independent** of every other lane until E5. It can run from the first day beside syntax, and it is the second longest chain (K1 to K5, E6, E7).
- **M0 is paper** and starts on the first day, so that the gate is open by the time S5 and E4 land.
- **Three lanes at a time is the useful width**: syntax, kernel, and one of harness or elaborator. After wave 6 the lanes are elaborator, mutation, and ownership.
- **E9, K8, R2, and R3 are fillers**: small, off the long chains, good for an idle lane.

### Who owns which files

Parallel work collides in files, not in ideas. Each lane owns directories, and the shared files are named so that a commit touching one lands quickly.

| Lane | Owns | Shares |
|---|---|---|
| Syntax | src/lexer.rs, src/parser.rs, src/ast.rs, tests/frontend.rs | none after S5 |
| Kernel | src/kernel, tests/kernel_*.rs, the Kernel contract | none |
| Elaborator | src/elab, tests/elaborate.rs | src/typed/tree.rs with Mutation |
| Mutation | src/exec, src/typed/lower.rs, src/erased | src/typed/tree.rs, src/elab for M2 to M5 |
| Ownership | the move and borrow passes (new files under src/elab), the build command, the Rust caller tests | src/erased/rust.rs with Mutation |
| Harness and robustness | tests/common, tests/corpus, tools | none |

There are no feature branches. Parallel work happens in separate working copies, and each commit is rebased onto main and lands only when tools/check.sh passes on top of main. Commits stay small so that a rebase is cheap.

## Rules for every commit

- tools/check.sh passes: fmt, clippy with warnings denied, and the fast tests with fixed seeds, offline. The extended suite runs before a milestone is declared.
- Nothing in the checker depends on a clock. Limits are counts of work, so that a file accepted on one machine is accepted on every machine. Timeouts exist only in the test harness, where firing means inconclusive.
- New behaviour arrives with accepted and rejected corpus files. A rejected file pins the error code and the line.
- A commit that changes the kernel, the check-IR checker, lower, or erase updates the Kernel contract or Architecture in the same commit, and says in its message what was added to the trusted base.
- The atlas moves in the same commit: the task to done, the tasks it covers, and the Language status rows.
- Old spellings leave with a migration diagnostic that carries a fix, and the fix is tested by applying it.
- A commit that makes a lane wait (a shared file) says so in its task and lands within the day.
- Anything learned that contradicts Target language is recorded there or as an open decision before the commit lands, not worked around in code.

## Kinds of test, and what each is for

| Kind | Guards against | Introduced |
|---|---|---|
| Corpus files, accepted and rejected | A feature that works in one stage of the pipeline and not the next | H1 |
| Differential: two interpreters | lower and erase disagreeing about a program | exists, extended by R1 |
| Compiled Rust against the interpreter, with overflow checks on and off | The printer, and Locus meaning something Rust does not in either build | exists, generalized by H1, E4, E6 |
| rustc as the oracle for rejections | Locus accepting what rustc refuses, in moves, borrows, and visibility | O1, O2, O3 |
| Exhaustive at 8 bits, boundaries and random above | A wrong machine model or operator table, which would be trusted and wrong | K4, K5 |
| Axiom near misses | An axiom stated more strongly than intended | K2 |
| Certificates and proofs against claims known to be false; fabricated constraints | A kernel that accepts too much | K6, R2 |
| Wrong answers from untrusted parts | Trust creeping into the solver, the procedure, or the flow analysis | K7, M4 |
| Random programs, three ways | Everything the hand-written tests did not think of | R1 |
| Parser fuzz; separate tests of progress and depth | Panics and overflows on hostile input; the fuzz shows robustness, the other two are what totality rests on | H2 |
| Golden diagnostics and generated Rust | Messages and output changing by accident | R3 |
| Contract against code | An axiom that is in the kernel and not in the contract | K2 |

## The lanes

- **Harness.** Before anything else, make adding a test cost one file.
- **Syntax.** The parser is finished first and for everything, including what the elaborator will not accept for months. Constructs that are parsed and not yet checked answer with one uniform diagnostic. This takes parser.rs off the path of every other lane.
- **Kernel.** Int and the machine integers, independent of the surface language until E5. Every commit here adds to the trusted base and updates the Kernel contract. The certificate rule of K6 has its whole boundary written, with one complete certificate, before any code.
- **Elaborator.** Semantics that need no mutation: exact evidence, promises, integers, operators, arithmetic in holes, logic-only types. Where a construct needs new checked semantics (operators that may panic, the forms that panic), the check IR and its checker change first and are tested on IR built by hand, and the surface follows.
- **Mutation.** Gated by the design in M0. The check IR changes first, then the surface reaches it.
- **Ownership.** Moves, the export boundary, and references as parameters. O2 depends on little and can land early.
- **Robustness.** Standing tests that grow with the language, and the acceptance commit.

## The commits

One task each, in the Core build project, where the scope, the tests, and the condition for done are written. This list and the table of waves are written by python3 tools/atlas.py plan from the tasks, which are the only record of the order.

### Harness

- **H1** LOC-151. A corpus runner: every .lc file is checked, run in both interpreters, compiled, and compared. After: nothing.
- **H2** LOC-152. A seeded generator with no dependencies, parser fuzz tests, and tools/check.sh in a fast and an extended form. After: nothing.
- **H3** LOC-153. Split src/elab/exprs.rs by construct, with no change in behaviour. After: nothing.

### Syntax

- **S1** LOC-154. The lexer tokenizes as Rust: every keyword reserved, string literals, integer literals in full. After: H2.
- **S2** LOC-155. Built-in forms and the formula grammar: prop!, prove!, @P, forall and exists, rewrite!, unfold!, fold!. After: S1.
- **S3** LOC-156. Rust's operator precedence exactly, with as, unary minus, and + - * / % parsed. After: S2.
- **S4** LOC-157. Items: the closed attribute set, pub and its restricted forms, impl blocks, variants with named fields, long paths, const. After: S3.
- **S5** LOC-158. Statements and expressions: let mut, assignment, return, Rust's loop forms, references, the never type. After: S4.

### Kernel

- **K1** LOC-159. Integers of any size in the kernel: subtraction, multiplication, division, comparison, sign. After: nothing.
- **K2** LOC-160. Int as a kernel type: literals, the ring and order axioms, discreteness, closed evaluation. After: K1.
- **K3** LOC-161. Int division and remainder: truncating, total, with x / 0 == 0. After: K2.
- **K4** LOC-162. A model for each machine integer type, from one table. After: K3.
- **K5** LOC-163. The table of primitive operations: result type, panic condition, logical meaning. After: K4.
- **K6** LOC-164. A kernel rule that checks a linear arithmetic certificate, with its whole boundary written first. After: K2.
- **K7** LOC-165. The arithmetic procedure: finds certificates for linear goals, machine ranges, and division by a literal. After: K4, K6.
- **K8** LOC-166. Lemmas about Int and the machine types, callable by name. After: K4.

### Elaborator

- **E1** LOC-167. Evidence must match exactly after computing; dependent patterns open over their own names. After: S2, H3.
- **E2** LOC-168. Promises: terminates, no_panic, no_alloc, no_io, checked and never inferred. After: S4.
- **E3** LOC-169. Remove math fn. After: E2.
- **E4** LOC-170. A panic is a third outcome: the interpreters, the erased tree, and the harness. After: H1.
- **E5** LOC-171. Every machine integer type, literal typing, as, Int, and comparison within one type. After: K4, S3.
- **E6** LOC-172. The operators + - * / % with their panic conditions and what the logic learns. After: K5, E2, E4, E5, M0.
- **E7** LOC-173. Arithmetic in holes and prove!. After: K7, E1, E6.
- **E8** LOC-174. Logic-only types and the one erasure rule: Ghost<T>, snapshot!, erased positions. After: E5, E2.
- **E9** LOC-175. Variants with named fields, const as a Rust const, separate type and value namespaces. After: S4.
- **E10** LOC-190. The forms that panic: panic!, assert!, unreachable!, todo!, debug_assert!. After: M1, E2, S2.
- **E11** LOC-192. Found proofs are stored beside the source, and locus check --locked never searches. After: E7.

### Mutation

- **M0** LOC-176. Write the design of the checked IR for mutation, early exit, panics, and operations that may panic. After: nothing.
- **M1** LOC-177. The check IR gains return and panic as ways a block ends. After: M0, E4.
- **M2** LOC-178. let mut and assignment, lowered to versions. After: M1, S5.
- **M3** LOC-179. Rust's loops over let mut; the state-passing loop and the bounded for are removed. After: M2.
- **M4** LOC-180. Tracked evidence: let mut evidence, invalidated by assignment, refreshed explicitly. After: M3, E1.
- **M5** LOC-181. return and the never type in the surface language. After: M3.

### Ownership

- **O1** LOC-182. Moves, and derive with a closed list. After: M2, S4.
- **O2** LOC-183. Visibility, the export boundary, and locus build. After: S4, E2.
- **O3** LOC-184. References as parameters: &T and &mut T, path arguments, disjointness, old!. After: O1, M4.
- **O4** LOC-185. Inherent impl blocks and self; the protected type end to end. After: O2, O3.

### Robustness

- **R1** LOC-186. A generator of random well-typed programs, compared three ways. After: H1, H2.
- **R2** LOC-187. Kernel soundness under mutation: no proof is accepted for a claim known to be false. After: H1.
- **R3** LOC-188. Golden diagnostics, and a test that every error code is exercised. After: H1.
- **R4** LOC-189. Acceptance: the target examples run, the legacy is gone, and Now is rewritten. After: E11, E3, E7, E8, E9, E10, M5, O4, K8, R1, R2, R3.

## Decisions

Decided on 21 September, and recorded in Target language: the order of this plan (LOC-136); one kernel rule that checks linear certificates (K6); Seq<T> leaves the core; Percent uses an enum of its own until Option exists; a function that invalidates evidence behind a &mut parameter must promise no_panic (LOC-191).

Still needed:

- **What locus build emits** (LOC-135), before O2.
- **The design of M0** (LOC-129), before M1 and, for its seventh case, before E6. Until it is agreed, the mutation lane does not start.
- **The contract of the arithmetic certificate**, written into the Kernel contract with one complete certificate, before K6 is coded.
- Not blocking: a finite loop that may promise to terminate (LOC-144), a lighter spelling for promises (LOC-143) and for a proposition value (LOC-115). Each is a small change after the core if decided later.

## Left out of the core on purpose

Recursion and decreases, shared references with lifetimes, generics and traits, Seq, Map, and Set, usize and slices, bit operators and shifts, match parity, floats, modules across files and packages, the header and implementation split, and the polish list. Each has its project in the Plan.

## After the core build: reconciling the code with the Vision

Direction agreed 22 September, not yet planned into commits. Three tiers of cost, in order. First, reconcile the core, mostly in the elaborator: logic fn and logic blocks, with an ordinary fn never entering the logic; Bool with the default model rule; prop with named arms over the kernel's existing declared propositions, with proof patterns in ordinary code; Ghost<T> and snapshot! replaced by model observations; the default cleanup of markers in the printer; and the stale paragraphs of Now rewritten. The three target examples in their new spelling are the acceptance test. Second, logical data: generics with Logical bounds, derive(Logical), recursive logical enums with positivity and induction, Seq, and the Model trait; this is where the kernel grows. Third, references beyond one call, heap-state versions, and the generic verified collection of the Design concerns. Implementation does not begin until the first tier is planned.
