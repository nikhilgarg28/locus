# Spike: one lowered text for build mode

> The marker for logical data is called `ErasedData` here, as it was when this spike ran. The RFCs now call it `LogicalData`.

A throwaway experiment for [RFC 0001](../../design/rfcs/0001-compiler-pipeline.md), decision 15. Nothing outside `spikes/` may depend on it.

## Question

In build mode, the checking program and the runtime program are one lowered Rust text, compiled twice: against the checking version of Locus's library and against the runtime version. Ghost values stay in the text as zero-sized markers, `Erased` for proofs and `ErasedData` for other logical data. Lowering uses only syntax, lexical scopes and item-level names. Does that hold up?

1. **Hard cases.** For each, do both versions compile? Does the runtime version need any edit beyond the library swap? Did lowering need type information?
2. **Identical tokens for macros.** Does the external review's attribute macro, which returned 0 and 1 under the old design, now return the same value in both programs?
3. **Locus macros that rustc expands,** inside `vec![..]`, in a macro's output, through an alias, inside `stringify!`. In particular a logical snapshot read inside `vec![..]` beside a runtime root that must be observed precisely. Which of (a) whole roots only, (b) a side table read through `proc_macro::Span` positions, (c) rewriting invocations inside other macros' input, or (d) something better works?
4. **Facts.** Do rustc's typed bodies of the checking version say which callee each call resolved to, which arguments fill parameters whose source type is logical, and where markers flow?
5. **Borrow checking.** Does the runtime version compile whenever the checking version does? Does the checking version reject logical code that reads a moved value or a mutably borrowed field?

## Setup

- `library/` is Locus's library, built twice by `build-lib.sh`, with and without `--cfg locus_check`:
  - `locus.rs` is crate `__locus`. It defines `Erased`, which derives `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `PartialOrd`, `Ord` and `Hash`, and `ErasedData`, which derives only `Clone`, `Copy` and `Debug`. It also has the prelude aliases `Int`, `Nat`, `Prop` and `Seq<T> = <PhantomData<T> as Proj>::Out`, and `observe`, `probe` and `stub`. Its macros are of three kinds:
    - `Proof![P]`, a type macro that is `Erased` in both versions.
    - `proof!`, `data!` and `observe!`, for the invocations that lowering writes. Each takes `[&place, ...] [item, ...] logical code`. In the checking version they observe each place, probe each item, and give the marker. In the runtime version they give the bare marker.
    - `__locus_ghosts!`, the default for option (d).
  - `macros.rs` is the proc-macro crate `locus_macros`, re-exported as `logic!`, `prop!` and `prove!`. These handle the invocations that rustc expands, because lowering never saw them. In the checking version they find the places that the logical code reads and observe each one. In the runtime version they give the marker. The env var `LOCUS_ROOTS` picks how each root is classified: `precise`, `whole` (a), `table` (b), `dispatch` (d) or `combined`. `LOCUS_SCOPES` names (b)'s side table. The place finder is a toy, not Locus's parser.
- `cases/` holds hand lowerings for questions 1 and 5. Each `name.lc` is a source in provisional syntax. `name.rs` is its lowering, kept line for line, with an `// expect: check=... run=...` line. `*.old.rs` is the previous design's text, where ghost parameters were removed. The lowering rules used:
  - `@P` becomes `::__locus::Proof![P]`, and a hole `_` becomes `::__locus::proof!([] [])`.
  - `prove!`, `logic!` and `prop!` written in the function itself become `proof!` or `data!`, with the places computed from the syntax. Lowering never rewrites inside another macro's input.
  - A proposition in a `let` annotation adds `::__locus::observe!(&places);` after the `let`.
  - An operator on lexically ghost operands becomes `data!`.
  - `logic fn` and `prop` become stub functions.
  - A `#[derive(Logical)]` type becomes a stub struct `__locus_T` and a projection alias `type T = <__locus_T as Proj>::Out`.
  - `pub(locus)` becomes `pub`, and named tuple components lose their names.
  - The last line, `#[macro_use] extern crate __locus; use __locus::prelude::*;`, stands in for the generated module's prelude, without `prelude_import`.
