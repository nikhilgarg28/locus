# RFC 0001: The compiler pipeline

- **Status:** proposed
- **Issue:** none
- **Supersedes:** none

## Summary

Locus compiles a Cargo crate in four steps:

1. **Parse.** Locus parses its sources, Rust plus Locus syntax, into lossless syntax trees.
2. **Check with rustc.** Locus lowers the crate to a **checking program**: ordinary Rust in which logical types become zero-sized placeholders, logic items become stubs, and every place a proof reads becomes a borrow. rustc compiles it through a small driver, which reports back what Locus needs: the types of the places proofs read, what the names in logical code refer to, and the typed bodies of the functions being verified. rustc's borrow checker decides whether each proof may read what it reads.
3. **Check the logic.** Locus checks the logical layer against those facts, constructs proofs, and has the kernel check them.
4. **Emit.** Locus erases the logical layer by editing the source text into the **runtime program**, readable Rust that Cargo builds like any other crate.

Locus writes its own parser, the lowering, the logical layer, the kernel, erasure, and diagnostics. Everything else that Rust already does is rustc's: name resolution, macro expansion, type inference, trait resolution, ownership and borrow checking, and code generation.

## Motivation

The [vision](../../VISION.md) commits Locus to being a superset of Rust in which "rustc does Rust's work". v0 did the opposite. It had its own front end for a growing subset of Rust, reimplementing moves, borrows, traits, and generics, and most of its 71,000 lines went there. This RFC decides how the work is split instead, which representations exist, how information flows between Locus and rustc, and how rustc's errors reach the user.

It settles two of the vision's foundational questions:
- **Type information:** where Locus gets Rust's types and resolutions.
- **Ownership of logical code:** how proofs, which rustc never runs, are kept from reading values they may not read.

It also fixes the rules that any Locus syntax must follow, which the syntax RFC will work within.

A [spike](../../spikes/rustc-facts/README.md) tested the central mechanism before this RFC was written. A hand-written checking program, compiled by a 150-line rustc driver running as Cargo's wrapper, reported:
- the types of the places its proofs read, including a field of a type from a dependency;
- what the paths in its proofs resolve to;
- the typed body of a function, with indexing resolved to `<Vec<u32> as Index<usize>>::index`.

rustc's borrow checker rejected the one proof that read a moved value, with E0382 on the matching line. The driver built against stable rustc 1.97 (with `RUSTC_BOOTSTRAP=1`) and against a nightly four releases later, needing a one-line change between them.

## Design

### The pipeline

```text
 crate sources (.rs, .lc)
   │
   │ parse ............................................. Locus
   ▼
 syntax trees (lossless, one per file)
   │
   │ lower ............................................. Locus
   ├──────────────► checking program + origin map
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
 erase: edit the syntax trees ............................. Locus
   │
   ▼
 runtime program + origin map ─► cargo build ............... rustc
```

### Who does what

| Job | Done by |
|---|---|
| Parsing Rust and Locus syntax | Locus, with a lossless parser |
| Module tree, name resolution, imports, visibility | rustc |
| Rust macro expansion, including procedural macros | rustc |
| Type inference, trait and method resolution, coercions, auto-dereferencing | rustc |
| Ownership and borrow checking, including what proofs read | rustc |
| Checking logical types, logic functions, propositions, and models | Locus |
| The meaning of runtime code in the logic | Locus, from rustc's typed bodies; the semantics are a later RFC |
| Proof search | Locus, untrusted |
| Proof checking | Locus's kernel, trusted |
| Erasure, and emitting readable Rust | Locus |
| Monomorphization, code generation, linking | rustc |
| Diagnostics: rendering, and mapping rustc's back to the source | Locus |

### Sources

Locus compiles one Cargo crate at a time, in the crate's real Cargo context: its edition, features, `cfg`s, and dependencies.

- **`.rs` and `.lc` files.** A crate's `.rs` files are plain Rust. Its `.lc` files may also contain Locus syntax. Renaming a `.rs` file to `.lc` changes nothing about the program.
- **Locus syntax is recognized only in `.lc` files.** Plain Cargo and Rust tooling never see it.
- **Every file goes through the pipeline,** because plain Rust code may still call logical functions or carry proofs.
- **Modules are resolved by rustc.** The checking program and the runtime program mirror the source tree file for file: each `name.lc` becomes `name.rs` and each `.rs` file is carried over. So `mod name;` finds its file by Rust's usual rules, and Locus never resolves modules itself.

