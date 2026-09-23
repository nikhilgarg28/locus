+++
id = "logical-core"
title = "Logical computation and erasure"
group = "Vision"
created = "2026-09-22T21:24:37.000Z"
updated = "2026-09-22T23:44:20.000Z"
route = "vision/logical-core.html"
order = 5
+++

# Locus: logical computation and erasure

Locked Vision decisions from 22 September 2026, integrated into [Target language](target-language.md). This describes the intended design, not implemented compiler behavior. Syntax for logical callables and quantifier constructors is schematic.

**Types.** Every type is either `Logical` or non-logical (runtime). `Logical` is a compiler-controlled trait: `Int`, `Nat`, `Bool`, `Seq`, `Map`, `Prop`, and all proof types are logical; `u8`, `usize`, and `bool` are runtime types. There is no unchecked implementation. Derivation requires logical fields and payloads; logical containers require logical element types, so `Seq<u8>` is invalid. Runtime structs/tuples may contain logical fields. Runtime `Option<@P>` retains its tag and erases its proof payload. `Box<T>` is always runtime, even for logical `T`; a logical record cannot contain a `Box` field. Logical integers have no executable representation.

**Library data and recursion.** Logical enums can be directly recursive without `Box`; they describe finite inductive values and require checked formation, recursion, and induction rules. `Int`, `Seq`, and finite maps can be library definitions over `Nat` and these general mechanisms; even `Nat` can be inductively defined. Native arithmetic support may remain with a specified sound correspondence. Runtime boxed structures obtain separate logical models. Examples and bootstrap requirements are in [Target language](target-language.md).

**Functions and blocks.** Ordinary `fn` executes and may accept/return runtime data, logical values, or mixtures. `logic fn` is erased, returns a logical type, and is checked pure and total: no runtime effects, panics, invalid operations, or unchecked divergence. These requirements are implicit; recursion/loops need termination justification. There is no `math fn` or `logic let`; bindings get their classification from their types. Generic substitution never changes function mode. A logical function returning `T` requires `T: Logical`; generic comparison needs the appropriate runtime or logical comparison interface. Runtime generic functions may transport logical arguments without inspecting their contents.

**Erasure safety.** Runtime `if` requires `bool`; runtime `match` cannot discriminate on a logical value. Logical `if` requires `Bool`; logical matching may examine logical values or permitted observations of runtime data. Logical fields, constructors, and primitive operations preserve logical types. Logical computation cannot export runtime results, modify runtime storage, consume runtime ownership, trigger destructors, call ordinary functions, or transfer control into/out of an enclosing runtime computation. Runtime address/reflection operations cannot expose logical contents. Logical branch selection therefore cannot determine executable behavior.

**Calls and evaluation order.** Logical calls/operators are allowed directly in ordinary code: `let count = items.len(); let next = count + 1;` creates logical `Nat` values. Resolution identifies the logical callee/operator; the result type alone never authorizes erasing a call. Arguments and receivers are checked in their surrounding context. In runtime code, preserve their required runtime evaluation in source order before erasing the logical application:

```rust
let q = derive(mutate_and_prove(&mut x));
// Normalize, then erase logical computation and values:
let temporary: Erased = mutate_and_prove(&mut x);
let q: Erased = Erased;
```

An explicit `logic { ... }` makes its entire contents logical, so the nested ordinary call would be rejected there. Never hoist effects across logical branches, loops, or explicit logical blocks. Preserve temporary lifetimes and executable divergence, panic, and drop behavior.

**Models and snapshots.** A logical destination `M` may implement `Model<T>` via `logic fn model(source: &T) -> Self`. Multiple models are permitted, with an unambiguous implementation per `(T, M)`. `x as M` selects that implementation and observes current state; it performs no runtime move, copy, allocation, or conversion. Access is checked like a short shared borrow, ending after observation. With an active mutable borrow, observe through the authorized reference, not the unavailable original binding. The result contains no live runtime references and survives subsequent mutation or destruction. Typed elaboration records binding and heap-state versions; unchanged pointers can observe changed contents. Models define interpretations; operation correctness still requires proofs relating before/after models. Interior mutation and concurrency require dedicated observation rules.

**Propositions and captures.** `prop!(P)` and `prove!(P)` take a proposition expression in which a logical `Bool` is lifted through `holds: Bool -> Prop` and a `Prop` stands as it is. It is not a general quantifier syntax. A machine value in a logical context is observed through its default model, `Int` for every machine integer type and `Bool` for `bool`, so for runtime `x: u8`, `prop!(x > 3)` means `prop!((x as Int) > 3)`; `as M` picks another model. `prop` and `logic` are reserved keywords. The compiler retains a logical term over immutable captures, including memory-state versions; macros need not manipulate SSA identifiers. Stored/returned propositions retain these logical captures, not source-variable lifetimes. Constructing a claim establishes nothing; `@P` requires kernel-checked evidence for that specific proposition. Changed runtime state requires evidence about the new observation.

**Quantifiers through existing machinery.** Predicates are logical functions/closures `T -> Prop`. `Exists(P)` has a witness constructor requiring `value: T` and `@P(value)`. `ForAll(P)` has a constructor carrying `logic Fn(x: T) -> @P(x)`; proof elimination applies it to establish `P(x)`. Matching an existential may use its witness to prove another claim, but cannot extract arbitrary logical data from an irrelevant proof. More general projections must satisfy the kernel's elimination rules. These can be library propositions when the kernel supports inductive propositions and dependent proof-returning functions; dedicated quantifier keywords/nodes are unnecessary. The alternative `Not(Exists(x => Not(P(x))))` generally needs classical reasoning to recover `ForAll(P)`; decidable predicates also suffice.

**Lowering contract.** Verify logical terms and proof dependencies before erasure. Replace every logical type/value with one singleton `Erased` marker; erase logical computation while preserving runtime evaluation and aggregate tags/fields. Ordinary calls returning only proofs still execute. Preserve resolved generic/trait dispatch when collapsing types. The marker grants no authority: Rust-facing interfaces must prevent fabricated or unrelated markers from bypassing verified invariants. Optimizations may remove marker bookkeeping; semantic correctness must not depend on those optimizations.

**Default Rust cleanup.** After erasure, remove unused Erased locals and dead marker assignments, preserving runtime evaluation of their initializers in source order (for example `let _ = mutate_and_prove(&mut x);`). Use wildcards/underscore names for unused retained pattern bindings/parameters without changing signatures, layout, lifetimes, or drops. Avoid marker-induced rustc warnings by default in every build; no build flag or blanket warning suppression is required. A pre-cleanup debug dump can be considered separately.
