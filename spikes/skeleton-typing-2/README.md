# Spike: typing skeletons, round 2: a conversion policy, names, snapshots, and more shapes

A throwaway experiment that extends `spikes/skeleton-typing` (round 1). It was built outside the repository, from a copy of that spike, and depends on nothing in the repository when it is built. Nothing may depend on it.

Round 1 showed that rustc can type logical code: `locus!` lowers each logical type to a zero-sized Rust type of its own, and writes each piece of logical code a second time as a Rust expression over stubs, a *typing skeleton*, in a closure that is never called. It left open where Rust's inference and RFC 0002's implicit conversions disagree. During this round the owner decided that the direction stands and that conversions are **explicit to start with**, and chose the rule for `Bool` and `Prop`. This round implements that policy, measures what it makes programs write, and takes up the other things round 1 left open.

## Question

1. **What does an explicit conversion policy cost?** With `Nat` to `Int` written (`n as Int`), comparisons that give a `Prop`, and a literal that is an `Int` unless something stands beside it: how many places in real specifications, and in the litmus corpus, write something that RFC 0002's implicit rules do not? What does rustc say when one is left out?
2. **What does "a comparison is a `Prop`" break or make awkward,** with and without an implicit conversion of a `Bool` where a proposition is wanted?
3. Can a runtime place given to a parameter whose type is a type parameter be its model, with no ambiguity (P4)? Can a method call find a logic function on the runtime type first, then on its model (P5)?
4. Can the names rustc shows read as the programmer thinks of them?
5. Can every runtime type have a snapshot (RFC 0002), on stable Rust, with `Snap<T>` still a type family?
6. Do the positions and shapes that round 1 did not cover have a skeleton: a proposition in a field's type, a contract on a trait method with no body, a nested variant pattern, a logical enum imported by name, `impl Model` and `#[derive(Model)]`, and two crates?
7. Does what worked in round 1 still work?

## Setup

A Cargo workspace of its own, stable rustc 1.97.0 and cargo 1.97.0. `run.sh` reproduces everything and `run.log` is its output (155 seconds with `target/` built, about 3,150 lines). `[profile.dev]` has no debug information and no incremental state, so `target/` is 1.7 GB where round 1's was 16 GB. `round1-run.log` is the output of round 1's own `run.sh`, run once before anything was changed (517 seconds), and `baseline/` holds its reports, which the comparisons below read.

- `locus-macros/` is `locus!`: `src/skeleton.rs` (1,339 lines, 1,101 in round 1) and `src/lib.rs` (4,380 lines, 3,784 in round 1). `locus/` is the library (1,153 lines). `driver/` is round 1's rustc driver, unchanged.
- The sets of round 1, unchanged: `corpus/` (26 files: 22 litmus examples and 4 others), `ghost-lets/` (17), `cases/` (58), `skel/` (38), `more/` (39).
- New sets:
  - `spec/master/`: the four specifications, with each place that a policy makes the source write marked. `tools/gen-spec.py` writes them for seven policies into `spec/` (the default) and `spec-p2x-v2/`, `spec-p2x-v0/`, `spec-p2i-v3/`, `spec-p2i-v2/`, `spec-p2i-v0/`, `spec-rfc/`. Markers stay on their line, so line N is line N in every version, and the versions can be read side by side with `diff`.
  - `spec-omit/`: each specification with one written conversion left out, eight files, from `tools/gen-omissions.py`.
  - `corpus-explicit/`: the corpus with what the default policy makes it write, from `tools/make-explicit.py`, which lists every edit with its cause.
  - `policy/`, 35 files: the policy's cases (`p*`), the `Bool` and `Prop` cases (`v*`), the shapes (`d*`), snapshots (`n*`), and two diagnostics samples (`e04`, `e05`). The first line of each says what the two compilations must do by Locus's rules.
  - `two-lib/` and `two-app/`: a library crate and a crate that uses it.
- `probes2/`: nine hand-written Rust files that tried an encoding before the macro wrote it (`q1`, `q2`, `q3`, `q5`) or that show why one does not exist (`s1` to `s5`). `probes/`, `first-mode/` and `scale/` are round 1's, kept and not run again.

### The policy, and its switches

The default of the library and of `run.sh` is the owner's choice.

| | Default | Switch |
|---|---|---|
| P1, literals | Beside another operand of an arithmetic operator, a comparison, `===` or `!==`, a literal has that operand's type. An index is a `Nat`. A literal with a written type has it: `0 as Nat`. Otherwise it is an `Int`. | `LOCUS_SPIKE_LITERALS=expected`: an argument, a field's initializer and a logic function's result take the type rustc infers there. `=inferred`: round 1's rule |
| P2, `Nat` to `Int` | Written everywhere: `n as Int`, RFC 0002's own spelling. `Nat - Nat` is an `Int` (a result type). `Int::from(n)` works too, for a logical `n`. | feature `mixed-ops`: implicit inside arithmetic and comparisons that mix the two (round 1's default) |
| P3, `Bool` and `Prop` | **V3**: a comparison gives a `Prop`. `&&`, `\|\|`, `!` are connectives on `Prop`, and operators on `Bool` when every operand is a `Bool`. A `Bool` is a proposition where the grammar wants one: `@(..)`, `prove!`, `prop!`, a quantifier's body, an operand of `==>` or `<==>`, an operand of `&&` or `\|\|` whose other operand is a `Prop`, an arm of a `prop`, the body of a logic function that returns `Prop`. `if` takes a `Bool` or a `Prop`. | feature `p3-v2`: V3 without the implicit conversion, `b === true` is written. `p3-v0`: a comparison gives a `Bool`, conversion implicit (round 1). `p3-v1`: a comparison gives a `Bool`, `b === true` is written |
| P4, a runtime place as an argument | To a parameter of a runtime type, the place itself. To one of a logical type, its model. To one whose type is a type parameter, the program writes `model(x)` or `snap(x)`. | none in the macro; `probes2/q2.rs` is an encoding in which nothing is written |
| P5, a method call on a place | A logic function declared on the place's own type is found first, through `&`, `&mut` and `Box`, then one on its model. | none |

Beyond the brief, two things changed because the new cases needed them: a trait's stubs are items of the lowered trait by default (round 1's hidden trait is `LOCUS_SPIKE_HIDDEN_TRAIT=1`), since an import of a trait from another crate does not bring a hidden trait; and `e as T` is translated.

### What `locus!` writes now

Only what differs from round 1's table.

| Logical code | Skeleton |
|---|---|
| a piece of logical code | `::locus::logic::logic(\|\| ..)` for `logic!`, `prove(..)` for `prove!`, `prop(..)` for `prop!`, `proposition(..)` for `@(..)`, `logic_fn(..)` for a logic function's body. No library macro stands between: round 1's `skeleton!` is gone |
| `x`, `f(x)`, `snap(x)`, `final!(r)` | `logic::model(&x)`, `f(logic::argument(&x))`, see below, `*logic::final_value(&r)` |
| `3`, `n + 3`, `xs[3]`, `3 as Nat` | `logic::literal::<Int>(3)`, `logic::literal_like(&n, 3)`, `logic::literal::<Nat>(3)` twice |
| `e as T` | `logic::cast::<_, T>(e)`, bounded by `As<T>` |
| `x.room()` for a place `x` | `({ use logic::{OnTheType as _, OnTheModel as _}; (&logic::receiver::<N<'r', N<'o', N<'o', N<'m', ()>>>>, _>(&x)).__locus_receiver() }).__logic_room()` |
| `snap(x)` | `{ use logic::{SnapDeclared as _, SnapOpaque as _}; (&logic::snapshot_of(&x)).__locus_snap() }` |
| `match t { Tree::Node(Tree::Leaf, n, _) => e, .. }` | `match logic::matched(t) { logic::Variants::<Tree>::Node(__locus_p1, n, _) => { let () = match logic::matched(__locus_p1) { logic::Variants::<Tree>::Leaf => (), _ => logic::hole() }; e } .. }` |

