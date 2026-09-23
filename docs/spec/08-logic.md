+++
id = "language-logic"
title = "Logical computation"
group = "Now"
spec_chapter = 1
order = 107
route = "specification/logic.html"
description = "Logic functions and blocks, total recursion, and the boundary with runtime execution."
+++

# Logical computation

<!-- spec: 1.0:9 informative -->
Logical computation builds the values used to state and prove properties. Its result is erased from the generated program, so it has stricter rules than ordinary execution. Here the distinction is made explicit through logical types, functions, closures and checked recursion.

## Expressions and computation modes

<!-- spec: 1.5:1 dynamic-semantics -->
Runtime expressions use Rust-like evaluation order: operands and arguments are evaluated left to right, while assignment evaluates its right side before its destination. Effects of ordinary calls are retained even when their returned value is Logical.

<!-- spec: 1.5:2 dynamic-semantics -->
`fn` declares ordinary execution. Its body may mutate, panic or diverge subject to its stated promises. It can accept or return data, propositions and evidence; the logical values occupy erased positions. A result's type does not make the call logical. An ordinary function cannot be unfolded or called inside a logical computation, even if it promises every effect restriction.

<!-- spec: 1.5:3 dynamic-semantics -->
`logic fn` declares checked logical computation. Its result must be Logical; it cannot perform physical mutation, allocate physical objects, panic, or call ordinary functions. It can inspect authorized immutable snapshots of runtime inputs and call other logical definitions. `logic { ... }`, `prop!(...)`, proof annotations and `prove!(...)` establish logical contexts. User-defined total recursion requires the logical-data rules in [Logical data](#logical-data). The four runtime promises do not infer logical mode.

<!-- spec: 1.5:4 dynamic-semantics -->
Ordinary code may call a logical function or compute with Logical operands without first wrapping the expression in `logic { ... }`. These logical operations disappear, but any ordinary argument-producing calls run once in source order. Inside an explicitly logical block such calls are rejected. Runtime control depends on physical `bool` or a physical enum tag; a logical conditional uses `Bool` and produces only logical computation. A logical value cannot be converted into physical data or used to choose a runtime branch.

<!-- spec: 1.5:5 dynamic-semantics -->
Operator selection follows the operand types. `count + 1` with `count: Int` is erased integer arithmetic; the same syntax at `u8` is a runtime operation with Rust overflow behavior. Logical `Bool` and physical `bool` are different surface types even though the kernel shares a boolean representation. A separately checked erasure layout preserves the distinction inside products, fields, arguments, results and control-flow joins.

## Logical data

<!-- spec: 1.25:1 legality-rule -->
`#[derive(Logical)]` structs and finite recursive enums erase completely. Recursive logical enums require no runtime Box. Mutually referring logical enum declarations form one checked group; generic groups are specialized first. Matching across group members can expose the same-typed structural descendant for a recursive function or proof. Mutually recursive functions remain unsupported. `library/logical.lc` defines Nat and Seq as ordinary enums and proves the Nat/Int correspondence; Int remains the primitive mathematical integer. Library maps, membership and reachability are checked source declarations. Explicit `--library` input includes them without a hidden privileged prelude.

<!-- spec: 1.25:2 legality-rule -->
Structural recursion is checked against constructor subdata; recursive theorem calls become checked induction. Int recursion uses `recurse!(decreases, function(next_args))`, naming the current function, where decreases proves `0 <= next && next < current`. The descent proof is checked before admitting the recursive call, and cannot use that call to justify itself. Arbitrary nonpositive recursive proposition occurrences are rejected. The kernel checks computed arm bodies, recursive declarations, induction and reductions independently of the source elaborator.

<!-- spec: 1.25:3 legality-rule -->
Logical closures are ordinary lambda terms with typed parameters, inferred captures, and dependent proof results. A runtime capture must be explicitly observed through a model, as in `|x: Int| x + (n as Int)`. Closure parameter and result types must be Logical; this is stricter than a named `logic fn`, which may take physical input observations. `logic Fn(x: T) -> U` is callable in ordinary code with erased results; any eager ordinary callee/argument work is retained. It is not a runtime closure type.

## Quantification

<!-- spec: 1.25:4 legality-rule -->
`Exists<T>::Witness(value) @ proof` and `ForAll<T>::Each(prove_each) @ True::Intro` are the library quantifier constructors. Predicate arguments are Logical callables returning Prop. Applying universal evidence specializes its checked proof function. Eliminating an existential proof permits its witness only within further proof construction; it does not create a witness extraction operation.

## Models

<!-- spec: 1.25:5 legality-rule -->
`impl Model<Runtime> for LogicalDestination { logic fn model(source: &Runtime) -> Self { ... } }` defines an observational bridge. The definition is checked, not admitted as an axiom. A model cast records the current value version and requires a live observation permission. Models compose and a source may have multiple distinct destinations; overlapping source/destination implementations are rejected. Physical buffer shapes are retained for model selection even though their logical content snapshots share a kernel representation. [Models and heap data](11-models.md) explains storage, lifetime and native-call behavior.
