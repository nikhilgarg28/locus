# Vision

Locus is a superset of Rust in which a program can state facts about itself and prove them. Its approach to logic and proof is inspired by Lean; its home is Rust and systems programming. Every Rust program is a Locus program and means the same thing. What Locus adds, propositions and proofs and the logical data they are stated over, is checked by a small kernel and then erased, so a proven program becomes ordinary Rust and runs as fast as the Rust it becomes.

> This is the owner's document and the top of the design authority: [RFCs](design/rfcs/README.md) and the [specification](spec/README.md) must be consistent with it. It says what Locus is for and what it commits to. It does not define the language; accepted RFCs and the specification do.

## Why Locus

Rust guarantees memory safety: a program without `unsafe` cannot corrupt memory. It cannot guarantee that the program is right: that a search finds what it should, that a counter never passes its limit, that an index is in bounds before it is used. Tests sample such properties. Proofs establish them.

Proof assistants such as Lean can state and check facts like these, but the programs they produce are not systems programs, and they do not live in a Rust codebase. Locus brings Lean's approach into Rust itself. Facts and their proofs sit next to the code they are about, in the same files, built by the same tools, and they cost nothing when the program runs.

### Programming with AI

Locus is designed for a world in which much of the code is written by AI. Reading AI-written code line by line does not scale, and tests cannot show that bugs are absent. A checked proof can: a reviewer needs to read what was claimed, not how it was established.

Locus's explicit design goal is to make programming with AI safer. The price is the work of writing proofs, a burden people have often been unwilling to carry and one AI carries readily. People decide what must be true; AI does the work of making it so; and the kernel, not the AI, a search, or a solver, decides whether it is.

## The experience

A sketch, with illustrative syntax that RFCs will decide:

```rust
fn find(xs: &[u32], target: u32, sorted: Option<@Sorted(xs)>) -> Option<usize> {
    match sorted {
        Some(proof) => binary_search(xs, target, proof),
        None => linear_search(xs, target),
    }
}
```

`@Sorted(xs)` is the type of a proof that `xs` is sorted. `binary_search` requires one, so running a binary search over unsorted data is a compile-time error rather than a wrong answer. A caller that sorted the slice holds a proof from the sort and passes it; a caller that did not passes `None` and gets the linear search. After checking, the proof disappears: the generated Rust keeps only whether a proof was passed, and reads like code a Rust programmer would write. The crate is published through Cargo, and Rust programs use it like any other crate, through an interface that does not let them break what was proven.

## Commitments

1. **A superset of Rust.** Every valid Rust program is a valid Locus program with the same meaning, including behavior that differs between builds, such as integer overflow. Locus adds only contextual keywords and syntax that is not valid Rust, so it never changes what Rust code means. Being a superset in what Locus accepts comes first; what Locus can reason about grows over time. Code outside that part is still compiled and can still be called, but nothing can be proven about its internals beyond what is assumed about it.

2. **Proofs cost nothing at run time.** Propositions, proofs and logical data are erased after checking, and erasure never changes what the rest of the program does, except that source locations reported at run time, such as in panic messages, name the generated file. Data that carries a proof keeps its runtime parts: an `Option` that may hold a proof still records whether it does. Over time, proofs will also remove runtime checks they make unnecessary, such as a bounds check on an index proven in range or an overflow check on arithmetic proven not to overflow. There will be few such optimizations at first and more as Locus matures.

3. **Proofs are explicit and checked by a kernel.** A program is accepted because a small kernel checked explicit evidence, never because a search or a solver reported success. Search, decision procedures, SMT solvers and AI may all propose proofs; each is a candidate until the kernel accepts it. Acceptance is deterministic: the same program gives the same answer on any machine. Proofs found automatically are stored, so that checking a program never depends on finding them again.

4. **Lean's logic.** Locus's logic follows Lean's: propositions as types, proof irrelevance, and classical reasoning through the same standard axioms. Any result can report the axioms and assumptions it depends on. Unlike Lean, Locus keeps dependent types inside propositions: values can appear in what a proof states, but never change a runtime type or its layout.

