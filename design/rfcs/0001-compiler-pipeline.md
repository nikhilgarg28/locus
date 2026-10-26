# RFC 0001: The compiler pipeline

- **Status:** proposed
- **Issue:** none
- **Supersedes:** none

## Summary

Locus compiles a Cargo crate in four steps:

1. **Parse.** Locus parses its `.lc` sources, Rust plus Locus syntax, into lossless syntax trees.
2. **Lower.** Locus writes each `.lc` file as one Rust text, the **lowered program**. In it, proofs and logical values are zero-sized markers, `Erased` and `ErasedData`, in every position they occupy; logical items are stubs that only resolve names; and logical code is an invocation of a macro from Locus's library. Lowering decides only what the source shows, never an expression's type.
3. **Check.** rustc compiles the lowered program against the *checking* version of Locus's library, through a small driver. That is the **checking program**: its macros turn every place logical code reads into a borrow, and the driver reports what Locus needs: the types of those places, what names resolve to, and the typed bodies of the functions being verified. rustc's borrow checker decides whether logical code may read what it reads. Locus then checks the logical layer against those facts, constructs proofs, and has the kernel check them.
4. **Build or export.** Only when the check passes, Locus produces the **runtime program**, and validates that it has the typed bodies of the program it checked, apart from logical code:
   - **Build mode,** the default: rustc compiles the same lowered program against the *runtime* version of Locus's library, whose macros expand to the markers and nothing else. `cargo locus build` builds it under `target/`, in the configuration being built. Every crate in the build that depends on a Locus crate is checked too, so contracts hold across crates.
   - **Export mode:** `cargo locus export` erases the logical layer by editing each `.lc` file into readable Rust, written beside the sources as `.rs` files that Rust users build with plain Cargo. The crate's public interface must then be plain Rust: it has no logical values and states no contracts.

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
| The meaning of runtime code in the logic | Locus, from rustc's typed bodies; the semantics are [#6](https://github.com/nikhilgarg28/locus/issues/6) |
| Proof search | Locus, untrusted |
| Proof checking | Locus's kernel, trusted |
| Erasure to readable Rust, in export mode | Locus |
| Validating that the program built or exported is the one checked | Locus, from rustc's typed bodies of both |
| Monomorphization, code generation, linking | rustc |
| Diagnostics: rendering, and mapping rustc's back to the source | Locus |

### Sources and modes

Locus compiles one Cargo crate at a time, in the crate's real Cargo context: its edition, features, `cfg`s, and dependencies.

- **`.rs` files are plain Rust.** They are compiled as they are, and they may not name logical items: no logic functions, logical types, or proofs. The driver reports what every path resolves to, and a `.rs` path that resolves to a logical item is an error.
- **`.lc` files may also contain Locus syntax and use Locus names.** Renaming a `.rs` file to `.lc` changes nothing about the program.
- **Modules work as in Rust.** `mod name;` finds `name.lc` or `name.rs` by Rust's usual rules. When both exist, `name.rs` is the file exported from `name.lc` (below), and Locus reads `name.lc`. A crate whose root is a `.lc` file names it in its manifest, as `[lib] path = "src/lib.lc"`. Cargo accepts that (tested), and needs it, since it finds no target when `src/lib.rs` is missing.
- **Each crate has one generated module,** under a fresh name, declared by the crate root. In the lowered program it holds the prelude that makes Locus's names visible (see [Locus syntax and Locus names](#locus-syntax-and-locus-names)). In export mode, it holds the crate's own markers `Erased` and `ErasedData`, in the checking program and in the exported one, and the exported program's copy holds the configuration guard too (see [Configurations](#configurations)).
- **The lowered program is never committed.** It lives under `target/locus/lowered/` and mirrors the crate: each `name.lc` lowered, each `.rs` copied.

