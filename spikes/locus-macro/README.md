# Spike: `locus!`, a function-like macro for macro-first Locus

A throwaway experiment for the macro-first plan: before the full toolchain of [RFC 0001](../../design/rfcs/0001-compiler-pipeline.md), Locus code sits in `locus! { ... }` blocks inside ordinary `.rs` files of an ordinary Cargo crate. Stock `cargo build` gives the runtime expansion; `--cfg __locus_check` gives the checking expansion, whose facts a checker reads later. Nothing outside `spikes/` may depend on this spike, and it depends on no other spike: what it reuses from [`one-text-lowering`](../one-text-lowering/README.md) is copied.

## Question

1. **Does the surface syntax survive as macro input?** Does every `litmus/**/*.lc` file's full text, comments included, tokenize as a Rust token stream, with proc_macro2's lexer and with rustc's own, which is what a `locus!` invocation in a real crate gets?
2. **Two expansions from one macro.** Can a toy `locus!` turn representative litmus examples into Rust for both `cfg`s, following the one-text-lowering rules? Do they compile both ways with stock rustc 1.97 in a Cargo crate, and do the accepted ones produce their `run` values? What did the macro need that lowering did not, such as type information or state from another invocation?
3. **Diagnostics.** Does rustc's error for a plain Rust type error inside a `locus!` block point at the user's line and column, in both expansions?
4. **Tooling.** What does `cargo fmt` do with `locus!` blocks? Does rust-analyzer resolve the items and types inside them?
5. **Visibility across crates.** With `lib_a` defining `pub fn divide(x: u32, y: u32, nonzero: @(y != 0)) -> u32` in a `locus!` block, can plain-Rust `user_b` call it in the runtime expansion without a proof, when (i) the proof parameter becomes the marker `Erased`, or (ii) it is removed? What does each mean for a second Locus crate calling `divide` from its own `locus!` block?

## Setup

A Cargo workspace of its own (`Cargo.toml` here has a `[workspace]` table), so the repository's root workspace is untouched. All outputs go under `target/`, which `.gitignore` excludes.

- `locus/` is the library, crate `locus`, adapted from the earlier spike's `library/locus.rs`:
  - the markers `Erased` (all eight standard derives but `Default`) and `LogicalData` (`Clone`, `Copy`, `Debug`), the projection trait `Proj`, the aliases `Int`, `Nat`, `Prop` and `Seq<T>`, and `observe`, `probe` and `stub`;
  - `Proof![P]`, a type macro that is `Erased`;
  - `proof!`, `data!` and `observe!`, which take `[&place, ...] [item, ...] logical code`. Unlike the earlier spike's library, which had a checking and a runtime version, nothing in this one depends on the `cfg`: these macros put `#[cfg(__locus_check)] { observe(..); probe(..); }` in their *output*, so the `cfg` is evaluated in the crate where they expand. (Cargo still builds it once per target directory, since `RUSTFLAGS` changes every crate's fingerprint.) The library re-exports `locus!`, `prove!`, `logic!` and `prop!` from `locus-macros`, and has a `prelude` module with the aliases and those three macros.
