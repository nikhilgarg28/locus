# RFC 0001: The compiler pipeline

- **Status:** proposed
- **Issue:** none
- **Supersedes:** none

## Summary

Locus compiles a Cargo crate in four steps:

1. **Parse.** Locus parses its `.lc` sources, Rust plus Locus syntax, into lossless syntax trees.
2. **Lower.** Locus writes each `.lc` file as one Rust text, the **lowered program**. In it, proofs and logical values are zero-sized markers, `Erased` and `LogicalData`, in every position they occupy; logical items are stubs that only resolve names; and logical code is an invocation of a macro from Locus's library. Lowering decides only what the source shows, never an expression's type.
3. **Check.** rustc compiles the lowered program through a small driver, with the *checking* expansions of Locus's macros selected. That is the **checking program**: its macros turn every place logical code reads into a borrow, and the driver reports what Locus needs: the types of those places, what names resolve to, and the typed bodies of the functions being verified. rustc's borrow checker decides whether logical code may read what it reads. Locus then checks the logical layer against those facts, constructs proofs, and has the kernel check them.
4. **Build or export.** Only when the check passes, Locus produces the **runtime program**, and validates that it has the typed bodies of the program it checked, apart from logical code:
   - **Build mode,** the default: rustc compiles the same lowered program again, with the *runtime* expansions, in which Locus's macros give the markers and nothing else. `cargo locus build` builds it under `target/`, in the configuration being built. Every crate in the build that depends on a Locus crate is checked too, so contracts hold across crates.
   - **Export mode:** `cargo locus export` erases the logical layer by editing each `.lc` file into readable Rust, written beside the sources as `.rs` files that Rust users build with plain Cargo. The crate's public interface must then be plain Rust: it has no logical values and states no contracts.