Where the `.lc` sources and the generated `.rs` files live in a published package belongs to distribution, a later RFC. This RFC requires only that the runtime program be a Rust source tree with the crate's module structure.

### Parsing

- **Locus parses `.lc` files into lossless syntax trees,** in which every byte, including comments and whitespace, belongs to a node. The trees use [rowan](https://github.com/rust-analyzer/rowan), the tree library behind rust-analyzer. Lossless trees are what make erasure a matter of editing the source (see [The runtime program](#the-runtime-program)).
- **The grammar starts from rust-analyzer's parser,** forked, with Locus's productions added. Being a superset means parsing all of Rust's syntax from the first day. rust-analyzer's parser already does, tolerates errors, and follows Rust's grammar as it changes. The cost is a fork to keep in step with upstream.
- **A mechanical test keeps the parser honest:** every file in rustc's own test suite that rustc parses must also parse with Locus, and print back byte for byte.
- **Rust macro invocations stay token trees,** as in Rust. Locus syntax inside a Rust macro invocation is not recognized, and Locus syntax cannot be produced by a Rust macro's expansion.

### What Locus syntax may look like

The syntax itself is the subject of its own RFC. This RFC fixes the rules that guarantee Locus never changes what a Rust program means, and the initial set of forms the parser must support.

**Rules:**

1. **Locus syntax is recognized from the tokens alone,** without resolving any name, and only where those tokens are not valid Rust. No Rust program can be read as Locus.
2. **Locus keywords are contextual.** `logic` and `prop` are keywords only in the item positions below; everywhere else they are ordinary identifiers.
3. **Locus has no built-in macros.** A Rust program can define its own `prove!` or `model!`, so recognizing such a name as Locus syntax would need name resolution and could change the meaning of a Rust program. Operations that v0 spelled as macros become either syntax that is not valid Rust (below) or ordinary logical functions in the standard library, resolved like any other item.

**Initial forms**, with provisional spellings:

| Form | Where | Example | Why Rust cannot mean it |
|---|---|---|---|
| Logic function | Item | `logic fn total(xs: &[u32]) -> Nat { ... }` | An identifier cannot precede `fn` |
| Logical type | Item | `logic struct Range { lo: Nat, hi: Nat }`, `logic enum List<T> { Nil, Cons(T, List<T>) }` | An identifier cannot precede `struct` or `enum` |
| Proposition declaration | Item | `prop Sorted(xs: Seq<u32>) { ... }` | `prop Name(` is not a Rust item |
| Proof type | Type | `h: @(x > 0)`, `@Sorted(xs)` | `@` cannot begin a type |
| Named result | Return type | `-> (out: u8, @(out > n))` | Tuple types have no field names |
| Proof hole | Expression | `(out, _)` | `_` is an expression only as the target of an assignment |
| Logical block | Expression | `let n = @{ total(&v) + 1 };` | `@` cannot begin an expression |

v0's `logic { ... }` block cannot survive: `logic { x }` is already a valid Rust struct literal.

### The checking program

The checking program is what rustc sees. It is ordinary Rust, generated from the syntax trees, in which:
- runtime code is unchanged;
- the logical layer is reduced to what rustc needs to type-check and borrow-check the runtime code around it.

Logical code is never given to rustc. Its operators mean different things from Rust's (`+` on `Nat` cannot overflow), and its types do not fit Rust's (a logical `List` can contain itself without a `Box`). Locus checks it.

| In the source | In the checking program |
|---|---|
| Runtime code | Copied unchanged |
| A logical type (`Nat`, `Seq<T>`, a `logic struct`) | A zero-sized, `Copy` stand-in with the same name and generic parameters, and no fields or variants |
| A proof type, `@P` | One zero-sized placeholder, `__locus::Proof`. Propositions are not types rustc can see |
| A named result `(out: T, @P)` | The plain tuple `(T, __locus::Proof)` |
| A logic function or `prop` declaration | A stub with the same name, generics, and signature, with placeholders for logical types and a body that never returns |
| A proof hole, `_` | `__locus::hole()`, which returns a placeholder |
| A logical block, or a proof expression inside runtime code | A block that *observes* every place it reads (`__locus::observe(&place)`), *resolves* every path it names (`__locus::resolve(path)`), and yields a placeholder |
| A proposition in a signature | A prologue at the start of the body that observes the places the proposition reads |

**Observations are how rustc checks ownership for proofs.** A proof that reads a place is checked as a shared borrow of that place at the point where the proof appears, ending immediately.

So rustc's own borrow checker rejects a proof that reads:
- a value after it has moved;
- a place while it is mutably borrowed;
- through a reference that is no longer live.

Locus needs no permission checker of its own. The spike showed rustc rejecting exactly this case, with E0382, on the right line.

Exactly where a proposition is evaluated is part of the runtime semantics, a later RFC. Preconditions are evaluated at entry; postconditions that read `&mut` parameters must be evaluated at each return. Observations go wherever that RFC says propositions are evaluated.

**Probes are how Locus asks rustc questions.** `__locus::observe` and `__locus::resolve` are the only two probes, and the driver recognizes them by resolving them. It reads:
- **from an observation:** the type of the observed place;
- **from a resolution:** the item the path refers to, whether in the crate, a dependency, or the standard library.

Paths in logical code are therefore resolved by rustc, with Rust's rules for modules, imports, visibility, and shadowing, because each logical item has a stub in the checking program.

The checking program is never shipped. Keeping it separate from the runtime program lets it carry probes and stubs that would make the shipped Rust harder to read.

### Running rustc: `locus-driver`

`locus-driver` is a small program linked against rustc's internal crates, the way Clippy is. Cargo runs it in place of rustc through `RUSTC_WORKSPACE_WRAPPER`.

- **Crates that Locus checks:** the driver substitutes the checking program's root for the crate's root, records facts after type checking, and lets rustc continue so that the borrow checker runs. It stops before code generation, like `cargo check`.
- **Every other crate,** including dependencies: it behaves exactly like rustc.

Cargo does the orchestration, as for any build. Locus gets the exact compiler invocation, with its edition, `cfg`s, features, and `--extern` paths.

**Only `locus-driver` touches rustc's internals.** It writes facts in a format Locus owns and versions; the rest of Locus reads that format, builds on stable Rust, and never links rustc. This keeps rustc's API churn in one small crate, which the spike measured at one renamed function across four releases for its surface. It also lets another producer, such as rust-analyzer or Charon, replace the driver later without touching anything else.

**Toolchain.** The driver is built for one pinned rustc release. The preference is the current stable release, built with `RUSTC_BOOTSTRAP=1`, which the spike showed works. The fallback is a pinned nightly, if a needed internal API exists only there. Either way:
- `locus check` needs that one toolchain, which Locus installs.
- The runtime program is plain Rust that any stable compiler supporting the crate's edition builds.
- Locus follows stable releases, so the pinned compiler is at most one release behind.

The Rust project does not support `RUSTC_BOOTSTRAP` outside its own builds. In practice it ties the driver to one exact release, just as a nightly pin does.

### Data structures

Each stage has one representation, and each representation has one owner.

1. **Source database.** Files by `FileId`, with their path and text. A `Span` is a `FileId` and a byte range.
2. **Syntax trees.** One lossless rowan tree per file. Its node kinds are rust-analyzer's, plus one kind per Locus form. Typed wrappers give the AST view.
3. **Origin maps.** For every generated file, checking or runtime, an ordered list of segments covering the whole file:

   ```rust
   enum Segment {
       /// Text copied from the source unchanged: offsets translate one to one.
       Copied { generated: TextRange, source: Span },
       /// Text Locus wrote: an observation, a stub, a placeholder.
       Synthesized { generated: TextRange, origin: Origin },
   }

   struct Origin {
       node: SyntaxNodePtr, // the Locus construct that caused the text
       role: Role,          // Observation, Resolution, Stub, Placeholder, Hole, ...
   }
   ```

   Every span rustc reports passes through an origin map before a user sees it.
4. **Facts.** What the driver reports, in Locus's own types rather than rustc's:

   ```rust
   struct Facts {
       version: u32,
       observations: Vec<(ProbeSpan, Ty)>,       // the type of each observed place
       resolutions: Vec<(ProbeSpan, ItemRef)>,   // the item each path names
       items: Map<ItemRef, ItemInfo>,            // signatures, fields, variants, generics, bounds
       bodies: Map<ItemRef, TypedBody>,          // typed bodies of the functions being verified
   }
   ```

   - `Ty` and `ItemRef` describe Rust types and items by path and generic arguments, independently of rustc.
   - `TypedBody` is derived from rustc's THIR: every dereference, borrow, coercion, and call made explicit, with trait implementations resolved. The spike printed one.
   - Facts are serialized, as JSON at first, so that the driver and the rest of Locus can run as separate processes.
5. **Logical IR.** Locus's typed representation of logical items, propositions, and proofs. It refers to runtime types and items only through `Ty` and `ItemRef` from the facts.
6. **Kernel terms and obligations.** Defined by the kernel RFC.
7. **Edits.** The runtime program is a list of `Edit { range, replacement }` applied to each source file, with an origin map like the checking program's.

### The runtime program

Erasure edits the source text; it does not print a new program. Locus syntax is removed or replaced, and every other byte is kept.

- **Readable by construction.** A `.lc` file with no Locus syntax becomes an identical `.rs` file. Comments, formatting, and names survive everywhere.
- **What the edits do:**
  - Logical items disappear.
  - Logical and proof types in runtime positions become `Erased`: one private, zero-sized type per crate, with no public constructor.
  - Proof expressions become `Erased`. Any runtime evaluation inside them is kept: an ordinary call in an argument still runs, in the same order.
  - Named results become plain tuples.
  - Proof parameters stay in signatures as `Erased`, so that function shapes and call sites do not change.
- **Clean output.** The runtime program compiles without warnings whenever the source does. A binding used only by proofs is removed, or renamed with a leading underscore.
- **No new borrow errors.** Erasure only removes shared borrows that end immediately, zero-sized values, and stubs, so the runtime program borrow-checks whenever the checking program does.
- **Confirmed by compiling.** Locus's own tests build the runtime program with rustc, and `locus check` can be asked to. A failure there is a Locus bug.

### Diagnostics

Diagnostics come from four places, and each is handled differently.

| Source | Code | What the user sees |
|---|---|---|
| Locus's parser | `Lnnnn` | Locus's message |
| rustc, on the checking program, at user code | rustc's `Ennnn` | rustc's message, mapped to the source |
| rustc, on the checking program, at synthesized code | `Lnnnn` | Locus's message, with rustc's code and message as a note |
| Locus's logical pass and kernel | `Lnnnn` | Locus's message |
| rustc, on the runtime program | — | An internal error: this is a Locus bug |

- **Mapping.** Every span in a rustc diagnostic, including labels, notes, and suggestions, goes through the origin map:
  - A span in copied text maps to the exact source span.
  - A span in synthesized text maps to the construct that caused it, such as the place inside a proposition.
  - A suggestion whose span is synthesized is dropped, because it would edit code the user did not write.
- **Rust's errors stay Rust's.** An error whose primary span is in the user's code is reported with rustc's code and message, so a Rust program's errors read as rustc reports them. Placeholder names are shown as Locus types: `__locus::Proof` as a proof type, `__locus::Nat` as `Nat`.
- **Errors at synthesized code are explained in Locus's terms.** For example, rustc's E0382 at an observation becomes "this proof reads `v` after `v` was moved", with a note "rustc: E0382, borrow of moved value". An error at synthesized code that Locus does not recognize is reported as an internal error, never blamed on the user.
- **Warnings.** Warnings at the user's code are forwarded; warnings caused only by synthesized code are dropped. The spike showed both kinds.
- **Stopping.** If rustc rejects the checking program, Locus reports rustc's errors and stops before the logical pass, which needs rustc's facts.
- **Rendering and data.** Locus renders every diagnostic in rustc's layout, and offers the same diagnostics as JSON, with a field for rustc's code. `locus explain Ennnn` defers to `rustc --explain`.
- **Litmus examples.** Annotations will accept rustc's codes, such as `//~ error[E0382]`, once this RFC is accepted. Until then, examples of rustc's errors give the reason in words.

### Components

The pipeline suggests these crates, each with one job. They would be created by build tasks after this RFC is accepted.

| Crate | Job | Links rustc's internals | Trusted |
|---|---|---|---|
| `locus-syntax` | Lexer, parser, lossless trees | No | Yes: it decides where Locus syntax begins and ends |
| `locus-facts` | The facts format and its serialization | No | Yes |
| `locus-driver` | The rustc driver that reports facts | **Yes, alone** | Yes |
| `locus-lower` | Checking program and origin maps | No | Yes |
| `locus-logic` | Logical items, typing, elaboration, obligations | No | Obligation generation only |
| `locus-search` | Proof search | No | No |
| `locus-kernel` | Proof checking | No | Yes |
| `locus-erase` | Runtime program and origin maps | No | Yes |
| `locus-diagnostics` | Mapping, wording, rendering | No | No |
| `locus` | Command line, Cargo orchestration | No | No |

### Later: building executables directly

The same driver can compile the runtime program itself, through code generation in the same process, instead of writing Rust files for Cargo. Nothing else in the pipeline changes.

## Rules

The rules below go into the specification with this RFC, in [Compatibility with Rust](../../spec/compatibility.md) and [Observing runtime values](../../spec/observation.md).

| Rule | Examples |
|---|---|
| `compat.superset.same-meaning` | `litmus/compatibility/plain-rust.accept.lc`, `litmus/compatibility/clone-before-move.accept.lc` |
| `compat.errors.rustc` | `litmus/compatibility/use-after-move.reject.lc`, `litmus/compatibility/clone-before-move.accept.lc` |
| `compat.syntax.not-rust` | `litmus/compatibility/logic-block-is-a-struct-literal.reject.lc`, `litmus/compatibility/keywords-as-names.accept.lc` |
| `compat.syntax.contextual-keywords` | `litmus/compatibility/keywords-as-names.accept.lc`, `litmus/compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.macros.none-built-in` | `litmus/compatibility/user-macro-named-prove.accept.lc`, `litmus/compatibility/prove-is-not-built-in.reject.lc` |
| `observe.place.readable` | `litmus/observation/before-move.accept.lc`, `litmus/observation/after-move.reject.lc`, `litmus/observation/after-mutable-borrow-ends.accept.lc`, `litmus/observation/during-mutable-borrow.reject.lc` |

The observation examples use provisional syntax for proof types and propositions. The syntax RFC may respell them without changing their outcomes.

## Interactions

| Area | Effect |
|---|---|
| Types | rustc is the authority on every runtime type. Logical types reach rustc only as zero-sized stand-ins, and Locus types the logical layer itself. |
| Propositions and proofs | rustc never sees a proposition. It sees an observation for each place a proof reads, and a resolution for each item it names. |
| Ownership and mutation | rustc checks all ownership, including what proofs read. How mutation is modeled in the logic is built from rustc's typed bodies and is decided by the runtime-semantics RFC. |
| Erasure and runtime behavior | Erasure edits the source. Runtime code is untouched, proofs become one zero-sized type, and runtime evaluation inside proof expressions is kept in order. |
| Rust interoperability | Crates build through Cargo as usual, with dependencies compiled by rustc unchanged. How a Locus crate carries logical interfaces to the Locus crates that depend on it is decided by the distribution RFC. |
| Diagnostics | Rust's errors keep their codes and messages, with spans mapped to the source. Errors caused by synthesized code are explained in Locus's terms. |
| Trust model | rustc's type checker and borrow checker, `locus-driver`, the parser, the lowering, and erasure join the trust model. See [design/TRUST.md](../TRUST.md). |

## Alternatives

**No Locus syntax: encode everything in Rust macros and attributes.** Proof types would be `proof![x > 0]`, logic functions `#[logic] fn`, proof holes `prove!()`. Creusot and Prusti work this way.
- *For:* Locus would need no parser for Rust at all. rust-analyzer, rustfmt, and editors would work on Locus code unchanged. rustc would report errors directly on the user's files, so no mapping would be needed.
- *Against:*
  - Proofs as first-class values live in signatures, which are what people review, and macro encodings make signatures noisier: named results become `-> proven![(out: u8), out > n]`.
  - Macro names can be shadowed, so the macros must be namespaced and imported.
  - Errors inside macro invocations are reported less precisely.
  - The vision describes Locus syntax.

This is the main alternative, and choosing between it and this RFC is the first decision this RFC asks for.

**Locus's own front end for Rust**, as in v0. Rejected: it reimplements rustc, which is where v0's size and inconsistency came from.

**rust-analyzer's libraries instead of a rustc driver.** These work on stable Rust without `RUSTC_BOOTSTRAP`. But rust-analyzer's type inference is not rustc's, and where they differ, proofs would be about a program other than the one that runs. It also has no borrow checker, so Locus would need its own permission checker for proofs, which is v0 again. It remains a candidate for editor support, where speed matters more than authority.

**Charon** exports rustc's MIR in a stable format, and is a candidate source of typed bodies for the runtime-semantics RFC. It does not answer the questions probes answer, because it reports bodies, not the types of places in proofs or the resolution of paths in them. It also pins its own nightly.

**One program instead of two**, shipping observations as calls to empty functions. Rejected: the shipped Rust would be littered with probes and stubs.

**Give rustc the logical code too**, as real Rust code over placeholder types with operator implementations, close to what Verus does with its proof code. Rejected: logical operators would type-check with Rust's meaning (`x + 1` on a `u8` is not `x + 1` on a `Nat`), and logical types such as recursive enums without `Box` do not fit Rust's types. Locus would have to override rustc's answers in exactly the places it relies on them.

**Write a new Rust parser** instead of forking rust-analyzer's. It would give Locus full control and cleaner code, at the cost of reaching all of Rust's syntax before anything else works. The mechanical test against rustc's test suite applies either way.

**Allow Locus syntax in `.rs` files.** Rejected: plain Cargo and Rust editors would fail on those files, and a file's extension would no longer tell a reader what language it is in.

## Prior art

- **Clippy** is a rustc driver run through `RUSTC_WORKSPACE_WRAPPER`, the model for `locus-driver`.
- **Verus** checks the ownership of its proof code by generating a separate Rust program for rustc's borrow checker, a close cousin of the checking program. It reads rustc's internals directly throughout, where Locus confines that to one crate.
- **Creusot, Prusti, Kani, and Flux** are rustc drivers pinned to a nightly. Creusot and Prusti erase their contracts in ordinary builds.
- **Charon**, used by Aeneas, exports what rustc knows in a format of its own, separating the part that depends on rustc from the analysis. `locus-facts` follows the same separation.
- **rust-analyzer** provides the lossless, error-tolerant parser and the rowan trees this design starts from.
- **TypeScript-based tools for Svelte and Vue** check generated code and map diagnostics back through source maps, as the origin maps do here.
- **v0** wrapped Cargo's rustc invocations to capture them, used `RUSTC_BOOTSTRAP` for rustdoc's unstable JSON on stable, and mapped spans across files with a source bundle.

## Unresolved questions

These are the decisions this RFC asks for, and the questions it leaves to later RFCs.

**Decisions for the owner:**
1. Locus syntax with its own parser, as proposed, or the macro and attribute encoding.
2. Fork rust-analyzer's parser, as proposed, or write a new one.
3. Pin the latest stable release with `RUSTC_BOOTSTRAP=1`, as proposed, or a nightly.
4. No built-in macros, so v0's `prove!`, `prop!`, `model!`, and `old!` do not return as macros.
5. Locus syntax only in `.lc` files.
6. Proof parameters stay in runtime signatures as `Erased`, rather than being removed.

**Left to later RFCs:**
- The syntax of propositions and proofs, within the rules above.
- The runtime semantics: how typed bodies become statements in the logic, where propositions are evaluated, and whether THIR or MIR is the better source.
- Distribution: package layout, what is published, and how Locus crates find each other's logical interfaces.
- Locus syntax inside Rust macro invocations, and macros that produce Locus syntax.
- Editor support, likely a language server that runs rust-analyzer on the checking program and maps results back through origin maps.
- Incremental checking and caching of facts.