- `locus-macros/` is the proc-macro crate. Nothing in it reads a `cfg`: `locus!` writes one text, the same under both `cfg`s.
  - `locus!` is a token rewriter with heuristics, about 1,400 lines after rustfmt, not Locus's parser. It follows the one-text-lowering rules:

    | Source | What `locus!` writes |
    |---|---|
    | `@P` in a type | `::locus::Proof![P]` |
    | Named tuple components `(out: T, @P)` in a type | `(T, ::locus::Proof![P])` |
    | `_` in an expression | `::locus::proof!([] [])`. Patterns, `let` patterns, match arms, closure parameters, `_ = ..` and `(a, _) = ..`, turbofish and `as` types, and `[x; _]` are left alone |
    | `prove!`, `logic!`, `prop!` written in a body | `::locus::proof!(..)` or `::locus::data!(..)`, with the places the logical code reads and the functions it calls. A ghost root is observed whole; binders of logical closures (`\|n: Int\|`) are not places |
    | A proposition in a `let` annotation | `::locus::observe!(&places);` after the `let` |
    | A ghost parameter or `let` | Marked `#[allow(unused_variables)]` for the runtime lints |
    | A proof field | Its marker, marked `#[allow(dead_code)]` |
    | `logic fn` | A stub function whose body is `::locus::stub()` |
    | `prop Name(..) { Arm(..) => {..} .. }` | A stub function `Name`, a field-less struct `Name {}` in the type namespace only, and a stub function for each arm |
    | `#[derive(Logical)]` type | A stub struct `__locus_Name`, its `Proj` impl, the alias `type Name = <__locus_Name as Proj>::Out`, and a stub function per variant; an `impl Name` in the same invocation becomes `impl __locus_Name` |
    | `pub(locus)` on an item or a named field | `pub`. In a tuple struct, `pub (locus)` keeps its Rust meaning |
    | Another macro's input, including `macro_rules!` | Unchanged |
    | Every block | Ends with `#[allow(unused_imports)] use ::locus::prelude::*;`, standing in for lowering's `prelude_import` |

    Copied tokens keep their spans; a token `locus!` writes takes the span of the source token it replaces. `LOCUS_SPIKE_EXPANSION=dir` writes each expansion to a file (`./run.sh expansions` shows them formatted).
  - `prove!`, `logic!` and `prop!` are for invocations that `locus!` leaves to rustc, inside another macro's input. They observe every root precisely, since the toy has no table of ghost roots.
  - `__lexdump!` writes the tokens rustc gave it, for question 1.
  - Two spike-only directives select question 5's strategies: `#![spike(strip_proof_params)]` puts `#[cfg(__locus_check)]` on each proof parameter, so the runtime signature has none; `#![spike(drop_hole_args)]` writes a call with a hole argument as two `cfg`-gated `let`s, one without the hole.
- **Question 1.** `tokenize/` runs `proc_macro2::TokenStream::from_str` (proc_macro2 1.0.107, its fallback lexer, since it is not a proc macro) on each file and writes a canonical dump of the tokens. `run.sh` then wraps each file's full text in `locus_macros::__lexdump! { "out"; <text> }` and compiles it with `rustc --edition 2024 --emit metadata`, which writes the tokens rustc's lexer gave the macro, and `compare-dumps.py` compares the two dumps. `tokenize/edge/` holds 24 snippets that are not in the corpus.
- **Question 2.** `cases/` is an ordinary Cargo package with one binary per example. `gen-cases.py` writes `cases/src/bin/<name>.rs` from 22 litmus examples: the full text, byte for byte, inside `locus! { }`, after a 7-line header, and a `main` outside the block that prints which expansion it was built with and each `//~ run:` call as `<call> => <Debug value>`. `gen-cases.py --check` confirms the copies. Four files are not litmus examples: `x-locus-name-collision.rs`, `x-split-logical-type.rs`, `x-joined-logical-type.rs` and `x-logic-kind.rs`.
- **Question 3.** `diag/src/bin/`: `plain-error.rs`, `locus-position-error.rs` and `check-only-error.rs`.
- **Question 4.** `diag/src/bin/fmt-probe.rs` is misformatted the same way inside and outside its `locus!` block. `diag/src/bin/plain-array-infer.rs` is plain Rust, for comparison with one rust-analyzer result.
- **Question 5.** `cross/lib_a_marker` (strategy i) and `cross/lib_a_removed` (strategy ii) each define `divide` and `check_nonzero(y: u32) -> @(y != 0)`, a runtime check that panics on zero. `cross/user_b` is plain Rust, with no `locus!`. `cross/user_c` calls `divide` from its own `locus!` blocks.
- Every crate whose code expands Locus's macros declares `check-cfg = ['cfg(__locus_check)']` through the workspace's `[lints]`, except `cross/user_c`, which shows what happens without it.
- `run.sh [all|q1|q2|q3|q4|q5|expansions]` reproduces everything. The runtime expansion is `cargo build`; the checking expansion is the same command with `RUSTFLAGS="--cfg __locus_check"`. The two use separate target directories, `target/runtime` and `target/check`.

## Results

These results come from stable rustc 1.97.0 (2d8144b78 2026-07-07) and cargo 1.97.0 (c980f4866 2026-06-30), rustfmt 1.9.0-stable (2d8144b788 2026-07-07) and rust-analyzer 1.97.0 (2d8144b 2026-07-07) from the same toolchain, on 2026-10-28. From an empty `target/`, `./run.sh` took 82 seconds of wall time, including every build and the five rust-analyzer runs, and left 441 MB in `target/`.

The litmus corpus was read at commit `c2b5725`. The task counted 251 `.lc` files; a commit made while this spike ran brought the corpus to 264, and every number below is for 264. None of the 22 examples copied for question 2 changed in that commit.

### Question 1: the surface syntax as macro input

