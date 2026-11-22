# Spike: typing skeletons, so that rustc types logical code

A throwaway experiment for the draft of RFC 0006 on traits, after [`marker-lowering`](../marker-lowering/README.md). It was built outside the repository, and depends on nothing in the repository when it is built: what it reuses from `marker-lowering`'s first mode is copied. Nothing may depend on it. (The links assume it is placed under `spikes/`.)

## Question

RFC 0001 never gives logical code to rustc as code: its tokens travel inside a library macro, and Locus types it itself, after rustc. The draft of RFC 0006 gives each logical type a zero-sized Rust type of its own, and `marker-lowering` showed what that leaves open: the ghost local whose type nobody wrote.

```rust
let before = logic! { v.len() };     // rustc must know this is a `Nat`, and the macro cannot say so
```

Its first mode guesses. A ghost local that only logical code mentions holds an untyped marker, and otherwise rustc infers its type from uses in runtime code. The guess has gaps: a plain copy, a tuple component, and a use inside a format string that is missed silently.

**What if rustc sees inside logical code too?** Lowering translates logical code into a Rust expression over stubs and zero-sized types, a *typing skeleton*, and puts it in a closure that is never called. rustc types it, resolves its names and its traits, and gives the ghost local its type, in both programs, with no guess and no annotation.

1. How much of real logical code can be given a skeleton that rustc types, by a translation that reads only tokens?
2. What does the translation have to insert or encode, where Locus's logical code and Rust's typing differ?
3. Where does Rust's typing give a different answer than the source means?
4. Where logical code has no skeleton, can the first mode's guess stand, with a check after rustc that closes its silent gap?
5. Can `Snap<T>` be a type family that rustc normalizes, in place of one marker for every snapshot?
6. Do the two programs still resolve every call alike?
7. What do type errors in logical code look like, and what does it cost?

## Setup

A Cargo workspace of its own, stable rustc 1.97.0 and cargo 1.97.0, proc-macro2 1.0.107. All outputs go under `target/` and `out/`, which `.gitignore` excludes. `run.sh` reproduces everything, and `run.log` is the output of the run that the results below come from.

- `locus-macros/` is `locus!`, the first mode's token rewriter with two additions: `src/skeleton.rs`, about 1,100 lines, which parses logical code (a small precedence parser over tokens) and writes its skeleton, and about 700 lines in `src/lib.rs` that call it and lower types differently. It still writes one text, the same under both `cfg`s.
- `locus/` is the library. `sk` holds the stubs that a skeleton is written over. `ModelOf` and `Snapshot` are the two projections. Nothing in `sk` runs, but for the functions that take a skeleton's closure, drop it, and give the marker.
- `driver/` is a rustc driver, about 340 lines, built against `rustc-dev` with `RUSTC_BOOTSTRAP=1`. Cargo runs it in place of rustc. It checks the fallback (question 4) and lists calls (question 6). Its call listing is copied from `marker-lowering`'s.
- `first-mode/` is the library and macro of `marker-lowering`, copied unchanged, as a workspace of its own over the same source files, for comparison.
- Five sets of programs, one binary each, 178 files:
  - `corpus/`, 26 files: 22 litmus examples and 4 others, as in `marker-lowering`.
  - `ghost-lets/`, 17 files: the litmus examples with an unannotated ghost `let`.
  - `cases/`, 58 files, and `more/`, 39 files: `marker-lowering`'s own cases. `more/` was written for its second mode, and its `expect` lines are not used here.
  - `skel/`, 38 files, written for this spike: one for each construct (`a`), deliberate type errors (`e`), places where Rust's typing and Locus's rules may part (`x`), snapshots (`c`), and the fallback (`f`). The first line of each says what the two compilations must do, by Locus's rules and not by what this toy was seen to do.
- `scale/` is one generated file of 500 functions, for the cost. `probes/` are six hand-written Rust files that tried an encoding before any macro wrote it, or after one failed.
- `tools/` are the Python scripts that `run.sh` calls. `measure.py` builds a set in both expansions and reads rustc's JSON; `summary.py` counts; `compare.py` compares two runs; `cost.py` times builds.

### What `locus!` writes

A piece of logical code is one of: the contents of a `logic!`, a `prove!` or a `prop!`; a proposition `@(..)` in a `let` annotation; the body of a `logic fn`; an arm of a `prop` declaration; and, beyond what the brief asked for, a proposition in the signature of a function that has a body. For each, `locus!` writes the skeleton beside what the first mode wrote. A piece with a skeleton needs no observations and no probes: its closure borrows the places it reads, and rustc resolves the names in it, in both expansions.

| Where | What is written |
|---|---|
| `let x = logic! { code };` | `let x = ::locus::skeleton!(typed { skeleton } code);`, which is `::locus::sk::typed(\|\| { skeleton })`. `typed<G: Logical>(_: impl FnOnce() -> G) -> G` gives the marker of the skeleton's type |
| `prove!(p)`, `prop!(p)` | the same with `sk::proved`, `sk::prop_value`: an `Erased`, a `Prop` |
| `let h: @(p) = ..;` | after the `let`, `::locus::sk::holds(\|\| { skeleton });`. A named component of the annotation is the closure's parameter: `holds1(\|i: usize\| ..)` for `Vec<(i: usize, @(i < n))>` |
| `logic fn f(..) -> T { body }` | the stub's body is `::locus::sk::body(\|\| { skeleton })`, typed against the signature |
| `fn f(a: A, h: @(p)) -> (r: R, @(q)) { .. }` | at the start of the body, `sk::holds(\|\| ..);` for `p` and `sk::holds1(\|r: R\| ..);` for `q` |

The translation, from tokens alone. It never knows a type, and never knows whether a name is a runtime local, a ghost local, or a binder of the logical code itself:

| Logical code | Skeleton |
|---|---|
| A place read as a value: `x`, `p.a`, `*r`, `LIMIT` | `sk::model_of(&x)`: the type's model, by the projection `ModelOf`; a logical type is its own |
| A place as an argument, a field's initializer, or the result of a logic function | `sk::arg(&x)`: rustc chooses between the place itself and its model, by the type that is wanted |
| `snap(x)` | `sk::snap(&x)`, of type `Snap<T>` |
| `final!(r)` | the place `*sk::final_of(&r)`, of `r`'s target type, then read as any place |
| `f(a)`, `geometry::area(a)` | kept: the stub has that name |
| `T::le(a, b)`, `Rational::le(a, b)`, `<T as Tr>::le(a, b)`, `x.len()` | `T::__logic_le(..)`, `<T as Tr>::__logic_le(..)`, `sk::model_of(&x).__logic_len()` |
| `p.first`, `Pair { first: a, second: b }` | kept: the lowered struct has its fields |
| `Tree::Node(l, 0, r)`, `Tree::Leaf` | kept: a stub function and a constant of the lowered type |
| `match t { Tree::Leaf => a, Tree::Node(l, _, r) => b }` | `match sk::view(t) { __locus_view_Tree::Leaf => a, __locus_view_Tree::Node(l, _, r) => b }` |
| `a + b`, `a - b`, `a * b`, `a / b`, `a % b`, `-a` | `sk::add(a, b)`, ..: bounded by the library's logical traits, `AddLogic<Rhs>` and its like |
| `a == b`, `a != b`, `a < b`, .. | `sk::eq(a, b)`, `sk::lt(a, b)`, ..: a `Bool` |
| `a === b`, `a !== b` | `sk::same(a, b)`, `sk::differ(a, b)`: two values of one type, a `Prop` |
| `a && b`, `a \|\| b`, `!a`, `a ==> b`, `a <==> b` | `sk::and(a, b)`, .., `sk::implies(a, b)`, `sk::iff(a, b)` |
| `if c { a } else { b }` | `if sk::cond(c) { a } else { b }` |
| `3` | `sk::lit(3)`; beside another operand, `sk::lit_like(&that, 3)`; `sk::lit::<Int>(3)` where both operands are literals, and for `-3` |
| `xs[i]` | `sk::index(xs, i)` |
| `forall \|x: T\| p`, `exists \|x: T, y: U\| p` | `sk::forall1(\|x: T\| p)`, `sk::exists2(\|x: T, y: U\| p)` |
| `\|x: T\| proof` | `sk::lambda1(\|x: T\| proof)`, an `Erased` |
| `_`, `prove!(p)`, a lemma call | `sk::hole()`, `sk::proved(\|\| p)`, the call |
| `match h { Even::Step(m) @ And::Intro(a, _) => e }` | `match sk::view(h) { __locus_view_Even::Step(m) => { let a = Erased; e } }` |

What it does not translate is an error that names the construct, and that piece keeps the first mode's lowering: a cast with `as`, a borrow `&`, `if let`, a statement that is not a `let`, an array, a literal pattern, a variant pattern inside a variant pattern, a closure parameter with no type or a closure of more than three parameters, `old(..)`, a macro it does not know, and a read of a ghost local that was left untyped.

What it wrote for two functions of the corpus (from `./run.sh expansions`, `::locus::` and lint attributes removed):

```rust
fn size(t: Tree) -> Nat {
    sk::body(|| match sk::view(sk::model_of(&t)) {
        __locus_view_Tree::Leaf => sk::lit(0),
        __locus_view_Tree::Node(left, _, right) => sk::add(
            {
                let __locus_l = size(sk::arg(&left));
                let __locus_r = sk::lit_like(&__locus_l, 1);
                sk::add(__locus_l, __locus_r)
            },
            size(sk::arg(&right)),
        ),
    })
}

fn first_mut(pair: &mut (u32, u32)) -> (&mut u32, Proof![r == pair.0 && final!(pair) === (final!(r), pair.1)]) {
    sk::holds1(|r: &mut u32| {
        sk::and(
            sk::eq(sk::model_of(&r), sk::model_of(&pair.0)),
            sk::same(
                sk::model_of(&(*sk::final_of(&pair))),
                (sk::model_of(&(*sk::final_of(&r))), sk::model_of(&pair.1)),
            ),
        )
    });
    (&mut pair.0, proof!([] [] []))
}
```

### Logical types, models and snapshots