- `q2/` holds the attribute macro `#[token_sensitive]`, `answer.lc`, its one lowered text, and the old design's two texts.
- `q3/` has copies of the five litmus examples (`*.accept.lc`, unchanged) and their lowerings, plus the new cases. `*.scopes` is (b)'s side table: for each binding of a name that is ever ghost, its lexical scope in the lowered file. `*.c.rs` is option (c). `respan.rs` is an attribute macro that gives every token of its item its own call-site span.
  - For option (d), lowering writes a local `macro_rules! __locus_ghosts` after each binding that changes which names are ghost. Its arms observe a ghost root whole and any other root precisely.
  - The proc macro then emits `__locus_ghosts!(root; &place)` with the root's span, so rustc picks the arm through its own textual scoping of `macro_rules`.
- `facts/` is a rustc driver of about 220 lines, adapted from [`spikes/rustc-facts`](../../spikes/rustc-facts/README.md). It is a plain rustc replacement. After expansion, it prints facts from rustc's typeck results for the code written in the file, then lets compilation continue.
- `run.sh` reproduces everything. Run `./run.sh [all|cases|q2|q3|spans|facts]`. It needs `rustup component add rustc-dev --toolchain stable`.
  - For each case, it compiles the text against both versions of the library, compares the error codes with the `expect` line, and runs both binaries when both compiled.
  - The checking version is compiled with `--cap-lints allow`, as in RFC 0001.
  - The checking binaries are run only to compare outputs; a real checking program is never run.

## Results

These results come from stable rustc 1.97.0 (2d8144b78 2026-07-07), on 2026-10-26. `./run.sh` took 53 and 59 seconds of wall time in two runs, including every build: both versions of the library, the test proc macros, and the driver. All 108 compiled pairs matched their `expect` lines: 30 cases, and 78 rows of question 3.

### Questions 1 and 5: cases

When both versions compiled, both binaries printed the same output, shown in the table.

| Case | Checking | Runtime | Output, or what decides |
|---|---|---|---|
| 01 `gates.first().unwrap().admit(n, pos)`, `Some(ok)` | ok | ok | `check(&gates, 5, _) = 5` |
| 02 `identity(positive)`, `Some(identity(p))` | ok | ok | `kept.is_some() = true` |
| 03 `for (i, ok) in checked` over `Vec<(usize, @P)>` | ok | ok | `sum(&[1, 2, 3]) = 6` |
| 04 `let f = halve; f(10, _)` | ok | ok | `pick() = 5` |
| 04b `halve` as `fn(u32) -> u32` | E0308 | E0308 | rustc: arity |
| 04c `[7].map(halve)` | E0593 | E0593 | rustc: arity |
| 04d `black_box(halve)` | ok | ok | rustc accepts; Locus must reject |
| 04e `let f: fn(u32, @P) -> u32 = halve`, `fold(10, halve)` | ok | ok | rustc accepts; Locus must reject |
| 05 `Interval { lo, hi, ordered: self.order(lo, hi) }` | ok | ok | `width(2, 5) = 3`, `width(5, 2) panicked = true` |
| 06 `values.push(logic! { 3 })`, annotated and not; `Vec<Nat>` as `Vec<Seq<Int>>` | ok | ok | rustc accepts all three; Locus must reject the last two |
| 07 inherent `go(&self, a, h: @P)` vs trait `go(self, a)` | E0061 | E0061 | rustc: `this method takes 1 argument but 2 arguments were supplied` |
| 07.old, `h` removed | ok | ok | `call(Thing, _) = 101`: the trait method, silently |
| 08 `make(positive(x))`, `make<T: Default>` | E0277 | E0277 | rustc: `` the trait bound `Erased: Default` is not satisfied `` |
| 08.old, result `()` | ok | ok | `make::<()>`, silently |
| 09a `a == b` on `&Vec<Nat>` | E0369 | E0369 | rustc |
| 09b `derive(PartialEq, Hash)` with a `Nat` field | E0369, E0277 | E0369, E0277 | rustc |
| 09c all eight derives with a proof field | ok | ok | `w == w: true, set.len() = 1, Witnessed { x: 1, ok: Erased }` |
| 09d `derive(Default)` with a proof field | E0277 | E0277 | rustc |
| 10 `let both = p && q;` on `prop!` bindings | ok | ok | `both(3) = 3` |
| 10b `ok && ok` on a pattern-bound proof | E0308 | E0308 | rustc; Locus should say "write `logic!`" |
| 11 `logic fn`, `prop`, `derive(Logical)`, `pub(locus)`, named result | ok | ok | `use_them() = 4` |
| 11b naive `type Point = ErasedData;` | E0091, E0116 | E0091, E0116 | why projection aliases |
| 11c probe `__locus_List::Nil`, no annotation | E0282 | ok | the probe's form |
| 11d probe `__locus_List::<::__locus::ErasedData>::Nil` | ok | ok | `empty() = 0` |
| 12 `prove!(v.len() == 3)` after `let moved = v;` | E0382 | ok | rustc's borrow checker |
| 13 `prove!(p.b > 0)` while `&mut p.a` lives | ok | ok | `bump = 2, a = 2` |
| 13b `prove!(p.a > 0)` while `&mut p.a` lives | E0502 | ok | rustc's borrow checker |
| 14 `values.push(logic! { values.len() })` | ok | ok | `grow() = 1`: two-phase borrow |
| 15 `prove!(p.a == 0)` before `p`'s type is known | E0282 | ok | the observation needs the type there |
| 16 lowering's lint marks | ok | ok | `uses() = 4` |

