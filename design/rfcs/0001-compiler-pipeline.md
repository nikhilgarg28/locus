# RFC 0001: The compiler pipeline

- **Status:** proposed
- **Issue:** none
- **Supersedes:** none

## Summary

Locus compiles a Cargo crate in four steps:

1. **Parse.** Locus parses its `.lc` sources, Rust plus Locus syntax, into lossless syntax trees.
2. **Lower.** Locus writes each `.lc` file as one Rust text, the **lowered program**. In it, each logical type is a zero-sized Rust type of its own, and every proof is one zero-sized marker, `Proof`, in every position they occupy; a logic function is a stub with its real signature over those types; and each piece of logical code is written a second time as a Rust expression over stubs, in a closure that is never called, which is its **typing skeleton**. So rustc types logical code, resolves its names and its traits, and checks what it borrows, and never runs it. Lowering decides only what the source shows, never an expression's type.
3. **Check.** rustc compiles the lowered program through a small driver, with the *checking* expansions of Locus's macros selected. That is the **checking program**. The driver reports what Locus needs: the types rustc gave logical code, what names resolve to, and the typed bodies of the functions being verified. rustc's borrow checker decides whether logical code may read what it reads. Locus then checks what rustc cannot: each proposition and proof, for which rustc has one marker, and the rules of the logical layer that types do not decide. It constructs proofs, and has the kernel check them.
4. **Build or export.** Only when the check passes, Locus produces the **runtime program**, and validates that it has the typed bodies of the program it checked, apart from logical code:
   - **Build mode,** the default: rustc compiles the same lowered program again, with the *runtime* expansions, which carry nothing to the driver. The two programs have the same types everywhere, so rustc resolves every call alike in both. `cargo locus build` builds it under `target/`, in the configuration being built. Every crate in the build that depends on a Locus crate is checked too, so contracts hold across crates.
   - **Export mode:** `cargo locus export` erases the logical layer by editing each `.lc` file into readable Rust, written beside the sources as `.rs` files that Rust users build with plain Cargo. The crate's public interface must then be plain Rust: it has no logical values and states no contracts.

