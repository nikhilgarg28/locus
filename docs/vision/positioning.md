+++
id = "positioning"
title = "Positioning"
group = "Vision"
created = "2026-09-21T21:03:06.000Z"
updated = "2026-09-22T23:44:20.000Z"
route = "vision/positioning.html"
order = 4
+++

# Positioning

Locus is a tool and a language, not a company. This document records what it is for, why it has the shape it has, and what it gives up. Design decisions are measured against it.

## In one line

Review the guarantees, check the implementation, and use the result from ordinary Rust.

## In one paragraph

Locus is a Rust-like language for the parts of a crate whose correctness matters most. Propositions and proofs compose with ordinary functions and data, and explicit proof evidence is checked, deterministically, by a small, auditable kernel. Locus is designed to verify systems implementations with precise arithmetic, ownership, and mutation semantics, then emit readable Rust without runtime proof machinery. Generated interfaces protect proven invariants at the boundary with safe Rust callers. A programmer or an AI supplies the implementation and evidence against a human-reviewed specification; proof construction can be automated, while acceptance rests on independent checking.

This is a statement of destination. What is true after the core build of September 2026: the kernel, the checking of evidence, a bounded search that fills holes with a linear arithmetic procedure behind it, stored proofs and a check that never searches, readable generated Rust with a tested boundary that Rust callers cannot cross, every machine integer type, let mut, loops that carry evidence, &mut parameters, moves, and impl blocks. Not yet true: the logical and runtime type split of the Vision, named-arm propositions, logical data beyond Int, generics, and references beyond one call. Each release should say which systems features its guarantees cover.

## The reasoning

1. **The generated code must be efficient and safe.** A prover such as Lean can state and prove facts about machine integers and structures, and its proofs are kernel-checked. Its compiled code, however, runs on a runtime with reference-counted heap objects and boxing, and it reaches other languages only through a C interface.
2. **Efficient code needs a source language that knows about ownership, mutability, and layout.** Avoiding boxing, heap allocation, and reference counting requires knowing statically what is uniquely owned and how data is laid out. A pure functional language with free sharing and types that depend on values cannot add that afterwards. A Rust-like language has it from the start.
3. **Proofs and program belong in one language.** The alternative keeps them apart: write Rust, translate it into a prover, and write specifications and proofs there, as Aeneas does with Lean. That verifies existing code and has the full power of the prover. It also means two artifacts kept in step by a tool, specifications stated against a generated model, and proofs that break far from the edit that broke them, in a language a Rust programmer does not read. Locus puts the specification in the function's type and the proof in the function's body.
4. **Acceptance rests on checking evidence, whoever produced it.** A person, an AI, a bounded search, or a decision procedure may construct a proof. What matters is that the program is accepted because a small kernel checked explicit evidence, and not because a search succeeded. The alternative, trusting a solver's answer, saves effort, and what it finds can change between versions and time out. Checked evidence is accepted deterministically and is not limited by what one search happens to find. "Explicit" describes the evidence, not the effort: a `_` is filled by a search that emits a proof, and any later decision procedure must emit one or a certificate for a small checker. Writing out intermediate facts is tedious for a person and cheap for an AI, which makes this choice more affordable than it was. Deterministic is not the same as inexpensive; what checking costs is measured, not assumed.
5. **The target is Rust, which makes protected interfaces possible.** Rust supplies the mechanisms: private fields, shared references that cannot mutate, no null, no uninitialized value, exhaustive enums. A caller in C, or across a C interface, can forge anything. The mechanisms do not preserve a guarantee by themselves. Locus has to generate, and verify, the abstraction that uses them: private storage, constructors that validate or demand evidence, and operations that keep the invariant. Exposing unrestricted mutable access to a validated value would undo it. The claim is therefore narrow: a generated interface preserves the invariants it exposes to safe callers; `unsafe` in the caller can break them. A guarantee that relates several values, such as "this output is a permutation of that input", is available to Locus callers as evidence and does not become a Rust type; a Rust caller gets it as documentation of a checked fact, not as something the compiler tracks.

Hence one language with a Rust-like type system, propositions, and explicit proofs, lowered to Rust.

## What is trusted

The proof kernel is small and auditable. It is not the whole trusted base. The correspondence between the source, the logical statements checked, and the Rust emitted is trusted too: lowering, the checker for executable code, erasure, the Rust printer, and the Rust toolchain ([IR architecture](architecture.md)). A sound kernel cannot establish that the program checked and the program run agree; differential testing of the two branches addresses that, and an erasure theorem is the longer-term answer. "Proofs about the real code" is likewise a commitment, not a consequence of the syntax: each systems feature, from machine arithmetic to aliasing, panics, and imported operations, needs a precise meaning in the logic that compilation preserves.

