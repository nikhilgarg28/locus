# Architecture

This document describes how the compiler is built: its intermediate representations, what each contains, which passes connect them, which passes are trusted, and the elaborator that produces the first of them from source. The language is defined in [language.md](language.md) and the kernel's rules in [the kernel contract](kernel-contract.md).

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

The resolved tree is a stage of the design and not a data structure in the implementation: the elaborator resolves each name against its scope as it builds the typed tree, after ordering the declarations by what they mention (see The elaborator, below).

## The typed tree

The typed tree is the source program with nothing left implicit. It has two kinds of content.

The executable skeleton mirrors the source: the same nesting, names, and declaration order; `if` stays `if`, `match` stays `match`, method-call syntax is kept, `let` patterns stay patterns. This is what `erase` projects and the printer prints, so it must look like what was written.

The logical annotations are kernel-level: propositions and ghost values are kernel `Term`s, proofs are kernel `Proof`s, and types are kernel `Type`s. They never reach the output, so they need no source shape. Every hole has been replaced by the proof the elaborator found.

The typed tree carries identities for everything the checker will bind: each `let`'s variable and its defining equation, each branch's and arm's fact, each arm's payload variables, each loop's state variables, and each call's result. The elaborator writes proofs against these identities, and `lower` uses the same ones, which is what keeps a proof written against the typed tree valid in the check IR.

The nodes that stand for a place or a call carry their type (a variable, a field access, a call), and the control forms carry their result type. That is enough to tell, for any expression, whether its value is a proof, which both `lower` and `erase` need to know. Types here are kernel types written over the identities of the binders in scope; `lower` turns a signature or a loop's state into the telescope the kernel wants.

The body of a math function is a source-shaped tree too, because it is printed like any other function. `lower` turns it into a kernel `Term`.

The metadata the typed tree needs for faithful printing is modest and lives in the tree naturally: identifier and field names, literal spellings (the lexer keeps them), and documentation comments on declarations.

Initially the typed tree has flat patterns only: one constructor deep, as the kernel's `case` is. The elaborator nests matches, and the output shows nested `match`es. Keeping nested patterns in the typed tree, with `lower` compiling them to case trees, is a later refinement that adds trusted code.

## The check IR

The check IR exists to be checked. It is never printed or run in production.

- Types are kernel `Type`s. Pure subexpressions are kernel `Term`s. Every ghost position holds a kernel `Proof`.
- An ordinary `fn` body is in let-normal form. Every intermediate result of a computation that may diverge has a name. The right side of a binding is a pure kernel term, a call to an ordinary `fn`, or a control form (`match`, `loop`, `for`). A `for` here is for a body that is not pure, one that calls ordinary functions; a `for` with a pure body is a kernel term. Its state is given as a function type from the index to the state's tuple type, which is how a state type mentions the index. A block ends in a value, `break`, `continue`, or a `match` of blocks.
- A math function body is a kernel `Term`.

Let-normal form makes the trusted checker a simple walk that maintains a kernel `Context`, which is specification section 6.3 made concrete:

| Check IR | Effect on the kernel context |
|---|---|
| `let x = pure term` | declare `x` and assume `x == term` |
| `have h: P by proof` | check the proof, then assume `P` as `h` |
| `let x = f(args)`, `f` an ordinary `fn` | check the arguments against `f`'s parameters; declare `x : R[args]` with no defining equation |
| a `match` arm | declare the payload variables and assume `scrutinee == variant(payload)` |
| a `loop` | declare abstract state variables; `continue` arguments and the initial values are checked against the state telescope, `break` values against the result type |
| a bounded `for` | check that the bounds are executable bytes and that the given proof shows `lo <= hi`; check the initial values against the state at `lo`; declare the index and abstract state at the index, and assume `lo <= index` and `index < hi`; `continue` arguments are checked against the state at `index + 1`; there is no `break`; the result is the state at `hi` |