**Build mode** is the default. It is for code whose builders have Locus: applications, internal workspaces, and Locus crates that other Locus crates use.
- **`cargo locus build` builds the runtime program,** and `cargo locus test` and `cargo locus run` work the same way. The runtime program lives under `target/locus/run/`, and nothing is written beside the sources. `cargo locus check` checks without building.
- **Every crate that depends on a Locus crate, directly or through other crates, is checked,** whatever its files are: the other crates of the workspace, integration tests, and examples. So every call of a function with a precondition is checked, and a crate's whole interface, contracts included, is available to the crates that use it. Doctests, which rustdoc compiles outside Cargo's wrapper, are [#9](https://github.com/nikhilgarg28/locus/issues/9).
- **Locus dependencies are checked from their sources,** so a Locus crate published for Locus users includes its `.lc` files.
- **Exactly the configuration being built is checked** (see [Configurations](#configurations)).

**Export mode** is for crates that Rust users depend on.
- **`cargo locus export` writes each `name.lc`'s runtime program beside it, as `name.rs`.**
  - Its last lines say that it is generated, name its source, and record the source's hash. They come last so that they do not move the lines above them.
  - So plain `cargo build`, rustc, rust-analyzer, docs.rs, and anyone depending on the crate see an ordinary Rust crate. They need neither Locus nor a build script.
- **Exported files are committed and published,** like the output of other code generators.
  - `cargo locus export --check` fails when an exported file does not match what its source would produce, whether it is stale or was edited by hand.
  - rustfmt formats every module file, so `cargo fmt` can make an exported file stale. rustfmt leaves a module alone when its `mod` declaration is marked `#[rustfmt::skip]` (tested), and erasure adds that mark where the declaration is in a `.lc` file. Protecting the rest, an exported crate root or a declaration in plain Rust, is [#7](https://github.com/nikhilgarg28/locus/issues/7).
  - How an exported package's manifest names its exported crate root, and what else the package contains, is [#7](https://github.com/nikhilgarg28/locus/issues/7).
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
- **They are visible the way std's prelude is.** In the checking program, every module sees a prelude that combines std's with Locus's, so rustc resolves Locus names with Rust's own rules.
  - The prelude is in the checking program's copy of the crate's generated module, and re-exports Locus's names from its library.
  - Declaring it needs rustc's internal `prelude_import` feature, which the driver enables for the checking program only.
- **A program's own definitions and imports take precedence,** exactly as they do over std's prelude: its own `macro_rules! prove`, `use other::prove`, `struct Nat`, or a glob import of a type named `Nat`. All of these were tested.
- **Glob-imported macros are the one exception in Rust's rules.** They collide with prelude macros (E0659), and that is already true of std's `vec!`. So does a macro defined by another macro's expansion. When rustc reports such an ambiguity involving a Locus name, Locus checks the crate again without that name in its prelude. The program's own macro keeps its Rust meaning, and that crate cannot use Locus's macro of that name.
- **Recognizing Locus's names.** Lowering assumes that each of these is Locus's:
  - an invocation of a Locus macro name, or of a name that the crate's imports make an alias of one, such as `check!` after `use prove as check;`;
  - a `derive(Logical)`;
  - an `impl Model`;
  - a type path that resolves, by Locus's own reading of the crate's items, imports, and aliases, to a logical type, which decides what lowering removes.

  The driver then reports which definition each one actually resolved to, by the definition's identity rather than its path. If one turns out to be the program's own, Locus discards everything from that run, errors included, and lowers again, treating it as plain Rust. A crate normally needs one rustc run, and needs a second only when names collide.
- **Locus's macros are real macros.** Lowering lowers every invocation of a Locus macro that it recognizes, knowing what is logical, as it does all logical code. The rest reach rustc, which expands them like any other macro: inside another macro's input, such as `vec![prove!(...)]`, and in a macro's output.
  - Locus's library defines them as procedural macros that contain Locus's parser for logical code.
  - In the checking program, such an expansion observes each place that its logical code reads, with the invocation's own hygiene, and probes each name it uses. It also carries its logical code, which the driver reports with its spans, so Locus checks that code like any other.
  - Every field it can name exists in the checking program, logical fields included, because they stay as fields of type `Erased` (see [The checking program](#the-checking-program)). Lowering keeps each ghost local and parameter bound, as `Erased`, in the checking program, so that names in such expansions resolve too.
  - In build mode's runtime program, Locus's library expands each of them to `Erased`. Code that erasure cannot remove as text, such as a program's own macro whose output invokes `prove!`, is kept as written, with `Erased` for its written types.
  - In export mode, the exported program cannot depend on Locus's library, so erasure removes every invocation as text. It can do that only for one written directly in the crate's own code, so export mode rejects invocations inside another macro's input or output ([RFC 0002](0002-logical-layer.md#export-mode)).
  - A Locus macro name that is never expanded, such as inside `stringify!`, is only tokens.

### How facts enter the logic

There is no wall between Rust and Locus inside a crate. Plain Rust and Locus code mix freely, and every file goes through the pipeline. Soundness comes instead from the only four ways facts about runtime values can enter the logic:

1. **Snapshots and models:** observing a runtime value as what it is, its snapshot, or through its type's model, what it means.
2. **Typed bodies Locus understands:** rustc's typed bodies, translated into the logic, together with what Rust's ownership rules guarantee: a call changes only what it is given mutable access to, and a mutable borrow's final value is what its place holds when the borrow ends.
3. **Assumed specifications:** each declared with a reason, and listed by the audit.
4. **Invariants of the program's own types,** stated by their proof fields, as described below.

Each is sound no matter who wrote the code.

- **Snapshots and models read only data that cannot change while the value is shared.** Rust's aliasing rules guarantee this, except for the contents of an `UnsafeCell` and the targets of raw pointers, and a snapshot holds neither.
  - A snapshot is derived from a type's definition, field by field. A model is a logic function of the snapshot.
  - Types from the standard library with `unsafe` internals (`Vec`, `Rc`, `String`) get snapshots and models declared by Locus's library, each as a trusted entry with its reason.
  - `Cell`, `RefCell`, `Mutex`, and the atomics have no model, and their snapshots hold nothing of their contents.

  So **a call Locus knows nothing about, taking `&x`, never changes `x`'s snapshot or model**: Rust freezes what they hold, and they never hold what Rust does not freeze.
- **Plain Rust in a Locus crate is still checked by Locus.** For example, plain Rust that assigns a field that a proof field reads must keep that proof field true. Locus proves it for the new value as if a hole were written there, and rejects the assignment when it cannot.
- **Code Locus cannot translate is opaque:** dependencies, trait objects, inline assembly, and anything else outside what Locus models. It runs, but its results are known only by their types, and anything it receives through `&mut` may come back as any value of its type.
- **Locus never believes what it did not check.** Proofs and logical values are ghost ([RFC 0002](0002-logical-layer.md)): both programs hold them only as zero-sized markers, `Erased` and `ErasedData`, which carry no proposition and no value. Locus tracks what a value holds itself, only through code it checks and standard library code whose specification it has. Passed to any other code, ghost contents are forgotten. So no Rust code, through `Any`, specialization, or anything else, can convert or forge a proof, and Locus never relies on Rust's generics being parametric.
- **Invariants hold for every value.** Every value of a type with proof fields satisfies them. Only checked code constructs the type or writes the fields its proofs read. In build mode, every crate that can name the type is checked. In export mode, other crates can neither construct it nor reach those fields. So a value that comes back from opaque code still satisfies its invariant: opaque code can only move it, copy it, drop it, or call the type's own checked functions.
- **Statics.** Logical code may read only immutable statics.

The details of models are [RFC 0002](0002-logical-layer.md).

### The checking program

The checking program is what rustc sees.

- **It is written as ordinary `.rs` files** under `target/locus/check/`, and rustc parses them like any crate. Files on disk can be opened, compared, and compiled by hand when something goes wrong. If disk access ever matters, the driver can serve the same text from memory through rustc's file-loader hook, without changing anything else.
- **Lowering keeps each source line on the same line of the checking program where it can,** and appends synthesized items at the end. Raw rustc output is then readable when debugging Locus itself. The origin maps, not the alignment, are the authority.
- **Locus's library crate holds its macros, its probes, the stubs of its logical items,** such as `Nat` and `Model`, and the marker `Erased` that both programs use in build mode. rustc receives it under a crate name the program does not use.
- **New names are fresh.** Every name Locus introduces, such as the library's crate name and the generated module, is chosen so that it appears nowhere in the crate's sources, so generated items cannot collide with the user's, even under `#![no_implicit_prelude]`.
- **The checking program is compiled with `--cap-lints allow`.** Lints there would be artifacts of the lowering, and a user's `#![deny(warnings)]` or `forbid` must not turn synthesized code into errors. This was tested. Real lints come from building the runtime program.

Logical code is never given to rustc: its operators mean different things from Rust's (`+` on `Nat` cannot overflow), and its types do not fit Rust's (a logical `List` can contain itself without a `Box`). What rustc sees instead:

| In the source | In the checking program |
|---|---|
| Runtime code | Copied unchanged |
| A parameter or local whose type is logical, proof parameters included, and every argument for one | Removed; see below |
| A field whose type is logical, proof fields included | A field of type `Erased`; see below |
| A result whose type is logical | `()` |
| A logical type argument of a runtime type, such as `Vec<Nat>` or `Option<@P>`, a logical tuple component, and a ghost value where Rust needs one | `Erased`: `Vec<Erased>`, `Option<Erased>`, `(u32, Erased)` |
| A named result `(out: T, @P)` | `(T, Erased)`: tuples keep their shape |
| A call of a runtime function whose result is entirely ghost | The call where it was: as a statement, or `match call { () => Erased }` where a ghost value is needed |
| A logic function, `prop` declaration, or `#[derive(Logical)]` type | A stub: an empty type, or a function with the same name, generics, and signature, whose body panics if it is ever run |
| An `impl Model` | A real implementation of the library's `Model` trait, with its logic function as a stub beside it, so that rustc checks `T: Model` bounds and coherence |
| A proof hole, `_` | Removed with the proof it stands for |
| A `logic!`, `prop!`, or `prove!` invocation that lowering recognizes, a proposition in a type annotation inside a body, or an argument of logical type | A block that observes every place the logical code reads, where the logical code reads it, and resolves every name it uses |
| Any other expansion of a Locus macro | Expanded by rustc: an observation of each place it reads, and a probe for each name it uses |
| A proposition in a signature or a struct field's type | Nothing; see below |

**Stubs exist only to resolve names.**
- Runtime code never refers to a stub, so no stub is ever a runtime value. Only probes and other stubs mention them.
- A stub type is an empty, zero-sized type whose generic parameters are carried in a `PhantomData` field.
- Proofs in stub signatures are `Erased`.
- Stub functions are `const fn` where Rust allows, so that probes work in constant contexts. Probes are type-checked but never evaluated, and stub bodies panic rather than loop, so that a mistake fails rather than hangs constant evaluation.

**Proofs and logical values never reach rustc as values.** Ghost values are removed from the checking program, or become `Erased`, as they do in the runtime program ([RFC 0002](0002-logical-layer.md)). So rustc never computes a type that involves a proposition, and Locus alone checks every proof against the proposition its position requires, including the proof for every proof parameter at every call.

**Logical and proof fields stay, as fields of type `Erased`,** in structs and enum variants, in both programs, just as a logical type argument becomes `Erased`. So fields keep their positions, generic parameters stay used, and logical code that reads or writes such a field is checked as a borrow of that field alone.
- Constructing the type supplies `Erased` for the field, and a pattern that binds it binds an `Erased`.
- Reading the field is logical code, and is observed as a shared borrow of the field.
- An assignment to a logical data field is a ghost statement. Its right-hand side is logical code, and the checking program writes `Erased` there, so rustc checks the write as a write.
- A derive sees the field as an `Erased`. `Erased` implements `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash`, so those derives work. Others, such as `Default` or serde's, fail on the field unless told to skip it.
- rustc's warning that such a field is never read is not shown: logical code reads it.

**Logical code contains no runtime evaluation.** It reads places, and it never calls runtime functions, mutates, or has effects; the arguments of a logical call are logical code too. A runtime function that returns a proof is runtime code: it is called, and kept, in both programs. So erasing a logical construct never needs to preserve anything it did.

**Logical bindings carry no logical value in either program.** A `let` that binds a logical value, such as a `logic!` block's, binds `Erased` in the checking program, after its observations, so that later expansions of Locus's macros can name it. The runtime program removes it where it can. A call whose result is entirely ghost stays, as a statement. So rustc never has to infer a type for a logical value.

**Observations are how rustc checks what logical code reads.** A place that logical code reads is checked as a shared borrow at the point where the logical code reads it, ending immediately. So rustc's borrow checker rejects logical code that reads a value after it has moved, a place while it is mutably borrowed, or through a reference that is no longer live. Observations follow Rust's rules exactly:
- two-phase borrows apply as they would to any shared read;
- as in Rust, code after a `return` is not borrow-checked, and logical code there is never evaluated.

Observations come only from logical code inside bodies:
- **Propositions in signatures and struct field types need none.**
  - A precondition reads parameters at entry, which are always readable.
  - A postcondition reads parameters at entry, the function's results, and the final values `final!(x)` of its `&mut` parameters ([RFC 0002](0002-logical-layer.md)). These are well defined however the body exits, so it does not read anything at a point in the body.
  - A field's proposition reads the struct's own fields.
  - Their types come from item signatures in the facts.
- **A proposition in a `let` type annotation is read after the initializer,** where the `let`'s bindings come into scope, and is observed there.
- **Logical code inside a closure or an async block is rejected for now:** an observation there would become a capture, and change what the closure or future captures.
- **Logical code that reads a packed field is rejected for now:** Rust does not allow a shared borrow of one.

Both cases are [#8](https://github.com/nikhilgarg28/locus/issues/8).

**Probes are how Locus asks rustc questions.** One kind of probe observes a place. Others resolve a name. A name-resolving probe mirrors the way logical code uses the name, with placeholder arguments, so that rustc resolves it in that shape:
- a value;
- a type;
- a function called with its number of arguments;
- a method;
- an item of a trait.

Method calls and indexing in logical code are resolved on logical types, after runtime values are taken to their models. They never resolve to runtime methods, or to overloaded `Deref` and `Index`. How is [RFC 0002](0002-logical-layer.md).

The driver recognizes probes by the identity of their definitions in Locus's library, never by a path written in the source. The exact form of each probe is part of the lowering's specification and is tested against rustc. The spike tested only the simplest form.

The checking program is never shipped.

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
- **In export mode, Locus checks the configurations the crate declares,** because Rust users build the exported files in configurations Locus never sees. How a crate declares them is [#7](https://github.com/nikhilgarg28/locus/issues/7). `cargo locus export` checks each one, and passes only if all of them pass.
  - **The default** is the dev, release, and test builds, with default features, for a target with 64-bit pointers. These are what `cargo build`, `cargo build --release`, and `cargo test` build.
  - **The default does not depend on the machine that runs the check.** For now, an exported crate whose checked code tests other properties of the target, such as `target_os`, must declare its configurations.
- **Code that no checked configuration enables is removed,** in either mode, since it was never checked.
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

  - A `cfg` test can only examine the names it mentions. So the guard tests what can change checked code: every `cfg` name that appears in checked code, which is the `.lc` files and any plain Rust whose typed body a proof relies on. It also always tests `target_pointer_width`, which fixes the range of `usize`, and `debug_assertions`, which std's own macros test.
  - It takes these names from the sources, not from the output, which no longer mentions code that erasure removed.
  - A `cfg` that only plain, unproven Rust tests does not constrain the guard. So the rest of a mostly-Rust crate keeps all its configurations.
- **The cost is Cargo's feature unification.** A downstream crate can enable any combination of an exported library's features. If that combination changes what checked code sees and was not checked, the downstream build fails with the guard's message. Library authors therefore declare the combinations they support. The alternatives are worse:
  - checking one configuration and trusting the rest is unsound;
  - checking every combination is infeasible;
  - banning `cfg` in checked code rules out ordinary platform-specific code.
- **Behavior that differs between builds without a `cfg`** is the runtime semantics' to handle. Integer overflow, for example, depends on the `overflow-checks` setting, which no stable `cfg` can test, so a proof must hold either way ([#6](https://github.com/nikhilgarg28/locus/issues/6)).
- **What the guard cannot test** becomes an assumption about exported crates in [the trust model](../TRUST.md):
  - the compiler that builds the exported program;
  - the versions and features of dependencies.

  The distribution RFC ([#7](https://github.com/nikhilgarg28/locus/issues/7)) will narrow these. The inputs to macros are an assumption in both modes, because the checking and runtime programs expand macros separately.
- **Corner cases.** [#9](https://github.com/nikhilgarg28/locus/issues/9) collects the cases this design does not yet settle, among them `cargo test --release`, docs.rs's `docsrs`, target features, build-script `cfg`s, and `cfg`s written by other crates' macros.

### Running rustc: `locus-driver`

`locus-driver` is a small program linked against rustc's internal crates, the way Clippy is.

- **How Cargo runs it.** Cargo runs it in place of rustc, through `RUSTC_WORKSPACE_WRAPPER` for workspace members, or through `RUSTC_WRAPPER` when dependencies must be checked too, as Locus dependencies are in build mode. Either way, Cargo does the orchestration, as for any build, and Locus gets the exact compiler invocation, with its edition, `cfg`s, features, and `--extern` paths.
- **For a crate Locus checks,** the driver substitutes the checking program's root for the crate's root, and records facts after type checking. It then lets rustc continue so that the borrow checker runs, and stops before code generation.
- **In build mode, one Cargo run checks and builds every crate.** For a checked crate, the driver compiles the checking program as above, hands the facts to Locus, which runs as a separate process, and waits for the logical pass and erasure. It then compiles the runtime program in the same invocation, with code generation, as the artifact Cargo asked for. Cargo builds a crate's dependencies first, so their checked interfaces are ready. A crate's checking program is compiled against its dependencies' checking programs, and its runtime program against their runtime programs.
- **For every other crate,** it behaves exactly like rustc.
- **It fails closed.** If it was asked to check a crate and cannot produce facts for it, the check fails. The spike's silent fallback to plain compilation is not acceptable.
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
   }
   ```

   - **Items are identified by their definitions, not their paths.** An `ItemRef` names a definition by its crate's stable identity and its definition's path hash, both of which rustc computes, together with the generic arguments it is instantiated with. Impls, closures, and other anonymous items get identities the same way, and an associated item is named through its trait and `Self` type. A path is display text only: two paths can name one item, and one path can name different items in different crates.
   - `Ty` describes Rust types in the same terms, independently of rustc's own data structures. Where the semantic translation needs lifetimes or hygiene ([#6](https://github.com/nikhilgarg28/locus/issues/6)), facts carry them too.
   - `TypedBody` is derived from rustc's THIR. Every dereference, borrow, coercion, and call is explicit. A call to a trait method names the method and its `Self` type; where the types are concrete, the driver also resolves the implementation with rustc's instance resolution. In generic code the call stays a trait method. The semantic translation decides whether THIR is enough or MIR is needed ([#6](https://github.com/nikhilgarg28/locus/issues/6)).
   - Facts are serialized, as JSON at first, so that the driver and the rest of Locus can run as separate processes.
5. **Logical IR.** Locus's typed representation of logical items, propositions, and proofs. It refers to runtime types and items only through `Ty` and `ItemRef` from the facts.
6. **Kernel terms and obligations.** Defined by the kernel RFC.
7. **Interfaces.** When Locus checks a crate, it writes the crate's **interface**: its logical items, the contracts and ghost signatures of its functions, its invariants, and its models, in Locus's own terms. A crate that depends on it is checked against that interface, since rustc's metadata holds only markers. The interface's hash is part of what a dependent's check relied on.
8. **Manifests.** Every successful check writes a **manifest** of what its result relies on, so that a reviewer can see what was claimed and on what grounds:
   - the hashes of the sources, and the configuration checked;
   - the interfaces of the dependencies, by hash;
   - the version of the semantic translation, and of the facts format;
   - the models and specifications from Locus's library that the proofs used;
   - the assumed specifications, the axioms, and the trust model's entries the result depends on, by ID, including `assumed.unsafe.invariants` when it relies on an invariant that `unsafe` code can reach;
   - the certificate: the proofs, stored or found, that the kernel checked.

   The audit command prints it, and the vision's promise that a result can report its assumptions (commitment 4) is this list. Its format, and how stored proofs are found again, are [#13](https://github.com/nikhilgarg28/locus/issues/13).
9. **Edits.** In export mode, the exported program is a list of `Edit { range, replacement }` applied to each `.lc` file, with an origin map like the lowered program's.

### The runtime program

The runtime program is what runs: in build mode, the lowered program compiled against the runtime version of Locus's library; in export mode, the `.rs` files that erasure writes. Either way, Locus produces it only after the check passes, and validates it against the checking program before using it.

**In build mode, it is the same text.**
- Every Locus macro expands to its marker, so logical code leaves nothing at run time: no borrow, no evaluation.
- Ghost parameters, arguments, locals, and results stay, as markers. A marker is zero-sized, `Copy`, and `'static`, with no drop glue, so it costs nothing at run time. No one reads this program; it lives under `target/` only.
- Stubs, `impl Model`, and `T: Model` bounds stay too. Runtime code never refers to a stub.
- **No new borrow errors.** The two programs differ only in what Locus's macros expand to: observations, which are shared borrows that end immediately, and probes in the checking program, and nothing in the runtime program. So they have identical types, and the runtime program borrow-checks whenever the checking program does.
- **The same tokens for every macro.** An attribute macro, a derive, or a program's own macro sees the same input in both programs, because both compile one text. A macro that behaves differently when ghost code is absent has nothing to react to.

**In export mode, erasure writes readable Rust.** It edits each `.lc` file into a `.rs` file beside it. Locus syntax and Locus's macros are removed or replaced, and every other byte is kept.
- **Readable by construction.** A `.lc` file with no Locus syntax becomes an identical `.rs` file, apart from its trailer. Comments, formatting, and names survive everywhere. Where there is Locus syntax, what remains is the Rust a programmer would have written without proofs.
- **No logical values.** The exported program contains no proofs, no logical values, and no logical items. Ghost values inside runtime types and fields are the crate's own private markers `Erased` and `ErasedData`, in its generated module, which no exported signature mentions. Erasure writes them through an import that it appends to each file that needs one, under a fresh name where those names are taken.
- **What the edits do:**
  - **Removed:** logic functions, `prop` declarations, `#[derive(Logical)]` types, the model types and `impl`s of `#[derive(Model)]`, `Model` implementations and `T: Model` bounds, and `impl` blocks holding only those; parameters and locals of logical type, proof parameters included, with every argument for them and every `logic!`, `prop!`, `prove!`, and proof hole; `use` declarations and private items that only logical code uses; and code that no checked configuration enables. Which of these apply comes from the facts.
  - **Kept:** a call of a runtime function whose result is entirely ghost, where it was. As a statement it stays as it is. Where a ghost value is needed, as a logical field's initializer, a logical tuple component, or a payload, it becomes `match call { _ => Erased }`, which keeps its order and keeps its temporaries alive to the end of the enclosing statement, as in the source.
  - **Simplified:** a logical field becomes a field of its marker's type; a tuple, including a named result, keeps its shape, with markers for its logical components; a logical type argument becomes its marker, and so does a ghost value that Rust needs: `Vec<Nat>` becomes `Vec<ErasedData>`, `Option<@P>` becomes `Option<Erased>`, and `Some(h)` becomes `Some(Erased)` or `Some(_)`; `pub(locus)` becomes `pub(crate)`; and `cfg` attributes that every checked configuration satisfies are dropped.
- **Ghost code is written where erasure can remove it.** Erasure edits text, and text inside another macro's input means whatever that macro makes of it. So in export mode, Locus's macros, and calls that pass ghost arguments, are written directly in the crate's own code ([RFC 0002](0002-logical-layer.md#export-mode)).

**Validation.** Before Locus builds or exports a runtime program, it compiles it with the pinned rustc and compares it with the checking program: for every item and typed body that the facts describe, the runtime program must have the same item and the same typed body, apart from the expansions of Locus's macros and, in export mode, the ghost parameters, arguments, and locals that erasure removed and the `match` it adds. A difference is an error, never a program other than the one checked.
- In export mode, it catches a macro that sees different tokens once ghost code is erased, such as an attribute macro around a function whose ghost binding was removed, and any bug in erasure.
- In build mode, the text is the same, so it catches only a macro that reads the environment or a file that changed between the two compiles.
- It compares only what the logic used, the items and bodies in the facts, so its cost is bounded by what was verified.
- What it cannot compare, the guard and the visibilities that export mode writes, is trusted on its own ([the trust model](../TRUST.md)).

**Erasure adds no warnings of its own,** and neither does lowering.
- Lowering marks the items it synthesizes, and ghost bindings that no runtime code uses, so that they cause no lints in the runtime program of build mode.
- In export mode, a binding that only logical code uses is renamed, never removed, so its value is dropped at the same point. The new name begins with an underscore and is fresh in its scope, so it cannot capture another binding. Imports and private items that only logical code uses are removed.
- A runtime field that only logical code reads stays, because removing it would change the type. rustc's dead-code warning for it is reported to the user as "field `x` is read only by logical code", at the user's lint level.
- Likewise, a value that only logical code reads, such as an assignment overwritten before runtime code reads it, keeps its assignment. rustc's warning for it is reported as "the value assigned to `n` is read only by logical code".

Building the runtime program is how Locus reports lints and confirms its own output. Any other warning there is either about the user's code or a Locus bug.

**Source locations.** `file!`, `line!`, `column!`, `Location::caller`, and panic messages report positions in the file that was compiled: the lowered file in build mode, the exported file in export mode. Lowering and erasure keep runtime code on its own line where they can, but a removed item moves the lines after it. This is the one stated exception to "erasure never changes what the program does".

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

The pipeline suggests these components, each with one job. Build tasks create them after this RFC is accepted. Several may start as modules of one crate, and be split when a dependency or a testing boundary calls for it ([#18](https://github.com/nikhilgarg28/locus/issues/18)); the driver, the facts format, the library, and the kernel are separate from the start, because each is linked or trusted differently.

| Crate | Job | Links rustc's internals | Trusted |
|---|---|---|---|
| `locus-syntax` | Lexer, parser, lossless trees | No | Yes: it decides where Locus syntax begins and ends |
| `locus-facts` | The facts format and its serialization | No | Yes |
| `locus-driver` | The rustc driver that reports facts | **Yes, alone** | Yes |
| `locus-library` | Locus's macros, probes, stubs of its logical items, and the markers, as Rust, in a checking and a runtime version | No | Yes: probes are recognized by its definitions, and its macros decide what both programs contain |
| `locus-lower` | The lowered program, generated module, and origin map | No | Yes |
| `locus-logic` | Logical items, typing, the ghost checks, elaboration, obligations | No | Yes, for its ghost checks; obligation generation as decided by [#6](https://github.com/nikhilgarg28/locus/issues/6) |
| `locus-search` | Proof search | No | No |
| `locus-kernel` | Proof checking | No | Yes |
| `locus-erase` | Export mode's runtime program, export surface, guard, and origin maps | No | No for bodies, which validation checks; yes for the guard and visibilities |
| `locus-diagnostics` | Mapping, wording, rendering | No | No |
| `locus` | Command line, both modes, Cargo orchestration, validation, and manifests | No | Yes: it pairs facts with programs, decides which crates are checked, decides when to stop, and validates the runtime program |

## Rules

The rules below go into the specification with this RFC, in [Compatibility with Rust](../../spec/compatibility.md), [Observing runtime values](../../spec/observation.md), and [Exporting to Rust](../../spec/export.md).

| Rule | Examples |
|---|---|
| `compat.superset.same-meaning` | `compatibility/clone-before-move.accept.lc`, `compatibility/plain-rust.accept.lc`, `compatibility/user-method-named-model.accept.lc` |
| `compat.errors.rustc` | `compatibility/logic-block-is-a-struct-literal.reject.lc`, `compatibility/use-after-move.reject.lc` |
| `compat.syntax.not-rust` | `compatibility/at-in-patterns.accept.lc`, `compatibility/keywords-as-names.accept.lc`, `compatibility/pub-locus-as-tuple-field-type.accept.lc`, `compatibility/tokens-in-macro-input.accept.lc`, `compatibility/underscore-in-rust-positions.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.syntax.contextual-keywords` | `compatibility/keywords-as-names.accept.lc`, `compatibility/raw-identifier-logic.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.names.shadowable` | `compatibility/glob-imported-macro-named-prove.accept.lc`, `compatibility/own-prove-inside-vec.accept.lc`, `compatibility/user-macro-named-prove.accept.lc`, `compatibility/user-type-named-nat.accept.lc` |
| `compat.names.no-methods` | `compatibility/user-method-named-model.accept.lc` |
| `compat.names.macros-expand` | `compatibility/locus-macro-inside-rust-macro.accept.lc`, `compatibility/prove-in-macro-output.accept.lc`, `compatibility/prove-in-stringify.accept.lc`, `compatibility/prove-through-alias.accept.lc`, `observation/macro-in-vec-reads-sibling-field.accept.lc` |
| `observe.place.readable` | `observation/after-mutable-borrow-ends.accept.lc`, `observation/before-move.accept.lc`, `observation/macro-in-vec-reads-sibling-field.accept.lc`, `observation/macro-reads-sibling-field.accept.lc`, `observation/proof-before-move.accept.lc`, `observation/after-move.reject.lc`, `observation/during-mutable-borrow.reject.lc`, `observation/let-annotation-after-initializer.reject.lc`, `observation/proof-after-move.reject.lc` |
| `observe.let.after-initializer` | `observation/let-annotation-after-initializer.reject.lc` |
| `observe.captures.rejected` | `observation/logic-in-async-block.reject.lc`, `observation/logic-in-closure.reject.lc` |
| `export.locus-visibility` | `export/locus-logical-field.accept.lc`, `export/wrapped-precondition.accept.lc`, `export/reexport-locus-item.reject.lc` |
| `export.macros-direct` | `export/ghost-call-outside-macro.accept.lc`, `export/prove-through-alias.accept.lc`, `export/ghost-call-inside-macro.reject.lc`, `export/prove-in-macro-output.reject.lc`, `export/prove-inside-rust-macro.reject.lc` |

The accepted near-misses `before-move`, `after-mutable-borrow-ends`, and `proof-before-move` read `v.len()` in logical code, which [RFC 0002](0002-logical-layer.md) defines through `Vec`'s model, and rely on its versions: restating a proof before the move needs `v` to be the same version as at entry. So they are decided together with it.

The observation examples use provisional syntax for propositions ([#5](https://github.com/nikhilgarg28/locus/issues/5)); respelling them does not change their outcomes.

## Interactions

| Area | Effect |
|---|---|
| Types | rustc is the authority on every runtime type. Logical types reach rustc only as stubs that resolve names, and as the marker `Erased` inside runtime types, which carries no proposition ([RFC 0002](0002-logical-layer.md)). Locus types the logical layer itself. |
| Propositions and proofs | rustc never sees a proposition. It sees an observation for each place logical code reads, and a probe for each name it uses. |
| Ownership and mutation | rustc checks all ownership, including what logical code reads. How mutation is modeled in the logic is built from rustc's typed bodies ([#6](https://github.com/nikhilgarg28/locus/issues/6)). |
| Erasure and runtime behavior | Erasure edits the source. Runtime code is untouched, logical code does no runtime evaluation, ghost values are erased from both programs alike, and only source locations change. |
| Configurations | Each configuration is a separate program. Build mode checks the configuration being built. An exported crate is checked in the configurations it declares, and its exported code refuses to compile in any other ([#9](https://github.com/nikhilgarg28/locus/issues/9)). |
| Rust interoperability | In build mode, every crate that depends on a Locus crate is checked, so contracts work across crates. In export mode, `.rs` files sit beside their `.lc` sources, so the crate builds with plain Cargo, and its interface is plain Rust, with no proofs, logical values, or contracts; `pub(locus)` marks what only Locus crates see ([#7](https://github.com/nikhilgarg28/locus/issues/7)). |
| Diagnostics | Rust's errors keep their codes and messages, with spans mapped to the source. Errors that involve logical code are explained in Locus's terms. Lints come from the runtime program. |
| Trust model | New trusted components: rustc's type checker and borrow checker, `locus-driver`, the facts format, Locus's library, the parser, the lowering, obligation generation, erasure, and the orchestration. New assumptions: macro inputs, and, for exported crates, toolchains, dependencies, and published files. [RFC 0002](0002-logical-layer.md) adds the rest. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Remove ghost parameters before rustc runs,** so that the checking program, like the runtime program, has no proof parameters. This was the previous draft. Rejected:
- Lowering would have to know, at every call, which arguments to delete and whether a result is ghost, which needs the callee's signature, and so the receiver's type, method resolution, and trait resolution: rustc's work.
- A wrong guess can compile with another meaning. Tested with rustc 1.97: with an inherent `go(&self, a, h: @P)` and a trait method `go(self, a)` in scope, deleting `h` from `x.go(1, h)` compiled and called the trait method; and a proof-returning call lowered to return `()` instantiated `make::<()>` and compiled.
- The two programs would differ in their ghost bindings, so a macro around a function would see different tokens in each. An external review built a deterministic attribute macro that returned `0` in the checking program and `1` in the runtime program.

**Keep the markers in the checking program only,** and remove them from build mode's runtime program afterwards, using the facts. Rejected: it keeps the runtime program of build mode readable, but no one reads it, and the two programs would again differ under every attribute macro.

**A discovery run:** compile a provisional program first to learn types and resolutions, then lower precisely and compile again. Rejected for now. It costs a second compile of every checked crate, and it must be shown to converge. One lowered text needs neither. It stays the fallback if a case arises that only types can decide.

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

**Charon** exports rustc's MIR in a stable format, and is a candidate source of typed bodies for [#6](https://github.com/nikhilgarg28/locus/issues/6). Run on the checking program, it could also report what the probes ask. It pins its own nightly, and adopting it would make its format part of the trusted path.

**Distinct stand-ins for each logical type in the checking program, and one marker in the runtime program.** Rejected: macros would see different types in the two programs, and types that are distinct in one would collide in the other. Both programs use the same two markers, and no `impl` mentions a logical type ([RFC 0002](0002-logical-layer.md)).

**Zero-sized stand-ins for proofs that Locus believes by their type,** in both programs. Rejected: Rust code that can tell types apart, through `Any` or specialization, could turn one proof into another. [RFC 0002](0002-logical-layer.md) makes proofs ghost instead: `Erased` is still one type for every proof, but Locus never believes it.

**One Rust type per proposition,** so that rustc could tell proofs apart. Not possible: propositions mention runtime values, so `@(y != 0)` is a different proposition for each `y`.

**Remove logical and proof fields from types.** Rejected at the owner's direction. It needs renumbered tuple fields, extra `PhantomData` fields, and observations of the whole value where logical code reads one field, and Locus's macros expanded inside other macros could not name the field at all. Kept as `Erased`, a field costs derives that `Erased` does not implement, such as serde's, and shows up in `Debug` output.

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
8. Proofs and logical values are ghost ([RFC 0002](0002-logical-layer.md)): zero-sized markers, `Erased` for proofs and `ErasedData` for other logical data.
9. Logical and proof fields stay in both programs, as fields of their marker's type.
10. Traits among Locus's names have no methods.
11. The parser starts from a fork of rust-analyzer's. The owner left the choice to this RFC.
12. Locus's macros are real macros, which rustc expands wherever it expands macros. Export mode accepts only invocations written directly in the crate's own code.
13. An exported crate uses its Locus dependencies through their exported interfaces, so contracts between crates work only in build mode.
14. Build mode checks Locus dependencies from their sources, so a Locus crate published for Locus users includes its `.lc` files.
15. **One lowered text for build mode.** The checking and runtime programs of build mode are one lowered text, compiled against two versions of Locus's library, with every ghost position kept as a marker. Lowering decides only what the source shows, and confirms every assumption against the facts. This revisits decision 8 of the previous draft, that proof parameters are removed, for build mode only, whose runtime program no one reads; an exported program still removes them. It is subject to [the spikes](#spikes) below.
16. **Validation.** In both modes, Locus compares the runtime program's items and typed bodies with the checking program's, for everything the facts describe, and fails on any difference. Export mode keeps erasure textual, so its output stays readable.
17. **Interfaces and manifests.** Each checked crate gets an interface that its dependents are checked against, and each result a manifest of what it relies on.
18. **Acceptance waits for the spikes** of build mode's lowering and of the Cargo graph. A semantic slice, verifying one mutable function with a reborrow and a destructor end to end, goes with the RFC for the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)).

**Still for the owner:**
1. `.rs` files may not name logical items, as proposed.
2. Logical code in closures and async blocks, and reads of packed fields, are rejected for now, as proposed.
3. Traits among Locus's names also have no associated functions or constants (an extension of 10 above).
4. Until [#9](https://github.com/nikhilgarg28/locus/issues/9) decides more, an exported crate whose checked code tests target properties other than pointer width must declare its configurations.

**Left to other RFCs**, each with its issue:
- Logical types, propositions, proofs, models, and contracts: [RFC 0002](0002-logical-layer.md), proposed with this one.
- The remaining syntax of propositions and proofs: [#5](https://github.com/nikhilgarg28/locus/issues/5).
- The runtime semantics: how typed bodies become statements in the logic, THIR or MIR, where borrows end, integer overflow across profiles, and whether observations of a value with proof fields can be narrowed to the fields its propositions read: [#6](https://github.com/nikhilgarg28/locus/issues/6).
- Distribution: package contents, including how an exported package's manifest names its exported crate root; how configurations are declared; dependency versions; how Locus recognizes Locus dependencies; and keeping rustfmt off exported files: [#7](https://github.com/nikhilgarg28/locus/issues/7).
- Logical code in closures and async blocks, Locus syntax inside other macros' input, and reads of packed fields: [#8](https://github.com/nikhilgarg28/locus/issues/8).
- Corner cases of `cfg` and compiler flags, and doctests in build mode: [#9](https://github.com/nikhilgarg28/locus/issues/9).
- Editor support, likely a language server that runs rust-analyzer on the checking program and maps results back through origin maps.
- Incremental checking and caching of facts.
