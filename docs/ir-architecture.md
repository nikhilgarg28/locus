# IR architecture

This document describes the compiler's intermediate representations: how many there are, what each contains, which passes connect them, and which passes are trusted. It refines step 3 of the order of work in [the core plan](core-plan.md). The language is defined in [the specification](core-language-spec.md) and the kernel's rules in [the kernel contract](kernel-contract.md).

## The three representations in brief

| Name | What it is | Made by | Used by |
|---|---|---|---|
| **Typed tree** | The source program made fully explicit: every name resolved, every type present, every hole replaced by the proof that was found. It keeps the shape of the source. | the elaborator (untrusted) | `lower` and `erase` |
| **Check IR** | A flattened form of the typed tree that exists only to be verified: executable code in let-normal form over kernel terms and proofs. Nothing prints or runs it. It lives in `src/exec/`. | `lower` (trusted) | the exec checker and the kernel |
| **Erased tree** | The typed tree with the logic taken out: the same shape, containing only what exists at runtime. | `erase` (trusted) | the reference interpreter and the Rust printer |

One program, the typed tree, is both verified, through the check IR, and executed, through the erased tree.

## Goals

1. What is verified is what runs. A program is checked and executed from one shared artifact.
2. The generated Rust reads like the Locus that was written: the same names, nesting, declaration order, and control structure. Layout is left to rustfmt.
3. The trusted passes are small, syntax-directed, and individually testable.
4. Proof search, inference, and diagnostics stay untrusted.

Goal 2 rules out printing Rust from a lowered form. Once a program is in let-normal form with case trees and positional fields, recovering `let next = i.wrapping_add(1);` is decompilation, and the pass that does it would be both fragile and trusted. So the shared artifact is source-shaped, and lowering is a side branch used only for checking.

## The pipeline

~~~
source text
   |  parser                                   (exists)
   v
Surface AST        syntax as written: spans, sugar, names as strings
   |  name resolution                          (untrusted)
   v
Resolved AST       the same shape; every name is a binding or declaration identity
   |  elaborator                               (untrusted)
   v
Typed tree         the source program made fully explicit
   |
   +--- lower (TRUSTED) ---> Check IR ---> exec checker + kernel (TRUSTED)
   |
   +--- erase (TRUSTED) ---> Erased tree ---> Rust printer ---> rustfmt ---> rustc
                                        \---> reference interpreter
~~~

| Layer | Contains | Holds once built |
|---|---|---|
| Surface AST | what the user typed | it parses |
| Resolved AST | identities in place of names | scoping and shadowing are settled (specification section 2) |
| Typed tree | explicit types, filled holes, identities for everything that binds | nothing; it is a claim until its check IR is accepted |
| Check IR | let-normal executable code over kernel terms and proofs | accepted by the exec checker and the kernel |
| Erased tree | data and control only, in the shape of the typed tree | a proof or a ghost cannot be represented |

## The typed tree

The typed tree is the source program with nothing left implicit. It has two kinds of content.

The executable skeleton mirrors the source: the same nesting, names, and declaration order; `if` stays `if`, `match` stays `match`, method-call syntax is kept, `let` patterns stay patterns. This is what `erase` projects and the printer prints, so it must look like what was written.

The logical annotations are kernel-level: propositions and ghost values are kernel `Term`s, proofs are kernel `Proof`s, and types are kernel `Type`s. They never reach the output, so they need no source shape. Every hole has been replaced by the proof the elaborator found.

The typed tree carries identities for everything the checker will bind: each `let`'s variable and its defining equation, each branch's and arm's fact, each arm's payload variables, each loop's state variables, and each call's result. The elaborator writes proofs against these identities, and `lower` uses the same ones, which is what keeps a proof written against the typed tree valid in the check IR.

The body of a math function is a source-shaped tree too, because it is printed like any other function. `lower` turns it into a kernel `Term`.

