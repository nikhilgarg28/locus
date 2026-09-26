+++
summary = "General traits, runtime closures and syntax extensions beyond the current language core."
id = "p24"
name = "Language abstractions and syntax"
status = "planned"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/generics.html"
order = 3
kind = "project"
+++

# Language abstractions and syntax

Extend the implemented generic/syntax subset without conflating parsing with semantics. Concrete traits ([LOC-21](generics.md#LOC-21)) establish the base for where/impl bounds ([LOC-22](generics.md#LOC-22)), user operators ([LOC-125](generics.md#LOC-125), [LOC-126](generics.md#LOC-126)), dynamic objects ([LOC-34](generics.md#LOC-34)) and runtime callable traits ([LOC-24](generics.md#LOC-24)). Logical closures, type specialization and fixed-width integers are already complete. Independent ergonomic additions remain explicitly scoped below; floating-point and bitwise operations need their own logical models.

<a id="LOC-20"></a>
## LOC-20 · Floating-point types and proof models
<!-- task: {"id": "t25", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Fixed-width integers are complete ([LOC-171](core-build.md#LOC-171)). Remaining: f32/f64 runtime support and an explicit account of NaN, infinities, rounding, comparisons and model soundness. Do not infer this from the integer model or lexer recognition.

<a id="LOC-21"></a>
## LOC-21 · User-defined traits and checked implementations
<!-- task: {"id": "t26", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-25T17:27:33.366974+00:00"} -->

Implemented and validated on `feat/native-traits`: concrete traits and implementations, associated types/constants, logical methods, explicit proof slots, inherited defaults checked per implementation, scoped selection and qualification, and physical Rust trait exports/imports. No new proof axiom or execution IR form is introduced. See the [plan](../plans/native-traits.md) and [manual](../spec/20-traits.md). Validation covers focused edge cases, real Cargo/directory fixtures, warning-denied Rust runs, checked documentation and a completed extended gate. Generic bounds are implemented by LOC-22; compiler-integrated traits, supertraits and dynamic dispatch remain separate follow-ups. The built-in Logical classification, Model registration and closed derives remain compiler-owned. Required by [LOC-22](generics.md#LOC-22), [LOC-125](generics.md#LOC-125) and iterator integration ([LOC-30](memory-layout.md#LOC-30)).

<a id="LOC-22"></a>
## LOC-22 · General generic bounds and where clauses
<!-- task: {"id": "t27", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-26T02:51:53.048858+00:00"} -->

Static trait bounds are implemented on `feat/native-traits`: inline and where requirements, associated equality/projection bounds, bound-directed member selection, conditional complete-family implementations and inherent methods, and checked concrete instantiation. Proof and effect semantics reuse the ordinary checker. See the [plan](../plans/trait-bounds.md), [manual](../spec/20-traits.md#generic-bounds) and `tests/trait_bounds.rs`. Validation includes 27 focused regression groups, real module/Cargo fixtures, warning-denied generated Rust, checked documentation, desktop/mobile manual review and a completed extended gate. The debug suite exceeded its advisory 120-second target. The independent checking-interpreter gap for indirect logical callable values is pinned and tracked in LOC-268; that case is not claimed as successful differential coverage. Final timings and freshness are recorded in generated status.

Explicit follow-ups: generic trait parameters/supertraits (LOC-263), method-local parameters (LOC-264), universal generic checking and Rust export (LOC-265), compiler-owned Model/other bound integration (LOC-266), and broader implementation patterns/projection normalization (LOC-267). Scoped proof arguments of nonrecursive aggregates remain implemented by LOC-243–246. Generic functions/propositions with open proof arguments and recursive dependent families remain separate work.

<a id="LOC-24"></a>
## LOC-24 · Runtime closures and callable traits
<!-- task: {"id": "t29", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Logical closures with immutable captures and dependent results are complete ([LOC-225](reconciliation.md#LOC-225); tests/reconcile_closures.rs). Remaining: runtime capture ownership, Fn/FnMut/FnOnce calls and emitted Rust closure behavior. Depends on traits ([LOC-21](generics.md#LOC-21)) and broader borrowing ([LOC-33](memory-layout.md#LOC-33)).

<a id="LOC-34"></a>
## LOC-34 · Shared dynamic trait objects
<!-- task: {"id": "t41", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented the [borrowed dyn plan](../plans/borrowed-dyn.md): shared physical interfaces, fixed associated types, checked implementation selection, opaque snapshots, both interpreters and real Rust trait-object emission. Shared locals, fields and input-linked results reuse the existing provenance checker. Validation covers hostile tables, lifetime/ownership failures, runtime branch selection and a completed extended gate. The plan records the coverage and measured validation results. Broader interface shapes and ownership are LOC-269; proof-bearing objects are LOC-270; external Rust object identities are LOC-271. [LOC-23](core-build.md#LOC-23) records the earlier non-goal.

<a id="LOC-69"></a>
## LOC-69 · Question-mark propagation and let-else
<!-- task: {"id": "t86", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Early return is delivered ([LOC-181](core-build.md#LOC-181)), and Option/Result data exist ([LOC-68](core-build.md#LOC-68)). Remaining: checked let-else and ? propagation, including divergence rules, residual/trait semantics and proof fields on each path. Depends on richer patterns ([LOC-102](memory-layout.md#LOC-102)) and traits where the chosen ? model requires them.

<a id="LOC-71"></a>
## LOC-71 · matches! and the remaining Rust-style built-in behavior
<!-- task: {"id": "t88", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

panic!, assert!, unreachable!, todo! and debug_assert! are implemented ([LOC-190](core-build.md#LOC-190); tests/exec_endings.rs). matches! is recognized but still reports L0290 in src/elab/forms.rs. Implement its patterns and branch evidence without silently widening the current pattern grammar; depends on [LOC-102](memory-layout.md#LOC-102).

<a id="LOC-104"></a>
## LOC-104 · Omit -> () for unit-returning functions
<!-- task: {"id": "t198", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-25T17:07:25.165396+00:00"} -->

Delivered: Rust-compatible unit-result omission for declarations, methods and headers, with parser/diagnostic, spec matching and generated-output tests. The complete extended gate passed; see the [implementation and validation record](../plans/platform-types-and-structs.md).

<a id="LOC-106"></a>
## LOC-106 · Struct update syntax
<!-- task: {"id": "t200", "status": "backlog", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Field-init shorthand is implemented in src/parser.rs::struct_literal and examples. Remaining: ..base updates, ownership of unchanged fields and dependent proof-field revalidation. Do not mark this complete from shorthand support alone.

<a id="LOC-110"></a>
## LOC-110 · Emit documentation and selected source attributes
<!-- task: {"id": "t204", "status": "backlog", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Doc comments are lexed/stored (tests/frontend.rs); general pass-through to generated Rust is not established. Remaining: supported output policy and tests for item/field docs and a reviewed attribute set. Do not forward attributes that bypass checking or change layout/effects without a contract.

<a id="LOC-111"></a>
## LOC-111 · Explicit enum discriminants
<!-- task: {"id": "t205", "status": "backlog", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: parsing, type/layout constraints, Rust output and logical case semantics for user-written discriminants. Current enum case indices are compiler-managed.

<a id="LOC-112"></a>
## LOC-112 · Tuple and unit structs
<!-- task: {"id": "t206", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-25T17:07:25.165396+00:00"} -->

Delivered: nominal tuple/unit declarations, constructors, positional access and irrefutable patterns; dependent proof fields, privacy, moves, logical erasure, models, specs and Rust exports share the existing checked product semantics. `tests/struct_forms.rs` covers positive cases and hostile clients. The complete extended gate passed; see the [implementation and validation record](../plans/platform-types-and-structs.md).

<a id="LOC-116"></a>
## LOC-116 · Bit operators and shifts with a checked model
<!-- task: {"id": "t227", "status": "backlog", "priority": 0, "created": "2026-09-21T20:34:38.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: &, |, ^, integer !, << and >>, including shift-width obligations and signed behavior. The original arithmetic core deliberately excluded these because they need a bit-level model; ordinary arithmetic completion does not close this task.

<a id="LOC-125"></a>
## LOC-125 · User-defined operator traits
<!-- task: {"id": "t239", "status": "backlog", "priority": 0, "created": "2026-09-21T20:54:27.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: Add/Sub/Mul/Div/Rem/Neg/Not/comparison/Index implementations with checked conditions, generated Rust compatibility and logical models. Depends on [LOC-21](generics.md#LOC-21) and the contract decision [LOC-126](generics.md#LOC-126). Ordinary fn implementations will not become logical merely by effect promises; the current logic/runtime split must be preserved.

<a id="LOC-126"></a>
## LOC-126 · Contracts for fallible user-defined operators
<!-- task: {"id": "t240", "status": "todo", "priority": 1, "created": "2026-09-21T20:54:27.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Still unresolved before user-defined operator traits. Rust operator syntax/signatures do not expose an extra proof argument, so decide how an operator implementation states its panic precondition and how no_panic callers discharge it. Historical candidates were conditional promises or implicit evidence holes, with the latter constrained by Rust trait signatures. Depends on [LOC-21](generics.md#LOC-21), [LOC-125](generics.md#LOC-125).

## Scoped proof arguments

Support `Option<@Sorted(items)>`: a runtime container whose proof payload refers to the input's immutable logical snapshot. Its runtime tag survives; its proof payload and logical type parameters erase. This extends generic checking, not the runtime layout language. Implementation proceeds on `scoped-proof-generics`.

The compiler must preserve the proposition in the type through construction, matching, arguments, results, nested containers, and substitution. Different claims remain different checking types even when their Rust layout is identical. SSA identities distinguish shadowed names and changed contents. Old immutable evidence retains its old claim; it cannot certify changed data without a checked transport. Tracked evidence must retain its invalidation rules through the new type forms.

Use explicitly bound logical parameters on nominal type families, with checked applications of those families. Abstract proof claims during generic specialization rather than emitting global declarations containing free local names. Constructors and eliminators instantiate their payload types with the application's arguments. The kernel and independent execution-IR checker must reject dangling parameters, wrong arguments, and payloads of another claim. All existing proof/ownership/export restrictions continue to apply.

Acceptance: a checked `binary_search(items, key, sorted: @Sorted(items))` and `search(items, key, sorted: Option<@Sorted(items)>)`, with `Some` forwarding the proof and `None` executing linear search. Define sortedness and the algorithms in source, state precisely which correctness properties are proved, and exercise empty inputs, duplicates, absent keys, endpoints, and both dispatch paths. The short website excerpt hides supporting definitions but links/discloses the complete checked program.

<a id="LOC-243"></a>
## LOC-243 · Scoped nominal families and independent checking
<!-- task: {"id": "t243-scoped", "status": "done", "priority": 1} -->

Add logical parameters/applications to the checking representation, bind/substitute them capture-free, and validate constructor and match payloads independently. Keep kernel scope/depth/positivity checks and equality rules explicit. Add direct adversarial kernel and execution-IR tests. Update the kernel and formal-core contracts and trusted-base inventory where needed.

Implemented in `src/kernel/term.rs`, `defs.rs`, and `check.rs`, with certificate encoding in `src/store/text.rs`. Eight direct tests in `tests/kernel_scoped.rs` check constructors, projections, substitution, corrupt certificates, independent execution-IR matches, and forbidden recursion hidden in indices. Kernel depth regressions pass without changing the resource limits.

<a id="LOC-244"></a>
## LOC-244 · Generic elaboration, snapshots, and erasure
<!-- task: {"id": "t244-scoped", "status": "done", "priority": 1} -->

Replace the closed-proof-argument restriction with scoped application checking for supported generic aggregates. Cover expected-type inference for Some/None, calls and returns, nested containers, shadowing, branch joins, mutation, and stale tracked evidence. Separate logical argument identity from runtime representation; retain enum tags and ordinary effects. Preserve export rejection for externally supplied evidence. Document precise unsupported cases rather than accepting unchecked fallbacks.

Implemented in generic specialization and the typed/checking/erased representations. `tests/scoped_generics.rs` covers nested containers, exact snapshot identity, expected-type inference, normal-return mutation, scope escape, tracked invalidation, effects, and export rejection. The manual lists the remaining restrictions on recursive families and open arguments to generic functions.

<a id="LOC-245"></a>
## LOC-245 · Checked optional-evidence search example
<!-- task: {"id": "t245-scoped", "status": "done", "priority": 1} -->

Write a source definition of Sorted, binary search requiring its proof, and the optional-evidence dispatcher with linear fallback. Test real search behavior and proof transport, reject mismatched/stale evidence and false correctness claims. Replace homepage/examples design sketches with checked excerpts whose complete sources are tested by the documentation harness.

`examples/optional_search.lc` defines sortedness, both algorithms, and the dispatcher without trusted assumptions. Logical sortedness agrees with an independent adjacent-pair oracle for 341 finite lists. Both interpreters and generated Rust with overflow checks enabled/disabled agree on 2,055 input/key/dispatch scenarios. The proof contract establishes the sorted input and no-panic bounds/arithmetic; result completeness and termination are tested, not claimed as proved. Homepage and examples excerpts are checked by the documentation harness.

<a id="LOC-246"></a>
## LOC-246 · Scoped evidence validation and documentation
<!-- task: {"id": "t246-scoped", "status": "done", "priority": 1} -->

Add focused source regressions and generated-Rust tests for positive/negative cases, erasure and export boundaries. Run affected suites, executable documentation checks, website checks, and the extended compiler gate, including differential interpreter/Rust runs. Review diagnostics and public examples on desktop/mobile. Record only actual passing validation and leave incomplete work open.

Completed: `tools/check.sh --extended` passed the standard and release stress suites, spec/test traceability, checked documentation, website/link checks, highlighting, formatting and clippy. Desktop/mobile browser review covered both excerpts, search and complete-source disclosures. The standard suite exceeded its advisory 120-second target; this is not represented as a performance pass. The small-stack evaluator and certificate-reader regressions pass at their original bounds. Final validation is recorded by the gate receipt and freshness-aware generated status.

<a id="LOC-247"></a>
## LOC-247 · Reference-transparent logical observation
<!-- task: {"id": "t247-observation", "status": "done", "priority": 1} -->

Normalize outer shared references in logical function declarations and arguments: `T`, `&T`, and `&&T` describe the same observed contents. Preserve exact snapshots and read permissions, leave nested reference fields and runtime calling conventions unchanged, and apply canonical models only for logical parameter types. Model implementations follow the same observation convention; `model!(path)` continues to select a physical path explicitly.

Implementation: normalize parameter types/layouts and generic inference; check observed arguments without consuming places; preserve eager runtime effects and validate reference provenance before erasure. Update the sorted-search example and public excerpts to use `Sorted(items)`.

Acceptance: cross-product tests of declaration/argument reference depths, equivalent proof identities, generic inference, custom models, logical methods, mutation and stale references, non-Copy reuse, nested runtime moves/effects, wrapper rejection, and unchanged runtime calls. Run focused tests, checked documentation and the full compiler gate. No new kernel axiom or proof rule is required.

Implemented in logical parameter/call elaboration, model registration, callable checking and generic inference. `tests/logical_observations.rs` adds fifteen focused regression groups, including a declaration/argument depth matrix and interpreter/Rust comparisons for eager effects and structural models. The checked search example now uses `Sorted(items)` and replays its existing certificates. Desktop/mobile review covers the public example and spec test disclosures. The final validation gate is `tools/check.sh`; timings and generated status remain governed by the normal freshness policy.

<a id="LOC-251"></a>
## LOC-251 · Const functions across the language and Rust boundary
<!-- task: {"id": "language-251", "status": "backlog", "priority": 2} -->

Implement `const fn` as a checked physical function callable both at runtime and in constant initializers. Cover free functions, inherent methods, module paths, spec headers and matching implementations, generated Rust and proof-result facades where supported. Extend trait methods and native imports when those facilities exist; unsupported combinations must report a specific limitation. Const capability is part of an interface contract: an ordinary implementation cannot satisfy a const header. Coordinate specs and native binding with [LOC-250](interop.md#LOC-250) and [LOC-259](interop.md#LOC-259).

Define the permitted constant-evaluation subset and its calls, local mutation, control flow, borrowing, allocation/destruction restrictions and evaluation limits. Preserve checked machine arithmetic and target behavior. A constant-evaluation panic or resource limit needs a diagnostic; reaching an evaluation limit is not evidence of divergence or a proof. `const fn` does not imply `logic fn`, totality or absence of runtime panics, and does not by itself admit physical calls into propositions. Any use in proofs must retain the checked model/kernel boundary.

Acceptance: initializer/runtime results agree with both interpreters and compiled Rust; overflow and division failures are stable across build modes; non-const calls in constant contexts, invalid effects, cycles and limits are diagnosed. Test signatures across files, visibility, exported const-callable facades, imported native constness and toolchain compatibility without assuming values or behavior from a signature. Update grammar, manual examples, diagnostics, IR/erasure contracts and import generation together. General const generics remain a separate design question.

<a id="LOC-261"></a>
## LOC-261 · Trait promises and universal defaults
<!-- task: {"id": "traits-261", "status": "backlog", "priority": 2} -->

Revisit the language's promise design and define which effect/termination guarantees a trait interface may impose on all implementations. Keep exporting traits with promises forbidden until Rust implementations can satisfy the same obligations through a defined boundary. Decide whether sealing, checked adapters or a different contract model is appropriate; do not infer guarantees from `&self`.

Defaults currently receive checks for each concrete implementation. Before exporting reusable Rust default bodies or claiming universal checking of unused defaults, check them against the abstract trait interface, including override-dependent logical definitions, associated bindings and cycles. Today generated Rust implementations contain the checked defaults while the exported Rust trait exposes required signatures. Coordinate general bounds in LOC-22 and interior mutability in LOC-47.

<a id="LOC-262"></a>
## LOC-262 · Trait implementations beyond named local types
<!-- task: {"id": "traits-262", "status": "backlog", "priority": 1} -->

Concrete implementations currently lower to checked inherent helpers on named Locus structs, enums and opaque spec types. Extend dispatch and emission to primitives, references and built-in containers without generating illegal Rust inherent implementations. Preserve trait/type identity, coherence, scoped lookup, ownership and erasure; keep compiler-integrated traits separate. Generic implementations belong to [LOC-22](#LOC-22), native opaque types and cross-package ABI work to [LOC-44](interop.md#LOC-44).

Acceptance: positive and conflicting implementations, qualified and method-call selection, proof contracts, interpreter agreement and warning-denied generated Rust for each admitted target family. Do not admit a target merely because its type grammar parses.

<a id="LOC-263"></a>
## LOC-263 · Generic trait parameters and supertraits
<!-- task: {"id": "bounds-263", "status": "backlog", "priority": 1} -->

Add trait type parameters and inherited requirements, including associated-item bounds beyond Logical. Preserve trait argument identity through native imports, selection, ambiguity/coherence and erasure. Check implied obligations without unbounded recursive expansion; test diamond inheritance and defaults. Generic associated types, negative bounds and specialization require separate designs.

<a id="LOC-264"></a>
## LOC-264 · Method-local type parameters
<!-- task: {"id": "bounds-264", "status": "backlog", "priority": 1} -->

Add independent generic parameters to inherent, trait and spec methods. Existing method `where` clauses can constrain enclosing family parameters. Extend inference, receiver handling, explicit method turbofish, signature matching and concrete specialization without changing borrow/evaluation order. Cover a generic method on a generic owner and logical/proof-dependent results.

<a id="LOC-265"></a>
## LOC-265 · Universal generic checking and open Rust exports
<!-- task: {"id": "bounds-265", "status": "backlog", "priority": 1} -->

Represent abstract types, projections and logical trait operations throughout checking. Check unused generic bodies and proofs from interface laws once, independent of concrete definitions. Preserve explicit proof slots and total logical calls. Define when an open generic Rust export may admit arbitrary Rust implementations without dropping logical bounds. Coordinate default checking and promises with LOC-261; retain concrete instantiation checking until this is complete.

<a id="LOC-266"></a>
## LOC-266 · Compiler-owned interfaces as bounds
<!-- task: {"id": "bounds-266", "status": "backlog", "priority": 1} -->

Integrate canonical Model registration and other compiler-known capabilities into the ordinary obligation machinery. Logical classification is already supported. Define `<T as Model>::Logic` for primitives, registered/derived models and native storage observations without inventing a second model or admitting runtime reads from erased data. Copy, operator and derive integration must preserve their existing compiler rules; source trait declarations cannot impersonate them.

<a id="LOC-267"></a>
## LOC-267 · Broader implementation patterns and projection normalization
<!-- task: {"id": "bounds-267", "status": "backlog", "priority": 2} -->

Extend complete named-family implementations to partial type patterns, bare-parameter blanket implementations, and nested/chained associated projections. Define conservative overlap checking across packages and recursive obligations. Normalize equivalent associated constraints for spec-family matching. Keep specialization and proof-driven implementation selection out unless separately designed. Coordinate primitive/reference targets with LOC-262 and native generic forwarding with LOC-44.

<a id="LOC-268"></a>
## LOC-268 · Indirect logical calls in the checking interpreter
<!-- task: {"id": "bounds-268", "status": "backlog", "priority": 1} -->

Extend the independent checking-IR interpreter to recognize logical callable values through bindings and projections. A literal lambda already skips its logical computation; `let f = |x: T| x; f(value)` can instead report `a call through a function value`. Source checking and erased execution support this form. `tests/trait_bounds.rs` pins the exact interpreter limitation while checking the accepted program and erased result; an unexpected pass must remove the known-bug marker. Cover proof-valued results and preserve ordinary callee/argument effects before claiming differential coverage for this case.

<a id="LOC-269"></a>
## LOC-269 · Broader dyn compatibility and ownership
<!-- task: {"id": "dyn-269", "status": "backlog", "priority": 1} -->

Extend shared physical objects to generic implementation families and nominal/reference method signatures. Add compiler-owned Sized bounds and Self: Sized method exclusions before supporting general ?Sized helpers; reject unsized value positions independently of trait lookup. Coordinate supertrait/upcasting and generic methods with LOC-263–264, general DSTs with LOC-50, mutable receivers/objects with LOC-33 and owned objects/destruction with LOC-48. Define auto-trait, downcasting and lifetime rules before admitting those forms. Each added form must agree in both interpreters and warning-denied Rust, with hostile borrow/layout cases.

<a id="LOC-270"></a>
## LOC-270 · Logical observers and proof-bearing dyn interfaces
<!-- task: {"id": "dyn-270", "status": "backlog", "priority": 1} -->

Define abstract logical observations, method input/output evidence and hidden concrete type identity for dynamic interfaces. Keep proofs tied to the selected implementation and the right snapshots; erasure must not permit substituting an unrelated marker or observer. Decide admissible associated logical types and laws, and account for mutation/interior mutability (LOC-47) and promises (LOC-261). The physical dyn slice supplies no such facts or assumptions.

<a id="LOC-271"></a>
## LOC-271 · Rust trait-object import and export identity
<!-- task: {"id": "dyn-271", "status": "backlog", "priority": 1} -->

Preserve original native/source trait identity across a public dyn ABI, including associated bindings, supported receivers, default methods and lifetime bounds. Admit arbitrary Rust implementors only for interfaces whose guarantees Rust actually enforces. Reject logical/proof/promise leakage throughout reachable public types. Test rustdoc imports, Cargo target/features, native caller-provided objects and hostile reimplementations. Internal specialized Rust dyn traits from LOC-34 are deliberately private and cannot be reused as an exported source-trait identity.