**Locus is built in stages** ([Stages](#stages)). The first needs neither `.lc` files nor a compiler wrapper: Locus code is written in `.rs` files inside `locus! { ... }` blocks, a macro that does what lowering does, and `cargo locus check` checks the crate. The language is built out on that first stage. Build mode follows, and export mode if it is still wanted.

Locus writes its own parser, the lowering, the logical layer, the kernel, erasure, and diagnostics. Everything else that Rust already does is rustc's: name resolution, macro expansion, type inference, trait resolution, ownership and borrow checking, lints, and code generation. Since each piece of logical code has a typing skeleton, rustc resolves, types, and borrow-checks logical code too.

## Motivation

The [vision](../../VISION.md) commits Locus to being a superset of Rust in which "rustc does Rust's work". A front end of Locus's own would have to reimplement moves, borrows, traits, and generics, and every difference from rustc would be a difference between the program checked and the program run. This RFC decides how the work is split instead, which representations exist, how information flows between Locus and rustc, how rustc's errors reach the user, and what Locus produces.

It settles two of the vision's foundational questions:
- **Type information:** where Locus gets Rust's types and resolutions.
- **Ownership of logical code:** how logical code, which rustc never runs, is kept from reading values it may not read.

It also fixes the rules that Locus syntax and Locus names must follow.

A first [spike](../../spikes/rustc-facts/README.md) tested the central mechanism. A hand-written checking program, compiled by a 150-line rustc driver running as Cargo's wrapper, reported:
- the types of the places its proofs read, including a field of a type from a dependency;
- what a path in a proof resolves to;
- the typed body of a function, with indexing resolved to the trait method `<Vec<u32> as Index<usize>>::index`.

rustc's borrow checker rejected the one proof that read a moved value, with E0382 on the matching line. The driver built against stable rustc 1.97 (with `RUSTC_BOOTSTRAP=1`) and against a nightly four releases later, needing a one-line change between them.

[RFC 0002](0002-logical-layer.md) defines the logical layer that this pipeline checks and erases, and the two RFCs are proposed together. Its treatment of proofs as ghost values, which no Rust code can forge, is why the checking and runtime programs contain markers rather than proofs, and why a crate's contracts are visible only to code that Locus verifies.

## Design

**Snapshot reconciliation:** [RFC 0008](0008-snapshot-capture-and-views.md) replaces implicit models with snapshot capture and ordinary explicit `View`, uses type-based `Unit` barriers, and updates collection APIs. The current design below incorporates it; numbered historical decisions retain their original rationale and are superseded where the RFC 0008 inventory says so.

### The pipeline

```text
 crate sources (.rs, .lc)
   │
   │ parse ............................................. Locus
   ▼
 syntax trees (lossless, one per .lc file)
   │
   │ lower ............................................. Locus
   ▼
 lowered program + origin map
   │
   │ compile with the checking library, through locus-driver .. rustc
   ▼
 facts + rustc diagnostics
   │
   ▼
 logical pass: elaborate with rustc's types, check ........ Locus
   │
   ▼
 obligations ─► proof search (untrusted) ─► kernel (trusted) Locus
   │
   │ every proof checks, in every checked configuration
   ▼
 the runtime program, validated against the checked one ... Locus
   │
   ├─► build mode: the lowered program with the runtime library,
   │               compiled under target/ ................... rustc
   └─► export mode: each .lc erased to readable Rust, written
                    beside the sources as .rs files .......... Locus
```

### Who does what

| Job | Done by |
|---|---|
| Parsing Locus syntax in `.lc` files | Locus, with a lossless parser |
| Module tree, name resolution, imports, visibility, including Locus's own names | rustc |
| Rust macro expansion, including procedural macros | rustc |
| Type inference, trait and method resolution, coercions, auto-dereferencing | rustc |
| The types of logical code, and the implementations, bounds, and associated types of logical types | rustc, through the typing skeletons |
| Ownership and borrow checking, including what logical code reads | rustc |
| Lints | rustc, on the runtime program |
| Checking each proposition, and the rules on logical types, logic functions, and models that rustc's typing does not decide | Locus |
| Checking every proof against its proposition: rustc has one marker for all of them | Locus ([RFC 0002](0002-logical-layer.md)) |
| The meaning of runtime code in the logic | Locus, from rustc's typed bodies; the semantics are [runtime-semantics](../../issues/runtime-semantics.md) |
| Proof search | Locus, untrusted |
| Proof checking | Locus's kernel, trusted |
| Erasure to readable Rust, in export mode | Locus |
| Validating that the program built or exported is the one checked | Locus, from rustc's typed bodies of both |
| Monomorphization, code generation, linking | rustc |
| Diagnostics: rendering, and mapping rustc's back to the source | Locus |

### Stages

Locus is built in stages, and the first needs neither `.lc` files nor a compiler wrapper. Locus code is written in ordinary `.rs` files, inside `locus! { ... }` blocks, and the first stage is used to build out the language itself: its logical layer, its proofs, and its library. The rest of this RFC describes the full toolchain, which follows.

1. **The `locus!` macro.** A function-like macro from Locus's library takes Locus syntax as its input and expands to the lowered program of [The lowered program](#the-lowered-program). It is the lowering, done by rustc's macro expansion over the tokens of each block instead of by Locus over whole files ([spike](../../spikes/locus-macro/README.md)).
   - **Function-like, because an attribute macro cannot carry Locus syntax.** rustc parses an item before any attribute on it runs, so `fn f(p: @(y != 0))` is a syntax error even under `#[cfg(any())]` (tested). Inside `locus! { }`, all 264 litmus examples reach the macro as tokens through rustc's own lexer, comments included.
   - **One text, two expansions.** The macro writes one text, and the library's macros put `#[cfg(__locus_check)]` in their own output, so the `cfg` is evaluated in the user's crate: the checking expansion under `--cfg __locus_check`, the runtime expansion otherwise. In the spike, 20 accepted examples compiled both ways and printed their `run` values in both, and two rejected ones failed only in the checking expansion, at their annotated lines (E0382, E0502). With typing skeletons, which are in both expansions, those two fail in both ([skeleton-typing](../../spikes/skeleton-typing/README.md)). Every crate that uses Locus declares `cfg(__locus_check)` in its `check-cfg`, or rustc warns at every block.
   - **`cargo locus check` checks.** It compiles the crate through the driver with the checking `cfg`, runs the logical pass and the kernel, and validates the runtime program against the checked one by compiling the crate a second time without the `cfg`, as build mode does. Its own build of the dependencies runs through the driver too, only to record which of them contain `unsafe` code (RFC 0002, decisions 52 and 95).
   - **`cargo build` builds, unchanged.** To Cargo, rustc, rust-analyzer, and the crate's users, a crate with `locus!` blocks is an ordinary Rust crate that depends on Locus's library. rustc's errors point at the user's lines in both expansions; rustfmt leaves a block exactly as written, plain Rust inside it included; and rust-analyzer expands the blocks and resolves what is in them, one expansion at a time (tested). Building does not check, though it now does more than it did: rustc types and borrow-checks logical code in a plain build too, so a type error or a borrow error there stops it. It checks no proposition, no proof, and none of the rules that Locus checks after rustc ([Running rustc](#running-rustc-locus-driver)). A result holds for the program built from the sources and the configuration that `cargo locus check` checked, so a project runs the check in CI, as it runs its tests.
   - **Code outside `locus!` blocks is plain Rust,** as `.rs` files are in the full toolchain. The driver reports what every path resolves to, and a path outside a block that names a logical item, or calls a function with ghost parameters, is an error. A function there is verified or unverified by the same inference as any other (RFC 0002, decision 65), and a verified one is checked by the same rules as a function inside a block. It writes no Locus syntax, so it states no proposition of its own, but what it does is checked like any other code: a construction of a type with proof fields proves them for the values it gives, and a field that a proof reads is never assigned in place (RFC 0002, decision 66; `contracts.invariants`, `contracts.invariants.assignment`). A generic one may be instantiated with logical data when it passes the boundary's check.
   - **A block is erased by itself, and checked with its crate.** What a block becomes at run time depends only on that block's own text, so rustc can expand each block without knowing the others. Checking reads every block of the crate together, and changes nothing at run time: it only accepts or rejects. So one source has two readers that agree on the program it runs: plain rustc, which expands the blocks, and Locus, which checks across them and then builds, or writes out Rust.
   - **Ghost parameters stay, as markers, in both expansions,** as in build mode. Removing them from the runtime expansion, as export mode does, would give a function two signatures, which breaks a plain-Rust caller in whichever expansion it was not written for, and would need, at each Locus call site, the callee's signature to know which arguments to drop, which a macro over one block does not have (tested).
   - **Within the crate, the semantics are build mode's.** Every rule of this RFC and of RFC 0002 that holds in build mode holds, and the litmus examples are checked with each file's text inside `locus! { }`. The two examples about `.lc` files, `include-from-source` and `reserved-check-cfg`, apply once there are `.lc` files, and an example whose `pub` items state contracts, which build mode accepts, applies as written once build mode exists (below).
   - **Other crates are not checked, so what they can reach is plain Rust.** Interfaces, contracts across crates, and checking the crates that depend on a Locus crate wait for build mode. Until then a plain-Rust crate could call a function that needs a proof, and pass a marker that proves nothing: it compiles, and reaches the division by zero the precondition rules out (tested). So in the first stage, what other crates can reach follows export mode's interface rules ([RFC 0002](0002-logical-layer.md#export-mode)): nothing that other crates can reach exposes logical data, so no function has a proof parameter or a proof in its result, and other crates reach no field that exposes logical data or that a proof reads. A sealed type may be public, also as the type of a field that other crates reach: other crates hold its values, and can neither build a value that has a proof field nor reach a field that a proof reads (RFC 0002, decisions 68, 73, and 92). `pub(locus)` means `pub(crate)` until build mode, and a function with a contract gets a plain `pub` wrapper, as in export mode. Nothing is then assumed of the crates that use a Locus crate.
   - **What a block sees.** A block sees only its own tokens, and three things follow:
     - snapshot generation is local to the type declaration; ordinary `View` implementations follow Rust coherence and may be declared separately. Deferred logical storage and unsupported foreign fields retain the encoding gates described below;
     - the library is named `locus` in the user's namespace, rather than under a fresh name, and items inside a block are macro-expanded, so a user's own item named `locus` can make the name ambiguous (tested: E0659);
     - writing a skeleton needs to know what some names are, and a block has only its tokens: whether a path names a variant of a logical enum that another block declares, and how many type parameters a logical enum of another crate has, which the second round of the skeleton spike had the program write, as `Maybe::<Nat>::Just(n)`. Which marker a `logic!` gives, when its code is a proof, is no longer among them: rustc takes it from the skeleton.
   - **Not needed in the first stage:** `.lc` files and their parser, the file loader, rewriting dep-info and `#[path]`, the compiler wrapper and its identity, checking every crate that depends on a Locus crate, and export mode.
2. **Build mode, with `.lc` files.** Locus syntax anywhere in a `.lc` file, lowering of whole files, the compiler wrapper, interfaces and contracts across crates, and every dependent crate checked. A `.lc` file lowers to what its text would expand to inside `locus! { }`, so the library, the driver, the facts, the logical pass, and validation carry over, and lowering whole files lifts the first stage's limits on what a block sees. `.lc` files exist so that the macro is not needed for the syntax (RFC 0002, decision 72).
3. **Export mode, if it is still wanted.** Readable `.rs` files for Rust users of a crate written in `.lc` files. A crate written with `locus!` blocks is already plain Rust to its users, so export mode is needed only to publish crates written in `.lc` files to Rust users.

### Sources and modes

Locus compiles one Cargo crate at a time, in the crate's real Cargo context: its edition, features, `cfg`s, and dependencies.

- **`.rs` files are plain Rust.** They are compiled as they are, and they may not name logical items: no logic functions, logical types, or proofs. The driver reports what every path resolves to, and a `.rs` path that resolves to a logical item is an error.
- **`.lc` files may also contain Locus syntax and use Locus names.** Renaming a `.rs` file to `.lc` changes nothing about the program.
- **Modules work as in Rust.** `mod name;` finds `name.lc` or `name.rs` by Rust's usual rules. When both exist, `name.rs` is the file exported from `name.lc` (below), and Locus reads `name.lc`. A crate whose root is a `.lc` file names it in its manifest, as `[lib] path = "src/lib.lc"`. Cargo accepts that (tested), and needs it, since it finds no target when `src/lib.rs` is missing.
- **Each crate has one generated module,** under a fresh name, declared by the crate root. In the lowered program it holds the prelude that makes Locus's names visible (see [Locus syntax and Locus names](#locus-syntax-and-locus-names)). In export mode, it holds the crate's own markers, `Proof` and one for each logical type that the crate's runtime types hold, and the exported program's copy holds the configuration guard too (see [Configurations](#configurations)). How export mode's two programs are kept to the same types, now that the checking program types logical code against the library's logical types, is still for the owner ([Unresolved questions](#unresolved-questions)).
- **The lowered program is never committed, and never written beside the sources.** rustc reads it at the sources' own paths, through the driver's file loader ([The lowered program](#the-lowered-program)), and a copy is kept under `target/locus/lowered/` for inspection.

**Build mode** is the default. It is for code whose builders have Locus: applications, internal workspaces, and Locus crates that other Locus crates use.
- **`cargo locus build` builds the runtime program,** and `cargo locus test` and `cargo locus run` work the same way. The runtime program lives under `target/locus/run/`, and nothing is written beside the sources. `cargo locus check` checks without building.
- **Every crate that depends on a Locus crate, directly or through other crates, is checked,** whatever its files are: the other crates of the workspace, integration tests, and examples. So every call of a function with a precondition is checked, and a crate's whole interface, contracts included, is available to the crates that use it. A procedural macro crate is checked too: it runs at compile time, but it calls Locus code. Doctests are compiled by rustdoc outside Cargo's wrapper, so Locus cannot check them yet. Until [cfg-corner-cases](../../issues/cfg-corner-cases.md) decides how, `cargo locus test` refuses to run the doctests of a crate that depends on a Locus crate, and says so.
- **Locus dependencies are checked from their sources,** so a Locus crate published for Locus users includes its `.lc` files.
- **Exactly the configuration being built is checked** (see [Configurations](#configurations)).

**Export mode** is for crates that Rust users depend on.
- **`cargo locus export` writes each `name.lc`'s runtime program beside it, as `name.rs`.**
  - Its last lines say that it is generated, name its source, and record the source's hash. They come last so that they do not move the lines above them.
  - So plain `cargo build`, rustc, rust-analyzer, docs.rs, and anyone depending on the crate see an ordinary Rust crate. They need neither Locus nor a build script.
- **Exported files are committed and published,** like the output of other code generators.
  - `cargo locus export --check` fails when an exported file does not match what its source would produce, whether it is stale or was edited by hand.
  - rustfmt formats every module file, so `cargo fmt` can make an exported file stale. rustfmt leaves a module alone when its `mod` declaration is marked `#[rustfmt::skip]` (tested), and erasure adds that mark where the declaration is in a `.lc` file. Protecting the rest, an exported crate root or a declaration in plain Rust, is [distribution-and-export](../../issues/distribution-and-export.md).
  - How an exported package's manifest names its exported crate root, and what else the package contains, is [distribution-and-export](../../issues/distribution-and-export.md).
- **The exported interface is plain Rust.** What other crates can reach has no logical values in its types and states no contracts, and no other crate can construct a value without its invariant ([RFC 0002](0002-logical-layer.md#export-mode)).
- **`pub(locus)` marks what only Locus crates see.** An item or field declared `pub(locus)` is public when the crate is built, and visible only within the crate when it is exported. Lowering and erasure write it as `pub` in build mode, and as `pub(crate)` in both programs of export mode. Plain `pub` means the same in both modes.
  - So a crate decides, item by item, what Rust users see. A function with a precondition is `pub(locus)`, and a plain `pub` wrapper checks the proposition at run time; a function with a postcondition gets a wrapper that drops it.
  - Nothing changes visibility silently. In export mode, a plain `pub` item that breaks the export rules is an error that suggests `pub(locus)`. So is a plain `pub use` of a `pub(locus)` item, which rustc itself rejects there (E0364, tested), because the checking program already writes the item as `pub(crate)`.
  - So the interface of an exported crate is its interface in build mode without the `pub(locus)` items, at the same paths, and a Rust crate that builds against the exported crate builds against it in build mode too.
- **An exported crate uses its Locus dependencies as Rust does,** through their exported interfaces, because those are what its Rust users build it against.
- **The crate is checked in the configurations it declares,** and a guard refuses the others (see [Configurations](#configurations)).
- **Declarations.** When the crate root is a `.lc` file, erasure adds the declaration of the generated module. When it is plain Rust, the crate root declares it itself, with a line that `cargo locus export` gives.

This is what makes the common case cheap. A crate that is mostly Rust adopts Locus by adding a `.lc` file. In a workspace built with Locus, nothing else changes. A library that Rust users depend on also commits its exported files, and runs `cargo locus export --check` in its CI.

### Locus syntax and Locus names

Locus extends Rust in two ways. Neither can change what a Rust program means.

**Locus syntax** is recognized from the tokens alone, without resolving any name. It appears only where rustc rejects those tokens in code it compiles.

| Form | Where | Example | Why Rust cannot mean it |
|---|---|---|---|
| Logic function | Item | `logic fn total(xs: Seq<Nat>) -> Nat { ... }` | Rust has no item beginning `logic fn` |
| Proposition declaration | Item | `prop Sorted(xs: Seq<Nat>) { ... }` | Rust has no item beginning `prop Name(` |
| Proof type | Type | `h: @(x > 0)`, `@Sorted(xs)` | `@` cannot begin a type |
| Named tuple component | Type, including a function's result | `-> (out: u8, @(out > n))`, `Vec<(i: usize, @(i < n))>` | Tuple types have no field names |
| Proof hole | Expression | `(out, _)` | rustc rejects `_` as an expression except as an assignment target, an array length, or a generic argument, and the hole is recognized nowhere else |
| Assumption | Expression | `(out, unsafe assume!(_))` | In an expression, rustc accepts `unsafe` only before a block |
| Named condition | Expression | `if small: n < 100 { ... }` | rustc accepts no `name:` between `if` and its condition ([RFC 0003](0003-proof-construction.md)) |
| Locus-only visibility | Visibility | `pub(locus) fn halve_even(...)` | rustc accepts only `crate`, `self`, `super`, and `in` paths inside `pub(...)`, and rejects anything else with E0704 (tested) |

`@` is reserved for proofs. In a tuple struct's field, `pub (locus)` with nothing after it is Rust's: a public field whose type is a path `locus` in parentheses, which rustc accepts (tested). It keeps that meaning.

- **What "valid Rust" means here: code that rustc accepts when it is compiled.** Code that `cfg` removes is only parsed by rustc, and a Locus form there has no meaning in either language.
- **Locus syntax is not recognized inside macro invocations or attribute arguments.** Those are token trees, and their meaning is the macro's.
- **Rust's own contextual keywords can precede `fn`** (`safe`, `default`, `const`, `async`, `unsafe`, `extern`). `logic` is not one of them. If Rust ever gives `logic fn` or `prop Name(` a meaning, Locus follows Rust and respells its forms.
- **[RFC 0002](0002-logical-layer.md) adds further forms** under the same rule: proof constructors and patterns (`Even::Step(m) @ h`), `final!(r)`, logical function types, and measures.

**Locus names** are ordinary Rust items:
- the macros `logic! { ... }` for logical blocks, `prop!(...)` for propositions, and `prove!(...)` for proofs, from Locus's library;
- the attributes `#[verified]` and `#[unverified]`, which state which kind a function is ([RFC 0002](0002-logical-layer.md#unverified-functions-and-unsafe-code));
- the type-based barrier `Opaque<T>`; there is no field-erasure attribute;
- the derive `Logical`;
- the trait `Snapshot`;
- the logical types and functions of Locus's standard library, such as `Nat`;
- the procedures `compute` and `linear`, and the functions of `eq`, with which proofs are built ([RFC 0003](0003-proof-construction.md));
- the runtime types that Locus's library provides in place of some of the standard library's, under `locus::std` ([RFC 0002](0002-logical-layer.md#assumptions-and-the-standard-library)). Unlike the names above, these are never in the prelude: a program imports them by path, as in `use locus::std::vec::Vec;`, and until it does, `Vec` is the standard library's.

`Logical` can only be derived, never implemented by hand. Deriving it fails unless every field and payload is logical. `Snapshot` is generated by the language and cannot be overridden. `View` is an ordinary library trait whose function is called explicitly.

**Traits among Locus's names have no methods, associated functions, or associated constants that a program can name.** In Rust, bringing a trait with a method into scope can change what an existing method call resolves to, or make it ambiguous. This was tested: a hypothetical prelude trait with a `model` method made a call to a user trait's own `model` method fail with E0034. That is why Rust adds prelude traits only at edition boundaries. `Snapshot` supplies no callable source method. A library view's logical function takes the snapshot as an ordinary argument, not a runtime receiver. In the lowered trait its stub has a reserved name, which no method call or path that a program writes can mean ([The lowered program](#stubs)).

How the names are found:
- **They are visible the way std's prelude is.** In the lowered program, every module sees a prelude that combines std's with Locus's, so rustc resolves Locus names with Rust's own rules, in both of build mode's compiles.
  - The prelude is in the lowered program's copy of the crate's generated module, and re-exports Locus's names from its library.
  - Declaring it needs rustc's internal `prelude_import` feature, which the driver enables for lowered programs only. The spike stood in for it with an ordinary glob import, so the real prelude is untested.
- **A program's own definitions and imports take precedence,** exactly as they do over std's prelude: its own `macro_rules! prove`, `use other::prove`, `struct Nat`, or a glob import of a type named `Nat`. All of these were tested.
- **Glob-imported macros are the one exception in Rust's rules.** They collide with prelude macros (E0659), and that is already true of std's `vec!`. So does a macro defined by another macro's expansion. When rustc reports such an ambiguity involving a Locus name, Locus checks the crate again without that name in its prelude. The program's own macro keeps its Rust meaning, and that crate cannot use Locus's macro of that name.
- **Recognizing Locus's names.** Lowering assumes that each of these is Locus's:
  - an invocation of a Locus macro name, or of a name that the crate's imports make an alias of one, such as `check!` after `use prove as check;`;
  - a `derive(Logical)`;
  - a path to one of Locus's logical types, such as `Nat` in `let k: Nat` or `const B: Nat = 1;`, which decides whether a binding is shown to be ghost and whether a constant is logical code ([RFC 0002](0002-logical-layer.md#logical-code)).

  The library declares each logical type as a type under its own name, so rustc resolves `Nat`, `type Count = Nat;`, and a glob import of a program's own `Nat` by its own rules.

  The driver then reports which definition each assumed name actually resolved to, by the definition's identity rather than its path. If one turns out to be the program's own, Locus discards everything from that run, errors included, and lowers again, treating it as plain Rust. A crate normally needs one rustc run, and needs a second only when names collide.
- **Locus's macros are real macros.** Lowering writes every invocation of a Locus macro that it recognizes as that construct's helper, given the typing skeleton of the logical code ([The lowered program](#typing-skeletons)). The rest reach rustc, which expands them like any other macro: inside another macro's input, such as `vec![prove!(...)]`, in a macro's output, and under an alias.
  - Locus's library defines `logic!`, `prop!`, and `prove!` as procedural macros that contain Locus's parser for logical code.
  - Such an expansion writes the typing skeleton of its logical code, with the invocation's own hygiene, in both programs. So rustc types that code, resolves its names, and borrows each place it reads, wherever the macro is expanded. In the checking program it also carries its logical code, with its spans, to the driver, so Locus checks that code like any other.
  - In the runtime program, the same invocation expands to the same skeleton, and carries nothing.
  - **An expansion need not know which roots are ghost.** In `vec![prove!(before.a == pair.a)]`, `before` may be a logical snapshot and `pair` a runtime value. With one marker a snapshot had no field `a`, so the expansion had to be told which roots were ghost, and lowering recorded the ghost bindings in scope for it in two ways (decision 20). Now a snapshot's type has its fields, and the stub that reads a place takes a logical value to itself and a runtime value to its capture. So the skeleton is written the same way for both roots, and rustc types each. Its closure borrows `pair.a` alone, so a mutable borrow of `pair.b` elsewhere is not a conflict. The first round of the skeleton spike typed its corpus's `prove!` inside `vec![..]` with no such record. So this RFC no longer keeps one for expansions, which the owner has not seen ([Unresolved questions](#unresolved-questions)).
  - **Lowering never rewrites tokens inside another macro's input.** Rewriting there would make `stringify!(prove!(x > 0))` produce the rewritten text, against [`prove-in-stringify`](../../litmus/compatibility/prove-in-stringify.accept.lc), as the spike showed. A Locus macro name that is never expanded, such as inside `stringify!`, is only tokens.
  - **A proof hole inside another macro's input is not a hole.** `vec![_]` reaches rustc as `_`, which is rustc's error; Locus suggests `prove!(...)` there.
  - Every field such an expansion can name exists in the lowered program, logical fields included, because they stay as fields of their own types, and every ghost binding stays bound to its marker.
  - In export mode, the exported program cannot depend on Locus's macros, so erasure removes every invocation as text. Whether it may depend on the runtime types of `locus::std` is open ([distribution-and-export](../../issues/distribution-and-export.md)). It can do that only for one written directly in the crate's own code, so export mode rejects invocations in any macro's output or in a function-like macro's input. An item that an attribute macro or a derive takes is the crate's own code: erasure edits it before the macro sees it, and validation compares the result ([RFC 0002](0002-logical-layer.md#export-mode)).

### How facts enter the logic

There is no wall between Rust and Locus inside a crate. Plain Rust and Locus code mix freely, and every file goes through the pipeline. Soundness comes instead from the only four ways facts about runtime values can enter the logic:

1. **Snapshots and models:** observing a runtime value as what it is, its snapshot, and explicit library views of that snapshot.
2. **Typed bodies Locus understands:** rustc's typed bodies of verified functions, translated into the logic, together with what Rust's ownership rules guarantee: a call changes only what its effect allows, which is what it is given mutable access to, while it holds it, and a mutable borrow's final value is what its place holds when the borrow ends. A function that contains `unsafe` code, handles a raw pointer, or is marked `#[unverified]` is not translated: Locus knows it by its Rust type ([RFC 0002](0002-logical-layer.md#unverified-functions-and-unsafe-code)).
3. **Assumptions:** each proposition stated with `unsafe assume!`, in the program, its dependencies, or Locus's library, listed by the audit with its place (RFC 0002, decision 80).
4. **Invariants of the program's own types,** stated by their proof fields, as described below.

Each is sound no matter who wrote the code.

**That rustc types logical code adds no fifth way.** A typing skeleton gives Locus a type for each expression of logical code, and what each name in it resolves to. It gives no fact about a runtime value: nothing in a skeleton is evaluated, and no call in runtime code is read as an expression because a trait says how. A fact is still a proof in a result, or one of the finite list of facts that the language infers from constructs it already names (decision 52).

- **Snapshots and models read only data that cannot change while the value is shared.** Rust's aliasing rules guarantee this, except for the contents of an `UnsafeCell` and the targets of raw pointers, and a snapshot holds neither.
  - A snapshot is derived from a type's definition, field by field. An explicit view is a logic function of the snapshot.
  - Types from the standard library with `unsafe` internals (`Vec`, `Rc`, `String`) get no snapshot from their fields, which do not say what they hold. A collection of Locus's library, around the standard one, keeps its contents in a logical field, and its snapshot is that struct's ([RFC 0002](0002-logical-layer.md#unresolved-questions), decisions 113 to 119). The standard library's own `Vec` has neither a snapshot nor a model.
  - The snapshots of `Cell`, `RefCell`, `Mutex`, and the atomics hold nothing of their contents. Explicit views cannot recover omitted payloads.

  So **a call taking `&x` preserves its structural snapshot only under the specified footprint and frame conditions**: Rust freezes what they hold, and they never hold what Rust does not freeze. For what `x` owns on the heap, such as a `Vec`'s elements, the freezing is the callee keeping Rust's safety contract, which the trust model lists as `assumed.unsafe.sound`: Rust's aliasing rules alone protect only what the reference itself points to ([RFC 0002](0002-logical-layer.md#the-boundary-with-unverified-code)).
- **A call keeps only the facts it cannot change.** Each verified function's summary of what it may write is inferred and kept in its crate's interface; code whose effect is unknown, a Rust crate's or an unverified function's, is assumed to write everything it is given mutably, while it holds it, and nothing afterwards, because it is taken to be sound Rust (`assumed.unsafe.sound`). A raw pointer exists only inside an unverified function, so no place of verified code changes without a write or a call that Locus sees ([RFC 0002](0002-logical-layer.md#unverified-functions-and-unsafe-code)).
- **Plain Rust in a Locus crate is still checked by Locus.** Verified code is checked by the same rules whether or not it contains Locus syntax. For example, plain Rust that assigns a field that a proof field reads is rejected, as the same assignment is anywhere else: such a field is never assigned in place, and changes only when the whole value is replaced, with every proof field proved for the new value (RFC 0002, decision 66; `contracts.invariants.assignment`). A construction in plain Rust proves the type's proof fields too, so one that takes a proof field from another value, for other field values, is rejected (`contracts.invariants`).
- **Code Locus does not translate is unverified:** dependencies, the crate's own unverified functions, trait objects, and anything else outside what Locus models. It runs, but its results are known only by their types, and anything it receives through `&mut` may come back as any value of its type.
- **Locus never believes what it did not check.** Proofs and logical values are ghost ([RFC 0002](0002-logical-layer.md)): both programs hold them only as zero-sized markers, `Proof` for every proof and a type of its own for each logical type, which carry no proposition and no value. Locus tracks what a value holds itself, only through code it verifies and its own library. No other code is given a value whose type exposes logical data, or more than a shared reference through which it can read one and not change it, and nothing logical is taken back from it (RFC 0002, decisions 61 and 76). So no Rust code, through `Any`, specialization, or anything else, can hand Locus a converted or forged proof, and Locus never relies on Rust's generics being parametric.
- **Invariants hold for every value.** Every value of a type with proof fields satisfies them. Only verified code constructs the type or writes the fields its proofs read. A field that a proof reads never changes in place, so the value is valid at every point. In build mode, every crate that can name the type is verified; in export mode and the first stage, other crates can neither construct it nor reach those fields. So a value that comes back from unverified code still satisfies its invariant: unverified code can only move it, copy it, drop it, or call the type's own verified functions. `unsafe` code could do more, and keeping the invariant is an obligation Locus adds for it, beyond Rust's ([RFC 0002](0002-logical-layer.md#unverified-functions-and-unsafe-code)).
- **Statics.** Logical code may read only immutable statics, and no static that code outside the program can write, a `static mut` or one that is `extern`, `#[no_mangle]`, or `#[export_name]`, holds logical data ([RFC 0002](0002-logical-layer.md#logical-code)).

The details of capture and views are [RFC 0002](0002-logical-layer.md).

### The lowered program

The lowered program is what rustc compiles: once with the checking expansions of Locus's macros, as the checking program, and in build mode once more with the runtime expansions, as the runtime program. A [spike](../../spikes/one-text-lowering/README.md) lowered 30 cases by hand, and compiled every one both ways with stable rustc 1.97. Six later spikes changed what the text holds ([Spikes](#spikes)): each logical type is now a Rust type of its own, and rustc types logical code.

- **rustc reads it at the sources' own paths.** The driver gives rustc a file loader that serves each `.lc` file's lowered text, and lowering points each `mod` declaration that finds a `.lc` file at that file with `#[path]`. A module that rustc loads through `#[path]` looks for its own submodules beside its file, not in a directory named after it (tested: E0583), so lowering also gives a `#[path]` to every `mod` declaration beneath one it pointed, `.rs` files included, naming the file that rustc's rules would have found without it, and the module tree is unchanged. That includes a `#[path]` the source already writes. Beneath a pointed module, rustc resolves a `#[path = "leaf.rs"]` inside an inline `mod inner { }` from another directory: `src/inner/leaf.rs` where the source meant `src/a/inner/leaf.rs` (tested). So lowering rewrites it to name the file the source meant. So a path that the source resolves from its own file, in `include_str!`, `include!`, `#[path]`, or the common `#![doc = include_str!("../README.md")]`, resolves as it does in the source, and `file!` names the `.lc` file. A copy of the lowered text is written under `target/locus/lowered/`, where it can be opened, compared, and compiled by hand when something goes wrong.
- **Lowering keeps each source line on the same line of the lowered program where it can,** and appends synthesized items at the end. Raw rustc output is then readable when debugging Locus itself. The origin map, not the alignment, is the authority.
- **Locus's library crate holds the logical types and `Proof`, its macros, the stubs that logical code is typed against, and `Snapshot`.** Its macros expand one way in the checking program and another in the runtime program, selected by a `cfg` that only the checking compile sets ([Running rustc](#running-rustc-locus-driver)). The two expansions differ only in what they carry to the driver: no type, no implementation, and no typing skeleton is under that `cfg`. rustc receives the library under a crate name the program does not use.
- **New names are fresh.** Every name Locus introduces for one crate alone, such as the library's crate name and the generated module, is chosen so that it appears nowhere in the crate's sources, so generated items cannot collide with the user's, even under `#![no_implicit_prelude]`. One fixed name is `__locus_check`, the `cfg` that selects the checking expansions, which the shared library is compiled with. A crate's sources may not test it ([Running rustc](#running-rustc-locus-driver)). Another is `locus`, in the path `locus::std` that a program writes to import the library's runtime types; what a program that already has a crate of that name does is for [distribution-and-export](../../issues/distribution-and-export.md).
- **Names that must be the same in every crate are reserved.** A name that one crate writes and another must find cannot be chosen fresh in each: the stub of a trait's logic function is such a name ([below](#stubs)). Locus reserves two prefixes for these, `__logic_` and `__locus_`, and a program may not write an identifier that begins with either (`compat.names.reserved`). The second prefix covers `__locus_check`. This is one more exception to "every Rust program is a Locus program", beside the `cfg`: a spike found a program that wrote such a name, a method `__logic_answer` in a trait with a logic function `answer`, and rustc reported an ambiguity or a duplicate definition about a name the program never saw in its own source (E0034, E0428; [marker-lowering](../../spikes/marker-lowering/README.md), `r07` and `r08`).
- **The checking program is compiled with `--cap-lints allow`.** Lints there would be artifacts of the lowering, and a user's `#![deny(warnings)]` or `forbid` must not turn synthesized code into errors. This was tested. Real lints come from building the runtime program, where lowering marks the items it synthesizes, and ghost bindings and fields, with `#[allow]` for the lints they would otherwise cause.

#### A Rust type for each logical type

Each logical type is a zero-sized Rust type of its own, the same type in both programs: `Int`, `Nat`, `Bool`, `Prop`, and `Seq<T>` of Locus's library, each logical struct or enum that a program declares, and each snapshot type. So rustc tells `Int` from `Nat`.

- **rustc resolves traits on logical types as on any Rust type:** implementations, bounds, associated types, implementations with bounds, and the orphan rule. A [spike](../../spikes/logical-markers/README.md) showed it with Rust written by hand, in twenty cases, and a [second](../../spikes/marker-lowering/README.md) with a lowering that a macro performs: the 20 accepted litmus examples that the one-marker lowering compiled still compiled both ways, with no source changed.
- **With one marker it could not.** Two implementations of one trait, for `Int` and for `Nat`, were two implementations for one Rust type (E0119). An associated type could not differ between two logical types. A program could not give a standard trait to its logical type, which was the library's marker (E0117). And `Int::from(nat)` already resolved, to the standard library's `impl<T> From<T> for T`: rustc took the conversion to be the identity ([logical-markers](../../spikes/logical-markers/README.md), `r01` to `r05`).
- **rustc now rejects what only Locus rejected.** A `Vec<Nat>` used as a `Vec<Seq<Int>>` is E0308. With one marker both were `Vec<LogicalData>`, and only Locus's own typing told them apart.
- **A logical struct is a zero-sized struct with its fields,** each of a zero-sized type, so rustc types `p.first` and `Pair { .. }` as it types any struct.
- **A logical enum is a zero-sized struct too,** and its variants are those of an enum beside it, which only logical code reaches, through the type. A Rust enum of two variants has a tag, one byte, in a runtime struct too, and Rust rejects a type that contains itself with no indirection even when it is zero-sized (E0072), which a logical `List` does. Construction, `match`, and rustc's check that a `match` leaves no variant out work through the second enum ([skeleton-typing](../../spikes/skeleton-typing/README.md), [second round](../../spikes/skeleton-typing-2/README.md)).
- **Every proof is one marker, `Proof`.** A proposition is not a Rust type: it mentions runtime values, so `@(y != 0)` is a different proposition for each `y`. So `@P` is `Proof` for every `P`, rustc never computes a type that involves a proposition, and Locus alone checks every proof against the proposition its position requires, including the proof for every proof parameter at every call.

What rustc sees:

| In the source | In the lowered program |
|---|---|
| Runtime code | Copied unchanged |
| A parameter, local, or result whose type is logical, proof parameters included, and every argument for one | Kept, with its type: `fn halve(n: u32, even: Proof) -> u32`, `fn count(n: Nat) -> Nat` |
| A proof type `@P` | `Proof![P]`, a type macro of the library that is `Proof` |
| A logical type written as a path, such as `Nat` or `Seq<Nat>` | Kept. It names a type of the library, or of the program, under its own name, so rustc resolves the path by its own rules |
| `Snap<T>` | Kept. It is an alias of an associated type, which rustc normalizes ([below](#snapshots-and-models-as-rustc-is-given-them)) |
| A field whose type is logical, a logical type argument, a logical tuple component | Kept: `Vec<Nat>`, `Option<Proof>`, `(u32, Proof)` |
| A named result `(out: T, @P)` | `(T, Proof)`: tuples keep their shape |
| A call of a runtime function whose result is entirely ghost | Kept as written: it returns the marker |
| A logic function or `prop` declaration | A stub function with the same name, generics, and signature, whose body holds the typing skeleton of the function's body and then panics, since it is never run |
| A logic function that a type or a trait declares | A stub in the lowered `impl` or trait, under a reserved name |
| A `#[derive(Logical)]` type | A zero-sized type under its own name, with its fields |
| A struct or an enum that is not logical | Kept, with a snapshot type and its implementations written beside it ([below](#snapshots-and-models-as-rustc-is-given-them)) |
| An `impl View` | An ordinary trait implementation with its logical function stub; no default implementation is generated |
| A proof hole `_`, an assumption `unsafe assume!(…)`, and `final!(r)` | An invocation of a library macro that gives the marker; `final!` reads no place |
| A `logic!`, `prop!`, or `prove!` invocation that lowering recognizes, a proposition in a type annotation inside a body, a call of a logic function, or an operator on bindings shown to be ghost | The helper for that construct, given the typing skeleton of the logical code as a closure: `::locus::logic::prove(\|\| ..)`. It never calls the closure, and gives the marker of the closure's type |
| Any other invocation of a Locus macro | Expanded by rustc, as [Locus syntax and Locus names](#locus-syntax-and-locus-names) describes, to the same helper and skeleton |
| `#[verified]` and `#[unverified]` | Attributes of Locus's library that expand to their item unchanged, in both programs. An exported program has neither |
| An `Opaque<T>` field | Kept as an owning wrapper; its snapshot component is `Unit` by type identity |
| A proposition in a signature or a struct field's type | Inside `Proof![...]`, with a typing skeleton where its reads are always valid ([below](#observations)) |
| `pub(locus)` | `pub` |

#### Typing skeletons

Logical code is not Rust. Its operators mean different things from Rust's: `+` on `Nat` cannot overflow, and a comparison gives a proposition. It reads a runtime place as its snapshot. And half of its operators, `===` and `==>` among them, are not Rust's at all. So lowering does not give it to rustc as it is written. It writes each piece of logical code a second time, as a Rust expression over stubs of Locus's library, in a closure that is never called. That expression is the piece's **typing skeleton**, and rustc types it.

```rust
let before = logic! { v.elements().len() };
// is lowered to something like
let before = ::locus::logic::logic(|| ::locus::logic::capture(&v).__logic_len());
```

Here `locus` stands for the name under which the crate receives the library.

- **What has a skeleton:** the contents of a `logic!`, a `prove!`, or a `prop!`; a proposition in a type, in a `let` annotation, a signature, or a struct field; the body of a logic function; an arm of a `prop` declaration; and a call of a logic function or an operator on bindings shown to be ghost. Every piece of logical code has one. In the [first round](../../spikes/skeleton-typing/README.md) of the spike, a translation that read only tokens gave a skeleton to each of the 51 pieces of logical code in its corpus of 26 files, 22 of them litmus examples, and rustc typed them all, in both programs, with no source changed. The [second round](../../spikes/skeleton-typing-2/README.md) added the shapes the first had left out: a proposition in a field's type, a contract on a trait method with no body, a nested variant pattern, both kinds of model, and two crates.
- **What it is made of.** A place that is read becomes a stub applied to a borrow of the place, whose result has the type of the place's capture, or of its snapshot under `snap` ([RFC 0002](0002-logical-layer.md#snapshots-and-models)). An operator becomes a call of a stub, never Rust's own operator: Rust fixes the result of `<` as `bool`, `Add::add` is a method that runs, and a stub's bound can carry a message. A literal, an `if`, a `match`, a quantifier, a cast, and a hole each have a stub. A call of a logic function stays a call, since its stub has that name.
- **It never runs.** The helper takes the closure, drops it, and gives the marker of the closure's result type. Lowering writes a stub's name only inside a skeleton, so no stub is called, and a stub's body panics in case one is. In the first two rounds of the spike, no binary that compiled both ways reached a stub. The toy lowering did not yet put a lemma called outside a `logic!` inside a skeleton, which RFC 0002 makes logical code: a first draft of one specification called one from runtime code, reached the stub, and panicked (second round).
- **It is the same text in both programs.** No `cfg` selects it. So the two programs give logical code, and every ghost value, the same types.
- **A ghost local with no annotation takes its type from it.** The helper returns the type of the closure's result, so `before` above is a `Nat` to rustc, forward from the logical code, in both programs, and nothing is guessed. In the first round the skeleton gave their types to all 13 ghost locals of the corpus that a `logic!` initializes, 8 of them with no annotation, and to 18 of the 19 in the litmus examples with an unannotated ghost `let`; the other is in a rejected example whose logical code calls a runtime function. No litmus example needed an annotation.
- **It decides which marker a `logic!` gives.** A `logic!` whose code is a proof, such as `logic! { add_zero(n) }` for a lemma `add_zero`, has a skeleton of type `Proof`. Lowering no longer has to know it from the source, which did not show it.
- **It is the observation.** The closure borrows each place it reads, shared, by rustc's own capture rules, and is dropped at once ([below](#observations)). No separate observation is written.
- **It resolves the names.** rustc resolves each name in it, and each trait: which implementation a logic function comes from, and under which bound. No separate probe is written. A member named through its trait, `OrderSpec::le(r, r)`, which no probe could write, resolves once the stubs are in the trait, since the arguments give `Self`.
- **A borrow error or a type error in logical code now stops a plain build as well as the check.** The skeleton is in the runtime program too. In the first round, the two rejected litmus examples whose logical code reads a moved or a mutably borrowed place failed in both programs (E0382, E0502), where the earlier lowering failed them in the checking program alone. A program whose logical code does not type or borrow-check has no runtime program.
- **Where the translation needs a name's meaning.** A skeleton is written from tokens, and some of what it writes depends on what a name is: whether a path names a variant of a logical enum, whether `A::b(..)` is a logic function of a type or a function in a module, whether a name is a place. The spikes' translations guessed from capital letters, and each README lists its guesses. Lowering decides them as it decides which names are Locus's: from the crate's items and imports, confirmed against the facts ([Locus syntax and Locus names](#locus-syntax-and-locus-names)). No spike built that.

The driver recognizes a skeleton by the identity of the helper it is given to, in Locus's library, never by a path written in the source. The exact form of each skeleton is part of the lowering's specification and is tested against rustc.

#### Stubs

Stubs are what logical code is typed against. Nothing runs one.

- **A logic function's stub has the function's real signature,** over the logical types' own Rust types, with `Proof` for each proof. So rustc checks every call of a logic function in logical code against it: a `Nat` given where a `Seq<Nat>` is wanted is E0308 at the user's tokens, with "function defined here" at the user's `logic fn`.
- **A stub's body is the skeleton of the function's body,** typed against the signature, and then a panic. Stub bodies panic rather than loop, so that a mistake fails rather than hangs.
- **A trait's logic functions have their stubs in the lowered trait,** and an `impl`'s in the lowered `impl`, each under a reserved name: `__logic_` before the function's own name in the spikes. So a bound `T: OrderSpec` gives rustc the stubs for `T`, an import of the trait brings them, and rustc checks that each `impl` gives each member.
  - **A hidden trait beside the lowered one does not work.** It was the first placement tried: the stubs in a second trait that the lowered trait has as a supertrait. `use spec::OrderSpec;` does not import the hidden trait, so an `impl` in another module cannot name the trait it must also implement (E0405), and `Rational::le(..)` in logical code does not find its stub (E0599) ([marker-lowering](../../spikes/marker-lowering/README.md), `t13a`, `t13c`, `t13d`, `t17`). Across two crates the same failure stopped the build ([second round](../../spikes/skeleton-typing-2/README.md)). With the stubs in the trait, each of those cases compiles both ways, and so do the two crates of the second and third rounds.
  - **A reserved name is what keeps a stub out of Rust's own resolution.** A stub is a member of the trait to rustc, and a member is a candidate when rustc resolves a method call or a path that Rust code wrote. No program writes a reserved name (`compat.names.reserved`), so a stub is a candidate for none ([marker-lowering](../../spikes/marker-lowering/README.md), `t19`).
  - **The cost:** plain Rust code cannot implement a trait that has a logic function, since it cannot write the stub.
- **Runtime code never refers to a stub,** so no stub is ever a runtime value. Only skeletons and other stubs mention them. Locus checks that from the facts.
- **A stub type's generic parameters are carried in a `PhantomData` field.**
- **Stub functions are `const fn` where Rust allows,** so that logical code can stand in a constant context. No spike typed logical code there.

#### Snapshots and models, as rustc is given them

The heading preserves earlier links. Capture is now the snapshot; `View` is an ordinary explicit library trait ([RFC 0008](0008-snapshot-capture-and-views.md)).

- **One snapshot family.** The protected library trait `Snapshot` carries an associated logical type; `Snap<T>` names it. A logical type snapshots to itself. Both lowered programs receive the same nominal types, implementations, bounds, and typing skeletons.
- **Structural generation.** For each supported source aggregate, lowering writes a logical snapshot aggregate over the normalized snapshots of its fields, preserving visibility, variants, proof fields, and declaration-fixed cuts. Snapshot types contain no source lifetime. `Opaque<T>` and `UnsafeCell<T>` contribute `Unit` without a payload snapshot bound; there is no field attribute for erasure.
- **Explicit views.** `View: Snapshot` has `type V: Logical` and `logic fn view(s: Snap<Self>) -> Self::V`. Users implement it as an ordinary Locus trait under Rust coherence. There is no compiler-generated identity view, implicit conversion, or same-block requirement. A view must have a snapshot available for its body.
- **Conditional generation.** Generated snapshot implementations are conditional on included field snapshots. Missing support must reject capture without rejecting ordinary Rust declarations. The hand-written two-trait `capture-family` cases support this separation, but the higher-ranked-lifetime form that defers otherwise concrete unsatisfied bounds remains an encoding gate, tracked in [unmarked-fields-and-the-superset](../../issues/unmarked-fields-and-the-superset.md). There is no blanket foreign fallback or assumed specialization.
- **Deferred logical storage.** A logical field such as `contents: Seq<Snap<T>>` is stored using an internal nominal `SnapOf<T>` carrier requiring no bound on the runtime struct. Under `T: Snapshot`, reads expose the source type `Seq<Snap<T>>`; writes preserve the exact value, with `read(store(s)) = s`. Field loans, helper calls, assignments and destructuring preserve place identity. Frozen snapshots erase source lifetimes. This is internal storage elaboration, not the false equation that an ordinary logical sequence changes type when captured. The hand-written carrier is spiked; generation and logical value preservation remain validation gates.
- **Unsupported cases.** Generic recursive snapshots, associated-type cuts, and unions require a uniform representable encoding. No spike establishes all of them. The current implementation must reject unsupported logical capture while preserving ordinary runtime use.

#### `Snapshot` is an `unsafe` trait

The compiler-owned `Snapshot` carrier is unsafe in lowered Rust so ordinary safe Rust cannot forge it. Only lowering and designated library support supply implementations; source code cannot override one, even with `unsafe impl`. A sealed Rust trait would prevent cross-crate generated implementations, so the driver checks implementation provenance instead. This applies in every dependency, not merely verified modules, and provenance is never inferred from a writable identifier.

`View` is an ordinary checked Locus trait. It is neither a protected snapshot implementation nor an implicit capture policy. Its logical stub has the same reserved-name protection as other logical members. User implementations remain subject to the existing logical-trait boundary rules.

#### What lowering decides

**Lowering decides only what the source shows:**
- Locus syntax, from the tokens;
- which names are Locus's, from the crate's items and imports, confirmed against the facts;
- which bindings are shown to be ghost ([RFC 0002](0002-logical-layer.md#logical-code)), from lexical scopes;
- which places logical code reads, from its syntax.

It never needs an expression's type or a callee's signature. rustc resolves the rest with the markers in place, as the one-text spike showed:
- `gates.first().unwrap().admit(n, pos)` resolved to `Gate::admit`, and `Some(ok)` to `Option::<Proof>::Some`, with no help from lowering.
- The traps of removing ghost arguments before rustc became loud. With an inherent `go(&self, a, h: @P)` and a trait method `go(self, a)` in scope, `x.go(1, h)` is E0061, where the previous design silently called the trait method. A proof passed to `make<T: Default>` is E0277, `Proof: Default`, where it silently instantiated `make::<()>`.

#### What rustc decides, and what Locus checks after it

**rustc decides,** in both programs:
- every type of logical code, and so the type of every ghost value, apart from which proposition a proof proves;
- every trait implementation, every bound, and every associated type, on logical types as on runtime ones;
- what each name in logical code resolves to;
- whether logical code may read each place it reads.

Locus reasons with these answers and computes none of its own. It has no type checker for logical code: it reads the types rustc gave each skeleton, and elaborates the logical code itself with them.

**Locus checks after rustc,** from the facts:
- **Each proposition and proof.** rustc sees one marker for all of them. `Option<@P>` and `Option<@Q>` are one type to it, so Locus checks that two types that hold proofs are the same wherever one must match another (`logic.ghost.type-identity`), and that every proof proves the proposition its position requires.
- **The rules that syntax cannot decide and rustc does not make.** Lowering sees tokens, and these need resolved names or types. The spikes list them:
  - **Which headers an implementation may have** (`logic.ghost.no-trait-impls`). Lowering rejects a logical type that a header writes. Behind an alias, as in `type Shot = Snap<u32>; impl Describe for Shot`, it sees only the name `Shot` ([marker-lowering](../../spikes/marker-lowering/README.md), `h04`).
  - **The checks on declarations that need resolved names.** That each parameter of a logic function is a logical type (`logic.fn.parameters`): the third round's test went by the type's name, and took a type parameter, and a type of another block or crate, to be logical.
  - **Where Rust's typing accepts more than Locus's rules.** rustc types `p.f` through an overloaded `Deref` in a skeleton, which RFC 0002 excludes. It infers a ghost local's type backward from a later use, which `logic.ghost.types-forward` excludes. It does not check that a variant pattern inside another leaves no variant out.
  - **That no path outside a skeleton names a stub.**
- **The ghost checks,** as before: the boundary with unverified code, sealed types, invariants, and the rest of what [the trust model](../TRUST.md) lists under `trusted.ghost-checks`.
- **That the two programs agree** ([Validation](#the-runtime-program)).

None of these checks exists yet. The spikes typed programs with rustc and proved nothing.

**What Locus learns from the facts.** The one-text spike's driver reported, from rustc's type-checking results, even when they contain errors:
- the callee of each call, with its generic arguments;
- which arguments fill parameters of marker type, and which generic parameters markers flow into;
- which locals hold markers;
- which functions with marker parameters are used as values, and as what type.

The skeleton spikes' driver also listed, for each call in a skeleton, the item it resolved to and the implementation it comes from. From these, Locus:
- reads the logical type of each ghost value, which rustc now knows, and works out only which proposition each `Proof` stands for, forward from signatures and annotations;
- confirms each assumption lowering made, by the identity of the definition each name resolved to;
- rejects what rustc accepts but Locus must not. rustc rejects coercing `halve` to `fn(u32) -> u32` (E0308) and `[7].map(halve)` (E0593) by arity, but accepts `black_box(halve)`, a pointer of type `fn(u32, @P) -> u32`, and `fold(10, halve)`. RFC 0002 rejects these.

#### The two programs have the same types

**The two programs have the same types everywhere, so rustc resolves every call alike in both.** The design rests on this. What is checked is the checking program, and what runs is the runtime program: if a call resolved to one item in the first and another in the second, a proof would be about code that does not run.

- **What establishes it.** Both programs are one lowered text. Each logical type, `Proof`, each stub, each snapshot type, and each implementation is the same item in both, from one library and one text. The `cfg` that tells the programs apart selects no type, no implementation, and no skeleton, and a program may not test it. So every expression has the same type in both, and rustc's resolution, which goes by types, gives the same answer. Validation then compares every item and typed body of the two, so a difference from any other cause is an error.
- **What only tests it.** The spikes' driver lists, for each call, method call, and associated constant that the source wrote, the item rustc resolved it to and the implementation it comes from, in each program. No line differed: over 100 binaries and 387 lines in the [first round](../../spikes/skeleton-typing/README.md), 126 and 577 in the [second](../../spikes/skeleton-typing-2/README.md), and 151 and 718 in the [third](../../spikes/skeleton-typing-3/README.md). A listing shows that those binaries agree. It does not show that every program does, and in generic code it compares a trait's item, so it does not see a difference that appears only at an instantiation.
- **Two encodings broke it, and were rejected.** Both kept one marker where a value's type is written ([marker-lowering](../../spikes/marker-lowering/README.md), the two later variants).
  - **One marker at run time, and distinct types when checking.** Of the 48 cases that both programs accepted, 5 resolved a call differently. With an inherent method for `Wrap<Int>` and a trait's method for every `Wrap<T>`, a call on a `Wrap<Nat>` reached the trait's method in the checking program and the inherent one in the runtime program (`r10`). In `r11` the runtime program took the branch that, as checked, holds a proof of `False`. The order of autoref and of deref, and an associated constant, did the same (`r09`, `r13`, `r14`). `TypeId` told `Int` from `Nat` in one program and not in the other (`r15`), which no listing of calls sees.
  - **Implementations placed on stubs.** Values kept the one marker, and a distinct stub type stood only where an `impl` names a logical type, with questions about stubs put to rustc through a driver. The two programs then agreed with each other and not with the logical types: rustc resolved a call on a `Wrap<Nat>` to the method written for `Wrap<Int>`, in both programs, and reported nothing (`r10`, `r11`, `r13`, `r14`). Two implementations that differ only in a logical type inside a runtime type collided (E0119: `r01`, `r02`).

#### Fields and bindings

**Logical and proof fields stay, as fields of their own types,** in structs and enum variants, in both programs. So fields keep their positions, generic parameters stay used, and logical code that reads or writes such a field is checked as a borrow of that field alone.
- Constructing the type supplies the marker for the field, and a pattern that binds it binds the marker.
- Reading the field is logical code, and is observed as a shared borrow of the field.
- An assignment to a logical data field is a ghost statement. Its right-hand side is logical code, which gives the marker, so rustc checks the write as a write, and now checks its type too.
- A derive sees the field at its own type. On a proof field, `Proof` lets every standard derive but `Default` work; the one-text spike's eight derives on a struct with a proof field compiled and behaved. On a logical data field, the logical type's own marker makes `PartialEq`, `Hash`, `Ord`, and `Default` fail (E0369, E0277), as RFC 0002 intends. Others, such as serde's, fail on either kind unless told to skip it.

**Logical code contains no runtime evaluation.** It reads places, and it never calls runtime functions, mutates, or has effects; the arguments of a logical call are logical code too. A runtime function that returns a proof is runtime code: it is called, and kept, in both programs.

**Ghost bindings hold markers in both programs,** so that later logical code can name them. Each has the type that rustc gave its skeleton or that its annotation wrote.

#### Observations

**Observations are how rustc checks what logical code reads.** A place that logical code reads is borrowed, shared, by the closure of the code's skeleton, where the logical code stands, and the borrow ends at once, when the helper drops the closure. So rustc's borrow checker rejects logical code that reads a value after it has moved (E0382), a place while it is mutably borrowed (E0502), or through a reference that is no longer live, and it does so in both programs. Observations follow Rust's rules exactly:
- two-phase borrows apply as they would to any shared read;
- a closure borrows only the fields it reads, so reading `pair.a` does not conflict with a mutable borrow of `pair.b`. In an edition before 2021 a closure borrows the whole variable, so the read is coarser there, which only rejects more;
- as in Rust, code after a `return` is not borrow-checked, and logical code there is never evaluated;
- as for Rust's field access, the place's type must be known where it is read. Logical code that reads `p.a` before `p`'s type is inferred is an error that asks for an annotation (E0282).

Where a skeleton stands:
- **Logical code in a body is typed and observed where it is written.**
- **A proposition in a signature is typed at the function's entry.**
  - A precondition reads parameters at entry, which are always readable.
  - A postcondition reads parameters at entry, the function's results, and the final values `final!(x)` of its `&mut` parameters ([RFC 0002](0002-logical-layer.md)). These are well defined however the body exits, so it does not read anything at a point in the body. Its skeleton takes the result as the closure's parameter.
- **A proposition in a struct field's type reads the struct's own fields,** and is typed in a function of lowering's own that borrows the struct. A proposition of a trait method with no body is typed in a hidden method beside it, with the same parameters (second round, `d1` and `d2`).
- **A proposition in a `let` type annotation is read after the initializer,** where the `let`'s bindings come into scope, and is typed and observed there.
- **Logical code inside a closure or an async block is rejected for now:** a skeleton there would add its borrows to what the closure or future captures. A ghost binding named there would become a capture of its marker too.
- **Logical code that reads a packed field is rejected for now:** Rust does not allow a shared borrow of one.

Both cases are [closures-async-packed-macros](../../issues/closures-async-packed-macros.md).

Method calls and indexing in logical code are resolved on logical types, by rustc, on the type of the captured value. They never resolve to runtime methods: a logic function's stub has a reserved name, which no runtime method has. How a place is captured is [RFC 0002](0002-logical-layer.md).

#### Names that rustc can show

**Every name that lowering writes and that rustc can show reads as the programmer thinks of it.** This is a rule of this RFC, and lowering and Locus's library are held to it. rustc shows names in its messages, and a program can print them, through `Debug` and `type_name`. A plain `cargo build` prints rustc's messages as rustc writes them, with no Locus to reword them, so the names themselves have to read well.

| What lowering writes | Its name |
|---|---|
| The marker of every proof | `Proof` |
| A logical type | Its own name: `Nat`, `Seq<Nat>`, the program's `Rational` |
| The helper that takes a skeleton | The construct's name: `logic` for `logic!`, `prove` for `prove!`, `prop` for `prop!` |
| The stubs that a skeleton calls | In the library's module `logic`, named for the operation: `locus::logic::add`, `locus::logic::capture` |
| The library's helper traits | Named for what they ask, each with a message that says what to write, through `#[diagnostic::on_unimplemented]`: `` `T` cannot be read in logical code ``, with rustc's own help to add the bound `T: Snapshot` |
| The stub of a logic function `le` of a type or a trait | `__logic_le` |
| The snapshot type of `Stack` | Named for its type: `Stack_Snap` in the third round |
| The variants of a logical enum `Tree` | `Tree::Leaf`, which rustc prints as `_::Tree::Leaf` |

- **The second round measured it,** on five deliberate errors. A `Nat` where a `Seq` is wanted, and an unknown field, read as if the logical code were Rust, and mention nothing of the encoding. A missing operator reads `` `+` is not defined on a `Rational` and a `_` in logical code ``, where the first round had ``the trait bound `Rational: AddLogic<_>` is not satisfied``.
- **What still shows the encoding.** The [second](../../spikes/skeleton-typing-2/README.md) and [third](../../spikes/skeleton-typing-3/README.md) rounds list it:
  - `__logic_room` in "no method named", and with it the snapshot type's parameters: ``no method named `__logic_room` found for struct `Stack_Snap<F0, F1>` ``;
  - "borrow occurs due to use in closure" in a borrow error of logical code;
  - `_::` before a logical enum's variants, which is the anonymous constant that holds them, and rustc's suggested `match` arm, which is garbled;
  - a helper type of the library, `NoSuchOperand`, in an operator's error, which rustc also reports twice;
  - notes that do not fit: a note of `on_unimplemented` cannot depend on the type, so the note for a type parameter is also printed for a type of another crate, and rustc adds "use parentheses to call this closure" where a closure has no model;
  - the name of the trait that defers a check, in a note, where a struct with an unmarked field is read;
  - the name of a derived model's type, `Point_Logic` in the second round.
- **`cargo locus` still maps and rewords** rustc's diagnostics ([Diagnostics](#diagnostics)). The principle is for what it does not reach: a plain build, an editor that runs rustc, and what a program prints.

#### Costs and limits

- **What the programmer writes.** Rust has no implicit conversions and no universal trait, so typing logical code by Rust's rules made some things explicit that RFC 0002's earlier rules left implicit: `as Int`, a written `T: Snapshot`, `Opaque<T>`, explicit view calls. They are the reconciled rules now, with what each costs: its decisions 97 to 110.
- **Build time.** In the [first round](../../spikes/skeleton-typing/README.md), on its corpus of 26 small binaries, the difference was inside the noise: about 2 seconds with skeletons and without. On one generated file of 500 functions and 2,001 pieces of logical code, skeletons added 0.18 to 0.27 seconds to builds of 0.43 to 0.50 seconds, about 0.1 milliseconds for each piece (medians of 5, debug builds, one machine). The second and third rounds added a snapshot type and implementations for each declared type, and did not measure again.
- **Run time is not measured.** A skeleton's closure is built and dropped, and never called. No logical code is evaluated. Whether building it leaves anything in an unoptimized binary, and the size of a binary with them, were not looked at: every build of the spikes was a debug build.
- **A plain build can fail for logical code.** That is the other side of rustc typing it: a type error or a borrow error in logical code stops `cargo build`, where before only the check reported it.
- **Lints change.** rustc now sees what logical code reads. In the first round, the runtime program lost `unused variable` for a parameter that only a proposition reads, and `value assigned is never read` where only a later proposition reads the value. A skeleton also brought a lint of its own, `unnecessary parentheses`, which lowering has to silence.
- **rustc's message says what to write only where the failure is a missing bound.** A missing `Snapshot` bound, a missing operator, and a type that cannot be read each have a message of the library's, and rustc's own help adds the bound. Everywhere else rustc reports a mismatch of two types, which is clear and gives no advice, and for a generic argument it asks for a type annotation, which misleads (second round). `cargo locus` can reword those from the facts. A plain build cannot.
- **A foreign trait's generic method.** In an implementation of a trait that another crate declares, a value of a method's own type parameter cannot be captured: without the bound it is E0277, and an implementation may not add a bound that its trait does not have (E0276; third round, the `d2` cases, with `Extend::extend<I>`). RFC 0002 states this as a limit (decision 106). Lowering cannot lift it, because it inserts nothing and rustc decides.
- **The deferred form rests on rustc not checking a `where` clause behind a higher-ranked lifetime where an implementation is declared.** It does not on rustc 1.97.0, and nobody has established that as intended. If it cannot be relied on, a struct with an unmarked field of a type that Locus cannot see is an error at its declaration, which is an exception to commitment 1 of the vision, or lowering needs types before rustc runs, which is staged typing ([Alternatives](#alternatives)). [unmarked-fields-and-the-superset](../../issues/unmarked-fields-and-the-superset.md) is open for it.
- **A trait with a logic function cannot be implemented by plain Rust code,** as above.

Neither the lowered program nor the checking program is ever shipped.

### Configurations

A configuration is one way of building the crate:
- the rustc that Locus is pinned to;
- one target;
- one set of enabled features;
- one set of `cfg` values, including `debug_assertions` and `test`, and any that build scripts or `RUSTFLAGS` set;
- the environment that macros such as `env!` and `include!` read;
- the resolved versions of the crate's dependencies, and their features.

rustc removes code whose `cfg` is false before it type-checks anything, so checking one configuration says nothing about code that configuration removes. Locus therefore treats each configuration as a different program, and a proof holds in the configurations it was checked in.

- **In build mode, Locus checks exactly the configuration being built,** which it reads from Cargo's invocation of the compiler, and builds what it checked. Nothing needs guarding.
- **In export mode, Locus checks the configurations the crate declares,** because Rust users build the exported files in configurations Locus never sees. How a crate declares them is [distribution-and-export](../../issues/distribution-and-export.md). `cargo locus export` checks each one, and passes only if all of them pass.
  - **The default** is the dev, release, and test builds, with default features, for a target with 64-bit pointers. These are what `cargo build`, `cargo build --release`, and `cargo test` build.
  - **The default does not depend on the machine that runs the check.** For now, an exported crate whose verified code tests other properties of the target, such as `target_os`, must declare its configurations.
- **Code that no checked configuration enables is removed** from `.lc` files, in either mode, since it was never checked. In `.rs` files, which export does not edit, the guard refuses every configuration that would enable it.
- **One exported program for all of them.** In export mode, erasure writes one runtime program, which must be right in every checked configuration.
  - A `cfg` attribute that every checked configuration satisfies is dropped. Every other `cfg` attribute is kept as written.
  - Erasure's decisions that depend on facts, such as whether an import names only logical items, must agree in every checked configuration in which that code is active, or the check fails.
- **The exported program refuses to compile in any other configuration.** The crate's generated module holds a guard that turns every unchecked configuration into a `compile_error!` naming the checked ones:

  ```rust
  #[cfg(not(any(
      all(target_pointer_width = "64", not(feature = "fast"), debug_assertions, not(test)),
      all(target_pointer_width = "64", not(feature = "fast"), not(debug_assertions), not(test)),
      all(target_pointer_width = "64", not(feature = "fast"), debug_assertions, test),
  )))]
  compile_error!("this crate was checked by Locus only in its declared configurations; ...");
  ```

  - A `cfg` test can only examine the names it mentions. So the guard tests every `cfg` name that appears anywhere in the crate's sources, `.lc` and `.rs` files alike. It also always tests `target_pointer_width`, which fixes the range of `usize`, and `debug_assertions`, which std's own macros test.
  - It takes these names from the sources, not from the output, which no longer mentions code that erasure removed.
  - Plain Rust counts too. Export leaves `.rs` files as they are, so code in one that no checked configuration compiles was never checked, and in another configuration could call a function whose proof parameters export removed, as `#[cfg(feature = "fast")] fn quick() -> u32 { crate::needs_proof(0) }` would. So a mostly-Rust crate declares the configurations it supports, rather than keeping all of them.
- **The cost is Cargo's feature unification.** A downstream crate can enable any combination of an exported library's features. If that combination changes what verified code sees and was not checked, the downstream build fails with the guard's message. Library authors therefore declare the combinations they support. The alternatives are worse:
  - checking one configuration and trusting the rest is unsound;
  - checking every combination is infeasible;
  - banning `cfg` in verified code rules out ordinary platform-specific code.
- **Behavior that differs between builds without a `cfg`** is the runtime semantics' to handle. Integer overflow, for example, depends on the `overflow-checks` setting, which no stable `cfg` can test, so a proof must hold either way ([runtime-semantics](../../issues/runtime-semantics.md)).
- **What the guard cannot test** becomes an assumption about exported crates in [the trust model](../TRUST.md):
  - the compiler that builds the exported program;
  - the versions and features of dependencies.

  The distribution RFC ([distribution-and-export](../../issues/distribution-and-export.md)) will narrow these. The inputs to macros that read the environment or files are an assumption for exported crates, which Rust users build in environments Locus never sees. In build mode, validation compares the two compiles, so a macro whose input changed between them is an error.
- **Corner cases.** [cfg-corner-cases](../../issues/cfg-corner-cases.md) collects the cases this design does not yet settle, among them `cargo test --release`, docs.rs's `docsrs`, target features, build-script `cfg`s, and `cfg`s written by other crates' macros.

### Running rustc: `locus-driver`

`locus-driver` is a small program linked against rustc's internal crates, the way Clippy is.

- **How Cargo runs it.** Cargo runs it in place of rustc, through `RUSTC_WRAPPER`, so that it sees every crate in the build, including Locus dependencies outside the workspace, which `RUSTC_WORKSPACE_WRAPPER` does not wrap ([spike](../../spikes/cargo-graph/README.md)). Cargo does the orchestration, as for any build, and Locus gets the exact compiler invocation, with its edition, `cfg`s, features, and `--extern` paths. Locus is the only wrapper: a build that configures another, such as sccache through `build.rustc-wrapper`, is refused, since Cargo would chain the two and the driver would not see rustc's own invocation.
- **For a crate Locus checks, one invocation compiles twice.**
  1. **The checking program.** The driver compiles the lowered program with Cargo's own arguments, changed only in its root, in emitting metadata to a directory of Locus's own, in its incremental directory, in capped lints, and in the `cfg` that selects the checking expansions. It records facts after type checking, and lets rustc continue, so that the borrow checker runs, before code generation. This compile must send Cargo no notification that an artifact is ready: in the spike, one that did made Cargo start a dependent before its dependency had an interface, and Cargo then panicked.
  2. **The logical pass.** Locus, as a separate process, checks the crate against the facts and its dependencies' interfaces, and writes the crate's interface.
  3. **The runtime program.** Only if the pass succeeds, the driver compiles the same lowered program with Cargo's own arguments, changed only in its root, with code generation, as the artifact Cargo asked for. Locus validates it against the checking program, then lets Cargo have it.

  In the spike, the two programs' items had identical definition hashes in every crate, so validation can pair them by those hashes.
- **One library, two expansions.** The spike compared four ways for both programs to share Locus's library across crates:
  - **Two libraries of one name,** swapped through `--extern`. The markers of a dependency's runtime artifact are then not those of a checking program (E0308). It works only when every checking program is compiled against its dependencies' checking metadata, kept twice.
  - **One crate of markers, with each version's macros in another.** It silently loses proofs: a dependency's macro that invokes Locus's through `$crate` expanded to the runtime version, so a proof that read a moved value was accepted.
  - **A procedural macro** that reads which program it is in from the environment. It works, at the cost of an input rustc does not track.
  - **One library whose macros expand under a `cfg` that only the checking compile sets.** It works, with one artifact per dependency, because a `cfg` is evaluated where a macro expands, whoever wrote the macro.

  This RFC proposes the last. The `cfg`'s name, `__locus_check`, is fixed by the library, which every crate shares, so user code could test it. Lowering rejects a source that does, and validation, which compares every item and body, catches any other code that does, such as a macro's output. Since typing skeletons are in both programs, the `cfg` now selects only what Locus's macros carry to the driver. Whether a second compile is then still needed is not taken up here.
- **Cargo must never take an unchecked artifact for a checked one.**
  - `RUSTC_WRAPPER` is not part of Cargo's fingerprint. In the spike, after a plain `cargo build`, the build with the driver was entirely Fresh, and a crate whose check fails built with exit status 0.
  - What Cargo does fingerprint is the wrapper's `rustc -vV` output. So the driver adds a line to it that identifies itself, Locus's library, and `locus`. Cargo then gives build mode artifacts of their own, distinct for every version of Locus.
  - Build mode also uses a target directory of its own, under `target/locus/`.
  - After a failed check, Cargo had already removed the crate's artifact, and the next run checked it again.
- **Every input is in the dep-info.** The driver rewrites the dep-info file that Cargo reads to list every `.lc` file of the crate, its dependencies' interfaces, Locus's library, and `locus`. In the spike, which compiled lowered files from disk, rustc's own dep-info listed only those, and Cargo missed an edit to a `.lc` module, and left every crate Fresh with a contract its dependent no longer met. With the file loader, rustc's own lists the `.lc` files it read, but still not the interfaces.
- **Invalidation is conservative.** Cargo rebuilds every dependent of a rebuilt crate, so an edit to a body alone checks its dependents again. Skipping dependents whose dependency's interface is unchanged is an optimization for later.
- **The cost, in the spike's small debug builds,** was 1.9 to 3.8 times the wall time of the same builds without the check (medians of three runs). The logical pass is on the critical path, because a dependent starts only once its dependency's runtime program, and so its interface, is ready. Overlapping them would need the interface before the pass ends, which is for later.
- **Kinds of crates:**
  - a build script that depends on a Locus crate is checked like any crate, and the `cfg`s it sets reach the checking program;
  - an integration test, and every harness `cargo test` builds, is a crate of its own, checked with no interface;
  - a procedural macro crate that depends on a Locus crate is checked like any crate, and one that does not is compiled as plain Rust;
  - doctests are compiled by rustdoc outside any wrapper, so build mode refuses the doctests of a crate that depends on a Locus crate until [cfg-corner-cases](../../issues/cfg-corner-cases.md) decides how to check them.
- **For every other crate,** it compiles exactly as rustc does, and records whether the crate contains `unsafe` code after macro expansion. RFC 0002 uses that to decide which unverified crates need to be named as trusted. A function that foreign code can call, a static that foreign code can reach, and a naked function count as `unsafe` code in that record, whether or not the crate writes `unsafe` (RFC 0002, decision 95). It refuses a user-written implementation of `Snapshot` there too.
- **It refuses a user-written implementation of `Snapshot`.** In every crate it compiles, verified or not, the driver accepts only implementations supplied by lowering or designated library support. It identifies their origin from the lowering and macro crate identity, never a writable name. Ordinary `View` implementations are checked as other logical traits, not rejected as forged snapshots.
- **What a build without the driver guarantees.** A plain `cargo build` compiles the same text. So rustc types and borrow-checks logical code in it, and refuses an implementation of `Snapshot` that does not say `unsafe` (E0200). It does not refuse one that does. It checks no proposition and no proof, since each proof is `Proof` to rustc, and none of the rules that Locus checks after rustc. So a plain build shows that the program is well-typed Rust, logical code included, and nothing about what the program proves. A result is what `cargo locus check` gave, for the sources and the configuration it checked.
- **It fails closed.** If it was asked to check a crate and cannot produce facts for it, the check fails. The first spike's silent fallback to plain compilation is not acceptable.
- **Facts are tied to their program.** The driver records an identity of the checking program it compiled, and Locus rejects facts that do not match the program it generated.

**Only `locus-driver` touches rustc's internals.** It writes facts in a format that Locus owns and versions. The rest of Locus reads that format, builds on stable Rust, and never links rustc. This keeps rustc's API churn in one small crate; the spike measured one renamed function across four releases, for the part of the API it uses. It also lets another producer replace the driver later without touching anything else.

**Toolchain.** The driver is built for one pinned rustc release. The preference is the current stable release, built with `RUSTC_BOOTSTRAP=1`, which the spike showed works. The fallback is a pinned nightly, if a needed internal API exists only there. Either way:
- Locus needs that one toolchain, which it installs.
- In build mode, the same compiler builds the runtime program and every other crate in the build.
- An exported program is plain Rust, built by whatever compiler builds the crate (see [Configurations](#configurations)).
- Locus follows stable releases, so its pinned compiler is at most one release behind the newest.

The Rust project does not support `RUSTC_BOOTSTRAP` outside its own builds. In practice it ties the driver to one exact release, as a nightly pin does.

### Data structures

Each stage has one representation, and each representation has one owner.

1. **Source database.** Files by `FileId`, with their path and text. A `Span` is a `FileId` and a byte range.
2. **Syntax trees.** One lossless rowan tree per `.lc` file, in which every byte, including comments and whitespace, belongs to a node.
   - Node kinds are rust-analyzer's, plus one kind for each Locus form.
   - Typed wrappers give the AST view.
   - The grammar starts from rust-analyzer's parser, forked, with Locus's productions added. Being a superset means parsing all of Rust's syntax from the first day. rust-analyzer's parser already does, tolerates errors, and follows Rust's grammar as it changes; the cost is a fork to keep in step with upstream.
   - **Parser tests.** Every file in rustc's own test suite that stable rustc accepts is parsed *as a `.lc` file*. It must print back byte for byte, and no Locus form may appear in it except in code `cfg` removes, with any exceptions listed and reviewed.
3. **Origin maps.** Every generated file, lowered or exported, has an ordered list of segments covering the whole file. In build mode both programs compile the same lowered text, so they share one map:

   ```rust
   enum Segment {
       /// Text copied from the source unchanged: offsets translate one to one.
       Copied { generated: TextRange, source: Span },
       /// Text Locus wrote: a typing skeleton, a stub, an implementation, the guard.
       Synthesized { generated: TextRange, origin: Origin },
   }

   struct Origin {
       node: SyntaxNodePtr, // the Locus construct that caused the text
       role: Role,          // Skeleton, Stub, Implementation, Guard, ...
   }
   ```

   Every span rustc reports passes through an origin map before a user sees it.
4. **Facts.** What the driver reports, in Locus's own types rather than rustc's:

   ```rust
   struct Facts {
       version: u32,
       program: ProgramId,                       // which checking program these describe
       logical_types: Vec<(Span, Ty)>,           // the type rustc gave each expression of logical code, through its skeleton
       resolutions: Vec<(Span, ItemRef)>,        // the item each name in logical code resolves to, and the implementation it comes from
       snapshot_impls: Vec<(ItemRef, WrittenBy)>,   // each implementation of `Snapshot`, and who wrote it
       locus_names: Vec<(Span, ItemRef)>,        // the definition each assumed Locus name resolved to
       expansions: Vec<(Span, LogicalCode)>,     // Locus macros that rustc expanded, with the logical code each carries
       paths: Vec<(Span, ItemRef)>,              // resolutions of paths in `use`, `impl` headers, and .rs files
       items: Map<ItemRef, ItemInfo>,            // signatures, fields, variants, generics, bounds
       bodies: Map<ItemRef, TypedBody>,          // typed bodies of the functions being verified
       body_hashes: Map<ItemRef, BodyHash>,      // a hash of every typed body, for validation
   }
   ```

   - **Within one build, items are identified by their definitions, not their paths.** An `ItemRef` names a definition by its definition path hash, which rustc computes from the crate's stable identity and the item's path, together with the generic arguments it is instantiated with. A path written in the source is display text only: two paths can name one item, and the spike's calls through a re-export resolved to the defining item.
   - **Across builds, names must survive edits.** A crate's stable identity changes with Cargo's metadata, which depends on the wrapper and on whether the crate is a test harness, and an impl's hash depends on its position in its file. So an interface, which dependents rely on and hash, names an item of its own crate by its definition path, and a trait impl by its trait and its `Self` type ([spike](../../spikes/cargo-graph/README.md)). It names an item of a dependency by the dependency's identity and the item's definition path, because two versions of one crate in a build share every path.
   - **A dependency's identity** is Cargo's resolved package identity, its name, version, and source, with the crate target within the package, since a package's library crate name need not match the package's name. A plain-Rust dependency, which has no interface, is named this way too. A Locus dependency's identity adds the hash of the interface it was checked against.
   - **Whether an earlier check still applies is a separate question,** answered by the manifest rather than by every name: the fingerprints of the sources and dependencies, the features, the target and configuration, the assumptions relied on, and the version of Locus. A path dependency can change while its package identity stays the same, and one package can be built with different features.
   - `Ty` describes Rust types in the same terms, independently of rustc's own data structures. Where the semantic translation needs lifetimes or hygiene ([runtime-semantics](../../issues/runtime-semantics.md)), facts carry them too.
   - `TypedBody` is derived from rustc's THIR. Every dereference, borrow, coercion, and call is explicit. A call to a trait method names the method and its `Self` type; where the types are concrete, the driver also resolves the implementation with rustc's instance resolution. In generic code the call stays a trait method. The semantic translation decides whether THIR is enough or MIR is needed ([runtime-semantics](../../issues/runtime-semantics.md)).
   - Facts are serialized, as JSON at first, so that the driver and the rest of Locus can run as separate processes.
5. **Logical IR.** Locus's typed representation of logical items, propositions, and proofs. It refers to runtime types and items only through `Ty` and `ItemRef` from the facts.
6. **Kernel terms and obligations.** Defined by the kernel RFC.
7. **Interfaces.** When Locus checks a crate, it writes the crate's **interface**: its logical items, the contracts and ghost signatures of its functions, the summary of what each verified function may write, which of its functions are unverified, how the values of each type parameter of a verified generic function reach unverified code, its invariants, snapshot definitions, and explicit views, in Locus's own terms. A crate that depends on it is checked against that interface, since rustc's metadata holds no proposition and no logic function's body. The interface's hash is part of what a dependent's check relied on.
8. **Manifests.** Every successful check writes a **manifest** of what its result relies on, so that a reviewer can see what was claimed and on what grounds:
   - the hashes of the sources, and the configuration checked;
   - the interfaces of the dependencies, by hash;
   - the version of the semantic translation, and of the facts format;
   - the models from Locus's library that the proofs used;
   - the assumptions stated with `unsafe assume!`, each with its proposition and its place, the axioms, and the trust model's entries the result depends on, by ID, including `assumed.unsafe.sound` with the named crates it relies on and each unverified function with `unsafe` code, and each `unsafe impl`, of a verified crate that the result passes through, by location, `assumed.unsafe.invariants` with the types whose invariants it relies on, and `assumed.foreign` with the functions of the build that foreign code can call and the foreign functions declared `safe` that verified code calls;
   - the certificate: the proofs, stored or found, that the kernel checked.

   The audit command prints it, and the vision's promise that a result can report its assumptions (commitment 4) is this list. Of the assumptions stated with `unsafe assume!`, a function's result lists those in its own body and in the functions it calls, and the program's lists every one in every crate it links, which is the list promised complete (RFC 0002, decision 90). Its format, and how stored proofs are found again, are [proof-search-and-storage](../../issues/proof-search-and-storage.md).
9. **Edits.** In export mode, the exported program is a list of `Edit { range, replacement }` applied to each `.lc` file, with an origin map like the lowered program's.

### The runtime program

The runtime program is what runs: in build mode, the lowered program compiled with the runtime expansions; in export mode, the `.rs` files that erasure writes. Either way, Locus produces it only after the check passes, and validates it against the checking program before using it.

**In build mode, it is the same text.**
- Every piece of logical code is its skeleton's closure, which is built, dropped, and never called, so no logical code is evaluated at run time. Whether an unoptimized build keeps anything of those closures was not measured ([Costs and limits](#costs-and-limits)).
- Ghost parameters, arguments, locals, and results stay, as markers. A marker is zero-sized, `Copy`, and `'static`, with no drop glue, so it costs nothing at run time. No one reads this program; it lives under `target/` only.
- Stubs, snapshot types, the implementations of `Snapshot`, and `T: Snapshot` bounds stay too. Each is zero-sized or never called, and runtime code never refers to a stub.
- **No new borrow errors, and no new type errors.** The two programs differ only in what Locus's macros carry to the driver in the checking program. The skeletons, which hold every borrow and every type of logical code, are the same text in both. So the two have identical types, and the runtime program type-checks and borrow-checks whenever the checking program does ([The lowered program](#the-two-programs-have-the-same-types)).
- **The same tokens for every macro.** An attribute macro, a derive, or a program's own macro sees the same input in both programs, because both compile one text. A macro that behaves differently when ghost code is absent has nothing to react to.

**In export mode, erasure writes readable Rust.** It edits each `.lc` file into a `.rs` file beside it. Locus syntax and Locus's macros are removed or replaced, and every other byte is kept.
- **Readable by construction.** A `.lc` file with no Locus syntax becomes an identical `.rs` file, apart from its trailer. Comments, formatting, and names survive everywhere. Where there is Locus syntax, what remains is the Rust a programmer would have written without proofs.
- **No logical values.** The exported program contains no proofs, no logical values, and no logical items. Ghost values inside runtime types and fields are the crate's own private markers, `Proof` and one for each logical type they hold, in its generated module, which no exported signature mentions. Erasure writes them through an import that it appends to each file that needs one, under a fresh name where those names are taken.
- **What the edits do:**
  - **Removed:** logic functions, `prop` declarations, `#[derive(Logical)]` types, snapshot types and generated `Snapshot` implementations, ordinary logical `View` implementations and `T: Snapshot` bounds, and `impl` blocks holding only those; parameters and locals of logical type, proof parameters included, with every argument for them and every `logic!`, `prop!`, `prove!`, and proof hole, except the ghost parameters of a function used as a value, which stay with their markers' types, and whose arguments become markers, so that the function keeps the type it is used at ([RFC 0002](0002-logical-layer.md#export-mode)); `use` declarations and private items that only logical code uses; and code that no checked configuration enables. Which of these apply comes from the facts.
  - **Kept:** a call of a runtime function whose result is entirely ghost, where it was. As a statement it stays as it is. Where a ghost value is needed, as a logical field's initializer, a logical tuple component, or a payload, it becomes `match call { _ => Proof }`, which keeps its order and keeps its temporaries alive to the end of the enclosing statement, as in the source.
  - **Simplified:** a logical field becomes a field of its marker's type; a tuple, including a named result, keeps its shape, with markers for its logical components; a result that is entirely logical and not a tuple becomes `()`, except for a function used as a value, which keeps its marker; a ghost local that runtime code names inside a function-like macro's input stays, bound to its marker; a logical type argument becomes its marker, and so does a ghost value that Rust needs: `Vec<Nat>` keeps its spelling, with `Nat` the crate's own marker, `Option<@P>` becomes `Option<Proof>`, and `Some(h)` becomes `Some(Proof)` or `Some(_)`; `pub(locus)` becomes `pub(crate)`; and `cfg` attributes that every checked configuration satisfies are dropped.
- **Ghost code is written where erasure can remove it.** Erasure edits text, and text inside a function-like macro's input means whatever that macro makes of it. So in export mode, Locus's macros, and calls that pass ghost arguments, are written directly in the crate's own code: not in any macro's output, and not in a function-like macro's input. An item that an attribute macro or a derive takes may hold them, since erasure edits the item before the macro sees it, and validation compares the result ([RFC 0002](0002-logical-layer.md#export-mode)).

**Validation.** Before Locus builds or exports a runtime program, it compiles it with the pinned rustc and compares it with the checking program: for every item and typed body of the crate, the runtime program must have the same item and the same typed body, apart from the expansions of Locus's macros and, in export mode, the ghost parameters, arguments, and locals that erasure removed and the `match` it adds. A difference is an error, never a program other than the one checked.
- In export mode, it catches a macro that sees different tokens once ghost code is erased, such as an attribute macro around a function whose ghost binding was removed, and any bug in erasure.
- In build mode, the text is the same, so it catches only code that tests the checking `cfg`, and a macro that reads the environment or a file that changed between the two compiles, or that keeps state from one compile to the next.
- It compares every item and body, not only those the logic used. A body that is not in the facts, because the checking program's has no ghost values, could still, in the runtime program alone, call a function that needs a proof without one. The cost is a pass over typed bodies that both compiles have already computed.
- **What it compares of an item** is its signature, generics, and bounds; its ABI; its exported symbol and linkage, as `#[no_mangle]`, `#[export_name]`, `#[link_section]`, and `#[used]` set them; for a foreign item, the symbol it binds to and the library it links, as `#[link_name]` on the item and `#[link(...)]` on its `extern` block set them, with their options; its effective visibility; its representation and layout attributes; the target features it is compiled with; which impls exist; and the values of the constants and statics that the logic reads. Of a body, it compares the typed body. A macro that adds `#[cfg_attr(not(__locus_check), export_name = "...")]` changes no body, but changes the item's symbol, so validation catches it.
- Only the transformations that export mode states are exempt: ghost parameters and locals removed, `pub(locus)` written as `pub(crate)`, `cfg` attributes that every checked configuration satisfies dropped, and the guard added.
- What it cannot compare, the guard and the visibilities that export mode writes, is trusted on its own ([the trust model](../TRUST.md)).

**Erasure adds no warnings of its own,** and neither does lowering.
- Lowering marks the items it synthesizes, and ghost bindings that no runtime code uses, so that they cause no lints in the runtime program of build mode.
- In export mode, a binding that only logical code uses is renamed, never removed, so its value is dropped at the same point. The new name begins with an underscore and is fresh in its scope, so it cannot capture another binding. Imports and private items that only logical code uses are removed.
- A runtime field that only logical code reads stays, because removing it would change the type. rustc's dead-code warning for it is reported to the user as "field `x` is read only by logical code", at the user's lint level.
- Likewise, a value that only logical code reads, such as an assignment overwritten before runtime code reads it, keeps its assignment. rustc's warning for it is reported as "the value assigned to `n` is read only by logical code".
- In build mode rustc sees what logical code reads, through the skeletons, so it does not warn that a parameter, a field, or an assigned value that only logical code reads is unused. The two points above are then about an exported program, in which the logical code is gone.

Building the runtime program is how Locus reports lints and confirms its own output. In export mode, that is the compile of the exported program that validation needs anyway. Any other warning there is either about the user's code or a Locus bug.

**Source locations.** In build mode, `file!`, `line!`, `column!`, `Location::caller`, and panic messages name the `.lc` file, because rustc reads the lowered text at its path, and lowering keeps runtime code on its own line where it can, though a column moves where Locus syntax earlier on the line was lowered. In export mode, they name the exported file, whose lines move where erasure removed an item. That is the one stated exception to "erasure never changes what the program does".

**Contracts are visible only to code that Locus verifies.** No proof exists at run time. What unverified Rust code could do instead is call a function without meeting its precondition, rely on a postcondition, or build a value without its invariant. Neither mode lets it:
- **Within a crate,** every file is checked. A function whose parameters hold proofs never reaches code that Locus does not verify, as a function value, a function pointer, a trait method, through generic code, or as an exported symbol ([RFC 0002](0002-logical-layer.md#the-boundary-with-unverified-code)).
- **In build mode,** every crate that depends on a Locus crate is verified, so every call of its functions and every use of its types is.
- **In export mode,** what other crates can reach is plain Rust ([RFC 0002](0002-logical-layer.md#export-mode)): no contracts, no logical values in types, no public field that an invariant reads, and no way to construct a type with proof fields.

So Locus needs no proof tokens and no `unsafe` exports to protect contracts.

### Diagnostics

Diagnostics come from five places.

| Source | Code | What the user sees |
|---|---|---|
| Locus's parser | `Lnnnn` | Locus's message |
| rustc on the checking program, an error of the user's own | rustc's `Ennnn` | rustc's message, mapped to the source |
| rustc on the checking program, an error that involves logical code | `Lnnnn` | Locus's message, with rustc's code and message as a note |
| Locus's logical pass and kernel | `Lnnnn` | Locus's message |
| rustc on the runtime program | rustc's | Mapped to the source. An error in copied text is the user's, such as a post-monomorphization error that a check run misses. An error in synthesized text is a Locus bug |
| Validation of the runtime program | `Lnnnn` | Locus's message, naming the item whose body differs from the one checked, and the macro whose expansion differs, when there is one |

- **Classifying.** A rustc error is the user's own only if every span it reports lies in text the user wrote: copied text in the checking program, or code outside it, such as the standard library or a dependency. Every span counts: primary, secondary, labels, and notes.
- **Errors that involve logical code.** An error involves logical code if any span lies in synthesized text, or if its message names a stub, a helper, or anything else Locus generated.
  - Locus words it in its own terms, with the synthesized span as the primary span. For example, rustc's E0502 at a user's `x.push(2)`, caused by an observation earlier, becomes "logical code reads `x` while `x` is mutably borrowed", pointing at the logical code, with rustc's code as a note.
  - Some errors at synthesized code have known causes, such as reading a mutable static from logical code. Locus explains these.
  - Anything else that involves logical code is an internal error, never blamed on the user.
- **Errors with no span** are shown as rustc reports them, marked as not attributed.
- **Mapping.** Locus never shows rustc's rendered text. It reads rustc's structured diagnostics and maps every span through the origin maps, including macro backtraces and suggestions.
  - The two ends of a span map independently, so a span half in copied and half in synthesized text still covers the right source.
  - An end inside synthesized text maps to the edge of the construct that caused it.
  - Spans inside Locus's library map to the invocation that expanded there.
  - A suggestion whose edit lands in copied text is kept and applies to the `.lc` file. One that touches synthesized text is dropped.
- **Wording.** The names that lowering writes read as the programmer thinks of them to begin with ([Names that rustc can show](#names-that-rustc-can-show)). So a type error in logical code reads as rustc wrote it, at the user's tokens. Where a message still shows the encoding, such as a stub's reserved name, Locus shows the Locus term.
- **A plain build prints rustc's own messages,** unmapped and not reworded, which is why the names themselves have to read well.
- **Stopping.** If rustc rejects the checking program, Locus reports rustc's errors and stops before the logical pass, which needs rustc's facts.
- **Lints.** Lints come from building the runtime program, so they follow the user's lint levels exactly and never mention synthesized code.
- **Rendering and data.** Locus renders every diagnostic in rustc's layout, against the `.lc` source, and offers the same diagnostics as JSON with a field for rustc's code. `locus explain Ennnn` defers to `rustc --explain`.
- **Litmus annotations** will accept rustc's codes, such as `//~ error[E0382]`, once this RFC is accepted. Until then, examples of rustc's errors give its message.

### Components

The pipeline suggests these components, each with one job. Build tasks create them after this RFC is accepted. Several may start as modules of one crate, and be split when a dependency or a testing boundary calls for it ([external-review-ideas](../../issues/external-review-ideas.md)); the driver, the facts format, the library, and the kernel are separate from the start, because each is linked or trusted differently.

| Crate | Job | Links rustc's internals | Trusted |
|---|---|---|---|
| `locus-syntax` | Lexer, parser, lossless trees | No | Yes: it decides where Locus syntax begins and ends |
| `locus-facts` | The facts format and its serialization | No | Yes |
| `locus-driver` | The rustc driver that reports facts | **Yes, alone** | Yes |
| `locus-library` | The logical types and `Proof`, Locus's macros, the stubs and helper traits that typing skeletons are written over, and `Snapshot`, as one Rust library whose macros have a checking and a runtime expansion, selected by a `cfg` | No | Yes: rustc types logical code against its stubs, skeletons are recognized by its definitions, and its macros decide what both programs contain |
| `locus-lower` | The lowered program, generated module, and origin map | No | Yes |
| `locus-logic` | Logical items, the checks after rustc, the ghost checks, elaboration, obligations | No | Yes, for its ghost checks; obligation generation as decided by [runtime-semantics](../../issues/runtime-semantics.md) |
| `locus-search` | Proof search | No | No |
| `locus-kernel` | Proof checking | No | Yes |
| `locus-erase` | Export mode's runtime program, export surface, guard, and origin maps | No | No for bodies, which validation checks; yes for the guard and visibilities |
| `locus-diagnostics` | Mapping, wording, rendering | No | No |
| `locus` | Command line, both modes, Cargo orchestration, validation, and manifests | No | Yes: it pairs facts with programs, decides which crates are checked, decides when to stop, and validates the runtime program |

### Spikes

Spikes tested this design. Nothing depends on them, and each states what it did not test. The first four tested the pipeline.

- **[Facts from rustc](../../spikes/rustc-facts/README.md).** A 150-line driver run by Cargo reported the types of observed places, resolutions, and a typed body, on stable 1.97 with `RUSTC_BOOTSTRAP=1` and on a nightly, and rustc's borrow checker rejected a proof that read a moved value.
- **[One lowered text](../../spikes/one-text-lowering/README.md).** 30 hand-lowered cases and 78 rows of macro cases, each compiled both ways:
  - the runtime program needed no edit beyond the other expansions;
  - lowering needed no expression's type;
  - an attribute macro that saw different tokens under the previous design returned the same value in both programs;
  - expansions that rustc performs observed precisely with the combined record of ghost bindings.
- **[The Cargo graph](../../spikes/cargo-graph/README.md).** Two Locus libraries, an application, a build script, a procedural macro, and tests, built by Cargo through a wrapper that compiles twice, with a stand-in for the logical pass. It showed:
  - which ways of sharing Locus's library work;
  - that the dep-info must be rewritten and the wrapper must identify itself to Cargo's fingerprints;
  - what the check costs;
  - that definition hashes pair the two programs within a build, but interfaces need names that survive edits.

- **[The `locus!` macro](../../spikes/locus-macro/README.md).** A toy `locus!` over 22 litmus examples in an ordinary Cargo crate, every litmus file through rustc's lexer as macro input, rustfmt and rust-analyzer on the result, and a plain-Rust crate calling a Locus one. It showed:
  - that the surface syntax survives as macro input, and one text gives both expansions;
  - that diagnostics point at the user's code, rustfmt leaves blocks alone, and rust-analyzer resolves inside them;
  - that ghost parameters must stay as markers, and that a plain-Rust caller can then pass a marker that proves nothing;
  - what a block cannot see: logical types declared in another block, and which marker a `logic!` gives when its code is a proof.

Six more tested how logical types reach rustc. They are why each logical type is a type of its own and rustc types logical code. All ran on stable rustc 1.97.0, and the lowerings among them are toy macros with heuristics.

- **[A type for each logical type](../../spikes/logical-markers/README.md).** Twenty programs of hand-written Rust, and a driver that asked rustc's trait solver about types that the program never mentions. It showed:
  - that rustc resolves every trait on logical types once each is a type of its own, and that with one marker two implementations collide;
  - the obstacle: a ghost value whose type nobody wrote.
- **[A lowering with a type for each](../../spikes/marker-lowering/README.md).** A macro that performs the lowering, over 58 cases of its own, 26 corpus files, and the 17 litmus examples with an unannotated ghost `let`. It showed:
  - that a lowering can do it, with one text and two expansions;
  - that a guess from syntax at a ghost local's type has gaps, one of them silent;
  - that a hidden trait for a trait's stubs breaks when the trait is imported by name;
  - in two later variants, the two one-marker encodings that this RFC rejects.
- **Typing skeletons, in three rounds.**
  - The [first](../../spikes/skeleton-typing/README.md) typed all 51 pieces of logical code in its corpus, gave ghost locals their types with no annotation, and measured what skeletons cost to build.
  - The [second](../../spikes/skeleton-typing-2/README.md) applied the owner's conversion policy, renamed what rustc shows, covered the shapes the first had left out, and compiled two crates.
  - The [third](../../spikes/skeleton-typing-3/README.md) gave each type one implementation of `Model`, written where the type is declared, made the trait `unsafe`, and listed implementations by who wrote them.
  - Each round listed every call in both programs and found no difference.
- **[Snapshots and models as one family](../../spikes/capture-family/README.md).** Sixteen programs of hand-written Rust. It showed:
  - that a bound the program writes gives what no fallback could, and that a closure can never have a model;
  - that with two traits the generated implementations can be deferred while a declared model is not.

**Not yet tested:**
- any check that Locus makes after rustc. The spikes typed programs with rustc and proved nothing, and no case compares a type that rustc gives with what Locus's rules give;
- a typing skeleton written by a real lowering, which decides what a name is from the crate's items. Each spike's translation guessed from capital letters, and lists its guesses;
- the two-trait, deferred form of the generated implementations, in a lowering: it is hand-written Rust in `capture-family`;
- the driver refusing an implementation of `Snapshot`, and knowing lowering by the identity of its crate: the third round lists, by name;
- how rustc is given the snapshot of a generic type that contains itself, and of a union;
- logical code in a constant context;
- what skeletons cost at run time and in the size of a binary, in a release build, and to build beyond the first round's one synthetic file;
- three or more crates, a re-export of a logical type or of a type with a generated implementation, and rust-analyzer and rustfmt on a text with skeletons;
- real lowering from `.lc` files, and the real prelude;
- a real `locus!`, with Locus's parser, its own diagnostics, and the driver reading its checking expansion;
- serving the lowered text through rustc's file loader at the sources' paths, with `#[path]` for `.lc` modules;
- checking procedural macro crates that depend on Locus crates, and comparing every item's description and every body in validation;
- inferring effect summaries, and keeping them in interfaces;
- `#[path]` for modules beneath a `.lc` module, beyond the two resolutions tested: a submodule rustc looks for beside the file (E0583), and a `#[path]` the source writes inside an inline module;
- reporting the logical code of expansions rustc performs to the driver;
- cross-compilation, registry dependencies, and procedural macros that depend on Locus crates;
- release builds and `cargo check`;
- crates of realistic size.

The semantic slice, one mutable function verified end to end, goes with the runtime semantics ([runtime-semantics](../../issues/runtime-semantics.md)).

## Rules

The rules below go into the specification with this RFC, in [Compatibility with Rust](../../spec/compatibility.md), [Observing runtime values](../../spec/observation.md), and [Exporting to Rust](../../spec/export.md). The last two rows are rules that [RFC 0002](0002-logical-layer.md) introduced, in [The logical layer](../../spec/logical-layer.md), and that this RFC changes: they name the markers.

| Rule | Examples |
|---|---|
| `compat.superset.same-meaning` | `compatibility/clone-before-move.accept.lc`, `compatibility/include-from-source.accept.lc`, `compatibility/model-where-unsafe-is-forbidden.accept.lc`, `compatibility/name-resembling-reserved-prefix.accept.lc`, `compatibility/plain-rust.accept.lc`, `compatibility/user-method-named-model.accept.lc`, `compatibility/reserved-check-cfg.reject.lc`, `compatibility/reserved-prefix-in-a-name.reject.lc` |
| `compat.errors.rustc` | `compatibility/logic-block-is-a-struct-literal.reject.lc`, `compatibility/use-after-move.reject.lc` |
| `compat.syntax.not-rust` | `compatibility/at-in-patterns.accept.lc`, `compatibility/keywords-as-names.accept.lc`, `compatibility/pub-locus-as-tuple-field-type.accept.lc`, `compatibility/tokens-in-macro-input.accept.lc`, `compatibility/underscore-in-rust-positions.accept.lc`, `compatibility/hole-in-macro-input.reject.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.syntax.contextual-keywords` | `compatibility/keywords-as-names.accept.lc`, `compatibility/raw-identifier-logic.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.names.shadowable` | `compatibility/glob-imported-macro-named-prove.accept.lc`, `compatibility/own-prove-inside-vec.accept.lc`, `compatibility/user-macro-named-prove.accept.lc`, `compatibility/user-type-named-nat.accept.lc` |
| `compat.names.no-methods` | `compatibility/user-method-named-model.accept.lc` |
| `compat.names.macros-expand` | `compatibility/locus-macro-inside-rust-macro.accept.lc`, `compatibility/prove-in-macro-output.accept.lc`, `compatibility/prove-in-stringify.accept.lc`, `compatibility/prove-through-alias.accept.lc`, `observation/macro-in-vec-reads-sibling-field.accept.lc`, `compatibility/expanded-macro-checked.reject.lc` |
| `compat.names.reserved` | `compatibility/name-resembling-reserved-prefix.accept.lc`, `compatibility/reserved-prefix-in-a-name.reject.lc` |
| `compat.names.model-impls` | `compatibility/model-where-unsafe-is-forbidden.accept.lc`, `compatibility/model-impl-marked-unsafe.reject.lc` |
| `observe.place.readable` | `models/read-through-cut-reference.accept.lc`, `observation/after-mutable-borrow-ends.accept.lc`, `observation/before-move.accept.lc`, `observation/let-annotation-reads-kept-place.accept.lc`, `observation/logical-closure-in-body.accept.lc`, `observation/macro-in-vec-reads-sibling-field.accept.lc`, `observation/macro-reads-sibling-field.accept.lc`, `observation/proof-before-move.accept.lc`, `observation/after-move.reject.lc`, `observation/during-mutable-borrow.reject.lc`, `observation/let-annotation-after-initializer.reject.lc`, `observation/proof-after-move.reject.lc` |
| `observe.let.after-initializer` | `observation/let-annotation-reads-kept-place.accept.lc`, `observation/let-annotation-after-initializer.reject.lc` |
| `observe.captures.rejected` | `observation/logical-closure-in-body.accept.lc`, `observation/logic-in-async-block.reject.lc`, `observation/logic-in-closure.reject.lc` |
| `export.locus-visibility` | `export/locus-logical-field.accept.lc`, `export/wrapped-precondition.accept.lc`, `export/reexport-locus-item.reject.lc` |
| `export.macros-direct` | `export/ghost-call-outside-macro.accept.lc`, `export/ghost-local-in-macro-input.accept.lc`, `export/prove-in-test-function.accept.lc`, `export/prove-through-alias.accept.lc`, `export/ghost-call-inside-macro.reject.lc`, `export/prove-in-macro-output.reject.lc`, `export/prove-inside-rust-macro.reject.lc` |
| `export.function-values` | `export/function-with-ghost-parameter-as-value.accept.lc`, `export/function-with-ghost-result-as-value.accept.lc` |
| `logic.ghost.erased` | `logical-layer/ghost-parameters.accept.lc`, `logical-layer/logical-tuple-keeps-shape.accept.lc`, `logical-layer/logical-values-in-vec.accept.lc`, `logical-layer/runtime-call-bound-first.accept.lc`, `logical-layer/vector-of-nat-as-vector-of-int.reject.lc` |
| `logic.ghost.marker-traits` | `logical-layer/derive-eq-with-proof-field.accept.lc`, `logical-layer/proof-holder-formatted.accept.lc`, `contracts/invariant-default-derive.reject.lc`, `logical-layer/compare-logical-vectors.reject.lc`, `logical-layer/derive-eq-with-logical-field.reject.lc` |

The accepted near-misses `before-move`, `after-mutable-borrow-ends`, and `proof-before-move` read `v.elements().len()` in logical code, which [RFC 0008](0008-snapshot-capture-and-views.md) defines through the vector snapshot's explicit observer, and rely on its versions: restating a proof before the move needs `v` to be the same version as at entry. So they are decided together with it.

The observation examples use provisional syntax for propositions ([proposition-syntax](../../issues/proposition-syntax.md)); respelling them does not change their outcomes.

Four decided examples change a `run` expectation with the marker's name, from `Erased` to `Proof`: `logical-layer/proof-holder-formatted`, `logical-layer/proof-holder-as-shared-trait-object`, `logical-layer/own-generic-lends-proof-holder`, and `logical-layer/derived-debug-on-type-with-proof-field`. No example changes its outcome.

**What has no rule.** A rule says something that an example of one file can show. Four things that this RFC guarantees are about what a tool does, or need a second file or crate, and have no rule of their own, as item 1 of what is still for the owner has none:
- **That the two programs resolve every call alike.** Under the present rules no `impl` names a logical type (`logic.ghost.no-trait-impls`), so no program that the specification accepts can tell this lowering from the encodings that broke the property: every case that broke them writes such an `impl`. Examples become possible with the RFC on traits.
- **That an implementation of `Snapshot` in plain Rust is refused:** in a `.rs` file, outside a `locus!` block, or in a crate that Locus does not lower. `compat.names.model-impls` says it, and its examples show the part that one `.lc` file can: an `unsafe impl Snapshot` in Locus code, and a crate that forbids `unsafe` code. [multi-file-litmus](../../issues/multi-file-litmus.md) has the rest.
- **That a plain build is stopped by a type error or a borrow error in logical code.** An example is checked by Locus, which rejects such a program either way (`observe.place.readable`).
- **That every name rustc can show reads as the programmer thinks.** An error's text is not pinned until error codes exist. What a program prints is: the examples that format a proof print `Proof` (`logic.ghost.marker-traits`).

## Interactions

| Area | Effect |
|---|---|
| Types | rustc is the authority on every runtime type, and now on every logical type too: each is a zero-sized Rust type of its own, and rustc types logical code through its typing skeleton. A proof reaches rustc only as the one marker `Proof`, which carries no proposition ([RFC 0002](0002-logical-layer.md)). Locus has no typing of its own for logical code: it reads rustc's, and works out only which proposition each proof proves. |
| Propositions and proofs | rustc never sees a proposition as a type. It sees the typing skeleton of each proposition's code, so it types the proposition and borrows each place it reads, and it sees `Proof` for every proof. |
| Ownership and mutation | rustc checks all ownership, including what logical code reads. How mutation is modeled in the logic is built from rustc's typed bodies ([runtime-semantics](../../issues/runtime-semantics.md)). |
| Erasure and runtime behavior | In build mode nothing is erased: both programs compile one lowered text, whose ghost values are markers and whose logical code is closures that are never called. In export mode erasure edits the source into readable Rust. Either way, runtime code is untouched, logical code does no runtime evaluation, the runtime program is validated against the checking program, and only source locations change. |
| Configurations | Each configuration is a separate program. Build mode checks the configuration being built. An exported crate is checked in the configurations it declares, and its exported code refuses to compile in any other ([cfg-corner-cases](../../issues/cfg-corner-cases.md)). |
| Rust interoperability | In build mode, every crate that depends on a Locus crate is verified, so contracts work across crates. In export mode, `.rs` files sit beside their `.lc` sources, so the crate builds with plain Cargo, and its interface is plain Rust, with no proofs, logical values, or contracts; `pub(locus)` marks what only Locus crates see ([distribution-and-export](../../issues/distribution-and-export.md)). |
| Diagnostics | Rust's errors keep their codes and messages, with spans mapped to the source. A type error or a borrow error in logical code is rustc's too, at the user's tokens, in a plain build as in a check, and the names that lowering writes read as the programmer thinks of them. Errors that still show the encoding are explained in Locus's terms. Lints come from the runtime program. |
| Trust model | New trusted components: rustc's type checker, for logical code as for runtime code, and its borrow checker, `locus-driver`, which also refuses an implementation of `Snapshot` that Locus did not write, the facts format, Locus's library, the parser, the lowering, obligation generation, translation validation, export's guard and visibilities, and the orchestration. New assumptions, for exported crates: toolchains, dependencies, macro inputs, and published files. Decisions 49 to 52 add no entry: they change what `trusted.rustc.types`, `trusted.rustc.expansion`, `trusted.driver`, `trusted.library`, and `trusted.lowering` cover. [RFC 0002](0002-logical-layer.md) adds the rest. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Remove ghost parameters before rustc runs,** so that the checking program, like the runtime program, has no proof parameters. This was the previous draft. Rejected:
- Lowering would have to know, at every call, which arguments to delete and whether a result is ghost, which needs the callee's signature, and so the receiver's type, method resolution, and trait resolution: rustc's work.
- A wrong guess can compile with another meaning. Tested with rustc 1.97: with an inherent `go(&self, a, h: @P)` and a trait method `go(self, a)` in scope, deleting `h` from `x.go(1, h)` compiled and called the trait method; and a proof-returning call lowered to return `()` instantiated `make::<()>` and compiled.
- The two programs would differ in their ghost bindings, so a macro around a function would see different tokens in each. An external review built a deterministic attribute macro that returned `0` in the checking program and `1` in the runtime program.

**Keep the markers in the checking program only,** and remove them from build mode's runtime program afterwards, using the facts. Rejected: it keeps the runtime program of build mode readable, but no one reads it, and the two programs would again differ under every attribute macro.

**A discovery run:** compile a provisional program first to learn types and resolutions, then lower precisely and compile again. Rejected for now. It costs a second compile of every verified crate, and it must be shown to converge. One lowered text needs neither. It stays the fallback if a case arises that only types can decide.

**Lowered files on disk, mirroring the crate under `target/`,** compiled from there. Rejected: a path that the source resolves from its own file, as in `#![doc = include_str!("../README.md")]`, would resolve from the copy instead, and `file!` would name it.

**Reject attribute macros around code that erasure edits,** in export mode, instead of validating. Rejected: it forbids common macros such as `#[tracing::instrument]` on any function with a proof, even when they behave the same either way. Validation accepts them when the bodies agree, and fails loudly when they do not.

**Encode everything in Rust macros and attributes,** with no Locus syntax: proof types as `proof![x > 0]`, logic functions as `#[logic] fn`. Creusot and Prusti work this way.
- *For:* Locus would need no parser of its own. rust-analyzer, rustfmt, and editors would work on Locus code unchanged.
- *Against:* proofs as values live in signatures, which are what people review, and macro encodings make signatures noisier. Errors inside macro invocations are also reported less precisely.

This RFC instead uses syntax for proofs, logic functions, and propositions, and names only for blocks, derives, and traits.

**A `locus! { ... }` macro inside ordinary `.rs` files,** erasing at build time. It needs no generated files. But a procedural macro sees no types and can only erase, never check, so a crate could be built and published with proofs that were never checked. Exported `.rs` files exist only when the check passed.

**One mode, whose output always serves Rust callers.** Generated files would sit beside their sources in every build, with `OUT_DIR` as an opt-in. To protect contracts from unverified callers, proof parameters would take tokens that only verified code supplies, other crates would obtain tokens through an `unsafe` constructor, and a function whose precondition sits in its parameters' data would be `unsafe`.
- Rejected. Every Locus crate paid for the Rust boundary, even when all its users were checked, and contracts between Locus crates went through `unsafe`.
- With two modes, contracts are visible only where Locus checks every use: everywhere in build mode, and nowhere outside the crate in export mode.

**An export root,** a file such as `src/export.lc` whose public items alone are exported, with every other item's visibility lowered in export mode. Rejected at the owner's direction: it changes visibilities without saying so where they are written, and needs a rule against names that collide between the two roots. `pub(locus)` marks the difference on the item.

**`pub(export)`, marking what is exported** rather than what is not. Rejected: a Rust file renamed to `.lc` would lose its whole interface from the exported crate until each item was marked.

**Locus's macros recognized only where lowering sees them,** with every other expansion an error. Rejected at the owner's direction: a Locus macro inside `vec![...]`, in a program's own macro, or under an alias is ordinary Rust, and rejecting it makes Locus's names behave unlike any other macro's.

**Other names for export mode,** such as transpile or generate. `export` names what the mode is for, giving a crate's interface to Rust users, where the others name how it works.

**`logic struct` and `logic enum` keywords** for logical types. Replaced by `#[derive(Logical)]`, which needs no new syntax.

**A `@{ ... }` logical block.** Replaced by `logic! { ... }`, so that `@` stays reserved for proofs.

**No built-in names,** with every Locus form written as syntax. Rejected: the names are worth having once rustc resolves them with Rust's own shadowing rules.

**Locus's own front end for Rust.** Rejected: it reimplements rustc, and wherever the two differ, proofs would be about a program other than the one that runs.

**rust-analyzer's libraries instead of a rustc driver.** These work on stable Rust without `RUSTC_BOOTSTRAP`. But rust-analyzer's type inference is not rustc's, and where they differ, proofs would be about a program other than the one that runs. Its borrow checking is also not rustc's. It remains a candidate for editor support, where speed matters more than authority.

**Charon** exports rustc's MIR in a stable format, and is a candidate source of typed bodies for [runtime-semantics](../../issues/runtime-semantics.md). Run on the checking program, it could also report the types that rustc gives the typing skeletons. It pins its own nightly, and adopting it would make its format part of the trusted path.

**One marker for every logical value, and two families of traits.** This is the second of the two lowerings that were left standing when the owner chose (decision 49). Regular traits are for runtime types. Logical traits are for logical types, and rustc sees one only as an empty trait that the marker implements. Locus types logical code itself, and asks rustc through the driver, on stub types that appear nowhere in the program, which implementation of a logical trait applies.
- *For:* this RFC would stand nearly as it was. Logical code would keep a typing of its own, with RFC 0002's implicit conversions, and a build would gain nothing. In [marker-lowering](../../spikes/marker-lowering/README.md), the driver answered 55 of 55 such questions as expected.
- *Against:* Locus would need a type checker of its own for logical code, generics included. None exists, and none of the checks that this lowering rests on was exercised. Its restrictions are on which programs exist at all. No two implementations may differ only in a logical type inside a runtime type (`r01`, `r02`, `s20`: E0119). An associated logical type must have the plain marker's shape, not a proof or a tuple (`t06`, `s06`, `s07`). A runtime type cannot implement a logical trait, and no trait or generic function spans both kinds of type. And rustc enforces nothing that tells logical types apart: Locus would have to check every logical bound at every use, and that each callee rustc selected is the one the logical types select.

**One marker in the runtime program, and a type for each logical type in the checking program.** Rejected: the two programs then have different types, and rustc resolves by types. [marker-lowering](../../spikes/marker-lowering/README.md) built it as its third variant. Of the 48 cases that both programs accepted, 5 called a different item in each, one of them into a branch that, as checked, holds a proof of `False`, and `TypeId` told `Int` from `Nat` in one program alone. A program that the checking program accepts could also fail in the runtime program for the one-marker reasons (E0308, E0119). A check that the two programs correspond would have to compare every call, at every instantiation, and keep a program from observing a type's identity.

**Implementations on stubs, with one marker where a value's type is written.** marker-lowering's second variant, from which the two-family lowering above was drawn. A distinct stub type stands only where an `impl` names a logical type. Rejected: an implementation for a runtime type at one logical argument then applies at every one. With an inherent method for `Wrap<Int>` and a trait's for every `Wrap<T>`, both programs called the inherent method on a `Wrap<Nat>`, and rustc reported nothing (`r10`); in `r11` that took the branch that holds a proof of `False`. Lowering must also know that the self type of an `impl` is logical before rustc runs, which for a type of another block it could not (`s17`: E0119).

**Ghost locals typed without a skeleton.** With a type for each logical type, a ghost local whose type nobody wrote needs its type from somewhere. Three ways were tried before the skeleton:
- *rustc's inference alone.* A ghost local that only later logical code mentions has nothing to infer from: "type annotations needed" (E0283; [logical-markers](../../spikes/logical-markers/README.md), `c06`). With every ghost local typed this way, 4 of the 23 corpus programs that compiled failed, and all 12 of those among the litmus examples with an unannotated ghost `let` ([marker-lowering](../../spikes/marker-lowering/README.md)). Each would need an annotation that RFC 0002 does not ask for.
- *A conversion at each use.* A ghost local has no logical type for rustc, and each use in runtime code converts. A declared parameter type decides (`c14`), and a generic use has nothing to go on (`c15`: E0283).
- *A guess from syntax.* A ghost local that only logical code mentions holds an untyped marker, and otherwise rustc infers. marker-lowering's first variant built it. A local that is only copied, and a ghost component of a runtime tuple, then asked for a type (`g13`, `g14`: E0282). A use inside a format string was missed silently: the program compiled, and the local's type was the untyped marker (`g15`). The skeleton types all three with no annotation.

**Staged typing.** Locus computes the types of logical code itself, or learns them from a first compile, before the final Rust compilation, and writes them into the lowered text. Kept in reserve. It would give what Rust's inference cannot, such as the mark on a field of a type that Locus cannot see, and it is where the design goes if the deferred form of the generated implementations cannot be relied on ([Costs and limits](#costs-and-limits)). It gives up building the source with stock `cargo build`: a `locus!` block could no longer be expanded by rustc alone, since its expansion would need types. It was not prototyped, because the skeleton made it unnecessary in the corpus. It is a form of the discovery run above.

**An implicit `Model` bound on every type parameter.** Rejected: some types can never be read. A trait cannot be implemented for a closure or a function item except by a blanket implementation, which would overlap every other, so an implied bound would refuse every closure argument ([capture-family](../../spikes/capture-family/README.md), `c05`). The [second round](../../spikes/skeleton-typing-2/README.md) of the skeleton spike had lowering insert the bound where it saw the need, and it could not always see it: not in a method of a trait's `impl`, not in code it did not lower, and not across blocks or crates (`n2`, `n3`, `n4`). So the bound is written (RFC 0002, decision 106).

**One opaque type for everything unknown.** The second round's fallback: `snap(x)` of a type that nothing declares a snapshot for gives one opaque type. Rejected: the snapshots of two unrelated types of other crates are then one Rust type, so rustc accepts one where the other is wanted (`n6`), and a `u32` read through an unbounded type parameter is another type than a `u32` read directly, silently (`n2`, and the probe `s5`). Without specialization, no encoding gave such a type a snapshot type of its own. So it has no snapshot until a field is marked (RFC 0002, decision 107), and a marked field's opaque type is distinct for each field type.

**One marker for every snapshot,** `Snap<T>` as a projection that forgets `T`. It keeps no borrow, where a `Snap<T>` that kept `T` kept `T`'s lifetimes and rejected Rust code that is fine today ([logical-markers](../../spikes/logical-markers/README.md), `c09` and `c08`: E0499). Not chosen: no implementation could then tell `Snap<A>` from `Snap<B>`, and rustc accepted a `Snap<Vec<u32>>` where a `Snap<bool>` was wanted. A snapshot type that is generic over its fields' snapshots holds no lifetime either, and rustc rejects that program (E0308; [first round](../../spikes/skeleton-typing/README.md), `g18`).

**A sealed trait for `Model`.** Rejected: a sealed Rust trait cannot be implemented outside its crate, and lowering implements `Model` in every crate that declares a type. An `unsafe trait`, with the driver refusing what Locus did not write, is the form that lets it (decision 51).

**A hidden trait for a trait's stubs,** beside the lowered trait. Rejected: an import of the trait does not bring it ([The lowered program](#stubs)).

**Zero-sized stand-ins for proofs that Locus believes by their type,** in both programs. Rejected: Rust code that can tell types apart, through `Any` or specialization, could turn one proof into another. [RFC 0002](0002-logical-layer.md) makes proofs ghost instead: `Proof` is still one type for every proof, but Locus never believes it.

**One Rust type per proposition,** so that rustc could tell proofs apart. Not possible: propositions mention runtime values, so `@(y != 0)` is a different proposition for each `y`.

**Remove logical and proof fields from types.** Rejected at the owner's direction. It needs renumbered tuple fields, extra `PhantomData` fields, and observations of the whole value where logical code reads one field, and Locus's macros expanded inside other macros could not name the field at all. Kept as a marker, a field costs derives that its marker does not implement, such as serde's, and shows up in `Debug` output.

**Default to the host's configuration.** Rejected: the same crate would check differently on different machines.

**Give rustc logical code as it is written,** as real Rust over the logical types, with Rust's operator traits implemented on them, close to what Verus does with its proof code. Rejected in that form: Rust fixes the result of `<` as the runtime `bool` (E0308; [logical-markers](../../spikes/logical-markers/README.md), `c13`), `Add::add` is a method that runs, half of Locus's operators are not Rust's, and a logical enum that contains itself is not a Rust enum. The typing skeleton keeps what this alternative was after, rustc's typing of logical code, and writes each operator as a stub. An earlier revision of this RFC rejected the whole idea, for the same two reasons.

**Write a new Rust parser** instead of forking rust-analyzer's. It would give Locus full control and cleaner code, at the cost of reaching all of Rust's syntax before anything else works. The parser tests apply either way.

## Prior art

- **Clippy** is a rustc driver run through `RUSTC_WORKSPACE_WRAPPER`, the model for `locus-driver`.
- **Verus** checks the ownership of its proof code by generating a separate Rust program for rustc's borrow checker, a close cousin of the checking program. It also has rustc type its specification and proof code, over types of its own library, as the typing skeletons do here. It reads rustc's internals directly throughout, where Locus confines that to one crate.
- **Creusot, Prusti, Kani, and Flux** are rustc drivers pinned to a nightly. Creusot and Prusti erase their contracts in ordinary builds.
- **Charon**, used by Aeneas, exports what rustc knows in a format of its own, separating the part that depends on rustc from the analysis. `locus-facts` follows the same separation.
- **rust-analyzer** provides the lossless, error-tolerant parser and the rowan trees this design starts from.
- **Code generators that commit their output,** such as LALRPOP's and some users of prost, show that generated Rust beside its source is workable when a check catches stale output.
- **TypeScript-based tools for Svelte and Vue** check generated code and map diagnostics back through source maps, as the origin maps do here.

## Unresolved questions

**Decided by the owner while this RFC was drafted.** Recorded here; accepting the RFC confirms them.
1. Locus syntax only for proofs, logic functions, and propositions. Names for blocks, derives, and traits, resolved like std's prelude.
2. `#[derive(Logical)]`, never implemented by hand, and a `Model` trait, in place of `logic struct` and `logic enum`.
3. The four ways facts enter the logic. `Cell`, `RefCell`, `Mutex`, and the atomics have no model, and their snapshots hold nothing of their contents. Since RFC 0002's decision 99 each has a model, which is its snapshot and still holds nothing of their contents. Decision 52 below confirms the four ways.
4. The current stable release with `RUSTC_BOOTSTRAP=1` preferred; a pinned nightly acceptable.
5. Two modes. Build mode, the default, compiles the runtime program under `target/`, in one Cargo run, and checks every crate that depends on a Locus crate. Export mode writes `.rs` files beside the sources for Rust users. `pub(locus)` marks what only Locus crates see: public in build mode, and `pub(crate)` in export mode.
6. In build mode, the configuration being built is checked. In export mode, declared configurations with a guard; the default is dev, release, and test builds with default features and 64-bit pointers, independent of the host. Erasure removes code no checked configuration enables.
7. An exported interface has no logical values and states no contracts, postconditions included. There are no proof tokens and no `unsafe` exports.
8. Proofs and logical values are ghost ([RFC 0002](0002-logical-layer.md)): zero-sized markers, `Erased` for proofs and `LogicalData` for other logical data. Decisions 49 and 50 below replace the two markers: every proof is `Proof`, and each logical type is a type of its own.
9. Logical and proof fields stay in both programs, as fields of their marker's type.
10. Traits among Locus's names have no methods.
11. The parser starts from a fork of rust-analyzer's. The owner left the choice to this RFC.
12. Locus's macros are real macros, which rustc expands wherever it expands macros. Export mode accepts only invocations written directly in the crate's own code.
13. An exported crate uses its Locus dependencies through their exported interfaces, so contracts between crates work only in build mode.
14. Build mode checks Locus dependencies from their sources, so a Locus crate published for Locus users includes its `.lc` files.
15. **One lowered text for build mode.** The checking and runtime programs of build mode are one lowered text, compiled twice with different expansions of Locus's macros, with every ghost position kept as a marker. Lowering decides only what the source shows, and confirms every assumption against the facts. This revisits decision 8 of the previous draft, that proof parameters are removed, for build mode only, whose runtime program no one reads; an exported program still removes them. The [spikes](#spikes) support it.
16. **Validation.** In both modes, Locus compares the runtime program's items and typed bodies with the checking program's, for every item and body, and fails on any difference. Export mode keeps erasure textual, so its output stays readable.
17. **Interfaces and manifests.** Each verified crate gets an interface that its dependents are checked against, and each result a manifest of what it relies on.
18. **Acceptance waits for the spikes** of build mode's lowering and of the Cargo graph. A semantic slice, verifying one mutable function with a reborrow and a destructor end to end, goes with the RFC for the runtime semantics ([runtime-semantics](../../issues/runtime-semantics.md)).
19. **One library, with its macros' expansions selected by a `cfg`** that only the checking compile sets, rather than two versions of the library swapped per program. The Cargo spike found it the simplest way that works across crates; two libraries need every dependency's checking metadata kept and passed separately.
20. **Ghost bindings recorded both ways** for expansions that rustc performs: a table of lexical scopes, consulted by position, and a local `macro_rules!` dispatch, resolved by hygiene. Each alone failed a case in the spike, and both together passed every one. With decision 49 an expansion no longer needs the record, and item 9 below asks the owner to confirm that it goes.
21. **A proof hole inside another macro's input is rustc's error,** with Locus's suggestion to write `prove!(...)`, rather than a hole spelled as a macro.

**Decided by the owner after the review of this revision.**

22. **Validation compares every item and body,** not only those in the facts. The checking `cfg`'s name, `__locus_check`, is fixed by the shared library, and a crate's sources may not test it.
23. **Procedural macro crates that depend on Locus crates are checked.** Doctests of crates that depend on Locus crates are refused until [cfg-corner-cases](../../issues/cfg-corner-cases.md).
24. **rustc reads the lowered text at the sources' own paths,** through the driver's file loader, so relative paths and source locations are those of the `.lc` files.
25. **Paths to Locus's logical types are among the names lowering assumes,** confirmed against the facts like the others.
26. **In export mode, a function used as a value keeps its ghost parameters, as markers,** so that a crate that builds also exports.
27. **In export mode, Locus's macros may appear in items that an attribute macro or a derive takes,** whose results validation compares, but not in a function-like macro's input or in any macro's output.

**Decided by the owner after the second review of this revision.**

28. **The export guard tests every `cfg` name in the crate,** `.rs` files included, so a mostly-Rust crate declares its configurations.
29. **Validation compares a precise description of each item:** signature, ABI, symbol and linkage, effective visibility, layout attributes, target features, impls, and the constants the logic reads. A spike of validation, with deliberately changed bodies, callees, attributes, layouts, symbols, and visibilities, comes before the semantic slice.
30. **Interfaces carry each function's inferred effect,** and name a dependency's items by its identity and interface hash, not by its crate name alone.
31. **Lowering gives `#[path]` to every module beneath one it points,** so the module tree is unchanged.
32. **Locus is the only `RUSTC_WRAPPER`,** and refuses a build that configures another.
33. **Lints in export mode** come from compiling the exported program.

**Decided by the owner after the third review,** following the recommendations.

34. **Locus is built in stages, starting with the `locus!` macro.** Locus code is first written in `.rs` files inside `locus!` blocks and checked with `cargo locus check`, and the language is built out on that. Build mode, with `.lc` files, follows, and export mode if it is still wanted.
35. **A dependency is named by Cargo's package identity,** its name, version, and source, with its crate target, and a Locus dependency also by its interface hash. Whether an earlier check still applies, given features, configuration, sources, assumptions, and Locus's version, is the manifest's question, not part of every name.
36. **Lowering rewrites the `#[path]`s the source writes** beneath a module it points, as well as adding the missing ones.
37. **Validation compares foreign linkage:** `#[link_name]` on a foreign item and `#[link(...)]` on its `extern` block, with their options.

**Decided by the owner after the fourth review,** with [RFC 0002's](0002-logical-layer.md#unresolved-questions) decisions 44 to 50.

38. **Summaries and manifests follow the trust model's new entry.** Code that Locus does not verify is taken to be sound Rust (`assumed.unsafe.sound`), so a function's summary records what it may write, whether it lets an address escape, and whether it may run unverified code or an unknown `unsafe` operation, and no longer what access it may retain. A manifest lists the unverified crates whose calls a result relies on. RFC 0002's decision 54 narrows a summary to what a function may write.
39. **In the first stage, what other crates can reach is plain Rust,** by export mode's interface rules, and `pub(locus)` means `pub(crate)`. This settles item 5 of the list below as it stood, and removes `assumed.stage1.dependents` from the trust model: a plain-Rust caller could forge a marker with no `unsafe` code.
40. **The driver records which crates contain `unsafe` code,** and a manifest lists the named crates and the `unsafe` code of verified crates that a result relies on (RFC 0002, decisions 52 and 53). With RFC 0002's decision 54, what is listed for a verified crate is each unverified function that contains `unsafe` code, and each `unsafe impl`.
41. **`open!` is among Locus's macros** (RFC 0002, decision 51). Its contents are runtime code, lowered like the code around it, and it expands the same way in both programs, to those contents under a guard. Withdrawn by decision 43 below.
42. **Functions are verified or unverified** (RFC 0002, decisions 54 to 57). The logical pass translates only verified functions, and reads the others' types. `#[unverified]` is among Locus's names: an attribute that expands to its item unchanged. An interface records which of a crate's functions are unverified, and this RFC now says verified and unverified where it said checked and unchecked code.

**Decided by the owner on 2026-11-16,** with [RFC 0002's](0002-logical-layer.md#unresolved-questions) decisions 61 to 72.

43. **The pipeline follows the boundary of RFC 0002's decisions 61 to 67.** Unverified code is given nothing that exposes logical data, and that is checked for each instantiation, so an interface records, for each type parameter of a verified generic function, whether it reaches unverified code. `#[verified]` joins `#[unverified]` among Locus's names, and `open!` leaves them.
44. **An item that exposes logical data is visible only to crates that Locus verifies** (RFC 0002, decisions 68 and 73). In the first stage and in export mode, other crates reach no function with a proof in its signature and no field that holds logical data or that a proof reads. A sealed struct may be public.
45. **`locus!` blocks carry the full syntax, and come first.** This confirms decision 34. A block is erased by itself and checked with its crate, so a crate can be built by plain rustc without being checked; a result holds for the sources that Locus checked. `.lc` files follow, so that the macro is not needed for the syntax.

**Decided by the owner on 2026-11-21,** with [RFC 0002's](0002-logical-layer.md#unresolved-questions) decision 88.

46. **A manifest lists what crosses to foreign code.** Foreign code is given and gives only plain data (RFC 0002, decision 88), and is trusted to keep to the signatures declared for it (`assumed.foreign`). A function that foreign code can call has no `unsafe` code in its body, so a manifest lists each such function of the build, and each foreign function declared `safe` that verified code calls.

**Confirmed tentatively by the owner on 2026-11-22,** with [RFC 0002's](0002-logical-layer.md#unresolved-questions) decisions 89 to 96.

47. **Interfaces, manifests, and the first stage follow RFC 0002's decisions 90, 92, and 94.** An interface records, for each type parameter of a verified generic function, whether its values never reach unverified code, are only lent to it, or reach it otherwise, with the type of each value lent. A function's result lists the assumptions in its own body and in the functions it calls, and the program's manifest lists every assumption in every crate it links. In the first stage, as in export mode, a field that other crates can reach may have a sealed type. Each is confirmed tentatively, and RFC 0002's item 26 keeps it open.
48. **The driver's record of `unsafe` code counts what foreign code can call** (RFC 0002, decisions 95 and 96). For a crate that Locus does not verify, the driver records that the crate contains `unsafe` code when it has a function that foreign code can call, a static that foreign code can reach, or a naked function, even when the crate writes no `unsafe`. RFC 0002 says which those are: a function whose ABI is not Rust's, a function or a static with `#[no_mangle]` or `#[export_name]`, and one that a `sym` operand of `asm!`, `global_asm!`, or `naked_asm!` names. rustc's lint `unsafe_code` does not give this record: it reports `#[no_mangle]`, `#[export_name]`, and `global_asm!`, and reports neither a function whose ABI is not Rust's nor a naked function (tested with rustc 1.97). In a verified crate, a naked function is an unverified function with `unsafe` code, and a manifest lists it as one. What a manifest lists for `assumed.foreign` includes the functions that a `sym` operand names. Confirmed tentatively, and RFC 0002's item 28 keeps it open.

**Decided by the owner on 2026-11-22 and 2026-11-23,** on the lowering. With one marker for every logical value, rustc cannot tell `Int` from `Nat`, so a logical type cannot implement a trait ([traits-on-logical-types](../../issues/traits-on-logical-types.md)). Six spikes asked how to change that ([Spikes](#spikes)), and two lowerings were left standing. RFC 0002's decisions 97 to 110 are the language's side of what the owner chose, and these four are the pipeline's. The owner's words are quoted, and they decide what they say. What the writing chose beyond them is items 6 to 10 below.

49. **A Rust type for each logical type, and rustc types logical code.** Shown the two lowerings left standing, with what each spike measured, the owner said: "Yes I'm aligned with option 1 now and conversion policy is okay." The first is: each logical type is a zero-sized Rust type of its own, the same in both programs (`Int`, `Nat`, `Bool`, `Prop`, `Seq<T>`, and each logical struct or enum that a program declares); every proof is still one marker, because a proposition is not a Rust type; and lowering also writes each piece of logical code a second time as a Rust expression over stubs, in a closure that is never called, a typing skeleton, so that rustc types logical code in both programs and a ghost local takes its type from it. The second, not chosen, is one marker with traits split into two families ([Alternatives](#alternatives)). The conversion policy is RFC 0002's decisions 97 and 98. This replaces `LogicalData` in decision 8, and this RFC's earlier rule that logical code is never given to rustc as code.
50. **Names that rustc can show read as the programmer thinks.** The owner proposed: "why not replace all proofs with ZST called "Proof" literally vs Erased? Also, this closure/function 'typed' - looks like we will use it for logic blocks - if so, maybe call it 'logic' (not that programmer will ever see it)". And then, of the principle that every name lowering writes and rustc can show should read as the programmer thinks of it: "Yes, using symbols to make error messages good is the right principle, please adopt it". So the marker for every proof is `Proof`, which replaces `Erased` in decision 8; the helpers are named for the construct, `logic` and `prove`; the stubs are in `locus::logic`; and the library gives its helper traits messages that say what to write, with `#[diagnostic::on_unimplemented]`.
51. **`Model` is an `unsafe trait`.** The snapshot and the model of a type are given to rustc by implementations that lowering writes where the type is declared (RFC 0002, decisions 106 to 108). A program never writes `unsafe` for it: lowering does. Asked why it is `unsafe`, and told that a sealed Rust trait cannot be implemented downstream at all, that the real rule is the driver refusing any implementation that lowering or Locus's library did not write, and that `unsafe` is a second guard for builds in which the driver does not run, the owner said: "And yes, okay with unsafe trait for Model."
52. **Nothing changes in how facts enter the logic.** "Fact is always proof in the result or small/finite list of facts language infers from some constructs (which we have already defined). Don't want to go back on that. And don't want to introduce pure functions either right now." So rustc's typing of logical code gives Locus types, and no facts. This confirms decision 3.

**Decided by the owner on 2026-11-23,** on what decisions 49 to 52 had left for the owner. Told that two issues propose words for three sentences of the vision, and given five choices made in writing those decisions, each as one line, the owner said: "Follow all your recommendation - please go ahead and reflect them in the PR".

53. **The choices put to the owner stand as written, and the vision says what the lowering does.**
    - **The vision is edited,** in the same change. Its sixth commitment now says that logical code reaches rustc "only as code that it types and never runs, and logical items only as stubs that are never called". Its first names the reserved names as its one exception: `__locus_check`, and an identifier that begins with `__locus_` or `__logic_`. Its fifth gives a snapshot "to every runtime value whose type it can see" (RFC 0002, decision 107). The words are those that [typed-logical-code-and-the-vision](../../issues/typed-logical-code-and-the-vision.md) and [unmarked-fields-and-the-superset](../../issues/unmarked-fields-and-the-superset.md) proposed; where an issue proposed two, the first is taken. The owner was told that the issues propose words, and did not read the words themselves.
    - **Both prefixes are reserved,** `__logic_` and `__locus_` (item 6).
    - **A ghost local's type never flows backward,** as a check that Locus makes after rustc (item 9).
    - **Every piece of logical code has a skeleton,** with no fallback to an untyped marker (item 9).
    - **Plain Rust does not implement a trait that has a logic function,** which follows from where a trait's stubs live.
    - **An `unsafe impl Model` written in Locus code is rejected, and what lowering writes is not the program's `unsafe` code,** so `#![forbid(unsafe_code)]` still compiles (item 8).
    - The owner saw each as one line, and not the rule texts. Items 6 to 10 below stay, for their reasons and for what they list beyond these.

**Still for the owner.** Each is written into the spec and the examples as proposed where an example can show it, so accepting this RFC accepts it. Item 1 has no rule yet, because an example would need a `.rs` file beside a `.lc` one, which the litmus format cannot yet express:
1. `.rs` files may not name logical items, as proposed.
2. Logical code in closures and async blocks, and reads of packed fields, are rejected for now, as proposed.
3. Traits among Locus's names also have no associated functions or constants (an extension of 10 above). Since decision 49 such a trait holds, for each logic function it declares, a stub that is an associated function to rustc, under a reserved name that no program can write (item 6). So the rule now says: none that a program can name.
4. Until [cfg-corner-cases](../../issues/cfg-corner-cases.md) decides more, an exported crate whose verified code tests target properties other than pointer width must declare its configurations.
5. **Which marker a `logic!` gives** when its code is a proof, such as `logic! { add_zero(n) }` for a lemma `add_zero`. Settled by decision 49: the marker is the type that rustc gives the code's typing skeleton, here `Proof`, and lowering does not decide it.
6. **A program may not write a name that begins with `__logic_` or `__locus_`** (`compat.names.reserved`). The owner adopted the principle on names, and saw no reserved prefix. It follows from where a trait's stubs live: in the lowered trait and the lowered `impl`, where a name of the program's could meet them, and under a name that every crate must agree on, so it cannot be chosen fresh. The spikes found no other placement that works across crates. The rule rejects such an identifier wherever the source writes it, the tokens of a macro's input included, which is the reading that accepts less. Two things come with it. Plain Rust code cannot implement a trait that has a logic function. And it is a second exception to commitment 1 of the vision, which names one: [typed-logical-code-and-the-vision](../../issues/typed-logical-code-and-the-vision.md) asks the owner for the words.
7. **How rustc is given snapshots and models:** two traits, one for the snapshot and `Model`; a generated snapshot type for each struct and enum; and the generated implementations deferred, while a declared model is not. The owner approved that a struct with an unmarked field of a type that Locus cannot see still compiles (RFC 0002, decision 107). The form that gives it is hand-written Rust in a spike, and rests on a behaviour of rustc that nobody has established as intended ([unmarked-fields-and-the-superset](../../issues/unmarked-fields-and-the-superset.md)).
8. **What decision 51 needs beside the owner's words:**
    - that the driver refuses a hand-written implementation in every crate it compiles, verified or not, and knows lowering's by identity;
    - that `unsafe impl Model` written in Locus code is rejected, which is the reading that accepts less;
    - that what lowering writes is not the program's `unsafe` code, so that `#![forbid(unsafe_code)]` still compiles, no manifest lists it, and `contracts.unsafe-impls` does not reject it on a type with proof fields.
9. **What goes with the one marker, and what stays.**
    - **Every piece of logical code has a skeleton,** and there is no fallback to an untyped marker. The first round's fallback could not follow a macro that rustc expands.
    - **The record of ghost bindings that decision 20 kept for expansions is no longer written,** since a skeleton is written the same way for a ghost root and a runtime one.
    - **`logic.ghost.types-forward` stays,** as a check after rustc. rustc infers a ghost local's type backward from a later use, and RFC 0002's decision 16 says that Locus never does. Keeping the rule is the reading that accepts less. No spike built the check, and with rustc doing the typing its only work is to refuse what rustc inferred. Dropping the rule would accept `let mut v = Vec::new();` followed by ghost pushes.
    - **Whether build mode still needs its second compile** is not taken up. The checking `cfg` now selects only what Locus's macros carry to the driver.
10. **Smaller choices of the writing:**
    - the names beyond those the owner's words give, which are the spikes': `prop`, each stub's name, `Stack_Snap` for a snapshot type, and how a logical enum's variants print;
    - what the marker of a logical value prints under `Debug` is not decided. Only `Proof` is pinned by an example;
    - the owner had named `LogicalData` so that rustc's errors say that a value is logical (RFC 0002, decision 10). With a type for each logical type an error names the type itself, such as `Nat`, and no longer says that it is logical;
    - in build mode, a read in a skeleton counts as a use for rustc's lints;
    - what a closure that is never called costs in an unoptimized build is not measured, and commitment 2 of the vision says that proofs cost nothing at run time;
    - export mode's markers are the exported program's own `Proof` and one marker for each logical type it holds. How its two programs are kept to the same types, when the checking program types logical code against the library's logical types, is not worked out, and waits for export mode ([distribution-and-export](../../issues/distribution-and-export.md));
    - commitment 6 of the vision says that logical code reaches rustc only as input to Locus's macros. It now reaches rustc as code that rustc types and never runs. `VISION.md` is not edited here, and [typed-logical-code-and-the-vision](../../issues/typed-logical-code-and-the-vision.md) proposes the words.

**Left to other RFCs**, each with its issue:
- Logical types, propositions, proofs, models, and contracts: [RFC 0002](0002-logical-layer.md), proposed with this one.
- Traits on logical types, beyond where a trait's stubs live: [traits-on-logical-types](../../issues/traits-on-logical-types.md). Locus's collections: [standard-library](../../issues/standard-library.md). Permissions over memory: [permissioned-pointers](../../issues/permissioned-pointers.md).
- The remaining syntax of propositions and proofs: [proposition-syntax](../../issues/proposition-syntax.md).
- The runtime semantics: how typed bodies become statements in the logic, THIR or MIR, where borrows end, integer overflow across profiles, and whether observations of a value with proof fields can be narrowed to the fields its propositions read: [runtime-semantics](../../issues/runtime-semantics.md).
- Distribution: package contents, including how an exported package's manifest names its exported crate root; how configurations are declared; dependency versions; how Locus recognizes Locus dependencies; and keeping rustfmt off exported files: [distribution-and-export](../../issues/distribution-and-export.md).
- Logical code in closures and async blocks, Locus syntax inside other macros' input, and reads of packed fields: [closures-async-packed-macros](../../issues/closures-async-packed-macros.md).
- Corner cases of `cfg` and compiler flags, and doctests in build mode: [cfg-corner-cases](../../issues/cfg-corner-cases.md).
- Editor support, likely a language server that runs rust-analyzer on the checking program and maps results back through origin maps.
- Incremental checking and caching of facts.