- **A `#[derive(Logical)]` struct has its fields.** Each field's type is zero-sized, so the struct is, and rustc types `p.first` and `Pair { .. }`. `Copy` now asks each type parameter to be `Logical`.
- **A `#[derive(Logical)]` enum is a zero-sized struct with no payload, and its variants are those of a view enum beside it,** `__locus_view_Tree`, reached at a `match` through `sk::view`. Two reasons, both checked by hand first (`probes/p2.rs`, `p3.rs`): Rust rejects a recursive type with no indirection even when it is zero-sized (E0072), and a Rust enum with two variants has a tag, one byte, in a runtime struct too. A variant with a payload is constructed by a stub function of the type, as in the first mode, and a unit variant is an associated constant. A `prop` declaration gets a view too, for a `match` on its proofs.
- **`ModelOf`** is implemented by the library for the machine integers (`Nat`, `Int`), `bool`, references and `Box` (transparent), `Vec`, slices and arrays (`Seq` of the element's model), and tuples; and by every logical type for itself. `Logical` has it as a supertrait, `ModelOf<Logic = Self>`, so that generic code normalizes. A type with no model has no implementation.
- **`Snap<T>` is `<T as Snapshot>::S`.** A machine integer has a bounded type of its own (`BoundedU32`), `bool` is `Bool`, a reference and a `Box` are their target's snapshot, `Vec<T>`, `[T]` and `[T; N]` are `Seq<Snap<T>>`, a tuple is the tuple of snapshots, a logical type is its own (`Logical: Snapshot<S = Self>`), and `Cell`, `RefCell`, `Option`, `String` and raw pointers have opaque ones. For each struct, enum or union of the program `locus!` writes `__locus_snap_Name`, zero-sized, with no fields and no lifetime, generic over the *snapshots* of the type's type parameters, and `impl<'a, T: Snapshot> Snapshot for Name<'a, T> { type S = __locus_snap_Name<Snap<T>>; }`.

### Switches

| Switch | What it does |
|---|---|
| feature `homogeneous-ops` of `locus` | takes away the operators on an `Int` and a `Nat` |
| feature `widen-args` of `locus` | a `Nat` place may be given where an `Int` is wanted |
| `LOCUS_SPIKE_LITERAL_INT=1` | a `logic!` that is one integer literal, in a `let` with no annotation, is an `Int` |
| `LOCUS_SPIKE_IN_TRAIT=1` | a trait's stubs are items of the lowered trait, as in `marker-lowering` |
| `LOCUS_SPIKE_NO_SKELETON=1`, `#![spike(no_skeleton)]` | no piece gets a skeleton |
| `LOCUS_SPIKE_FALLBACK_UNTYPED=1` | a ghost local whose initializer has no skeleton is left untyped whatever mentions it |
| `LOCUS_CHECK_UNTYPED=1`, `LOCUS_CALLS_DIR=dir` | the driver's check, and its listing |

### How pieces are counted

With `LOCUS_SPIKE_PIECES=dir`, `locus!` records each piece: its kind, where it is, and whether it was translated. `measure.py` gives each of rustc's errors to the innermost piece that contains its primary span. So "no error" means that rustc reported none inside the piece in either expansion, in a build of the whole file. An error in one piece can hide another's, and rustc does not report an ambiguity where a function already has another error.

## Results

Run on 2026-11-22. `./run.sh` took 333 seconds with `target/` already built. (The last lines of `run.log` were appended afterwards, and say so: `./run.sh probes` run once more after `probes/p6.rs` was added, and two messages that `./run.sh diagnostics` did not print yet.) Its summary lines:

```text
## corpus (first): 26 files, 23 compile in both expansions
  accepted litmus examples: 20; compile in both expansions: 20
  rejected litmus examples: 2; rustc rejects them in both expansions: 0; in the checking expansion alone: 2
## ghost-lets (first): 17 files, 12 compile in both expansions
  accepted litmus examples: 10; compile in both expansions: 8
  rejected litmus examples: 7; rustc rejects them in both expansions: 1; in the checking expansion alone: 2
## cases (first): 58 files, 29 compile in both expansions
## corpus (sk): 26 files, 23 compile in both expansions
  accepted litmus examples: 20; compile in both expansions: 20
  rejected litmus examples: 2; rustc rejects them in both expansions: 2; in the checking expansion alone: 0
  all                          pieces  51  translated  51  no error in either expansion  49  with an error   2
## ghost-lets (sk): 17 files, 11 compile in both expansions
  accepted litmus examples: 10; compile in both expansions: 7
  rejected litmus examples: 7; rustc rejects them in both expansions: 3; in the checking expansion alone: 0
  all                          pieces  31  translated  30  no error in either expansion  25  with an error   5
## cases (sk): 58 files, 29 compile in both expansions
  all                          pieces 184  translated 175  no error in either expansion 167  with an error   8
## skel (sk): 38 files, 16 compile in both expansions
  all                          pieces 135  translated 128  no error in either expansion 103  with an error  25
## more (sk): 39 files, 21 compile in both expansions
  all                          pieces  53  translated  52  no error in either expansion  45  with an error   7
corpus: 26 files; compile in both expansions: first 23, skeleton 23; outcome or output differs in 2
ghost-lets: 17 files; compile in both expansions: first 12, skeleton 11; outcome or output differs in 5
cases: 58 files; compile in both expansions: first 29, skeleton 29; outcome or output differs in 19
corpus: binaries that both expansions accept: 23; calls, method calls and constants listed in each: 55; binaries with one resolved differently: 0
ghost-lets: binaries that both expansions accept: 11; calls, method calls and constants listed in each: 16; binaries with one resolved differently: 0
cases: binaries that both expansions accept: 29; calls, method calls and constants listed in each: 133; binaries with one resolved differently: 0
skel: binaries that both expansions accept: 16; calls, method calls and constants listed in each: 114; binaries with one resolved differently: 0
more: binaries that both expansions accept: 21; calls, method calls and constants listed in each: 69; binaries with one resolved differently: 0
```

Every binary that compiled both ways printed `expansion: runtime` from one build and `expansion: checking` from the other, and the same lines after that. No stub ran: the 200 binaries of the five sets were run once more by hand, and each exits with success. (`measure.py` now reports a binary that does not. The run in `run.log` did not yet check it; a later `./run.sh coverage` did, and reported none.)

### Coverage

**In the corpus, every one of the 51 pieces of logical code has a skeleton, and rustc types all of them.** The two with an error are the two rejected litmus examples, and the error is the one they are there for (below). No source was changed, and no annotation was added.

| Set | Files | Pieces | Translated | No error | With an error | Not translated |
|---|---|---|---|---|---|---|
| `corpus` | 26 | 51 | 51 | 49 | 2 | 0 |
| `ghost-lets` | 17 | 31 | 30 | 25 | 5 | 1 |
| `cases` | 58 | 184 | 175 | 167 | 8 | 9 |
| `more` | 39 | 53 | 52 | 45 | 7 | 1 |
| `skel` | 38 | 135 | 128 | 103 | 25 | 7 |

By kind, in the corpus: 13 `logic!`, 11 bodies of logic functions, 11 propositions in `let` annotations, 13 propositions in signatures, 2 arms of a `prop`, and 1 `prove!` that rustc expands inside `vec![..]`. One proposition in a struct field's type was not attempted.

**The pieces that were not translated,** in the four sets that this spike did not write:

- `gl-logic-calls-runtime-function`, `logic! { count(&v) }`: a borrow `&`. It is a rejected litmus example, and logical code has no borrows.
- 10 propositions in the signatures of trait methods that have no body: nowhere to hold the skeleton. 9 are in `cases` (`t01`, `t02`, `t03`, `t07`, `t08`, `t09`) and 1 is in `more` (`r11`).

In `skel`, 5 are casts written to force the fallback, 1 reads a local left untyped, and 1 is a variant pattern inside a variant pattern (`x08`).

**The translated pieces in which rustc reports an error,** in the same four sets, each in both expansions:

| Piece | Error | Why |
|---|---|---|
| `after-move`, `gl-after-move`: `logic! { v.len() }` | E0382 | Meant: logical code reads `v` after it moved |
| `during-mutable-borrow`, `gl-during-mutable-borrow` | E0502 | Meant: logical code reads `v` while it is mutably borrowed |
| `g20`: a `logic!` where a `usize` is wanted | E0308 | Meant |
| `s03`: `is_max(flag, ..)` for a `Bool`, which has no `OrderSpec` | E0277 | Meant by the first mode's rules: rustc now checks the bound of a logical call |
| `gl-snapshot-with-associated-type-field`, 2 pieces: `snap(h)` for `h: Holder<F>` | E0277, `F` has no snapshot type | The family needs `F: Snapshot` ([below](#snapshots-as-a-type-family)) |
| `gl-hole-written-out-with-an-earlier-value`: `u32::wrap(n + 1)` in a signature | E0599 | The library has no stub for a logic function of a primitive type |
| `g11`: `prove!(seen == v.len())`, after `let mut seen = logic! { 0 };`; `s21`: `keep(logic! { 5 })` | E0283 | A literal alone ([below](#where-rusts-typing-gives-a-different-answer)) |
| `t04`: `Rational { num: t, den: 1 }` for `t: Nat`; `r04`, `r05`: `{ t }` for `t: Nat` where an `Int` results; `r02`: `{ x }` for `x: u32` where an `Int` results | E0308, E0271 | A `Nat` where an `Int` is wanted |
| `t09`: `fold(Seq::empty()) + Int::twice(three_i)` | E0283 | An operand whose type nothing gives |
| `t13c`, `t13d`, `t15`, `t17`, `s16` | E0599, E0405, E0782 | The first mode's hidden trait for a trait's stubs, not the skeleton: all five compile with `LOCUS_SPIKE_IN_TRAIT=1` |
| `r06`: `a < b` for two `Snap<u32>` | E0277 | The bounded integers have no operators here |

### What the translation inserts or encodes

Each is a place where Locus's logical code and Rust's typing differ, and what bridges it.

1. **The model.** A runtime place read as a value is its type's model. Bridge: every read of a place is `sk::model_of(&place)`, whose result is the projection `<T as ModelOf>::Logic`. Logical types are their own model, so the translation need not know which names are ghost. `Logical: ModelOf<Logic = Self>` lets rustc normalize the projection for a type parameter.
2. **A place where a type is wanted.** A logic function declared for a runtime type takes the runtime value (`T::le(*a, *b)` in the draft's `OrderSpec`), and one declared on `Nat` takes its model (`double(v.len())`, `double(n)` for `n: u32`). The translation does not see the parameter. Bridge: `sk::arg(&place)`, with a result type `P` that rustc infers from the parameter, bounded by `ArgAs<P, Via>`. Two implementations, the place itself and its model, are told apart by a marker type in `Via`, since rustc cannot see that `T` and `<T as ModelOf>::Logic` differ. A marker trait `Runtime` keeps logical types out of the second, so that a logical argument is only ever itself.
3. **A place is kept as a Rust place.** `p.a`, `*r`, `pair.0` are typed by rustc as Rust types them, on the runtime type, and only the whole is taken to a model. So `c.hits + c.limit` adds a `u32` field and a `u8` field as two `Nat`s. `final!(r)` is a stub that returns a reference to `r`'s target type, never run, so that `final!(r).count` is a Rust field access too.
4. **`Bool` where Rust wants `bool`.** `if` in logical code is `if sk::cond(c)`. Rust's `==` and `<` give `bool` whatever the implementation, so every comparison is a stub that gives `Bool`.
5. **A `Bool` used as a proposition.** Where the source shows a proposition (`@(..)`, `prove!`, `prop!`, a quantifier's body, `==>`, a logic function that returns `Prop`), the stub takes `impl Truth`, which `Bool` and `Prop` implement. `&&` and `||` have an output type: `Bool` on two `Bool`s and `Prop` otherwise.
6. **Operators that are not Rust's.** `===`, `!==`, `==>` and `<==>` reach a macro as two or three Rust tokens. The translation parses them, with a precedence of its own. All operators are stub calls, not Rust's operator traits on the markers: half of them could not be Rust's anyway, `Add::add` is a method that runs, which RFC 0006's draft forbids on a logical type, and a stub's bound can carry a message.
7. **Integer literals.** `sk::lit(3)` is generic over `Int` and `Nat`, and rustc infers which from a parameter, a field, or an annotation. Rust has no default for a literal of a type that is not its own, so two more rules are syntactic: beside another operand a literal has that operand's type (`lit_like`), and where both operands are literals, or under a minus, it is an `Int`.
8. **A type that contains itself, and a zero-sized enum.** The view enum, above. A `match` in logical code is on `sk::view(x)`, and the paths in its patterns are rewritten to name the view.
9. **Members of types and traits.** A logic function of a type or a trait has a mangled stub, so the translation writes `T::__logic_le` and `.__logic_len()`. GUESS: a capital on the segment before the last.
10. **Named components.** `(i: usize, @(i < n))` binds `i` in a type. The skeleton binds it as a closure's parameter, with the type the source wrote.
11. **Holes and proofs.** `_` is `sk::hole::<T>()` for whatever `T` is wanted, since a hole may stand for a value: in `recover(_, about_the_end)` it is a `Nat`. A proof pattern after `@` binds each of its names to `Erased`.
12. **Quantifiers.** A closure, typed by the parameter types the source must write. The number of parameters is in the stub's name.
13. **"A `Nat` is used as an `Int` wherever an `Int` is expected."** Rust has no conversion between two types of a library. Bridge: one more implementation for each place it may happen. The library has the operators on an `Int` and a `Nat`. A `Nat` place given where an `Int` is wanted is the feature `widen-args`, off by default, because of what it costs ([below](#where-rusts-typing-gives-a-different-answer)).
14. **Borrows.** The closure of a skeleton borrows each place it reads, shared, and is dropped at once. That is the observation of RFC 0001, written by rustc's own capture rules, and it is in both expansions.

### Where Rust's typing gives a different answer

Each `x` case says in its first line what Locus's rules give. "Default" is what the two expansions did.

| Case | The source | Locus's rules | Default | With a switch |
|---|---|---|---|---|
| `x01`, `g11`, `s21` | `let zero = logic! { 0 };`, read only by logical code | `Int`: nothing is expected of the literal | E0283, at the `let` | `LITERAL_INT`: `x01` compiles, as an `Int`. `g11` is then E0308 in its second function, which is the next row |
| `x02`, `g05`, `g11` | `let zero = logic! { 0 };` then `takes_nat(zero)` in runtime code | `Int`, read forward, and the call is an error | compiles: rustc infers `Nat` backward from the later use | `LITERAL_INT`: E0308, `Int` where `Nat` is wanted |
| `x03`, `t04`, `r02`, `r04`, `r05` | a `Nat` place as an `Int` argument, field, or result | accepted | E0308, E0271 | `widen-args`: `x03`, `r02`, `r04`, `r05` compile. `t04`'s field is accepted, and it fails two rows down |
| `x04` | `negate(v.len())`: a `Nat` that is not a place, as an `Int` argument | accepted | E0308 | none: no stub stands around a call's result |
| `x05` | `same(n)` for `n: u32` and `logic fn same<T>(t: T)` | `T` is `Nat`: an argument of a logical parameter is the place's model | E0283: the place itself fits as well | none |
| `t04`, `a04` | `widen(n)` for `n: Nat` and a type parameter | `T` is `Nat` | compiles | `widen-args`: E0283, `Nat` and `Int` both fit |
| `x06` | `length(*v)` for `logic fn length(s: Snap<Vec<u32>>)`; `let s: Snap<u8> = logic! { b };` | the place stays a snapshot where one is expected | E0271, E0308: the skeleton gives the model | none |
| `x07`, `t09` | `fold(Seq::empty()) + i` for `i: Int` | `Int` is the one `Monoid` that adds to an `Int` | E0283 | none: rustc selects no implementation for a type it does not know |
| `x10` | `twice(x > 0)` for `logic fn twice(p: Prop)` | a `Bool` is a proposition | E0308 | none: a parameter's type is not seen |
| `x11` | `s.room()` for `impl Stack { logic fn room(self) -> Nat }`, a runtime struct | found on the runtime type first, then on its model | E0277, `Stack` has no model | none. `Stack::room(*s)` in the same case compiles |
| `x08` | `Tree::Node(Tree::Leaf, n, _)` in a `match` | accepted | compiles, with no skeleton for that function | none: a view's payload is the logical type, which has no variants for rustc |
| `x14` | `use shapes::Tree;` and then `match t { Tree::Leaf => .. }` | accepted | E0433, ``cannot find module or crate `__locus_view_Tree` in this scope`` | none: the import does not bring the view. The same `match` written with `super::shapes::Tree::Leaf` compiles |

Three notes on these.

- **The literal.** rustc's own message for `x01` asks for a type on the user's `let`, and names the encoding (its first lines):

  ```text
  error[E0283]: type annotations needed
     --> skel/src/bin/x01-literal-alone.rs:13:13
      |
   13 |         let zero = logic! { 0 };
      |             ^^^^            - type must be known at this point
      |
      = note: the type must implement `locus::sk::Literal`
  ```

  `g05` is the first spike's case for a mutable borrow of a ghost local: `let mut seen = logic! { 0 };`, then `advance(&mut seen, 1)` for `fn advance(n: &mut Nat, ..)`. It compiles because rustc types `seen` backward. With `LOCUS_SPIKE_LITERAL_INT=1` it is E0308 at the call, which is what a forward reading of RFC 0002's rule gives.
- **The two argument ambiguities are one mechanism.** Every route by which a place may become an argument is one more implementation of `ArgAs`, and where the parameter is a type parameter, nothing chooses among them. With the place itself and its model, a runtime place with a model is ambiguous (`x05`). Add `Nat` to `Int`, and a `Nat` place is too (`t04`, `a04`). A third route for snapshots was tried by hand (`probes/p5.rs`): it works where the model and the snapshot are different types, and is E0283 where they are one, as for `bool`.
- **Checked the same by both.** rustc rejects a `match` that leaves a variant out, on the view (`x09`, E0004), a value of a type with no model used as a value (`x12`, E0277, with the message the library gives the bound), and a snapshot of one type where another's is wanted (`g18`, E0308).

### Ghost locals

A ghost `let` initialized by a `logic!`, by where its type comes from:

| Set | From its skeleton | From the fallback | rustc asks for a type |
|---|---|---|---|
| `corpus` | 13 (5 have an annotation) | 0 | 0 |
| `ghost-lets` | 18 | 1, left untyped (`count(&v)`) | 0 |
| `cases` | 52 (10 have an annotation) | 0 | 2: `g11` line 13, `t09` line 48 |

- **None of the litmus examples needs an annotation, as before.**
- **The three programs that the first mode got wrong are right.** `g13`, a ghost local that is only copied, and `g14`, a ghost component of a runtime tuple, compile with no annotation, where the first mode had E0282. `g15`, the format string, prints `"Nat"`, where the first mode silently printed `"Untyped"`.
- **`g06`, passed only to a generic function, compiles,** where the draft and the first mode ask for an annotation. `g16`, passed only to `fn only<M: Monoid>`, is now E0277, `Nat: Monoid`: the type that the program would have had to write was never going to fit.
- **Two ask for a type that they did not need before.** `g11`'s `logic! { 0 }`, above. `t09`'s `let total: Int = logic! { fold(Seq::empty()) + Int::twice(three_i) };` has an annotation, and what rustc wants is the element type of `Seq::empty()`, inside the logical code. The first mode compiled both, because it typed no logical code.

### Fallback, checked

Where an initializer has no skeleton, the first mode's lowering stands, and so does its gap. The driver closes it: after rustc, it reports every use of a local whose type holds the untyped marker, unless the use is inside what a macro of the library wrote, or is the target of a ghost statement. The `f` cases are `g13`, `g14` and `g15` with an initializer that has no skeleton, `logic! { v.len() as Int }`.

```text
== f15-format-capture: expect runtime=reject check=reject; got runtime=error check=error; as expected
  runtime: 11:19: error[error]: locus: the ghost local `before` has no logical type for rustc, and it is used here outside logical code: its type is `locus::Untyped`. Write the type
```

| Case | The first mode's guess, checked | Every such local left untyped, checked |
|---|---|---|
| `f15`, `g15`: `format!("{before:?}")` | **caught by the driver**, at the name inside the string. rustc alone accepts | caught by the driver |
| `f13`, `g13`: `let again = before;` | rustc: "type annotations needed" (E0283 with skeletons around it, E0282 with none) | **caught by the driver**, at the copy |
| `f14`, `g14`: `let pair = (v.len(), logic! { .. });` | rustc: E0282 | accepted, and prints 3: runtime code uses only `pair.0`, and the driver looks at the component that is used |
| `f17`: the same, then `let copy = pair;` | rustc: E0282 | **caught by the driver**, at the copy, naming `pair` |
| `f16`: read by a `prove!` and by a `prove!` inside `vec![..]` | E0277, `Untyped` has no model | the same |

The originals `g13`, `g14`, `g15` were run with `LOCUS_SPIKE_NO_SKELETON=1`, and behave as their `f` twins.

- **With the first mode's guess the driver has one job,** the silent case. The loud ones stay rustc's.
- **With every such local left untyped, the rule is one line:** a ghost local whose initializer has no skeleton may be mentioned by logical code only. The three shapes then get one error or none, and `g14` needs no annotation.
- **`f16` is not solved.** A `prove!` that rustc expands, inside another macro's input, does not know that a local was left untyped. It writes a skeleton, and the skeleton does not type. The fallback is contagious, and a macro that rustc expands cannot follow it.
- The check added nothing to rustc's errors on any other file of the five sets.

### Snapshots as a type family

- **A ghost snapshot of a `&mut` keeps no borrow** (`g12`, `c01`). `c01` also holds the snapshot of a `Link<'a>` after the struct and what it borrowed are gone. It prints `([1, 2, 3], "locus::Seq<locus::BoundedU32>", "c01_no_borrow::__locus_snap_Link")`.
- **`Snap<Vec<u32>>` passes where `Seq<Snap<u32>>` is wanted** (`c02`), through `fn generic<T: Snapshot>(s: Snap<Vec<T>>) -> Seq<Snap<T>>`, through a slice behind a reference, and for a struct of the program over a reference, `Snap<Pair<&T>>` as `Snap<Pair<T>>`.
- **rustc rejects a snapshot of one type as another's.** `g18`, a `Snap<Vec<u32>>` passed where a `Snap<bool>` is wanted, is E0308, ``expected `Bool`, found `Seq<BoundedU32>` ``. With one marker rustc accepted it.
- **An implementation for `Snap<u32>` is not selected for `Snap<bool>`** (`c03`, `h03`): E0277, ``the trait bound `locus::Bool: Describe` is not satisfied``, with the help ``the trait `Describe` is implemented for `<u32 as Snapshot>::S` ``. It is selected at `Snap<u32>` and at `Snap<&mut Box<u32>>` (`c04`). `h04`, the implementation behind an alias, which the first mode could not reject, compiles as before; whether it is now selected at `Snap<u32>` alone was not checked.
- **But two snapshot types in two headers collide** (`c07`), in a crate that does not declare the family:

  ```text
  error[E0119]: conflicting implementations of trait `Describe` for type `<u32 as Snapshot>::S`
    --> skel/src/bin/c07-two-snap-headers.rs:13:5
     |
  12 |     impl Describe for Snap<u32> { type Width = Nat; }
     |     --------------------------- first implementation here
  13 |     impl Describe for Snap<u8> { type Width = Int; }
     |     ^^^^^^^^^^^^^^^^^^^^^^^^^^ conflicting implementation for `<u32 as Snapshot>::S`
  ```

  The same two implementations in the crate that declares the family compile (`probes/p4.rs`). `r06` shows the same against an implementation for `Nat`.
- **A type with no snapshot implementation is an error where its snapshot is taken** (`c05`, a `BTreeSet<u32>`): E0277. RFC 0002 gives every type a snapshot, opaque unless said otherwise. A family has no default member: an implementation for every type would overlap each of the others.
- **A type parameter with no bound has no snapshot** (`c06`):

  ```text
  error[E0277]: `T` has no snapshot type that rustc can name
    --> skel/src/bin/c06-unbounded-parameter.rs:14:20
     |
  14 |     fn takes<T>(s: Snap<T>) -> u32 { 1 }
     |                    ^^^^^^^ its snapshot is taken here
     |
     = note: a type parameter needs the bound `Snapshot`, and a type of another crate needs the library to declare its snapshot
  help: consider restricting type parameter `T` with trait `Snapshot`
  ```

  With the bound written, the same function compiles. **This is the one accepted litmus example that the first mode compiles and this spike does not:** `snapshot-with-associated-type-field`, `fn kept<F: Family>(h: Holder<F>)` with `snap(h)`. The generated `Snap<Holder<F>>` is `__locus_snap_Holder<Snap<F>>`, which needs `F: Snapshot`. It is generic over `Snap<F>` and not over `F` so that it has none of `F`'s lifetimes.

### Both programs agree

A piece with a skeleton is one text in both expansions, with the same types. The driver lists, for each call, method call and associated constant that the source wrote, the item rustc resolved it to and the implementation it comes from. **Over the 100 binaries that both expansions accept, 387 lines in each listing, no line differs.**

The nine cases of `marker-lowering` that the brief names:

```text
== r01-runtime-wrapper: both expansions compile; 5 lines in each listing, the same; prints ['run() => (1, 2)']
== r02-outside-self: got runtime=E0271 check=E0271: no listing to compare
== r03-description: both expansions compile; 5 lines in each listing, the same; prints ['run() => ((8, 1), (16, 2))']
== r04-blanket-conversions: got runtime=E0308 check=E0308: no listing to compare
== r09-two-expansions-two-impls: both expansions compile; 3 lines in each listing, the same; prints ['run() => "the fallback, for Inner"']
== r10-inherent-beats-trait: both expansions compile; 3 lines in each listing, the same; prints ['run() => 2']
== r11-inherent-beats-trait-proof: both expansions compile; 4 lines in each listing, the same; prints ['run() => "the branch was not taken"']
== r13-autoref-order: both expansions compile; 5 lines in each listing, the same; prints ['run() => (2, 2)']
== r14-associated-constant: both expansions compile; 2 lines in each listing, the same; prints ['run() => 2']
```

`r02` and `r04` do not compile by default, for a `Nat` where an `Int` results. With the feature `widen-args` both compile, print 3 and 7, and their listings are the same in both expansions.

The listing now reaches inside logical code. For `t01`, where the first mode carried `Rational::le_total(half, third)` as tokens:

```text
spec_only::{closure#1}:076:041 => the definition at 29:15 | __locus_logic_OrderSpec::__logic_le_total -> <Rational as __locus_logic_OrderSpec>::__logic_le_total with [Rational]
max::{closure#1}:056:033 => the definition at 13:15 | __locus_logic_OrderSpec::__logic_le_total with [T/#0]
```

Which implementation a logic function comes from, and under which bound, is rustc's answer, with no question asked of a driver.

### Diagnostics

Three deliberate type errors. In each, the two expansions print the same message.

**A `Nat` where a `Seq` is wanted** (`e01`). It points at the user's tokens, and does not mention the encoding, for a place and for a call alike:

```text
error[E0308]: mismatched types
  --> skel/src/bin/e01-nat-for-seq.rs:11:43
   |
11 |         let from_a_place = logic! { count(n) };
   |                                     ----- ^ expected `Seq<Nat>`, found `Nat`
   |                                     |
   |                                     arguments to this function are incorrect
   |
   = note: expected struct `locus::Seq<locus::Nat>`
              found struct `locus::Nat`
note: function defined here
  --> skel/src/bin/e01-nat-for-seq.rs:7:14
   |
 7 |     logic fn count(xs: Seq<Nat>) -> Nat { xs.len() }
   |              ^^^^^ ------------
```

**An unknown field** (`e02`). At the user's token, with rustc's own suggestion from the logical struct's fields, and no mention of the encoding:

```text
error[E0609]: no field `nom` on type `Rational`
  --> skel/src/bin/e02-unknown-field.rs:12:37
   |
12 |         let on_a_local = logic! { r.nom };
   |                                     ^^^ unknown field
   |
help: a field with a similar name exists
   |
12 -         let on_a_local = logic! { r.nom };
12 +         let on_a_local = logic! { r.num };
   |
```

**A missing implementation for an operator** (`e03`). At the user's operator, and it names the encoding: the stub `locus::sk::add`, the trait, and the library's implementations with their lines. For `r < r` the first line is the library's own text, through `#[diagnostic::on_unimplemented]`, and the rest names `locus::sk::OrdLogic` and `locus::sk::lt`. rustc reports `r + r` twice, once for each operand. (Some lines are left out of both messages below.)

```text
error[E0277]: the trait bound `Rational: AddLogic<_>` is not satisfied
   --> skel/src/bin/e03-missing-operator.rs:11:28
    |
 11 |         let sum = logic! { r + r };
    |                            ^ - required by a bound introduced by this call
    |                            |
    |                            unsatisfied trait bound
    |
help: the trait `AddLogic<_>` is not implemented for `Rational`
   --> skel/src/bin/e03-missing-operator.rs:8:12
    |
  8 |     struct Rational { num: Int, den: Int }
    |            ^^^^^^^^
help: the following other types implement trait `AddLogic<Rhs>`
   --> locus/src/lib.rs:656:9
    |
656 |         impl AddLogic for Int {
    |         ^^^^^^^^^^^^^^^^^^^^^ `locus::Int` implements `AddLogic`
...
note: required by a bound in `locus::sk::add`
   --> locus/src/lib.rs:550:19
    |
550 |     pub fn add<A: AddLogic<B>, B: Logical>(_: A, _: B) -> A::Output {
    |                   ^^^^^^^^^^^ required by this bound in `add`

error[E0277]: a `Rational` and a `_` have no order in logical code
   --> skel/src/bin/e03-missing-operator.rs:12:27
    |
 12 |         let less = prove!(r < r);
    |                           ^ - required by a bound introduced by this call
    |                           |
    |                           no order for these
```

Others, in `run.log`: the ambiguities name `locus::sk::ArgAs` and its two implementations (`x05`); a non-exhaustive `match` names the view, `__locus_view_Tree::Leaf`, and suggests an arm written with it (`x09`); a snapshot where a model is wanted is E0271, ``type mismatch resolving `<Vec<u32> as ModelOf>::Logic == Seq<BoundedU32>` `` (`x06`).

**Borrow errors of logical code are now in both expansions,** and point at the `logic!` and at the place. They mention the encoding twice: the borrow is "due to use in closure", and a note further down says that the error originates in the macro `::locus::skeleton`. (The rest of the message is left out.)

```text
error[E0382]: borrow of moved value: `v`
  --> corpus/src/bin/after-move.rs:19:13
   |
17 |     let v = vec![1, 2, 3];
   |         - move occurs because `v` has type `Vec<u32>`, which does not implement the `Copy` trait
18 |     let count = consume(v);
   |                         - value moved here
19 |     let n = logic! { v.len() }; //~ error: logical code reads `v` after `v` was moved
   |             ^^^^^    - borrow occurs due to use in closure
   |             |
   |             value borrowed here after move
```

### Cost

`cargo build -j1` of every binary of a set, the median of 5, and `cargo check` and `cargo build` of `scale`, one file of 500 functions with 2,001 pieces. "Without" is this spike's lowering with `LOCUS_SPIKE_NO_SKELETON=1`.

```text
corpus, runtime expansion, cargo build, with skeletons: median 2.06s of 5 (min 1.92s, max 2.14s); 23 of 26 binaries built
corpus, runtime expansion, cargo build, without (LOCUS_SPIKE_NO_SKELETON=1): median 2.00s of 5 (min 1.96s, max 2.04s); 25 of 26 binaries built
corpus, runtime expansion, cargo build, the first mode: median 1.94s of 5 (min 1.92s, max 2.35s); 25 of 26 binaries built
corpus, check expansion, cargo build, with skeletons: median 1.91s of 5 (min 1.85s, max 1.97s); 23 of 26 binaries built
corpus, check expansion, cargo build, without (LOCUS_SPIKE_NO_SKELETON=1): median 2.09s of 5 (min 1.95s, max 2.15s); 23 of 26 binaries built
corpus, check expansion, cargo build, the first mode: median 1.78s of 5 (min 1.72s, max 1.97s); 23 of 26 binaries built
scale, runtime expansion, cargo check, with skeletons: median 0.67s of 5 (min 0.64s, max 0.72s)
scale, runtime expansion, cargo check, without (LOCUS_SPIKE_NO_SKELETON=1): median 0.43s of 5 (min 0.40s, max 0.50s)
scale, runtime expansion, cargo check, the first mode: median 0.40s of 5 (min 0.38s, max 0.41s)
scale, check expansion, cargo check, with skeletons: median 0.64s of 5 (min 0.62s, max 0.71s)
scale, check expansion, cargo check, without (LOCUS_SPIKE_NO_SKELETON=1): median 0.46s of 5 (min 0.42s, max 0.51s)
scale, check expansion, cargo check, the first mode: median 0.44s of 5 (min 0.41s, max 0.51s)
scale, runtime expansion, cargo build, with skeletons: median 0.73s of 5 (min 0.71s, max 0.80s); 1 of 1 binaries built
scale, runtime expansion, cargo build, without (LOCUS_SPIKE_NO_SKELETON=1): median 0.46s of 5 (min 0.44s, max 0.48s); 1 of 1 binaries built
scale, runtime expansion, cargo build, the first mode: median 0.43s of 5 (min 0.41s, max 0.48s); 1 of 1 binaries built
scale, check expansion, cargo build, with skeletons: median 0.77s of 5 (min 0.70s, max 0.86s); 1 of 1 binaries built
scale, check expansion, cargo build, without (LOCUS_SPIKE_NO_SKELETON=1): median 0.50s of 5 (min 0.48s, max 0.61s); 1 of 1 binaries built
scale, check expansion, cargo build, the first mode: median 0.48s of 5 (min 0.46s, max 0.50s); 1 of 1 binaries built
```

- **On the corpus the difference is inside the noise:** about 2 seconds for 26 small binaries either way, most of it starting rustc and linking. On `cases`, 58 binaries of which half fail, the medians with skeletons are 4.05 and 4.23 seconds, against 3.71 and 3.95 without, and 3.90 and 3.67 for the first mode (`run.log`).
- **On a file that is nearly all logical code, skeletons add about a quarter of a second:** 0.18 to 0.27 seconds on 0.43 to 0.50, or about 0.1 milliseconds for each piece.

### What changed from the first mode

`./run.sh coverage` compares every file of `corpus`, `ghost-lets` and `cases` under the two lowerings. 26 of 101 change outcome or output.

| Change | Files |
|---|---|
| A rejection that the checking expansion alone made is now made by both: logical code that reads a moved or mutably borrowed place, and a type or a name that only a probe saw | `after-move`, `during-mutable-borrow`, `gl-after-move`, `gl-during-mutable-borrow`, `d05`, `t13c`, `t13d`, `t15` |
| Compiled in one expansion, now in both | `d06`: the probe that had no type to write is gone. `t16`: `OrderSpec::le(r, r)` with the stubs in the trait, since the arguments give `Self` |
| Needed an annotation, now compile | `g06`, `g13`, `g14` |
| Silently wrong, now right | `g15` prints `"Nat"` |
| Compiled, now a type error by design | `g18`, `h03`: snapshots are distinct types |
| Compiled, now ask for a type or fail | `g11`, `t09` (E0283), `t04` (E0308), `gl-snapshot-with-associated-type-field` (E0277) |
| Another error for the same failure | `gl-branch-on-logical`: E0369, ``binary operation `>` cannot be applied to type `Nat` ``, where it was E0283 at the `let`. `g16`, `g20`, `h05`, `t17`, `gl-hole-written-out-with-an-earlier-value` |

**Lints of the runtime program change too.** In the corpus's runtime expansion the first mode has `unused variable: x` for a parameter that only a proposition reads, and `value assigned to n is never read` where only a later proposition reads `n`. With skeletons rustc sees those reads and says nothing. A skeleton also brings a lint of its own, `unnecessary parentheses around function argument`, for `(x == y || x != y)` kept as an argument of a stub.

## What it means

1. **rustc can type logical code.** A translation that reads only tokens gave a skeleton to every piece of logical code in the 26-file corpus, 51 of them, signatures included, and rustc typed them all, in both expansions, with no source changed. Over the four sets that this spike did not write (four litmus examples are in two of them), 308 of 319 pieces have a skeleton, and the 11 without are a borrow in a rejected example and 10 propositions of trait methods with no body.
2. **The ghost local is solved where there is a skeleton.** Its type is the type of its logical code, forward, in both programs, with no guess. The first mode's three wrong programs are right, a ghost local passed only to generic code needs no annotation, and the draft's rule "a ghost local that only logical code mentions has no logical type for rustc" is needed only for the fallback.
3. **The price is that Rust's inference now applies to logical code, and it is not Locus's.** Four differences were found, and none has a syntactic fix:
   - a literal alone has no default, and may be typed backward by a later runtime use;
   - "a `Nat` is an `Int` where one is expected" is one more implementation at each place, and each one makes some other inference ambiguous;
   - whether a place is read as itself, its model, or its snapshot depends on what is expected, and rustc can choose among at most two of them, and not at all for a type parameter;
   - rustc selects no implementation for a type it does not know yet, where a typing made for Locus could use the bound.

   Each surfaces as "type annotations needed" or a mismatch, on the user's tokens. Two programs of the first spike that compiled now ask for a type (`g11`, `t09`), and four fail for a `Nat` where an `Int` is wanted (`t04`, and `r02`, `r04`, `r05` of `more`).
4. **So the skeleton cannot be the only typing.** Either Locus's rules for literals, widening and place reading are restated so that Rust's inference gives the same answer, or Locus types logical code itself as RFC 0001 says and the skeleton is a second opinion that must agree. In the second case the skeleton still gives what the draft needs from rustc: the ghost local's Rust type, trait resolution inside logical code, and borrows.
5. **rustc now resolves traits inside logical code.** The call listing shows the implementation that `Rational::le_total` comes from and the bound that `T::le_total` uses. `marker-lowering` listed operators and method calls in logical code as not probed; here `x + x` under `T: AddLogic<Output = T>` and `t.into()` are typed. A member named through its trait, `OrderSpec::le(r, r)`, which no probe could write, resolves once the stubs are in the trait (`t15`, `t16`).
6. **Probes and observations are not needed where there is a skeleton,** and the two expansions are then one text. The rejections that only the checking expansion made are made by both, and a program whose logical code does not borrow-check has no runtime program. Over 100 accepted binaries no call resolves differently.
7. **A logical enum cannot be a Rust enum.** It would have a tag, and could not contain itself. A zero-sized struct with a view enum beside it works for construction, `match` and exhaustiveness. It does not work for a variant pattern inside a variant pattern, and its hidden name has the weakness that `marker-lowering` found for hidden traits: an import of the type by name does not bring it (`x14`). The view cannot be reached through the type itself either: on stable Rust a tuple variant's pattern cannot be written through an associated type (`probes/p6.rs`, E0658).
8. **Snapshots as a family are precise, and cost three things.** rustc tells `Snap<A>` from `Snap<B>`, normalizes a vector's snapshot to a sequence of snapshots, and keeps no borrow. Against that: every type parameter whose snapshot is taken needs a `Snapshot` bound, which one accepted litmus example does not write; a type that no one declared a snapshot for has none, where RFC 0002 says opaque; and two snapshot types in two `impl` headers still collide, so the draft's rule against snapshot types in headers stays, for a new reason.
9. **A checked fallback closes the silent gap, and cannot follow a macro that rustc expands.** With every local of an untranslated initializer left untyped, the rule is that only logical code may mention it, and the driver reports the rest by name.
10. **Diagnostics are mostly the user's.** Type mismatches and unknown fields read as if the logical code were Rust. Missing implementations and ambiguities name the stubs and their traits, and the library can word the first line.

## Where the translation guesses

Beside the first mode's guesses, which stand. Each is from the tokens alone.

| What it decides | How | What it would need |
|---|---|---|
| Whether a name is a place | One lowercase segment; or a name in capitals alone or after a lowercase segment, taken for a static or a constant | Name resolution: a unit struct, a constant of a type, a local closure that is called |
| Whether `A::b(..)` is a logic function of a type or a function in a module | A capital on `A`. `u32::wrap(..)` is taken for a module's | Name resolution |
| Whether a path in a pattern names a variant of a logical enum | Two or more segments and a capital on the last | Name resolution, and the type: a variant brought into scope by `use`, a struct in a module |
| Whether `Name { .. }` is a struct literal | A capital on the last segment, outside the condition of an `if` and the scrutinee of a `match` | A parser of Locus |
| Whether a logic function returns a proposition | Its result is written `Prop` | Resolution of aliases |
| Where a snapshot is wanted in place of a model | Never | The parameter's type |
| The type of a literal that nothing gives | `Int` where both operands are literals, and under a minus; otherwise rustc's inference | Locus's own typing |
| The precedence of `===`, `==>`, `<==>` | As comparisons; below `\|\|`, to the right; below that | The grammar, which no RFC states yet |

## Not tested

- **Locus's own typing.** Nothing here types logical code but rustc, so no case compares the type rustc gives a ghost local with the type Locus would. The differences above are argued from RFC 0002's text.
- `#[derive(Model)]` and `impl Model` for a program's type; `Maybe`, `Set`, `Map` and `Unit`; a model's fields (`model(p).x`); arithmetic on a snapshot's bounded integers; cut snapshots and `CutSnap`; fields of a generated snapshot (`snap(t).id`).
- Propositions in a struct field's type, and in a trait method with no body. A hidden provided method could hold the second.
- What the translation rejects: casts, arrays, `if let`, literal patterns, logical closures that are not proofs, more than three closure parameters.
- Places that Rust types and Locus does not allow: `.field` through an overloaded `Deref`, which rustc accepts in a skeleton and RFC 0002 excludes.
- Logical code in a constant context: `sk::typed` is not a `const fn`. Logical code in closures and async blocks, which RFC 0001 rejects.
- Another crate: a logical type, a trait with logic functions, or a view from a dependency. `c07` and `probes/p4.rs` are the only results that depend on which crate declares what.
- Lints. Only the two changes above were looked at; lowering marks nothing it writes in a skeleton.
- rust-analyzer, rustfmt, and the size or speed of the runtime binary with closures that are never called. Debug builds only.
- The cost on anything but one synthetic file. 5 rounds, one machine.
- The rest of the litmus corpus: 35 distinct examples were lowered.