```text
litmus files: 264
proc_macro2 fallback lexer: 264 of 264 tokenize, 0 fail
rustc lexer, inside a proc macro invocation: 264 of 264 tokenize, 0 fail
token dumps of files both lexers accept: 250 identical, 14 differ only in Spacing, 0 differ otherwise
   15 x `&` Joint (fallback), Alone (rustc), before Punct ' Joint
   40 x `<` Joint (fallback), Alone (rustc), before Punct ' Joint
```

Every file tokenizes with both lexers, comments included. The two lexers give the same tokens for all 264. In 14 files they differ only in the `Spacing` of a `&` or `<` written before a lifetime, as in `&'a` and `<'a>`: proc_macro2's fallback says `Joint`, rustc says `Alone`.

The corpus has no doc comments, so `tokenize/edge/` adds them, with other constructs neither lexer has seen there:

| Snippet | proc_macro2 | rustc 1.97, edition 2024 |
|---|---|---|
| 01 `/// doc` with quotes, `\` and a backtick; 02 `/** block doc */` | ok | ok, the same tokens: `#`, `[doc = " ..."]` |
| 03 `<'a>`, `&'static`, `'a'`, `'\''`; 04 `'r#a` | ok | ok; only the `Spacing` above differs |
| 09 `c"..."`, `br"..."`, `r#"..."#`; 10 `pair.0.1`; 13 `===`, `!==`, `==>`, `<==>`, `@(..)`; 14 `α`, `naïve`; 15 nested block comment; 18 numeric literals; 20 literal suffixes; 21 a `#!/usr/bin/env` line; 24 `$`, `?`, `~` | ok | ok, the same tokens |
| 05 `k#foo` | ok | error: prefix `k` is unknown |
| 06 `f"hello {x}"` | ok | error: prefix `f` is unknown |
| 07 `#"guarded"#`, 08 `##"guarded"##` | ok | error: invalid string literal (reserved since Rust 2024) |
| 12 `x'`, a prime after a name | fail | error: prefix `x` is unknown |
| 11 `` `y` `` in code | fail | error: unknown start of token |
| 16 unterminated `/*` | fail | error[E0758] |
| 17 `(]` | fail | error: mismatched closing delimiter |
| 19 `'ab'` | fail | error: character literal may only contain one codepoint |
| 22 unterminated `"` | fail | error[E0765] |
| 23 `"\u{110000}"` | fail | error: invalid unicode character escape |

So proc_macro2's fallback lexer accepts four constructs that rustc's lexer rejects in edition 2024 (05 to 08). For those four, rustc's lexer recovered and still called the macro: for 07 it passed one literal `#"guarded"#`, beside the error.

### Question 2: two expansions from one macro

The 22 litmus examples, each compiled both ways and run both ways. The checking binaries are run only to compare outputs; each printed `expansion: checking`, and each runtime binary printed `expansion: runtime`.

| Example | What it exercises | Runtime | Checking | `run` values, both binaries |
|---|---|---|---|---|
| `ghost-parameters` | `@P` parameter, hole in an argument | ok | ok | `ten_halved() => 5` |
| `checked-indices` | named components in `Vec<(i: usize, @(..))>`, hole in a tuple | ok | ok | `in_bounds(..).len() => 2` |
| `returned-borrow` | named result `(r: &mut u32, @(..))`, `final!`, `===`, `let` with a proposition | ok | ok | `nine() => 9` |
| `final-value-at-return` | `final!` in a result, hole as a tail, `let` with a proposition | ok | ok | `six() => 6` |
| `derive-eq-with-proof-field` | a proof field, `derive(PartialEq, Eq, Hash)` | ok | ok | `same(2, 7) => true` |
| `match-every-arm` | `prop` with named arms, `logic fn`, `@Even(1)`, `@False` | ok | ok | no `run` lines |
| `logical-fields` | `#[derive(Logical)]` enum and struct, `logic fn`, `Seq<Nat>` | ok | ok | no `run` lines |
| `same-is-a-proposition` | `#[derive(Logical)]`, `===` and `!==` in signatures | ok | ok | no `run` lines |
| `logic-calls-logic-function` | `logic! { double(v.len()) }`: a place and a probe | ok | ok | no `run` lines |
| `wrapped-precondition` | `pub(locus)`, hole inside `Some(..)` | ok | ok | `halve(10) => Some(5)`, `halve(7) => None` |
| `logical-closure-in-body` | `logic! { \|n: Int\| prove!(..) }`, `forall`, `===`, `let` with a proposition | ok | ok | `commutes(3) => 3` |
| `snapshot-survives-assignment` | a ghost root (`before`) in a `let`'s proposition | ok | ok | `snapshot() => 0` |
| `reproved-after-assignment` | two `let`s with propositions | ok | ok | `reproved() => 4` |
| `locus-macro-inside-rust-macro` | `vec![prove!(x == x)]`, expanded by rustc | ok | ok | `check(3) => 1` |
| `tokens-in-macro-input` | `@` inside `macro_rules!` and `stringify!` input | ok | ok | `shown() => "@(x > 0)"`, `joined(1, 2) => (1, 2)` |
| `underscore-in-rust-positions` | `_` where Rust accepts it | ok | ok | `first_of((7, 9)) => 7`, `zeros() => 4` |
| `pub-locus-as-tuple-field-type` | `pub (locus)` as a tuple field | ok | ok | `wrap() => 1` |
| `at-in-patterns` | `@` in a pattern | ok | ok | `bucket(7) => 7`, `bucket(42) => 4` |
| `raw-identifier-logic` | `r#logic` and `logic(1)` | ok | ok | `call() => 5` |
| `keywords-as-names` | `struct logic`, `fn prop` | ok | ok | `read(prop(7)) => 7` |
| `after-move` (reject) | `logic!` reads a moved `v` | ok | **E0382** at line 12 of the example | |
| `during-mutable-borrow` (reject) | `logic!` reads a mutably borrowed `v` | ok | **E0502** at line 8 of the example | |

