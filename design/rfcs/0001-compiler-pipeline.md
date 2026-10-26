# RFC 0001: The compiler pipeline

- **Status:** proposed
- **Issue:** none
- **Supersedes:** none

## Summary

Locus compiles a Cargo crate in four steps:

1. **Parse.** Locus parses its `.lc` sources, Rust plus Locus syntax, into lossless syntax trees.
2. **Check with rustc.** Locus lowers the crate to a **checking program**: ordinary Rust in which proofs and logical values are removed, logical items are stubs that only resolve names, and every place logical code reads becomes a borrow. rustc compiles it through a small driver. The driver reports what Locus needs: the types of the places logical code reads, what the names in logical code refer to, and the typed bodies of the functions being verified. rustc's borrow checker decides whether logical code may read what it reads.
3. **Check the logic.** Locus checks the logical layer against those facts, constructs proofs, and has the kernel check them.
4. **Emit.** Locus erases the logical layer by editing each `.lc` file into the `.rs` file beside it: readable Rust, which Cargo builds like any other crate. The `.rs` file is written only when the check passes.

Locus writes its own parser, the lowering, the logical layer, the kernel, erasure, and diagnostics. Everything else that Rust already does is rustc's: name resolution, macro expansion, type inference, trait resolution, ownership and borrow checking, lints, and code generation.

## Motivation

The [vision](../../VISION.md) commits Locus to being a superset of Rust in which "rustc does Rust's work". v0 did the opposite. It had its own front end for a growing subset of Rust, reimplementing moves, borrows, traits, and generics, and most of its 71,000 lines went there. This RFC decides how the work is split instead, which representations exist, how information flows between Locus and rustc, how rustc's errors reach the user, and where generated code lives.

It settles two of the vision's foundational questions:
- **Type information:** where Locus gets Rust's types and resolutions.
- **Ownership of logical code:** how logical code, which rustc never runs, is kept from reading values it may not read.

It also fixes the rules that Locus syntax and Locus names must follow.

A [spike](../../spikes/rustc-facts/README.md) tested the central mechanism. A hand-written checking program, compiled by a 150-line rustc driver running as Cargo's wrapper, reported:
- the types of the places its proofs read, including a field of a type from a dependency;
- what a path in a proof resolves to;
- the typed body of a function, with indexing resolved to the trait method `<Vec<u32> as Index<usize>>::index`.

rustc's borrow checker rejected the one proof that read a moved value, with E0382 on the matching line. The driver built against stable rustc 1.97 (with `RUSTC_BOOTSTRAP=1`) and against a nightly four releases later, needing a one-line change between them.

Two adversarial reviews of earlier drafts found problems in several areas:
- configurations;
- divergence between the checking and runtime programs;
- where observations happen;
- how errors are attributed and how Locus's names are found;
- one way to forge proofs, through `Any`, when every proof had the same zero-sized Rust type.

This revision addresses them. Each is answered below, recorded as an open question, or tracked in an issue. The last one led to removing proofs and logical values from the programs rustc compiles altogether, as [RFC 0002](0002-logical-layer.md) describes; the two RFCs are proposed together.

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
   ├──────────────► checking program + origin maps
   │                     │
   │                     │ compile, through locus-driver .. rustc
   │                     ▼
   │                facts + rustc diagnostics
   │                     │
   ▼                     ▼
 logical pass: resolve, type-check, elaborate ............. Locus
   │
   ▼
 obligations ─► proof search (untrusted) ─► kernel (trusted) Locus
   │
   │ every proof checks, in every checked configuration
   ▼
 erase: edit each .lc into its .rs ........................ Locus
   │
   ▼
 runtime program ─► cargo build ............................ rustc
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
| Erasure, and emitting readable Rust | Locus |
| Monomorphization, code generation, linking | rustc |
| Diagnostics: rendering, and mapping rustc's back to the source | Locus |

### Sources and generated files

Locus compiles one Cargo crate at a time, in the crate's real Cargo context: its edition, features, `cfg`s, and dependencies.

- **`.rs` files are plain Rust.** They are compiled as they are, and they may not name logical items: no logic functions, logical types, or proofs. The driver reports what every path resolves to, and a `.rs` path that resolves to a logical item is an error.
- **`.lc` files may also contain Locus syntax and use Locus names.** Renaming a `.rs` file to `.lc` changes nothing about the program.
- **Each `name.lc` has its runtime program beside it, as `name.rs`.**
  - `cargo locus build` writes it only after the whole crate checks, in every checked configuration.
  - Its last lines say that it is generated, name its source, and record the source's hash.
  - They come last so that they do not move the lines above them.
