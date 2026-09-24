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

Delivered: concrete type specialization, inference and Logical bounds ([LOC-218](reconciliation.md#LOC-218), [LOC-219](reconciliation.md#LOC-219); tests/reconcile_generics.rs). Remaining: general where clauses, trait-bound checking, generic impl blocks and a stated policy for checking generic bodies. Current unused templates are not universal proofs. Type arguments must also be closed over local values: `Option<@True>` works, but `Option<@(n > 0)>` for a local `n` is rejected (tests/reconcile_generics.rs). Dependent aggregate fields are the current alternative.

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