The metadata the typed tree needs for faithful printing is modest and lives in the tree naturally: identifier and field names, literal spellings (the lexer keeps them), and documentation comments on declarations.

Initially the typed tree has flat patterns only: one constructor deep, as the kernel's `case` is. The elaborator nests matches, and the output shows nested `match`es. Keeping nested patterns in the typed tree, with `lower` compiling them to case trees, is a later refinement that adds trusted code.

## The check IR

The check IR exists to be checked. It is never printed or run in production.

- Types are kernel `Type`s. Pure subexpressions are kernel `Term`s. Every ghost position holds a kernel `Proof`.
- An ordinary `fn` body is in let-normal form. Every intermediate result of a computation that may diverge has a name. The right side of a binding is a pure kernel term, a call to an ordinary `fn`, or a control form (`match`, `loop`, `for`). A block ends in a value, `break`, `continue`, or a `match` of blocks.
- A math function body is a kernel `Term`.

Let-normal form makes the trusted checker a simple walk that maintains a kernel `Context`, which is specification section 6.3 made concrete:

| Check IR | Effect on the kernel context |
|---|---|
| `let x = pure term` | declare `x` and assume `x == term` |
| `have h: P by proof` | check the proof, then assume `P` as `h` |
| `let x = f(args)`, `f` an ordinary `fn` | check the arguments against `f`'s parameters; declare `x : R[args]` with no defining equation |
| a `match` arm | declare the payload variables and assume `scrutinee == variant(payload)` |
| a `loop` | declare abstract state variables; `continue` arguments and the initial values are checked against the state telescope, `break` values against the result type |

Pure terms are typed by the kernel in `Executable` mode, which is what enforces the ghost rules: a ghost variable cannot be the scrutinee of an executable match, an executable argument, or executable data. A value returned by a call is an opaque variable; evidence returned with it is reached by projection. Nothing about termination is checked, and a math function cannot mention an ordinary `fn` at all, because kernel terms have no way to name one.

## Lowering and erasure

Both are defined by recursion on the typed tree, against one stated evaluation order: left to right, as the specification says.

`lower` is a desugaring: `if` to `case` on `bool`, nested expressions to let-normal form, operators and methods to primitives, patterns to projections and case arms, each binder to the identity the typed tree already gave it.

`erase` is a projection that preserves shape exactly (specification section 11). Each type becomes its erased type. A ghost position does not disappear: it is filled by a named zero-sized marker, `Proved` for a proof and `Ghost` for any other ghost value, which in the core means a proposition. So field positions, tuple arity, parameter lists, and patterns are the same in the output as in the source, and `erase` never renumbers, collapses, or moves anything.

| Locus | Rust |
|---|---|
| `fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)])` | `fn increment(n: u8) -> (u8, Proved)` |
| `(out, _)` | `(out, Proved)` |
| `let (value, _) = increment(n);` | `let (value, _) = increment(n);` |
| `let h: @[n != 0] = _;` | `let h = Proved;` |
| `continue(next, next_bound);` | the same, as an assignment of the loop's state |
| `struct NonZero { value: u8, evidence: @[value != 0] }` | `struct NonZero { value: u8, evidence: Proved }` |
| `let p: Prop = [n == 3];` | `let p = Ghost;` |

A ghost-typed expression that is a variable stays that variable: a proof bound by a `let` or a pattern is an ordinary Rust binding of type `Proved`. Any other ghost-typed expression becomes its marker when its computation is erasable, and `{ effects; marker }` when something in it must still run. `And::Intro(spin(), h)`, with `spin` a divergent `fn` returning a proof, becomes `{ spin(); Proved }`. A call such as `spin()` on its own needs nothing: its erased result type is already `Proved`. Evaluation order is the source's, because nothing has been removed.

A math function whose signature is entirely ghost, a lemma or a predicate, has no runtime form and is not emitted. An empty match used for its value becomes a trap.