"Without runtime proof machinery" means no dedicated runtime and no proof checking when the program runs. It does not mean no checks: validating input that arrives from outside, and the wrapper around an exported function with a precondition, are executable tests.

## What it is not, and what it gives up

- **It does not verify existing Rust.** Keeping proofs and program together means only code written in Locus is verified. Verifying code that already exists is what Aeneas, Verus, Creusot, and Kani are for.
- **It is not as powerful as Lean.** The logic is deliberately small: dependency only inside propositions, no tactic language, no mathematics library. Where the difficulty is mathematics rather than bounds, states, and invariants, a prover is the right tool. Locus emits plain safe Rust, so taking a generated core into such a tool later remains possible.
- **It is not a superset of Rust, and may never be one in full.** The aspiration is a growing subset of safe Rust, plus propositions and proofs, in which anything Locus and Rust both have means what it means in Rust. `unsafe`, interior mutability, and concurrency stay behind a stated trusted boundary. Giving logical meaning to all of Rust, which has no finished formal semantics, would make "kernel-checked" relative to a large trusted model.

- **It is not alone.** Verified systems code, familiarity to Rust programmers, and static verification are what Verus offers too, with far more automation. What Locus wagers on is a combination: proof-bearing interfaces that compose as ordinary values, evidence that is checked independently of whatever produced it, proof construction a Rust programmer finds approachable, and Rust exports that protect what was proven. Those are experiences to demonstrate, not properties to assert.

Its closest relative is Low\* with KaRaMeL, a restricted language inside F\* that compiles to readable C with no runtime, more than it is Lean.

## The assumption everything rests on

A Rust-like language with ownership and mutation, a logic over immutable values, and explicit proofs, all at a proof burden people tolerate. The functional reading of `&mut` is known to work. That it stays pleasant without a solver absorbing the obligations it generates is not known, and Locus today has no mutation at all. The experiment that tests the thesis is `let mut` and `&mut` under that reading; the Aeneas papers are the reference for it. Close behind is the expressiveness of specifications: mathematical integers, sequences and maps, logical models, and opaque types, without which a specification can say little of interest.

## How it is delivered

- **A verified core inside a Rust crate.** `.lc` files in a crate, compiled by a cargo command or `build.rs` into an ordinary module. The output requires no proof runtime or Locus toolchain for downstream users. Logical Int/Nat, collections, propositions, proofs, and models introduce no runtime dependency for reasoning. They lower to a singleton marker where a placeholder is required; default code generation removes unused marker bindings while preserving runtime evaluation.
- **A macro as the way in.** `locus! { ... }` for a first proven function with one dependency added. It is a second driver over the same library, which is why Locus source tokenizes as Rust. It is not the main route: a macro sees only its own tokens, pushes proof checking into every downstream build, and can report little.
- **Rust only.** The erased tree is target-neutral and stays so, and nothing beyond Rust is promised, because the boundary guarantee depends on the host language.
- **Rust's modules when modules are needed.** `mod`, `use`, and `pub` exactly as in Rust. `pub` and field privacy arrive earlier, with interoperation, because privacy is what protects a validated type.

## The use case that motivates the next steps

A person writes a header: types, declared propositions, logic functions with their bodies, and the signatures of the public functions, which carry their invariants. An AI writes the implementation and its proofs in another file, against that fixed specification. People review only the header.

This moves review to a smaller artifact; it does not decide what correctness means. The header becomes the critical document. A sorting contract that says only "the result is sorted" permits returning an empty collection. The specification has to cover the intended behavior and its assumptions, and the AI must be unable to weaken it or to add trusted assumptions while satisfying it. Tooling can help a reviewer trust a header, with executable examples and a warning when a precondition cannot be satisfied, and cannot write it for them.

For that to be sound:

- the header elaborates on its own, so that every word a claim uses is defined where it is reviewed;
- the checker requires each implementation to match its signature exactly;
- termination is visible in the header, since a function that never returns satisfies any result type, and `loop` is the only source of divergence;
- `Erased` has no public constructor, and an exported function never takes evidence from Rust: it has no proof precondition, takes a validated type, or is a wrapper that checks at runtime;
- an audit command lists everything trusted: foreign declarations, classical reasoning, divergence, traps.

For it to be pleasant: diagnostics as data, with the claim, the facts, the gap, and the failing case; a short reference and example corpus written for a model that has never seen the language; a cache of found proofs; and decision procedures that produce certificates once integers are wider than a byte, where evaluating every case stops being possible.