For each of the 16 accepted examples with `run` lines, both binaries printed exactly the example's `run` lines; the other 4 accepted examples have none. The two rejected examples fail only in the checking expansion, at the line their annotation names (line N of an example is line N + 7 of its case file):

```text
== after-move: runtime=ok check=E0382
  check:   cases/src/bin/after-move.rs:19:22: error[E0382]: borrow of moved value: `v`: value borrowed here after move
== during-mutable-borrow: runtime=ok check=E0502
  check:   cases/src/bin/during-mutable-borrow.rs:15:22: error[E0502]: cannot borrow `v` as immutable because it is also borrowed as mutable: immutable borrow occurs here
```

What `locus!` wrote for two of them (`./run.sh expansions`, formatted by rustfmt; the spaced-out tokens are rustfmt leaving macro arguments alone):

```rust
// returned-borrow
fn first_mut(
    pair: &mut (u32, u32),
) -> (
    &mut u32,
    :: locus :: Proof!
[r == pair.0 && final ! (pair) === (final!(r), pair.1)],
) {
    (&mut pair.0, :: locus :: proof! ([] []))
}
fn nine() -> u32 {
    let mut pair: (u32, u32) = (1, 2);
    let (first, related) = first_mut(&mut pair);
    *first = 9;
    #[allow(unused_variables)]
    let is_nine: ::locus::Proof![pair.0 == 9] = :: locus :: proof! ([] []);
    ::locus::observe!(&pair.0);
    pair.0
}
#[allow(unused_imports)]
use ::locus::prelude::*;

// logic-calls-logic-function
#[allow(dead_code, non_snake_case, unused_variables)]
fn double(n: Nat) -> Nat {
    ::locus::stub()
}
fn f(v: Vec<u32>) -> usize {
    #[allow(unused_variables)]
    let twice = :: locus :: data!
    ([& v] [double] double(v.len()));
    v.len()
}
```

The runtime expansion still warns about four runtime values that only logical code reads, or proofs that a runtime call returns, as the earlier spike found: `related` (returned-borrow), `grew` (final-value-at-return), `x` (read only by `vec![prove!(x == x)]`), and `value assigned to n is never read` (snapshot-survives-assignment, read only by `logic! { n }`).

**What the macro needed that lowering would not.** Four things, each shown by a case:

1. **State from another invocation: which types are logical.** `x-split-logical-type.rs` puts the earlier spike's case 11 in three `locus!` blocks in one module: `#[derive(Logical)] struct Point` (with `logic fn double`) in the first, `impl Point { logic fn origin() .. }` in the second, and `logic! { double(3) }` and `logic! { Point::origin() }` in the third. The second invocation cannot know that `Point` is logical, so it writes `impl Point` on the alias, and the third probes `Point::origin` rather than `__locus_Point::origin`:

   ```text
   == x-split-logical-type: runtime=E0118 check=E0118,E0599
     runtime: cases/src/bin/x-split-logical-type.rs:20:5: error[E0118]: no nominal type found for inherent implementation: impl requires a nominal type
     check:   cases/src/bin/x-split-logical-type.rs:30:40: error[E0599]: no associated function or constant named `origin` found for struct `LogicalData` in the current scope
   == x-joined-logical-type: runtime=ok check=ok
   ```

   The same code in one invocation (`x-joined-logical-type.rs`) compiles both ways. `double`, a plain logic function defined in one invocation and called in another, needed nothing: rustc resolved the probe. The E0599 is in the checking expansion only, because probes exist only there.