5. **A logical layer over Rust.** Logical types have no runtime representation; they exist to state and prove facts. Logic functions and blocks compute only with logical values, and are pure and total. A model maps a runtime type to the logical type that describes it, such as a `u32` to the natural number it holds. The language provides one logical data type, `Nat`; the standard library builds the others, such as integers, booleans, sequences and maps, from `Nat` and logical types that anyone can define. Logical values never decide what the runtime program does.

6. **rustc does Rust's work.** Locus checks the logical layer and leaves the rest to rustc: type inference, trait resolution, ownership and borrow checking, monomorphization, and code generation. rustc never sees logical code: logical types reach it only as zero-sized placeholders, and logical items only as stubs. Locus should work with a stable Rust toolchain; if a capability requires nightly, that is acceptable, and the dependency is stated.

7. **Readable Rust first, executables later.** Locus first compiles to Rust source. That source must be readable, close to what a Rust programmer would write for the same program, because people audit it and Rust programs depend on it. Later, Locus will also build executables directly, by driving rustc rather than with a code generator of its own.

8. **Distributed through Cargo.** A Locus crate is a Cargo package. Publishing it includes the generated Rust, so Rust programs depend on it like any other crate, in the configurations its author checked, with no Locus toolchain and no runtime support. What Locus needs to reuse proofs across crates travels in the package's metadata. Interfaces exported to Rust protect what was proven: Rust callers cannot supply proofs or build values that would break an invariant. Locus code can use Rust crates and state assumed facts about them; how, and how those assumptions are audited, is for RFCs to decide.

9. **What a proof guarantees.** Safe Locus cannot cause undefined behavior, as safe Rust cannot. A proven result holds whenever the program is built and used through safe Locus or safe Rust, given three things: the kernel and the [trust model](design/TRUST.md); the assumptions the result depends on, which Locus can list; and any `unsafe` code involved, in the crate, its dependencies or the standard library, upholding Rust's safety contract. A proof returned by a function holds when the function returns; a panic, or a loop that never ends, proves nothing. Whether a function terminates is a separate claim, made explicitly.

## Non-goals

- **Changing Rust.** Locus does not change the meaning of any Rust program, or replace Rust's type system, ownership or traits.
- **Proving facts about code Locus does not build.** Locus can call any Rust crate and state assumed facts about it. Verifying existing Rust crates is what tools such as Verus, Creusot, Kani and Aeneas are for.
- **Being a general-purpose proof assistant.** Locus aims at facts about programs, such as bounds, states and invariants, not mathematics at large. Lean remains the tool for that.
- **A code generator of its own.** Executables come through rustc.

Some things are not goals *yet*. Concurrency, interior mutability, and reasoning about `unsafe` code can all be used from the start, but proving facts about them is later work.

## Relation to other work

- **Lean** is the model for Locus's logic, and for trusting a small kernel rather than the automation around it.
- **Verus, Creusot, Prusti and Flux** verify Rust code from annotations, mostly by sending verification conditions to automated solvers. Locus differs in four ways:
  - it accepts only certificates checked by a kernel;
  - proofs are first-class values that can be carried in data and are erased;
  - it is a superset of Rust at the level of whole files and crates;
  - its generated interfaces protect what was proven from Rust callers.
- **Aeneas** translates safe Rust into Lean to prove facts about it there; Locus keeps the proofs in the program. Aeneas's reading of borrow-checked Rust as pure functions is a likely basis for how Locus reasons about mutation.
- **Low\* with KaRaMeL**, a verified subset of F\* compiled to readable C, is the closest in spirit: verified systems code that leaves no trace of its proofs.

## Foundational questions

Open decisions that shape everything else. Each will be settled by an RFC.

1. **Type information.** How Locus obtains Rust's types and trait resolutions for the code it reasons about, and whether that can be done on stable Rust.
2. **Ownership of logical code.** rustc never sees erased code, so something else must ensure that a proof does not observe a value after it has moved, or read through a reference that is no longer valid.
3. **Runtime meaning.** What proofs take Rust's semantics to be (candidates include the Ferrocene Language Specification and MiniRust), including behavior that varies between builds.
4. **Syntax.** How propositions and proofs are written using only contextual keywords and syntax that is not valid Rust, including inside macros.
5. **Models.** Whether a runtime type has one logical model or may have several.
6. **Interoperation.** Using Rust crates, stating and auditing assumed facts, and carrying proofs between Locus crates.
7. **Termination.** How functions claim to terminate, and how those claims are proven.
