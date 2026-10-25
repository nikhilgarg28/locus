# RFC 0001: The compiler pipeline

- **Status:** proposed
- **Issue:** none
- **Supersedes:** none

## Summary

Locus compiles a Cargo crate in four steps:

1. **Parse.** Locus parses its `.lc` sources, Rust plus Locus syntax, into lossless syntax trees.
2. **Check with rustc.** Locus lowers the crate to a **checking program**: ordinary Rust in which logical and proof types are zero-sized stand-ins, logic items are stubs, and every place logical code reads becomes a borrow. rustc compiles it through a small driver. The driver reports what Locus needs: the types of the places logical code reads, what the names in logical code refer to, and the typed bodies of the functions being verified. rustc's borrow checker decides whether logical code may read what it reads.
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

An adversarial review of the first draft found gaps in configurations, divergence between the checking and runtime programs, observation placement, error attribution, and generated names. This revision addresses them; each is answered below or recorded as an open question.

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
   │ every proof checks
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
- **Each `name.lc` has its runtime program beside it, as `name.rs`.** `cargo locus build` writes it only after the whole crate checks. Its first line says it is generated, names its source, and records the source's hash.
- **Modules work as in Rust.** `mod name;` finds `name.rs` by Rust's usual rules. So plain `cargo build`, rustc, rust-analyzer, docs.rs, and anyone depending on the crate see an ordinary Rust crate. They need neither Locus nor a build script.
- **Generated files are committed and published,** like the output of other code generators. `cargo locus check` fails when a generated file does not match what its source would produce, whether it is stale or was edited by hand. An optional, dependency-free build-script helper can warn during a plain `cargo build` when a `.lc` file has changed since its `.rs` was generated.
- **The stand-ins have one home per crate.** The zero-sized stand-ins that erasure leaves in place of proofs and logical types (see [The runtime program](#the-runtime-program)) are defined once per crate, in a generated module file that the crate root declares.
  - When the root is a `.lc` file, erasure adds the declaration.
  - When the root is plain Rust, the user adds one line, such as `mod locus_erased;`, and `cargo locus check` says exactly which line.
- **The checking program is never committed.** It lives under `target/locus/check/` and mirrors the crate: each `name.lc` lowered, each `.rs` copied.

This is what makes the common case cheap. A crate that is mostly Rust adopts Locus by adding a `.lc` file, its generated `.rs`, and one line in its crate root. Nothing changes for anyone who builds the crate, and the only new command is `cargo locus check` in its CI.

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

**Locus names** are ordinary Rust items in Locus's library:
- the macros `logic! { ... }` for logical blocks, `prop!(...)` for propositions, and `prove!(...)` for proofs;
- the derive `Logical`;
- the trait `Model`;
- the logical types and functions of Locus's standard library, such as `Nat`.

`Logical` can only be derived, never implemented by hand. Deriving it fails unless every field and payload is logical. `Model` may be implemented by hand or derived. What they mean is [#4](https://github.com/nikhilgarg28/locus/issues/4).

How the names are found:
- **They are visible the way std's prelude is.** In the checking program, every module sees a prelude that combines std's with Locus's, so rustc resolves Locus names with Rust's own rules.
- **A program's own definitions and imports take precedence,** exactly as they do over std's prelude: its own `macro_rules! prove`, `use other::prove`, `struct Nat`, or a glob import of a type named `Nat`. All of these were tested.
- **Glob-imported macros are the one exception in Rust's rules.** They collide with prelude macros (E0659), and that is already true of std's `vec!`. When rustc reports such a collision for a Locus name, Locus checks the crate again without that name in its prelude. The program's own macro keeps its Rust meaning, and Locus's version remains reachable by its full path.
- **Recognizing Locus's macros.** Lowering assumes that each invocation of a Locus macro name is Locus's. The driver then reports which definition each invocation actually expanded from, by the definition's identity rather than its path. If an invocation turns out to be the program's own macro, Locus lowers again, treating it as plain Rust. A crate normally needs one rustc run, and needs a second only when names collide.
- **Not inside other macros' input.** A Locus macro invoked inside another macro's input, such as `vec![prove!(...)]`, is rejected for now: Locus cannot lower what it cannot see ([#8](https://github.com/nikhilgarg28/locus/issues/8)).

### How facts enter the logic

There is no wall between Rust and Locus inside a crate. Plain Rust and Locus code mix freely, and every file goes through the pipeline. Soundness comes instead from the only three ways facts about runtime values can enter the logic:

1. **Models:** observing a runtime value through its type's model.
2. **Typed bodies Locus understands:** rustc's typed bodies, translated into the logic.
3. **Assumed specifications:** each declared with a reason, and listed by the audit.

Each is sound no matter who wrote the code.

- **Models read only data that cannot change while the value is shared.** Rust's type system guarantees this, except for the contents of an `UnsafeCell` and the targets of raw pointers.
  - Derived models are built from their fields' models.
  - Hand-written models are logic code, and they can read fields only through those fields' models.
  - Types from the standard library with `unsafe` internals (`Vec`, `Rc`, `String`) get models declared by Locus's library, each as a trusted entry with its reason.
  - `Cell`, `RefCell`, `Mutex`, and the atomics have no model of their contents.

  So **a call Locus knows nothing about, taking `&x`, never changes `x`'s model**: Rust freezes what the model reads, and the model never reads what Rust does not freeze.
- **Plain Rust in a Locus crate is still checked by Locus.** For example, plain Rust that assigns to a field constrained by a proof must re-establish the proof, or the crate fails to check.
- **Code Locus cannot translate is opaque:** dependencies, trait objects, inline assembly, and anything else outside what Locus models. It runs, but its results are known only by their types, and anything it receives through `&mut` may come back as any value of its type.
- **Statics.** Logical code may read only immutable statics whose types have models.

The details of models are [#4](https://github.com/nikhilgarg28/locus/issues/4).

### The checking program

The checking program is what rustc sees.

- **It is written as ordinary `.rs` files** under `target/locus/check/`, and rustc parses them like any crate. Files on disk can be opened, compared, and compiled by hand when something goes wrong. If disk access ever matters, the driver can serve the same text from memory through rustc's file-loader hook, without changing anything else.
- **Lowering keeps each source line on the same line of the checking program where it can,** and appends synthesized items at the end. Raw rustc output is then readable when debugging Locus itself. The origin maps, not the alignment, are the authority.
- **Locus's own machinery lives in Locus's library crate:** the probes, and the stand-ins for the standard library's logical types. rustc receives it under a name the program does not use. Every name Locus generates is chosen so that it appears nowhere in the crate's sources. User code therefore cannot name generated items, and generated items cannot collide with the user's, even under `#![no_implicit_prelude]`.
- **The checking program is compiled with `--cap-lints allow`.** Lints there would be artifacts of the lowering, and a user's `#![deny(warnings)]` or `forbid` must not turn synthesized code into errors. This was tested. Real lints come from building the runtime program.

Logical code is never given to rustc: its operators mean different things from Rust's (`+` on `Nat` cannot overflow), and its types do not fit Rust's (a logical `List` can contain itself without a `Box`). What rustc sees instead:

| In the source | In the checking program |
|---|---|
| Runtime code | Copied unchanged |
| A logical type in a runtime position: a parameter, field, local, or generic argument | Its zero-sized stand-in, with the same name and generic parameters, implementing `Clone` and `Copy` and nothing else |
| A `#[derive(Logical)]` struct or enum | Its stand-in, in place of the definition |
| A proof type, `@P` | The crate's proof stand-in, one zero-sized type implementing `Clone` and `Copy` |
| A named result `(out: T, @P)` | The plain tuple of `T` and the proof stand-in |
| A logic function, `prop` declaration, or `Model` implementation | A stub with the same name, generics, and signature, with stand-ins for logical types and a body that never returns |
| A proof hole, `_` | A stand-in value |
| A `logic!`, `prop!`, or `prove!` invocation, or a proposition in a type annotation inside a body | A block at the same position that observes every place the logical code reads, resolves every name it uses, and yields a stand-in value |
| A proposition in a signature or a struct field's type | Nothing; see below |

Stand-ins with generic parameters carry them in a `PhantomData` field. The probes, `hole`, and stubs are `const fn` where Rust allows, so they work in constant contexts.

**Logical code contains no runtime evaluation.** It reads places, and it never calls runtime functions, mutates, or has effects; the arguments of a logical call are logical code too. A runtime function that returns a proof is runtime code: it is called, and kept, in both programs. So erasing a logical construct never needs to preserve anything it did.

**Observations are how rustc checks what logical code reads.** A place that logical code reads is checked as a shared borrow at the point where the logical code appears, ending immediately. So rustc's borrow checker rejects logical code that reads a value after it has moved, a place while it is mutably borrowed, or through a reference that is no longer live. Observations follow Rust's rules exactly:
- two-phase borrows apply as they would to any shared read;
- as in Rust, code after a `return` is not borrow-checked, and logical code there is never evaluated.

Observations come only from logical code inside bodies:
- **Propositions in signatures and struct field types need none.** A precondition reads parameters at entry, which are always readable. A postcondition is about the function's results and the final values behind its `&mut` parameters, which are well defined however the body exits; it does not read anything at a point in the body. A field's proposition reads the struct's own fields. Their types come from item signatures in the facts.
- **A proposition in a `let` type annotation is evaluated where the `let` is,** and is observed there.
- **Logical code inside a closure body is rejected for now:** an observation there would become a capture, and change what the closure captures.
- **Logical code that reads a packed field is rejected for now:** Rust does not allow a shared borrow of one.

Both cases are [#8](https://github.com/nikhilgarg28/locus/issues/8).

**Probes are how Locus asks rustc questions.** One kind of probe observes a place. Others resolve a name. A name-resolving probe mirrors the way logical code uses the name, with placeholder arguments, so that rustc resolves it in that shape:
- a value;
- a type;
- a function called with its number of arguments;
- a method;
- an item of a trait.

The driver recognizes probes by the identity of their definitions in Locus's library, never by a path written in the source. The exact form of each probe is part of the lowering's specification and is tested against rustc. The spike tested only the simplest form.

The checking program is never shipped.

### Configurations

A check covers one configuration:
- the rustc that Locus is pinned to;
- one target;
- one set of enabled features;
- one set of `cfg` values, including `debug_assertions`;
- the environment that macros such as `env!` and `include!` read;
- the resolved versions of the crate's dependencies.

Code that `cfg` removes in that configuration is not checked.

- **Which configurations are checked.** A crate declares the configurations it is checked for. By default these are the dev and release profiles of the build that ran the check, which differ in `debug_assertions`. `cargo locus check` checks each one.
- **The runtime program refuses to compile in any other configuration.** Each generated file carries a guard that turns any other combination of target, features, and `cfg` values into a `compile_error!` naming the configurations that were checked. v0 guarded its output against a different pointer width the same way.
- **Behavior that differs between profiles.** Integer overflow, for example, is handled by the runtime semantics: a proof must hold in every profile ([#6](https://github.com/nikhilgarg28/locus/issues/6)).
- **What the guard cannot test** becomes an assumption in [the trust model](../TRUST.md): the compiler that builds the runtime program, the versions of dependencies, and the inputs to macros. The distribution RFC ([#7](https://github.com/nikhilgarg28/locus/issues/7)) will narrow these. For example, it may require exact versions for Locus dependencies, or have erasure write fully qualified paths where a newer compiler could resolve a name differently.

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
       /// Text Locus wrote: an observation, a stub, a stand-in.
       Synthesized { generated: TextRange, origin: Origin },
   }

   struct Origin {
       node: SyntaxNodePtr, // the Locus construct that caused the text
       role: Role,          // Observation, Resolution, Stub, StandIn, Guard, ...
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
       macro_origins: Vec<(Span, ItemRef)>,      // the definition each Locus macro name expanded from
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

**Readable by construction.** A `.lc` file with no Locus syntax becomes an identical `.rs` file, apart from its header and guard. Comments, formatting, and names survive everywhere.

**The same stand-ins as the checking program.** The runtime program defines the same zero-sized stand-ins, under the same names, in the crate's generated module (see [Sources and generated files](#sources-and-generated-files)).
- Macros and derives therefore see the same tokens in both programs.
- Distinct logical types stay distinct types, so `impl Tr for Vec<Nat>` and `impl Tr for Vec<Seq<u32>>` do not collide.
- Generic parameters used only by logical fields stay used.
- Imports of stand-ins are inserted where a Locus name resolved to Locus's item. This is safe, because a name resolves to Locus's item only where no definition or import of the program's own is in scope.

**What the edits do:**
- **Removed:** logic functions, `prop` declarations, `Model` implementations, `impl` blocks holding only those, and `use` declarations that name only logical items. Which `use` and `impl` items are logical comes from the facts.
- **Replaced by stand-ins:** `#[derive(Logical)]` types; the contents of `logic!`, `prop!`, `prove!`, and proof holes become stand-in values.
- **Simplified:** named results become plain tuples. Proof parameters stay in signatures as the proof stand-in, so function shapes and call sites do not change.

**Clean output.** The runtime program compiles without warnings whenever the source does. A binding used only by logical code is renamed with a leading underscore, never removed, so its value is dropped at the same point. Building the runtime program is how Locus reports lints and confirms its own output. A warning there is either about the user's code or a Locus bug.

**Source locations.** `file!`, `line!`, `column!`, `Location::caller`, and panic messages name the runtime program's file. Erasure keeps every line on its line number, so only file names and columns differ from the `.lc` source. This is the one stated exception to "erasure never changes what the program does".

**No new borrow errors.** The checking and runtime programs have identical types, and differ only in:
- observations, which are shared borrows ending immediately;
- stubs, which are never called at run time;
- logical constructs, which do no runtime evaluation.

So the runtime program borrow-checks whenever the checking program does. Building it confirms this.

**Proofs cannot be forged from Rust.** The proof stand-in is private to the crate.
- **Across crates, Rust's type privacy stops forgery.** It rejects any expression of the stand-in's type outside the crate, even when the type is inferred or wrapped, as in `Option<Proof>`. A test showed that another crate can neither build a proof nor pass one along.
- **Inside the crate, every file goes through Locus,** and generated names cannot be written in the source.
- **The exported Rust surface contains no logical or proof types:**
  - A function that returns a proof is exported through a wrapper that returns only its data.
  - A function that needs a proof is exported as a `pub unsafe fn` without its proof parameters, with its proposition in its `# Safety` documentation, as Rust's own `get_unchecked` is. A Locus crate that depends on it calls it inside `unsafe`, with a generated `// SAFETY:` comment naming the proof Locus checked. A plain Rust caller can call it too, but only by writing `unsafe` and taking responsibility for the precondition.
  - A field that a proof constrains is not a public Rust field.

  The details are [#7](https://github.com/nikhilgarg28/locus/issues/7).

### Diagnostics

Diagnostics come from five places.

| Source | Code | What the user sees |
|---|---|---|
| Locus's parser | `Lnnnn` | Locus's message |
| rustc on the checking program, every span in copied text | rustc's `Ennnn` | rustc's message, mapped to the source |
| rustc on the checking program, any span in synthesized text | `Lnnnn` | Locus's message, with rustc's code and message as a note |
| Locus's logical pass and kernel | `Lnnnn` | Locus's message |
| rustc on the runtime program | rustc's | Mapped to the source. An error in copied text is the user's, such as a post-monomorphization error that a check run misses. An error in synthesized text is a Locus bug |

- **Classifying by every span.** A rustc error is the user's own only if every span it reports lies in copied text: primary, secondary, labels, and notes. If any lies in synthesized text, the error involves logical code.
  - Locus words it in its own terms, with the synthesized span as the primary span. For example, rustc's E0502 at a user's `x.push(2)`, caused by an observation earlier, becomes "logical code reads `x` while `x` is mutably borrowed", pointing at the logical code, with rustc's code as a note.
  - Some errors at synthesized code have known causes, such as reading a mutable static from logical code. Locus explains these.
  - Anything else at synthesized code is an internal error, never blamed on the user.
- **Mapping.** Locus never shows rustc's rendered text. It reads rustc's structured diagnostics and maps every span through the origin maps, including macro backtraces and suggestions.
  - The two ends of a span map independently, so a span half in copied and half in synthesized text still covers the right source.
  - An end inside synthesized text maps to the edge of the construct that caused it.
  - Spans inside Locus's library map to the invocation that expanded there.
  - A suggestion whose edit lands in copied text is kept and applies to the `.lc` file. One that touches synthesized text is dropped.
- **Wording.** Names of stand-ins and probes in rustc's messages are shown as Locus terms: a proof type, or `Nat`.
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
| `locus-library` | Locus's names, probes, and stand-ins, as Rust | No | Yes: probes are recognized by its definitions |
| `locus-lower` | Checking program and origin maps | No | Yes |
| `locus-logic` | Logical items, typing, elaboration, obligations | No | Obligation generation, as decided by [#6](https://github.com/nikhilgarg28/locus/issues/6) |
| `locus-search` | Proof search | No | No |
| `locus-kernel` | Proof checking | No | Yes |
| `locus-erase` | Runtime program, guards, and origin maps | No | Yes |
| `locus-diagnostics` | Mapping, wording, rendering | No | No |
| `locus` | Command line, Cargo orchestration | No | Yes: it pairs facts with programs and decides when to stop |

### Later: building executables directly

The same driver can compile the runtime program itself, through code generation in the same process. Nothing else in the pipeline changes.

## Rules

The rules below go into the specification with this RFC, in [Compatibility with Rust](../../spec/compatibility.md) and [Observing runtime values](../../spec/observation.md).

| Rule | Examples |
|---|---|
| `compat.superset.same-meaning` | `compatibility/plain-rust.accept.lc`, `compatibility/clone-before-move.accept.lc` |
| `compat.errors.rustc` | `compatibility/use-after-move.reject.lc` |
| `compat.syntax.not-rust` | `compatibility/underscore-in-rust-positions.accept.lc`, `compatibility/at-in-patterns.accept.lc`, `compatibility/tokens-in-macro-input.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.syntax.contextual-keywords` | `compatibility/keywords-as-names.accept.lc`, `compatibility/raw-identifier-logic.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.names.shadowable` | `compatibility/user-macro-named-prove.accept.lc`, `compatibility/glob-imported-macro-named-prove.accept.lc`, `compatibility/user-type-named-nat.accept.lc` |
| `compat.names.not-in-macro-input` | `compatibility/locus-macro-inside-rust-macro.reject.lc` |
| `observe.place.readable` | `observation/after-move.reject.lc`, `observation/during-mutable-borrow.reject.lc`, `observation/proof-after-move.reject.lc` |
| `observe.closures.rejected` | `observation/logic-in-closure.reject.lc` |

The accepted near-misses of the observation rules (`before-move`, `after-mutable-borrow-ends`, `proof-before-move`) are open examples. Whether they are accepted depends on what logical code may read, which is [#4](https://github.com/nikhilgarg28/locus/issues/4). The observation examples use provisional syntax for propositions ([#5](https://github.com/nikhilgarg28/locus/issues/5)); respelling them does not change their outcomes.

## Interactions

| Area | Effect |
|---|---|
| Types | rustc is the authority on every runtime type. Logical types reach rustc only as zero-sized stand-ins, identical in both programs, and Locus types the logical layer itself. |
| Propositions and proofs | rustc never sees a proposition. It sees an observation for each place logical code reads, and a probe for each name it uses. |
| Ownership and mutation | rustc checks all ownership, including what logical code reads. How mutation is modeled in the logic is built from rustc's typed bodies ([#6](https://github.com/nikhilgarg28/locus/issues/6)). |
| Erasure and runtime behavior | Erasure edits the source. Runtime code is untouched, logical code does no runtime evaluation, stand-ins are identical in both programs, and source locations change only in file names and columns. |
| Configurations | A check covers one configuration, and generated code refuses to compile in configurations that were not checked. |
| Rust interoperability | Generated `.rs` files sit beside their `.lc` sources, so the crate builds with plain Cargo. Rust callers cannot forge proofs, and anything that needs a proof is `unsafe` at the Rust boundary ([#7](https://github.com/nikhilgarg28/locus/issues/7)). |
| Diagnostics | Rust's errors keep their codes and messages, with spans mapped to the source. Errors that involve synthesized code are explained in Locus's terms. Lints come from the runtime program. |
| Trust model | rustc's type checker and borrow checker, `locus-driver`, the facts format, Locus's library, the parser, the lowering, erasure, and the orchestration join the trust model, with assumptions about toolchains, dependencies, macro inputs, and procedural macros. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**Encode everything in Rust macros and attributes,** with no Locus syntax: proof types as `proof![x > 0]`, logic functions as `#[logic] fn`. Creusot and Prusti work this way.
- *For:* Locus would need no parser of its own. rust-analyzer, rustfmt, and editors would work on Locus code unchanged.
- *Against:* proofs as values live in signatures, which are what people review, and macro encodings make signatures noisier. Errors inside macro invocations are also reported less precisely.

This RFC instead uses syntax for proofs, logic functions, and propositions, and names only for blocks, derives, and traits.

**A `locus! { ... }` macro inside ordinary `.rs` files,** erasing at build time. It needs no generated files. But a procedural macro sees no types and can only erase, never check, so a crate could be built and published with proofs that were never checked. Generated `.rs` files exist only when the check passed.

**Generate the runtime program into `OUT_DIR` from a build script.** This keeps generated code out of the source tree, but everyone who builds the crate, including Rust users depending on it, would need Locus installed.

**`logic struct` and `logic enum` keywords** for logical types. Replaced by `#[derive(Logical)]`, which needs no new syntax.

**A `@{ ... }` logical block.** Replaced by `logic! { ... }`, so that `@` stays reserved for proofs.

**No built-in names,** with v0's `prove!` and `model!` dropped. Rejected: the names are worth having once rustc resolves them with Rust's own shadowing rules.

**Locus's own front end for Rust**, as in v0. Rejected: it reimplements rustc, which is where v0's size and inconsistency came from.

**rust-analyzer's libraries instead of a rustc driver.** These work on stable Rust without `RUSTC_BOOTSTRAP`. But rust-analyzer's type inference is not rustc's, and where they differ, proofs would be about a program other than the one that runs. Its borrow checking is also not rustc's. It remains a candidate for editor support, where speed matters more than authority.

**Charon** exports rustc's MIR in a stable format, and is a candidate source of typed bodies for [#6](https://github.com/nikhilgarg28/locus/issues/6). Run on the checking program, it could also report what the probes ask. It pins its own nightly, and adopting it would make its format part of the trusted path.

**Collapse every logical type into one `Erased` type,** as the first draft did. Rejected: macros would see different types in the two programs, distinct types would collide after erasure, and generic parameters could become unused.

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

**Decisions for the owner:**
1. Locus syntax for proofs, logic functions, and propositions, with names for blocks, derives, and traits, as proposed; or everything as macros and attributes.
2. Fork rust-analyzer's parser, as proposed, or write a new one.
3. Pin the latest stable release with `RUSTC_BOOTSTRAP=1`, as proposed, or a nightly.
4. Generated `.rs` files beside their `.lc` sources, committed and published, as proposed; or generation from a build script, or a `locus!` macro.
5. Check declared configurations and refuse to compile others, as proposed.
6. At the Rust boundary, anything that needs a proof is `unsafe`, as proposed.
7. `.rs` files may not name logical items, as proposed.
8. Logical code inside closures and packed-field reads are rejected for now, as proposed.

**Left to later RFCs**, each with its issue:
- Logical types and models, including what logical code may read: [#4](https://github.com/nikhilgarg28/locus/issues/4).
- The syntax of propositions and proofs: [#5](https://github.com/nikhilgarg28/locus/issues/5).
- The runtime semantics: how typed bodies become statements in the logic, THIR or MIR, and integer overflow across profiles: [#6](https://github.com/nikhilgarg28/locus/issues/6).
- Distribution and the export boundary: wrappers, `unsafe` exports, package contents, how configurations are declared, and dependency versions: [#7](https://github.com/nikhilgarg28/locus/issues/7).
- Logical code in closures, in other macros' input, and on packed fields: [#8](https://github.com/nikhilgarg28/locus/issues/8).
- Editor support, likely a language server that runs rust-analyzer on the checking program and maps results back through origin maps.
- Incremental checking and caching of facts.