2. **The type of logical code, for which marker a `logic!` gives.** RFC 0002 allows a lemma, `logic fn add_zero(n: Int) -> @(n + 0 === n)`, and `logic-closure-in-body` uses a `logic!` whose closure proves a `forall`. Each gives a proof. The toy, like the earlier spike, picks `proof!` only when the `let`'s annotation is `@P`, and `data!` otherwise. `x-logic-kind.rs` passes both without an annotation:

   ```text
   == x-logic-kind: runtime=E0308 check=E0308
     runtime: cases/src/bin/x-logic-kind.rs:30:21: error[E0308]: mismatched types: expected `Erased`, found `LogicalData`
     runtime: cases/src/bin/x-logic-kind.rs:30:43: error[E0308]: mismatched types: expected `Erased`, found `LogicalData`
   ```

   The annotated versions in the same file compile. This is not specific to the macro: RFC 0001's lowering decides "never an expression's type", and its table does not say how it picks between `proof!` and `data!` for a `logic!`. For a lemma, lowering of a whole crate could read the logic function's signature from the crate's items; `locus!` would need it from another invocation, or from another crate.
3. **Names in the user's namespace.** The output names the library as `::locus::...`, so the dependency must be called `locus` and not renamed. `use ::locus::prelude::*;` makes `Nat`, `prove!` and the rest visible to all code in the module, not only to Locus code. And items written inside `locus!` are macro-expanded items, which changes how rustc resolves names around them. The first version of `pub-locus-as-tuple-field-type`'s case had `use locus::locus;`; the example declares `pub struct locus;`, inside the block:

   ```text
   == x-locus-name-collision: runtime=E0433,E0659 check=E0433,E0659
     runtime: cases/src/bin/x-locus-name-collision.rs:5:5: error[E0659]: `locus` is ambiguous: ambiguous name
   ```

   rustc's note says `ambiguous because of a conflict between a macro-expanded name and a less macro-expanded name from outer scope during import or macro resolution`. The generated cases use `use ::locus::locus;`, which compiles.
4. **A `check-cfg` declaration in every crate that uses Locus.** The `cfg` in the library's macro output is evaluated in the user's crate, so a crate that does not declare `cfg(__locus_check)` gets a warning at every invocation (from `cross/user_c`):

   ```text
   warning: unexpected `cfg` condition name: `__locus_check`
    --> cross/user_c/src/bin/c-marker-hole.rs:6:45
     |
   6 |         let q = lib_a_marker::divide(10, 2, _);
     |                                             ^
     |
     = note: using a cfg inside a macro will use the cfgs from the destination crate and not the ones from the defining crate
     = help: try referring to `::locus::proof` crate for guidance on how handle this unexpected cfg
   ```

Everything else was decided from the tokens of one invocation: which parameters and `let`s are ghost, holes, which places logical code reads, named components, `prop` and `logic fn` stubs, and which tokens belong to another macro's input.

### Question 3: diagnostics

rustc points at the user's line and column in the `.rs` file, in both expansions. The plain type error, identical in both:

```text
== plain-error, runtime expansion
error[E0308]: mismatched types
  --> diag/src/bin/plain-error.rs:10:25
   |
10 |         let four: u32 = "four";
   |                   ---   ^^^^^^ expected `u32`, found `&str`
   |                   |
   |                   expected due to this
== plain-error, check expansion
error[E0308]: mismatched types
  --> diag/src/bin/plain-error.rs:10:25
   |
10 |         let four: u32 = "four";
   |                   ---   ^^^^^^ expected `u32`, found `&str`
   |                   |
   |                   expected due to this
```

At positions `locus!` rewrote, the errors point at the source token too, identically in both expansions. A hole where a `u32` belongs points at the `_`, and names the library's macro. A proof parameter returned as a `u32` points at the parameter's use:

```text
error[E0308]: mismatched types
  --> diag/src/bin/locus-position-error.rs:11:15
   |
10 |     fn hole_for_runtime() -> u32 {
   |                              --- expected `u32` because of return type
11 |         halve(_, _)
   |               ^ expected `u32`, found `Erased`
   |
   = note: this error originates in the macro `::locus::proof` (in Nightly builds, run with -Z macro-backtrace for more info)
error[E0308]: mismatched types
  --> diag/src/bin/locus-position-error.rs:15:9
   |
14 |     fn proof_as_runtime(n: u32, even: @(n % 2 == 0)) -> u32 {
   |                                                         --- expected `u32` because of return type
15 |         even
   |         ^^^^ expected `u32`, found `Erased`
```

The messages are rustc's, in rustc's terms (`Erased`), not Locus's. An error that exists only in the checking expansion, logical code reading a moved vector, points at the place:

```text
== check-only-error, runtime expansion
  (compiled)
== check-only-error, check expansion
error[E0382]: borrow of moved value: `v`
  --> diag/src/bin/check-only-error.rs:13:26
   |
11 |         let v = vec![1, 2, 3];
   |             - move occurs because `v` has type `Vec<u32>`, which does not implement the `Copy` trait
12 |         let count = consume(v);
   |                             - value moved here
13 |         let n = logic! { v.len() };
   |                          ^ value borrowed here after move
...
help: consider cloning the value if the performance cost is acceptable
   |
12 |         let count = consume(v.clone());
   |                              ++++++++
```

### Question 4: tooling

**rustfmt leaves `locus!` blocks byte for byte as written, and formats the rest of the file.** It does not fail. `cargo fmt --check --all` exited 1, reporting only `diag/src/bin/fmt-probe.rs`, and `rustfmt` on a copy of that file exited 0 and changed only the function outside the block:

```text
### rustfmt on a copy of diag/src/bin/fmt-probe.rs, then diff against the original
rustfmt exit status: 0
9c9,11
< fn   plain_outside( x:u32 )->u32{x+1}
---
> fn plain_outside(x: u32) -> u32 {
>     x + 1
> }
```

Inside the block, `fn   halve( n:u32,even : @(n%2==0) )->u32{ n/2 }` and the plain-Rust `fn plain_inside( x:u32 )->u32{x+1}` stayed as written. So Locus code is never formatted, plain Rust inside a block included. The same holds for the 22 cases: `gen-cases.py` passes each through rustfmt and then checks that the example's text is still there byte for byte, which it is; rustfmt reformatted only a harness `main`, outside the block.

**rust-analyzer** 1.97.0 was installed with `rustup component add rust-analyzer`, through the proxy, with no other setup. It ran `cargo check` itself and expanded `locus!` with the toolchain's `rust-analyzer-proc-macro-srv`.