What necessarily differs between the Locus source and the Rust output: the markers; `math fn` becomes `fn`; `loop (s: T = init) -> R { ... continue(next) }` becomes mutable state and a Rust `loop`; `for i in lo..hi (state) { ... }` becomes mutable state and a Rust `for`; and Locus-only syntax such as proof types in annotations.

### The markers

Every generated crate carries two unit structs, `Proved` and `Ghost`, that are `Clone` and `Copy`. They occupy no space and the optimizer removes every trace of them.

The alternatives considered were removing ghost positions, which gives the most idiomatic signatures but renumbers positions, collapses one-field tuples, and forces retained computations to be hoisted into statements; and a bare `()`, which keeps shape but says nothing. Named markers keep the shape of the source, make the output self-describing, and keep `erase` a pure projection. Their cost is that signatures show them.

Two refinements are expected and change nothing now. When the `ghost` keyword brings ghost data, `Ghost` can carry the type it stands for, `Ghost<T>` backed by `PhantomData<T>`: `ghost model: Seq<T>` would print as `model: Ghost<Seq<T>>`, which keeps the type visible and keeps a type parameter that occurs only in ghost fields in use, where Rust would otherwise reject the declaration. With generics, a type parameter instantiated with a proof type is simply `Proved`.

A marker does not make an exported function safe to call from Rust: Rust cannot tie a `Proved` to the value it is about, so foreign code could pass one for the wrong value. For exported interfaces the evidence is carried by privacy instead, a validated type with a private field and a checked constructor, and functions that take markers stay crate-private. That belongs to the Rust interoperability milestone.

## Agreement between the two branches

The checker sees the lowering of the typed tree; the machine runs its erasure. Their agreement rests on three things.

1. Construction. Both passes are small, syntax-directed recursions over one tree with one evaluation order.
2. Differential testing. An interpreter for the check IR that skips ghosts and the interpreter for the erased tree must agree on every test program, including on running out of fuel. This runs on every example.
3. A theorem, eventually. "Erasure preserves behavior" is already a proof obligation in specification section 16. It is restated as: for every typed tree, its erasure behaves as its lowering does with ghosts ignored.

A second cheap check guards `erase` alone: the erased tree has its own simple type checker, and everything `erase` emits must pass it. A dangling ghost reference or a renumbering mistake fails there.

## Interpreter and Rust generation

Both consume the erased tree.

The reference interpreter comes first. It is small, needs no toolchain in tests, gives the semantics an executable definition, and has a fuel counter so that divergence is observable: "the caller of a divergent function still diverges after erasure" is tested as running out of fuel, not as returning a value. It takes the meaning of the primitives from the kernel's native evaluation, so logic and execution share one definition. It does not reuse the kernel's term evaluator, which is trusted and covers total terms only.

Rust generation is a printer over the erased tree, tested by compiling its output and comparing results with the interpreter.

## The trusted base

The kernel; the exec checker; `lower`; `erase`; the Rust printer; and the Rust toolchain. The parser, name resolution, the elaborator, the hole solver, the derived proof forms, the prelude's lemmas, and both interpreters are not trusted: a fault in any of them produces a rejection or a failing test, not a false theorem.

Compared with checking a lowered core directly, `lower` has moved into the trusted base. In exchange the typed tree is much closer to the source than a lowered core is, so the distance between what was written and what was verified is smaller.

## Build order

1. Kernel: give the arms of a term-level `case` the hypothesis `scrutinee == variant(payload)`, so that a math function's branches know what an executable `match` arm knows.
2. The check IR and the exec checker, driven by hand-built programs: `increment`, `preserve`, `classify`, `bounded_walk`, and `spin` with its caller.
3. The typed tree, `lower`, and the ghost-skipping interpreter for the check IR.
4. The erased tree, `erase`, its type checker, and the reference interpreter with fuel; the differential and divergence tests.
5. The Rust printer, and compiled-versus-interpreted tests.
6. The frontend: realignment of the parser, name resolution, and the elaborator targeting the typed tree.
