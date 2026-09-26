+++
summary = "Completed compiler foundations, delivered language features and the design decisions they replaced."
id = "p266"
created = "2026-09-21T22:14:21.000Z"
name = "Core foundations and decision history"
status = "done"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/core-build.html"
order = 0
kind = "project"
+++

# Core foundations and decision history

Completed foundation work, the feature requests it delivered, and superseded design decisions. This project consolidates the old Core Language, Kernel arithmetic, visibility and settled Open decisions lists so they no longer appear as duplicate backlogs. Historical implementation notes retain original scope and acceptance evidence; they are not the current language reference. In particular math-by-promises, Ghost/snapshot and v1 sidecars were replaced by [LOC-210](reconciliation.md#LOC-210), [LOC-209](reconciliation.md#LOC-209) and [LOC-232](process.md#LOC-232).

Historical acceptance: the original Core build completed 41 commits in 12 waves with 549 tests. Its per-commit policy required fmt, warning-free clippy, fast tests and synchronized specification/contract updates. Timing targets recorded in old tasks are goals or measurements of that run, not assertions about today; current measurements come from Generated status.

<a id="LOC-1"></a>
## LOC-1 · Immutable and mutable bindings
<!-- task: {"id": "t3", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-178](core-build.md#LOC-178); `src/elab/mutation.rs`, `tests/corpus/accept/tracked_refresh.lc`, and `tests/elaborate.rs` implement assignment through checked SSA versions.

<a id="LOC-2"></a>
## LOC-2 · Core data, propositions and proof types
<!-- task: {"id": "t4", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-157](core-build.md#LOC-157), [LOC-171](core-build.md#LOC-171) and [LOC-206](reconciliation.md#LOC-206): fixed-width integers, bool, products, nominal types, dependent proof fields and function declarations are implemented; `tests/kernel_products.rs`, `tests/reconcile_props.rs` and `tests/elaborate.rs` exercise them.

<a id="LOC-3"></a>
## LOC-3 · Unbounded Int and model casts
<!-- task: {"id": "t5", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-160](core-build.md#LOC-160) and [LOC-171](core-build.md#LOC-171); `src/kernel/int.rs`, `src/elab/literals.rs`, `tests/kernel_int.rs` and `tests/elaborate_operators.rs`.

<a id="LOC-4"></a>
## LOC-4 · Nonrecursive structs and enums
<!-- task: {"id": "t6", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-157](core-build.md#LOC-157); `src/elab/items.rs`, `tests/kernel_products.rs` and `tests/elaborate.rs`. Recursive logical data and boxed runtime enums were added separately in [LOC-221](reconciliation.md#LOC-221) and [LOC-231](reconciliation.md#LOC-231).

<a id="LOC-5"></a>
## LOC-5 · Control flows -- if/else, loop, match
<!-- task: {"id": "t7", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-21T19:19:39.000Z"} -->

Delivered. `src/elab/control.rs` and `src/elab/loops.rs` implement if/else, match and loops; `tests/exec_check.rs` and the corpus exercise the checked/erased/Rust paths. More pattern syntax and iterator loops remain [LOC-102](memory-layout.md#LOC-102) and [LOC-30](memory-layout.md#LOC-30).

<a id="LOC-6"></a>
## LOC-6 · IR hierarchy -> frontend to elaborated IR (EIR) -> one branch takes to kernel IR and another branch does erasure & generated IR
<!-- task: {"id": "t8", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-21T19:19:39.000Z"} -->

Delivered. `src/typed`, `src/exec`, `src/kernel` and `src/erased` separate elaborated structure, checked execution IR, proof checking and erased Rust emission. `tests/typed_lower.rs`, `tests/exec_check.rs` and `tests/random_programs.rs` verify the interfaces; the preservation theorem remains [LOC-57](assurance.md#LOC-57).

<a id="LOC-7"></a>
## LOC-7 · locus check and locus build
<!-- task: {"id": "t9", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-183](core-build.md#LOC-183); `src/main.rs`, `src/build.rs` and `tests/build.rs`. Checking, readable Rust crate output and protected Rust exports are implemented; this does not imply Cargo dependency integration.

<a id="LOC-8"></a>
## LOC-8 · Sidecar proof storage superseded by Locus.lock
<!-- task: {"id": "t10", "status": "canceled", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. The original sidecar implementation shipped as [LOC-192](core-build.md#LOC-192). The active format and migration are complete in [LOC-232](process.md#LOC-232); this is not an additional backlog for writing sidecars.

<a id="LOC-9"></a>
## LOC-9 · Machine casts through as
<!-- task: {"id": "t11", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-171](core-build.md#LOC-171); `src/elab/literals.rs`, `tests/kernel_machine.rs` and `tests/elaborate_operators.rs` cover truncation/wrapping and mathematical model observations.

<a id="LOC-10"></a>
## LOC-10 · comments
<!-- task: {"id": "t12", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-21T19:19:39.000Z"} -->

Delivered. The lexer handles source comments and frontend tests cover them. Item doc comments are retained in the AST; their generated-Rust policy is separately tracked in [LOC-110](generics.md#LOC-110).

<a id="LOC-11"></a>
## LOC-11 · Arithmetic operators + - * / %
<!-- task: {"id": "t13", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-172](core-build.md#LOC-172); the operation table in `src/kernel/ops.rs`, `tests/operators.rs` and `tests/elaborate_operators.rs` cover runtime meaning and checked logical equations. Bit operations remain [LOC-116](generics.md#LOC-116).

<a id="LOC-12"></a>
## LOC-12 · Overflow obligations under no_panic
<!-- task: {"id": "t14", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-168](core-build.md#LOC-168) and [LOC-172](core-build.md#LOC-172); `tests/exec_operations.rs`, `tests/operators.rs` and the midpoint acceptance example distinguish ordinary wrapping/panic behavior from a checked no_panic promise.

<a id="LOC-13"></a>
## LOC-13 · Checked runtime effect promises
<!-- task: {"id": "t15", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-168](core-build.md#LOC-168); `src/exec/check.rs` and `tests/exec_endings.rs` cover terminates, no_panic, no_alloc and no_io. These promises do not make an ordinary function callable in logic; [LOC-210](reconciliation.md#LOC-210) defines that boundary.

<a id="LOC-14"></a>
## LOC-14 · Mathy ordinary functions superseded by logic fn
<!-- task: {"id": "t16", "status": "canceled", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. The old question is settled by [LOC-193](core-build.md#LOC-193)/LOC-210: ordinary fn never enters logic merely because it is pure or promised total. Logical definitions are explicitly logic fn.

<a id="LOC-15"></a>
## LOC-15 · Explicit proof construction
<!-- task: {"id": "t17", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-155](core-build.md#LOC-155) and [LOC-212](reconciliation.md#LOC-212); `prove!`, explicit proof arguments/results, named proposition arms and fold/unfold/rewrite are exercised by `tests/reconcile_props.rs` and `examples/proofs.lc`.

<a id="LOC-16"></a>
## LOC-16 · Tracked evidence for mutable state
<!-- task: {"id": "t18", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-180](core-build.md#LOC-180) and [LOC-213](reconciliation.md#LOC-213); `src/elab/mutation.rs`, `tests/reconcile_scope.rs` and tracked-evidence corpus cases verify invalidation and refresh.

<a id="LOC-17"></a>
## LOC-17 · Exact facts fill basic proof holes
<!-- task: {"id": "t19", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-167](core-build.md#LOC-167); `src/elab/solve.rs`, `tests/elaborate.rs` and `tests/store.rs` cover exact evidence and checked replay. Stronger heuristics are tracked in [LOC-51](proof-automation.md#LOC-51).

<a id="LOC-18"></a>
## LOC-18 · Restricted visibility and protected exports
<!-- task: {"id": "t21", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-183](core-build.md#LOC-183); `src/elab/items.rs`, `tests/build.rs` and `tests/methods.rs` cover pub, restricted forms and proof-bearing export rejection. Full module resolution is still [LOC-41](interop.md#LOC-41).

<a id="LOC-19"></a>
## LOC-19 · Safe Rust callers cannot forge exported invariants
<!-- task: {"id": "t22", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-183](core-build.md#LOC-183) and [LOC-185](core-build.md#LOC-185); `tests/build.rs` and `tests/acceptance.rs::a_rust_caller_is_held_at_the_boundary` check private fields, non-public marker construction and exclusion of evidence-taking public APIs. The guarantee applies to the generated crate boundary, not unsafe Rust or arbitrary same-crate insertion.

<a id="LOC-23"></a>
## LOC-23 · No dyn in the initial generic tier
<!-- task: {"id": "t28", "status": "canceled", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. This was a scope exclusion, not implementation work. The actual deferred dynamic-object design is [LOC-34](generics.md#LOC-34).

<a id="LOC-25"></a>
## LOC-25 · Closed derives for runtime and Logical types
<!-- task: {"id": "t30", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-182](core-build.md#LOC-182) and [LOC-220](reconciliation.md#LOC-220); `src/elab/items.rs`, `tests/moves.rs` and `tests/logical_data.rs` cover Clone/Copy/PartialEq/Eq/Debug and derive(Logical), including refusal to expose erased information. This is a closed built-in list, not user-defined traits.

<a id="LOC-27"></a>
## LOC-27 · Type parameters in the kernel and logical definitions
<!-- task: {"id": "t32", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-218](reconciliation.md#LOC-218) and [LOC-219](reconciliation.md#LOC-219); `tests/kernel_generics.rs` and `tests/reconcile_generics.rs` check substitution and concrete specializations. Unused generic bodies are not universally verified.

<a id="LOC-28"></a>
## LOC-28 · while loops over checked mutable state
<!-- task: {"id": "t34", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-179](core-build.md#LOC-179); `src/elab/loops.rs`, `tests/exec_check.rs` and loop corpus cases. A while loop does not thereby gain a termination proof.

<a id="LOC-32"></a>
## LOC-32 · Arrays, slices and Vec in the supported collection tier
<!-- task: {"id": "t39", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-228](reconciliation.md#LOC-228) and [LOC-229](reconciliation.md#LOC-229); `tests/collections.rs`, `tests/logical_containers.rs` and `tests/reconcile_collection.rs` cover lengths, checked bounds, reads, writes, push and element erasure. Runtime lengths and indices migrate to usize under [LOC-103](memory-layout.md#LOC-103). A complete standard Vec API remains [LOC-55](memory-layout.md#LOC-55).

<a id="LOC-37"></a>
## LOC-37 · Owned values, moves and closed Clone/Copy derivation
<!-- task: {"id": "t44", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-182](core-build.md#LOC-182); `src/elab/moves.rs`, `tests/moves.rs` and generated-Rust rejection oracles. Drop and interior mutability have separate open work.

<a id="LOC-38"></a>
## LOC-38 · Inherent impl blocks and self receivers
<!-- task: {"id": "t46", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-185](core-build.md#LOC-185); `tests/methods.rs` and `tests/references.rs` cover value/shared/mutable receivers and dependent results. Generic trait impl blocks remain part of [LOC-21](generics.md#LOC-21), [LOC-22](generics.md#LOC-22).

<a id="LOC-54"></a>
## LOC-54 · Checked induction in the kernel
<!-- task: {"id": "t66", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-221](reconciliation.md#LOC-221) and [LOC-222](reconciliation.md#LOC-222); `tests/kernel_recursive.rs`, `tests/kernel_inductive_props.rs`, `tests/kernel_measured.rs` and `tests/logical_data.rs` cover structural data/proof induction and bounded Int descent. This is not a claim of arbitrary recursive-function support.

<a id="LOC-59"></a>
## LOC-59 · locus audit
<!-- task: {"id": "t73", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-196](process.md#LOC-196); `src/audit.rs`, `tests/audit.rs` and `tests/audit/` verify checked discovery and divergence, panic, allocation, classical and trusted-contract reporting.

<a id="LOC-61"></a>
## LOC-61 · Versioned JSON diagnostics
<!-- task: {"id": "t76", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-200](process.md#LOC-200); `src/diagnostic/json.rs`, `tests/diagnostics_json.rs` and committed diagnostic goldens.

<a id="LOC-65"></a>
## LOC-65 · Model reference and checked examples
<!-- task: {"id": "t80", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-224](reconciliation.md#LOC-224) and [LOC-227](reconciliation.md#LOC-227); `library/README.md`, `library/buffer_model.lc`, `library/runtime_list.lc`, `tests/reconcile_models.rs`, `tests/reconcile_library_cli.rs` and the target corpus demonstrate model definition, observation and proof use.

<a id="LOC-66"></a>
## LOC-66 · Existential introduction and elimination
<!-- task: {"id": "t82", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-226](reconciliation.md#LOC-226); checked Exists/ForAll library constructors, witness rules and keyword sugar are exercised in `tests/reconcile_quantifier_library.rs`. Witness elimination cannot extract runtime data from erased evidence.

<a id="LOC-68"></a>
## LOC-68 · Option and Result with generic payloads
<!-- task: {"id": "t85", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-219](reconciliation.md#LOC-219); `tests/reconcile_generics.rs::option_and_result_prelude_keep_runtime_tags` and erasure cases. These compile to specialized generated enums, not Rust std::Option/std::Result ABI aliases; mapping those ABIs belongs to [LOC-44](interop.md#LOC-44).

<a id="LOC-70"></a>
## LOC-70 · Richer patterns consolidated
<!-- task: {"id": "t87", "status": "canceled", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. The substantive pattern backlog is [LOC-102](memory-layout.md#LOC-102), with guards tracked separately by [LOC-86](memory-layout.md#LOC-86). No independent duplicate batch remains.

<a id="LOC-72"></a>
## LOC-72 · Resolve 32-bit overflow proof construction
<!-- task: {"id": "t90", "status": "done", "priority": 2, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-131](core-build.md#LOC-131), [LOC-164](core-build.md#LOC-164), [LOC-165](core-build.md#LOC-165) and [LOC-173](core-build.md#LOC-173) delivered the checked linear-arithmetic path used by midpoint. `tests/arith.rs`, `tests/kernel_linear.rs` and `tests/acceptance.rs` cover it; enumeration of byte values is not the solution.

<a id="LOC-73"></a>
## LOC-73 · Three-promise logical-call rule superseded
<!-- task: {"id": "t91", "status": "canceled", "priority": 2, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. The original proposed replacement for math fn was superseded by [LOC-193](core-build.md#LOC-193)/LOC-210: pure/total logical functions require logic fn; runtime promises remain about runtime behavior.

<a id="LOC-74"></a>
## LOC-74 · Consolidate proof-automation planning
<!-- task: {"id": "t92", "status": "done", "priority": 2, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. The formerly empty stronger-inference project and the proof-automation/logic-growth backlogs are consolidated into Proof ergonomics and model libraries. [LOC-51](proof-automation.md#LOC-51) owns additional hole search; [LOC-52](proof-automation.md#LOC-52) owns proved lemmas; [LOC-84](proof-automation.md#LOC-84) owns explicit search hints. No additional solver is implied.

<a id="LOC-75"></a>
## LOC-75 · Make remaining dependencies explicit
<!-- task: {"id": "t93", "status": "done", "priority": 2, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. This roadmap audit records dependencies in each surviving project. Logical closures and basic Vec/model support are already delivered. General iterators wait for traits and runtime closures; crate integration waits for modules; formal mechanization starts from [LOC-203](process.md#LOC-203). Existing historical [LOC-136](core-build.md#LOC-136) and Reconciliation wave dependencies are retained.

<a id="LOC-76"></a>
## LOC-76 · Reference tiers
<!-- task: {"id": "t94", "status": "done", "priority": 2, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-184](core-build.md#LOC-184) delivered call-duration &T/&mut T parameters; [LOC-230](reconciliation.md#LOC-230) delivered stored shared references with explicit lifetime provenance and last-use checks. `tests/references.rs` and `tests/reconcile_shared.rs` pin the tiers. Broader mutable-reference storage and Rust borrowing remain [LOC-33](memory-layout.md#LOC-33).

<a id="LOC-77"></a>
## LOC-77 · prove! with an explicit proposition
<!-- task: {"id": "t170", "status": "done", "priority": 0, "created": "2026-09-21T19:37:49.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-155](core-build.md#LOC-155)/LOC-173 and `src/elab/forms.rs` implement local checked evidence. The current spelling is @P (or @(formula)); runtime representation is the single Erased marker, not the original Proved spelling. Explicit hint syntax remains [LOC-84](proof-automation.md#LOC-84).

<a id="LOC-78"></a>
## LOC-78 · rewrite!, unfold! and fold! spellings
<!-- task: {"id": "t171", "status": "done", "priority": 0, "created": "2026-09-21T19:37:49.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-155](core-build.md#LOC-155); `examples/proofs.lc`, `tests/elaborate.rs` and parser migration cases cover the macro-like spellings and checked equality transport.

<a id="LOC-80"></a>
## LOC-80 · Closed built-in forms and attribute parsing
<!-- task: {"id": "t174", "status": "done", "priority": 0, "created": "2026-09-21T19:41:53.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-155](core-build.md#LOC-155)/LOC-157; `tests/frontend.rs` checks allowed names/delimiters and unsupported user macros. Recognition of a name is not completion of its semantics: matches! remains [LOC-71](generics.md#LOC-71).

<a id="LOC-81"></a>
## LOC-81 · Snapshot helper split superseded by model observations
<!-- task: {"id": "t175", "status": "canceled", "priority": 0, "created": "2026-09-21T19:41:53.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. old! for mutable parameter entry state is implemented ([LOC-184](core-build.md#LOC-184)). The separate snapshot!/Ghost<T> design was removed by [LOC-209](reconciliation.md#LOC-209); use a typed model observation such as value as Int. There is no remaining task to restore snapshot!.

<a id="LOC-82"></a>
## LOC-82 · recurse! for checked logical descent
<!-- task: {"id": "t176", "status": "done", "priority": 0, "created": "2026-09-21T19:41:53.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-221](reconciliation.md#LOC-221); `src/elab/proofs.rs`, `tests/kernel_measured.rs` and `tests/logical_data.rs` cover `recurse!(evidence, call)` with nonnegative decreasing Int evidence and structural recursion. Runtime/mutual function recursion and a general decreases-attribute syntax remain [LOC-53](proof-automation.md#LOC-53).

<a id="LOC-83"></a>
## LOC-83 · vec! for the supported Vec tier
<!-- task: {"id": "t177", "status": "done", "priority": 0, "created": "2026-09-21T19:41:53.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-229](reconciliation.md#LOC-229); `src/elab/collections.rs`, `tests/collections.rs` and `tests/logical_containers.rs` cover construction, eager effects and Logical payload erasure.

<a id="LOC-85"></a>
## LOC-85 · Preserve runtime panic forms during erasure
<!-- task: {"id": "t179", "status": "done", "priority": 0, "created": "2026-09-21T19:41:53.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-190](core-build.md#LOC-190) and [LOC-214](reconciliation.md#LOC-214) settled the behavior: checked no_panic evidence establishes unreachable failure, while erasure preserves ordinary condition/effect evaluation. `src/elab/forms.rs`, `tests/exec_endings.rs` and panic corpus tests cover the generated behavior; source macro spelling is not a guaranteed printer ABI.

<a id="LOC-87"></a>
## LOC-87 · Typed and suffixed integer literals
<!-- task: {"id": "t180", "status": "done", "priority": 0, "created": "2026-09-21T19:55:01.438Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-154](core-build.md#LOC-154) and [LOC-171](core-build.md#LOC-171); `tests/frontend.rs::integer_literals_carry_their_value_and_suffix` and integer elaboration tests cover the supported 8-, 16-, 32-, and 64-bit machine types. The lexer can recognize wider suffix tokens, but `u128` and `i128` are not implemented surface types.

<a id="LOC-88"></a>
## LOC-88 · prop! and @P replace bracketed forms
<!-- task: {"id": "t182", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-155](core-build.md#LOC-155)/LOC-206; migration diagnostics and the complete examples/corpus use the chosen spellings, verified by `tests/frontend.rs` and `tests/reconcile_props.rs`.

<a id="LOC-89"></a>
## LOC-89 · Move-by-default values with checked derives
<!-- task: {"id": "t183", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-182](core-build.md#LOC-182); `tests/moves.rs` and the rustc rejection oracle cover use-after-move and structural Copy/Clone rules.

<a id="LOC-90"></a>
## LOC-90 · Private-by-default items and fields
<!-- task: {"id": "t184", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-183](core-build.md#LOC-183); `tests/build.rs` and `tests/acceptance.rs` verify generated visibility and protected fields.

<a id="LOC-91"></a>
## LOC-91 · Reserve Rust keywords
<!-- task: {"id": "t185", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-154](core-build.md#LOC-154) and [LOC-205](reconciliation.md#LOC-205); `tests/frontend.rs` verifies Rust keywords and the added prop/logic keywords. forall/exists are contextual within formulas, as settled in [LOC-147](core-build.md#LOC-147).

<a id="LOC-92"></a>
## LOC-92 · Never result type and coercion
<!-- task: {"id": "t186", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-181](core-build.md#LOC-181); `src/elab/exprs.rs` and early-return/panic corpus cases cover unreachable values. The supported ! spelling is a diverging function result, not every future Rust never-type position.

<a id="LOC-93"></a>
## LOC-93 · Early return with checked lowering
<!-- task: {"id": "t187", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-177](core-build.md#LOC-177) and [LOC-181](core-build.md#LOC-181); `tests/exec_endings.rs` and early-return corpus cases cover the typed/check/erased paths.

<a id="LOC-94"></a>
## LOC-94 · Integer literal inference and bases
<!-- task: {"id": "t188", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-154](core-build.md#LOC-154)/LOC-171; frontend and operator elaboration tests cover expected types, default i32, suffixes and hexadecimal/octal/binary syntax.

<a id="LOC-95"></a>
## LOC-95 · String literals for built-in messages
<!-- task: {"id": "t189", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-154](core-build.md#LOC-154)/LOC-190; frontend escape tests and panic diagnostics cover literal messages. General runtime strings remain [LOC-35](memory-layout.md#LOC-35).

<a id="LOC-96"></a>
## LOC-96 · Qualified paths and separate namespaces
<!-- task: {"id": "t190", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-157](core-build.md#LOC-157)/LOC-175; `tests/frontend.rs`, `tests/methods.rs` and associated-constant tests cover paths and type/value namespaces within a compilation unit. This does not supply import resolution ([LOC-41](interop.md#LOC-41)).

<a id="LOC-97"></a>
## LOC-97 · Ghost<T> type arguments superseded
<!-- task: {"id": "t191", "status": "canceled", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. Ghost<T> was removed by [LOC-209](reconciliation.md#LOC-209). Real generic type arguments, Seq and logical bounds are delivered by [LOC-219](reconciliation.md#LOC-219), [LOC-223](reconciliation.md#LOC-223); no special Ghost generic remains to implement.

<a id="LOC-98"></a>
## LOC-98 · Rust operator precedence
<!-- task: {"id": "t192", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-156](core-build.md#LOC-156); `tests/frontend.rs` and `tests/elaborate_operators.rs` cover parsing and evaluation.

<a id="LOC-99"></a>
## LOC-99 · Equality for booleans, integers and derived runtime data
<!-- task: {"id": "t193", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-175](core-build.md#LOC-175)/LOC-182; equality and derives are checked in `tests/elaborate.rs` and `tests/moves.rs`; comparing erased contents as observable runtime data is rejected.

<a id="LOC-100"></a>
## LOC-100 · Enum variants with named fields
<!-- task: {"id": "t194", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-175](core-build.md#LOC-175); parser, constructor and pattern tests cover named variants; [LOC-206](reconciliation.md#LOC-206) adds analogous named witnesses to propositions.

<a id="LOC-101"></a>
## LOC-101 · Rust const items
<!-- task: {"id": "t195", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-175](core-build.md#LOC-175); `tests/frontend.rs` and `tests/elaborate.rs` cover typed const declarations and compile-time-compatible emission. static storage remains [LOC-36](memory-layout.md#LOC-36).

<a id="LOC-105"></a>
## LOC-105 · Mutable value parameters
<!-- task: {"id": "t199", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-184](core-build.md#LOC-184); src/elab/references.rs applies the parameter flag to the passing mode. tests/corpus/accept/references.lc::count_up assigns a mut value parameter, executes it and asserts the generated Rust retains mut. This is distinct from an &mut reference parameter.

<a id="LOC-108"></a>
## LOC-108 · Integer MIN/MAX and the closed wrapping-method list
<!-- task: {"id": "t202", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-171](core-build.md#LOC-171)/LOC-172; `src/elab/literals.rs`, `src/elab/calls.rs` and `tests/elaborate_operators.rs` implement MIN, MAX, wrapping_add/sub/mul and signed wrapping_neg. Additional integer methods require their own models.

<a id="LOC-113"></a>
## LOC-113 · Decide what the integer operators mean to the logic
<!-- task: {"id": "t207", "status": "done", "priority": 3, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Current rule: runtime machine operations use the checked operation table; propositions use logical models and logic fn. Ordinary wrapping-method calls do not enter logic through the old three-promise rule ([LOC-210](reconciliation.md#LOC-210), [LOC-211](reconciliation.md#LOC-211)). The discussion below records the earlier design alternatives.

Historical decision record:

Decided 21 September: Target language: Integers, in code and in propositions. The test is Target language: Functions in propositions applied to operators: x + 1 on a machine integer may panic, so it is not valid in a proposition; x as Int + 1 and x.wrapping_add(1) are.

Target language. Overflow panics in one build and wraps in another, so without no_panic the only fact true in every build is that the result, if there is one, is the wrapped value. / and % truncate toward zero and panic on a zero divisor; a shift overflows at the bit width; as truncates.

Proposal (21 September), not yet accepted:

1. In code an operator means what it means in Rust, and is printed as written.
2. To the logic, the result of a + b is the sum of the two values as Int, wrapped into the type. Under no_panic there is also an obligation that the sum is in range, after which the wrap is the identity and the result is the exact sum. Without no_panic there is no obligation and only the wrapped value is known, since that is the one fact true in every build.
3. Inside a proposition, arithmetic is over Int: a machine integer is read as its Int value, so a specification cannot overflow. Code and specification agree exactly when nothing overflows, which is what no_panic establishes. To speak of wrapping in a proposition, write wrapping_add.
4. An operation that panics in every build teaches its condition to what follows: after a / b, the divisor is known not to be zero, as after assert!(c), c is known. An operation that panics only in some builds teaches nothing.
5. / and % on Int truncate toward zero, as in Rust, so that code and specification agree. Int division by zero is given the value 0 in the logic, where it must be total; code under no_panic can never reach it.
6. as between integer types is the same wrap and is never an obligation. as Int is exact.
7. The reference interpreter panics on overflow, which is the stricter of Rust's two behaviors.
8. Kernel work this implies: Int as an ordered ring; for each machine type a view to Int and a wrap from Int, generalizing the present u8 model; the evaluator; and an arithmetic procedure that is linear, treats multiplication by a literal as linear, and leaves other products to lemmas.

<a id="LOC-114"></a>
## LOC-114 · Decide which operators are in the first cut
<!-- task: {"id": "t208", "status": "done", "priority": 3, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered by [LOC-172](core-build.md#LOC-172); bit operations remain [LOC-116](generics.md#LOC-116). The reference to a Nat-only kernel below is historical, not the current implementation.

Historical decision record:

Target language. Multiplication makes arithmetic non-linear; the bit operators and shifts need a bit-level model; the kernel today has only Nat with addition.

Decided: + - * / % in the core, with multiplication. The bit operators and shifts later.

<a id="LOC-115"></a>
## LOC-115 · Proposition-literal spelling settled for this language tier
<!-- task: {"id": "t209", "status": "canceled", "priority": 1, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. Current source deliberately uses prop!(...) and @P/@(...) ([LOC-206](reconciliation.md#LOC-206), [LOC-211](reconciliation.md#LOC-211)). The earlier open-ended search for a lighter literal is no longer a blocking decision. Any later redesign should identify a concrete ergonomic problem and migration plan.

<a id="LOC-117"></a>
## LOC-117 · Decide whether a comparison in a proposition may mix integer types
<!-- task: {"id": "t228", "status": "done", "priority": 1, "created": "2026-09-21T20:49:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

The explicit same-logical-type comparison rule is implemented; [LOC-211](reconciliation.md#LOC-211) later added automatic machine-to-Int observations in logical contexts. Explicit model casts remain available.

Historical decision record:

Decided 21 September: no. A comparison is always within one type, Int included: out as Int == x as Int + 1. Target language: Integers, in code and in propositions.

Target language: Integers, in code and in propositions. out == x as Int + 1 with out a u32, or out as Int == x as Int + 1.

<a id="LOC-118"></a>
## LOC-118 · Native Int axioms and evaluation
<!-- task: {"id": "t230", "status": "done", "priority": 0, "created": "2026-09-21T20:49:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-159](core-build.md#LOC-159)/LOC-160; `tests/kernel_int.rs`, `tests/kernel_evaluation.rs` and the contract inventory cover ring, order and discreteness. The old kernel Nat was removed; inductive Nat now lives in the checked library ([LOC-223](reconciliation.md#LOC-223)).

<a id="LOC-119"></a>
## LOC-119 · Total truncating Int division and remainder
<!-- task: {"id": "t231", "status": "done", "priority": 0, "created": "2026-09-21T20:49:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-161](core-build.md#LOC-161); `tests/kernel_int_div.rs` covers signs and division by zero. Runtime machine division still follows Rust panic conditions.

<a id="LOC-120"></a>
## LOC-120 · Machine view/wrap models
<!-- task: {"id": "t232", "status": "done", "priority": 0, "created": "2026-09-21T20:49:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-162](core-build.md#LOC-162); `tests/kernel_machine.rs` covers ranges, round trips and casts at each fixed width.

<a id="LOC-121"></a>
## LOC-121 · One checked primitive-operation table
<!-- task: {"id": "t233", "status": "done", "priority": 0, "created": "2026-09-21T20:49:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-163](core-build.md#LOC-163); `src/kernel/ops.rs`, `tests/kernel_ops.rs` and `tests/operators.rs` exercise result types, models and panic conditions.

<a id="LOC-122"></a>
## LOC-122 · Closed Int and machine evaluation
<!-- task: {"id": "t234", "status": "done", "priority": 0, "created": "2026-09-21T20:49:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-160](core-build.md#LOC-160)/LOC-171; `tests/kernel_evaluation.rs` and `tests/operators.rs` cover evaluation. The historical byte-wide evaluate_all proof rule was removed, not generalized.

<a id="LOC-123"></a>
## LOC-123 · Certificate-producing linear arithmetic
<!-- task: {"id": "t235", "status": "done", "priority": 3, "created": "2026-09-21T20:49:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-164](core-build.md#LOC-164)/LOC-165/LOC-173; `src/arith`, `tests/arith.rs` and `tests/kernel_linear.rs`. Literal multiplication and division by a literal are supported; general nonlinear search remains out of scope.

<a id="LOC-124"></a>
## LOC-124 · Callable integer and machine lemmas
<!-- task: {"id": "t236", "status": "done", "priority": 0, "created": "2026-09-21T20:49:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-166](core-build.md#LOC-166); `tests/kernel_lemmas.rs` tests named lemma use, wrong premises and wrong conclusions. Library expansion is [LOC-52](proof-automation.md#LOC-52).

<a id="LOC-127"></a>
## LOC-127 · Rust loops over mutable state
<!-- task: {"id": "t242", "status": "done", "priority": 0, "created": "2026-09-21T21:03:06.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-179](core-build.md#LOC-179); `src/elab/loops.rs` and the corpus replace historical state-passing loop syntax. Iterator objects and terminating-loop proofs remain [LOC-30](memory-layout.md#LOC-30) and [LOC-49](memory-layout.md#LOC-49).

<a id="LOC-128"></a>
## LOC-128 · Call-duration reference parameters
<!-- task: {"id": "t243", "status": "done", "priority": 0, "created": "2026-09-21T21:03:06.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-184](core-build.md#LOC-184); `tests/references.rs` verifies &T, &mut T, disjoint roots/paths, write-back and old!. Stored shared references are [LOC-230](reconciliation.md#LOC-230); broader borrowing is [LOC-33](memory-layout.md#LOC-33).

<a id="LOC-129"></a>
## LOC-129 · Design the checked IR for mutation, early exit, and panics before coding it
<!-- task: {"id": "t244", "status": "done", "priority": 4, "created": "2026-09-21T21:08:15.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented by [LOC-176](core-build.md#LOC-176)–[LOC-180](core-build.md#LOC-180). The design-before-code gate below has been satisfied; its cases remain useful regression requirements.

Historical decision record:

Done 21 September as M0 of the Build plan, after one review. Target language: How mutation is checked.

This is commit M0 ([LOC-176](core-build.md#LOC-176)) of the Build plan.

A gate: mutation is not implemented until this is written (21 September, after review). The design must state, for each case below, the versions, the facts available at each point, and the types of results, and not only whether the program is accepted:
1. One branch refreshes tracked evidence; the other returns early.
2. A loop with several continue and break paths.
3. Snapshot evidence survives an assignment, while tracked evidence needs a refresh.
4. A proposition value captures an old value, and then becomes the type of tracked evidence.
5. A value is moved and initialized again, with evidence about its earlier state in scope.
6. A call changes two disjoint fields and returns evidence about both.

Direction decided 21 September (Target language: How mutation is checked): versions, in lowering, watched by comparing the two interpreters. The design itself is still to be written.

Raised in the last review before the core. Today lowering is small because the language is pure. With let mut, assignment, loops over mutable state, return, moves, and panics, either lowering does a large translation into versions and state passing, all of it trusted, or the check IR gains mutable variables and the checker grows instead. The rules the target language leaves "to be written with mut" belong to the same design and are one flow analysis: validity of tracked evidence, what is known after a join, what a loop forgets, initialization and moves, and early return. Write this as a design before implementing any part of it.

<a id="LOC-130"></a>
## LOC-130 · Decide how moves meet the logic
<!-- task: {"id": "t245", "status": "done", "priority": 3, "created": "2026-09-21T21:08:15.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded details: Ghost<T> was removed and logical-producing ordinary expressions retain eager effects ([LOC-209](reconciliation.md#LOC-209), [LOC-210](reconciliation.md#LOC-210), [LOC-214](reconciliation.md#LOC-214)). The accepted current model is type-based erasure with SSA snapshots, moves and explicit shared-reference provenance, not the interim three-promise wording below.

Historical decision record:

Decided 21 September. Target language: Values and types. Matching through & is noted under Mutation and references.

Evidence, Ghost<T>, Int, and every type with no runtime form should be Copy, whatever they speak of. A proposition should be able to mention a value that has been moved, since a result type speaks of parameters the body consumed. An expression that is erased should have to keep the three promises and should never move anything, which would replace the present rule that keeps the effects of an erased expression. Matching through a & parameter binds references in Rust, which tier 0 has no place for: only Copy fields could be bound.

<a id="LOC-131"></a>
## LOC-131 · Write the acceptance examples of the new core in the target syntax, and count what their proofs need
<!-- task: {"id": "t246", "status": "done", "priority": 3, "created": "2026-09-21T21:08:15.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

The target examples are executable acceptance cases ([LOC-189](core-build.md#LOC-189), [LOC-216](reconciliation.md#LOC-216)). Their original findings led to explicit logic fn ([LOC-210](reconciliation.md#LOC-210)) and the still-open loop termination task [LOC-49](memory-layout.md#LOC-49).

Historical decision record:

Done 21 September: the Target examples document, with six findings. Two became new decisions, [LOC-143](core-build.md#LOC-143) and [LOC-144](core-build.md#LOC-144).

Before building: the lock example with u32, and two or three more, written by hand with prop!, @(...), as Int, promises on every function, tracked evidence, and explicit refreshes. This tests the spelling cheaply, and shows which holes must fill for the core to be usable. With only the basic hole, every overflow obligation over 32 bits is a chain of lemma calls; the expected conclusion is that the linear arithmetic procedure belongs inside the core milestone. Subsumes [LOC-72](core-build.md#LOC-72).

<a id="LOC-132"></a>
## LOC-132 · Decide whether evidence of one claim is still accepted for another
<!-- task: {"id": "t247", "status": "done", "priority": 2, "created": "2026-09-21T21:08:15.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented by [LOC-167](core-build.md#LOC-167). Explicit evidence must match after definitional computation; search is requested through a hole or prove!, not implicitly by giving a proof of another proposition.

Historical decision record:

Decided 21 September: exact evidence, after computing; no bridging. Target language: Propositions and evidence.

Open question 3. The elaborator does it today, and it is what makes destructuring a result need no proof repair. It is implicit, which sits badly with preferring explicit steps, and prove!(P) now offers an explicit way to restate a fact. Decide before the elaborator is reworked.

<a id="LOC-133"></a>
## LOC-133 · Decide how Int enters the kernel, and how the machine models are tested
<!-- task: {"id": "t248", "status": "done", "priority": 2, "created": "2026-09-21T21:08:15.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented by [LOC-159](core-build.md#LOC-159)–[LOC-166](core-build.md#LOC-166); native Int and the tested machine operation table are current. Nat was subsequently reintroduced as checked library data in [LOC-223](reconciliation.md#LOC-223).

Historical decision record:

Decided 21 September: by axioms; tested by random instances and against Rust. Target language: Integers.

Axioms for an ordered ring, or a construction from Nat. The u8 model was tested against native arithmetic on every pair of bytes; that is impossible at 32 bits, so the models and the evaluator need boundary and randomized testing against Rust. A wrong axiom here makes everything provable.

<a id="LOC-134"></a>
## LOC-134 · Decide what a panic is to the interpreter, the differential tests, and erasure
<!-- task: {"id": "t249", "status": "done", "priority": 2, "created": "2026-09-21T21:08:15.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented by [LOC-170](core-build.md#LOC-170), [LOC-172](core-build.md#LOC-172) and the three-way harness. Panic outcomes and ordinary effects survive erasure; no-return behavior is separate from logical totality.

Historical decision record:

Decided 21 September: a panic is a third outcome. Target language: Effects are promises.

Today a program returns or does not. A panic is a third outcome: the reference interpreter, the comparison with compiled Rust, and the statement that erasure preserves behavior all have to speak of it. Later, with &mut and protected types: a panic in the middle of an update leaves the caller's value half changed, and a Rust caller that catches the panic can see it.

<a id="LOC-135"></a>
## LOC-135 · Decide what locus build emits
<!-- task: {"id": "t250", "status": "done", "priority": 2, "created": "2026-09-21T21:08:15.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented by [LOC-183](core-build.md#LOC-183) with tests/build.rs. Broader dependency/Cargo integration remains [LOC-44](interop.md#LOC-44), [LOC-63](interop.md#LOC-63).

Historical decision record:

Decided 22 September by O2: a plain Cargo layout. locus build writes <dir>/Cargo.toml (edition 2024, no dependencies), src/lib.rs with the header lints, the marker types Proved and Ghost (a struct with a private field and a private constant, so descendants can name the value and a Rust caller cannot construct it), and one pub mod per file; each Locus file becomes src/<stem>.rs. Output is byte-identical across runs.

Partly decided 21 September (Target language: What is generated): Proved and Ghost cannot be constructed outside generated code, and are defined once for all generated modules. Still open: the layout on disk, and how a crate includes it.

One module per file, to be declared with mod? Where Proved and Ghost live when there are several files, since each file defining its own makes them different Rust types; and whether Proved can be made unconstructible outside generated code from the start, which shapes every signature that is generated.

<a id="LOC-136"></a>
## LOC-136 · Decide the order of implementation, and what leaves the core
<!-- task: {"id": "t251", "status": "done", "priority": 2, "created": "2026-09-21T21:08:15.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

The original core-build sequencing is historical and completed. Reconciliation followed in three accepted tiers; current remaining dependencies are listed in the surviving project introductions and [LOC-75](core-build.md#LOC-75).

Historical decision record:

Decided 21 September: the order of the Build plan is accepted.

Proposed 21 September in the Build plan document and the Core build project; waiting for confirmation.

A second review (21 September) proposed nearly the same order: 1. the syntax changes, explicit visibility, and exact evidence; 2. promises, Int, machine arithmetic, and the certificates the examples need; 3. mutable locals and tracked evidence, after [LOC-129](core-build.md#LOC-129); 4. moves, references as parameters, and protected exports. The lock file, a lighter spelling for promises, richer patterns, and collections can wait.

The present pipeline works end to end with about 200 tests. A suggested order that keeps it working: the changes of syntax (prop!, @P, keywords, precedence, literals, paths, pub); promises in place of math fn; Int and the machine integers in the kernel, with arithmetic; let mut, tracked evidence, and the loop forms; moves and derive; return, the never type, and the panicking forms. The lock file of found proofs depends on a stable form for proofs and could leave the core.

<a id="LOC-137"></a>
## LOC-137 · Restrict proof-taking Rust exports
<!-- task: {"id": "t252", "status": "done", "priority": 0, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-183](core-build.md#LOC-183)/LOC-185; `tests/build.rs` and adversarial safe-Rust callers verify that reusable Erased markers cannot forge a public invariant. The generated crate boundary, not marker identity, supplies protection.

<a id="LOC-138"></a>
## LOC-138 · Panic as a distinct checked/interpreted outcome
<!-- task: {"id": "t253", "status": "done", "priority": 0, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-170](core-build.md#LOC-170); `tests/exec_endings.rs`, `tests/operators.rs` and three-way corpus execution include panic paths.

<a id="LOC-139"></a>
## LOC-139 · Dependent destructuring opens fresh field names
<!-- task: {"id": "t254", "status": "done", "priority": 0, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-167](core-build.md#LOC-167); `tests/reconcile_props.rs`, `tests/reconcile_scope.rs` and product tests verify proof fields depend on the actual unpacked values.

<a id="LOC-140"></a>
## LOC-140 · Explicit proof values must match their stated claim
<!-- task: {"id": "t255", "status": "done", "priority": 0, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-167](core-build.md#LOC-167); normalization may establish definitional equality, but arbitrary evidence does not become a different claim by heuristic search. `tests/corpus/reject/evidence_of_another_claim.lc` and acceptance tests pin the distinction from hole filling.

<a id="LOC-141"></a>
## LOC-141 · Three-promise erasure rule superseded
<!-- task: {"id": "t256", "status": "canceled", "priority": 0, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. [LOC-209](reconciliation.md#LOC-209), [LOC-210](reconciliation.md#LOC-210), [LOC-214](reconciliation.md#LOC-214) define type-based logical classification and effect-preserving erasure. Pure logical observations are erased, but eager ordinary effects in logical-producing expressions still execute exactly once; replacing every erased-typed expression blindly would be incorrect. Historical snapshots and moved values follow the explicit scope/SSA rules of [LOC-213](reconciliation.md#LOC-213).

<a id="LOC-142"></a>
## LOC-142 · SSA lowering for assignments and endings
<!-- task: {"id": "t257", "status": "done", "priority": 0, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-176](core-build.md#LOC-176)–[LOC-180](core-build.md#LOC-180); `src/typed/lower.rs`, `tests/typed_lower.rs`, `tests/exec_endings.rs` and three-way execution cover versions, joins, return and panic.

<a id="LOC-143"></a>
## LOC-143 · Lighter logical promises resolved by logic fn
<!-- task: {"id": "t258", "status": "canceled", "priority": 2, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. [LOC-210](reconciliation.md#LOC-210) supplies the dedicated logical function/block form with implicit purity/totality requirements, so the old plan for a shorthand combining three runtime promises is superseded. Ordinary effect promises remain explicit and separate.

<a id="LOC-144"></a>
## LOC-144 · Terminating-loop decision consolidated
<!-- task: {"id": "t259", "status": "canceled", "priority": 2, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Superseded. The missing loop termination rule remains open as [LOC-49](memory-layout.md#LOC-49). Logical recursion now supplies total repeated computation ([LOC-221](reconciliation.md#LOC-221)), but it does not discharge the bounded-for/while question. This duplicate decision is closed, not the capability.

<a id="LOC-145"></a>
## LOC-145 · Machine-model and evaluator differential tests
<!-- task: {"id": "t260", "status": "done", "priority": 0, "created": "2026-09-21T21:22:55.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-162](core-build.md#LOC-162) and [LOC-186](core-build.md#LOC-186); `tests/kernel_machine.rs`, `tests/operators.rs`, `tests/differential.rs` and `tests/random_programs.rs` compare boundary and generated cases against Rust. This supplies testing evidence, not a consistency proof for axioms.

<a id="LOC-146"></a>
## LOC-146 · Decide whether forall and exists stay keywords, or become names that take a closure
<!-- task: {"id": "t261", "status": "done", "priority": 1, "created": "2026-09-21T21:35:37.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Resolved by [LOC-226](reconciliation.md#LOC-226): Exists/ForAll are checked library propositions, with source quantifier syntax as sugar over logical callables. Low-level kernel compatibility forms are not a second source implementation.

Historical decision record:

Reversed later on 21 September: they stay keywords. See the target language, Propositions and evidence.

Decided 21 September: names that take a closure. [LOC-147](core-build.md#LOC-147).

Target language: The rule about Rust. forall(|x: u8| x <= 255) is plain Rust syntax, a call that takes a closure, where forall (x: u8) { x <= 255 } is not. It would leave prop as the only word Locus adds, and it fits the logic-only lambdas expected later. The closure is never run.

<a id="LOC-147"></a>
## LOC-147 · Decide whether forall and exists are reserved everywhere or only inside a formula
<!-- task: {"id": "t262", "status": "done", "priority": 1, "created": "2026-09-21T21:40:52.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented: forall/exists are contextual in formulas; Rust keywords plus prop/logic are reserved. See tests/frontend.rs and [LOC-205](reconciliation.md#LOC-205), [LOC-226](reconciliation.md#LOC-226).

Historical decision record:

Decided 21 September: only inside a formula.

Target language: Propositions and evidence. The closure spelling was withdrawn on 21 September; the quantifiers stay keywords, written as they are today. A formula is always inside prop!(...), prove!(...), or @(...), where => already has a meaning it has nowhere else, so the two words could be keywords there alone and ordinary names elsewhere.

<a id="LOC-148"></a>
## LOC-148 · Decide whether a declared proposition is written as an attribute on an enum, so that prop is not a keyword
<!-- task: {"id": "t263", "status": "canceled", "priority": 1, "created": "2026-09-21T21:40:52.000Z", "updated": "2026-09-21T21:45:09.000Z"} -->

Decided 21 September: no. A declared proposition keeps a keyword of its own; keywords are acceptable. The proposal below is kept for the record.

Proposal, 21 September. A proposition is an enum whose values are its proofs, which is how it was first explained, so write it as one:

    #[prop(n: u32)]
    enum Small {
        #[proves(Small(0))]
        Zero,
        Below { bound: @(n < 10) },
    }

The parameters go in the attribute; a variant that proves the proposition at particular arguments says so with #[proves(...)], and one without it proves it at the parameters; fields with names use Rust's struct-like variants. Everything but the @ types is Rust syntax. Use is unchanged: @Small(n), Small::Zero, match on evidence. It is erased whole. Observation: until recursion arrives, a declared proposition adds names and not power, since without recursion it can be written with ||, &&, and exists; what it is really for is inductive predicates.

<a id="LOC-149"></a>
## LOC-149 · Formula-only quantifiers and implication
<!-- task: {"id": "t264", "status": "done", "priority": 0, "created": "2026-09-21T21:51:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered. [LOC-155](core-build.md#LOC-155)/LOC-226; `tests/frontend.rs::quantifiers_and_implication_are_read_only_inside_a_formula` and quantifier-library tests cover parsing and checked sugar.

<a id="LOC-150"></a>
## LOC-150 · Decide how a function is marked visible to other Locus modules but not exported to Rust
<!-- task: {"id": "t265", "status": "done", "priority": 2, "created": "2026-09-21T21:51:51.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

The restricted-visibility/export rule is implemented by [LOC-183](core-build.md#LOC-183). Its protection assumes the generated crate/module boundary; arbitrary handwritten code inserted inside that protected boundary is not an additional enforcement domain.

Historical decision record:

Decided 21 September: Rust's restricted visibility, with one checked rule. Target language: What is generated.

Target language: What is generated. Rust's pub(crate) would let hand-written Rust in the same crate call it, so it does not serve. Either the signature decides (a pub function that takes evidence is emitted visible to generated modules only), or there is an explicit spelling.

Proposal (21 September): no new syntax. Rust already has restricted visibility: pub(super), pub(in path), pub(crate). One checked rule does the work: a function that takes evidence may be visible no further than the root of the generated modules. Plain pub on such a function is an error that says so and offers the two ways out, a narrower visibility or a validated type. In a crate that mixes Locus and hand-written Rust the programmer writes pub(in crate::verified), or pub(super) when the modules are siblings under a generated root; in a crate that is all Locus, pub(crate) is already right. Every spelling means what it means in Rust. Not covered: one Locus crate calling evidence-taking functions of another. Rust has no visibility meaning "generated code in other crates", so that would rest on a hidden name and a convention, as serde's __private does, and on locus audit. Alternatives considered: inferring it from the signature (implicit, and a small change to a signature would silently change what is exported); an #[export] attribute with pub meaning less than it does in Rust; and pub(locus), which is not Rust syntax.

<a id="LOC-151"></a>
## LOC-151 · H1 · A corpus runner: every .lc file is checked, run in both interpreters, compiled, and compared
<!-- task: {"id": "t267", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T22:53:26.000Z"} -->

Landed 21 September as tests/corpus.rs. Directives: //~ proofs: N, //~ run: f(args) => value (or => panic, refused until E4), //~ rust: text, //~ error: L0204 [message] with ^ for the line above. The corpus is examples/, tests/corpus/accept, and tests/corpus/reject; one rustc call; all failures listed. Found on the way: a name that is a Rust keyword and not a Locus one, such as move, is accepted and printed unescaped, and rustc rejects the output; S1 closes this by reserving every Rust keyword.

Lane: Harness. Wave 1.
Depends on: nothing.
Unblocks: E4 ([LOC-170](core-build.md#LOC-170)), R1 ([LOC-186](core-build.md#LOC-186)), R2 ([LOC-187](core-build.md#LOC-187)), R3 ([LOC-188](core-build.md#LOC-188)).
Covers: [LOC-7](core-build.md#LOC-7).

Scope. tests/corpus/accept and tests/corpus/reject hold .lc files whose expectations are written in comments: the number of proofs, run lines such as f(1, 2) => 3 or f(255) => panic, substrings the generated Rust must contain, and for rejected files the code of the error on the line where it is expected. One runner does for each accepted file what cli.rs and rust_output.rs do by hand today: check, run every run line in the check-IR interpreter and the erased interpreter, print Rust, compile all files in one rustc call with -D warnings, run, compare. The five examples move under it.

Tests.
- The five examples pass through the runner.
- The runner is tested on itself: a wrong expected value, a missing error, and an error on the wrong line each make it report a failure.
- All failures in a run are reported, not the first.

Done when. From here on a feature is tested by adding files. The whole suite stays under a minute because rustc runs once.

<a id="LOC-152"></a>
## LOC-152 · H2 · A seeded generator with no dependencies, parser fuzz tests, and tools/check.sh in a fast and an extended form
<!-- task: {"id": "t268", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T22:56:33.000Z"} -->

Landed 21 September. tests/common/rng.rs (xorshift64*, included with #[path]), tests/parser_fuzz.rs, Parsed::stats with a step counter, tools/check.sh and --extended. Fast: 50,000 sequences and 5,000 edited files in about 6 s; extended: 5,000,000 and 500,000 in about 45 s in release. Steps are at most 4 times the tokens; the most seen is 2.99. The fuzzing found no panic; it did find that the for header lookahead rescanned to the end of the file, quadratic on hostile input, now a delimiter table built once. The nesting test passes on a 640 KB stack and fails at 512 KB, so the margin on 1 MB is under two; the syntax lane must keep frames between depth guards small.

Lane: Harness. Wave 1.
Depends on: nothing.
Unblocks: S1 ([LOC-154](core-build.md#LOC-154)), R1 ([LOC-186](core-build.md#LOC-186)).

Scope. A small xorshift generator in tests/common, since the build is offline and has one dependency. The parser is run on random sequences drawn from the token vocabulary and on the example files with random edits (delete, duplicate, swap a token). tools/check.sh runs fmt, clippy, and the tests with fixed seeds and small counts, and is what every commit must pass. tools/check.sh --extended sets LOCUS_EXTENDED, under which the fuzz counts are a hundred times larger and the random program comparison compiles thousands of programs; it runs before a milestone is declared, not on every commit.

Tests.
- Fast: 50,000 token sequences and 5,000 edited files with a fixed seed: the parser returns an AST or at least one diagnostic, without a panic, on a 1 MB stack.
- A failure prints its seed.
- Separately from the fuzzing, the two guarantees that make the parser total are stated in Architecture and each has its own test: every loop consumes a token or stops (a test counts parser steps against the number of tokens), and recursion is bounded by MAX_DEPTH (the existing nesting test, extended to each new form).

Done when. The fuzz tests show robustness on what they tried and claim no more. Totality rests on the progress and depth guarantees, which are stated and tested on their own.

<a id="LOC-153"></a>
## LOC-153 · H3 · Split src/elab/exprs.rs by construct, with no change in behaviour
<!-- task: {"id": "t269", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T22:49:01.000Z"} -->

Landed 21 September. exprs.rs is now exprs, operators, data, calls, control, loops, blocks, and patterns, the largest 284 lines; 198 tests unchanged.

Lane: Harness. Wave 1.
Depends on: nothing.
Unblocks: E1 ([LOC-167](core-build.md#LOC-167)).

Scope. exprs.rs is 1,400 lines and every later lane edits it. Split it into calls, control flow, patterns, operators, and blocks so that lanes touch different files.

Tests.
- Every existing test passes unchanged.
- No file under src/elab is over about 600 lines.

Done when. Two people or agents can work in the elaborator at once without colliding.

<a id="LOC-154"></a>
## LOC-154 · S1 · The lexer tokenizes as Rust: every keyword reserved, string literals, integer literals in full
<!-- task: {"id": "t270", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T23:27:10.000Z"} -->

Landed 21 September. Every Rust keyword reserved (L0115, with a fix), a not-in-Locus-yet family (L0005 from the lexer, L0116 from the parser) with one message per construct, integer literals of any size as kernel::Natural with a suffix, string literals with Rust escapes (L0006, L0007), attributes skipped with L0105. Note for S3 to S5: the stack margin was recovered by moving keyword checks into #[inline(never)] helpers and narrowing Token; keep doing that.

Lane: Syntax. Wave 2.
Depends on: H2 ([LOC-152](core-build.md#LOC-152)).
Unblocks: S2 ([LOC-155](core-build.md#LOC-155)).
Covers: [LOC-91](core-build.md#LOC-91), [LOC-94](core-build.md#LOC-94), [LOC-95](core-build.md#LOC-95).

Scope. Reserve every strict and reserved Rust keyword. String literals with Rust escapes. Integer literals in decimal, hex, octal, and binary, with underscores and suffixes. Rust tokens Locus does not use yet (lifetimes, compound assignment, shifts, char and float literals) are recognized and reported as not in Locus yet, so that no Rust source produces a confusing token error.

Tests.
- A table test over every Rust keyword: using it as a name is an error with a code.
- A table of literals with value and suffix, including the largest of each type and one past it.
- Each not-yet token has a test of its message.
- The case H1 found, fn move(ref: u8), is a rejected corpus file.

Done when. A Rust programmer who pastes Rust gets either a parse or a plain statement that the construct is not in Locus yet.

<a id="LOC-155"></a>
## LOC-155 · S2 · Built-in forms and the formula grammar: prop!, prove!, @P, forall and exists, rewrite!, unfold!, fold!
<!-- task: {"id": "t271", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T00:01:37.000Z"} -->

Landed 21 September. ExprKind::Form with a closed Form enum of fifteen; formulas parsed under a flag inside prop!(...), prove!(...), @(...); forall, exists, and def are contextual names, no longer tokens; brackets now report arrays as not in Locus yet. Migration diagnostics with fixes: L0117 for [F] and @[F], L0231 for bare rewrite/unfold/fold; L0118 unknown form; L0119 => or forall outside a formula. prove! reuses the hole solver and counts as one proof; as a statement it adds the fact to scope and produces nothing in the typed tree. lock.lc checks with the same eight proofs at the same sizes.

Lane: Syntax. Wave 3.
Depends on: S1 ([LOC-154](core-build.md#LOC-154)).
Unblocks: S3 ([LOC-156](core-build.md#LOC-156)), E1 ([LOC-167](core-build.md#LOC-167)), E10 ([LOC-190](core-build.md#LOC-190)).
Covers: [LOC-80](core-build.md#LOC-80), [LOC-88](core-build.md#LOC-88), [LOC-149](core-build.md#LOC-149), [LOC-77](core-build.md#LOC-77), [LOC-78](core-build.md#LOC-78).

Scope. Parse name!(...) for the closed list of forms. Formulas, with forall (x: T) { ... }, exists, and =>, are parsed only inside prop!(...), prove!(...), and @(...). @P takes a name, an application, or a parenthesized formula. [P], @[P], and the bare rewrite, unfold, and fold go, each with a migration diagnostic that carries a fix. prove!(P) elaborates as a claim filled where it stands. Examples and tests are respelled.

Tests.
- Every elaborator test passes after respelling, with the same proofs found.
- Each old spelling gives its migration diagnostic; applying the fix gives a file that parses, tested by applying it.
- forall and exists are ordinary names outside a formula; => outside a formula or a match arm is rejected; an unknown form names the closed list.

Done when. Brackets are free for arrays, and the source has no token Rust lacks.

<a id="LOC-156"></a>
## LOC-156 · S3 · Rust's operator precedence exactly, with as, unary minus, and + - * / % parsed
<!-- task: {"id": "t272", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T01:04:23.000Z"} -->

Landed 21 September. Binding powers follow the Rust Reference; comparisons are non-associative (L0103 with the chained-operator wording); ExprKind::Unary (Neg), ExprKind::Cast, ten new BinaryOps; arithmetic, unary minus, as, and bit operators parse and the elaborator reports each as not in Locus yet naming its commit (L0290). 107 expressions checked against a fully parenthesized rendering. The parser frame was restructured; the nesting test now passes at 576 KB. Note for the printer (E6): a as u8 < b groups as ((a as u8) < b) in Locus but rustc reads < after a cast type as generics, so generated Rust must parenthesize a cast that is the left operand of < or <<.

Lane: Syntax. Wave 4.
Depends on: S2 ([LOC-155](core-build.md#LOC-155)).
Unblocks: S4 ([LOC-157](core-build.md#LOC-157)), E5 ([LOC-171](core-build.md#LOC-171)).
Covers: [LOC-98](core-build.md#LOC-98), [LOC-99](core-build.md#LOC-99), [LOC-9](core-build.md#LOC-9).

Scope. Replace the binding-power table with Rust's. Comparisons do not chain. The arithmetic operators and as parse into the AST; until E6 the elaborator answers that they are not checked yet.

Tests.
- A table of about eighty expressions, taken from the Rust Reference, each compared with its fully parenthesized rendering.
- a == b == c and a < b < c are rejected as in Rust.
- The deep-nesting stack test covers the new operators.

Done when. An expression valid in both languages groups the same way in both.

<a id="LOC-157"></a>
## LOC-157 · S4 · Items: the closed attribute set, pub and its restricted forms, impl blocks, variants with named fields, long paths, const
<!-- task: {"id": "t273", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T02:05:21.000Z"} -->

Landed 21 September. Attributes as a closed set (L0120 unknown, L0121 shape or place), file-level #![...], doc comments as tokens kept on declarations, pub and its restricted forms (L0122 misplaced), inherent impl blocks with self parameters, variants with named fields, paths of any length, type arguments in the AST. The elaborator reports each unchecked thing once (L0290) naming its commit. tests/corpus/target holds the three target examples marked parse-only: midpoint and percent parse clean; lock has four parser diagnostics, all S5's (let mut, assignment). Nesting test passes at 560 KB; Type and Pattern boxes their paths to keep it so. The parser is finished for items.

Lane: Syntax. Wave 5.
Depends on: S3 ([LOC-156](core-build.md#LOC-156)).
Unblocks: S5 ([LOC-158](core-build.md#LOC-158)), E2 ([LOC-168](core-build.md#LOC-168)), E9 ([LOC-175](core-build.md#LOC-175)), O1 ([LOC-182](core-build.md#LOC-182)), O2 ([LOC-183](core-build.md#LOC-183)).
Covers: [LOC-13](core-build.md#LOC-13), [LOC-90](core-build.md#LOC-90), [LOC-96](core-build.md#LOC-96), [LOC-100](core-build.md#LOC-100), [LOC-101](core-build.md#LOC-101), [LOC-18](core-build.md#LOC-18).

Scope. Parse #[terminates], #[terminates(decreases = e)], #[no_panic], #[no_alloc], #[no_io], #[derive(...)], and the file-level #![...]; doc comments are kept for the printer. pub, pub(crate), pub(super), and pub(in path) on items and fields. Inherent impl blocks, enum variants with named fields, paths of any length. What the elaborator does not handle yet is answered by one uniform diagnostic, parsed but not yet checked, that names the task.

Tests.
- AST snapshot tests for each item form.
- An unknown attribute is rejected and the message lists the closed set.
- The item skeletons of the three target examples parse.
- The fuzz vocabulary gains the new tokens.

Done when. The parser is finished for items, so later lanes never edit parser.rs.

<a id="LOC-158"></a>
## LOC-158 · S5 · Statements and expressions: let mut, assignment, return, Rust's loop forms, references, the never type
<!-- task: {"id": "t274", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T04:05:28.000Z"} -->

Landed 21 September. let mut, assignment as a statement (L0123 bad place, L0124 in value position), mut on bindings and parameters (L0125 misplaced), return, loop, while, while let, for over a range or any expression, break with a value, plain continue beside the old continue(...), & and &mut in types and expressions, ! as a type, ..= in for headers. Rust's statement rule: a block-like expression in statement position ends the statement, which fixed a silent misparse of for { } (a, b) as a call. All three target files parse clean. Nesting test passes at 544 KB. The syntax lane is done: later lanes never edit parser.rs.

Lane: Syntax. Wave 6.
Depends on: S4 ([LOC-157](core-build.md#LOC-157)).
Unblocks: M2 ([LOC-178](core-build.md#LOC-178)).
Covers: [LOC-1](core-build.md#LOC-1), [LOC-93](core-build.md#LOC-93), [LOC-92](core-build.md#LOC-92), [LOC-127](core-build.md#LOC-127).

Scope. Parse let mut, assignment to a variable or field path, return, loop, while, while let, for pattern in range, break with a value, continue, & and &mut in types and arguments, and ! as a type. Rust's rule against struct literals in condition position is kept. The state-passing loop still parses until M3 removes it.

Tests.
- The three target examples are added verbatim under tests/corpus/target, marked parse-only. Later commits raise each file's marking to check and then to run; the marking is the visible progress of the whole plan.
- Fuzz and stack-depth tests cover the new forms.

Done when. Every program in Target examples parses. The syntax lane is done.

<a id="LOC-159"></a>
## LOC-159 · K1 · Integers of any size in the kernel: subtraction, multiplication, division, comparison, sign
<!-- task: {"id": "t275", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T22:56:33.000Z"} -->

Landed 21 September. kernel::Natural gains checked_sub, mul, div_rem, Ord, parsing; kernel::Integer is a sign and a magnitude with a canonical zero, truncating div and rem, a / 0 == 0 and a % 0 == a. Schoolbook algorithms, base 2^32 limbs. tests/kernel_integers.rs: boundary sets crossed for every operation, 100,000 random pairs per operation against i128 and u128, identities to about 768 bits.

Lane: Kernel. Wave 1.
Depends on: nothing.
Unblocks: K2 ([LOC-160](core-build.md#LOC-160)).
Covers: [LOC-118](core-build.md#LOC-118).

Scope. kernel::nat already holds naturals of any size with addition. Add subtraction, multiplication, division with remainder, and comparison, and an Int value as a sign and a magnitude. Values only; no terms or axioms yet.

Tests.
- Against i128 on a boundary set and 100,000 random pairs, for every operation.
- Beyond i128, identities: (a * b) / b == a, (a + b) - b == a, a == (a / b) * b + a % b.

Done when. The arithmetic the evaluator will trust is tested on its own, before any rule depends on it.

<a id="LOC-160"></a>
## LOC-160 · K2 · Int as a kernel type: literals, the ring and order axioms, discreteness, closed evaluation
<!-- task: {"id": "t276", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T23:19:59.000Z"} -->

Landed 21 September. Type::Int (ghost, as Nat), literals of any size, int_add, int_sub, int_mul, int_neg, and one primitive order int_le; a < b is the term a + 1 <= b. Sixteen axioms, each named in the Kernel contract; int_induction over the non-negative integers; evaluate computes closed Int terms and decides a closed <= or == at Int either way. Added beyond the brief: multiplication is charged steps in proportion to the size of its operands, so repeated squaring cannot build a number of 2^100 bits. A test reads the contract out of the atlas and fails if an axiom, rule, or primitive is not named there. Proof sizes from the axioms alone: 2 to 23 nodes for the small facts tried, and one transport for every reassociation of a sum, which is the case for K6.

Lane: Kernel. Wave 2.
Depends on: K1 ([LOC-159](core-build.md#LOC-159)).
Unblocks: K3 ([LOC-161](core-build.md#LOC-161)), K6 ([LOC-164](core-build.md#LOC-164)).
Covers: [LOC-118](core-build.md#LOC-118), [LOC-122](core-build.md#LOC-122).

Scope. Int joins the kernel's types. Axioms: the commutative ring laws, the order and its compatibility with + and *, discreteness (nothing lies between n and n + 1), and induction. Evaluate computes closed Int terms. The Kernel contract is updated in the same commit, as for every kernel commit.

Tests.
- For each axiom one test that uses it and one near miss that must be rejected (the axiom at the wrong type, with an argument swapped, or with < for <=).
- A few derived facts proved in tests from the axioms alone, such as x < y gives x + 1 <= y, to show the set is usable.
- Evaluate agrees with K1 on random closed terms.
- A test reads the contract out of the atlas and fails if a kernel axiom is not named in it.

Done when. Int exists in the logic, and the contract lists exactly what is assumed about it.

<a id="LOC-161"></a>
## LOC-161 · K3 · Int division and remainder: truncating, total, with x / 0 == 0
<!-- task: {"id": "t277", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T23:37:20.000Z"} -->

Landed 21 September. int_div and int_rem, eight axioms named in the Kernel contract (int_div_rem, int_div_zero, four remainder bounds, two sign axioms), each true at every b including zero and each bound false at b == 0 without its condition, which the tests refute. a % 0 == a derived in 33 nodes. Division is charged steps by operand size. Noted for later: Proof::Literal computes a primitive without a step budget, so literal(int_mul(huge, huge)) does unbounded work; a small K-lane follow-up.

Lane: Kernel. Wave 3.
Depends on: K2 ([LOC-160](core-build.md#LOC-160)).
Unblocks: K4 ([LOC-162](core-build.md#LOC-162)).
Covers: [LOC-119](core-build.md#LOC-119).

Scope. The axioms, as formulas over all a and b in Int. (1) a == (a / b) * b + a % b, with no condition. (2) a / 0 == 0. From these a % 0 == a follows and is not a separate axiom. (3) 0 < b gives -b < a % b and a % b < b; b < 0 gives b < a % b and a % b < -b. (4) 0 <= a gives 0 <= a % b; a <= 0 gives a % b <= 0. For b not zero, (1), (3), and (4) determine the quotient and remainder uniquely, and they are Rust's.

Tests.
- The four sign combinations and zero, as named tests, including a % 0 == a derived from (1) and (2) inside the kernel.
- Evaluation against Rust's / and % on i128 at random for a divisor that is not zero, and against the formulas for a zero divisor.
- A flooring instance, -7 / 2 == -4, is refuted by evaluation.
- Near misses: the bound of (3) stated without its condition on b is rejected, and would be false at b == 0.

Done when. Int division means what Rust's means wherever Rust's is defined.

<a id="LOC-162"></a>
## LOC-162 · K4 · A model for each machine integer type, from one table
<!-- task: {"id": "t278", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T00:04:53.000Z"} -->

Landed 21 September. MachineInt table (u8 to u64, i8 to i64) in src/kernel/machine.rs; Type::Machine for the seven non-u8 types beside Type::U8; Term::Machine literals checked in range; view, wrap, cast primitives; six axiom schemas named in the contract. The u8 model over Nat is kept beside the Int model until E5 moves the elaborator, then removed (added to E5). Exhaustive at 8 bits against Rust, boundaries and random above, 19 s extended.

Lane: Kernel. Wave 4.
Depends on: K3 ([LOC-161](core-build.md#LOC-161)).
Unblocks: K5 ([LOC-163](core-build.md#LOC-163)), K7 ([LOC-165](core-build.md#LOC-165)), K8 ([LOC-166](core-build.md#LOC-166)), E5 ([LOC-171](core-build.md#LOC-171)).
Covers: [LOC-120](core-build.md#LOC-120), [LOC-145](core-build.md#LOC-145).

Scope. For u8 to u64 and i8 to i64: a view to Int, a wrap from Int, the range of the view, and the round trips. as between machine types is the wrap of the view. The u8 model over Nat is restated over Int and its separate axioms are removed, so the contract holds one schema and not a list per type.

Tests.
- The 8-bit types exhaustively against Rust: every value for the view and round trip, every pair of types for as.
- The wider types at a boundary set (0, 1, MAX, MAX - 1, MIN, MIN + 1, -1, powers of two and their neighbours) and at random.
- Every existing kernel_numbers test passes on the new model.

Done when. What the logic says about a machine value is what Rust computes, tested where it can be tested exhaustively and at the edges where it cannot.

<a id="LOC-163"></a>
## LOC-163 · K5 · The table of primitive operations: result type, panic condition, logical meaning
<!-- task: {"id": "t279", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T01:13:10.000Z"} -->

Landed 21 September as src/kernel/ops.rs: Op x MachineInt, 72 rows, each with arity, panic condition (never, overflow, division), whether a wrapping build wraps it instead, the total meaning wrap(exact), and the exact meaning under fits. Prim::Op(op, T) evaluated totally by the kernel; axioms op_model (every row) and op_exact (overflowing rows, under fits). Exhaustive at 8 bits against Rust's checked and wrapping operations; the contract argues op_model at a panicking pair is harmless because nothing continues. The interpreters consult Row::panic and wraps_instead in E6.

Lane: Kernel. Wave 5.
Depends on: K4 ([LOC-162](core-build.md#LOC-162)).
Unblocks: E6 ([LOC-172](core-build.md#LOC-172)).
Covers: [LOC-121](core-build.md#LOC-121), [LOC-12](core-build.md#LOC-12).

Scope. One trusted table with a row for each type and each of + - * / %, and for the wrapping methods. The panic condition and the meaning are terms over the model of K4.

Tests.
- At 8 bits, every pair of operands for every row: the panic condition holds exactly when Rust's checked operation returns None, and otherwise the meaning equals Rust's result.
- At wider types the boundary set crossed with itself, and random pairs.
- MIN / -1 and MIN % -1 as named tests for each signed type.
- Unary minus on the signed types: panics when the operand is MIN, wraps to MIN. In wrapping mode only the overflow of + - * and unary minus wraps; / and % by zero, MIN / -1, and MIN % -1 panic in every build, and the table says so row by row.

Done when. The table is small enough to read and is tested against Rust row by row.

<a id="LOC-164"></a>
## LOC-164 · K6 · A kernel rule that checks a linear arithmetic certificate, with its whole boundary written first
<!-- task: {"id": "t280", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T00:48:34.000Z"} -->

Landed 21 September as src/kernel/linear.rs, 321 lines, reviewed line by line. Proof::Linear { goal, goal_coefficient, pairs }; conclusions are read from checked proofs, never stated; limits are counts (256 pairs, 256 atoms, 512-bit literals). The contract's four midpoint certificates and six more from the target examples are accepted; 549 one-step mutants agree with an i128 recomputation (4 stay valid); 100,000 random certificates for false goals, none accepted. Text form is Display only until E11. Decided in code and stated in the contract: atoms that cancel are dropped, so (x - x) * y reads as 0; no commuting inside atoms.

The contract and one complete certificate are written: Target language, The linear certificate, in full. The four sums in it were checked by machine. Decided there: the goal is s <= t or False, and an equation is two inequalities and int_le_antisymm; a conclusion is read as s <= t, its negation, or an equation at Int; equations may take a coefficient of either sign; the coefficient of the negated goal is explicit; nothing is divided or rounded.

Lane: Kernel. Wave 3.
Depends on: K2 ([LOC-160](core-build.md#LOC-160)).
Unblocks: K7 ([LOC-165](core-build.md#LOC-165)).
Covers: [LOC-123](core-build.md#LOC-123).

Scope. A certificate is a goal, a list of pairs of a kernel proof and a non-negative literal coefficient, and nothing else. Every constraint enters as the conclusion of a proof the kernel checks in the ordinary way: a hypothesis from the context, a range fact as an instance of the axiom of K4, a quotient and remainder constraint as instances of the axioms of K3, the bridge from a comparison of machine values to a comparison of their views as an instance of K4 or K8. The rule itself does three things, all trusted: it reads each conclusion and the negated goal as a linear expression over Int, treating any term that is not +, -, a literal, or multiplication by a literal as an opaque atom compared by same; it tightens strict inequalities by discreteness; and it checks that the combination sums to a false statement about literals. The procedure states no constraint on its own authority. Before any code, one complete certificate is written into the Kernel contract for the last obligation of midpoint, showing the goal and hypotheses, their linear forms, the proof behind each range fact and each division constraint, and the final sum. The certificate is given a text form with a version in this commit, since E11 stores it.

Tests.
- The written certificate, and certificates by hand for the other six arithmetic obligations of Target examples, are accepted.
- A test-side recomputation of the combination in i128 decides whether a certificate is valid; over all changes of one coefficient, sign, or hypothesis, the kernel accepts exactly the valid ones. Some changes leave a certificate valid, and those must be accepted.
- Fabricated constraints: a range fact with no proof, a range fact for the wrong type, a quotient constraint whose proof is for another divisor, a remainder bound taken without b != 0. Each is rejected, and rejected as a bad proof and not as bad arithmetic.
- Goals that are false on a small bounded domain are never accepted under any of 100,000 random certificates.
- Two atoms that differ are never merged; two that are same are.
- Size limits as counts, with a test at the limit.

Done when. The kernel can check arithmetic it did not find, and everything the check relies on is either a kernel proof or one of three trusted steps named in the contract.

<a id="LOC-165"></a>
## LOC-165 · K7 · The arithmetic procedure: finds certificates for linear goals, machine ranges, and division by a literal
<!-- task: {"id": "t281", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T02:05:21.000Z"} -->

Landed 21 September as src/arith (untrusted): collects hypotheses, And parts, proof-typed variables, view ranges, and division facts, each with its kernel proof; Fourier-Motzkin with multiplier bookkeeping, then branch and bound on int_le_total when the rational point is fractional; budgets are counts only; every returned proof is checked by the kernel before it is returned. All seven target obligations found with certificates matching the contract's sizes; random box problems: 97 to 98 percent of true goals found, no false goal ever proved, verified counterexamples for the rest. Deviation: prove takes the Prelude as a parameter. Limits for E7: exact-result equations of operators must be supplied as hypotheses; proof fields of struct variables are not read.

Lane: Kernel. Wave 5.
Depends on: K4 ([LOC-162](core-build.md#LOC-162)), K6 ([LOC-164](core-build.md#LOC-164)).
Unblocks: E7 ([LOC-173](core-build.md#LOC-173)).
Covers: [LOC-123](core-build.md#LOC-123), [LOC-72](core-build.md#LOC-72).

Scope. Outside the kernel and untrusted. Turns the goal and the facts in scope into linear constraints over Int, asks for the range of every machine view and the quotient and remainder facts for every division by a literal as axiom instances, eliminates variables, and emits a certificate for K6. Its effort is limited by counts (variables eliminated, constraints generated, size of the certificate), never by a clock, so that the same file is accepted or rejected on every machine.

Tests.
- Finds all seven obligations from Target examples.
- Random small linear problems in which every variable carries the hypotheses -8 <= x and x <= 8, so that brute force over that range decides exactly the problem posed: a false one is never accepted, asserted through the kernel, and the share of true ones found is printed and must be at least 95 percent within the fragment.
- At the budget, the diagnostic says the procedure gave up, names the budget, and lists the constraints it had. The same input gives the same outcome on every run.

Done when. Overflow obligations at 32 and 64 bits are dischargeable; this answers [LOC-72](core-build.md#LOC-72).

<a id="LOC-166"></a>
## LOC-166 · K8 · Lemmas about Int and the machine types, callable by name
<!-- task: {"id": "t282", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T01:49:13.000Z"} -->

Landed 21 September. Added to the trusted base: Prim::Cmp(Eq|Lt|Le, T) on every machine type and the axiom cmp_reflect, in the shape of reflect, over the views. Not trusted: 134 lemmas in theory.rs (6 on Int, 16 per machine type: le_refl, le_trans, le_of_lt, lt_of_le_of_ne, lt_irrefl, le_antisymm, view_injective derived from wrap_view, view_bounds, the cmp bridges both ways, lt_of_not_le, le_of_not_lt), several proved with Proof::Linear, all listed by Theory::lemma_names(). E5 exposes them to source and must rebind u8_le_refl, u8_le_trans, u8_lt_of_le_of_ne from the Nat-model lemmas to the table.

Lane: Kernel. Wave 5.
Depends on: K4 ([LOC-162](core-build.md#LOC-162)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).
Covers: [LOC-124](core-build.md#LOC-124).

Scope. The u8 ordering lemmas generalize to every machine type, plus the bridge between a comparison of machine values and the comparison of their views.

Tests.
- Each lemma is called once from a corpus file.
- The names are listed by a test so that a rename is deliberate.

Done when. A proof can be written by hand where the procedure does not reach.

<a id="LOC-167"></a>
## LOC-167 · E1 · Evidence must match exactly after computing; dependent patterns open over their own names
<!-- task: {"id": "t283", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T01:04:23.000Z"} -->

Landed 21 September. Removed from the solver: rewriting by equations in scope, unfolding without unfold!, the structural search, and the proof by 256 cases. Kept as computing: let and pattern names replaced, projections and matches of written values, literal arithmetic, reflexivity, Reflect of the branch fact, And-splitting, closed evaluation; and u8_zero_le for the 0..n range, which has no place for evidence. Tiers now reported: exact, computed, evaluation. A dependent pattern opens over its own names, so evidence out of a call is used exactly with no hole. lock.lc: 8 proofs of 775 nodes became 5 of 76, and its sizes are asserted. src/elab/explain.rs re-runs the removed tiers for diagnosis only and prints a suggestion it has checked. Seven lemmas added to theory.rs (u8_sub_le, u8_sub_le_sub, u8_eq_symm callable). Wart to remember: rewrite! replaces every occurrence, so n == 7 to n <= 7 needs a let-named left side.

Lane: Elaborator. Wave 4.
Depends on: S2 ([LOC-155](core-build.md#LOC-155)), H3 ([LOC-153](core-build.md#LOC-153)).
Unblocks: E7 ([LOC-173](core-build.md#LOC-173)), M4 ([LOC-180](core-build.md#LOC-180)).
Covers: [LOC-140](core-build.md#LOC-140), [LOC-139](core-build.md#LOC-139), [LOC-17](core-build.md#LOC-17).

Scope. Remove from the solver everything that bridges one claim to another: rewriting by facts in scope, unfolding without being asked, the structural search, and the proof by all 256 cases. Keep computing: let names replaced, projections and matches of written values, arithmetic on literals, and closed evaluation. let (next, still) = step(...) types still over next. lock.lc is rewritten with the evidence it now has to state.

Tests.
- For each removed tier, a rejected file whose diagnostic shows the claim, the computed claim, the nearest fact, and the explicit form that would be accepted.
- The pattern-opening test from finding 2 of Target examples.
- Proof sizes for lock.lc are asserted, so growth is visible.

Done when. Checking is deterministic and explainable: a hole is filled by a fact that matches or it is not filled.

<a id="LOC-168"></a>
## LOC-168 · E2 · Promises: terminates, no_panic, no_alloc, no_io, checked and never inferred
<!-- task: {"id": "t284", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T15:38:15.000Z"} -->

Landed 22 September. FnInfo carries the four promises from attributes plus the file default; a call to a callee lacking a promise the caller makes is L0232 at the call, a loop under terminates L0215 at the keyword, a promise on a non-function L0233; a function may appear in a proposition (or under unfold!/fold!, or as evidence) iff it promises terminates, no_panic, no_io and takes no &mut (L0209 names the missing one). Such functions lower as kernel functions with a defining equation, exactly as math fn did, so math fn is now one spelling of the three (E3 removes it). Promises reach the trusted checker through Session::declare_fn_promising; a hand-built discrepancy is caught there. 16x16 matrices tested, plus through file defaults. decreases is parsed and reported as the Recursion project's.

Lane: Elaborator. Wave 6.
Depends on: S4 ([LOC-157](core-build.md#LOC-157)).
Unblocks: E3 ([LOC-169](core-build.md#LOC-169)), E6 ([LOC-172](core-build.md#LOC-172)), E8 ([LOC-174](core-build.md#LOC-174)), O2 ([LOC-183](core-build.md#LOC-183)), E10 ([LOC-190](core-build.md#LOC-190)).
Covers: [LOC-13](core-build.md#LOC-13), [LOC-73](core-build.md#LOC-73), [LOC-14](core-build.md#LOC-14).

Scope. A function keeps a promise only if everything it calls makes it. terminates forbids loops and, until recursion arrives, any call cycle. #![...] sets a default for the file. A function may appear in a proposition exactly when it promises terminates, no_panic, and no_io and takes no &mut. math fn stays for one commit as another spelling of the three.

Tests.
- A generated matrix of caller promises against callee promises, 16 by 16, accepted or rejected with the missing promise named.
- A function mentioned in a proposition without a promise is rejected, naming the promise.
- The file default applies and an explicit list on a function adds to it.

Done when. The specification author states what a function may not do, and the checker holds them to it.

<a id="LOC-169"></a>
## LOC-169 · E3 · Remove math fn
<!-- task: {"id": "t285", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T16:34:29.000Z"} -->

Historical milestone. The replacement described here was itself superseded by explicit logic fn in [LOC-210](reconciliation.md#LOC-210).

Landed 22 September. math fn is gone: L0114 reports it with a machine-applicable fix to #[terminates] #[no_panic] #[no_io] fn; the logical test is the promises alone; fn(..) -> .. is the one function type. Generated Rust of every example and accept file was byte-identical before and after.

Lane: Elaborator. Wave 7.
Depends on: E2 ([LOC-168](core-build.md#LOC-168)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).
Covers: [LOC-14](core-build.md#LOC-14).

Scope. Migrate every example and test to the promises. A migration diagnostic with a fix replaces the keyword.

Tests.
- FunctionMode::Math is gone from the source.
- The generated Rust of every example is unchanged, byte for byte.

Done when. One way to say a function can be used in the logic.

<a id="LOC-170"></a>
## LOC-170 · E4 · A panic is a third outcome: the interpreters, the erased tree, and the harness
<!-- task: {"id": "t286", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T23:13:31.000Z"} -->

Landed 21 September. One Outcome type for both interpreters (Value, Panic(message), OutOfFuel); EExpr::Panic; the corpus runner understands => panic and => panic: message, builds twice (overflow checks on and off), and treats out of fuel and a timeout as inconclusive. Notes for later commits: the erased check now lets any never-yielding subexpression through and stops checking what follows it in a block, which M1 should tighten so that dead arms are still checked; an untyped panic in a let can defeat Rust's inference (E0282), so M1 needs a type on the let or on the panic. When O3 adds the state of &mut parameters to a panic, Panic becomes a struct.

Lane: Elaborator. Wave 2.
Depends on: H1 ([LOC-151](core-build.md#LOC-151)).
Unblocks: E6 ([LOC-172](core-build.md#LOC-172)), M1 ([LOC-177](core-build.md#LOC-177)).
Covers: [LOC-138](core-build.md#LOC-138).

Scope. Preparation only, with no surface syntax and no checked semantics. Both interpreters return a value, a panic with its message, or out of fuel. The erased tree and the printer gain a panic expression, reached for now only from trees built by hand. The compiled harness calls each function under catch_unwind, can build with overflow checks on or off, and runs the program under a process timeout; a timeout is reported as inconclusive and never as a disagreement or as divergence. The statement of erasure in Architecture is restated for three outcomes.

Tests.
- Erased trees built by hand that panic agree with their compiled Rust, message included.
- Out of fuel stays distinct from a panic and is never compared as one.
- The harness is tested on a program that loops forever: inconclusive, and the rest of the batch is still compared.

Done when. Tests can say f(255) => panic. What a panic means to the checker comes with M1 for block endings and with E6 for operators.

<a id="LOC-171"></a>
## LOC-171 · E5 · Every machine integer type, literal typing, as, Int, and comparison within one type
<!-- task: {"id": "t287", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T15:08:10.000Z"} -->

Landed 22 September (the agent ran out of credits after passing the checks; I committed its staged work and landed it). Every machine type in the typed tree, both interpreters, the erased check, and the printer; literals take the expected type, else suffix, else i32, and are range-checked; as wraps, as Int is the view; comparisons within one type, lowered to cmp, and in formulas the order of the views. The u8 model over Nat is gone from the kernel and the contract; the bounded for rests on the Int model; the 134 theory lemmas are callable by name. Corpus: casts.lc with 480 run lines against compiled Rust, integer_types.lc, lock_u32.lc; the ignored R1 test passes. Milestone 1 reached: the new surface.

Lane: Elaborator. Wave 5.
Depends on: K4 ([LOC-162](core-build.md#LOC-162)), S3 ([LOC-156](core-build.md#LOC-156)).
Unblocks: E6 ([LOC-172](core-build.md#LOC-172)), E8 ([LOC-174](core-build.md#LOC-174)).
Covers: [LOC-2](core-build.md#LOC-2), [LOC-3](core-build.md#LOC-3), [LOC-9](core-build.md#LOC-9), [LOC-94](core-build.md#LOC-94), [LOC-99](core-build.md#LOC-99), [LOC-96](core-build.md#LOC-96).

Scope. The typed tree, both interpreters, the erased check, and the printer learn u8 to u64 and i8 to i64. A literal takes the expected type, else i32, or its suffix, and is checked against the range. as between integer types wraps; as Int is exact and Int is logic-only. == != and the orderings need both sides of one type. u32::MAX and MIN resolve as associated constants.

Tests.
- Literals one past each range are rejected.
- as for every pair of types at the boundary set, compared with compiled Rust through the corpus runner.
- A comparison across types is rejected and the message shows the cast to add.
- lock.lc with u32 counts runs.
- The u8 model over Nat (to_nat, of_nat, the wrapping models, reflect on u8, the Nat lemmas of theory.rs) is removed once the elaborator builds on the Int model; the contract then lists one schema. The ignored test of R1 (a byte typed by a literal alone as a receiver, E0689) passes: the printer suffixes literals or annotates the let.
- Theory::lemma_names() (134 names from K8) is exposed to source; u8_le_refl, u8_le_trans, u8_lt_of_le_of_ne rebound from the Nat-model lemmas to the table.

Done when. Programs are no longer limited to bytes.

<a id="LOC-172"></a>
## LOC-172 · E6 · The operators + - * / % with their panic conditions and what the logic learns
<!-- task: {"id": "t288", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T17:42:04.000Z"} -->

Landed 22 September. Stmt::Operate in the check IR: the trusted checker defines the result by the every-build (wrapped) equation, checks the fits proofs when given, and then assumes the exact result through a derivation from op_exact that it builds and checks itself; under no_panic the proofs are required; after / and % the premises are assumed in every function. Both interpreters have Checks and Wrap modes; the corpus runner and the differential test run both against the matching rustc build; //~ run: f(255) => panic | 0 gives the two outcomes. The elaborator discharges an obligation by exact, computed, evaluation, then K7 (needed even for unsigned +, whose premise 0 <= a + b only the view ranges give); L0235 shows the premise written out with a counterexample; L0236 rejects an operator in a formula. 10,000 random programs with operators agreed three ways in both builds. Open point for the vision discussion: a fully promised function is a kernel term and has no place for an operator's evidence, so + is refused there (L0236) although the Vision allows + under no_panic.

Lane: Elaborator. Wave 7.
Depends on: K5 ([LOC-163](core-build.md#LOC-163)), E2 ([LOC-168](core-build.md#LOC-168)), E4 ([LOC-170](core-build.md#LOC-170)), E5 ([LOC-171](core-build.md#LOC-171)), M0 ([LOC-176](core-build.md#LOC-176)).
Unblocks: E7 ([LOC-173](core-build.md#LOC-173)).
Covers: [LOC-11](core-build.md#LOC-11), [LOC-12](core-build.md#LOC-12), [LOC-121](core-build.md#LOC-121).

Scope. Operators elaborate through the table of K5. The check IR gains primitive operations that may panic, as M0 designs them: under no_panic the panic condition is an obligation at the operator and the exact result is known after; otherwise only the wrapped result is known. After a / b the divisor is known not to be zero. In a formula, an operator on a machine type is rejected, because it may panic, and the message offers as Int and the wrapping method. The reference interpreters panic on overflow by default and have a wrapping mode, since Rust does either according to the build. This commit changes the checker of the check IR, which is trusted, and updates Architecture.

Tests.
- For every type and operator, at the boundary set: the interpreter in its default mode agrees with Rust built with overflow checks on, and in its wrapping mode with Rust built with them off. / and % by zero and MIN / -1 panic in both.
- What the logic learns outside no_panic, the wrapped value, holds in both builds; a test asserts a fact proved from it against both compiled results.
- An undischarged obligation under no_panic is reported at the operator with its condition written out.
- Check IR built by hand that omits the evidence at an operator under no_panic is rejected by the checker.
- The random program generator of R1 now emits arithmetic, in both build configurations.
- The printer parenthesizes a cast that is the left operand of < or << (S3 found rustc reads them as generics).

Done when. x + 1 means what it means in Rust, and a function that promises no_panic has shown it cannot overflow.

<a id="LOC-173"></a>
## LOC-173 · E7 · Arithmetic in holes and prove!
<!-- task: {"id": "t289", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T19:36:37.000Z"} -->

Landed 22 September. A hole is tried as exact, computed, evaluation, then arithmetic through K7 over the facts in both spellings; --holes prints the certificate size; the failure diagnostic shows the counterexample in source spelling or the budget hit. After a / k with a literal k the exact quotient is derived, nothing assumed. Milestone 2 reached: tests/corpus/target/midpoint.lc checks, runs, and compiles, with six obligations (1 evaluation, 5 arithmetic; the prediction of 3/4/7 for lock plus midpoint counted each operator once where a row has two premises, and folded evaluation into computed). lock32_step.lc holds step and remaining. The full target lock.lc waits on M4 (let mut ok) and O1 (derive). Interim rule for [LOC-193](core-build.md#LOC-193) in place: a fully promised function with an operator is an ExecFn with its promises and is refused in propositions with the reason.

Lane: Elaborator. Wave 8.
Depends on: K7 ([LOC-165](core-build.md#LOC-165)), E1 ([LOC-167](core-build.md#LOC-167)), E6 ([LOC-172](core-build.md#LOC-172)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)), E11 ([LOC-192](core-build.md#LOC-192)).
Covers: [LOC-72](core-build.md#LOC-72), [LOC-15](core-build.md#LOC-15).

Scope. A hole is tried as exact, then computed, then arithmetic. When the procedure fails and finds values that satisfy the facts and break the goal, the diagnostic shows them.

Tests.
- midpoint, and step and remaining from the 32-bit lock, check, run, and compile; their corpus files move from parse-only to run.
- check --stats classifies the obligations as Target examples predicted: three exact, four computed, seven arithmetic. A difference is a finding to record, not a number to adjust.
- midpoint without its ordered hypothesis is rejected with a counterexample.

Done when. The second milestone: real 32-bit programs with overflow shown impossible.

<a id="LOC-174"></a>
## LOC-174 · E8 · Logic-only types and the one erasure rule: Ghost<T>, snapshot!, erased positions
<!-- task: {"id": "t290", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T20:42:07.000Z"} -->

Historical milestone. Ghost<T> and snapshot! were later removed by [LOC-209](reconciliation.md#LOC-209); current Logical classification and model observations are the supported API.

Landed 22 September. Ghost<T> is the kernel type T with a ghost binder; it has no erased representation, and the erased check refuses any binding of ghost type as the second judge. snapshot!(e) builds one; a ghost local at runtime is L0201. The one erasure rule in erase.rs: a call is removed iff its result has no runtime form and the callee promises terminates, no_panic, and no_io (the &mut clause is a comment for O3); otherwise the call stays with its result replaced by the marker. Logic-only contexts (the argument of snapshot!, a let of type Ghost, Int, or Prop, a Ghost argument, proposition arguments, evidence constructors) require every callee to make the three promises (L0209 names the missing one). Seams for the discussion: arguments of an erased call are not a logic-only context and are kept as let _ = arg; a struct with only ghost fields is still a runtime struct of markers; no Seq anywhere.

Lane: Elaborator. Wave 7.
Depends on: E5 ([LOC-171](core-build.md#LOC-171)), E2 ([LOC-168](core-build.md#LOC-168)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).
Covers: [LOC-97](core-build.md#LOC-97), [LOC-81](core-build.md#LOC-81), [LOC-141](core-build.md#LOC-141).

Scope. Two rules that must not be confused. An erased result: a position is erased exactly when its type has no runtime form, so the evidence a call returns vanishes and the call stays. An erasable computation: a call is removed only if its result is logic-only, it promises terminates, no_panic, and no_io, and it takes no &mut to runtime storage. An expression in a logic-only context, such as the argument of snapshot!, the right side of a let of type Int, or a formula, must be an erasable computation throughout, and is rejected otherwise. Ghost<T> is built by snapshot!(e). Seq<T> is proposed to leave the core: no acceptance example needs it and it brings a theory of its own.

Tests.
- A pure call in a logic-only context: accepted, and absent from the generated Rust.
- A call that may panic in a logic-only context: rejected, naming the missing promise. The same for one that may not terminate, and one that does I/O.
- An ordinary runtime call that returns only evidence and may panic or loop: accepted, kept in the generated Rust, its result erased; the three-way comparison sees its panic.
- A call with a logic-only result and a &mut parameter to runtime storage: kept. This case is written now and enabled by O3.
- The generated Rust of a file using Ghost and Int holds nothing of them but markers, by golden file, and the erased check rejects a hand-built erased tree that keeps a logic-only value.

Done when. Specifications can carry values the program never computes.

<a id="LOC-175"></a>
## LOC-175 · E9 · Variants with named fields, const as a Rust const, separate type and value namespaces
<!-- task: {"id": "t291", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T16:34:29.000Z"} -->

Landed 22 September. Variants with named fields elaborate (fields by position in the kernel, names in the elaborator; L0224 for wrong fields; props too); const prints as a Rust const when Rust can evaluate the initializer (L0234 otherwise) and keeps its equation in the logic; Env has separate type and value namespaces, tested with a struct and a function of one name. Found and fixed on the way: false recursion (L0203) when a local was named like an item; order.rs is now scope-aware.

Lane: Elaborator. Wave 6.
Depends on: S4 ([LOC-157](core-build.md#LOC-157)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).
Covers: [LOC-100](core-build.md#LOC-100), [LOC-101](core-build.md#LOC-101), [LOC-96](core-build.md#LOC-96).

Scope. The remaining small parity items that touch only the elaborator and the printer.

Tests.
- Corpus files for each, run and compiled.
- A type and a value of one name coexist as in Rust.

Done when. Small, independent, and a good filler for an idle lane.

<a id="LOC-176"></a>
## LOC-176 · M0 · Write the design of the checked IR for mutation, early exit, panics, and operations that may panic
<!-- task: {"id": "t292", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T23:13:31.000Z"} -->

Written 21 September, reviewed, and revised the same day: evaluation order as part of the trusted translation, binding identities for the assigned set, one representation for a join that assigns, what may be lent, what a panic carries, all four promises in the checker, wrapping mode by operation, and cases 9 to 11. Target language: How mutation is checked.

Lane: Mutation. Wave 1.
Depends on: nothing.
Unblocks: E6 ([LOC-172](core-build.md#LOC-172)), M1 ([LOC-177](core-build.md#LOC-177)).
Covers: [LOC-129](core-build.md#LOC-129).

Scope. A section of Target language, not code. For each of the six cases of [LOC-129](core-build.md#LOC-129) it shows the source, the check IR after lowering to versions, and what the checker demands at each point: a branch that refreshes against one that returns early; several continue and break paths; snapshot against tracked evidence; a captured proposition used as a tracked type; move and reinitialise; a call that changes two disjoint fields. A seventh case covers a primitive operation that may panic, under no_panic and outside it, since E6 needs it before the rest. An eighth covers a protected value behind &mut when a panic unwinds, once that rule is decided.

Tests.
- Each case is written so that it can be copied into a test in M2 to M4 and O3.
- Reviewed by Nikhil before M1 starts.

Done when. The gate. No mutation code is written before this is agreed.

<a id="LOC-177"></a>
## LOC-177 · M1 · The check IR gains return and panic as ways a block ends
<!-- task: {"id": "t293", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T23:37:20.000Z"} -->

Landed 21 September. Promises on ExecFn, enforced by the checker: no_panic (callees and a proof of False at every panic), terminates (no loop or for anywhere, callees), no_io and no_alloc (callees). Tail::Return checked against the result type at that point; Tail::Panic with an optional proof. Both interpreters unwind a return. EExpr::Return in the erased tree, tested three ways. The erased check again checks everything after a never-yielding value, and bindings carry their type so the printer can annotate a let whose value diverges. Note for M3: LoopUnderTerminates refuses a bounded for too, as designed; relaxing it is one condition.

Lane: Mutation. Wave 3.
Depends on: M0 ([LOC-176](core-build.md#LOC-176)), E4 ([LOC-170](core-build.md#LOC-170)).
Unblocks: M2 ([LOC-178](core-build.md#LOC-178)), E10 ([LOC-190](core-build.md#LOC-190)).
Covers: [LOC-142](core-build.md#LOC-142), [LOC-93](core-build.md#LOC-93).

Scope. src/exec only: the IR, its checker, and its interpreter. At a return the checker demands the function's result type; at a panic it demands nothing, or under no_panic evidence that the point is unreachable.

Tests.
- Programs built by hand in the IR, as exec_check does today: a return inside a branch, a panic inside a branch, both.
- A return with the wrong evidence is rejected at the return.
- Architecture's account of the trusted base is updated.
- All four promises are enforced by the checker, no_io and no_alloc included, each with a hand-built function that breaks it.
- The erased check still checks the arms and statements that follow a never-yielding expression, which E4 stopped doing.
- A let bound to a panic gets a type in the generated Rust, so that inference cannot fail (E0282).

Done when. The checker is ready before any surface syntax reaches it.

<a id="LOC-178"></a>
## LOC-178 · M2 · let mut and assignment, lowered to versions
<!-- task: {"id": "t294", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T16:13:17.000Z"} -->

Landed 22 September. Stmt::Assign with a Place (binding plus field path) in the typed tree; lowering emits a let of the rebuilt value per assignment, verifies every executable mention is the current version, and lowers a branch that assigns an outer binding to a match whose result is the tuple of new versions then the value; the tree carries the version identities so proofs can mention them and lowering checks the join set against its own computation. Erasure maps versions back and prints let mut and x.f = e as written; mut only where assigned. Diagnostics: L0232 assign twice to immutable, L0233 a field that evidence depends on. Cases 9 and 10 in the corpus (case 9's &mut form waits for O3); the five assigned-set rules tested on the IR shape; the generator emits mutable locals and branching assignment, 10,000 programs agreed three ways. Deviation from the design, recorded: lowering verifies versions the tree supplies rather than inventing them, since proofs must name them; soundness is the same.

Lane: Mutation. Wave 7.
Depends on: M1 ([LOC-177](core-build.md#LOC-177)), S5 ([LOC-158](core-build.md#LOC-158)).
Unblocks: M3 ([LOC-179](core-build.md#LOC-179)), O1 ([LOC-182](core-build.md#LOC-182)).
Covers: [LOC-1](core-build.md#LOC-1), [LOC-142](core-build.md#LOC-142).

Scope. The typed tree gains assignment to a variable or field path and stays source-shaped. lower gives each assignment a new version and makes a variable assigned in a branch a result of the branch. The printer emits let mut and assignment as written.

Tests.
- Differential and compiled tests for straight-line and branching mutation.
- The generator of R1 emits mutable locals, and the three-way comparison holds.
- The generated Rust has no unused mut, which -D warnings already enforces.
- Case 9: the right side changes what the left side names; lo == 3 and hi == 7 three ways. The same for the two argument orders.
- Case 10: both arms assign and refresh, and the condition reads the entry version.
- The assigned set, on the check IR lowering produces and through both interpreters: an outer binding assigned in a nested block; a field write counting for its root; a local let mut inside a branch staying local; a shadowing binding assigned while the outer one is not, and the reverse.

Done when. The first program that reads like ordinary imperative Rust.

<a id="LOC-179"></a>
## LOC-179 · M3 · Rust's loops over let mut; the state-passing loop and the bounded for are removed
<!-- task: {"id": "t295", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T17:55:18.000Z"} -->

Landed 22 September (rebased over E3, E9, E6). loop, while, for over a range of any machine type with ..=, break with a value from loop, continue; the old state-passing forms are reported with L0234 and the new spelling. The loop state is the tuple of outer bindings the body (or a while condition) assigns, computed by lowering; the bounded For of the check IR lost its ordered proof and index-dependent state, gained inclusive and break. No loop lowers to a kernel term any more. lock.lc keeps run by carrying (lock, evidence) as one let mut tuple, an M2-legal program; the tracked spelling is M4. The extended random run found a real lower-versus-erase disagreement (an effectful argument of a dropped lemma call), fixed in erase.rs. The while exit fact is not exported after the loop until M4.

Lane: Mutation. Wave 8.
Depends on: M2 ([LOC-178](core-build.md#LOC-178)).
Unblocks: M4 ([LOC-180](core-build.md#LOC-180)), M5 ([LOC-181](core-build.md#LOC-181)).
Covers: [LOC-127](core-build.md#LOC-127), [LOC-5](core-build.md#LOC-5).

Scope. loop, while, and for i in a..b. A variable assigned in a loop becomes loop state in the check IR. break with a value, continue.

Tests.
- Every old loop test is ported and passes.
- The generated Rust uses the loop form the source used, by golden file.
- The generator of R1 emits loops; fuel bounds them.
- The assigned set for loops: a let mut inside the body is not loop state; an assignment in a while condition is; a &mut call in the condition is, once O3 lands.

Done when. The provisional loops are gone, as planned since they were added.

<a id="LOC-180"></a>
## LOC-180 · M4 · Tracked evidence: let mut evidence, invalidated by assignment, refreshed explicitly
<!-- task: {"id": "t296", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T20:22:59.000Z"} -->

Landed 22 September. Milestone 3 reached: tests/corpus/target/lock.lc checks, runs, and compiles in full (11 proofs), with let mut ok: @within_limit(lock.failures) carried by the loop and refreshed each pass. A tracked binding is a let mut whose type mentions other mutable bindings; assigning a dependency makes it stale (L0245 on use, L0246 at a continue, break, body end, or while exit); a refresh is an assignment checked against the claim over the current versions. Lowering types every version at joins and loop state over the current versions, so the trusted checker rejects a stale use as a type mismatch and needs nothing new; tested with a hand-built tree. Two lowering rules sharpened: an arm that leaves contributes nothing to a join; a proof-typed binding may be left out of a join or loop state. The while exit fact remains unexported.

Lane: Mutation. Wave 9.
Depends on: M3 ([LOC-179](core-build.md#LOC-179)), E1 ([LOC-167](core-build.md#LOC-167)).
Unblocks: O3 ([LOC-184](core-build.md#LOC-184)).
Covers: [LOC-16](core-build.md#LOC-16).

Scope. Assigning to a variable invalidates tracked evidence that mentions it; using it before a refresh is an error shaped like use after move. ok = _; or ok = proof; refreshes. Around a loop it must hold on entry and at every continue and at the end of the body, and is known after.

Tests.
- The first four cases of M0 as named tests.
- The flow analysis is tested as untrusted: typed trees built by hand that skip a refresh are rejected by the checker of the check IR, not by the elaborator.
- run from the 32-bit lock checks, runs, and compiles; the lock file is complete.

Done when. The third milestone: a loop that carries a proof, with no invariant construct.

<a id="LOC-181"></a>
## LOC-181 · M5 · return and the never type in the surface language
<!-- task: {"id": "t297", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T21:13:00.000Z"} -->

Landed 22 September. return lowers to Tail::Return in tail position and, elsewhere, to the two-arm leaving match E10 introduced for panics; a returning arm contributes nothing to a join. Never-typed values (return, break, continue, the panic forms, calls to -> ! functions) coerce to any expected type; a -> ! function has result type @False in the kernel and its body must end never-typed. Diagnostics gained a warning level: L0247 unreachable statement, as rustc warns, and locus check exits 0 on warnings; the corpus has //~ warning: directives and goldens beside accepted files. L0216 removed. Evidence owed at an early return is reported at the return.

Lane: Mutation. Wave 9.
Depends on: M3 ([LOC-179](core-build.md#LOC-179)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).
Covers: [LOC-93](core-build.md#LOC-93), [LOC-92](core-build.md#LOC-92).

Scope. return lowers to the block ending of M1. ! is the type of return, break, continue, and the panicking forms, and coerces to any type.

Tests.
- A return inside a loop inside a branch, three ways.
- Evidence owed at an early return is reported at the return.
- Code after a never-typed expression is reported as unreachable, as rustc would warn.

Done when. Early exit works as in Rust.

<a id="LOC-182"></a>
## LOC-182 · O1 · Moves, and derive with a closed list
<!-- task: {"id": "t298", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T19:45:50.000Z"} -->

Landed 22 September. src/elab/moves.rs, interleaved with elaboration: values move by default, Copy types never; partial moves as rustc does them; branches join by union over live arms, loops report a move of an outer value at the back edge; assignment reinitialises. L0240 use of moved value (rustc wording), L0241 a proposition mentioning a moved local, L0242 and L0243 for the derive list and its field rules (PartialEq refused where logic-only data sits at any depth). The printer emits exactly the derives the source declares. rustc as oracle: 15 rejected shapes, all E0382 when printed with the analysis skipped. Cost of the rule: lock32_step's (next, prove!(next.failures <= 3)) needed Copy on Lock, since the tuple moved next before the claim named it.

Lane: Ownership. Wave 8.
Depends on: M2 ([LOC-178](core-build.md#LOC-178)), S4 ([LOC-157](core-build.md#LOC-157)).
Unblocks: O3 ([LOC-184](core-build.md#LOC-184)).
Covers: [LOC-89](core-build.md#LOC-89), [LOC-25](core-build.md#LOC-25), [LOC-141](core-build.md#LOC-141), [LOC-99](core-build.md#LOC-99).

Scope. Values move by default. Copy, Clone, PartialEq, Eq, and Debug by derive only. Types with no runtime form are always Copy. Use after move is an error; assignment reinitialises. A proposition in a body cannot mention a moved local. PartialEq is derivable only for a type with no logic-only data at any depth.

Tests.
- rustc as the oracle, for runtime uses only: each program rejected for using a moved runtime value, printed as Rust through a test hook that skips the move check, is also rejected by rustc with E0382. A program rejected only because a proposition or erased expression mentions a moved local has no counterpart in the generated Rust, where that use is gone; those are tested as Locus rejections alone.
- Every accepted program compiles, which the runner already enforces.
- The fifth case of M0.

Done when. Locus is never more permissive than rustc about moves, and the generated Rust never surprises its compiler.

<a id="LOC-183"></a>
## LOC-183 · O2 · Visibility, the export boundary, and locus build
<!-- task: {"id": "t299", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T20:30:17.000Z"} -->

Landed 22 September. Visibility recorded and printed as written, private by default. The export boundary: L0244 a plain pub function with a forgeable parameter type (evidence, or a tuple, enum, or struct through which Rust could pass a marker or set a field the evidence speaks of); L0245 a pub struct carrying evidence with a pub field. locus build writes a Cargo crate ([LOC-135](core-build.md#LOC-135) closed). From the Rust side: calling a pub(crate) evidence-taking function, naming or constructing the marker, and building a struct with private fields are E0603, E0451, E0616; the marker replay fails to compile; a plain pub function runs. For the vision discussion: Target examples write pub fn step, remaining, and midpoint with evidence parameters, which the Vision's own rule forbids; the corpus copies say pub(crate).

Lane: Ownership. Wave 7.
Depends on: S4 ([LOC-157](core-build.md#LOC-157)), E2 ([LOC-168](core-build.md#LOC-168)).
Unblocks: O4 ([LOC-185](core-build.md#LOC-185)).
Covers: [LOC-90](core-build.md#LOC-90), [LOC-137](core-build.md#LOC-137), [LOC-18](core-build.md#LOC-18), [LOC-19](core-build.md#LOC-19), [LOC-135](core-build.md#LOC-135).

Scope. Private by default. The restricted forms of pub mean what they mean in Rust. A function that takes evidence, directly or inside a parameter type Rust could construct, may be visible no further than the generated root; plain pub on it is an error that offers a narrower visibility or a validated type. locus build writes the root module with the markers and one module per file; [LOC-135](core-build.md#LOC-135) must be decided first.

Tests.
- A hand-written Rust crate compiled against the generated module: calling an evidence-taking function, naming a marker constructor, and building a struct with private fields are each rejected by rustc with the expected error code; calling a plain pub function works and runs.
- The marker replay attack is written out as a test and fails to compile.
- Evidence hidden in a pub field of a pub struct is caught.

Done when. The guarantee at the boundary with Rust rests on what is exported and is tested from the Rust side.

<a id="LOC-184"></a>
## LOC-184 · O3 · References as parameters: &T and &mut T, path arguments, disjointness, old!
<!-- task: {"id": "t300", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T21:43:47.000Z"} -->

Landed 22 September. Tier 0 of references: &T and &mut T as parameter types only; arguments are paths; a &mut parameter is a let mut inside the callee whose final version is one more field of the result tuple, bound at the call and written back to the root by the inside-out rebuilding an assignment uses, so tracked evidence goes stale and the callee's returned evidence about the exit value refreshes it; old!(x) names the entry value in propositions. Overlapping arguments are refused by lowering and, earlier, by the elaborator with rustc's codes (L0263: E0499, E0502, E0503, E0505); E0596, E0507 likewise; all observed from rustc on the printed twins. The panic rule as designed: the value at the moment of the panic is what the caller observes, in both interpreters (a side table the checker never reads) and in the compiled harness under catch_unwind. Lending a field that evidence depends on is refused (L0233). The generator lends; E8's &mut test is enabled. *x place syntax waits for O4.

Lane: Ownership. Wave 10.
Depends on: O1 ([LOC-182](core-build.md#LOC-182)), M4 ([LOC-180](core-build.md#LOC-180)).
Unblocks: O4 ([LOC-185](core-build.md#LOC-185)).
Covers: [LOC-76](core-build.md#LOC-76), [LOC-81](core-build.md#LOC-81).

Scope. A &mut parameter lowers to a value passed in and a new version passed out. Arguments are paths. Two arguments may not overlap. old!(x) names the entry value. Matching through & binds only Copy fields.

Tests.
- rustc as the oracle for rejected aliasing of runtime values (E0499, E0502, E0506).
- The sixth case of M0, and the eighth: whatever rule is decided for a panic that unwinds past a protected value behind &mut, a rejected file shows it.
- The mutating-call case of E8 is enabled.
- The generator of R1 emits calls with &mut.
- A field that evidence in its struct depends on cannot be lent: &mut percent.value is rejected by lowering, and &mut percent is accepted.
- A panic carries the values of the &mut parameters at that moment, in both interpreters and in the compiled harness, which reads the arguments after catch_unwind. A callee that writes and then panics, called directly and through a caller that lent a field of its own &mut parameter.

Done when. Tier 0 of references is complete.

<a id="LOC-185"></a>
## LOC-185 · O4 · Inherent impl blocks and self; the protected type end to end
<!-- task: {"id": "t301", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T22:29:26.000Z"} -->

Landed 22 September. Milestone 4 reached: every program in Target examples checks, runs, and compiles. Methods are functions named Type::name with self as the first parameter passed as written; a method call is rewritten to the path call with the receiver lent or moved, so O3's rules apply unchanged; *self is the one dereference Locus reads. The printer alone rebuilds impl blocks from the erased tree's owner and receiver marks. From Rust: Percent::checked works, Percent::new is E0624, the literal E0451, the field E0616; set_twice under catch_unwind leaves the valid value written before the panic. Deviations of the target files from the atlas text, for the discussion: pub(crate) on evidence-taking functions; an enum Checked in place of Option<Percent>; Debug derives; value <= 100 written without the cast. percent.lc: 3 obligations, all computed (the text predicted 1 exact).

Lane: Ownership. Wave 11.
Depends on: O2 ([LOC-183](core-build.md#LOC-183)), O3 ([LOC-184](core-build.md#LOC-184)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).

Scope. Methods and associated functions in impl blocks, with self, &self, and &mut self. The Percent example checks and builds, with an enum of its own in place of Option, which waits for generics.

Tests.
- Percent moves from parse-only to run.
- The Rust caller crate of O2 uses Percent: checked works, new is not visible, the fields are not visible.
- The Rust caller calls a method that updates a Percent through &mut self under catch_unwind, with an input chosen to make it panic if anything can, and then reads the value that survives: it satisfies the invariant. If the rule decided forbids the method instead, the rejected file is the test.
- Case 11: a method that completes one valid replacement and then panics; the Rust caller finds the new valid value.

Done when. The fourth milestone: every program in Target examples works.

<a id="LOC-186"></a>
## LOC-186 · R1 · A generator of random well-typed programs, compared three ways
<!-- task: {"id": "t302", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T00:04:53.000Z"} -->

Landed 21 September as tests/random_programs.rs with tests/common/compiled.rs (harness pieces copied from tests/corpus.rs, to deduplicate later). Type-directed generation of typed trees with weighted productions; loops bounded by construction; every program declared through the real Session and checked. Fast: 300 programs, 2,617 cases, 6 s; extended: 10,000 programs, 90,633 cases, 0 inconclusive, 0 disagreements, 180 s. Shrinking tested on a planted disagreement. Finding, not fixed: the printer writes byte literals without a suffix, so let k = 200; k.wrapping_add(n) and a for index used as a receiver are accepted by Locus and rejected by rustc (E0689); kept as an ignored test and given to E5.

Lane: Robustness. Wave 2.
Depends on: H1 ([LOC-151](core-build.md#LOC-151)), H2 ([LOC-152](core-build.md#LOC-152)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).

Scope. Programs over the runtime fragment with trivial evidence, run in the check-IR interpreter, the erased interpreter, and as compiled Rust, two hundred to a rustc call. Loops are generated bounded by construction (a counter with a literal limit, never assigned in the body), because interpreter fuel does not bound the compiled program; the process timeout of E4 is a safeguard and its firing is inconclusive. The fast suite runs a few hundred programs with a fixed seed; the extended suite runs thousands with fresh seeds. E6, M2, M3, and O3 each extend the generator as part of their own exit.

Tests.
- Three-way agreement on every generated program, panics included once E4 lands, and in both overflow configurations once E6 lands.
- A failing program is printed with its seed and shrunk by deleting statements.
- An inconclusive run is counted and printed, and the extended suite fails if more than a handful are.

Done when. Differences between what is checked and what runs are found by machine, not by luck.

<a id="LOC-187"></a>
## LOC-187 · R2 · Kernel soundness under mutation: no proof is accepted for a claim known to be false
<!-- task: {"id": "t303", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T23:42:04.000Z"} -->

Landed 21 September as tests/kernel_soundness.rs. Triples come from 67 hand-built proofs covering every rule and axiom family (Int and division included), the 13 theory lemmas, and every proof the elaborator finds for the corpus (recorded through a new HoleReport::found). The oracle is an evaluator of its own, three-valued, exhaustive over bool and u8 and sampling over Nat and Int; a claim is false only when a witness satisfies every hypothesis. Mutations are generic over axioms through Axiom::terms(). Fast: about 17,000 mutants in 3 s; extended: 1.7 million in 78 s, no finding. Sabotage: Evaluate deciding <= as < gives 67 findings; an axiom stated wrongly is caught at setup by its own positive triple. K4 and later kernel commits must extend the exhaustive matches here, which the compiler enforces.

Lane: Robustness. Wave 2.
Depends on: H1 ([LOC-151](core-build.md#LOC-151)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).

Scope. Accepting a changed proof of a true claim is harmless, and a changed proof may validly prove some other proposition, so neither is a finding. What would be a finding is a proof accepted for a false claim. For every proof the corpus produces, the test makes claims known to be false (the negation of the original where that is closed or bounded, and perturbed claims refuted by evaluation over a small domain) and checks them against the original proof and against random changes of one node of it: another hypothesis, another axiom argument, a swapped subproof.

Tests.
- The kernel rejects every pairing of a proof with a claim known to be false.
- A changed proof checked against the original claim may be accepted; the count is printed, for interest only.
- Runs over every proof in the corpus, so it grows with the language.
- Covers the Int forms of K2 (rebased after K2 landed).

Done when. A standing check on the one component everything trusts.

<a id="LOC-188"></a>
## LOC-188 · R3 · Golden diagnostics, and a test that every error code is exercised
<!-- task: {"id": "t304", "status": "done", "priority": 3, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-21T23:37:20.000Z"} -->

Landed 21 September, with a follow-up for the S1 messages. tests/diagnostics_golden.rs renders every rejected file through the library and compares with NAME.stderr; a second test checks the binary prints the same; LOCUS_BLESS=1 rewrites. A meta test scans src for L-codes and fails if one has no rejected file: 50 codes before S1, all covered, 105 files. Thirteen diagnostics judged misleading are listed in the task and should be fixed as reviewed golden diffs (see the list below).

Misleading diagnostics found while writing the goldens, to fix later as golden diffs:
1. L0299 (the checker rejected this) surfaces kernel messages: an if with evidence branches points at the function name; let next = continue(...) says break and continue may only end a block; [h == g] on two proofs, evidence returned from an enum payload, fold toward @q(1) leaking a VarId, and g(1).1 on a dependent tuple all reach L0299.
2. unexpected_character.lc: the parser's L0102 is printed before the lexer's L0001 that caused it.
3. evidence_as_proposition_parameter.lc: a prop parameter naming an earlier parameter gives L0204 unknown name rather than L0225.
4. false_in_one_case.lc: after the arm's equation the claim computes to 7 <= 6 and the note says nothing known speaks of it, although n == 7 is known; no counterexample note for a closed false claim.
5. reversed_range.lc: nothing says the obligation comes from the range 5..n.
6. L0216/L0220 span a whole block instead of the offending statement; L0215 spans a whole loop.
7. L0208 labels only the path, not the wrong pattern list; L0228 points at the argument, not the call.
8. L0206 (_ asks for evidence) says nothing about what to write instead.
9. The nesting and chain limit errors could name the limit.
10. unknown_enum.lc gives unknown name for Light::Red where unknown enum would match the neighbouring wording.

Lane: Robustness. Wave 2.
Depends on: H1 ([LOC-151](core-build.md#LOC-151)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).

Scope. The rendered diagnostic of every rejected corpus file is stored and compared. A meta test lists the error codes in the source and fails if one has no rejected file.

Tests.
- LOCUS_BLESS=1 rewrites the goldens; the diff is reviewed like code.

Done when. Diagnostics are the product for an author, human or AI, and change only on purpose.

<a id="LOC-189"></a>
## LOC-189 · R4 · Acceptance: the target examples run, the legacy is gone, and Now is rewritten
<!-- task: {"id": "t305", "status": "done", "priority": 2, "created": "2026-09-21T22:14:21.000Z", "updated": "2026-09-22T23:24:34.000Z"} -->

Landed 22 September. tests/acceptance.rs holds the twelve exit criteria; tools/check.sh --extended runs them and prints one line each: eleven met (three target files agree three ways in both builds; the boundary held by rustc; counterexamples in every failing diagnostic; eight files check --locked with no search; determinism; never more permissive than rustc; 10,000 random programs and 82,159 cases agreed; 5.4 million kernel mutants without a finding; parser total; the legacy gone) and one not: the fast suite took 161 s in debug against the 120 s bound, reported by check.sh rather than failed. Removed: the bracket, def, math fn, and state-passing loop spellings and their migrations; Nat and its axioms, induction, and lemmas from the kernel; the proof by 256 cases. Kept: kernel::Natural as the magnitude of Integer; the kernel for term, a removal candidate. Language, as built, Overview, README, and the Kernel contract rewritten from the implementation; 63 status rows moved; finding 7 added to Target examples.

Lane: Robustness. Wave 12.
Depends on: E11 ([LOC-192](core-build.md#LOC-192)), E3 ([LOC-169](core-build.md#LOC-169)), E7 ([LOC-173](core-build.md#LOC-173)), E8 ([LOC-174](core-build.md#LOC-174)), E9 ([LOC-175](core-build.md#LOC-175)), E10 ([LOC-190](core-build.md#LOC-190)), M5 ([LOC-181](core-build.md#LOC-181)), O4 ([LOC-185](core-build.md#LOC-185)), K8 ([LOC-166](core-build.md#LOC-166)), R1 ([LOC-186](core-build.md#LOC-186)), R2 ([LOC-187](core-build.md#LOC-187)), R3 ([LOC-188](core-build.md#LOC-188)).

Scope. The closing commit. Language, as built is rewritten from the implementation, Overview and README show the 32-bit lock, and the Language status table moves every core row to built.

Tests.
- The overall exit criteria of the Build plan, each as a test or a command, under tools/check.sh --extended.

Done when. The core language of the Vision is the language of the code.

<a id="LOC-190"></a>
## LOC-190 · E10 · The forms that panic: panic!, assert!, unreachable!, todo!, debug_assert!
<!-- task: {"id": "t306", "status": "done", "priority": 2, "created": "2026-09-21T22:27:28.000Z", "updated": "2026-09-22T20:14:12.000Z"} -->

Landed 22 September. panic!, todo!, unreachable! are never-typed and lower to Tail::Panic (or a two-arm match that panics, in statement position); assert! lowers to a match whose failing arm panics and whose passing arm yields evidence of the condition, so the condition is a fact afterwards; debug_assert! teaches nothing. Under no_panic: panic! and todo! are refused (L0239), assert! must prove its condition and unreachable! must prove False through the usual tiers; without the promise the proofs are tried and attached when found. Messages are Rust's and the harness compares them exactly three ways, so the printer passes the message explicitly. Noted for M5: statements after a never-typed statement still hit L0216. debug_assert! is checked in both builds because the harness's two rustc builds both have debug_assertions on.

Lane: Elaborator. Wave 7.
Depends on: M1 ([LOC-177](core-build.md#LOC-177)), E2 ([LOC-168](core-build.md#LOC-168)), S2 ([LOC-155](core-build.md#LOC-155)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).
Covers: [LOC-71](generics.md#LOC-71), [LOC-85](core-build.md#LOC-85), [LOC-95](core-build.md#LOC-95).

Scope. The forms with their Rust meaning and string literal messages, lowered to the panic ending of M1. Their type is the never type where Rust's is. Under no_panic, assert!(c) needs evidence of c and unreachable!() needs evidence of false, as Target language states; panic! and todo! are rejected there.

Tests.
- Each form three ways, message included.
- After assert!(c), c is a fact; under no_panic without evidence it is an error at the assert.
- Check IR built by hand with a panic ending and no evidence of false is rejected under no_panic by the checker.

Done when. A half-written file checks with todo!(), and a runtime check can teach the logic a fact.

<a id="LOC-191"></a>
## LOC-191 · Decide the rule for a protected value behind &mut when a panic unwinds past a broken invariant
<!-- task: {"id": "t307", "status": "done", "priority": 3, "created": "2026-09-21T22:27:28.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

The conservative protected-update policy is implemented and tested by [LOC-185](core-build.md#LOC-185), [LOC-231](reconciliation.md#LOC-231) and tests/acceptance.rs. The historical field-by-field temporary-invariant proposal remains a possible later refinement, not permission already granted by the current checker.

Historical decision record:

Refined the same day while writing the design of M0: in the core the scenario cannot arise, because a field that evidence depends on is never assigned alone and a whole-value write evaluates its operands first. The no_panic rule is kept for the day field-by-field update is allowed. The Rust caller test stays in O4.

Decided 21 September: the conservative rule below. Target language: Effects are promises.

Raised by review of the Build plan, 21 September; Target language mentions the question under Effects and leaves it to protected exports. A method takes &mut self on a type whose fields carry evidence, assigns a field, which invalidates the evidence, and panics before refreshing it. The checker looks only at the return path, so it is satisfied. A Rust caller that wrapped the call in catch_unwind now holds a value whose invariant is false, in safe Rust. A value owned by the function that panics is not a problem: it is dropped unseen, since Locus types have no Drop.

Recommended, conservative: between an assignment that invalidates evidence stored in a place reached through a &mut parameter and the refresh of that evidence, nothing may panic; to begin with this is enforced by requiring #[no_panic] on any function that makes such an assignment. Replacing the whole value at once, *self = Percent { ... }, invalidates nothing and needs no promise. A finer rule, which looks only at the window between the assignment and the refresh, can replace it later without breaking any program.

Needed before O3 ([LOC-184](core-build.md#LOC-184)). The Rust caller test is in O4 ([LOC-185](core-build.md#LOC-185)), and the design case is the eighth of M0.

<a id="LOC-192"></a>
## LOC-192 · E11 · Found proofs are stored beside the source, and locus check --locked never searches
<!-- task: {"id": "t308", "status": "done", "priority": 2, "created": "2026-09-21T22:41:03.000Z", "updated": "2026-09-22T20:37:36.000Z"} -->

Historical milestone. Version-1 per-source sidecars were replaced and migrated by [LOC-232](process.md#LOC-232); current storage is directory Locus.lock files.

Landed 22 September. <source>.proofs beside each file, version 1: one entry per obligation keyed by an FNV-1a hash of the context (variables with types, ghostness, hypotheses) and the claim in a text form of kernel terms; definitions by name, context entries by position, rebound on read; every rule and axiom round-trips; a stored proof is parsed by a total, depth-limited reader and kernel-checked, never trusted. locus check reads, searches on a miss, writes only on change; --locked never searches and names the missing obligation; --no-store, LOCUS_PROOFS=off, and LOCUS_SEARCH=none (the upgrade hook) exist. The five examples' files are committed, 1.8 KB in all; reformatting, renaming, and editing another function keep every entry in use; hostile files are always a miss.

Lane: Elaborator. Wave 9.
Depends on: E7 ([LOC-173](core-build.md#LOC-173)).
Unblocks: R4 ([LOC-189](core-build.md#LOC-189)).
Covers: [LOC-8](core-build.md#LOC-8).

Scope. Target language: Found proofs are stored. A text format for kernel proofs and certificates with a version, names for definitions, and one entry per obligation keyed by a hash of the claim and its available facts as kernel terms. A file beside each source file. locus check reads, searches on a miss, writes, and drops entries nothing asked for; locus check --locked never searches.

Tests.
- Round trip: every proof the corpus produces is written, read back, and accepted; the file is byte-identical on a second run.
- Reformatting a source file, adding comments, and editing an unrelated function leave every entry in use, counted by a test.
- The file as hostile input: entries swapped between obligations, a proof edited by hand, a truncated file, random bytes. Each is a miss or a reported error, never an accepted false claim and never a panic; the reader is added to the fuzz tests.
- --locked with a missing entry fails and names the obligation; with a complete file it runs no search, asserted by a counter.
- A file written with the search deliberately weakened by a test hook still checks, which is what surviving an upgrade means.

Done when. A reviewer or CI checks a crate with the kernel alone, and a change to the automation cannot break a build that has its proofs.

<a id="LOC-193"></a>
## LOC-193 · Decide how a fully promised function whose body is not a kernel term enters the logic
<!-- task: {"id": "t309", "status": "done", "priority": 3, "created": "2026-09-22T17:42:38.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Implemented by [LOC-210](reconciliation.md#LOC-210); the interim logical-by-promises rule described below has been removed. Ordinary fn never becomes a logical definition.

Historical decision record:

Resolved 22 September by the Vision: an ordinary fn never enters the logic, whatever it promises; a logic fn does and is checked pure and total; the interim rule stays in the code until the reconciliation replaces it (Build plan, After the core build).

Found by E6 on 22 September. A function promising terminates, no_panic, and no_io is lowered as a kernel function with a defining equation, which is what lets it appear in propositions and be unfolded. An operator that may panic is a statement of the check IR, not a term, so such a function cannot contain one (L0236), yet the target example midpoint is fully promised and uses - / +. Functions in propositions already says a function is known by its contract, and its equation is visible only where its body is expressible as a kernel term; Termination says a recursive function enters the logic as a function symbol known by its contract alone. What is missing is the kernel form for that: a declared function symbol with a signature and no body, or the same through a top-level variable of function type. Until decided, E7 checks such a function as an ordinary ExecFn with its promises and refuses its use in a proposition with a message that says why. To discuss with the vision edits.