- **Items and types inside `locus!` blocks are resolved.** `analysis-stats -v -v` analyzed 54 bodies written inside `locus!` blocks in `cases` (every body there other than each binary's `main`). Each had 0 expressions of unknown type, except:
  - one expression of unknown type for each `observe!` statement: `in_bounds`, `nine`, `six`, `commutes`, `check` (1 each), and `snapshot`, `reproved`, `annotated` (2 each). In the runtime expansion, `observe!` expands to a `cfg`-disabled block. Under `RUSTFLAGS="--cfg __locus_check"`, where it expands to observations, these unknowns are gone;
  - `[0; _]` in `underscore-in-rust-positions`, which rust-analyzer does not infer in plain Rust either (`plain-array-infer.rs 4:17-4:18: Unknown type`);
  - and, as type mismatches rather than unknown types, the case meant to fail: `x-logic-kind.rs 30:20-30:25: Expected Erased, got LogicalData`, and the same at `30:42-30:45`.
- **Type mismatches inside a block are reported at the user's position**, the same ones rustc reports: `plain-error.rs 10:24-10:30: Expected u32, got &'static str`, and `locus-position-error.rs 15:8-15:12: Expected u32, got Erased` (line 1-based, column 0-based). For the hole, rust-analyzer gives the position of `locus!` itself, `5:0-5:6`, where rustc gave the `_`.
- `rust-analyzer diagnostics .` reported, inside blocks, the E0308s above and the argument-count errors of question 5 (it calls them E0107). It reported nothing for E0118 in `x-split-logical-type`, nor the E0659 in `x-locus-name-collision` (only `unresolved macro locus::__expansion!`).
- **It analyzes one expansion at a time.** With `RUSTFLAGS="--cfg __locus_check"`, it analyzed the checking expansion: `diagnostics` then also reported `x-split-logical-type.rs` line 30 `E0599 ... no such associated item`, the probe that exists only there, and the question 5 errors changed sides. It reported no borrow errors (E0382, E0502) in either expansion.
- `rust-analyzer unresolved-references .` panicked (exit status 101: `Try to use attached db, but not db is attached`) while processing its first crate, `tokenize`, which is plain Rust with no `locus!`. So it gave no result.
- Unrelated to Locus: `diagnostics` reported `tokenize/src/main.rs`'s `#[path = "../../locus-macros/src/dump.rs"] mod dump;` as E0583, although rustc compiles it. The workspace totals vary between runs only in `tokenize::main` (2 unknown expressions in one run, 9 in another), so the totals are not shown as a measure.

### Question 5: visibility across crates

Each binary was built and, where it compiled, run, in both expansions:

| Binary | What it does | Runtime expansion | Checking expansion |
|---|---|---|---|
| **user_b, plain Rust** | | | |
| `b-marker-no-proof` | (i) `divide(10, 2)` | E0061: takes 3 arguments but 2 were supplied | E0061 |
| `b-marker-checked` | (i) `divide(10, 2, check_nonzero(2))` | prints 5 | prints 5 |
| `b-marker-forged` | (i) `divide(10, 0, locus::Erased)` | compiles; panics: attempt to divide by zero | the same |
| `b-marker-reused` | (i) `divide(10, 0, check_nonzero(2))`: a proof of `2 != 0` for `0 != 0` | compiles; panics: attempt to divide by zero | the same |
| `b-removed-no-proof` | (ii) `divide(10, 2)` | prints 5 | E0061: takes 3 arguments but 2 were supplied |
| `b-removed-forged` | (ii) `divide(10, 2, locus::Erased)` | E0061: takes 2 arguments but 3 were supplied | prints 5 |
| **user_c, in `locus!`** | | | |
| `c-marker-hole` | (i) `divide(10, 2, _)` | prints 5 | prints 5 |
| `c-marker-checked` | (i) `divide(10, 2, check_nonzero(2))` | prints 5 | prints 5 |
| `c-removed-hole` | (ii) `divide(10, 2, _)`, written as for (i) | E0061: takes 2 arguments but 3 were supplied | prints 5 |
| `c-removed-hole-dropped` | (ii) the same, with `drop_hole_args` | prints 5 | prints 5 |
| `c-removed-checked-dropped` | (ii) `divide(10, 2, check_nonzero(2))`, with `drop_hole_args`: not a hole, so kept | E0061: takes 2 arguments but 3 were supplied | prints 5 |
| `c-removed-some-dropped` | `Some(_)`, with `drop_hole_args` | E0061: this enum variant takes 1 argument but 0 were supplied | prints `Some(Erased)` |

What this shows, for a decision this spike does not make:

- **(i), the marker kept.** One signature in both expansions, so every crate in the build agrees on it whatever `cfg` each is compiled with. A plain-Rust caller cannot omit the proof, but can forge one in either expansion: by naming `locus::Erased`, or by passing a proof of another proposition, which needs no dependency on `locus` at all. Both compiled and reached the division by zero that the precondition rules out. A second Locus crate writes the proof argument as it would anywhere, and its `locus!` needs nothing about `divide`. Nothing in a stock `cargo build` checks `user_b`, so under (i) a contract holds only for callers that something checks.
- **(ii), the parameter removed at run time.** Plain Rust calls `divide(10, 2)` in the runtime expansion, but that same call is E0061 in the checking expansion: no plain-Rust source compiles under both `cfg`s, unless it tests `__locus_check` itself, which RFC 0001 forbids a program to do. So with `--cfg __locus_check` set for the whole build, a plain-Rust dependent of a (ii) crate does not build. A second Locus crate must drop the proof argument from the runtime call, and only `divide`'s signature, in another crate, says which argument that is. One invocation does not have it. Dropping holes by syntax works for `divide(10, 2, _)`, but keeps a proof from a runtime call (`check_nonzero(2)`, E0061), and also drops holes that fill a type's generic parameter (`Some(_)`, E0061). Dropping the call `check_nonzero(2)` instead would also remove the runtime check it performs.

## Findings

1. **The surface syntax survives as macro input.** All 264 litmus files tokenize with rustc's lexer inside a proc macro invocation, and give the same tokens as proc_macro2's lexer apart from the `Spacing` of `&` and `<` before a lifetime. proc_macro2's fallback lexer is not a substitute for rustc's: it accepted `k#foo`, `f"..."`, `#"..."#` and `##"..."##`, which rustc rejects in edition 2024. Syntax that would not survive, were Locus to want it, includes a prime after a name (`x'`, rejected by both lexers) and backticks in code.
2. **One macro, one text, two expansions works for these examples.** 20 decided accepted examples compiled both ways, the 16 with `run` lines printed their values in both binaries, and the two decided rejected examples failed only in the checking expansion, at the annotated line. Neither the macro nor the library reads the `cfg`: the library's macro output carries `#[cfg(__locus_check)]`, which is evaluated in the user's crate.
3. **One invocation sees less than lowering of a whole crate.** It needed state from other invocations for logical types (E0118, and E0599 when checking), the type of logical code for the marker a `logic!` gives (E0308), a name for the library in the user's namespace (E0659 against a user's `struct locus`), and a `check-cfg` declaration in each crate.
4. **rustc's diagnostics point at the user's code in both expansions,** for plain Rust and at rewritten positions, because the macro copies tokens with their spans and gives new tokens the span of the token they replace. The messages are rustc's (`expected u32, found Erased`), and borrow errors from observations are rustc's too.
5. **rustfmt never touches a `locus!` block,** plain Rust inside it included. **rust-analyzer expands the blocks and resolves what is in them,** for one expansion at a time, chosen by `RUSTFLAGS`.
6. **Across crates, (i) keeps one signature for both expansions and lets any unchecked caller forge or reuse a proof; (ii) gives two signatures, which breaks plain-Rust callers in the checking expansion and needs callee signatures at Locus call sites.**