The runtime version warns about ghost bindings: `` unused variable: `pos` `` (01), `` field `ordered` is never read `` (05), and stubs `never used` (11). With lowering's `#[allow(unused_variables)]` on a ghost parameter or `let`, and `#[allow(dead_code)]` on a proof field, those warnings go away (16). The marks cannot cover runtime values read only by logical code. These still warn in the runtime version:

```text
== 16-runtime-lints: check=ok run=ok (as-expected)
  runtime flavor:
    warning unused_variables 14:9 unused variable: `k`
    warning unused_variables 16:9 unused variable: `m`
```

Here `k` is read only by `prove!(k > 0)`, and `m` only by `vec![prove!(m > 0)]` and an annotation. Case 02 warns the same way about `x` in `fn relay(x: u32, positive: @(x > 0))`.

### Question 2

```text
Old design, two texts:
== answer.old-check, check flavor: ok
  output: answer() = 0
== answer.old-run, run flavor: ok
  output: answer() = 1
New design, one text:
== answer, check flavor: ok
  output: answer() = 0
== answer, run flavor: ok
    warning unused_variables 7:9 unused variable: `h`
  output: answer() = 0
```

### Question 3

The table shows the checking version's result for each mode. The runtime version compiled in every row except `hole-in-vec`, where it failed with the same error.

| File | precise | whole (a) | table (b) | dispatch (d) | combined | (c) |
|---|---|---|---|---|---|---|
| locus-macro-inside-rust-macro (decided accept) | ok | ok | ok | ok | ok | |
| prove-in-macro-output (decided accept) | ok | ok | ok | ok | ok | |
| prove-through-alias (decided accept) | ok | ok | ok | ok | ok | |
| prove-in-stringify (decided accept) | ok | ok | ok | ok | ok | ok, wrong string |
| macro-in-vec-reads-sibling-field (decided accept) | ok | **E0502** | ok | ok | ok | |
| macro-in-vec-reads-borrowed-field | E0502 | E0502 | E0502 | E0502 | E0502 | |
| macro-in-vec-after-move | E0382 | E0382 | E0382 | E0382 | E0382 | |
| hole-in-vec, `vec![_]` | error | error | error | error | error | |
| snapshot-in-vec (the hard case) | E0609 | E0502 | ok | ok | ok | ok |
| snapshot-shadowed: a runtime `before` in an inner block | E0609 | E0502 | ok | ok | ok | |
| snapshot-through-own-macro: `same_a!(before)`, `same_a!(pair)` | E0609 | E0502 | ok | ok | ok | E0609 |
| snapshot-macro-body: the macro's body names the ghost | E0609 | ok | ok | **E0609** | ok | |
| snapshot-nested-fn: `fn inner(mut before: Pair)` | ok | E0502 | ok | ok | ok | |
| snapshot-nested-fn.noreset: the same without (d)'s reset | ok | E0502 | ok | **E0502** | ok | |
| snapshot-respanned: under `#[respan]` | E0609 | E0502 | **E0609** | ok | ok | |

