+++
summary = "Broader borrowing, runtime collections and control flow built on the verified ownership core."
id = "p37"
name = "Ownership, runtime data and control flow"
status = "planned"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/memory-layout.html"
order = 4
kind = "project"
+++

# Ownership, runtime data and control flow

Build on accepted arrays/slices/Vec, physical Box and stored shared-reference provenance. The next ownership tier is broader mutable borrowing ([LOC-33](memory-layout.md#LOC-33)); Drop/interior mutability/shared ownership require explicit semantics before async or unsafe expansion. Iterator loops depend on general traits and runtime callables; their syntax must not be mistaken for the already-supported range loops. Runtime collection APIs ([LOC-55](memory-layout.md#LOC-55)) should be driven by checked models and real examples, not new primitive assumptions by default.

<a id="LOC-29"></a>
## LOC-29 · if let and related conditional destructuring
<!-- task: {"id": "t35", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: syntax and checked lowering for if let, including branch facts, binding scopes, moves and proof payloads. Ordinary if/else and match already exist. Coordinate let-else with [LOC-69](generics.md#LOC-69) rather than duplicating pattern semantics.

<a id="LOC-30"></a>
## LOC-30 · Iterator-based for loops
<!-- task: {"id": "t36", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Half-open/inclusive integer-range loops are implemented ([LOC-179](core-build.md#LOC-179)). Remaining: IntoIterator/Iterator semantics, next, owned/borrowed iteration and proof contracts. Depends on general traits ([LOC-21](generics.md#LOC-21)), runtime closures as needed ([LOC-24](generics.md#LOC-24)), and mutable borrowing ([LOC-33](memory-layout.md#LOC-33)).

<a id="LOC-31"></a>
## LOC-31 · Arc and shared heap ownership beyond Box
<!-- task: {"id": "t38", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Physical Box allocation/dereference and recursive boxed enums are delivered (tests/reconcile_box.rs; [LOC-231](reconciliation.md#LOC-231)). Remaining: Arc and any Rc policy, reference counts, cloning, aliasing and destruction. Requires explicit ownership/Drop assumptions, not just another model value; coordinate [LOC-47](memory-layout.md#LOC-47), [LOC-48](memory-layout.md#LOC-48).

<a id="LOC-33"></a>
## LOC-33 · Borrowing beyond the shared-reference tier
<!-- task: {"id": "t40", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered: call-duration &mut parameters ([LOC-184](core-build.md#LOC-184)) and shared references in locals/fields/results with lifetime provenance, last-use checks and erased observation permissions ([LOC-230](reconciliation.md#LOC-230); tests/reconcile_shared.rs). Remaining: stored mutable references, general reborrowing/lifetime relationships and deeper aliasing shapes. State each supported tier and validate against rustc before expanding it.

<a id="LOC-35"></a>
## LOC-35 · Runtime strings and str views
<!-- task: {"id": "t42", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Message literals are supported ([LOC-95](core-build.md#LOC-95)); String/str as ordinary values are not. Remaining: byte/Unicode model, allocation, indexing/bounds and borrowed view rules. Build on collections and reference provenance rather than treating every string operation as logical.

<a id="LOC-36"></a>
## LOC-36 · Static storage and initialization
<!-- task: {"id": "t43", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Const items are delivered ([LOC-101](core-build.md#LOC-101), [LOC-175](core-build.md#LOC-175)). Remaining: immutable/mutable static storage, initialization order, lifetimes, unsafe access where needed and logical observation validity. Coordinate interior mutability ([LOC-47](memory-layout.md#LOC-47)) and unsafe rules ([LOC-46](memory-layout.md#LOC-46)).

<a id="LOC-45"></a>
## LOC-45 · Async and suspension-aware proofs
<!-- task: {"id": "t55", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Explicitly deferred. Requires a design for owned/borrowed state across suspension, pinning, cancellation/destruction, Send/Sync and observations. Depends on runtime closures/traits, broader borrowing and Drop; no current logic fn or proof feature implies async support.

<a id="LOC-46"></a>
## LOC-46 · Unsafe code and explicit proof obligations
<!-- task: {"id": "t56", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Explicitly deferred beyond the current trusted-declaration boundary. Design pointer provenance, aliasing, initialization and the contracts that make unsafe operations reviewable. Keep source safety obligations distinct from accepting an unproved proposition; relate every trusted site to [LOC-196](process.md#LOC-196) audit.

<a id="LOC-47"></a>
## LOC-47 · Interior mutability and observation validity
<!-- task: {"id": "t57", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Explicitly deferred. Define Cell/RefCell/atomic or shared-mutable models and when a logical observation remains valid despite aliases. Existing SSA snapshots/shared-reference checks do not justify hidden writes. Cover native and Locus trait calls, observer stability, stored proofs, shared aliases and facts surviving a call. A shared receiver is not evidence of purity. Acceptance includes hostile implementations that mutate through shared references; old immutable snapshots remain historical, while claims about current storage must be invalidated or re-established. Depends on an aliasing/concurrency model and interacts with [LOC-31](memory-layout.md#LOC-31), [LOC-48](memory-layout.md#LOC-48).

<a id="LOC-48"></a>
## LOC-48 · Drop and unwinding-safe resource invariants
<!-- task: {"id": "t58", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Explicitly deferred. Define when destructors run, effectful destruction in erased positions, panic behavior and invariant preservation across partially executed updates. Current erasure preserves ordinary effects and owned temporaries but does not implement user Drop. Required before general shared ownership and async.

<a id="LOC-49"></a>
## LOC-49 · Checked termination of loops
<!-- task: {"id": "t59", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: a decreasing measure or bounded-iteration rule for loops, including all continue/break paths and mutation. Logical structural/Int-measured recursion is complete ([LOC-221](reconciliation.md#LOC-221)), but ordinary while/for syntax does not carry a termination proof. [LOC-144](core-build.md#LOC-144) is consolidated here; finite-range loops are an important first case.

<a id="LOC-50"></a>
## LOC-50 · Dynamically sized types beyond slice parameters
<!-- task: {"id": "t60", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Supported slices are a bounded view tier ([LOC-228](reconciliation.md#LOC-228), [LOC-230](reconciliation.md#LOC-230)). Remaining: general DST layout/metadata, unsized fields and coercions, with safe borrowing and Rust emission. Coordinate str ([LOC-35](memory-layout.md#LOC-35)) and dyn ([LOC-34](generics.md#LOC-34)), without equating their representations.

<a id="LOC-55"></a>
## LOC-55 · A broader verified runtime collection library
<!-- task: {"id": "t68", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Vec/array/slice primitives, Box and the generic verified collection example are delivered ([LOC-228](reconciliation.md#LOC-228)–231). Remaining: reusable maps, sets, deques and fuller Vec operations with end-to-end model/invariant proofs. Logical finite maps alone do not supply a runtime KV store. Depends on the logical laws in [LOC-26](proof-automation.md#LOC-26), [LOC-52](proof-automation.md#LOC-52) and relevant ownership/trait support.

<a id="LOC-86"></a>
## LOC-86 · match guards
<!-- task: {"id": "t179", "status": "todo", "priority": 0, "created": "2026-09-21T19:54:30.730Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: guard evaluation, branch facts, move/borrow behavior and exhaustiveness interaction. The AST currently has pattern/body arms without guards. Shares the richer-pattern foundation with [LOC-102](memory-layout.md#LOC-102).

<a id="LOC-102"></a>
## LOC-102 · Richer match patterns and exhaustiveness
<!-- task: {"id": "t196", "status": "backlog", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered: tuple/struct/enum destructuring, boolean/literal parsing, whole-value @ and proposition evidence patterns. Remaining: the full Rust subset originally requested, notably range/or-patterns, broader integer exhaustiveness and interactions with guards. Parser recognition alone is not semantic acceptance. [LOC-70](core-build.md#LOC-70) is consolidated here; guards stay [LOC-86](memory-layout.md#LOC-86).

<a id="LOC-103"></a>
## LOC-103 · Target-sized usize and isize
<!-- task: {"id": "t197", "status": "done", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-25T17:07:25.165396+00:00"} -->

Delivered: explicit 32/64-bit target identities, models, checked arithmetic, casts, proof-store isolation and generated-Rust width guards. Collection lengths and indices now use usize. `tests/platform_integers.rs` covers both layouts, Cargo configuration, receipts, independent arithmetic oracles and emitted Rust. The complete extended gate passed; see the [implementation and validation record](../plans/platform-types-and-structs.md).

<a id="LOC-107"></a>
## LOC-107 · Loop labels and labeled break/continue
<!-- task: {"id": "t201", "status": "backlog", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining: syntax, label resolution and checked state/evidence transfer to the selected loop, including early-return and shared-reference lifetimes. Current unlabeled loops are complete.

<a id="LOC-109"></a>
## LOC-109 · Range values outside for headers
<!-- task: {"id": "t203", "status": "backlog", "priority": 0, "created": "2026-09-21T20:29:22.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Half-open and inclusive ranges in for headers are implemented. Remaining: first-class range expressions/types and use outside headers; the parser currently reports inclusive ranges unsupported there. Iterator integration is [LOC-30](memory-layout.md#LOC-30).