And for declarations:

- **A logical enum's variants** are those of an enum with the type's own name inside `const _: () = { .. };`, which only `impl logic::Matchable for Tree { type Variants = .. }` names. A pattern reaches them through the type alone, by the library's alias `Variants<T> = <T as Matchable>::Variants` (`probes2/q1.rs`: a path through an alias of a projection names a variant on stable; round 1's `probes/p6.rs` had found that a qualified path does not).
- **An inherent `impl`** gets `impl logic::Declares<NAME> for Type { type Owner = Self; }` for each logic function that takes `self`, where `NAME` is the function's name as a type (`N<'r', N<'o', ..>>`, const generic `char`s). The receiver of a method call is then chosen by Rust's own method lookup, by value before by reference: `OnTheType` is implemented for `Receiver<NAME, T>` where `T: Declares<NAME>`, and `OnTheModel` for `&Receiver<NAME, T>`.
- **A struct with a proposition in a field's type** gets `const _: () = { fn __locus_fields<..>(__locus_self: &Name<..>) { let Name { lo, hi, .. } = __locus_self; logic::proposition(|| ..); } };`.
- **A trait method with no body** gets a hidden provided method beside it, `fn __locus_contract_area(&self, ..) where Self: Sized { logic::proposition(|| ..); .. }`, with the same parameters.
- **`impl Model for T`** is an implementation of the library's `Model` and of `Runtime`, and its logic function is a stub in a `const _`. **`#[derive(Model)]`** writes a logical type `Name_Logic`, generic over the models of the type's parameters, whose fields are `<FieldType as Model>::Logic`, without the proof fields.
- **`T: ::locus::Snapshot`** is added to type parameters ([Snapshots](#snapshots-for-every-type)).

## Results

Run on 2026-11-22. Every count below is from `run.log`.

### The conversion policy: what it makes the source write

**Source lines that change between RFC 0002's implicit rules and the default,** by cause. "RFC 0002" here is the text with nothing written; it compiles with the switches nearest to RFC 0002's rules (`mixed-ops`, `p3-v0`, `LOCUS_SPIKE_LITERALS=inferred` for the corpus and `=expected` for the specifications). The corpus is the litmus text, copied unchanged in round 1.

| | Lines | `as Int` | A literal that needs a type | `=== true` | The `bool` result bridge | `model(x)` at a generic argument | Changed lines, default (V3) | Changed lines, strict V2 |
|---|---|---|---|---|---|---|---|---|
| `corpus/`, 26 files | 798 | 1 line (4 places) | 4 lines | 0 | 0 | 0 | **5** | 5 |
| `ghost-lets/`, 17 files | 441 | 0 | 0 | 0 | 0 | 0 | **0** | 0 |
| `s1-stack` | 91 | 1 | 0 | V2 only: 1 | 1 | 0 | **2** | 2 |
| `s2-sorted-vec` | 90 | 0 | 2 lines (4 places) | 0 | 0 | 0 | **2** | 2 |
| `s3-binary-search` | 81 | 2 lines (6 places) | 0 | V2 only: 3 | 0 | 0 | **2** | 5 |
| `s4-container` | 109 | 0 | 2 lines (3 places) | 0 | 0 | 2 | **4** | 4 |
| the four specifications | 371 | 3 lines (7) | 4 lines (7) | V2 only: 4 | 1 | 2 | **10** | 13 |

- The corpus's five lines: `n + x === x + n` twice on one line for `n: Int`, `x: u32` (`logical-closure-in-body`), `Tree::Leaf => 0` for a `Nat` result (`logical-fields`), `@Even(1)` (`match-every-arm`), and `double(3)` twice (`x-joined-logical-type`, `x-split-logical-type`). With them changed, `corpus-explicit/` compiles as round 1's corpus did: 23 of 26 files in both expansions, all 20 accepted litmus examples, both rejected ones rejected by rustc in both expansions, 52 of 52 pieces translated.
- **The implicit conversion of a `Bool` saves nothing in the corpus,** which has no runtime `bool` in a proposition: `corpus-explicit/` and `ghost-lets/` compile unchanged under strict V2. In the specifications it saves 4 places on 4 lines, each a `bool` result or flag in a contract.
- **Most of the cost is not the owner's two choices.** 7 of the 17 written places in the specifications, and 4 of the 5 changed lines of the corpus, are literals that need a type because P1 makes a lone literal an `Int`. RFC 0002 says "an `Int`, or a `Nat` where a `Nat` is expected". With `LOCUS_SPIKE_LITERALS=expected`, 5 of those 7 go (the two that stay are the branches of an `if` that is an operand of `+`, of which nothing is expected), and all 4 of the corpus's: the corpus as written then has 22 files that compile in both expansions under the default, and the one left is the `as Int` line.

**Places, for each specification, under each policy.** Each cell is `as Int` / a literal's type / `=== true` / the bridge / `model(x)` = total. Every cell was compiled in both expansions and run; none fails.

```text
                             p2x-v3           p2x-v2           p2x-v0           p2i-v3           p2i-v2           p2i-v0              rfc
s1-stack                1/0/0/1/0=2      1/0/1/1/0=3      1/0/0/0/0=1      0/0/0/1/0=1      0/0/1/1/0=2      0/0/0/0/0=0      0/0/0/0/0=0
s2-sorted-vec           0/4/0/0/0=4      0/4/0/0/0=4      0/4/0/0/0=4      0/4/0/0/0=4      0/4/0/0/0=4      0/4/0/0/0=4      0/2/0/0/0=2
s3-binary-search        6/0/0/0/0=6      6/0/3/0/0=9      6/0/0/0/0=6      0/0/0/0/0=0      0/0/3/0/0=3      0/0/0/0/0=0      0/0/0/0/0=0
s4-container            0/3/0/0/2=5      0/3/0/0/2=5      0/3/0/0/2=5      0/3/0/0/2=5      0/3/0/0/2=5      0/3/0/0/2=5      0/0/0/0/2=2
all four               7/7/0/1/2=17     7/7/4/1/2=21     7/7/0/0/2=16     0/7/0/1/2=10     0/7/4/1/2=14      0/7/0/0/2=9      0/2/0/0/2=4
```

`p2x` is P2 explicit and `p2i` is `mixed-ops`; `v3` is the default, `v2` strict, `v0` round 1's rule; all six have P1's literals. `rfc` is `mixed-ops`, `p3-v0` and `LOCUS_SPIKE_LITERALS=expected`. V1 has a switch and was not written for the specifications; the policy's cases under it are [below](#what-a-comparison-as-a-prop-breaks-or-makes-awkward).

The lines, in `spec/src/bin/` (the default) and `spec-p2x-v2/src/bin/`:

```text
s1-stack, p2x-v3: as Int: lines [34]; Bool and Prop bridge: lines [42]
s1-stack, p2x-v2: as Int: lines [34]; === true: lines [42]; Bool and Prop bridge: lines [42]
s2-sorted-vec, p2x-v3: a literal's type: lines [17, 19]
s3-binary-search, p2x-v3: as Int: lines [28, 43]
s3-binary-search, p2x-v2: as Int: lines [28, 43]; === true: lines [32, 33, 65]
s4-container, p2x-v3: a literal's type: lines [36, 89]; model(x): lines [78, 82]
```

**The three worst lines under the default** (P2 explicit, V3):

```rust
// s3-binary-search.rs:28, four `as Int` in one proposition
-> @(lo as Int + (hi - lo) / 2 >= lo as Int && lo as Int + (hi - lo) / 2 < hi as Int) { _ }
// s2-sorted-vec.rs:19, three literal types
(if xs[0] < x { 1 as Nat } else { 0 as Nat }) + below(xs.skip(1 as Nat), x)
// s3-binary-search.rs:43
let middle: @(mid as Int == lo as Int + (hi - lo) / 2) = _;
```

**The three worst under strict V2** are the three contracts that mention a `bool` result (the lines above are the same under it):

```rust
@(found === true <==> contains(xs, x)),                              // s3:32
@(found === true ==> at < xs.len() && xs[at] == x))                  // s3:33
pub fn is_empty(&self) -> (r: bool, @(r === true <==> self.len() == 0)) {   // s1:42
```

**Does the result read well?** Mostly yes. Three of the four files have two or fewer changed lines in about 90, and in the default nothing is written for `Bool` and `Prop` at all. What reads badly is one cluster: subtraction. `hi - lo` is an `Int`, so every `Nat` that then meets it needs `as Int`, and a midpoint takes four. The literal types read worse than they cost: `0 as Nat` as the base case of a recursive function, and `skip(1 as Nat)`, are noise that RFC 0002's own rule does not ask for.

### What a comparison as a `Prop` breaks or makes awkward

Each is a file of `policy/`, compiled in both expansions under the default, and again under each other policy.

| | Case | Default (V3) | Strict V2 (`p3-v2`) |
|---|---|---|---|
| 1 | A runtime function that returns `bool`, tied to a proposition (`v1`, `v1w`) | all three spellings compile | `r <==> P` and the implications need `r === true`; `r === decide(P)` compiles as it is |
| 2 | A runtime `bool` field and local in a contract (`v2`) | compiles: `@(g.open)`, `@(g.open && n <= g.width)`, `@(ready ==> g.open)`, `@(!ready \|\| g.width > 0)` | E0277, 7 errors, each with the note below |
| 3 | `if` on a `Bool` and on a `Prop` (`v3`) | compiles; both go through one stub, `condition<C: Condition>`, and the branches have one type | compiles |
| 4 | A logic function with a comparison as its body (`v4a`, `v4b`) | `-> Prop` compiles; `-> Bool` is E0308; `decide(n < 10)` and `(n < 10) as Bool` compile | the same |
| 5 | A comparison in a ghost local with no annotation (`v5`) | a `locus::Prop`; then `@(small)` and `if small { .. }` compile | the same |
| 6 | `==` on two `Bool`s, `b == true`, `b === true` (`v6`) | each is a `Prop`; `a && !b` stays a `Bool` | the same |
| 7 | `flag && a < b`, `a < b \|\| flag` (`v7`) | compiles, a `Prop` | E0277, below |

**Item 1, the `bool` result.** RFC 0002 has no iff connective and no function from `Prop` to `Bool`. All three candidates type-check in the skeleton under the default (`v1-bool-result.rs`):

```rust
fn is_zero_iff(x: u32) -> (r: bool, @(r <==> x == 0)) { (x == 0, _) }
fn is_zero_implications(x: u32) -> (r: bool, @((r ==> x == 0) && (x == 0 ==> r))) { (x == 0, _) }
fn is_zero_decide(x: u32) -> (r: bool, @(r === decide(x == 0))) { (x == 0, _) }
```

- `r <==> P` reads best and is the shortest. It needs a new connective; the spike's parser gives it the lowest precedence, below `==>`. `s1-stack` (`is_empty`) and `s3-binary-search` (`found <==> contains(xs, x)`, and `has`) use it.
- The two implications need nothing new, and write `P` twice.
- `r === decide(P)` types best: it needs no conversion under any of the four variants, because it never puts a `Bool` where a proposition is wanted, and it is the only one that also works where a `Bool` is wanted, as in item 4. It needs one library function.
- Under round 1's rule (`p3-v0`), where a comparison is a `Bool`, the same contract is `r == (self.len() == 0)`. That line is the one "bridge" in the tables: it is E0308 under the default (`o1b`, below).

**Item 4's message** for the function declared `Bool`:

```text
error[E0308]: mismatched types
 --> policy/src/bin/v4b-returns-bool.rs:6:36
  |
6 |     logic fn small(n: Nat) -> Bool { n < 10 }
  |                               ---- ^^^^^^^^^^ expected `Bool`, found `Prop`
  |                               |
  |                               expected `locus::Bool` because of return type
```

It is clear, and says nothing of `decide`. In the suite of round 1 this is one of the two largest causes of change: 23 of its files change when a comparison becomes a `Prop`, and in 20 of them the first error is on a logic function declared `-> Bool`, such as `logic fn le(a: Rational, b: Rational) -> Bool { a.num * b.den <= b.num * a.den }`, which must now be declared `-> Prop`.

**Item 6.** Under the default `b == true` and `b === true` are both propositions and mean the same; no message tells them apart, and the library's notes always say `=== true`. Under `p3-v0` they differ in type (`v6` prints `Bool`, `Bool`, `Prop`, `Bool` there), which is the confusing arrangement.

**Item 7, strict V2,** names both sides and says what to write:

```text
error[E0277]: `&&` and `||` take two `Bool`s or two propositions, not a `locus::Bool` and a `Prop`
   --> policy/src/bin/v7-mixed-connective.rs:13:28
    |
 13 |         let and = logic! { flag && a < b };
    |                            ^^^^ - required by a bound introduced by this call
    |                            |
    |                            this is a `locus::Bool`, and the other side is a `Prop`
    |
    = help: the trait `Connective<Prop>` is not implemented for `locus::Bool`
    = note: a `Bool` is made a proposition by writing `=== true` after it: `b === true`
help: `locus::Bool` implements trait `Connective<Rhs>`
    ...
517 |     impl Connective<NoSuchOperand> for Bool {
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Connective<locus::NoSuchOperand>`
```

and for a `Bool` alone where a proposition is wanted (`v2`, strict V2):

```text
error[E0277]: a `locus::Bool` is not a proposition
   --> policy/src/bin/v2-bool-field-and-local.rs:9:41
    |
  9 |     fn pass(g: &Gate, n: u32, allowed: @(g.open), fits: @(g.open && n <= g.width)) -> u32 { n }
    |                                         ^^^^^^^^ a proposition is wanted here, and this is a `locus::Bool`
    |
    = help: the trait `Proposition` is not implemented for `locus::Bool`
    = note: a `Bool` is made a proposition by writing `=== true` after it: `b === true`
```

**Where a `Bool` is still written under the default** (`v8-bool-still-written.rs`; `v8w` is the same with `=== true` and compiles). Tested: an argument of a parameter of type `Prop`, a ghost local annotated `Prop`, and a generic position. Not tested, and expected for the same reason, that rustc sees one type where another is wanted and no stub stands there: either side of `===`, a field of type `Prop`, and the two branches of an `if` of which one is a `Bool` and the other a `Prop`.

```text
error[E0271]: type mismatch resolving `<bool as Model>::Logic == Prop`
   --> policy/src/bin/v8-bool-still-written.rs:11:45
    |
 11 |         let to_a_parameter = logic! { twice(flag) };
    |                                             ^^^^ expected `Prop`, found `Bool`
    |
note: required by a bound in `argument`
...
error[E0308]: mismatched types
  --> policy/src/bin/v8-bool-still-written.rs:12:31
   |
12 |         let annotated: Prop = logic! { flag };
   |                        ----   ^^^^^ expected `Prop`, found `Bool`
```

Neither says what to write. The first names the encoding (`Model::Logic`, `argument`).

**V1** (a comparison is a `Bool`, and it is written as a proposition) was not asked for in the end. Its switch exists; with the policy's cases as written for the default, 10 of 35 compile in both expansions under it, against 21 by default.

### What rustc says when a conversion is left out

**An omitted `as Int` in an operator** (`p2a`). The library's text is the first line and the note says what to write:

```text
error[E0277]: `+` is not defined on a `locus::Int` and a `Nat` in logical code
   --> policy/src/bin/p2a-omitted-as-int.rs:9:28
    |
  9 |         let sum = logic! { i + n };
    |                            ^ - required by a bound introduced by this call
    |                            |
    |                            this is a `locus::Int`, and the other side is a `Nat`
    |
    = help: the trait `AddLogic<Nat>` is not implemented for `locus::Int`
    = note: a `Nat` is added to an `Int` after it is made one: write `as Int` after the `Nat`
help: `locus::Int` implements trait `AddLogic<Rhs>`
   --> locus/src/lib.rs:899:9
    |
899 |         impl AddLogic for Int {
    |         ^^^^^^^^^^^^^^^^^^^^^ `AddLogic`
...
972 |         impl AddLogic<NoSuchOperand> for Int {
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `AddLogic<locus::NoSuchOperand>`
note: required by a bound in `locus::logic::add`
```

This took a trick (`probes2/q5.rs`). With one implementation of `AddLogic` for `Int`, rustc takes the right operand's type from it and reports a bare mismatch on `n`, E0308, "expected `Int`, found `Nat`", with none of the library's text. So each built-in operator has one more implementation, for a type `NoSuchOperand` that is not in the language, and the error becomes E0277 with `#[diagnostic::on_unimplemented]`'s message and note. The price is in the message: it lists that implementation, and rustc reports the error twice, once for each operand.

**How good is it, honestly.**

| What is left out | rustc's message | Says what to write? |
|---|---|---|
| `as Int` in arithmetic or a comparison (`p2a`, `o1a`, `o3a`, `o3b`) | E0277, the library's text and note | **Yes.** Noisy: twice, and names `NoSuchOperand` |
| `as Int` at an argument (`p2a`: `negate(n)` for `n: u32`) | E0271, ``type mismatch resolving `<u32 as Model>::Logic == Int` ``, "expected `Int`, found `Nat`" | No. It names the two types, and the encoding |
| `=== true`, strict V2 (`v2`, `v7`) | E0277, the library's text and note | **Yes** |
| `=== true` where it is still needed by default (`v8`) | E0271 or E0308, "expected `Prop`, found `Bool`" | No |
| a literal's type (`p1c`, `o2a`, `o2b`, `o4b`) | E0308, "expected `Nat`, found `Int`", at the literal; or "`if` and `else` have incompatible types" | No. The mismatch is plain, and points at the literal |
| `model(x)` at a generic argument, concrete type (`p4b`) | E0283, "type annotations needed", at the `let`, with a list of 27 types that implement `Logical` | **No, and it misleads:** it suggests a type on the `let` |
| `model(x)` at a generic argument, `x: T` (`o4a`) | E0308, "expected associated type `<T as locus::Model>::Logic`, found type parameter `T`" | No, but it shows the model is wanted |
| `<==>` written as `==` (`o1b`) | E0308, "expected `Bool`, found `Prop`" | No |

`#[diagnostic::on_unimplemented]` reaches only the errors that are an unsatisfied bound, and rustc reports a mismatch (E0308, E0271) whenever it can decide the type first. On stable there is nothing to attach to a mismatch. The two that say what to write do so because the operator stubs were arranged so that the failure is a bound.

**Two omissions in each specification** (`spec-omit/`; all eight are rejected in both expansions):

| File | Omission | rustc | Can the programmer act on it? |
|---|---|---|---|
| `o1a` | `s.room() == cap` | E0277 ``` `==` is not defined on a `locus::Int` and a `locus::Nat` in logical code ```, note: write `as Int` after the `Nat` | Yes |
| `o1b` | `r == (self.len() == 0)` | E0308 at `==`: expected `Bool`, found `Prop` | Only knowing the rule |
| `o2a` | `0` as the result's base case | E0308 "`if` and `else` have incompatible types", `0` "expected because of this", the other branch "expected `Int`, found `Nat`" | Partly: it blames the correct branch |
| `o2b` | `xs.skip(1)` | E0308 at `1`: expected `Nat`, found `Int`; "method defined here" shows `skip(self, n: Nat)` | Yes, the mismatch is at the literal |
| `o3a` | `mid == lo as Int + ..` | E0277 as `o1a` | Yes |
| `o3b` | `lo + (hi - lo) / 2` | E0277 ``` `+` is not defined on a `Nat` and a `locus::Int` ```, with the note | Yes |
| `o4a` | `total_of_pushed(before, x)` | E0308: expected associated type `<T as locus::Model>::Logic`, found type parameter `T`; then "consider further restricting this bound" | Partly, and the help is wrong |
| `o4b` | `shelf.items.take(0)` | E0308 at `0`: expected `Nat`, found `Int` | Yes |

### P1, P2, P4 and P5

- **P1 is forward.** `let zero = logic! { 0 };` then `takes_nat(zero)` is E0308 in both expansions, "expected `Nat`, found `Int`" (`p1a`). `p1b` prints `("locus::Int", "locus::Nat", "locus::Nat", "locus::Nat", "locus::Int", "locus::Int")` for a lone literal, `n + 1`, `v[0] + 1`, `3 as Nat`, `same(4)` and `if .. { 1 } else { 2 }`. With `LOCUS_SPIKE_LITERALS=expected`, `double(3)` compiles (`p1c`) and `same(4)` becomes E0283, "type annotations needed" (`p1b`): a literal given to a type parameter has no type, where P1 says `Int`. That is the price of RFC 0002's rule in this encoding.
- **P2.** The spelling is `n as Int`. Lowering rewrites `e as T` from syntax to `logic::cast::<_, T>(e)`, and a literal with `as` to a literal of that type. `n as Int < i` is read as a comparison (in Rust `<` after a cast's type starts generic arguments; this is one of the [guesses](#where-the-translation-guesses)). `p2b` compiles `i + n as Int`, `n as Int < i`, `negate(n as Int)` and `Int::from(n as Nat) + i`.
- **P4.** `p4a` compiles a place given as itself (`hits_of(r)` for `r: &Counter`), as its model (`double(n)`), and with `model(n)` and `snap(v)` written for a type parameter. Left out, it is E0283 (`p4b`). **An encoding in which the model wins exists, and is not in the macro** (`probes2/q2.rs`, which compiles and runs): the *callee's* stub takes each parameter through a bound, `fn same<T: Logical, A0: ModelArg<T, V0>, V0>(t: A0) -> T`, and the call passes the unread place, `same(place(&n))`. For a type parameter the only implementation for a place is its model, so `T` is `Nat`. The same encoding makes a literal argument a `Nat` where a `Nat` is wanted and an `Int` for a type parameter, forward: `let x = same(literal(0)); takes_nat(x)` is E0308. Its cost, seen in the probe: an argument of the wrong type is E0271 on a projection (``type mismatch resolving `<&Vec<u32> as Model>::Logic == Nat` ``) where round 1 had a plain E0308, every logic function's stub gets two type parameters for each parameter, which breaks a turbofish on a call, and a hole `_` as an argument needs a marker of its own. So in the specifications `model(x)` was written, twice, in `s4-container`.
- **A parameter declared `&T` takes a place of type `&T`.** Logical code has no `&`, so `Scale::limit_spec(s)` for `s: Scale` and `logic fn limit_spec(self: &Scale)` is E0308, and `two-app` binds `let sr = &s;` in runtime code first. RFC 0002's own example is `logic fn model(stack: &Stack)`.
- **P5 works, with nothing guessed about names.** `p5-method-order` prints `("locus::Int", "locus::Nat", "locus::Int", "locus::Int", "locus::Nat", "locus::Nat")`: `s.room()` is the runtime type's, `s.last()` is its model's, `s.len()` is the runtime type's own `len` (an `Int`) though its model has one too, both work through `&` and `Box`, `model(s).len()` is the model's, and `p.twice()` works for a type with no model. Round 1's `x11` now compiles. The call listing shows where each went: `Stack::new::{closure#0}:034:066 => the definition at 23:15 | Stack::__logic_room with []`. Where neither has the method, the error is rustc's E0599 on the model, ``no method named `__logic_top` found for struct `Seq<T>` `` (`probes2/q3.rs`). Limits: a logic function that a *trait* gives a runtime type is not marked, so `x.f()` for it looks at the model only (not tested); and the receiver must be a place.

### The four specifications

All four compile in both expansions under all seven policies, run, and print the same lines in both expansions; over the four default files the call listings have 93 lines each and none differs. They hold 102 pieces of logical code, all translated.

| | What it has | Written that RFC 0002 lets the programmer omit (default) |
|---|---|---|
| `s1-stack`, 91 lines | `Stack` over a `Vec<u32>` with `impl Model`, an invariant in a field, a logic function on the runtime type (`room`), two lemmas, contracts on `new`, `len`, `is_empty`, `push`, `pop`, `top` | line 34 `cap as Int`; line 42 `r <==> self.len() == 0` where round 1's rule writes `r == (self.len() == 0)` |
| `s2-sorted-vec`, 90 lines | a quantified `sorted`, a recursive `below`, three lemmas, an invariant, `insert` with a loop that keeps two proofs | lines 17 and 19: four literal types |
| `s3-binary-search`, 81 lines | `sorted`, `contains`, three lemmas, `search` with a found flag and three proof variables that the loop keeps and proves again, `has` | lines 28 and 43: six `as Int` |
| `s4-container`, 109 lines | a specification trait `Weighed` implemented for `Nat` and for a logical struct, three logic functions generic over it, `Even` with an invariant, `Shelf<T>` with an invariant on its snapshot, `put_weighed<T: Model> .. where T::Logic: Weighed` | lines 36 and 89: three literal types; lines 78 and 82: `model(x)` twice |

`snap(items).len()` in `s4` is not counted: RFC 0002 itself writes `snap(v).len()` for a `Vec` of values with no model.

**Where the macro guessed in them.** In all four: which names are places (lowercase), that `Stack::room`, `Self::le` name a type's logic function (a capital before the last segment), that `impl Model` is Locus's by the trait's name, and that the structs with field propositions are declared at module level. In `s2` and `s3`: that `n as Int < m` is a comparison did not arise (the casts there are followed by `+`, `==`, `>=`), and the precedence of `<==>` below `==>`. In `s4`: that `Shelf<T>` and its `impl` need `T: Snapshot` because they write `snap(..)`, and that `put_weighed` does. Nothing in them needed a logical type's number of parameters.

**What had to be written around, beyond conversions.** A lemma is called inside `logic! { .. }`: a call of a logic function in runtime code is not lowered by this toy, and its stub would run (it did, in the first draft of `s1`, and panicked). A hole inside `vec![..]` is not rewritten. Both are round 1's.

### Names

- The marker for every proof is `Proof`; the helpers are `logic::logic`, `logic::prove`, `logic::prop`, `logic::proposition`, `logic::logic_fn`; the stubs are `locus::logic::add`, `lt`, `model`, `argument`, `literal`, `condition`, `matched`.
- The traits a message may name: `Model` (was `ModelOf`), `GivenAs` (`ArgAs`), `Proposition` (`Truth`), `Condition`, `Negation`, `Connective`, `Equality` (`EqLogic`), `Ordered` (`OrdLogic`), `IntegerLiteral` (`Literal`), `As`, `Indexable`, `Matchable` (`HasView`). Each has a `#[diagnostic::on_unimplemented]` text; the library's arithmetic traits, `AddLogic` and the others, now have one too.
- A logical enum's variants print as `_::Tree::Leaf` (in a module, `shapes::_::Tree::Leaf`), and the note points at the user's `enum`.

The five samples, round 1 against this round. In each, the two expansions print the same.

**A `Nat` where a `Seq` is wanted** (`e01`): unchanged, word for word. E0308, ``expected `Seq<Nat>`, found `Nat` ``, "function defined here" at the user's `logic fn count(xs: Seq<Nat>)`.

**An unknown field** (`e02`): unchanged. E0609, ``no field `nom` on type `Rational` ``, with rustc's suggestion of `num`.

**A missing operator implementation** (`e03`), first lines. Before:

```text
error[E0277]: the trait bound `Rational: AddLogic<_>` is not satisfied
   --> skel/src/bin/e03-missing-operator.rs:11:28
    |
 11 |         let sum = logic! { r + r };
    |                            ^ - required by a bound introduced by this call
    |                            |
    |                            unsatisfied trait bound
...
note: required by a bound in `locus::sk::add`
```

After:

```text
error[E0277]: `+` is not defined on a `Rational` and a `_` in logical code
   --> skel/src/bin/e03-missing-operator.rs:11:28
    |
 11 |         let sum = logic! { r + r };
    |                            ^ - required by a bound introduced by this call
    |                            |
    |                            this is a `Rational`, and the other side is a `_`
    |
help: the trait `AddLogic<_>` is not implemented for `Rational`
...
    = note: a `Nat` is added to an `Int` after it is made one: write `as Int` after the `Nat`
...
note: required by a bound in `locus::logic::add`
```

Better in its first line and its label. Worse in two ways: the note about `as Int` is beside the point here (a note cannot depend on the types), and the right operand's type is `_`, because rustc no longer takes it from the only implementation.

**A `Bool` where a `Prop` parameter is wanted** (`e04`). Before: ``error[E0271]: type mismatch resolving `<bool as ModelOf>::Logic == Prop` `` with ``required by a bound in `locus::sk::arg` `` and `ArgAs<P, Via>`. After:

```text
error[E0271]: type mismatch resolving `<bool as Model>::Logic == Prop`
   --> policy/src/bin/e04-bool-for-prop-parameter.rs:8:43
    |
  8 |         let from_a_place = logic! { twice(flag) };
    |                                           ^^^^ expected `Prop`, found `Bool`
    |
note: required by a bound in `argument`
   --> locus/src/lib.rs:416:24
    |
416 |     pub fn argument<T: GivenAs<P, How> + ?Sized, P, How>(_: &T) -> P {
    |                        ^^^^^^^^^^^^^^^ required by this bound in `argument`

error[E0308]: mismatched types
 --> policy/src/bin/e04-bool-for-prop-parameter.rs:9:43
  |
9 |         let from_a_value = logic! { twice(true) };
  |                                     ----- ^^^^ expected `Prop`, found `Bool`
```

Only the names changed. For a place, the message is still about the encoding; for a value it reads as Rust.

**A non-exhaustive `match` on a logical enum** (`e05`). Before:

```text
error[E0004]: non-exhaustive patterns: `__locus_view_Tree::Leaf` not covered
...
note: `__locus_view_Tree` defined here
   = note: the matched value is of type `__locus_view_Tree`
```

After:

```text
error[E0004]: non-exhaustive patterns: `_::Tree::Leaf` not covered
  --> policy/src/bin/e05-missing-arm.rs:9:15
   |
 9 |         match t {
   |               ^ pattern `_::Tree::Leaf` not covered
   |
note: `_::Tree` defined here
  --> policy/src/bin/e05-missing-arm.rs:7:10
   |
 7 |     enum Tree { Leaf, Node(Tree, Nat, Tree) }
   |          ^^^^
   |          |
   |          not covered
   = note: the matched value is of type `_::Tree`
```

The `_::` is the anonymous constant, and is as close as was found: an enum of the same name at module level would collide with the type, and rustc prints an item's path. rustc's suggested arm is garbled in both rounds (`depth(l) +, _::Tree::Leaf => todo!() 1,`), because the skeleton of `depth(l) + 1` is a block.

What still shows the encoding: `__logic_room` in "no method named", `<T as Model>::Logic` and `argument` in the mismatches above, `NoSuchOperand` in the operator errors, "borrow occurs due to use in closure" in borrow errors (unchanged), and the type names `Point_Logic` and `__locus_snap_Keep`.

### Snapshots for every type

Round 1's family is kept: `Snap<T>` is `<T as Snapshot>::S`, `Snap<Vec<T>>` is `Seq<Snap<T>>`, and no snapshot type has a lifetime. Two things were added, and the rest was tried by hand.

**1. Lowering adds the bound** `T: ::locus::Snapshot` to the type parameters of a function, a type or an `impl` that writes `Snap<..>` or `snap(..)` (`mentions`), and of a function or `impl` that calls a function of the same invocation that got it (`calls`, the default).

```text
with no bound added (off): 11 files, 43 places where rustc reports that a type has no snapshot type:
  ghost-lets/gl-snapshot-with-associated-type-field: 3 places; with the bounds added (calls): compiles
  policy/n1-snapshot-bounds-inserted: 6 places; with the bounds added (calls): compiles (not with `mentions` alone)
  skel/c06-unbounded-parameter: 4 places; with the bounds added (calls): compiles
  spec/s4-container: 9 places; with the bounds added (calls): compiles
  ... (7 more, which still fail: below)
mentions: bounds added in 17 places (12 on a function, 3 on a type, 2 on an impl); by set: ghost-lets 1, policy 7, skel 6, spec 3
calls: bounds added in 18 places (13 on a function, 3 on a type, 2 on an impl); by set: ghost-lets 1, policy 8, skel 6, spec 3
  where lowering sees the need and cannot add the bound: ["policy/n2-snapshot-in-trait-impl:12 (a method of a trait's impl)"]
```

- **In the corpus and `cases`, no place needs it** (0 bounds added). One litmus example does, the one that regressed in round 1, and **it now compiles in both expansions**: `gl-snapshot-with-associated-type-field`, one bound on `fn kept<F: Family>`. Round 1's `c06` compiles too.
- **Lowering cannot always see the need.** Four ways, each a case that Locus's rules accept:
  - a method of a trait's `impl` takes a snapshot in its body, and the trait's declaration says nothing of snapshots, so the method may not ask for the bound (`n2`);
  - a generic function that `locus!` did not lower calls a lowered one that got the bound: E0277 in plain Rust, ``` `T` has no snapshot type that rustc can name ``` (`n3`);
  - a generic function passes its parameter to a function of another invocation or crate, or holds a type of another invocation whose field is a snapshot: E0277 twice (`n4`). `calls` goes by a function's name inside one invocation;
  - and the bound is now part of the lowered function's signature, which a caller that is not lowered must meet.

**2. `snap(x)` has an opaque fallback,** chosen by Rust's method lookup as a receiver is (`probes2/s5.rs`): the declared snapshot where the place's type has one, and otherwise `locus::Opaque`, one type with no parameter. `snap(set)` for a `BTreeSet<u32>` and for a `Duration` compiles and prints `locus::Opaque` (`n5`; round 1's `c05` compiles).

**The attempts at an opaque fallback for a concrete type,** each a probe:

| Attempt | Result |
|---|---|
| A blanket implementation beside the declared ones (`s1`) | ``error[E0119]: conflicting implementations of trait `Snapshot` for type `u32` `` |
| A helper trait with a blanket implementation that defers to the declared one (`s2`) | `error[E0658]: specialization is experimental`, and E0119 without it |
| The helper trait with a marker parameter, `SnapVia<Declared>` and `SnapVia<Fallback>` (`s3`) | Coherence accepts it. Where only the fallback applies, rustc chooses it. Where both apply it is `error[E0283]: type annotations needed`, ``multiple `impl`s satisfying `u32: SnapVia<_>` found``. In a type position the marker must be written, and only lowering could choose it |
| An explicit marker that lowering writes where it knows the type is foreign, `OpaqueOf<Foreign<'a>>` (`s4`) | It tells two foreign types apart, and **keeps the type's lifetimes**: holding the snapshot after the borrow is `error[E0503]: cannot use `n` because it was mutably borrowed`. And lowering would have to know from a name that a type is foreign |
| The choice by method lookup, in an expression (`s5`, and the macro) | Works for a concrete type. For a type parameter with no bound it is opaque whatever the parameter is later: `s5` prints `a u32 through an unbounded parameter: s5::Opaque; through a bounded one: s5::BoundedU32` |

**Nothing gives `Snap<Foreign>`, written as a type, a meaning without specialization.** The one opaque type has costs of its own: the snapshots of two unrelated foreign types are one Rust type, so rustc accepts one where the other is wanted (`n6`, which Locus's rules reject); and a snapshot taken where the bound could not be added is silently imprecise (`n2` prints `locus::Opaque` where the type is `u32`).

**Which programs still cannot name a snapshot's type,** with the best arrangement (the default):

- `Snap<X>` written as a type, for a concrete `X` that nothing declares a snapshot for: `fn named(s: Snap<BTreeSet<u32>>)` is E0277 (`n5`);
- `Snap<T>` or a type that holds one, for a type parameter, in a generic function that lowering did not write the bound on: not lowered (`n3`), or passing on to another invocation or crate (`n4`);
- a marker with a lifetime written by hand (`cases/h05`), which round 1 rejects by design;
- and two snapshot types in two `impl` headers still collide (`c07`, E0119), as in round 1.

### Positions and shapes

| | Case | Result, both expansions |
|---|---|---|
| A proposition in a struct field's type | `d1`: `ordered: @(lo <= hi)`, a named component `Vec<(m: u32, @(lo <= m && m <= hi))>`, a generic struct; all four specifications; `corpus/derive-eq-with-proof-field` | compiles. Round 1 did not attempt these; the corpus now has 52 of 52 pieces translated. `d1e`: a `Vec` compared with a number is E0277, and an unknown field name is ``E0425: cannot find value `high` in this scope`` |
| Contracts on trait methods with no body | `d2`: a runtime method with a proof parameter and a proposition on its result, one with a `where` clause, and a lemma | compiles and runs. `d2e`: E0277 in a runtime method's result and in a lemma's parameter. In round 1's sets the 10 propositions with "a declaration with no body to hold it" now have a skeleton: `cases` is 184 of 184 translated (175), `more` 53 of 53 (52) |
| A nested variant pattern | `d3`: two deep, with a generic enum inside | compiles. `d3e`: ``E0599: no variant .. named `Twig` found for enum `_::Tree` ``, `E0023: this pattern has 2 fields, but the corresponding tuple variant has 3 fields`, and E0308 for a binding of the wrong type. rustc does not check that the inner patterns are exhaustive |
| A logical enum imported by name | `d4`: `use super::shapes::Tree;`, a generic `Maybe<T>`, and a path through an alias of the module; round 1's `x14` | compiles. Nothing but the type is imported |
| `impl Model` and `#[derive(Model)]` | `d5`: a hand-written model, a derived struct with a proof field, a generic struct with a lifetime, an enum that contains itself through `Box`, `model(p).x`, a logic function on the model type, a generic runtime function | compiles; prints `"d5_models::Pair_Logic<locus::Nat, locus::Seq<locus::Int>>"` for `model(pair)` of a `Pair<'_, u8, Vec<i32>>` |
| Two crates | `two-lib`: two logical types, a generic logical enum, logic functions, a specification trait with a lemma, a generic logic function over it, a runtime trait with proofs in two methods' signatures. `two-app`: uses all of them, gives the specification trait to its own logical type and the runtime trait to its own runtime type, and matches on the library's enums | compiles in both expansions and prints `(13, 57, "two_lib::Rational", "two_lib::Sign", "app::Weight", "locus::Nat", "locus::Nat")` in both. The listings agree: `two_lib: 11 lines in each expansion's listing, identical`, `app: 25 lines in each expansion's listing, identical` |

- **The model of a generic type needs a bound that the program writes.** `fn first_of<A: Model, B: Model>(p: &Pair<'_, A, B>) where A::Logic: Logical, B::Logic: Logical` (`d5`): the derived model type is generic over logical types, and `Model::Logic` has no bound. Giving it one (`type Logic: Logical`) was tried and withdrawn, because `Seq<T>` is then its own model only for a logical `T`, and four of round 1's cases stopped compiling.
- **With round 1's hidden trait the two crates do not compile:** ``error[E0405]: cannot find trait `__locus_logic_OrderSpec` in this scope``, at `impl OrderSpec for Weight` in `two-app`. That is why the stubs are in the trait now. The cost is round 1's: a trait with a logic function cannot be implemented by plain Rust code.
- **A generic logical enum of another crate needs its arguments written** in a pattern: `Maybe::<Nat>::Just(n)` in `two-app`. Lowering knows the number of parameters only of a type of its own invocation or of the library. Written `Maybe::Just(n)` it is ``error[E0107]: missing generics for struct `Maybe` ``, at the user's `Maybe` (tried by hand; not a file of the suite).

### The first round's suite under the new defaults

First with the switches nearest to round 1's defaults, to see what the new lowering alone changed; then one change of policy at a time from there.

| Step | Files whose outcome or output changed | What changed |
|---|---|---|
| The new lowering, round 1's policy | 18 of 178: `cases` 5, `skel` 11, `more` 2 | 12 now compile: `t13a`, `t13c`, `t13d`, `t15`, `t17`, `s16` (stubs in the trait), `x11` (P5), `x14` (variants through the type), `f13`, `f14`, `f16`, `f17` (`as` is translated). 4 print `Proof` for `Erased`. `f15` prints `"Int"` for `"Untyped"`. `r07` is rejected with other codes |
| P1 | 24: `corpus` 4, `cases` 8, `skel` 8, `more` 4 | a lone literal is an `Int`: `Tree::Leaf => 0`, `double(3)`, `@Even(1)`, `let mut seen = logic! { 0 }` then a `Nat`. `x01` and `s21` now compile (they asked for a type) |
| P2 | 5: `corpus` 1, `skel` 4 | `Int + Nat` without `as Int` |
| P3 | 23: `cases` 13, `skel` 2, `more` 8 | in 20, a logic function declared `-> Bool` with a comparison as its body; `x10` (`twice(x > 0)` for a `Prop` parameter) now compiles; `t03` and `r06`, which round 1 also rejects, are rejected with one more error |
| Snapshots | 3: `ghost-lets` 1, `skel` 2 | all three now compile: the litmus example, `c05`, `c06` |

All together, against round 1:

```text
corpus: 26 files; compile in both expansions: round1 23, default 18; outcome or output differs in 5
ghost-lets: 17 files; compile in both expansions: round1 11, default 12; outcome or output differs in 1
cases: 58 files; compile in both expansions: round1 29, default 18; outcome or output differs in 21
skel: 38 files; compile in both expansions: round1 16, default 14; outcome or output differs in 19
more: 39 files; compile in both expansions: round1 21, default 15; outcome or output differs in 13
```

59 of 178 files change. Every case is listed with its first error in `run.log`, under "The first round's suite". The sources of these sets were not changed: they are written to RFC 0002's rules, and the drop from 100 to 77 compiling files is the policy, in files that were never edited for it. `corpus-explicit/` is the corpus edited, and has round 1's 23.

**The `f` cases no longer test the fallback.** They forced an initializer to have no skeleton with a cast, `v.len() as Int`, and a cast is now translated. The fallback's code is unchanged and was not run again.

### Both programs agree

The driver's listing of every call, method call and associated constant that the source wrote, in each expansion:

```text
all sets: binaries that both expansions accept: 126; lines in each listing: 577; binaries with a difference: 0
```

Round 1 had 0 of 387 over 100 binaries. By set: `corpus` 18 binaries and 44 lines, `ghost-lets` 12 and 16, `cases` 18 and 71, `skel` 14 and 72, `more` 15 and 52, `spec` 4 and 93, `policy` 21 and 149, `two-app` 1 and 25, `corpus-explicit` 23 and 55. Every binary that compiled both ways printed the same lines from both, and none reached a stub.

## What it means

1. **The explicit policy is cheap in specifications, and its cost is concentrated.** 10 changed lines in 371 over four specifications, 5 in 798 of litmus text. `as Int` clusters where a subtraction of `Nat`s meets a `Nat` again. Nothing is written for `Bool` and `Prop` under the default.
2. **A comparison as a `Prop` fits, with the implicit conversion.** In the corpus and the specifications nothing has to be written for it. What it changes is declarations: a logic function with a comparison as its body returns `Prop`, and 23 files of round 1's suite, written the other way, say so. The implicit conversion saves 4 places in the specifications and none in the corpus; all four are a runtime `bool` in a contract.
3. **A runtime `bool` result needs something RFC 0002 does not have.** `r <==> P` reads best; `r === decide(P)` needs no conversion under any variant and also serves where a `Bool` is wanted. The spike has both. Without either, the two implications work.
4. **The literal rule costs more than both conversions.** Seven of 17 written places, and four of the corpus's five lines, are `0 as Nat`. RFC 0002's "a `Nat` where a `Nat` is expected" removes most of them, and in this encoding it then leaves a literal given to a type parameter with no type. The callee-side encoding of `probes2/q2.rs` gives both: a `Nat` where one is wanted, an `Int` for a type parameter, forward.
5. **Messages can say what to write only where the failure is a bound.** That holds for an operator and for a `Bool` under the strict variant, after a trick with one more implementation. Everywhere else rustc reports a mismatch of two types, which is clear and gives no advice, and for a generic argument it asks for a type annotation, which misleads. A real Locus would have to rewrite those from the driver.
6. **P5 has an answer on stable; P4 has one that the spike did not build.** Method lookup's own order finds a logic function on the runtime type before its model, with the method's name as a type, with no guess about names. For P4 the callee must take its arguments through a bound, which costs the plain mismatch message and changes every stub's signature.
7. **Every type has a snapshot value, and not every type has a snapshot type.** `snap(x)` always types: declared, or opaque. `Snap<X>` written as a type needs an implementation or a bound, and lowering can add the bound only where it sees the need: not in a trait's `impl`, not in code it did not lower, not across invocations. Without specialization no encoding gives a foreign type's `Snap` a meaning, and the opaque value that works is one type for all of them.
8. **The shapes that round 1 left out have skeletons.** A field's proposition, a contract on a bodiless trait method, a nested pattern, an imported enum, both kinds of model, and two crates, each with a deliberate error that rustc reports in both expansions. Reaching a logical enum's variants through the type alone, by an alias of a projection, removes round 1's hidden name and its import problem together.
9. **Names: the first line is now the programmer's, the rest is not yet.** Operators, propositions and missing arms read as the source. Mismatches at an argument still show `Model::Logic` and `argument`, and method errors show `__logic_`.

## Where the translation guesses

Round 1's guesses stand (which names are places, that a capital before the last segment names a type, that `Name { .. }` is a struct literal, that a result written `Prop` is a proposition). Added in this round, each from the tokens alone:

| What it decides | How | What it would need |
|---|---|---|
| Where a cast's type ends | a path with no generic arguments, unless written `::<..>`, so `n as Int < m` is a comparison | the grammar |
| The type of a literal | beside one other operand: that operand's; an index: `Nat`; with `as T`: `T`; otherwise `Int`. A literal in parentheses is a literal | Locus's own typing, or the callee-side encoding |
| Whether a path in a pattern names a variant, and of what | two or more segments and a capital on the last; the owner is everything before it | name resolution |
| A logical enum's number of type parameters | known for a type of this invocation and of the library; otherwise none | the declaration |
| That a variant pattern inside another is exhaustive | not checked: it is matched on its own with a `_` arm | Locus's own check |
| That a logical enum, and a struct with a proposition in a field, is declared at module level | `self::Name` inside `const _` | the item's scope |
| Which `impl` is `impl Model`, which derive is `Model` | the name `Model` | name resolution |
| A model's logic function | its own generics and the `impl`'s `where` clause are not carried; `Self` is replaced as text | a parser of Locus |
| A derived model's field types | `<T as Model>::Logic` with every lifetime `'static`; a field whose type starts with `@` is left out | the types |
| Which type parameters need `Snapshot` | the tokens `Snap` and `snap(` in a function, a type or an `impl`; a call, by name, of a function of this invocation that got it | types, and other crates |
| Whether a method call's receiver is a place | syntax: a local, a field, a dereference, `final!` | nothing more |
| Which logic functions take `self` | the first parameter mentions `self`, in an inherent `impl` | traits' declarations |
| Where a trait's stubs are, for a trait of another invocation | in the trait | the trait's declaration |
| The precedence of `<==>` and `as` | `<==>` below `==>`; `as` above every binary operator | the grammar |

## What failed, and what was not done

- **P4's encoding is a probe, not in `locus!`.** Programs write `model(x)`, and the message for leaving it out is poor.
- **The specifications were not written for V1.** Its switch exists and the policy's cases were run under it.
- **`type Logic: Logical` on `Model` was withdrawn** (above), so a generic use of a derived model writes a `where` clause.
- **The first draft of `s1-stack` called lemmas from runtime code and panicked** with "a Locus stub was reached". Lowering does not erase a call of a logic function in runtime code; the specifications call lemmas inside `logic!`.
- **`tools/measure.py` reported a failure in a library crate as success** for the binaries that depend on it (seen with the hidden trait and two crates). It now gives the error to each binary.
- **rustc's message for an operator error is reported twice,** and shows `NoSuchOperand`.

## Not tested

- **Locus's own typing.** Nothing here types logical code but rustc. The counts are of what this encoding makes a program write, argued from RFC 0002's text for what it would not.
- Round 1's cost measurements, its `first-mode/` comparison, its hand probes `p1` to `p6`, the driver's check of the fallback, and its `expansions` were not run again. `scale/` was not built. Build time with the receiver and snapshot lookups, which add a trait import and a method call to every method call on a place, was not measured.
- A logic function that a trait gives a runtime type, called as a method; a method call whose receiver is not a place and is a runtime value (there is none in logical code); `x.f()` for `x: T` under a bound.
- `model(x)` and `snap(x)` of what is not a place. `Maybe`, `Set`, `Map`, `Unit`. Arithmetic on a snapshot's bounded integers. `as` between other types than `Nat`, `Int`, `Bool` and `Prop`.
- A proposition in a field of a tuple struct or of an enum's variant (recorded as not translated); a named component in a parameter's type; more than three named components.
- Three or more crates, a re-export of a logical type, a logical enum of another crate reached through `use ... as`, and a derived model of a type of another crate.
- A trait object of a trait that has a hidden contract method (the method is `where Self: Sized`, so the trait should stay usable as `dyn`; not compiled).
- Lints: the unused-variable warnings for ghost bindings that hold a lemma's result, and `unused_parens` around the receiver block, were seen and not addressed.
- rust-analyzer, rustfmt, release builds.