Wherever the decided examples compiled, both versions printed the same: `check(3) = 1`, `same(4) = 4` for both `prove-in-macro-output` and `prove-through-alias`, and `bump_a = 2, a = 2`. `prove-in-stringify` printed `shown() = "prove!(x > 0)"` in every mode except (c):

```text
== prove-in-stringify.c [c]: check=ok run=ok (as-expected)
  output, checking flavor: shown() = "::__locus::proof!([&x] [] x > 0)"
  output, runtime flavor:  shown() = "::__locus::proof!([&x] [] x > 0)"
```

The hard case, `let before = logic! { pair };` followed by `vec![prove!(before.a == before.a)]` and `vec![prove!(pair.a == before.a)]` while `&mut pair.b` lives:

```text
== snapshot-in-vec [precise]: check=E0609 run=ok (as-expected)
  check: error E0609 11:83 no field `a` on type `ErasedData`
  check: error E0609 11:95 no field `a` on type `ErasedData`
  check: error E0609 12:89 no field `a` on type `ErasedData`
== snapshot-in-vec [whole]: check=E0502 run=ok (as-expected)
  check: error E0502 12:72 cannot borrow `pair` as immutable because it is also borrowed as mutable
== snapshot-in-vec [table]: check=ok run=ok (as-expected)
  output, checking flavor: snapshot = 2, b = 3
  output, runtime flavor:  snapshot = 2, b = 3
== snapshot-in-vec [dispatch]: check=ok run=ok (as-expected)
  output, checking flavor: snapshot = 2, b = 3
  output, runtime flavor:  snapshot = 2, b = 3
```

A hole inside another macro's input:

```text
== hole-in-vec [precise]: check=error run=error (as-expected)
  check: error error 3:54 in expressions, `_` can only be used on the left-hand side of an assignment
  run:   error error 3:54 in expressions, `_` can only be used on the left-hand side of an assignment
```

`proc_macro::Span` on stable 1.97:

```text
  stable:   s.line()
  stable:   s.column()
  stable:   s.start()
  stable:   s.end()
  stable:   s.file()
  stable:   s.local_file()
  stable:   s.source_text()
  unstable: s.byte_range(): error[E0658]: use of unstable library feature `proc_macro_span`
  unstable: s.join(s): error[E0658]: use of unstable library feature `proc_macro_span`
  unstable: s.parent(): error[E0658]: use of unstable library feature `proc_macro_span`
  unstable: s.source(): error[E0658]: use of unstable library feature `proc_macro_span`
  unstable: proc_macro::tracked::path("scopes"): error[E0658]: use of unstable library feature `proc_macro_tracked_path`
  unstable: proc_macro::tracked::env_var("LOCUS_SCOPES"): error[E0658]: use of unstable library feature `proc_macro_tracked_env`
```

These are the positions that roots reach the proc macro with. In `snapshot-in-vec`, they are written inside `vec![..]`. In `snapshot-through-own-macro`, they are passed as `$e:expr` to the program's macro, at line 16 and line 17. In `snapshot-macro-body`, they are written in the macro's body, at line 13. In `snapshot-respanned`, they were rebuilt by `#[respan]`:

```text
== snapshot-in-vec
  locus-root before file=snapshot-in-vec.rs line=11 column=76 source_text="before" mode=table ghost=true
  locus-root before file=snapshot-in-vec.rs line=11 column=88 source_text="before" mode=table ghost=true
  locus-root pair file=snapshot-in-vec.rs line=12 column=72 source_text="pair" mode=table ghost=false
  locus-root before file=snapshot-in-vec.rs line=12 column=82 source_text="before" mode=table ghost=true
== snapshot-through-own-macro
  locus-root before file=snapshot-through-own-macro.rs line=16 column=71 source_text="before" mode=table ghost=true
  locus-root before file=snapshot-through-own-macro.rs line=16 column=71 source_text="before" mode=table ghost=true
  locus-root pair file=snapshot-through-own-macro.rs line=17 column=69 source_text="pair" mode=table ghost=false
  locus-root pair file=snapshot-through-own-macro.rs line=17 column=69 source_text="pair" mode=table ghost=false
== snapshot-macro-body
  locus-root before file=snapshot-macro-body.rs line=13 column=25 source_text="before" mode=table ghost=true
  locus-root before file=snapshot-macro-body.rs line=13 column=37 source_text="before" mode=table ghost=true
== snapshot-respanned
  locus-root before file=snapshot-respanned.rs line=10 column=1 source_text="#[respan]" mode=table ghost=false
  locus-root before file=snapshot-respanned.rs line=10 column=1 source_text="#[respan]" mode=table ghost=false
  locus-root pair file=snapshot-respanned.rs line=10 column=1 source_text="#[respan]" mode=table ghost=false
  locus-root before file=snapshot-respanned.rs line=10 column=1 source_text="#[respan]" mode=table ghost=false
```

### Question 4

These are excerpts from the driver's output on the checking version of cases 01, 04e and 07. `run.sh` also runs it on cases 02 and 03.

```text
== 01-inferred-receiver
  body check
  15:37 local `pos`: __locus::Erased
  16:14 method `admit` resolved to `Gate::admit`
    self `gates.first().unwrap()`: declared &Gate: runtime
    arg 1 `n`: declared u32: runtime
    arg 2 `pos`: declared __locus::Erased: logical parameter
    result: declared __locus::Erased, instantiated __locus::Erased
  16:14 method `unwrap` resolved to `std::option::Option::<&Gate>::unwrap`
    self `gates.first()`: declared std::option::Option<T>: runtime
  16:14 method `first` resolved to `core::slice::<impl [Gate]>::first`
    self `gates`: declared &[T]: runtime
  16:9 local `ok`: __locus::Erased
  17:16 call `Some(ok)` resolved to `std::option::Option::<__locus::Erased>::Some`
    arg 0 `ok`: declared T: markers flow into generic parameter (instantiated __locus::Erased)
    result: declared std::option::Option<T>, instantiated std::option::Option<__locus::Erased>
  17:9 local `held`: std::option::Option<__locus::Erased>
== 04e-matching-types
  body pointer
  9:61 function `halve` with a marker parameter used as a value, as `fn(u32, __locus::Erased) -> u32`
  9:9 local `f`: fn(u32, __locus::Erased) -> u32
  10:5 call `f(10, ::__locus::proof!([] []))` through a value of type `fn(u32, __locus::Erased) -> u32`
  body folded
  13:11 local `proofs`: std::vec::Vec<__locus::Erased>
  14:5 method `fold` resolved to `<std::vec::IntoIter<__locus::Erased> as std::iter::Iterator>::fold::<u32, fn(u32, __locus::Erased) -> u32 {halve}>`
  [...]
  14:33 function `halve` with a marker parameter used as a value, as `fn(u32, __locus::Erased) -> u32 {halve}`
== 07-method-order
  body call
  21:19 local `h`: __locus::Erased
  22:5 method `go` resolved to `<Thing as Go>::go`
    self `x`: declared Self: runtime
    arg 1 `1`: declared u32: runtime
    arg 2 `h`: no parameter
  [...]
  rustc exit status: 101; errors: E0061
```

## Findings

