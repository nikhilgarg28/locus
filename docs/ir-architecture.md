# IR architecture

This document describes the compiler's intermediate representations: how many there are, what each contains, which passes connect them, and which passes are trusted. It refines step 3 of the order of work in [the core plan](core-plan.md). The language is defined in [the specification](core-language-spec.md) and the kernel's rules in [the kernel contract](kernel-contract.md).

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
Typed tree T       the source program made fully explicit
   |
   +--- lower (TRUSTED) ---> Check IR L ---> exec checker + kernel (TRUSTED)
   |
   +--- erase (TRUSTED) ---> Erased tree E ---> Rust printer ---> rustfmt ---> rustc
                                          \---> reference interpreter
~~~

| Layer | Contains | Holds once built |
|---|---|---|
| Surface AST | what the user typed | it parses |
| Resolved AST | identities in place of names | scoping and shadowing are settled (specification section 2) |
| Typed tree T | explicit types, filled holes, identities for everything that binds | nothing; T is a claim until L is checked |
| Check IR L | let-normal executable code over kernel terms and proofs | accepted by the exec checker and the kernel |
| Erased tree E | data and control only, in the shape of T | a proof or a ghost cannot be represented |

## The typed tree T

T is the source program with nothing left implicit. It has two kinds of content.

The executable skeleton mirrors the source: the same nesting, names, and declaration order; `if` stays `if`, `match` stays `match`, method-call syntax is kept, `let` patterns stay patterns. This is what `erase` projects and the printer prints, so it must look like what was written.

The logical annotations are kernel-level: propositions and ghost values are kernel `Term`s, proofs are kernel `Proof`s, and types are kernel `Type`s. They never reach the output, so they need no source shape. Every hole has been replaced by the proof the elaborator found.

T carries identities for everything the checker will bind: each `let`'s variable and its defining equation, each branch's and arm's fact, each arm's payload variables, each loop's state variables, and each call's result. The elaborator writes proofs against these identities, and `lower` uses the same ones, which is what keeps a proof written against T valid in L.

The body of a math function is a source-shaped tree too, because it is printed like any other function. `lower` turns it into a kernel `Term`.

The metadata T needs for faithful printing is modest and lives in the tree naturally: identifier and field names, literal spellings (the lexer keeps them), and documentation comments on declarations.

Initially T has flat patterns only: one constructor deep, as the kernel's `case` is. The elaborator nests matches, and the output shows nested `match`es. Keeping nested patterns in T, with `lower` compiling them to case trees, is a later refinement that adds trusted code.

## The check IR L

L exists to be checked. It is never printed or run in production.

- Types are kernel `Type`s. Pure subexpressions are kernel `Term`s. Every ghost position holds a kernel `Proof`.
- An ordinary `fn` body is in let-normal form. Every intermediate result of a computation that may diverge has a name. The right side of a binding is a pure kernel term, a call to an ordinary `fn`, or a control form (`match`, `loop`, `for`). A block ends in a value, `break`, `continue`, or a `match` of blocks.
- A math function body is a kernel `Term`.

Let-normal form makes the trusted checker a simple walk that maintains a kernel `Context`, which is specification section 6.3 made concrete:

| L | Effect on the kernel context |
|---|---|
| `let x = pure term` | declare `x` and assume `x == term` |
| `have h: P by proof` | check the proof, then assume `P` as `h` |
| `let x = f(args)`, `f` an ordinary `fn` | check the arguments against `f`'s parameters; declare `x : R[args]` with no defining equation |
| a `match` arm | declare the payload variables and assume `scrutinee == variant(payload)` |
| a `loop` | declare abstract state variables; `continue` arguments and the initial values are checked against the state telescope, `break` values against the result type |

Pure terms are typed by the kernel in `Executable` mode, which is what enforces the ghost rules: a ghost variable cannot be the scrutinee of an executable match, an executable argument, or executable data. A value returned by a call is an opaque variable; evidence returned with it is reached by projection. Nothing about termination is checked, and a math function cannot mention an ordinary `fn` at all, because kernel terms have no way to name one.

## Lowering and erasure

Both are defined by recursion on T, against one stated evaluation order: left to right, as the specification says.

`lower` is a desugaring: `if` to `case` on `bool`, nested expressions to let-normal form, operators and methods to primitives, patterns to projections and case arms, each binder to the identity T already gave it.