## What contradicts the macro-first plan's assumptions

- **"Ghost values removed" is not an option at a Locus call site without signatures.** Strategy (ii) needs, at each call, which arguments are proofs, which a proc macro cannot know for a function in another invocation or crate.
- **"Stock `cargo build` gives the runtime expansion with no cfg set" holds, but a plain `cargo build` checks nothing,** and under (i) a plain-Rust crate can call a Locus function with a forged or reused proof, in both expansions.
- **"Lowering decides only what the source shows" does not determine `logic!`'s marker** when its logical code gives a proof, as a lemma or a logical closure proving a `forall` does. That is a gap in RFC 0001 as written, not only in the macro.
- **Logical types cannot be split across invocations** without state shared between them.
- **The library's name is not fresh.** RFC 0001 chooses names that appear nowhere in the sources; a macro's output must name a crate the user depends on, in the user's namespace, and items inside a block are macro-expanded, which makes a user's item named `locus` ambiguous (E0659).
- **Two decided examples of `compat.superset.same-meaning` assume lowering of whole files.** `include-from-source.accept.lc` expects `file!` and `include_str!` to resolve from the `.lc` file; in a `.rs` file they name the `.rs` file, as plain Rust does. `reserved-check-cfg.reject.lc` expects a program's own `#[cfg(__locus_check)]` to be rejected; `locus!` sees only its own block, so a test of the `cfg` outside a block is not seen, and a stock build does not reject it. Neither was run here.
- **Every Locus crate must declare `check-cfg = ['cfg(__locus_check)']`,** or rustc warns at every invocation.

## What it did not test

- A real parser for Locus's syntax: the toy handles the spike's examples and nothing more. A runtime binding that shadows a ghost name in the same block is still taken as ghost (each block has one set of ghost names), and roots in `prove!` inside another macro's input are always observed precisely, so the earlier spike's question 3 (ghost roots inside `vec![..]`) is not answered for the macro.
- The rustc driver, facts, the logical pass, and validation. The checking expansion here is only compiled, never read.
- Lints of the checking expansion: RFC 0001 caps them with `--cap-lints allow`, which `RUSTFLAGS` would apply to every crate. Not tried.
- `cargo test`, doctests, build scripts, `--target` builds, and `RUSTFLAGS` set alongside other flags.
- `mod` declarations of other files inside a block, inner attributes inside a block other than the spike's own `#![spike(..)]`, and a `macro_rules!` defined inside a block and used outside it.
- Errors that `locus!` itself reports, through `compile_error!` with spans: the toy has none, and panics instead.
- rust-analyzer in an editor (hover, go to definition, completion inside blocks): only its command-line `analysis-stats`, `diagnostics` and `unresolved-references` were run, and the last panicked.
- The other 242 litmus examples through `locus!`: question 1 only lexed them.
- A `logic!` whose marker is inferred from its use, rather than chosen by the macro, as a way around finding 3's E0308.
- Export mode, closures and async blocks, packed fields, snapshots and models.
- Nightly: everything ran on stable 1.97.