**Locus is built in stages** ([Stages](#stages)). The first needs neither `.lc` files nor a compiler wrapper: Locus code is written in `.rs` files inside `locus! { ... }` blocks, a macro that does what lowering does, and `cargo locus check` checks the crate. The language is built out on that first stage. Build mode follows, and export mode if it is still wanted.

Locus writes its own parser, the lowering, the logical layer, the kernel, erasure, and diagnostics. Everything else that Rust already does is rustc's: name resolution, macro expansion, type inference, trait resolution, ownership and borrow checking, lints, and code generation.

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

[RFC 0002](0002-logical-layer.md) defines the logical layer that this pipeline checks and erases, and the two RFCs are proposed together. Its treatment of proofs as ghost values, which no Rust code can forge, is why the checking and runtime programs contain markers rather than proofs, and why a crate's contracts are visible only to code that Locus checks.

## Design

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
 logical pass: resolve, type-check, elaborate ............. Locus
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
| Ownership and borrow checking, including what logical code reads | rustc |
| Lints | rustc, on the runtime program |
| Checking logical types, logic functions, propositions, and models | Locus |
| Checking every proof and logical value, none of which reach rustc | Locus ([RFC 0002](0002-logical-layer.md)) |
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
   - **One text, two expansions.** The macro writes one text, and the library's macros put `#[cfg(__locus_check)]` in their own output, so the `cfg` is evaluated in the user's crate: the checking expansion under `--cfg __locus_check`, the runtime expansion otherwise. In the spike, 20 accepted examples compiled both ways and printed their `run` values in both, and two rejected ones failed only in the checking expansion, at their annotated lines (E0382, E0502). Every crate that uses Locus declares `cfg(__locus_check)` in its `check-cfg`, or rustc warns at every block.
   - **`cargo locus check` checks.** It compiles the crate through the driver with the checking `cfg`, runs the logical pass and the kernel, and validates the runtime program against the checked one by compiling the crate a second time without the `cfg`, as build mode does.
   - **`cargo build` builds, unchanged.** To Cargo, rustc, rust-analyzer, and the crate's users, a crate with `locus!` blocks is an ordinary Rust crate that depends on Locus's library. rustc's errors point at the user's lines in both expansions; rustfmt leaves a block exactly as written, plain Rust inside it included; and rust-analyzer expands the blocks and resolves what is in them, one expansion at a time (tested). Building does not check: a result holds for the program built from the sources and the configuration that `cargo locus check` checked, so a project runs the check in CI, as it runs its tests.
   - **Code outside `locus!` blocks is plain Rust,** as `.rs` files are in the full toolchain. The driver reports what every path resolves to, and a path outside a block that names a logical item, or calls a function with ghost parameters, is an error.
   - **Ghost parameters stay, as markers, in both expansions,** as in build mode. Removing them from the runtime expansion, as export mode does, would give a function two signatures, which breaks a plain-Rust caller in whichever expansion it was not written for, and would need, at each Locus call site, the callee's signature to know which arguments to drop, which a macro over one block does not have (tested).
   - **Within the crate, the semantics are build mode's.** Every rule of this RFC and of RFC 0002 that holds in build mode holds, and the litmus examples are checked with each file's text inside `locus! { }`. The two examples about `.lc` files, `include-from-source` and `reserved-check-cfg`, apply once there are `.lc` files.
   - **Other crates are not checked.** Interfaces, contracts across crates, and checking the crates that depend on a Locus crate wait for build mode. A plain-Rust crate can call a function that needs a proof, and pass a marker that proves nothing: it compiles, and reaches the division by zero the precondition rules out (tested). So a crate that uses a Locus crate is trusted to call it as checked code would, with genuine proofs and without building values of its invariant types, and the trust model lists this as `assumed.stage1.dependents`. The alternative, export mode's interface rules for everything other crates can reach, is for the owner (below).
   - **What a block sees.** A block sees only its own tokens, and three things follow:
     - a logical type's declaration and its `impl`s are written in one block, because another block cannot know that a name is logical (tested: E0118);
     - the library is named `locus` in the user's namespace, rather than under a fresh name, and items inside a block are macro-expanded, so a user's own item named `locus` can make the name ambiguous (tested: E0659);
     - which marker a `logic!` block gives, when its code is a proof, as a lemma's call or a logical closure that proves a `forall` is, is not decided by its tokens. That is open for lowering too (below).
   - **Not needed in the first stage:** `.lc` files and their parser, the file loader, rewriting dep-info and `#[path]`, the compiler wrapper and its identity, checking every crate that depends on a Locus crate, and export mode.
2. **Build mode, with `.lc` files.** Locus syntax anywhere in a `.lc` file, lowering of whole files, the compiler wrapper, interfaces and contracts across crates, and every dependent crate checked. A `.lc` file lowers to what its text would expand to inside `locus! { }`, so the library, the driver, the facts, the logical pass, and validation carry over, and lowering whole files lifts the first stage's limits on what a block sees.
3. **Export mode, if it is still wanted.** Readable `.rs` files for Rust users of a crate written in `.lc` files. A crate written with `locus!` blocks is already plain Rust to its users, so export mode is needed only to publish crates written in `.lc` files to Rust users.

### Sources and modes

Locus compiles one Cargo crate at a time, in the crate's real Cargo context: its edition, features, `cfg`s, and dependencies.

- **`.rs` files are plain Rust.** They are compiled as they are, and they may not name logical items: no logic functions, logical types, or proofs. The driver reports what every path resolves to, and a `.rs` path that resolves to a logical item is an error.
- **`.lc` files may also contain Locus syntax and use Locus names.** Renaming a `.rs` file to `.lc` changes nothing about the program.
- **Modules work as in Rust.** `mod name;` finds `name.lc` or `name.rs` by Rust's usual rules. When both exist, `name.rs` is the file exported from `name.lc` (below), and Locus reads `name.lc`. A crate whose root is a `.lc` file names it in its manifest, as `[lib] path = "src/lib.lc"`. Cargo accepts that (tested), and needs it, since it finds no target when `src/lib.rs` is missing.
- **Each crate has one generated module,** under a fresh name, declared by the crate root. In the lowered program it holds the prelude that makes Locus's names visible (see [Locus syntax and Locus names](#locus-syntax-and-locus-names)). In export mode, it holds the crate's own markers `Erased` and `LogicalData`, in the checking program and in the exported one, and the exported program's copy holds the configuration guard too (see [Configurations](#configurations)).
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
| Logic function | Item | `logic fn total(xs: &[u32]) -> Nat { ... }` | Rust has no item beginning `logic fn` |
| Proposition declaration | Item | `prop Sorted(xs: Seq<Nat>) { ... }` | Rust has no item beginning `prop Name(` |
| Proof type | Type | `h: @(x > 0)`, `@Sorted(xs)` | `@` cannot begin a type |
| Named tuple component | Type, including a function's result | `-> (out: u8, @(out > n))`, `Vec<(i: usize, @(i < n))>` | Tuple types have no field names |
| Proof hole | Expression | `(out, _)` | rustc rejects `_` as an expression except as an assignment target, an array length, or a generic argument, and the hole is recognized nowhere else |
| Locus-only visibility | Visibility | `pub(locus) fn halve_even(...)` | rustc accepts only `crate`, `self`, `super`, and `in` paths inside `pub(...)`, and rejects anything else with E0704 (tested) |

`@` is reserved for proofs. In a tuple struct's field, `pub (locus)` with nothing after it is Rust's: a public field whose type is a path `locus` in parentheses, which rustc accepts (tested). It keeps that meaning.

- **What "valid Rust" means here: code that rustc accepts when it is compiled.** Code that `cfg` removes is only parsed by rustc, and a Locus form there has no meaning in either language.
- **Locus syntax is not recognized inside macro invocations or attribute arguments.** Those are token trees, and their meaning is the macro's.
- **Rust's own contextual keywords can precede `fn`** (`safe`, `default`, `const`, `async`, `unsafe`, `extern`). `logic` is not one of them. If Rust ever gives `logic fn` or `prop Name(` a meaning, Locus follows Rust and respells its forms.
- **[RFC 0002](0002-logical-layer.md) adds further forms** under the same rule: proof constructors and patterns (`Even::Step(m) @ h`), `final!(r)`, logical function types, and measures.

**Locus names** are ordinary Rust items:
- the macros `logic! { ... }` for logical blocks, `prop!(...)` for propositions, and `prove!(...)` for proofs, from Locus's library;
- the derives `Logical` and `Model`;
- the trait `Model`;
- the logical types and functions of Locus's standard library, such as `Nat`.

`Logical` can only be derived, never implemented by hand. Deriving it fails unless every field and payload is logical. `Model` is implemented by hand or derived. What they mean is [RFC 0002](0002-logical-layer.md).

**Traits among Locus's names have no methods, associated functions, or associated constants.** In Rust, bringing a trait with a method into scope can change what an existing method call resolves to, or make it ambiguous. This was tested: a prelude `Model` with a `model` method made a call to a user trait's own `model` method fail with E0034. That is why Rust adds prelude traits only at edition boundaries. So `Model`'s logical function takes the value as an ordinary argument.

How the names are found:
- **They are visible the way std's prelude is.** In the lowered program, every module sees a prelude that combines std's with Locus's, so rustc resolves Locus names with Rust's own rules, in both of build mode's compiles.
  - The prelude is in the lowered program's copy of the crate's generated module, and re-exports Locus's names from its library.
  - Declaring it needs rustc's internal `prelude_import` feature, which the driver enables for lowered programs only. The spike stood in for it with an ordinary glob import, so the real prelude is untested.
- **A program's own definitions and imports take precedence,** exactly as they do over std's prelude: its own `macro_rules! prove`, `use other::prove`, `struct Nat`, or a glob import of a type named `Nat`. All of these were tested.
- **Glob-imported macros are the one exception in Rust's rules.** They collide with prelude macros (E0659), and that is already true of std's `vec!`. So does a macro defined by another macro's expansion. When rustc reports such an ambiguity involving a Locus name, Locus checks the crate again without that name in its prelude. The program's own macro keeps its Rust meaning, and that crate cannot use Locus's macro of that name.
- **Recognizing Locus's names.** Lowering assumes that each of these is Locus's:
  - an invocation of a Locus macro name, or of a name that the crate's imports make an alias of one, such as `check!` after `use prove as check;`;
  - a `derive(Logical)` or `derive(Model)`;
  - an `impl Model`;
  - a path to one of Locus's logical types, such as `Nat` in `let k: Nat` or `const B: Nat = 1;`, which decides whether a binding is shown to be ghost and whether a constant is logical code ([RFC 0002](0002-logical-layer.md#logical-code)).

  The library also declares each logical type as an alias of its marker, so rustc resolves `Nat`, `type Count = Nat;`, and a glob import of a program's own `Nat` by its own rules.

  The driver then reports which definition each assumed name actually resolved to, by the definition's identity rather than its path. If one turns out to be the program's own, Locus discards everything from that run, errors included, and lowers again, treating it as plain Rust. A crate normally needs one rustc run, and needs a second only when names collide.
- **Locus's macros are real macros.** Lowering writes every invocation of a Locus macro that it recognizes as an invocation of one of the library's internal macros, with the places the logical code reads computed from its syntax ([The lowered program](#the-lowered-program)). The rest reach rustc, which expands them like any other macro: inside another macro's input, such as `vec![prove!(...)]`, in a macro's output, and under an alias.
  - Locus's library defines `logic!`, `prop!`, and `prove!` as procedural macros that contain Locus's parser for logical code.
  - In the checking program, such an expansion observes each place that its logical code reads, with the invocation's own hygiene, and probes each name it uses. It also carries its logical code, with its spans, to the driver, so Locus checks that code like any other.
  - In the runtime program, the same invocation expands to its marker.
  - **An expansion must know which roots are ghost.** In `vec![prove!(before.a == pair.a)]`, `before` may be a logical snapshot, whose marker has no field `a`, while `pair` is a runtime value whose field `a` must be observed alone, so that a mutable borrow of `pair.b` elsewhere is not a conflict. A procedural macro sees only its tokens, so lowering tells it. Lowering records the ghost bindings in scope in two ways, and the expansion uses whichever applies:
    - a table of their lexical scopes in the lowered file, which the expansion consults by the root's position, when the root's tokens are where they were written: inside `vec![..]`, or passed through the program's own `macro_rules!`;
    - a local `macro_rules!` that lowering writes after each binding that changes which names are ghost, which the expansion invokes with the root's own span, so that rustc resolves it by hygiene, when another macro has rebuilt the root's tokens.

    The spike tested each alone and both together on the decided examples and ten further cases, including a runtime shadow of a ghost name, a ghost named inside a program's macro body, nested functions, and an attribute macro that rebuilds every span. Each alone failed some case, and both together passed every one.
  - **A wrong classification is always an error,** never a program with another meaning. A ghost root observed as a place is a field access on a marker, which has no fields (E0609). A runtime root observed whole conflicts wherever the precise observation would (E0502). So the mechanism decides only which correct programs are accepted.
  - **Lowering never rewrites tokens inside another macro's input.** Rewriting there would make `stringify!(prove!(x > 0))` produce the rewritten text, against [`prove-in-stringify`](../../litmus/compatibility/prove-in-stringify.accept.lc), as the spike showed. A Locus macro name that is never expanded, such as inside `stringify!`, is only tokens.
  - **A proof hole inside another macro's input is not a hole.** `vec![_]` reaches rustc as `_`, which is rustc's error; Locus suggests `prove!(...)` there.
  - Every field such an expansion can name exists in the lowered program, logical fields included, because they stay as fields of their markers' types, and every ghost binding stays bound to its marker.
  - In export mode, the exported program cannot depend on Locus's library, so erasure removes every invocation as text. It can do that only for one written directly in the crate's own code, so export mode rejects invocations in any macro's output or in a function-like macro's input. An item that an attribute macro or a derive takes is the crate's own code: erasure edits it before the macro sees it, and validation compares the result ([RFC 0002](0002-logical-layer.md#export-mode)).

### How facts enter the logic

There is no wall between Rust and Locus inside a crate. Plain Rust and Locus code mix freely, and every file goes through the pipeline. Soundness comes instead from the only four ways facts about runtime values can enter the logic:

1. **Snapshots and models:** observing a runtime value as what it is, its snapshot, or through its type's model, what it means.
2. **Typed bodies Locus understands:** rustc's typed bodies, translated into the logic, together with what Rust's ownership and aliasing rules guarantee: a call changes only what its effect allows, which is what it is given mutable access to and what escaped to it earlier, and a mutable borrow's final value is what its place holds when the borrow ends.
3. **Assumed specifications:** each declared with a reason, and listed by the audit.
4. **Invariants of the program's own types,** stated by their proof fields, as described below.

Each is sound no matter who wrote the code.

- **Snapshots and models read only data that cannot change while the value is shared.** Rust's aliasing rules guarantee this, except for the contents of an `UnsafeCell` and the targets of raw pointers, and a snapshot holds neither.
  - A snapshot is derived from a type's definition, field by field. A model is a logic function of the snapshot.
  - Types from the standard library with `unsafe` internals (`Vec`, `Rc`, `String`) get snapshots and models declared by Locus's library, each as a trusted entry with its reason.
  - `Cell`, `RefCell`, `Mutex`, and the atomics have no model, and their snapshots hold nothing of their contents.

  So **a call Locus knows nothing about, taking `&x`, never changes `x`'s snapshot or model**: Rust freezes what they hold, and they never hold what Rust does not freeze.
- **A call keeps only the facts it cannot change.** Each checked function's effect, what it may write and what access it may retain, is inferred and kept in its crate's interface; code whose effect is unknown is assumed to write, and keep, everything its access allows. `unsafe` code can write a place through a raw pointer with no write that Locus sees, so a place whose address reaches a raw pointer, foreign code, inline assembly, or unknown code *escapes*, and a fact about it holds only until the next call or `unsafe` operation that may write it. Rust's provenance rules make that enough: code reaches memory only through a pointer derived from one it was given ([RFC 0002](0002-logical-layer.md#the-boundary-with-unchecked-code)).
- **Plain Rust in a Locus crate is still checked by Locus.** For example, plain Rust that assigns a field that a proof field reads must keep that proof field true. Locus proves it for the new value as if a hole were written there, and rejects the assignment when it cannot.
- **Code Locus cannot translate is opaque:** dependencies, trait objects, inline assembly, and anything else outside what Locus models. It runs, but its results are known only by their types, and anything it receives through `&mut` may come back as any value of its type.
- **Locus never believes what it did not check.** Proofs and logical values are ghost ([RFC 0002](0002-logical-layer.md)): both programs hold them only as zero-sized markers, `Erased` and `LogicalData`, which carry no proposition and no value. Locus tracks what a value holds itself, only through code it checks and standard library code whose specification it has. Passed to any other code, ghost contents are forgotten. So no Rust code, through `Any`, specialization, or anything else, can convert or forge a proof, and Locus never relies on Rust's generics being parametric.
- **Invariants hold for every value.** Every value of a type with proof fields satisfies them. Only checked code constructs the type or writes the fields its proofs read. In build mode, every crate that can name the type is checked. In export mode, other crates can neither construct it nor reach those fields. So a value that comes back from opaque code still satisfies its invariant: opaque code can only move it, copy it, drop it, or call the type's own checked functions. `unsafe` code could do more, and keeping the invariant is an obligation Locus adds for it, beyond Rust's ([RFC 0002](0002-logical-layer.md#unsafe-code)).
- **Statics.** Logical code may read only immutable statics, and no static that code outside the program can write, a `static mut` or one that is `extern`, `#[no_mangle]`, or `#[export_name]`, holds logical values that are not completable ([RFC 0002](0002-logical-layer.md#logical-code)).

The details of models are [RFC 0002](0002-logical-layer.md).

### The lowered program

The lowered program is what rustc compiles: once with the checking expansions of Locus's macros, as the checking program, and in build mode once more with the runtime expansions, as the runtime program. A [spike](../../spikes/one-text-lowering/README.md) lowered 30 cases by hand, and compiled every one both ways with stable rustc 1.97.

- **rustc reads it at the sources' own paths.** The driver gives rustc a file loader that serves each `.lc` file's lowered text, and lowering points each `mod` declaration that finds a `.lc` file at that file with `#[path]`. A module that rustc loads through `#[path]` looks for its own submodules beside its file, not in a directory named after it (tested: E0583), so lowering also gives a `#[path]` to every `mod` declaration beneath one it pointed, `.rs` files included, naming the file that rustc's rules would have found without it, and the module tree is unchanged. That includes a `#[path]` the source already writes. Beneath a pointed module, rustc resolves a `#[path = "leaf.rs"]` inside an inline `mod inner { }` from another directory: `src/inner/leaf.rs` where the source meant `src/a/inner/leaf.rs` (tested). So lowering rewrites it to name the file the source meant. So a path that the source resolves from its own file, in `include_str!`, `include!`, `#[path]`, or the common `#![doc = include_str!("../README.md")]`, resolves as it does in the source, and `file!` names the `.lc` file. A copy of the lowered text is written under `target/locus/lowered/`, where it can be opened, compared, and compiled by hand when something goes wrong.
- **Lowering keeps each source line on the same line of the lowered program where it can,** and appends synthesized items at the end. Raw rustc output is then readable when debugging Locus itself. The origin map, not the alignment, is the authority.
- **Locus's library crate holds its markers, its macros, its probes, and the stubs of its logical items,** such as `Nat` and `Model`. Its macros expand one way in the checking program and another in the runtime program, selected by a `cfg` that only the checking compile sets ([Running rustc](#running-rustc-locus-driver)). rustc receives it under a crate name the program does not use.
- **New names are fresh.** Every name Locus introduces, such as the library's crate name and the generated module, is chosen so that it appears nowhere in the crate's sources, so generated items cannot collide with the user's, even under `#![no_implicit_prelude]`. The one fixed name is `__locus_check`, the `cfg` that selects the checking expansions, which the shared library is compiled with. A crate's sources may not test it ([Running rustc](#running-rustc-locus-driver)).
- **The checking program is compiled with `--cap-lints allow`.** Lints there would be artifacts of the lowering, and a user's `#![deny(warnings)]` or `forbid` must not turn synthesized code into errors. This was tested. Real lints come from building the runtime program, where lowering marks the items it synthesizes, and ghost bindings and fields, with `#[allow]` for the lints they would otherwise cause.

Logical code is never given to rustc as code: its operators mean different things from Rust's (`+` on `Nat` cannot overflow), and its types do not fit Rust's (a logical `List` can contain itself without a `Box`). Its tokens travel inside invocations of the library's macros, which in the checking program observe what they read and carry the code to the driver, and in the runtime program are dropped. What rustc sees:

| In the source | In the lowered program |
|---|---|
| Runtime code | Copied unchanged |
| A parameter, local, or result whose type is logical, proof parameters included, and every argument for one | Kept, with its marker's type: `fn halve(n: u32, even: Erased) -> u32` |
| A proof type `@P` | `Proof![P]`, a type macro of the library that is `Erased` |
| A logical type written as a path, such as `Nat` or `Seq<Nat>` | Kept. The library declares each logical type as an alias of its marker, so rustc resolves the path by its own rules; a generic one is a projection alias, because `type Seq<T> = LogicalData;` is E0091 (tested) |
| A field whose type is logical, a logical type argument, a logical tuple component | Its marker: `Vec<LogicalData>`, `Option<Erased>`, `(u32, Erased)` |
| A named result `(out: T, @P)` | `(T, Erased)`: tuples keep their shape |
| A call of a runtime function whose result is entirely ghost | Kept as written: it returns the marker |
| A logic function or `prop` declaration | A stub function with the same name, generics, and signature, whose body panics if it is ever run |
| A `#[derive(Logical)]` type | A stub struct, with an alias of that name, because an inherent `impl` on an alias of a foreign type is E0116 (tested) |
| An `impl Model`, or `#[derive(Model)]` | A real implementation of the library's `Model` trait, with its logic function as a stub beside it, so that rustc checks `T: Model` bounds and coherence |
| A proof hole `_`, and `final!(r)` | An invocation of a library macro that gives the marker; `final!` reads no place |
| A `logic!`, `prop!`, or `prove!` invocation that lowering recognizes, a proposition in a type annotation inside a body, or an operator on bindings shown to be ghost | An invocation of a library macro that lowering writes, listing the places the logical code reads and the names it uses: `::__locus::proof!([&p.b] [total] total(p) > 0)`. In the checking program it observes each place and probes each name, then gives the marker; in the runtime program it gives the marker alone |
| Any other invocation of a Locus macro | Expanded by rustc, as [Locus syntax and Locus names](#locus-syntax-and-locus-names) describes |
| A proposition in a signature or a struct field's type | Inside `Proof![...]`; nothing to observe |
| `pub(locus)` | `pub` |

**Lowering decides only what the source shows:**
- Locus syntax, from the tokens;
- which names are Locus's, from the crate's items and imports, confirmed against the facts;
- which bindings are shown to be ghost ([RFC 0002](0002-logical-layer.md#logical-code)), from lexical scopes;
- which places logical code reads, from its syntax.

It never needs an expression's type or a callee's signature. rustc resolves the rest with the markers in place, as the spike showed:
- `gates.first().unwrap().admit(n, pos)` resolved to `Gate::admit`, and `Some(ok)` to `Option::<Erased>::Some`, with no help from lowering.
- The traps of removing ghost arguments before rustc became loud. With an inherent `go(&self, a, h: @P)` and a trait method `go(self, a)` in scope, `x.go(1, h)` is E0061, where the previous design silently called the trait method. A proof passed to `make<T: Default>` is E0277, `Erased: Default`, where it silently instantiated `make::<()>`.

**What Locus learns from the facts afterwards.** The spike's driver reported, from rustc's type-checking results, even when they contain errors:
- the callee of each call, with its generic arguments;
- which arguments fill parameters of marker type, and which generic parameters markers flow into;
- which locals hold markers;
- which functions with marker parameters are used as values, and as what type.

From these, Locus:
- works out which logical type each marker stands for, forward from signatures and annotations;
- confirms each assumption lowering made, by the identity of the definition each name resolved to;
- rejects what rustc accepts but Locus must not. rustc rejects coercing `halve` to `fn(u32) -> u32` (E0308) and `[7].map(halve)` (E0593) by arity, but accepts `black_box(halve)`, a pointer of type `fn(u32, @P) -> u32`, and `fold(10, halve)`. RFC 0002 rejects these. rustc also accepts a `Vec<Nat>` used as a `Vec<Seq<Int>>`, both `Vec<LogicalData>`, which Locus's own typing rejects.

**Stubs exist only to resolve names.**
- Runtime code never refers to a stub, so no stub is ever a runtime value. Only probes and other stubs mention them.
- A stub type is an empty, zero-sized type whose generic parameters are carried in a `PhantomData` field.
- Proofs in stub signatures are `Erased`.
- Stub functions are `const fn` where Rust allows, so that probes work in constant contexts. Probes are type-checked but never evaluated, and stub bodies panic rather than loop, so that a mistake fails rather than hangs constant evaluation.
- A probe writes every type argument of a stub explicitly, as a marker, so that it never asks rustc to infer one. Without that, a probe of an enum's variant was E0282 in the checking program alone (tested).

**Proofs and logical values reach rustc only as markers.** So rustc never computes a type that involves a proposition, and Locus alone checks every proof against the proposition its position requires, including the proof for every proof parameter at every call.

**Logical and proof fields stay, as fields of their markers' types,** in structs and enum variants, in both programs. So fields keep their positions, generic parameters stay used, and logical code that reads or writes such a field is checked as a borrow of that field alone.
- Constructing the type supplies the marker for the field, and a pattern that binds it binds the marker.
- Reading the field is logical code, and is observed as a shared borrow of the field.
- An assignment to a logical data field is a ghost statement. Its right-hand side is logical code, which gives the marker, so rustc checks the write as a write.
- A derive sees the field as its marker. On a proof field, `Erased` lets every standard derive but `Default` work; the spike's eight derives on a struct with a proof field compiled and behaved. On a logical data field, `LogicalData` makes `PartialEq`, `Hash`, `Ord`, and `Default` fail (E0369, E0277), as RFC 0002 intends. Others, such as serde's, fail on either kind unless told to skip it.

**Logical code contains no runtime evaluation.** It reads places, and it never calls runtime functions, mutates, or has effects; the arguments of a logical call are logical code too. A runtime function that returns a proof is runtime code: it is called, and kept, in both programs.

**Ghost bindings hold markers in both programs,** so that later invocations of Locus's macros can name them, and rustc infers their types as it infers any other.

**Observations are how rustc checks what logical code reads.** A place that logical code reads is checked as a shared borrow at the point where the logical code reads it, ending immediately. So rustc's borrow checker rejects logical code that reads a value after it has moved (E0382 in the spike), a place while it is mutably borrowed (E0502), or through a reference that is no longer live. Observations follow Rust's rules exactly:
- two-phase borrows apply as they would to any shared read;
- as in Rust, code after a `return` is not borrow-checked, and logical code there is never evaluated;
- as for Rust's field access, the place's type must be known where it is read. Logical code that reads `p.a` before `p`'s type is inferred is an error that asks for an annotation (E0282 in the spike's checking program).

Observations come only from logical code inside bodies:
- **Propositions in signatures and struct field types need none.**
  - A precondition reads parameters at entry, which are always readable.
  - A postcondition reads parameters at entry, the function's results, and the final values `final!(x)` of its `&mut` parameters ([RFC 0002](0002-logical-layer.md)). These are well defined however the body exits, so it does not read anything at a point in the body.
  - A field's proposition reads the struct's own fields.
  - Their types come from item signatures in the facts.
- **A proposition in a `let` type annotation is read after the initializer,** where the `let`'s bindings come into scope, and is observed there.
- **Logical code inside a closure or an async block is rejected for now:** an observation there would become a capture, and change what the closure or future captures. A ghost binding named there would become a capture of its marker too.
- **Logical code that reads a packed field is rejected for now:** Rust does not allow a shared borrow of one.

Both cases are [closures-async-packed-macros](../../issues/closures-async-packed-macros.md).

**Probes are how Locus asks rustc questions.** One kind of probe observes a place. Others resolve a name. A name-resolving probe mirrors the way logical code uses the name, with placeholder arguments, so that rustc resolves it in that shape:
- a value;
- a type;
- a function called with its number of arguments;
- a method;
- an item of a trait.

Method calls and indexing in logical code are resolved on logical types, after runtime values are taken to their models or snapshots. They never resolve to runtime methods, or to overloaded `Deref` and `Index`. How is [RFC 0002](0002-logical-layer.md).

The driver recognizes probes by the identity of their definitions in Locus's library, never by a path written in the source. The exact form of each probe is part of the lowering's specification and is tested against rustc.

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
  - **The default does not depend on the machine that runs the check.** For now, an exported crate whose checked code tests other properties of the target, such as `target_os`, must declare its configurations.
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
- **The cost is Cargo's feature unification.** A downstream crate can enable any combination of an exported library's features. If that combination changes what checked code sees and was not checked, the downstream build fails with the guard's message. Library authors therefore declare the combinations they support. The alternatives are worse:
  - checking one configuration and trusting the rest is unsound;
  - checking every combination is infeasible;
  - banning `cfg` in checked code rules out ordinary platform-specific code.
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

  This RFC proposes the last. The `cfg`'s name, `__locus_check`, is fixed by the library, which every crate shares, so user code could test it. Lowering rejects a source that does, and validation, which compares every item and body, catches any other code that does, such as a macro's output.
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
- **For every other crate,** it behaves exactly like rustc.
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
       /// Text Locus wrote: an observation, a stub, the guard.
       Synthesized { generated: TextRange, origin: Origin },
   }

   struct Origin {
       node: SyntaxNodePtr, // the Locus construct that caused the text
       role: Role,          // Observation, Resolution, Stub, Guard, ...
   }
   ```

   Every span rustc reports passes through an origin map before a user sees it.
4. **Facts.** What the driver reports, in Locus's own types rather than rustc's:

   ```rust
   struct Facts {
       version: u32,
       program: ProgramId,                       // which checking program these describe
       observations: Vec<(ProbeSpan, Ty)>,       // the type of each observed place
       resolutions: Vec<(ProbeSpan, ItemRef)>,   // the item each name resolves to
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
   - **Whether an earlier check still applies is a separate question,** answered by the manifest rather than by every name: the fingerprints of the sources and dependencies, the features, the target and configuration, the specifications relied on, and the version of Locus. A path dependency can change while its package identity stays the same, and one package can be built with different features.
   - `Ty` describes Rust types in the same terms, independently of rustc's own data structures. Where the semantic translation needs lifetimes or hygiene ([runtime-semantics](../../issues/runtime-semantics.md)), facts carry them too.
   - `TypedBody` is derived from rustc's THIR. Every dereference, borrow, coercion, and call is explicit. A call to a trait method names the method and its `Self` type; where the types are concrete, the driver also resolves the implementation with rustc's instance resolution. In generic code the call stays a trait method. The semantic translation decides whether THIR is enough or MIR is needed ([runtime-semantics](../../issues/runtime-semantics.md)).
   - Facts are serialized, as JSON at first, so that the driver and the rest of Locus can run as separate processes.
5. **Logical IR.** Locus's typed representation of logical items, propositions, and proofs. It refers to runtime types and items only through `Ty` and `ItemRef` from the facts.
6. **Kernel terms and obligations.** Defined by the kernel RFC.
7. **Interfaces.** When Locus checks a crate, it writes the crate's **interface**: its logical items, the contracts and ghost signatures of its functions, the effect of each function (what it may write, what access it may retain, and whether it runs code whose effect is unknown), its invariants, and its models, in Locus's own terms. A crate that depends on it is checked against that interface, since rustc's metadata holds only markers. The interface's hash is part of what a dependent's check relied on.
8. **Manifests.** Every successful check writes a **manifest** of what its result relies on, so that a reviewer can see what was claimed and on what grounds:
   - the hashes of the sources, and the configuration checked;
   - the interfaces of the dependencies, by hash;
   - the version of the semantic translation, and of the facts format;
   - the models and specifications from Locus's library that the proofs used;
   - the assumed specifications, the axioms, and the trust model's entries the result depends on, by ID, including `assumed.unsafe.invariants` with the types whose invariants it relies on, and `assumed.rust.aliasing`;
   - the certificate: the proofs, stored or found, that the kernel checked.

   The audit command prints it, and the vision's promise that a result can report its assumptions (commitment 4) is this list. Its format, and how stored proofs are found again, are [proof-search-and-storage](../../issues/proof-search-and-storage.md).
9. **Edits.** In export mode, the exported program is a list of `Edit { range, replacement }` applied to each `.lc` file, with an origin map like the lowered program's.

### The runtime program

The runtime program is what runs: in build mode, the lowered program compiled with the runtime expansions; in export mode, the `.rs` files that erasure writes. Either way, Locus produces it only after the check passes, and validates it against the checking program before using it.

**In build mode, it is the same text.**
- Every Locus macro expands to its marker, so logical code leaves nothing at run time: no borrow, no evaluation.
- Ghost parameters, arguments, locals, and results stay, as markers. A marker is zero-sized, `Copy`, and `'static`, with no drop glue, so it costs nothing at run time. No one reads this program; it lives under `target/` only.
- Stubs, `impl Model`, and `T: Model` bounds stay too. Runtime code never refers to a stub.
- **No new borrow errors.** The two programs differ only in what Locus's macros expand to: observations, which are shared borrows that end immediately, and probes in the checking program, and nothing in the runtime program. So they have identical types, and the runtime program borrow-checks whenever the checking program does.
- **The same tokens for every macro.** An attribute macro, a derive, or a program's own macro sees the same input in both programs, because both compile one text. A macro that behaves differently when ghost code is absent has nothing to react to.

**In export mode, erasure writes readable Rust.** It edits each `.lc` file into a `.rs` file beside it. Locus syntax and Locus's macros are removed or replaced, and every other byte is kept.
- **Readable by construction.** A `.lc` file with no Locus syntax becomes an identical `.rs` file, apart from its trailer. Comments, formatting, and names survive everywhere. Where there is Locus syntax, what remains is the Rust a programmer would have written without proofs.
- **No logical values.** The exported program contains no proofs, no logical values, and no logical items. Ghost values inside runtime types and fields are the crate's own private markers `Erased` and `LogicalData`, in its generated module, which no exported signature mentions. Erasure writes them through an import that it appends to each file that needs one, under a fresh name where those names are taken.
- **What the edits do:**
  - **Removed:** logic functions, `prop` declarations, `#[derive(Logical)]` types, the model types and `impl`s of `#[derive(Model)]`, `Model` implementations and `T: Model` bounds, and `impl` blocks holding only those; parameters and locals of logical type, proof parameters included, with every argument for them and every `logic!`, `prop!`, `prove!`, and proof hole, except the ghost parameters of a function used as a value, which stay with their markers' types, and whose arguments become markers, so that the function keeps the type it is used at ([RFC 0002](0002-logical-layer.md#export-mode)); `use` declarations and private items that only logical code uses; and code that no checked configuration enables. Which of these apply comes from the facts.
  - **Kept:** a call of a runtime function whose result is entirely ghost, where it was. As a statement it stays as it is. Where a ghost value is needed, as a logical field's initializer, a logical tuple component, or a payload, it becomes `match call { _ => Erased }`, which keeps its order and keeps its temporaries alive to the end of the enclosing statement, as in the source.
  - **Simplified:** a logical field becomes a field of its marker's type; a tuple, including a named result, keeps its shape, with markers for its logical components; a result that is entirely logical and not a tuple becomes `()`, except for a function used as a value, which keeps its marker; a ghost local that runtime code names inside a function-like macro's input stays, bound to its marker; a logical type argument becomes its marker, and so does a ghost value that Rust needs: `Vec<Nat>` becomes `Vec<LogicalData>`, `Option<@P>` becomes `Option<Erased>`, and `Some(h)` becomes `Some(Erased)` or `Some(_)`; `pub(locus)` becomes `pub(crate)`; and `cfg` attributes that every checked configuration satisfies are dropped.
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

Building the runtime program is how Locus reports lints and confirms its own output. In export mode, that is the compile of the exported program that validation needs anyway. Any other warning there is either about the user's code or a Locus bug.

**Source locations.** In build mode, `file!`, `line!`, `column!`, `Location::caller`, and panic messages name the `.lc` file, because rustc reads the lowered text at its path, and lowering keeps runtime code on its own line where it can, though a column moves where Locus syntax earlier on the line was lowered. In export mode, they name the exported file, whose lines move where erasure removed an item. That is the one stated exception to "erasure never changes what the program does".

**Contracts are visible only to code that Locus checks.** No proof exists at run time. What unchecked Rust code could do instead is call a function without meeting its precondition, rely on a postcondition, or build a value without its invariant. Neither mode lets it:
- **Within a crate,** every file is checked. A function whose parameters hold proofs never reaches code that Locus does not check, as a function value, a function pointer, a trait method, through generic code, or as an exported symbol ([RFC 0002](0002-logical-layer.md#the-boundary-with-unchecked-code)).
- **In build mode,** every crate that depends on a Locus crate is checked, so every call of its functions and every use of its types is.
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
- **Errors that involve logical code.** An error involves logical code if any span lies in synthesized text, or if its message names a stub, a probe, or anything else Locus generated.
  - Locus words it in its own terms, with the synthesized span as the primary span. For example, rustc's E0502 at a user's `x.push(2)`, caused by an observation earlier, becomes "logical code reads `x` while `x` is mutably borrowed", pointing at the logical code, with rustc's code as a note.
  - Some errors at synthesized code have known causes, such as reading a mutable static from logical code. Locus explains these.
  - Anything else that involves logical code is an internal error, never blamed on the user.
- **Errors with no span** are shown as rustc reports them, marked as not attributed.
- **Mapping.** Locus never shows rustc's rendered text. It reads rustc's structured diagnostics and maps every span through the origin maps, including macro backtraces and suggestions.
  - The two ends of a span map independently, so a span half in copied and half in synthesized text still covers the right source.
  - An end inside synthesized text maps to the edge of the construct that caused it.
  - Spans inside Locus's library map to the invocation that expanded there.
  - A suggestion whose edit lands in copied text is kept and applies to the `.lc` file. One that touches synthesized text is dropped.
- **Wording.** Names of stubs and probes in rustc's messages are shown as Locus terms, such as `Nat`.
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
| `locus-library` | Locus's macros, probes, stubs of its logical items, and the markers, as one Rust library whose macros have a checking and a runtime expansion, selected by a `cfg` | No | Yes: probes are recognized by its definitions, and its macros decide what both programs contain |
| `locus-lower` | The lowered program, generated module, and origin map | No | Yes |
| `locus-logic` | Logical items, typing, the ghost checks, elaboration, obligations | No | Yes, for its ghost checks; obligation generation as decided by [runtime-semantics](../../issues/runtime-semantics.md) |
| `locus-search` | Proof search | No | No |
| `locus-kernel` | Proof checking | No | Yes |
| `locus-erase` | Export mode's runtime program, export surface, guard, and origin maps | No | No for bodies, which validation checks; yes for the guard and visibilities |
| `locus-diagnostics` | Mapping, wording, rendering | No | No |
| `locus` | Command line, both modes, Cargo orchestration, validation, and manifests | No | Yes: it pairs facts with programs, decides which crates are checked, decides when to stop, and validates the runtime program |

### Spikes

Three spikes tested this design. Nothing depends on them, and each states what it did not test.

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

**Not yet tested:**
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

The rules below go into the specification with this RFC, in [Compatibility with Rust](../../spec/compatibility.md), [Observing runtime values](../../spec/observation.md), and [Exporting to Rust](../../spec/export.md).

| Rule | Examples |
|---|---|
| `compat.superset.same-meaning` | `compatibility/clone-before-move.accept.lc`, `compatibility/include-from-source.accept.lc`, `compatibility/plain-rust.accept.lc`, `compatibility/user-method-named-model.accept.lc`, `compatibility/reserved-check-cfg.reject.lc` |
| `compat.errors.rustc` | `compatibility/logic-block-is-a-struct-literal.reject.lc`, `compatibility/use-after-move.reject.lc` |
| `compat.syntax.not-rust` | `compatibility/at-in-patterns.accept.lc`, `compatibility/keywords-as-names.accept.lc`, `compatibility/pub-locus-as-tuple-field-type.accept.lc`, `compatibility/tokens-in-macro-input.accept.lc`, `compatibility/underscore-in-rust-positions.accept.lc`, `compatibility/hole-in-macro-input.reject.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.syntax.contextual-keywords` | `compatibility/keywords-as-names.accept.lc`, `compatibility/raw-identifier-logic.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.names.shadowable` | `compatibility/glob-imported-macro-named-prove.accept.lc`, `compatibility/own-prove-inside-vec.accept.lc`, `compatibility/user-macro-named-prove.accept.lc`, `compatibility/user-type-named-nat.accept.lc` |
| `compat.names.no-methods` | `compatibility/user-method-named-model.accept.lc` |
| `compat.names.macros-expand` | `compatibility/locus-macro-inside-rust-macro.accept.lc`, `compatibility/prove-in-macro-output.accept.lc`, `compatibility/prove-in-stringify.accept.lc`, `compatibility/prove-through-alias.accept.lc`, `observation/macro-in-vec-reads-sibling-field.accept.lc`, `compatibility/expanded-macro-checked.reject.lc` |
| `observe.place.readable` | `models/read-through-cut-reference.accept.lc`, `observation/after-mutable-borrow-ends.accept.lc`, `observation/before-move.accept.lc`, `observation/let-annotation-reads-kept-place.accept.lc`, `observation/logical-closure-in-body.accept.lc`, `observation/macro-in-vec-reads-sibling-field.accept.lc`, `observation/macro-reads-sibling-field.accept.lc`, `observation/proof-before-move.accept.lc`, `observation/after-move.reject.lc`, `observation/during-mutable-borrow.reject.lc`, `observation/let-annotation-after-initializer.reject.lc`, `observation/proof-after-move.reject.lc` |
| `observe.let.after-initializer` | `observation/let-annotation-reads-kept-place.accept.lc`, `observation/let-annotation-after-initializer.reject.lc` |
| `observe.captures.rejected` | `observation/logical-closure-in-body.accept.lc`, `observation/logic-in-async-block.reject.lc`, `observation/logic-in-closure.reject.lc` |
| `export.locus-visibility` | `export/locus-logical-field.accept.lc`, `export/wrapped-precondition.accept.lc`, `export/reexport-locus-item.reject.lc` |
| `export.macros-direct` | `export/ghost-call-outside-macro.accept.lc`, `export/ghost-local-in-macro-input.accept.lc`, `export/prove-in-test-function.accept.lc`, `export/prove-through-alias.accept.lc`, `export/ghost-call-inside-macro.reject.lc`, `export/prove-in-macro-output.reject.lc`, `export/prove-inside-rust-macro.reject.lc` |
| `export.function-values` | `export/function-with-ghost-parameter-as-value.accept.lc`, `export/function-with-ghost-result-as-value.accept.lc` |

The accepted near-misses `before-move`, `after-mutable-borrow-ends`, and `proof-before-move` read `v.len()` in logical code, which [RFC 0002](0002-logical-layer.md) defines through `Vec`'s model, and rely on its versions: restating a proof before the move needs `v` to be the same version as at entry. So they are decided together with it.

The observation examples use provisional syntax for propositions ([proposition-syntax](../../issues/proposition-syntax.md)); respelling them does not change their outcomes.

## Interactions

| Area | Effect |
|---|---|
| Types | rustc is the authority on every runtime type. Logical types reach rustc only as stubs and aliases that resolve names, and as the markers `Erased` and `LogicalData`, which carry no proposition and no value ([RFC 0002](0002-logical-layer.md)). Locus types the logical layer itself, working out which logical type each marker stands for. |
| Propositions and proofs | rustc never sees a proposition. It sees an observation for each place logical code reads, and a probe for each name it uses. |
| Ownership and mutation | rustc checks all ownership, including what logical code reads. How mutation is modeled in the logic is built from rustc's typed bodies ([runtime-semantics](../../issues/runtime-semantics.md)). |
| Erasure and runtime behavior | In build mode nothing is erased: both programs compile one lowered text, whose ghost values are markers. In export mode erasure edits the source into readable Rust. Either way, runtime code is untouched, logical code does no runtime evaluation, the runtime program is validated against the checking program, and only source locations change. |
| Configurations | Each configuration is a separate program. Build mode checks the configuration being built. An exported crate is checked in the configurations it declares, and its exported code refuses to compile in any other ([cfg-corner-cases](../../issues/cfg-corner-cases.md)). |
| Rust interoperability | In build mode, every crate that depends on a Locus crate is checked, so contracts work across crates. In export mode, `.rs` files sit beside their `.lc` sources, so the crate builds with plain Cargo, and its interface is plain Rust, with no proofs, logical values, or contracts; `pub(locus)` marks what only Locus crates see ([distribution-and-export](../../issues/distribution-and-export.md)). |
| Diagnostics | Rust's errors keep their codes and messages, with spans mapped to the source. Errors that involve logical code are explained in Locus's terms. Lints come from the runtime program. |
| Trust model | New trusted components: rustc's type checker and borrow checker, `locus-driver`, the facts format, Locus's library, the parser, the lowering, obligation generation, translation validation, export's guard and visibilities, and the orchestration. New assumptions, for exported crates: toolchains, dependencies, macro inputs, and published files. [RFC 0002](0002-logical-layer.md) adds the rest. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Remove ghost parameters before rustc runs,** so that the checking program, like the runtime program, has no proof parameters. This was the previous draft. Rejected:
- Lowering would have to know, at every call, which arguments to delete and whether a result is ghost, which needs the callee's signature, and so the receiver's type, method resolution, and trait resolution: rustc's work.
- A wrong guess can compile with another meaning. Tested with rustc 1.97: with an inherent `go(&self, a, h: @P)` and a trait method `go(self, a)` in scope, deleting `h` from `x.go(1, h)` compiled and called the trait method; and a proof-returning call lowered to return `()` instantiated `make::<()>` and compiled.
- The two programs would differ in their ghost bindings, so a macro around a function would see different tokens in each. An external review built a deterministic attribute macro that returned `0` in the checking program and `1` in the runtime program.

**Keep the markers in the checking program only,** and remove them from build mode's runtime program afterwards, using the facts. Rejected: it keeps the runtime program of build mode readable, but no one reads it, and the two programs would again differ under every attribute macro.

**A discovery run:** compile a provisional program first to learn types and resolutions, then lower precisely and compile again. Rejected for now. It costs a second compile of every checked crate, and it must be shown to converge. One lowered text needs neither. It stays the fallback if a case arises that only types can decide.

**Lowered files on disk, mirroring the crate under `target/`,** compiled from there. Rejected: a path that the source resolves from its own file, as in `#![doc = include_str!("../README.md")]`, would resolve from the copy instead, and `file!` would name it.

**Reject attribute macros around code that erasure edits,** in export mode, instead of validating. Rejected: it forbids common macros such as `#[tracing::instrument]` on any function with a proof, even when they behave the same either way. Validation accepts them when the bodies agree, and fails loudly when they do not.

**Encode everything in Rust macros and attributes,** with no Locus syntax: proof types as `proof![x > 0]`, logic functions as `#[logic] fn`. Creusot and Prusti work this way.
- *For:* Locus would need no parser of its own. rust-analyzer, rustfmt, and editors would work on Locus code unchanged.
- *Against:* proofs as values live in signatures, which are what people review, and macro encodings make signatures noisier. Errors inside macro invocations are also reported less precisely.

This RFC instead uses syntax for proofs, logic functions, and propositions, and names only for blocks, derives, and traits.

**A `locus! { ... }` macro inside ordinary `.rs` files,** erasing at build time. It needs no generated files. But a procedural macro sees no types and can only erase, never check, so a crate could be built and published with proofs that were never checked. Exported `.rs` files exist only when the check passed.

**One mode, whose output always serves Rust callers.** Generated files would sit beside their sources in every build, with `OUT_DIR` as an opt-in. To protect contracts from unchecked callers, proof parameters would take tokens that only checked code supplies, other crates would obtain tokens through an `unsafe` constructor, and a function whose precondition sits in its parameters' data would be `unsafe`.
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

**Charon** exports rustc's MIR in a stable format, and is a candidate source of typed bodies for [runtime-semantics](../../issues/runtime-semantics.md). Run on the checking program, it could also report what the probes ask. It pins its own nightly, and adopting it would make its format part of the trusted path.

**Distinct stand-ins for each logical type in the checking program, and one marker in the runtime program.** Rejected: macros would see different types in the two programs, and types that are distinct in one would collide in the other. Both programs use the same two markers, and no `impl` mentions a logical type ([RFC 0002](0002-logical-layer.md)).

**Zero-sized stand-ins for proofs that Locus believes by their type,** in both programs. Rejected: Rust code that can tell types apart, through `Any` or specialization, could turn one proof into another. [RFC 0002](0002-logical-layer.md) makes proofs ghost instead: `Erased` is still one type for every proof, but Locus never believes it.

**One Rust type per proposition,** so that rustc could tell proofs apart. Not possible: propositions mention runtime values, so `@(y != 0)` is a different proposition for each `y`.

**Remove logical and proof fields from types.** Rejected at the owner's direction. It needs renumbered tuple fields, extra `PhantomData` fields, and observations of the whole value where logical code reads one field, and Locus's macros expanded inside other macros could not name the field at all. Kept as a marker, a field costs derives that its marker does not implement, such as serde's, and shows up in `Debug` output.

**Default to the host's configuration.** Rejected: the same crate would check differently on different machines.

**Give rustc the logical code too,** as real Rust over stand-in types with operator implementations, close to what Verus does with its proof code. Rejected: logical operators would type-check with Rust's meaning, and logical types such as recursive enums without `Box` do not fit Rust's types.

**Write a new Rust parser** instead of forking rust-analyzer's. It would give Locus full control and cleaner code, at the cost of reaching all of Rust's syntax before anything else works. The parser tests apply either way.

## Prior art

- **Clippy** is a rustc driver run through `RUSTC_WORKSPACE_WRAPPER`, the model for `locus-driver`.
- **Verus** checks the ownership of its proof code by generating a separate Rust program for rustc's borrow checker, a close cousin of the checking program. It reads rustc's internals directly throughout, where Locus confines that to one crate.
- **Creusot, Prusti, Kani, and Flux** are rustc drivers pinned to a nightly. Creusot and Prusti erase their contracts in ordinary builds.
- **Charon**, used by Aeneas, exports what rustc knows in a format of its own, separating the part that depends on rustc from the analysis. `locus-facts` follows the same separation.
- **rust-analyzer** provides the lossless, error-tolerant parser and the rowan trees this design starts from.
- **Code generators that commit their output,** such as LALRPOP's and some users of prost, show that generated Rust beside its source is workable when a check catches stale output.
- **TypeScript-based tools for Svelte and Vue** check generated code and map diagnostics back through source maps, as the origin maps do here.

## Unresolved questions

**Decided by the owner while this RFC was drafted.** Recorded here; accepting the RFC confirms them.
1. Locus syntax only for proofs, logic functions, and propositions. Names for blocks, derives, and traits, resolved like std's prelude.
2. `#[derive(Logical)]`, never implemented by hand, and a `Model` trait, in place of `logic struct` and `logic enum`.
3. The four ways facts enter the logic. `Cell`, `RefCell`, `Mutex`, and the atomics have no model, and their snapshots hold nothing of their contents.
4. The current stable release with `RUSTC_BOOTSTRAP=1` preferred; a pinned nightly acceptable.
5. Two modes. Build mode, the default, compiles the runtime program under `target/`, in one Cargo run, and checks every crate that depends on a Locus crate. Export mode writes `.rs` files beside the sources for Rust users. `pub(locus)` marks what only Locus crates see: public in build mode, and `pub(crate)` in export mode.
6. In build mode, the configuration being built is checked. In export mode, declared configurations with a guard; the default is dev, release, and test builds with default features and 64-bit pointers, independent of the host. Erasure removes code no checked configuration enables.
7. An exported interface has no logical values and states no contracts, postconditions included. There are no proof tokens and no `unsafe` exports.
8. Proofs and logical values are ghost ([RFC 0002](0002-logical-layer.md)): zero-sized markers, `Erased` for proofs and `LogicalData` for other logical data.
9. Logical and proof fields stay in both programs, as fields of their marker's type.
10. Traits among Locus's names have no methods.
11. The parser starts from a fork of rust-analyzer's. The owner left the choice to this RFC.
12. Locus's macros are real macros, which rustc expands wherever it expands macros. Export mode accepts only invocations written directly in the crate's own code.
13. An exported crate uses its Locus dependencies through their exported interfaces, so contracts between crates work only in build mode.
14. Build mode checks Locus dependencies from their sources, so a Locus crate published for Locus users includes its `.lc` files.
15. **One lowered text for build mode.** The checking and runtime programs of build mode are one lowered text, compiled twice with different expansions of Locus's macros, with every ghost position kept as a marker. Lowering decides only what the source shows, and confirms every assumption against the facts. This revisits decision 8 of the previous draft, that proof parameters are removed, for build mode only, whose runtime program no one reads; an exported program still removes them. The [spikes](#spikes) support it.
16. **Validation.** In both modes, Locus compares the runtime program's items and typed bodies with the checking program's, for every item and body, and fails on any difference. Export mode keeps erasure textual, so its output stays readable.
17. **Interfaces and manifests.** Each checked crate gets an interface that its dependents are checked against, and each result a manifest of what it relies on.
18. **Acceptance waits for the spikes** of build mode's lowering and of the Cargo graph. A semantic slice, verifying one mutable function with a reborrow and a destructor end to end, goes with the RFC for the runtime semantics ([runtime-semantics](../../issues/runtime-semantics.md)).
19. **One library, with its macros' expansions selected by a `cfg`** that only the checking compile sets, rather than two versions of the library swapped per program. The Cargo spike found it the simplest way that works across crates; two libraries need every dependency's checking metadata kept and passed separately.
20. **Ghost bindings recorded both ways** for expansions that rustc performs: a table of lexical scopes, consulted by position, and a local `macro_rules!` dispatch, resolved by hygiene. Each alone failed a case in the spike, and both together passed every one.
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
35. **A dependency is named by Cargo's package identity,** its name, version, and source, with its crate target, and a Locus dependency also by its interface hash. Whether an earlier check still applies, given features, configuration, sources, specifications, and Locus's version, is the manifest's question, not part of every name.
36. **Lowering rewrites the `#[path]`s the source writes** beneath a module it points, as well as adding the missing ones.
37. **Validation compares foreign linkage:** `#[link_name]` on a foreign item and `#[link(...)]` on its `extern` block, with their options.

**Still for the owner.** Each is written into the spec and the examples as proposed where an example can show it, so accepting this RFC accepts it. Item 1 has no rule yet, because an example would need a `.rs` file beside a `.lc` one, which the litmus format cannot yet express:
1. `.rs` files may not name logical items, as proposed.
2. Logical code in closures and async blocks, and reads of packed fields, are rejected for now, as proposed.
3. Traits among Locus's names also have no associated functions or constants (an extension of 10 above).
4. Until [cfg-corner-cases](../../issues/cfg-corner-cases.md) decides more, an exported crate whose checked code tests target properties other than pointer width must declare its configurations.
5. **In the first stage, other crates are trusted,** as `assumed.stage1.dependents`, so that the crate is checked with build mode's semantics and every litmus example applies. The alternative is export mode's interface rules for everything other crates can reach: sound against plain-Rust callers from the start, but a crate could then make no public item with logical values in its signature until build mode exists. Both keep ghost parameters as markers.
6. **Which marker a `logic!` gives** when its code is a proof, such as `logic! { add_zero(n) }` for a lemma `add_zero`. Lowering decides only what the source shows, and the source does not show it. The options: the source shows it, by writing logical code that gives a proof as `prove!` and keeping `logic!` for data; or the marker is left to rustc's inference, as a value of a type that either marker can fill, and Locus confirms it against the logical type of the code. The first keeps lowering's rule; the second writes nothing new, and is untested.

**Left to other RFCs**, each with its issue:
- Logical types, propositions, proofs, models, and contracts: [RFC 0002](0002-logical-layer.md), proposed with this one.
- The remaining syntax of propositions and proofs: [proposition-syntax](../../issues/proposition-syntax.md).
- The runtime semantics: how typed bodies become statements in the logic, THIR or MIR, where borrows end, integer overflow across profiles, and whether observations of a value with proof fields can be narrowed to the fields its propositions read: [runtime-semantics](../../issues/runtime-semantics.md).
- Distribution: package contents, including how an exported package's manifest names its exported crate root; how configurations are declared; dependency versions; how Locus recognizes Locus dependencies; and keeping rustfmt off exported files: [distribution-and-export](../../issues/distribution-and-export.md).
- Logical code in closures and async blocks, Locus syntax inside other macros' input, and reads of packed fields: [closures-async-packed-macros](../../issues/closures-async-packed-macros.md).
- Corner cases of `cfg` and compiler flags, and doctests in build mode: [cfg-corner-cases](../../issues/cfg-corner-cases.md).
- Editor support, likely a language server that runs rust-analyzer on the checking program and maps results back through origin maps.
- Incremental checking and caching of facts.