`erase` is a projection that preserves structure: ghost parameters, arguments, fields, and `let`s disappear, proofs disappear, and each type becomes its erased type (specification section 11). Where a ghost position holds a computation that must still run, such as a proof field initialized by a call to a divergent `fn`, `erase` emits the call as a statement before the enclosing expression. That is the one place it restructures.

What necessarily differs between the Locus source and the Rust output: ghost things are gone; `math fn` becomes `fn`; `loop (s: T = init) -> R { ... continue(next) }` becomes mutable state and a Rust `loop`; `for i in lo..hi (state) { ... }` becomes mutable state and a Rust `for`; and an occasional hoisted statement as just described.

### Erased fields

How an erased field appears in Rust was open question 4 of the specification. The options:

| Option | `(out: u8, @P)` | `struct NonZero { value, evidence }` | Notes |
|---|---|---|---|
| Remove | `u8` | `struct NonZero { value: u8 }` | Idiomatic signatures and call sites. Positions are renumbered. A tuple that loses fields and is left with one becomes that field. |
| Unit placeholder | `(u8, ())` | `{ value: u8, evidence: () }` | Positions are stable. Zero runtime cost. Noise at every construction and in every exported signature. |
| Named zero-sized marker | `(u8, Proved)` | `{ value: u8, evidence: Proved }` | As the unit placeholder, self-describing. A marker cannot make an exported function safe to call from Rust: Rust cannot tie the marker to the value it is about. |

`PhantomData<T>` is not a fit for proof fields. It is a zero-sized carrier for a Rust type or lifetime parameter, and a proposition such as `value != 0` mentions a value, which no Rust type can express. It does have a job later: when erasure leaves a type or lifetime parameter of a generic struct unused, because the parameter occurred only in ghost fields, Rust rejects the declaration unless a `PhantomData` field mentions it. `erase` will insert one then.

The working choice is removal, for readable output. Two refinements are expected with generics: a type parameter instantiated with a ghost type must erase to `()`, since a generic Rust function needs some type there; and `PhantomData` insertion as above. For exported interfaces, the evidence is carried by privacy: a validated type with a private field and a checked constructor, not a marker argument.

## Agreement between the two branches

The checker sees `lower(T)`; the machine runs `erase(T)`. Their agreement rests on three things.

1. Construction. Both passes are small, syntax-directed recursions over one tree with one evaluation order.
2. Differential testing. An interpreter for L that skips ghosts and the interpreter for E must agree on every test program, including on running out of fuel. This runs on every example.
3. A theorem, eventually. "Erasure preserves behavior" is already a proof obligation in specification section 16. It is restated as: for every T, `erase(T)` behaves as `lower(T)` does with ghosts ignored.

A second cheap check guards `erase` alone: E has its own simple type checker, and everything `erase` emits must pass it. A dangling ghost reference or a renumbering mistake fails there.

## Interpreter and Rust generation

Both consume E.

The reference interpreter comes first. It is small, needs no toolchain in tests, gives the semantics an executable definition, and has a fuel counter so that divergence is observable: "the caller of a divergent function still diverges after erasure" is tested as running out of fuel, not as returning a value. It takes the meaning of the primitives from the kernel's native evaluation, so logic and execution share one definition. It does not reuse the kernel's term evaluator, which is trusted and covers total terms only.

Rust generation is a printer over E, tested by compiling its output and comparing results with the interpreter.

## The trusted base

The kernel; the exec checker; `lower`; `erase`; the Rust printer; and the Rust toolchain. The parser, name resolution, the elaborator, the hole solver, the derived proof forms, the prelude's lemmas, and both interpreters are not trusted: a fault in any of them produces a rejection or a failing test, not a false theorem.

Compared with checking a lowered core directly, `lower` has moved into the trusted base. In exchange T is much closer to the source than a lowered core is, so the distance between what was written and what was verified is smaller.

## Build order

1. Kernel: give the arms of a term-level `case` the hypothesis `scrutinee == variant(payload)`, so that a math function's branches know what an executable `match` arm knows.
2. The check IR L and the exec checker, driven by hand-built programs: `increment`, `preserve`, `classify`, `bounded_walk`, and `spin` with its caller.
3. The typed tree T, `lower`, and the ghost-skipping interpreter for L.
4. The erased tree E, `erase`, E's type checker, and the reference interpreter with fuel; the differential and divergence tests.
5. The Rust printer, and compiled-versus-interpreted tests.
6. The frontend: realignment of the parser, name resolution, and the elaborator targeting T.