- **Modules work as in Rust.** `mod name;` finds `name.rs` by Rust's usual rules. So plain `cargo build`, rustc, rust-analyzer, docs.rs, and anyone depending on the crate see an ordinary Rust crate. They need neither Locus nor a build script.
- **Generated files are committed and published,** like the output of other code generators.
  - `cargo locus check` fails when a generated file does not match what its source would produce, whether it is stale or was edited by hand.
  - An optional, dependency-free build-script helper can warn during a plain `cargo build` when a `.lc` file has changed since its `.rs` was generated.
  - rustfmt formats every module file, so `cargo fmt` can make a generated file stale. rustfmt leaves a module alone when its `mod` declaration is marked `#[rustfmt::skip]` (tested), and erasure adds that mark where the declaration is in a `.lc` file. Protecting the rest, a generated crate root or a declaration in plain Rust, is [#7](https://github.com/nikhilgarg28/locus/issues/7).
- **Each crate has one generated module,** a generated file that the crate root declares. In the runtime program it holds the configuration guard (see [Configurations](#configurations)). In the checking program it also holds the prelude that makes Locus's names visible (see [Locus syntax and Locus names](#locus-syntax-and-locus-names)).
  - When the root is a `.lc` file, erasure adds the declaration.
  - When the root is plain Rust, the user adds one line, such as `mod locus_erased;`, under a name the crate does not otherwise use, and `cargo locus check` says exactly which line.
- **The checking program is never committed.** It lives under `target/locus/check/` and mirrors the crate: each `name.lc` lowered, each `.rs` copied.
- **An opt-in mode generates into `OUT_DIR` instead,** from a build script, for crates whose builders all have Locus installed, such as applications and internal workspaces.
  - The crate includes each generated file with `include!(concat!(env!("OUT_DIR"), "/name.rs"))`. This was tested, including a `mod sub;` inside the included file, which finds its file beside it. An included file cannot have inner attributes.
  - In this mode, Locus checks the configuration of the build that runs it, and resolves that configuration's `cfg`s in its output. [#9](https://github.com/nikhilgarg28/locus/issues/9) records what a build script cannot see.
  - The details are [#7](https://github.com/nikhilgarg28/locus/issues/7).

This is what makes the common case cheap. A crate that is mostly Rust adopts Locus by adding a `.lc` file, its generated `.rs`, the generated module, and one line in its crate root. Nothing changes for anyone who builds the crate, and the only new command is `cargo locus check` in its CI.

### Locus syntax and Locus names

Locus extends Rust in two ways. Neither can change what a Rust program means.

**Locus syntax** is recognized from the tokens alone, without resolving any name. It appears only where rustc rejects those tokens in code it compiles.

| Form | Where | Example | Why Rust cannot mean it |
|---|---|---|---|
| Logic function | Item | `logic fn total(xs: &[u32]) -> Nat { ... }` | Rust has no item beginning `logic fn` |
| Proposition declaration | Item | `prop Sorted(xs: Seq<u32>) { ... }` | Rust has no item beginning `prop Name(` |
| Proof type | Type | `h: @(x > 0)`, `@Sorted(xs)` | `@` cannot begin a type |
| Named result | Return type | `-> (out: u8, @(out > n))` | Tuple types have no field names |
| Proof hole | Expression | `(out, _)` | rustc rejects `_` as an expression except as an assignment target, an array length, or a generic argument, and the hole is recognized nowhere else |

`@` is reserved for proofs.

- **What "valid Rust" means here: code that rustc accepts when it is compiled.** Code that `cfg` removes is only parsed by rustc, and a Locus form there has no meaning in either language.
- **Locus syntax is not recognized inside macro invocations or attribute arguments.** Those are token trees, and their meaning is the macro's.
- **Rust's own contextual keywords can precede `fn`** (`safe`, `default`, `const`, `async`, `unsafe`, `extern`). `logic` is not one of them. If Rust ever gives `logic fn` or `prop Name(` a meaning, Locus follows Rust and respells its forms.

**Locus names** are ordinary Rust items:
- the macros `logic! { ... }` for logical blocks, `prop!(...)` for propositions, and `prove!(...)` for proofs, from Locus's library;
- the derive `Logical`;
- the trait `Model`;
- the logical types and functions of Locus's standard library, such as `Nat`.

`Logical` can only be derived, never implemented by hand. Deriving it fails unless every field and payload is logical. `Model` is implemented by hand. What they mean is [RFC 0002](0002-logical-layer.md).

**Traits among Locus's names have no methods, associated functions, or associated constants.** In Rust, bringing a trait with a method into scope can change what an existing method call resolves to, or make it ambiguous. This was tested: a prelude `Model` with a `model` method made a call to a user trait's own `model` method fail with E0034. That is why Rust adds prelude traits only at edition boundaries. So `Model`'s logical function takes the value as an ordinary argument.

How the names are found:
- **They are visible the way std's prelude is.** In the checking program, every module sees a prelude that combines std's with Locus's, so rustc resolves Locus names with Rust's own rules.
  - The prelude is in the checking program's copy of the crate's generated module, and re-exports Locus's names from its library.
  - Declaring it needs rustc's internal `prelude_import` feature, which the driver enables for the checking program only.
- **A program's own definitions and imports take precedence,** exactly as they do over std's prelude: its own `macro_rules! prove`, `use other::prove`, `struct Nat`, or a glob import of a type named `Nat`. All of these were tested.
- **Glob-imported macros are the one exception in Rust's rules.** They collide with prelude macros (E0659), and that is already true of std's `vec!`. So does a macro defined by another macro's expansion. When rustc reports such an ambiguity involving a Locus name, Locus checks the crate again without that name in its prelude. The program's own macro keeps its Rust meaning, and that crate cannot use Locus's macro of that name.
- **Recognizing Locus's names.** Lowering assumes that each of these is Locus's:
  - an invocation of a Locus macro name;
  - a `derive(Logical)`;
  - an `impl Model`.

  The driver then reports which definition each one actually resolved to, by the definition's identity rather than its path. If one turns out to be the program's own, Locus discards everything from that run, errors included, and lowers again, treating it as plain Rust. A crate normally needs one rustc run, and needs a second only when names collide.
- **Locus's macros work only where Locus can see them.** Lowering replaces every invocation of a Locus macro that it recognizes. Locus's library defines each macro to expand to a marker, which the driver reports. So an invocation that rustc expands anyway is reported as a Locus error. Examples are an invocation inside another macro's input, such as `vec![prove!(...)]`, inside a macro's output, or under another name, through `use prove as check` (tested to resolve).
  - A Locus macro name that is never expanded, such as inside `stringify!`, is only tokens.
  - Lowering inside macros is [#8](https://github.com/nikhilgarg28/locus/issues/8).

### How facts enter the logic

There is no wall between Rust and Locus inside a crate. Plain Rust and Locus code mix freely, and every file goes through the pipeline. Soundness comes instead from the only four ways facts about runtime values can enter the logic:

1. **Models:** observing a runtime value through its type's model.
2. **Typed bodies Locus understands:** rustc's typed bodies, translated into the logic.
3. **Assumed specifications:** each declared with a reason, and listed by the audit.
4. **Invariants of the program's own types,** stated by their proof fields, as described below.

Each is sound no matter who wrote the code.

- **Models read only data that cannot change while the value is shared.** Rust's type system guarantees this, except for the contents of an `UnsafeCell` and the targets of raw pointers.
  - Models are logic code, and they can read fields only through those fields' models.
  - Types from the standard library with `unsafe` internals (`Vec`, `Rc`, `String`) get models declared by Locus's library, each as a trusted entry with its reason.
  - `Cell`, `RefCell`, `Mutex`, and the atomics have no model of their contents.

  So **a call Locus knows nothing about, taking `&x`, never changes `x`'s model**: Rust freezes what the model reads, and the model never reads what Rust does not freeze.
- **Plain Rust in a Locus crate is still checked by Locus.** For example, plain Rust that assigns on its own to a field that a proof field reads is rejected; it must replace the whole value, with a new proof.
- **Code Locus cannot translate is opaque:** dependencies, trait objects, inline assembly, and anything else outside what Locus models. It runs, but its results are known only by their types, and anything it receives through `&mut` may come back as any value of its type.
- **Opaque code never sees a proof.** Proofs and logical values are ghost: neither program contains them, and they never instantiate a generic parameter ([RFC 0002](0002-logical-layer.md)). So no Rust code, through `Any`, specialization, or anything else, can copy, convert, or forge one, and Locus never relies on Rust's generics being parametric.
- **Invariants hold for every value.** Every value of a type with proof fields satisfies them. Only the crate's checked code constructs the type or writes the fields its proofs read, which are not public. So a value that comes back from opaque code still satisfies its invariant: opaque code can only move it, copy it, drop it, or call the type's own checked functions.
- **Statics.** Logical code may read only immutable statics whose types have models.

The details of models are [RFC 0002](0002-logical-layer.md).

### The checking program

The checking program is what rustc sees.

- **It is written as ordinary `.rs` files** under `target/locus/check/`, and rustc parses them like any crate. Files on disk can be opened, compared, and compiled by hand when something goes wrong. If disk access ever matters, the driver can serve the same text from memory through rustc's file-loader hook, without changing anything else.
- **Lowering keeps each source line on the same line of the checking program where it can,** and appends synthesized items at the end. Raw rustc output is then readable when debugging Locus itself. The origin maps, not the alignment, are the authority.
- **Locus's library crate holds its macros, its probes, and the stubs of its logical items,** such as `Nat` and `Model`. rustc receives it under a crate name the program does not use.
- **New names are fresh.** Every name Locus introduces, such as the library's crate name and the generated module, is chosen so that it appears nowhere in the crate's sources. User code therefore cannot name generated items, and generated items cannot collide with the user's, even under `#![no_implicit_prelude]`.
- **The checking program is compiled with `--cap-lints allow`.** Lints there would be artifacts of the lowering, and a user's `#![deny(warnings)]` or `forbid` must not turn synthesized code into errors. This was tested. Real lints come from building the runtime program.

Logical code is never given to rustc: its operators mean different things from Rust's (`+` on `Nat` cannot overflow), and its types do not fit Rust's (a logical `List` can contain itself without a `Box`). What rustc sees instead:

| In the source | In the checking program |
|---|---|
| Runtime code | Copied unchanged |
| A parameter, result component, local, or field whose type is logical, proofs included, and every argument for one | Removed; see below |
| `Option<@P>`, and a `Result` with a proof payload | `Option<()>`, and `Result` with `()` in place of the proof |
| A named result `(out: T, @P)` | `T` |
| A call of a runtime function whose result is entirely ghost | The call, as a statement |
| A logic function, `prop` declaration, `#[derive(Logical)]` type, or `Model` implementation | A stub: an empty type, or a function with the same name, generics, and signature, whose body panics if it is ever run |
| A proof hole, `_` | Removed with the proof it stands for |
| A `logic!`, `prop!`, or `prove!` invocation, a proposition in a type annotation inside a body, or an argument of logical type | A block that observes every place the logical code reads, where the logical code reads it, and resolves every name it uses |
| A proposition in a signature or a struct field's type | Nothing; see below |

**Stubs exist only to resolve names.**
- Runtime code never refers to a stub, so no stub is ever a runtime value. Only probes and other stubs mention them.
- A stub type is an empty, zero-sized type whose generic parameters are carried in a `PhantomData` field.
- Proofs in stub signatures all use one placeholder type, which exists only in the checking program and never runs.
- Stub functions are `const fn` where Rust allows, so that probes work in constant contexts. Probes are type-checked but never evaluated, and stub bodies panic rather than loop, so that a mistake fails rather than hangs constant evaluation.

**Proofs and logical values never reach rustc.** Ghost values are removed from the checking program as they are from the runtime program ([RFC 0002](0002-logical-layer.md)). So rustc never computes a type that involves a proof, and Locus alone checks every proof against the proposition its position requires.

**Logical and proof fields are removed** from structs and enum variants, in both programs. A type's derives and its Rust API then see only its runtime fields, and no lint fires for them.
- Where a generic parameter would otherwise be unused, a `PhantomData` field under a fresh name takes its place.
- Fields of a tuple struct or tuple variant after a removed one are renumbered.
- Constructing the type omits the field. A pattern that binds the field binds nothing.
- Reading the field is logical code. The checking program observes the whole value there as a shared borrow, so rustc still rejects the read after the value has moved or while it is mutably borrowed.
- An assignment to a logical data field is erased, and its right-hand side is logical code. The checking program observes the value there as a mutable borrow.
- Whether these observations can be narrowed to the fields a proposition is about is [#6](https://github.com/nikhilgarg28/locus/issues/6).

**Logical code contains no runtime evaluation.** It reads places, and it never calls runtime functions, mutates, or has effects; the arguments of a logical call are logical code too. A runtime function that returns a proof is runtime code: it is called, and kept, in both programs. So erasing a logical construct never needs to preserve anything it did.

**Logical bindings are never bound in either program.** A `let` that binds a logical value, such as a `logic!` block's, leaves only its observations in the checking program, and nothing in the runtime program. A call whose result is entirely ghost stays, as a statement. So rustc never has to infer a type for a logical value.

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

- **Which configurations are checked.** A crate declares the configurations it is checked in; how is [#7](https://github.com/nikhilgarg28/locus/issues/7). `cargo locus check` checks each one, and passes only if all of them pass.
  - **The default** is the dev, release, and test builds, with default features, for a target with 64-bit pointers. These are what `cargo build`, `cargo build --release`, and `cargo test` build.
  - **The default does not depend on the machine that runs the check.** For now, a crate whose checked code tests other properties of the target, such as `target_os`, must declare its configurations.
- **One output for all of them.** Erasure writes one runtime program, which must be right in every checked configuration.
  - Code that no checked configuration enables is removed, since it was never checked.
  - A `cfg` attribute that every checked configuration satisfies is dropped. Every other `cfg` attribute is kept as written.
  - Erasure's decisions that depend on facts, such as whether an import names only logical items, must agree in every checked configuration in which that code is active, or the check fails.
- **The runtime program refuses to compile in any other configuration.** The crate's generated module holds a guard that turns every unchecked configuration into a `compile_error!` naming the checked ones:

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
  - v0 guarded its output against a different pointer width the same way.
- **The cost is Cargo's feature unification.** A downstream crate can enable any combination of a library's features. If that combination changes what checked code sees and was not checked, the downstream build fails with the guard's message. Library authors therefore declare the combinations they support. The alternatives are worse:
  - checking one configuration and trusting the rest is unsound;
  - checking every combination is infeasible;
  - banning `cfg` in checked code rules out ordinary platform-specific code.
- **Behavior that differs between builds without a `cfg`** is the runtime semantics' to handle. Integer overflow, for example, depends on the `overflow-checks` setting, which no stable `cfg` can test, so a proof must hold either way ([#6](https://github.com/nikhilgarg28/locus/issues/6)).
- **What the guard cannot test** becomes an assumption in [the trust model](../TRUST.md):
  - the compiler that builds the runtime program;
  - the versions and features of dependencies;
  - the inputs to macros.

  The distribution RFC ([#7](https://github.com/nikhilgarg28/locus/issues/7)) will narrow these.
- **Corner cases.** [#9](https://github.com/nikhilgarg28/locus/issues/9) collects the cases this design does not yet settle, among them `cargo test --release`, docs.rs's `docsrs`, target features, build-script `cfg`s, and `cfg`s written by other crates' macros.

### Running rustc: `locus-driver`

`locus-driver` is a small program linked against rustc's internal crates, the way Clippy is.

- **How Cargo runs it.** Cargo runs it in place of rustc, through `RUSTC_WORKSPACE_WRAPPER` for workspace members, or through `RUSTC_WRAPPER` when Locus dependencies must be checked too. Either way, Cargo does the orchestration, as for any build, and Locus gets the exact compiler invocation, with its edition, `cfg`s, features, and `--extern` paths.
- **For a crate Locus checks,** the driver substitutes the checking program's root for the crate's root, and records facts after type checking. It then lets rustc continue so that the borrow checker runs, and stops before code generation.
- **For every other crate,** it behaves exactly like rustc.
- **It fails closed.** If it was asked to check a crate and cannot produce facts for it, the check fails. The spike's silent fallback to plain compilation is not acceptable.
- **Facts are tied to their program.** The driver records an identity of the checking program it compiled, and Locus rejects facts that do not match the program it generated.

**Only `locus-driver` touches rustc's internals.** It writes facts in a format that Locus owns and versions. The rest of Locus reads that format, builds on stable Rust, and never links rustc. This keeps rustc's API churn in one small crate; the spike measured one renamed function across four releases, for the part of the API it uses. It also lets another producer replace the driver later without touching anything else.

**Toolchain.** The driver is built for one pinned rustc release. The preference is the current stable release, built with `RUSTC_BOOTSTRAP=1`, which the spike showed works. The fallback is a pinned nightly, if a needed internal API exists only there. Either way:
- `locus check` needs that one toolchain, which Locus installs.
- The runtime program is plain Rust, built by whatever compiler builds the crate (see [Configurations](#configurations)).
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
3. **Origin maps.** Every generated file, checking or runtime, has an ordered list of segments covering the whole file:

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
       markers: Vec<Span>,                       // Locus macros that rustc expanded unlowered
       paths: Vec<(Span, ItemRef)>,              // resolutions of paths in `use`, `impl` headers, and .rs files
       items: Map<ItemRef, ItemInfo>,            // signatures, fields, variants, generics, bounds
       bodies: Map<ItemRef, TypedBody>,          // typed bodies of the functions being verified
   }
   ```

   - `Ty` and `ItemRef` describe Rust types and items by path and generic arguments, independently of rustc.
   - `TypedBody` is derived from rustc's THIR. Every dereference, borrow, coercion, and call is explicit. A call to a trait method names the method and its `Self` type; where the types are concrete, the driver also resolves the implementation with rustc's instance resolution. In generic code the call stays a trait method.
   - Facts are serialized, as JSON at first, so that the driver and the rest of Locus can run as separate processes.
5. **Logical IR.** Locus's typed representation of logical items, propositions, and proofs. It refers to runtime types and items only through `Ty` and `ItemRef` from the facts.
6. **Kernel terms and obligations.** Defined by the kernel RFC.
7. **Edits.** The runtime program is a list of `Edit { range, replacement }` applied to each `.lc` file, with an origin map like the checking program's.

### The runtime program

Erasure edits each `.lc` file into its `.rs` file. Locus syntax is removed or replaced, and every other byte is kept.

**Readable by construction.** A `.lc` file with no Locus syntax becomes an identical `.rs` file, apart from its header. Comments, formatting, and names survive everywhere. Where there is Locus syntax, what remains is the Rust a programmer would have written without proofs.

**No logical values.** The runtime program contains no proofs, no logical values, and no logical items, so it has no stand-in types. Both programs remove the same ghost constructs, so macros and derives see the same tokens in both, and both have identical types.

**What the edits do:**
- **Removed:**
  - logic functions, `prop` declarations, `#[derive(Logical)]` types, `Model` implementations, and `impl` blocks holding only those;
  - parameters, result components, locals, and fields of logical type, proofs included, with every argument for them and every `logic!`, `prop!`, `prove!`, and proof hole;
  - `use` declarations and private items that only logical code uses;
  - code that no checked configuration enables.

  Which of these apply comes from the facts.
- **Kept:** a call of a runtime function whose result is entirely ghost, as a statement.
- **Simplified:**
  - A named result becomes its runtime components.
  - `Option<@P>` becomes `Option<()>`, and `Some(h)` becomes `Some(())` or `Some(_)`; `Result` with a proof payload likewise.
  - `cfg` attributes that every checked configuration satisfies are dropped.

**Erasure adds no warnings of its own.**
- A binding that only logical code uses is renamed, never removed, so its value is dropped at the same point. The new name begins with an underscore and is fresh in its scope, so it cannot capture another binding.
- Imports and private items that only logical code uses are removed.
- A runtime field that only logical code reads stays, because removing it would change the type. rustc's dead-code warning for it is reported to the user as "field `x` is read only by logical code", at the user's lint level.

Building the runtime program is how Locus reports lints and confirms its own output. Any other warning there is either about the user's code or a Locus bug.

**Source locations.** `file!`, `line!`, `column!`, `Location::caller`, and panic messages report positions in the runtime program's file. Erasure keeps runtime code on its own line where it can, but a removed item moves the lines after it. This is the one stated exception to "erasure never changes what the program does".

**No new borrow errors.** The checking and runtime programs have identical types. They differ only in:
- observations, which are borrows ending immediately;
- stubs, which the runtime program does not contain;
- ghost constructs, which do no runtime evaluation;
- items that only logical code uses.

So the runtime program borrow-checks whenever the checking program does. Building it confirms this.

**There is nothing to forge.** No proof exists at run time. What Rust code outside the crate could do instead is call a function without meeting its precondition, or build a value without its invariant. So the exported surface protects those:
- **A function with a precondition is `pub unsafe fn`,** with its proposition in its `# Safety` documentation, as Rust's own `get_unchecked` is. That covers proof parameters, and proofs inside an `Option` or `Result` parameter. The vision's `find(xs, target, sorted: Option<@Sorted(xs)>)` becomes `unsafe fn find(xs: &[u32], target: u32, sorted: Option<()>)`, which keeps the `Some` or `None`, with the safety condition "if `sorted` is `Some`, `xs` is sorted".
  - A Locus crate that depends on it calls it inside `unsafe`, with a generated `// SAFETY:` comment naming the proof Locus checked.
  - A plain Rust caller can call it too, but only by writing `unsafe` and taking responsibility for the precondition.
- **A function that returns proofs** simply returns its data. It needs no wrapper.
- **A field that a proof field reads is not a public Rust field,** so a value with an invariant can be built only through the crate's own functions.
- **A trait with contracts is `unsafe` to implement.**

The details, including how calls inside the crate are written, are [#7](https://github.com/nikhilgarg28/locus/issues/7).

### Diagnostics

Diagnostics come from five places.

| Source | Code | What the user sees |
|---|---|---|
| Locus's parser | `Lnnnn` | Locus's message |
| rustc on the checking program, an error of the user's own | rustc's `Ennnn` | rustc's message, mapped to the source |
| rustc on the checking program, an error that involves logical code | `Lnnnn` | Locus's message, with rustc's code and message as a note |
| Locus's logical pass and kernel | `Lnnnn` | Locus's message |
| rustc on the runtime program | rustc's | Mapped to the source. An error in copied text is the user's, such as a post-monomorphization error that a check run misses. An error in synthesized text is a Locus bug |

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

The pipeline suggests these crates, each with one job. They would be created by build tasks after this RFC is accepted.

| Crate | Job | Links rustc's internals | Trusted |
|---|---|---|---|
| `locus-syntax` | Lexer, parser, lossless trees | No | Yes: it decides where Locus syntax begins and ends |
| `locus-facts` | The facts format and its serialization | No | Yes |
| `locus-driver` | The rustc driver that reports facts | **Yes, alone** | Yes |
| `locus-library` | Locus's macros, probes, and stubs of its logical items, as Rust | No | Yes: probes are recognized by its definitions |
| `locus-lower` | Checking program, generated module, and origin maps | No | Yes |
| `locus-logic` | Logical items, typing, the ghost checks, elaboration, obligations | No | Yes, for its ghost checks; obligation generation as decided by [#6](https://github.com/nikhilgarg28/locus/issues/6) |
| `locus-search` | Proof search | No | No |
| `locus-kernel` | Proof checking | No | Yes |
| `locus-erase` | Runtime program, guard, and origin maps | No | Yes |
| `locus-diagnostics` | Mapping, wording, rendering | No | No |
| `locus` | Command line, Cargo orchestration | No | Yes: it pairs facts with programs and decides when to stop |

### Later: building executables directly

The same driver can compile the runtime program itself, through code generation in the same process. Nothing else in the pipeline changes.

## Rules

The rules below go into the specification with this RFC, in [Compatibility with Rust](../../spec/compatibility.md) and [Observing runtime values](../../spec/observation.md).

| Rule | Examples |
|---|---|
| `compat.superset.same-meaning` | `compatibility/plain-rust.accept.lc`, `compatibility/clone-before-move.accept.lc`, `compatibility/user-method-named-model.accept.lc` |
| `compat.errors.rustc` | `compatibility/use-after-move.reject.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.syntax.not-rust` | `compatibility/underscore-in-rust-positions.accept.lc`, `compatibility/at-in-patterns.accept.lc`, `compatibility/tokens-in-macro-input.accept.lc`, `compatibility/keywords-as-names.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.syntax.contextual-keywords` | `compatibility/keywords-as-names.accept.lc`, `compatibility/raw-identifier-logic.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.names.shadowable` | `compatibility/user-macro-named-prove.accept.lc`, `compatibility/glob-imported-macro-named-prove.accept.lc`, `compatibility/user-type-named-nat.accept.lc`, `compatibility/own-prove-inside-vec.accept.lc` |
| `compat.names.no-methods` | `compatibility/user-method-named-model.accept.lc` |
| `compat.names.direct-only` | `compatibility/locus-macro-inside-rust-macro.reject.lc`, `compatibility/prove-in-macro-output.reject.lc`, `compatibility/prove-through-alias.reject.lc`, `compatibility/prove-in-stringify.accept.lc`, `compatibility/own-prove-inside-vec.accept.lc` |
| `observe.place.readable` | `observation/before-move.accept.lc`, `observation/after-mutable-borrow-ends.accept.lc`, `observation/after-move.reject.lc`, `observation/during-mutable-borrow.reject.lc`, `observation/proof-after-move.reject.lc`, `observation/let-annotation-after-initializer.reject.lc` |
| `observe.let.after-initializer` | `observation/let-annotation-after-initializer.reject.lc` |
| `observe.captures.rejected` | `observation/logic-in-closure.reject.lc`, `observation/logic-in-async-block.reject.lc` |

The accepted near-misses `before-move` and `after-mutable-borrow-ends` read `v.len()` in logical code, which [RFC 0002](0002-logical-layer.md) defines through `Vec`'s model, so they are decided together with it. `proof-before-move` stays open: restating a proof needs `v` to be unchanged since entry, which is the runtime semantics ([#6](https://github.com/nikhilgarg28/locus/issues/6)).

The observation examples use provisional syntax for propositions ([#5](https://github.com/nikhilgarg28/locus/issues/5)); respelling them does not change their outcomes.

## Interactions

| Area | Effect |
|---|---|
| Types | rustc is the authority on every runtime type. Logical types reach rustc only as stubs that resolve names, and proofs and logical values never reach it ([RFC 0002](0002-logical-layer.md)). Locus types the logical layer itself. |
| Propositions and proofs | rustc never sees a proposition. It sees an observation for each place logical code reads, and a probe for each name it uses. |
| Ownership and mutation | rustc checks all ownership, including what logical code reads. How mutation is modeled in the logic is built from rustc's typed bodies ([#6](https://github.com/nikhilgarg28/locus/issues/6)). |
| Erasure and runtime behavior | Erasure edits the source. Runtime code is untouched, logical code does no runtime evaluation, ghost values are removed from both programs alike, and only source locations change. |
| Configurations | Each configuration is a separate program. A crate is checked in the configurations it declares, and its generated code refuses to compile in any other ([#9](https://github.com/nikhilgarg28/locus/issues/9)). |
| Rust interoperability | Generated `.rs` files sit beside their `.lc` sources, so the crate builds with plain Cargo. Rust code never sees a proof, and anything that needs one is `unsafe` at the Rust boundary ([#7](https://github.com/nikhilgarg28/locus/issues/7)). |
| Diagnostics | Rust's errors keep their codes and messages, with spans mapped to the source. Errors that involve logical code are explained in Locus's terms. Lints come from the runtime program. |
| Trust model | New trusted components: rustc's type checker and borrow checker, `locus-driver`, the facts format, Locus's library, the parser, the lowering, obligation generation, erasure, and the orchestration. New assumptions: toolchains, dependencies, macro inputs, and published generated files. [RFC 0002](0002-logical-layer.md) adds the rest. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Encode everything in Rust macros and attributes,** with no Locus syntax: proof types as `proof![x > 0]`, logic functions as `#[logic] fn`. Creusot and Prusti work this way.
- *For:* Locus would need no parser of its own. rust-analyzer, rustfmt, and editors would work on Locus code unchanged.
- *Against:* proofs as values live in signatures, which are what people review, and macro encodings make signatures noisier. Errors inside macro invocations are also reported less precisely.

This RFC instead uses syntax for proofs, logic functions, and propositions, and names only for blocks, derives, and traits.

**A `locus! { ... }` macro inside ordinary `.rs` files,** erasing at build time. It needs no generated files. But a procedural macro sees no types and can only erase, never check, so a crate could be built and published with proofs that were never checked. Generated `.rs` files exist only when the check passed.

**Generate into `OUT_DIR` by default.** This keeps generated code out of the source tree, but everyone who builds the crate, including Rust users depending on it, would need Locus installed. It is an opt-in mode instead.

**`logic struct` and `logic enum` keywords** for logical types. Replaced by `#[derive(Logical)]`, which needs no new syntax.

**A `@{ ... }` logical block.** Replaced by `logic! { ... }`, so that `@` stays reserved for proofs.

**No built-in names,** with v0's `prove!` and `model!` dropped. Rejected: the names are worth having once rustc resolves them with Rust's own shadowing rules.

**Locus's own front end for Rust**, as in v0. Rejected: it reimplements rustc, which is where v0's size and inconsistency came from.

**rust-analyzer's libraries instead of a rustc driver.** These work on stable Rust without `RUSTC_BOOTSTRAP`. But rust-analyzer's type inference is not rustc's, and where they differ, proofs would be about a program other than the one that runs. Its borrow checking is also not rustc's. It remains a candidate for editor support, where speed matters more than authority.

**Charon** exports rustc's MIR in a stable format, and is a candidate source of typed bodies for [#6](https://github.com/nikhilgarg28/locus/issues/6). Run on the checking program, it could also report what the probes ask. It pins its own nightly, and adopting it would make its format part of the trusted path.

**Collapse every logical type into one `Erased` type,** as the first draft did and v0 does. Rejected: macros would see different types in the two programs, distinct types would collide after erasure, and generic parameters could become unused.

**Zero-sized stand-ins for proofs and logical types in both programs,** with one stand-in for every proof. This was the previous draft. Rejected: Rust code that can tell types apart, through `Any` or specialization, could turn one proof into another, and rustc would compute types involving proofs without knowing their propositions. [RFC 0002](0002-logical-layer.md) makes proofs ghost instead.

**One Rust type per proposition,** so that rustc could tell proofs apart. Not possible: propositions mention runtime values, so `@(y != 0)` is a different proposition for each `y`.

**Keep logical and proof fields as zero-sized fields.** Rejected: every such field draws a "never read" lint, derives such as `Debug` fail on it, and it clutters the type's Rust API.

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
- **v0** wrapped Cargo's rustc invocations to capture them, used `RUSTC_BOOTSTRAP` for rustdoc's unstable JSON on stable, guarded its output against a different pointer width, and mapped spans across files with a source bundle.

## Unresolved questions

**Decided by the owner while this RFC was drafted.** Recorded here; accepting the RFC confirms them.
1. Locus syntax only for proofs, logic functions, and propositions. Names for blocks, derives, and traits, resolved like std's prelude.
2. `#[derive(Logical)]`, never implemented by hand, and a `Model` trait, in place of `logic struct` and `logic enum`.
3. The four ways facts enter the logic. `Cell`, `RefCell`, `Mutex`, and the atomics have no model of their contents.
4. The current stable release with `RUSTC_BOOTSTRAP=1` preferred; a pinned nightly acceptable.
5. Generated `.rs` files beside their `.lc` sources by default, with `OUT_DIR` as an opt-in mode.
6. Declared configurations with a guard. The default is dev, release, and test builds with default features and 64-bit pointers, independent of the host. Erasure removes code no checked configuration enables.
7. At the Rust boundary, anything that needs a proof is `unsafe`, with proofs inside data exported as `()`.
8. Proofs and logical values are ghost: neither program contains them ([RFC 0002](0002-logical-layer.md)). This replaces an earlier rule about `'static` bounds.
9. Logical and proof fields are removed from types in both programs.
10. Traits among Locus's names have no methods.
11. The parser starts from a fork of rust-analyzer's. The owner left the choice to this RFC.

**Still for the owner:**
1. `.rs` files may not name logical items, as proposed.
2. Logical code in closures and async blocks, and reads of packed fields, are rejected for now, as proposed.
3. Traits among Locus's names also have no associated functions or constants (an extension of 10 above).
4. Until [#9](https://github.com/nikhilgarg28/locus/issues/9) decides more, a crate whose checked code tests target properties other than pointer width must declare its configurations.

**Left to other RFCs**, each with its issue:
- Logical types, propositions, proofs, models, and contracts: [RFC 0002](0002-logical-layer.md), proposed with this one.
- The remaining syntax of propositions and proofs: [#5](https://github.com/nikhilgarg28/locus/issues/5).
- The runtime semantics: how typed bodies become statements in the logic, THIR or MIR, where borrows end, integer overflow across profiles, and whether observations of a value with proof fields can be narrowed to the fields its propositions read: [#6](https://github.com/nikhilgarg28/locus/issues/6).
- Distribution and the export boundary: `unsafe` exports and how the crate calls them, trait methods, package contents, how configurations are declared, dependency versions, and keeping rustfmt off generated files: [#7](https://github.com/nikhilgarg28/locus/issues/7).
- Logical code in closures and async blocks, in other macros' input and output, and on packed fields: [#8](https://github.com/nikhilgarg28/locus/issues/8).
- Corner cases of `cfg` and compiler flags: [#9](https://github.com/nikhilgarg28/locus/issues/9).
- Editor support, likely a language server that runs rust-analyzer on the checking program and maps results back through origin maps.
- Incremental checking and caching of facts.