Pure terms are typed by the kernel in `Executable` mode, which is what enforces the ghost rules: a ghost variable cannot be the scrutinee of an executable match, an executable argument, or executable data. A value returned by a call is an opaque variable; evidence returned with it is reached by projection. Nothing about termination is checked, and a math function cannot mention an ordinary `fn` at all, because kernel terms have no way to name one.

## Lowering and erasure

Both are defined by recursion on the typed tree, against one stated evaluation order: left to right, as the specification says.

`lower` is a desugaring: `if` to `case` on `bool`, nested expressions to let-normal form, operators and methods to primitives, patterns to projections and case arms, each binder to the identity the typed tree already gave it. It works item by item, in a session: each struct, enum, and function is checked as it is declared, and the identity it receives is what later items use to refer to it. In detail:

- A pure expression becomes a kernel term. An expression is pure when evaluating it always returns and transfers no control: no call to an ordinary function, no `loop`, no `break` or `continue`. A bounded `for` is pure when its body is, apart from the `continue` that ends it, and then it becomes the kernel's `for` term; otherwise it becomes a statement of the check IR. The body of a math function must be pure.
- An expression that is not pure is put in let-normal form: each step that may not return becomes a statement, in source order, under the identity the typed tree gave it. A nested call such as `increment(increment(n).0)` is two statements, and the tree had already named both results, so a proof can speak of either.
- A kernel term has no `let`. In a pure block, each `let` is substituted into what follows, and the equation it would have provided becomes an instance of reflexivity. In a block of the check IR, a `let` is a statement and keeps its equation.
- A condition that is a negation, `a != b`, branches on the comparison `a == b` with the branches exchanged, because branch facts are about the comparison performed, which is what the kernel's reflection axioms speak of.
- The kernel wants a proof-typed position inside a term to hold `proof(...)`. A proof-typed variable, field, or call is a fine expression in the source, as in `break (i, bound)`, so `lower` wraps it as `proof(of_term(...))`.

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

Rust generation is a printer over the erased tree, tested by compiling its output with rustc under `-D warnings`, running it, and comparing what it prints with the interpreter's results.

The two loop forms have no direct Rust spelling and are printed through mutable state. The state lives in variables of its own, and each iteration rebinds the source's names from them, so a `let` in the body that shadows a state name cannot disturb the loop, as in Locus:

~~~
fn bounded_walk(limit: u8) -> (u8, Proved) {
    let mut state_i: u8 = 0;
    let mut state_bound: Proved = Proved;
    loop {
        let i = state_i;
        let bound = state_bound;
        if i == limit {
            break (i, bound);
        } else {
            let differs = Proved;
            let below = Proved;
            let next = i.wrapping_add(1);
            let next_bound = Proved;
            (state_i, state_bound) = (next, next_bound);
            continue;
        }
    }
}
~~~

The right side of the assignment is evaluated in full before any state changes, which is what `continue(next...)` means. These temporaries are the kind of modest departure from the source that is accepted where it makes correctness simple. Every value in the core is immutable and freely reusable, so generated structs and enums derive `Copy`.

## The trusted base

The kernel; the exec checker; `lower`; `erase`; the Rust printer; and the Rust toolchain. The parser, name resolution, the elaborator, the hole solver, the derived proof forms, the prelude's lemmas, and both interpreters are not trusted: a fault in any of them produces a rejection or a failing test, not a false theorem.

Compared with checking a lowered core directly, `lower` has moved into the trusted base. In exchange the typed tree is much closer to the source than a lowered core is, so the distance between what was written and what was verified is smaller.

## The elaborator