1. **Hard cases: one text works. The runtime version needed no edit beyond the library swap, and lowering needed no type information.**
   - Every case compiled against both versions of the library, or failed with the same error codes in both, except where an observation or probe asks something of rustc (finding 10).
   - Lowering decided everything from syntax: which parameters are ghost, from `@P` and logical type paths; holes; Locus macro names; and which bindings are lexically ghost.
   - The receiver's type, the callee's signature, and which method was selected were all rustc's. Case 01's chain resolved to `Gate::admit`, and `Some(ok)` to `Option::<Erased>::Some`, with no help from lowering.
2. **Function values: rustc rejects some, and Locus must reject the rest from facts.**
   - `let f = halve; f(10, _)` compiles and runs.
   - Coercing `halve` to `fn(u32) -> u32` (E0308) and `[7].map(halve)` (E0593) are rejected by rustc, by arity, because the proof parameter is still there.
   - `black_box(halve)`, `let f: fn(u32, @P) -> u32 = halve`, and `fold(10, halve)` compile. rustc cannot reject them, so Locus must, under RFC 0002's boundary rule 2. The driver reports each one as a function with a marker parameter used as a value, with the type it becomes.
   - In the same way, rustc accepts the unannotated `Vec::new()` with ghost pushes, and `Vec<Nat>` used as `Vec<Seq<Int>>`, because both are `Vec<ErasedData>`. Locus must reject both from its own logical types: the first is the decided `types-not-written.reject.lc`.
3. **The two traps now fail loudly.** In the old design's texts, the method-order trap called the trait method (`101`), and the `()` trap instantiated `make::<()>`. With the ghost argument kept, rustc rejects them with E0061 and with `` `Erased: Default` `` (E0277). The facts show what happened behind the first error: the call resolved to `<Thing as Go>::go`, and `h` fills no parameter.
4. **Marker traits.** rustc itself rejects `==` on `Vec<Nat>` (E0369), `derive(PartialEq, Hash)` over a `Nat` field (E0369 and E0277), and `derive(Default)` over a proof field (E0277). All eight derives on a struct with a proof field compile and work: `w == w` is true, a `HashSet` holds it once, and `Debug` shows `ok: Erased`.
5. **Identical tokens: yes.** The review's `#[token_sensitive]` returned 0 and 1 for the old design's two texts, and 0 in both programs of the one text.
   - It sees the lowered tokens, such as `let h = ::__locus::proof!([&x] [] x == x);`, in both programs, not the source's `prove!(x == x)`.
   - An attribute macro always sees lowered `@P`, because rustc must parse the item before it can pass it on.
6. **Locus macros that rustc expands work in every mode for the decided examples, except (a) and (c).**
   - (a), observing whole roots only, rejects the decided `macro-in-vec-reads-sibling-field.accept.lc` with E0502.
   - (c), rewriting invocations inside other macros' input, makes `stringify!(prove!(x > 0))` give `"::__locus::proof!([&x] [] x > 0)"`, against the decided `prove-in-stringify.accept.lc`. It also cannot reach an invocation in a macro's body that reads a metavariable: `snapshot-through-own-macro.c.rs` gives E0609.
   - The hard case compiles under (b), (c), (d), and the combination, and as expected gives E0609 when every root is observed precisely.
7. **(b) works when positions are where tokens were written.** That holds for identifiers passed into `vec![..]` and through the program's own `macro_rules!`: they keep their positions, shown above.
   - Tokens written in a macro's body carry the body's positions. This agrees with `macro_rules!` hygiene for locals, so (b) gets `snapshot-macro-body` right.
   - (b) fails when another macro rebuilds the tokens. Under `#[respan]` every root is at `10:1`, outside every scope, and the result is E0609.
   - It needs only stable APIs: `line`, `column` and `local_file`. But the table is a file that rustc does not track, because `proc_macro::tracked::path` is unstable.
8. **(d) needs no side channel, but it follows the hygiene of macro names, not of locals.**
   - A local `macro_rules! __locus_ghosts`, resolved with the root's span, handles shadowing, the program's own macros, and respanned tokens.
   - It fails in `snapshot-macro-body`. A name in a `macro_rules!` body resolves the dispatch macro where the macro is invoked, under the runtime shadow, but resolves the local where the macro is defined: E0609.
   - It also needs a reset at the top of each nested `fn`, because a local `macro_rules!` is visible in nested items and a local binding is not. Without it, `snapshot-nested-fn.noreset` gives E0502.
   - **Combined**, an option (e): use the table when the root's `Span::source_text()` (stable) is the root's own text, and dispatch otherwise. It passed every row here.
