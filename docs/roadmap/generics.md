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

Extend the implemented generic/syntax subset without conflating parsing with semantics. General traits ([LOC-21](generics.md#LOC-21)) precede where/impl bounds ([LOC-22](generics.md#LOC-22)), user operators ([LOC-125](generics.md#LOC-125), [LOC-126](generics.md#LOC-126)), dynamic objects ([LOC-34](generics.md#LOC-34)) and runtime callable traits ([LOC-24](generics.md#LOC-24)). Logical closures, type specialization and fixed-width integers are already complete. Independent ergonomic additions remain explicitly scoped below; floating-point and bitwise operations need their own logical models.

<a id="LOC-20"></a>
## LOC-20 · Floating-point types and proof models
<!-- task: {"id": "t25", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Fixed-width integers are complete ([LOC-171](core-build.md#LOC-171)). Remaining: f32/f64 runtime support and an explicit account of NaN, infinities, rounding, comparisons and model soundness. Do not infer this from the integer model or lexer recognition.

<a id="LOC-21"></a>
## LOC-21 · User-defined traits and checked implementations
<!-- task: {"id": "t26", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: trait declarations, implementation selection/coherence, associated types/items and proof/effect contracts. The built-in Logical classification, Model registration and closed derives do not constitute a general trait system. Required by [LOC-22](generics.md#LOC-22), [LOC-125](generics.md#LOC-125) and iterator integration ([LOC-30](memory-layout.md#LOC-30)).

<a id="LOC-22"></a>
## LOC-22 · General generic bounds and where clauses
<!-- task: {"id": "t27", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered: concrete type specialization, inference and Logical bounds ([LOC-218](reconciliation.md#LOC-218), [LOC-219](reconciliation.md#LOC-219); tests/reconcile_generics.rs). Remaining: general where clauses, trait-bound checking, generic impl blocks and a stated policy for checking generic bodies. Current unused templates are not universal proofs. Scoped proof arguments of nonrecursive aggregates are implemented by [LOC-243–246](#LOC-243). Remaining dependent-generic work includes generic functions/propositions with open type arguments, recursive families and arguments combining inner binders with outer dependencies.

<a id="LOC-24"></a>
## LOC-24 · Runtime closures and callable traits
<!-- task: {"id": "t29", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Logical closures with immutable captures and dependent results are complete ([LOC-225](reconciliation.md#LOC-225); tests/reconcile_closures.rs). Remaining: runtime capture ownership, Fn/FnMut/FnOnce calls and emitted Rust closure behavior. Depends on traits ([LOC-21](generics.md#LOC-21)) and broader borrowing ([LOC-33](memory-layout.md#LOC-33)).

<a id="LOC-34"></a>
## LOC-34 · Dynamic trait objects and their proof boundary
<!-- task: {"id": "t41", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Deferred until traits and runtime callable/layout rules exist. Decide supported object-safe contracts, vtables, erased evidence and ownership before adding dyn. [LOC-23](core-build.md#LOC-23) records the earlier non-goal; it is not a second implementation backlog.

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
<!-- task: {"id": "t198", "status": "backlog", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Still open: src/parser.rs::function explicitly requires -> and a result type. Add the Rust-compatible omission with parser/diagnostic and generated-output tests.

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
<!-- task: {"id": "t206", "status": "backlog", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Named structs already support the newtype pattern manually. Remaining: tuple-struct and unit-struct declarations/constructors/patterns, visibility and proof-bearing fields. Enum tuple/unit variants are not completion of this task.

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

Still unresolved before traits. Rust operator syntax/signatures do not expose an extra proof argument, so decide how an operator implementation states its panic precondition and how no_panic callers discharge it. Historical candidates were conditional promises or implicit evidence holes, with the latter constrained by Rust trait signatures. Depends on [LOC-21](generics.md#LOC-21), [LOC-125](generics.md#LOC-125).

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