The elaborator turns a parsed file into typed trees and hands each item to the `Session`, which lowers it, has the kernel check it, and erases it ([IR architecture](#the-elaborator)). It lives in `src/elab/`. Nothing in it is trusted: a mistake here produces a program the kernel rejects, never an accepted wrong one.

It does four jobs that the plan lists separately: name resolution, type elaboration, filling each `_` with an explicit proof, and the diagnostics for all three. There is no separate resolved syntax tree. Names are resolved against a scope as expressions are elaborated, and declarations are ordered first by what they mention.

### Order of declarations

An item is checked when it is declared, so everything it mentions must already exist. `order.rs` reads dependencies off the syntax: every name an item writes that is also the name of an item. Items are elaborated dependencies first, whatever order the file lists them in. A cycle is an error (`L0203`), because the core has no recursion. A local that shadows an item's name counts as a mention of it, which can only add an edge.

When an item is rejected, the items that mention it are skipped without a second report.

### The mirrored context

Proofs in a typed tree refer to identities: the binder of each `let`, the fact of each branch and arm, the result of each call. The checker binds those identities as it walks the lowered program. While it works, the elaborator keeps a kernel context of its own with the same identities, bound in the same order, so that it can:

- ask the kernel what a term's type is, instead of recomputing it (a projection's type, a `let` binder's type);
- test every proof it finds with the kernel before using it, so that a failure is reported at the `_` and not as a rejection of the whole function.

The mirrored context must never hold more than the checker's will. Three rules keep it so. A branch, an arm, a loop body, and a quantifier end with a rollback. A block used as an expression keeps its declarations, because the checker splices its statements into the enclosing sequence, and forgets its names and facts. The term that stands for an expression's value is computed by `typed::value_term`, which is lowering's own rule without the lowering.

### Bidirectional elaboration

An expression is elaborated against the type expected of it when there is one. That is how `_` learns what to prove, how a tuple checked against `(out: u8, @[out == n])` learns that its second field speaks of its first, and how `Or::Left(h)` learns which disjunction it proves. Arguments, struct fields, variant payloads, loop state, and `continue` are all checked against a telescope: each value's term replaces its binder in the types that follow.

Inside `[ ... ]` and proof types, `logic.rs` builds kernel propositions directly: a comparison is a claim, and `&&`, `||`, `!`, `=>` are connectives. Outside, `a && b` on booleans is an `if`, which is also how the right operand comes to know the left one held.

Evidence of one claim is accepted where another is wanted when the solver can bridge them. This is what lets `still`, which is evidence about `step(...).0`, be passed where evidence about `next` is wanted after `let (next, still) = step(...)`.

### Filling a hole

The search is fixed and bounded. Every limit counts steps.

1. **A fact in scope.** Facts are: branch and arm evidence; the equation of each `let`; loop bounds; every evidence-typed name; and the evidence fields of every tuple and struct in scope, each stated about that value's own fields.
2. **The same after normalizing** the claim and every fact. Names are replaced by what they stand for, using the equations in scope whose left side is a variable or a field of one, read left to right. Projections of written tuples and structs, matches on written constructors, and arithmetic on literals are computed. Calls of the program's own math functions are unfolded; the prelude's orderings stay folded. Conjunctions among the facts are taken apart.
3. **Reflexivity; structure.** `&&` by proving both sides, `||` by proving one, `=>` by assuming the premise, `forall` by generalizing, `false` from a refuted fact whose claim can be shown. Depth at most 8.
4. **Closed evaluation.** A comparison of known values is run by the kernel.
5. **All 256 cases.** A comparison that speaks of exactly one unknown byte is decided by evaluating it at every byte, under the facts that speak of that byte alone. The kernel's `EvaluateAll` rule does the evaluation; the elaborator assembles the facts into the evaluated claim and discharges them afterwards. When a case fails, the failing byte is reported.

Ordering claims are compared in one form, as the outcome of the runtime test (`a < b` is `u8_lt(a, b) == true`), using the kernel's reflection axioms in both directions.

What this does not reach: a claim relating two unknown bytes that is not already a fact after normalizing, such as `x <= limit` and `limit <= 9` giving `x <= 9`. That step is a lemma call. The checked ordering lemmas are callable by name: `u8_le_refl`, `u8_zero_le`, `u8_le_trans`, `u8_lt_of_le_of_ne`, `u8_succ_le_of_lt`. Rewriting is oriented, from names to values; it is not a congruence closure.

Relative to specification section 12.3, tier 5 is an addition and the congruence closure of its tier 3 is not built. The specification has been updated to say so.

### When a hole cannot be filled

The report states, in source syntax:

- the claim, with written values put in their fields;
- the claim after computing, when that differs;
- a failing case, when evaluation found one: the byte, and that the known facts allow it;
- the facts that speak of a value the claim speaks of, normalized the same way, at most six.

~~~
error[L0230]: cannot show `within_limit(lock.failures.wrapping_add(1))`
  --> lock.loc:27:77
   = note: after computing, the claim is `lock.failures.wrapping_add(1) <= 3`
   = note: it fails when `lock.failures` is 3, which the facts known here allow
   = note: known here: `lock.failures <= 3`
~~~

A `let` whose value fails poisons the names it would have bound, so that their uses are not reported as unknown.

### Evidence written out

`proofs.rs` elaborates the explicit forms of specification section 8 into kernel proofs: proof constructors, with `And::Intro`, `Or::Left`, `Or::Right`, and `True::Intro` for the built-in connectives; `match` on evidence, where each arm receives its variant's index equations as facts; `match h {}` on evidence of `false`, at any type; `rewrite`, `unfold`, `fold`; evidence of a `forall` or an implication applied to an argument; and a `math fn` that returns evidence, named as evidence of its general claim.

### Measurements

`locus check file.loc --holes` lists every hole: where, which tier filled it, the size of the proof in nodes, and the time to find and check it once. `locus check file.loc --stats` lists, per function, the time to elaborate it, which includes the search, and the time to lower, check, and erase it.

For `examples/lock.loc` in a debug build: 8 proofs, about 800 proof nodes, about 8 ms elaborating, of which 6 ms is search, and about 5 ms checking. The largest proof is a 256-case one, at about 250 nodes; its size does not grow with the number of cases, because the kernel evaluates them. There is no cache of found proofs yet, so checking a file again after an edit costs the same as checking it the first time. Memory is not measured.

### Not yet elaborated

- Patterns beyond a name, `_`, and a tuple in `let`; beyond `Enum::Variant(names)` and `_` in `match`. Literal and struct patterns parse and are rejected here.
- `let And::Intro(a, b) = h;`. Use `match`.
- `exists`: formulas elaborate, and there is no source form yet for introducing or opening one.
- Values of `fn` type. A `math fn` type elaborates; no expression has one except a function named as evidence.
- Field access by name on a tuple with named fields. Use `.0` or `let`.
- A `for` with one state parameter yields a one-field tuple; the specification says it yields the value.
- In generated Rust, evidence that needed bridging prints as `Proved` and not as the name that was written, and `match` arms print in declaration order.
- In a `math fn`, a `for` whose body is not a sequence of `let`s ending in `continue` is rejected by lowering with an internal error, not with a message of its own.

## Build order

All six steps are done. Steps 1 to 5 were built against hand-built typed trees; step 6 produces typed trees from source, and is described in the section The elaborator.

1. Kernel: give the arms of a term-level `case` the hypothesis `scrutinee == variant(payload)`, so that a math function's branches know what an executable `match` arm knows.
2. The check IR and the exec checker, driven by hand-built programs: `increment`, `preserve`, `classify`, `bounded_walk`, and `spin` with its caller.
3. The typed tree, `lower`, and the ghost-skipping interpreter for the check IR.
4. The erased tree, `erase`, its type checker, and the reference interpreter with fuel; the differential and divergence tests.
5. The Rust printer, and compiled-versus-interpreted tests.
6. The frontend: realignment of the parser, name resolution, and the elaborator targeting the typed tree.