9. **Every misclassification failed loudly.**
   - A ghost root observed precisely is a field access on a marker, which has no fields, so it is always E0609.
   - A runtime root observed whole borrows a prefix of the place, so it conflicts wherever the precise borrow does. Here that was E0502.
   - No mode compiled any case with a different meaning. The choice between (b), (d), and the combination only decides which correct programs are accepted.
10. **Borrow checking.**
    - In every case and every mode where the checking version compiled, the runtime version compiled too: 30 cases and 78 rows of question 3.
    - The checking version rejects logical code that reads a moved value (E0382) or a mutably borrowed field (E0502), both written directly (12, 13b) and inside `vec![..]` (`macro-in-vec-after-move`, `macro-in-vec-reads-borrowed-field`).
    - A sibling field and a two-phase borrow are accepted.
    - The converse does not hold, and that direction is safe. Case 15 compiles only in the runtime version: an observation asks for `p`'s type where the logical code reads `p.a`, as a runtime `p.a` there would (E0282).
    - Case 11c does too, because of a probe's form. Writing every type argument of a stub probe as `ErasedData` (11d) avoids that without reading the annotation.
11. **Facts: yes, from typeck results alone, after expansion, even when typeck reports errors.** For each call and method call, the driver printed:
    - the callee with its generic arguments (`Gate::admit`, `identity::<Erased>`, `<Thing as Go>::go`);
    - for each argument, the parameter's declared type, marking `Erased` parameters as logical (`pos`, `ok`) and generic parameters that markers flow into (`Some::<Erased>`, `Vec::<(usize, Erased)>::push`);
    - each local holding a marker;
    - each function with a marker parameter used as a value.

    The driver skips desugared code, so the `for` loop's `IntoIterator` calls are not shown; the pattern's `ok: Erased` is. It needed one API change from the earlier spike's driver: `type_of(..).instantiate_identity()` now returns `Unnormalized<..>`.

## Open problems

These need the owner, or more work:

- **A hole inside another macro's input is never a hole.** `vec![_]` reaches rustc as `_`, and both versions fail with rustc's error. Lowering leaves macro input alone, as `tokens-in-macro-input.accept.lc` requires, and a hole is not a macro. Either `vec![_]` is an error that suggests `prove!()`, or Locus needs a hole spelled as a macro. No decided example covers it.
- **Choosing (b), (d), or the combination.**
  - (b) reads a file that rustc does not track, and depends on spans that macros may rebuild.
  - (d) inserts `macro_rules!` statements that attribute macros see, in both programs.
  - The combination has both costs.
  - All three fail loudly where they fail.
- **Lints.** Runtime values read only by logical code (`k`, `m`, and `x` in a signature's proposition) warn as unused in the runtime version, which RFC 0001 treats as the real lints. Lowering can mark those it sees. It cannot see reads in invocations that rustc expands.
- **Logical code in an expanded invocation must reach Locus.** The proc macro observes places, but nothing here reports the logical code itself, with its spans, to the driver. That was not tested.
- **Diagnostics.** Under (d), a borrow error points into lowering's `macro_rules!` text (`9:162` in `snapshot-nested-fn.noreset`). Under `#[respan]`, every error points at the attribute (`10:1`). Origin maps have to cover both.

Not tested:

- Cargo builds, modules other than the crate root, and the real `prelude_import` prelude. The trailing `#[macro_use] extern crate` line stands in for it.
- Overloaded `Deref` in a place: lowering cannot tell `rc.a` from `bx.a`, so only the facts could.
- Snapshots versus models (`snap(x)`): only places are observed.
- Closures, async blocks, and export mode.
- Nightly: everything ran on stable 1.97.
- Real lowering from `.lc`: every lowered text was written by hand.
