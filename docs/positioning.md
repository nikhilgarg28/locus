# Positioning

Locus is a tool and a language, not a company. This document records what it is for, why it has the shape it has, and what it gives up. Design decisions are measured against it.

## In one paragraph

Locus is a Rust-like language in which propositions and explicit, kernel-checked proofs are ordinary parts of a program. Because the source language knows about ownership, mutability, and layout, the proofs are about the real systems code, and that code is emitted as plain, readable Rust with no runtime. Because the target is Rust, guarantees survive into the caller's code: safe Rust cannot construct a value that breaks a proven invariant. Proofs are explicit rather than found by a solver, so checking is deterministic and the trusted base is small. It is for the part of a crate that must be right, written by a Rust programmer, or by an AI against a header that a person reviews.

## The reasoning

1. **The generated code must be efficient and safe.** A prover such as Lean can state and prove facts about machine integers and structures, and its proofs are kernel-checked. Its compiled code, however, runs on a runtime with reference-counted heap objects and boxing, and it reaches other languages only through a C interface.
2. **Efficient code needs a source language that knows about ownership, mutability, and layout.** Avoiding boxing, heap allocation, and reference counting requires knowing statically what is uniquely owned and how data is laid out. A pure functional language with free sharing and types that depend on values cannot add that afterwards. A Rust-like language has it from the start.
3. **Proofs and program belong in one language.** The alternative keeps them apart: write Rust, translate it into a prover, and write specifications and proofs there, as Aeneas does with Lean. That verifies existing code and has the full power of the prover. It also means two artifacts kept in step by a tool, specifications stated against a generated model, and proofs that break far from the edit that broke them, in a language a Rust programmer does not read. Locus puts the specification in the function's type and the proof in the function's body.
4. **Proofs are explicit and checked by a small kernel.** A solver saves human effort, and what it finds can change between versions and time out. An explicit proof succeeds deterministically, is not limited by what a search happens to find, and leaves a small trusted base. Automation still has a place when it produces something the kernel checks: a hole's bounded search does, and so must any later decision procedure. Writing out intermediate facts is tedious for a person and cheap for an AI, which is what makes this side of the choice affordable now.
5. **The target is Rust.** Rust's type system lets a guarantee cross the boundary: private fields mean only generated code can build a validated value, shared references cannot mutate it, there is no null and no uninitialized value, and enums are exhaustive. A caller in C, or across a C interface, can forge anything. The claim is that safe Rust cannot break a proven invariant; `unsafe` in the caller can.

Hence one language with a Rust-like type system, propositions, and explicit proofs, lowered to Rust.

## What it is not, and what it gives up

- **It does not verify existing Rust.** Keeping proofs and program together means only code written in Locus is verified. Verifying code that already exists is what Aeneas, Verus, Creusot, and Kani are for.
- **It is not as powerful as Lean.** The logic is deliberately small: dependency only inside propositions, no tactic language, no mathematics library. Where the difficulty is mathematics rather than bounds, states, and invariants, a prover is the right tool. Locus emits plain safe Rust, so taking a generated core into such a tool later remains possible.
- **It is not a superset of Rust, and may never be one in full.** The aspiration is a growing subset of safe Rust, plus propositions and proofs, in which anything Locus and Rust both have means what it means in Rust. `unsafe`, interior mutability, and concurrency stay behind a stated trusted boundary. Giving logical meaning to all of Rust, which has no finished formal semantics, would make "kernel-checked" relative to a large trusted model.

Its closest relative is Low\* with KaRaMeL, a restricted language inside F\* that compiles to readable C with no runtime, more than it is Lean.

## The assumption everything rests on

A Rust-like language with ownership and mutation, a logic over immutable values, and explicit proofs, all at a proof burden people tolerate. The functional reading of `&mut` is known to work. That it stays pleasant without a solver absorbing the obligations it generates is not known, and Locus today has no mutation at all. The experiment that tests the thesis is `let mut` and `&mut` under that reading; the Aeneas papers are the reference for it. Close behind is the expressiveness of specifications: mathematical integers, sequences and maps, ghost models, and opaque types, without which a specification can say little of interest.

## How it is delivered

- **A verified core inside a Rust crate.** `.loc` files in a crate, compiled by a cargo command or `build.rs` into an ordinary module. The output has no runtime and no dependency, so the users of the crate need nothing.
- **A macro as the way in.** `locus! { ... }` for a first proven function with one dependency added. It is a second driver over the same library, which is why Locus source tokenizes as Rust. It is not the main route: a macro sees only its own tokens, pushes proof checking into every downstream build, and can report little.
- **Rust only.** The erased tree is target-neutral and stays so, and nothing beyond Rust is promised, because the boundary guarantee depends on the host language.
- **Rust's modules when modules are needed.** `mod`, `use`, and `pub` exactly as in Rust. `pub` and field privacy arrive earlier, with interoperation, because privacy is what protects a validated type.

## The use case that motivates the next steps

A person writes a header: types, declared propositions, math functions with their bodies, and the signatures of the public functions, which carry their invariants. An AI writes the implementation and its proofs in another file. People review only the header.

For that to be sound:

- the header elaborates on its own, so that every word a claim uses is defined where it is reviewed;
- the checker requires each implementation to match its signature exactly;
- termination is visible in the header, since a function that never returns satisfies any result type, and `loop` is the only source of divergence;
- `Proved` cannot be constructed outside generated code, and an exported function never takes evidence from Rust: it has no proof precondition, takes a validated type, or is a wrapper that checks at runtime;
- an audit command lists everything trusted: foreign declarations, classical reasoning, divergence, traps.

For it to be pleasant: diagnostics as data, with the claim, the facts, the gap, and the failing case; a short reference and example corpus written for a model that has never seen the language; a cache of found proofs; and decision procedures that produce certificates once integers are wider than a byte, where evaluating every case stops being possible.
