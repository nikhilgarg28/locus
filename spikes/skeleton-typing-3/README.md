# Spike: typing skeletons, round 3: one family, by one implementation for each type

A throwaway experiment that extends `spikes/skeleton-typing-2` (round 2). It was built outside the repository, from a copy of that spike, and depends on nothing in the repository when it is built (one tool reads the repository's litmus tree as text). Nothing may depend on it.

Round 2 could not give every type a snapshot type: it inserted `T: Snapshot` bounds by heuristics, and `snap(x)` fell back to one opaque type. That fallback is rejected as a design. This round puts the design recommended to the owner on 2026-11-22 into the real lowering, with the six additions the reviewer asked for, and measures it.

## Question

1. **Does one implementation for each type work in the macro?** One trait, `Model`, with two associated types, the snapshot and the model; written by the library for the language's types, and by `locus!` where a struct or an enum is declared; with no fallback, and no bound that lowering inserts.
2. **Do capture and logic functions over logical types only remove round 2's choices?** A runtime place is its capture, always, at an argument too; a method call resolves on the captured value's type and nothing else is tried.
3. Does it pass the reviewer's seven required cases?
4. Can `Model` be closed to forgery with `unsafe trait`, and can the driver tell lowering's implementations from a hand's?
5. What do the four specifications and the litmus corpus now have to write, by cause?
6. Is there a type for the ghost field of Locus's `Vec` that needs no bound on the struct? Does a mutable element loan's contract type?
7. What does the plain form (an error at the declaration for a field with no model) cost against the conditional form?
8. Does what worked in round 2 still work?

## Setup

A Cargo workspace of its own, stable rustc 1.97.0 and cargo 1.97.0. `./run.sh 2>&1 | tee run.log` reproduces everything; `run.log` is its output (79 seconds with `target/` built, 861 lines). `[profile.dev]` is round 2's.

- `locus/` is the library (1,383 lines; 1,153 in round 2). `locus-macros/` is `locus!`: `src/family.rs` is new (718 lines), `src/skeleton.rs` 1,321 (1,339), `src/lib.rs` 4,244 (4,380). `driver/` is round 2's rustc driver with one more listing (409 lines).
- Round 2's sets, **unchanged**: `corpus/` (26), `ghost-lets/` (17), `cases/` (58), `skel/` (38), `more/` (39), `spec/` (4, regenerated from `spec/master/`), `policy/` (35), `spec-omit/` (8), `corpus-explicit/` (26). `two-lib/` and `two-app/` are rewritten and extended; round 2's text is in `baseline/round2-src/`. Round 2's reports are in `baseline/round2-*.json`, copied from its `out/`.
- New sets:
  - `family/`, 47 cases. The first line of each says what the two compilations must do. `a*`: the family and capture; `c*`: Locus's `Vec`; `d*`: the reviewer's seven required cases; `e*`: closures, function items, trait objects; `f*`: forgery inside `locus!`; `g*`, `h*`, `m*`, `v*`: the regressed litmus example, `#[derive(Model)]`, `model(s)`, validity.
  - `spec3/`: the four specifications rewritten for the new rules. Round 2's are beside them in `spec/`. `tools/spec3-table.py` holds the cause of every changed line and checks that list against `difflib`.
  - `ghost-lets3/`: `ghost-lets/` with the one line that this round's rules make it write.
  - `two-plain/`: a crate that is NOT lowered (plain Rust, no `locus!`), with a forged implementation of `Model`.
- Round 2's `first-mode/`, `scale/` and `probes/` were deleted from the copy. `probes2/` is kept and not run.

Round 2's conversion policy and its switches are exactly as they were. This round's switches:

| | Default | Switch |
|---|---|---|
| A field whose type has no model | **The plain form:** an error where the struct is declared, unless the field is marked `#[opaque]` | `LOCUS_SPIKE_DEFERRED=1`: the conditional form of capture-family `c11` (a `where` clause behind a higher-ranked lifetime) |
| The standard library's own `Vec` | Round 2's model, a `Seq`, kept so that the earlier sets, which read a `std::vec::Vec`, can be compared. **The design gives it none** | `--features locus/no-std-vec-model` |

### What the library is now

```rust
pub trait Logical: Copy + 'static + Model<Snap = Self, Logic = Self> { .. }

#[diagnostic::on_unimplemented(message = "`{Self}` cannot be read in logical code", ..)]
pub unsafe trait Model {
    type Snap: Logical;                               // snap(x)
    type Logic: Logical;                              // x, used as a value
    fn __logic_model(s: Self::Snap) -> Self::Logic;   // X::model(s)
    fn __logic_valid(s: Self::Snap) -> Prop;          // X::valid(s)
}
pub type Snap<T> = <T as Model>::Snap;
```

- **Implemented once each** for: the machine integers (`Nat` or `Int` for BOTH types; round 2's twelve bounded-integer types are gone), `bool`, `&T`, `&mut T` and `Box<T>` (as `T`'s), tuples of two and three, arrays and slices (`Seq`, part by part), `Option` (`Maybe<..>`, a logical enum of the library, the name RFC 0002 uses), Locus's `Vec`, and `Cell`, `RefCell`, `String`, `str` and raw pointers as round 2 had them, and `PhantomData` (each has a snapshot type of its own, which is also its model). The library has 47 implementations in all, its logical types' own included.
- **Removed:** `Snapshot`, `Runtime`, `GivenAs`/`argument` (round 2's P4), `Declares`/`OnTheType`/`OnTheModel` and the name-as-a-type (P5), `SnapDeclared`/`SnapOpaque` and the one `Opaque` (the fallback).
- `logic::capture(&place)` is the model, `logic::snap(&place)` the snapshot, `logic::model::<X>(s)` applies `X`'s model to a snapshot value, and `logic::value(&name)` is a name inside a logic function, which is itself.

### What `locus!` writes now

Only what differs from round 2.

| Source | Lowering |
|---|---|
| `x`, `p.a`, `*r` in logical code of a runtime function, as a value **or as an argument** | `logic::capture(&x)`: field selection on the place first, then capture |
| the same inside a logic function, a `prop`, or a lemma's signature | `logic::value(&x)`: every name there is a logical value |
| `snap(x)`; `model::<X>(s)`; `X::model(s)`; `X::valid(s)` | `logic::snap(&x)`; `logic::model::<X>(s)`; `X::__logic_model(s)`; `X::__logic_valid(s)` |
| `x.f(a)` for a place `x` | `logic::capture(&x).__logic_f(a)`. Nothing else is tried |
| `struct Stack { items: Vec<u32>, cap: usize }` | the struct; `struct Stack_Snap<F0, F1> { items: F0, cap: F1 }` with `Clone`, `Copy`, `Debug`, `Logical`, `Matchable`; and `unsafe impl Model for Stack { type Snap = Stack_Snap<<Vec<u32> as Model>::Snap, <usize as Model>::Snap>; type Logic = Self::Snap; fn __logic_model(s) { s } fn __logic_valid(s) { .. } }` |
| the same with `impl Model for Stack { type Logic = Seq<Nat>; logic fn model(s: Snap<Stack>) -> Seq<Nat> { .. } }` in the invocation | the struct and `Stack_Snap` only; the program's `impl` becomes the one `unsafe impl`, with `type Snap` and `valid` supplied, and `model` as the trait's own member |
| `struct Pair<T> { first: T, rest: Vec<T> }` | `unsafe impl<T: Model> Model for Pair<T>` |
| `struct Kept<F: Family> { out: F::Out, n: u32 }` | `unsafe impl<F: Family> Model for Kept<F> where F::Out: Model` |
| `#[opaque] sock: TcpListener`, `#[opaque] name: &'a str` | that parameter of the snapshot is `Opaque<TcpListener>`, `Opaque<&'static str>` |
| `enum Shape { Dot, Circle(u8), Boxed(Box<Shape>) }` | `Shape_Snap<F0>`, a zero-sized struct, and inside `const _` an enum `Shape<F0> { Dot, Circle(F0), Boxed(<Box<self::Shape> as Model>::Snap) }` that only `Shape_Snap`'s `Matchable` names. A pattern `Shape::Circle(r)` is `Variants::<Shape_Snap<_>>::Circle(r)` |
| `logic fn top(stack: &Stack)`, `logic fn room(self)` in `impl Stack` | an error from lowering |

**The snapshot type is generic over the snapshot types of its fields,** one parameter for each field, so its declaration names no type of the program and needs no bound, and no lifetime of the type reaches it (rustc normalizes `<&'a mut u32 as Model>::Snap` to `Nat`). A field whose type names the type itself cannot be a parameter (the type would contain itself), so its projection is written in the declaration, with every lifetime `'static`.

Every token of a generated `impl` carries the mark of the expansion (`Span::call_site().located_at(the type's name)`), which is what the driver reads.

## Results

Run on 2026-11-22. Every number below is from `run.log`.

```text
family: 47 cases; as their first line says, in both expansions: 47; compile and run in both: 20
all: outcome or output differs in 47: param 25; snapshot-trait 2; unbounded 3; no-model 2; names 3; now-ok 4; other-errors 8
all sets: binaries that both expansions accept: 151; lines in each listing: 718; binaries with a difference: 0
declarations of structs and enums that lowering saw: declared model 22; default model 104; derived model 6
all: 3 declarations need an `#[opaque]` mark, and 2 a bound on the type
```

### A. One implementation for each type

**It works in the macro.** In the binaries of every set, lowering saw 132 declarations of structs and enums and wrote the family for each: 104 with the default model, 22 with a model the program declares, 6 with `#[derive(Model)]`. No file failed because an implementation could not be generated. Where it could not be generated, and what happens:

- **A generic type that contains itself** (`struct List<T> { head: T, tail: Option<Box<List<T>>> }`): none is written, and the type is unreadable. The recursive field's projection names `T`, which the snapshot type does not have. No set holds one; a non-generic one works (`v1`: `Boxed(Box<Shape>)`; `h1`: `Cons(u32, Box<List>)`).
- **A union:** none is written.
- **A type that contains itself through another type** is not seen by lowering (not tested; rustc would report an overflow).

| Case | Result, both expansions |
|---|---|
| `a01` the identity default | compiles. `logic! { p }` is `Point_Snap<Nat, Int>`; `before.x` and `p.x` are both `Nat`; `@(p.x === before.x + 1)` types |
| `a02` a declared model, and a wrapper with no model around it | compiles. `w.s` is `Seq<Nat>`; `whole.s` (captured, then projected) is `Stack_Snap<Seq<Nat>, Nat>`; `Stack::model(whole.s)` and `model::<Stack>(whole.s)` are `Seq<Nat>` |
| `a03` the two confused | E0308, ``expected `Seq<Nat>`, found `Stack_Snap<Seq<Nat>, Nat>` `` |
| `a14` **a declared model of the snapshot's own type** | **compiles, silently.** See below |
| `a04` a generic type | compiles; `Pair_Snap<Nat, Seq<Nat>>` in concrete and generic code; a `Pair<Stack>`'s snapshot holds `Stack`'s snapshot, not its model |
| `a05`, `a05e` a field of type `F::Out` | compiles with `where F::Out: Model` on the function that reads; E0277 ``` `<F as Family>::Out` cannot be read in logical code ``` without |
| `a06`, `a06e` `#[opaque]` fields | compiles; `Server_Snap<Opaque<TcpListener>, Opaque<SocketAddr>, Opaque<&str>, Nat>`, used after the borrow in `name` ends; mixing two is E0308 |
| `a06f` an opaque field selected on the place first | E0277: `s.sock` is a place of the foreign type. It is reached as `snap(s).sock` |
| `a07`, `a07r` a foreign field with no mark | E0277 at the declaration (plain form) |
| `a11` a model declared in another invocation | an error from lowering, below |
| `h1` `#[derive(Model)]` (round 2's `d5`, for the new rules) | compiles; round 2's `where A::Logic: Logical` is no longer written |

**A model declared in another invocation than its type** is refused by lowering, which sees no declaration there:

```text
error: locus: a model is declared in the `locus!` that declares its type, where lowering writes the type's one implementation of `Model`: `Stack` is not declared here
 --> family/src/bin/a11-model-in-another-invocation.rs:9:5
```

Had lowering written an implementation there, the other invocation's default would make it rustc's E0119, which `d6e` shows for a second implementation written by hand (``conflicting implementations of trait `Model` for type `Point` ``). **If a model must be allowed elsewhere,** the mark has to be on the type's declaration, since that is where the default is written: something like `#[model(declared)]` on the struct, meaning "write the snapshot type and no implementation". The other invocation would then also need the snapshot type's arguments, which are the types of fields it cannot see, so the declaration's invocation would have to publish them, for instance through a second, hidden trait that carries only the snapshot (`impl SnapshotOf for Stack { type Snap = Stack_Snap<..> }`). Not built.

**The silent case (`a14`).** `Inner { n: u32 }` declares a model that is its snapshot with `n` replaced by 0, so `Logic` is `Snap<Inner>`. Inside `Outer { inner: Inner }`, which has no model:

```text
run() => ("a14_..::Inner_Snap<locus::Nat>", "a14_..::Inner_Snap<locus::Nat>", "locus::Prop")
```

`logic! { whole.inner }` (captured, then projected: `n` is 7) and `logic! { outer.inner }` (selected, then captured: `n` is 0) have the **same type**, `===` between them types, and rustc reports nothing in either expansion. The difference between the two orders is a type error only while a model's type is not its snapshot's type.

**No bound is inserted.** rustc's message is the same in a function (`a08`), an `impl` (`a09`) and a trait's method (`a10`), and its own help says what to write:

```text
error[E0277]: `T` cannot be read in logical code
   --> family/src/bin/a09-missing-bound-impl.rs:7:46
    |
  7 |         fn count(&self) -> (n: usize, @(n == self.items.len())) { (self.items.len(), _) }
    |                                              ----^^^^^^
    |                                              |
    |                                              this type has no snapshot that Locus knows
    |                                              required by a bound introduced by this call
    |
    = note: for a type parameter, add the bound `T: Model`
    = note: for a type of a crate that Locus does not check, mark the field `#[opaque]` or wrap the type in a struct of your own
    = note: required for `std::vec::Vec<T>` to implement `Model`
note: required by a bound in `capture`
help: consider restricting type parameter `T` with trait `Model`
    |
  6 |     impl<T: locus::Model> Shelf<T> {
    |           ++++++++++++++
```

In the function the help is `fn g<T: locus::Model>(x: &T)`, and in the trait's method `fn keep<T: locus::Model>(&self, x: &T)`, which rustc then refuses if the trait's declaration does not have it (`d2c`). The second note is beside the point for a type parameter, and the first for a foreign type: a note cannot depend on the type. rustc reports each twice where a `logic!` holds one read (once for `snap`, once for the marker's type).

### B. Capture, and logic functions over logical types only

- **A place given as an argument is its capture.** Round 2's `model(x)` at a generic argument is gone: `policy/p4b` and `skel/x05`, which were E0283 "type annotations needed", now compile, and the two `model(x)` of `s4-container` are no longer written.
- **A method call on a place captures it.** `s.len()` for a `Stack` is its model's `len`. Round 2's lookup order is gone.
- **A logic function over a type's snapshot is called by its path:** `Stack::room(snap(s))`, for `logic fn room(s: Snap<Stack>) -> Int` in `impl Stack` (`a02`, and `s1-stack`). **`snap(s).room()` does not work** (`a13e`): the stub is on `Stack` and the snapshot is another type, ``no method named `__logic_room` found for struct `Stack_Snap<F0, F1>` ``. Making it work needs a second stub on the snapshot type, which lowering can write only where it sees the type declared.
- **`model(s)` needs its type written** (`m1e`): bare `model(snap(s))` is E0282, "type annotations needed". A snapshot type does not say what it is a snapshot of: `Snap<u32>` and `Snap<u64>` are both `Nat`. `model::<Stack>(s)` and `Stack::model(s)` both work, and the second reads better.
- **A runtime-typed parameter is an error from lowering,** which says what to write (`a12`):

```text
error: locus: a logic function's parameters are logical types, and `stack : & Stack` is a runtime type: write `Snap<Stack>`, or the type of its model
error: locus: a logic function's parameters are logical types, and `xs : & [u32]` is a runtime type: write `Snap<[u32]>`, or the type of its model
error: locus: a logic function's parameters are logical types, and `self` is a runtime type: write `Snap<Stack>`, or the type of its model
```

- **Inside a logic function nothing is captured.** Every name there is `logic::value(&x)`, which asks nothing of the type, so `logic fn first<T>(s: Seq<T>) -> T` needs no bound, as in round 2. The cost: lowering's test for a runtime type is the only check that a parameter is logical (see the guesses).
- **A trait with a logic function over the implementing type's snapshot asks for `Model`:** `pub trait Bounded: Model { logic fn limit_spec(s: Snap<Self>) -> Nat; .. }` (`two-lib`).

### The seven required cases

| | Case | Files | Result, both expansions |
|---|---|---|---|
| 1 | A generic snapshot used where the concrete type is wanted | `d1` | compiles. `keep(&5u32)` is `locus::Nat`, given to `logic fn takes_nat(n: Nat)`, and `===` a snapshot of a `u64` |
| 2 | A snapshot inside a trait implementation that cannot strengthen a method's bounds | `d2a`, `d2b`, `d2c` | A trait of the program's own declares the bound, on the method or as a supertrait: compiles. A foreign trait's generic method (`Extend::extend<I>`): E0277 ``` `I` cannot be read in logical code ```, and with the bound added E0276 "impl has stricter requirements than trait". **A stated limit** |
| 3 | Through another invocation and another crate | `d3`; `two-lib`, `two-app` | compiles. `two_lib::keep(&rig)` for a struct of `two-app` that holds three types of `two-lib` is `app::Rig_Snap<two_lib::Meter_Snap<Nat>, two_lib::Stack_Snap<Seq<Nat>, Nat>, two_lib::Pair_Snap<app::Scale_Snap<Nat>, app::Scale_Snap<Nat>>>`; a logic function of the library over `Snap<Meter>` takes it |
| 4 | A reference's snapshot kept after the borrow ends | `d4` | compiles and runs: through a `&mut`, of a struct with `&'a mut` and `&'a [u32]` fields, and in generic code, each used after the place is written |
| 5 | Two foreign snapshot types kept distinct | `a06`, `a06e` | `Opaque<TcpListener>` and `Opaque<SocketAddr>`; mixed, E0308 |
| 6 | No overlap between implementations | `d6`, `d6e` | compiles: `&mut Box<[u32; 3]>`, a tuple with an `Option`, a slice of a declared type, `Option<(usize, Vec<bool>)>`, `&&&u64`, each the same type in concrete and generic code. There is no blanket implementation. A second implementation is E0119 |
| 7 | Model selection independent of where the declaration is visible | `d7`, `d7e` | compiles: `Seq<Nat>` in the declaring module, in a module that imports only the type, through a bounded parameter, and through another module's generic function; a `u32` is `Nat` both ways. Through an unbounded parameter it is E0277, where round 2 gave another type |

### Forgery and authorization

`Model` is an `unsafe trait`, and lowering writes `unsafe impl`. A crate that says `#![forbid(unsafe_code)]` still compiles with lowering's own (`f2`).

- **(a) A generated implementation in a downstream lowered crate works:** `two-app`'s `Rig`, above.
- **(b) In `two-plain`, which is not lowered,** a hand-written implementation without `unsafe` is refused, and with it compiles and is used by the lowered library's generic function exactly as the forger wrote it (`forged => ("locus::Nat", "locus::Seq<locus::Nat>", "locus::Nat")`):

```text
error[E0200]: the trait `Model` requires an `unsafe impl` declaration
 --> two-plain/src/bin/forge-without-unsafe.rs:9:1
  |
9 | impl locus::Model for Mine {
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: the trait `Model` enforces invariants that the compiler can't check. Review the trait documentation and make sure this implementation upholds those invariants before adding the `unsafe` keyword
help: add `unsafe` to this trait implementation
```

- **(c) The driver tells them apart.** With `LOCUS_MODELS_DIR=dir` it writes, for each crate it compiles, every implementation of `Model` that crate can see (`tcx.all_impls`), its own and its dependencies', with the crate, the type, and the origin read from the implementation's span:

```text
-- forge_with_unsafe.models: 47 of Locus's library, and:
forge_with_unsafe | Mine | WRITTEN BY HAND
forge_with_unsafe | Other | WRITTEN BY HAND, through the macro `locus`
two_lib | two_lib::Meter | lowering: the expansion of `locus!`
two_lib | two_lib::Meter_Snap<F0> | lowering: the expansion of `locus!`
two_lib | two_lib::Stack | lowering: the expansion of `locus!`
...
-- over every crate of two-app, two-plain and family that compiled: implementations outside Locus's library, by origin
      1 WRITTEN BY HAND
      1 WRITTEN BY HAND, through the macro `locus`
     80 lowering: the expansion of `locus!`
```

`Other` is forged through a `macro_rules! locus` of the forger's own, named as Locus's is; the listing shows the macro is not Locus's. The mark survives metadata: `two-lib`'s implementations are listed as lowering's when seen from `app` and from the forger's crate.

**What a forger could still do, and what the listing catches.**

- *Caught:* any `unsafe impl Model` whose tokens were not written by the expansion of Locus's `locus!`: in an unlowered crate, in a lowered crate outside `locus!`, or through any other macro. Inside `locus!` a hand-written one cannot even be spelled, since the trait's members have names with the prefix reserved for lowering (`f1e`: ``` `__logic_model` starts with a prefix that is reserved ```), and an `impl Model for X` there is always the declared model of a type declared there.
- *Caught only if someone looks:* the listing is made when a crate that can see the implementation is compiled with the driver. A build with plain `cargo build` lists nothing, and nothing here *refuses* a hand-written implementation; the spike only lists.
- *Not caught:* the driver knows the library and the macro by crate name (`locus`, `locus_macros`) and the macro's name. A forger's own crate named `locus_macros` with a macro `locus` would be listed as lowering. A real check has to identify the crate, not its name.
- *Not forgery, and not looked at:* a program may declare any model for its own type; whether that model is sensible is Locus's check, which this spike does not have. And the listing says nothing of whether a generated implementation is what the declaration should give, for instance under a modified macro crate.

### E. The four specifications

Round 2's text, unedited, under the new rules: `s3-binary-search` compiles (it is over a slice and its logic functions already take logical types); `s1`, `s2` and `s4` each fail first on `logic fn model(x: &X)`. Rewritten (`spec3/`), **all four compile in both expansions, run, and print the same lines**; 102 pieces of logical code, all translated; their call listings have 82 lines and agree. They use Locus's own `Vec` by an explicit import, as RFC 0002 decides, and compile unchanged with the standard `Vec` given no model.

Every place where the source now writes something it did not in round 2 (`tools/spec3-table.py`, which fails if its list is not exactly the changed code lines):

```text
                   lines changed    bound   opaque     snap    model    param   method elements   result   import     less
s1-stack              93      11        0        0        5        0        2        5        2        3        1        1
s2-sorted-vec         91       5        0        0        0        0        1        0        1        1        2        1
s3-binary-search      81       0        0        0        0        0        0        0        0        0        0        0
s4-container         110      13        2        0        0        0        1        0        6        1        1        3
all four             375      29        2        0        5        0        4        5        9        5        4        5
places where round 3 writes something round 2 did not: 34; places where it writes less: 5
```

| Column | Cause | Places |
|---|---|---|
| bound | a `T: Model` bound | 2 |
| opaque | an `#[opaque]` mark | 0 |
| snap | `snap(..)` | 5 |
| model | `model(..)` | 0 |
| param | a logic function's parameter rewritten from a runtime type | 4 |
| method | a method call moved from a place to its snapshot | 5 |
| elements | other: `.elements()` on a vector's snapshot (Locus's `Vec`) | 9 |
| result | other: a library method's result carries its fact (`.len().0`) | 5 |
| import | other: `use ::locus::Vec;`, and one `std::vec::Vec` written out | 4 |
| less | written no longer: `model(x)` of a place | 5 |

```text
s1-stack: snap: lines [36, 48, 49, 69]; param: lines [19, 25]; method: lines [36, 48, 49, 69]; elements: lines [19, 25]; result: lines [41, 60, 65]; import: lines [9]; less: lines [51]
s2-sorted-vec: param: lines [44]; elements: lines [44]; result: lines [60]; import: lines [9, 78]; less: lines [56]
s3-binary-search: nothing
s4-container: bound: lines [57, 63]; param: lines [52]; elements: lines [60, 64, 68, 69, 77]; result: lines [103]; import: lines [10]; less: lines [79, 82, 83]
```

- **The five `snap(..)` are the five moved method calls:** every one is `s.room()` become `Stack::room(snap(s))`. No `snap(..)` was needed anywhere else.
- **The two bounds are on a type and its `impl`:** `pub struct Shelf<T: Model>`, because its invariant, `fits: @(snap(items).elements().len() <= limit)`, reads a field whose type names `T`. Round 2 inserted that bound; now the program writes it, and Rust then asks for it on every `impl` of `Shelf`.
- **18 of the 34 places are the library's `Vec`, not the family:** the observer on its snapshot (9), results that carry their fact (5), and the import (4). With RFC 0002's decision 87 (a method that returns a value keeps the standard shape) the 5 `.0` go. The 9 `.elements()` are what recommendation 8's structural snapshot costs; 6 of them are in `s4`, where the length could be said of the model instead (`items.len()`) with nothing written.

**The three that read worst:**

```rust
// s1-stack.rs:49: three `Stack::room(snap(..))`, counting line 48's
-> @(final!(self) === pushed(*self, x) && Stack::room(snap(final!(self))) == Stack::room(snap(self)) - 1)
// s4-container.rs:69
-> @(snap(final!(self).items).elements().len() == snap(self.items).elements().len() + 1)
// s1-stack.rs:36
pub fn new(cap: usize) -> (s: Stack, @(s.len() == 0 && Stack::room(snap(s)) == cap as Int)) {
```

In round 2 the first was `final!(self).room() == self.room() - 1`.

**Where the macro guessed in them:** that `Stack`, `SortedVec`, `Even` and `Shelf` are the types of their `impl Model` (by name); that `stack: Snap<Stack>` is logical (by the name `Snap`); that `Stack::room` names a type's logic function (a capital); that `Shelf`'s parameter needs `T: Model` was NOT guessed: the program wrote it.

### E. The litmus corpus

**No file of the unedited corpus that compiled in round 2 fails under the new rules, and one litmus example of `ghost-lets/` does.**

| | Files | Compile in both, round 2 | Round 3 | Changed |
|---|---|---|---|---|
| `corpus/`, as the litmus text | 26 | 18 | 18 | 0 |
| `corpus-explicit/`, with round 2's edits for the conversion policy | 26 | 23 | 23 | 0 |
| `ghost-lets/` | 17 | 12 | 11 | 1 |
| `ghost-lets3/`, this round's edited copy | 17 | | 12 | |

So the edited copy that compiles as far as round 2's did is `corpus-explicit/` unchanged and `ghost-lets3/`, with **one line of one file**, by cause: a `Model` bound 1; everything else 0.

```text
< gl-snapshot-with-associated-type-field.rs:20:fn kept<F: Family>(h: Holder<F>) -> Holder<F> {
> gl-snapshot-with-associated-type-field.rs:20:fn kept<F: Family>(h: Holder<F>) -> Holder<F> where F::Out: Model {
```

**That file is the example that regressed in round 1** (`litmus/models/snapshot-with-associated-type-field.accept.lc`). The struct's declaration compiles as written, since its generated implementation is conditional on `F::Out: Model`. The function that takes its snapshot writes that bound. The other way, marking the field `#[opaque]` and writing no `Model` bound, is E0310 (`g2`): `Opaque<F::Out>` keeps whatever lifetimes `F::Out` has, so rustc asks for `where F::Out: 'static` in its place (`g2w` compiles with it).

**Why so little changes: this corpus holds none of the examples the rule is aimed at.** `tools/litmus-scan.py` applies lowering's test for a runtime-typed parameter to the text of the repository's whole litmus tree:

```text
474 examples; 26 logic functions in 19 files declare a parameter with a runtime type (by outcome: accept 7, reject 12)
```

None of the 19 is among the 39 litmus examples in `corpus/` and `ghost-lets/`. They were not built here.

**With the standard library's own `Vec` given no model,** as the design has it, more fails, with ``` `std::vec::Vec<u32>` cannot be read in logical code ``` (or `Vec<{integer}>`): `corpus-explicit/` 22 of 26 compile (23), `ghost-lets3/` 7 of 17 (12). Those files read a `vec![..]` in logical code and would import Locus's `Vec`.

### C. Locus's `Vec`, with its contents in a ghost field

```rust
pub struct Vec<T> { inner: std::vec::Vec<T>, contents: Contents<T> }   // no bound on T
pub struct Contents<T> { __locus: PhantomData<fn() -> PhantomData<T>> }
unsafe impl<T: Model> Model for Contents<T> { type Snap = Seq<T::Snap>; type Logic = Seq<T::Snap>; .. }
unsafe impl<T: Model> Model for Vec<T> { type Snap = VecSnap<T::Snap>; type Logic = Seq<T::Logic>; .. }
impl<S> VecSnap<S> { pub logic fn elements(self) -> Seq<S>; }
```

- **A field type that needs no bound exists, and it is nominal:** `Contents<T>` names `T` and asks nothing of it. What the field holds, `Seq<Snap<T>>`, is a type only where `T: Model`, so it is what the field is *read as*, under the observer's bound. Written as the sequence itself the field is an error at the struct (`c6e`): ``` `T` cannot be read in logical code ``` at `contents: Seq<Snap<T>>`, with rustc's help to put `T: locus::Model` on the struct.
- **What the nominal type costs:** it is not a logical type (it names `T`, so it is not `'static` for a borrowed `T`, and it is not its own snapshot), so `logic!` cannot produce it. The library writes the field through a constructor, `self.contents = Contents::holding(now)`, whose argument is the typed sequence.
- **With no bound on the struct** (`c1`), a `Vec<{closure}>` is named, built with `from_std`, and read through `Deref`; `fn count<T>(v: &Vec<T>)` needs nothing.
- **A snapshot of a `Vec<&mut u32>` keeps no borrow** (`c1`): it is `locus::VecSnap<locus::Nat>`, used after the vector is dropped and the place written.
- **What a bound on the struct would cost** (`c7e`): `fn count<T>(v: &Bounded<T>)`, which reads nothing, is E0277 with ``required by this bound in `Bounded` ``, and no closure can be an element.
- **Only the wrapper's own methods change it.** There is no `DerefMut`: `v.truncate(0)` is ``E0596: cannot borrow data in dereference of `locus::Vec<u32>` as mutable`` (`c5e`). Shared `Deref` stays. `into_std(self)` and `from_std(v)` are the way out and back.
- **The methods with contracts are in `impl<T: Model> Vec<T>`,** each over the elements' snapshots: `push`, `pop`, `len`, `get`, `get_mut`, `insert`. At an element type with no model (`c4e`) `push` is E0277 on the closure: rustc commits to the wrapper's method, and with no `DerefMut` there is nothing to fall through to.
- **The mutable element loan types in the skeleton,** in the library and through a caller:

```rust
pub fn get_mut(&mut self, i: usize, ok: @(i < self.len()))
    -> (r: &mut T, @(snap(final!(self)).elements() === snap(*self).elements().set(i, snap(final!(r)))))
```

  In this spike `final!(r)` is already the place `r` ends with, so the reviewer's `*final self` is `final!(self)` here; written `*final!(self)` it dereferences the `Vec` to the standard vector, through `Deref`, and silently types as that (this was my first draft, and rustc reported it only because `elements` was missing on a `Seq`).
- **Nested** (`c2`): `nested.get_mut(0, nonempty)` then `first.push(x)` for a `Vec<Vec<u32>>` compiles and runs, with a contract on the whole; the model is `Seq<Seq<Nat>>`.
- **`IndexMut` cannot carry the contract** (`c3e`): the trait fixes the signature.

```text
error[E0053]: method `index_mut` has an incompatible type for trait
   = note: expected signature `fn(&mut Row, _) -> &mut u32`
              found signature `fn(&mut Row, _) -> (&mut u32, Proof)`
```

  The library's own `IndexMut` is there and has no contract.
- **The snapshot is hand-written, not generated.** A field marked `#[opaque]` whose type names `T` (`inner: std::vec::Vec<T>`) would put `T`, and its lifetimes, into the snapshot type, so the library writes `VecSnap<S>` itself, over `T`'s snapshot only.

### The plain form and the conditional form

The default is now the plain form. Under it, over all 252 files of rounds 1 and 2 and the 12 specifications and copies of this round:

```text
corpus: 0 declarations need a mark        cases: 0        skel: 0        more: 0        spec: 0        spec3: 0
policy: 0 declarations need a mark; 2 need a bound: n1-snapshot-bounds-inserted:13 `Keep`: `T`; n4-snapshot-other-invocation:12 `Keep`: `T`
family: 3 declarations need a mark: a07..:8 `Server`: `TcpListener`; a07r..:6 `Server`: `TcpListener`; e2f..:7 `Scene`: `(dyn Shape + 'static)`
all: 3 declarations need an `#[opaque]` mark, and 2 a bound on the type
```

**No declaration in the corpus, the cases or the specifications needs an `#[opaque]` mark.** The three that do are cases written for it; 15 fields in 7 other files of `family/` carry the mark. One caveat: six declarations in `more/` and `cases/` hold a `PhantomData`, for which the spike's library declares a model; had it not, they would need the mark (counted by `grep`, not built that way).

The message of the plain form, at the declaration. It names the field's type and not the field:

```text
error[E0277]: `TcpListener` cannot be read in logical code
 --> family/src/bin/a07-foreign-field-unmarked.rs:8:12
  |
8 |     struct Server { sock: std::net::TcpListener, n: u32 }
  |            ^^^^^^ this type has no snapshot that Locus knows
  |
  = help: the trait `Model` is not implemented for `TcpListener`
  = note: for a type parameter, add the bound `TcpListener: Model`
  = note: for a type of a crate that Locus does not check, mark the field `#[opaque]` or wrap the type in a struct of your own
  = help: the following other types implement trait `Model`: &T, &mut T, (A, B), .. and 40 others
  = note: this error originates in the macro `locus`
```

**The conditional form, as built, accepts two files that the plain form refuses, and refuses fourteen that the plain form accepts.** 20 files differ:

- *Accepted only by it:* `a07` and `e2f`, the unmarked field in a struct that is not read. Read (`a07r`), the error comes at `snap(s)`, with one more note: ``required for `TcpListener` to implement `Deferred<'static>` ``.
- **Refused only by it, 10 files: every declared model that looks into a field** (`a02`, `a04`, `a14`, `d3`, `d7`, `s1-stack`, `s2-sorted-vec`, `s4-container`, `two-lib` and so `app` and `two-plain`). Inside the implementation the `where` clause is in scope, and rustc then does not normalize the field's snapshot type: ``no method named `__logic_elements` found for associated type `<locus::Vec<u32> as Deferred<'static>>::Snap` ``, or E0308 where the field is the model. This is the trick's own behavior, not the toy's.
- *Refused only by it, as built, 4 files:* a field with a lifetime of the declaration (`skel/c01`, `d4`: E0308 and "lifetime may not live long enough" at the struct), and a type that contains itself (`v1`, `h1`: ``E0275: overflow evaluating the requirement `Box<Shape>: locus::Model` ``).
- The other four (`policy/d5`, `d6e`, `m1e`, `forge-without-unsafe`) are rejected in both forms, with other errors.

Nothing else in the spike rests on the higher-ranked clause.

### Validity as a generated predicate

`fn __logic_valid(s: Self::Snap) -> Prop` is a required member of `Model`, called as `X::valid(s)`. Lowering writes it for each declared type as the conjunction of each field's, and the library's are written by hand over the same stubs a skeleton uses (`u8`: `s <= 255`; a signed type: both ends; `[T; N]`: `s.len() == N` and each element's; `[T]`: each element's; `&T`, `&mut T`, `Box<T>`: the target's; a tuple: each part's; `Option`: a `match` on `Maybe`, the payload's; a logical type and an opaque field: `True`).

**It types** (`v1`, both expansions): for a struct with `u8`, `[u8; 4]` and opaque fields; an enum with unit, tuple and named variants and one that contains itself through `Box`; a generic struct with `T`, `Vec<T>` and `Option<Box<T>>`; `<[u8; 4]>::valid(snap(arr))`; `T::valid(snap(x))` under `T: Model`; and in a quantifier over `Seq<Snap<T>>`. A `match` on a runtime enum's snapshot types too.

What a member in the trait breaks:
- **The conditional form:** a deferred field's `valid` cannot be called through `Model`, which that form does not assume. `Deferred` has to carry it too (it does, as `__locus_valid`).
- **Hand-written implementations** must write both members, under their mangled names (`two-plain`).
- **Object safety:** `Model` is not usable as `dyn Model`, and was not before (its associated types). Nothing uses it so.
- A primitive type's is written `<u8>::valid(..)`: the toy mangles `X::f` only after a capital.

### Closures, function items, trait objects

- **A closure and a function item** (`e1`, `e1e`): given to a generic that does not ask for `Model`, nothing is asked of them, and that generic may read its other parameters. Given to one that asks, each is E0277: ``` `{closure@..}` cannot be read in logical code ``` and ``` `fn(u32) -> u32 {twice}` cannot be read in logical code ```. rustc adds "use parentheses to call this closure", which is not what is wanted.
- **Trait objects** (`e2`, `e2e`, `e2f`): a `Box<dyn Shape>` for a trait declared in `locus!`, and a `&dyn Debug`, compile as parameters that are not read, and as fields marked `#[opaque]` (the snapshot is `Opaque<Vec<Box<dyn Shape>>>`). Read, each is E0277 (``` `dyn Shape` cannot be read in logical code ```). As an unmarked field it is an error at the declaration. Lowering writes no implementation for a trait: a `dyn Trait` has no model, whoever declares the trait.

### F. Round 2's suite under the new library and lowering

47 of 252 files change (`tools/changed.py`; every case is listed with its first error in `run.log`).

| Cause | Files | Which |
|---|---|---|
| A logic function declares a parameter with a runtime type: now an error from lowering | 25 | 9 compiled in round 2: `spec/s1`, `s2`, `s4`; `policy/d2`, `d5`, `p4a`, `p5`; `skel/x11`; `more/r09`. The other 16 were rejected in round 2 for another reason and are now rejected with this error too (six are `spec-omit`'s variants of the three specifications) |
| The source names round 2's trait `Snapshot` | 2 | `skel/c02`, `c06` |
| A type parameter, or an associated type of one, read with no bound: none is inserted | 3 | `ghost-lets/gl-snapshot-with-associated-type-field`, `policy/n1`, `n2` |
| A type that nothing declares a model for: no opaque fallback | 2 | `skel/c05`, `policy/n6` (`BTreeSet<u32>`). `n6` is the one that mixed two foreign snapshots, which Locus's rules reject and round 2 accepted |
| Compiles in both rounds, prints other type names | 3 | `skel/a02`, `c01` (`Seq<Nat>` for `Seq<BoundedU32>`, `Link_Snap<Nat, Nat>` for `__locus_snap_Link`), `two-app` (rewritten) |
| **Now compiles** | 4 | `skel/x05`, `policy/p4b`: a place given to a type parameter is its capture, where round 2 asked for `model(x)`. `skel/x06`: a `Vec<u32>`'s snapshot and model are one type. `skel/x12`: `Token` had no model, and now has the default |
| Rejected in both rounds, with other errors | 8 | `policy/e04`, `p2a`, `v8`: a mismatch at an argument is now a plain E0308 with "function defined here", where round 2 had E0271 on `<bool as Model>::Logic`. `cases/t13b`, `t13d`, `t14`, `more/s16`: one more error, a ghost value of a type parameter bounded only by a trait that does not say `Logical` cannot be captured. `more/r06`: one error fewer |

```text
corpus: 26 files; compile in both expansions: round 2 18, round 3 18; outcome or output differs in 0
ghost-lets: 17 files; compile in both expansions: round 2 12, round 3 11; outcome or output differs in 1
cases: 58 files; compile in both expansions: round 2 18, round 3 18; outcome or output differs in 9
skel: 38 files; compile in both expansions: round 2 14, round 3 13; outcome or output differs in 9
more: 39 files; compile in both expansions: round 2 15, round 3 14; outcome or output differs in 6
spec: 4 files; compile in both expansions: round 2 4, round 3 1; outcome or output differs in 3
policy: 35 files; compile in both expansions: round 2 21, round 3 15; outcome or output differs in 12
spec-omit: 8 files; compile in both expansions: round 2 0, round 3 0; outcome or output differs in 6
two-app: 1 files; compile in both expansions: round 2 1, round 3 1; outcome or output differs in 1
corpus-explicit: 26 files; compile in both expansions: round 2 23, round 3 23; outcome or output differs in 0
```

The sources of those sets were not changed. With the standard `Vec` given no model as well, 58 files of the same suite change against the default, almost all of them a `vec![..]` read in logical code.

### Both programs agree

```text
all sets: binaries that both expansions accept: 151; lines in each listing: 718; binaries with a difference: 0
```

Round 2 had 0 of 577 over 126 binaries. Over round 2's own sets the count is now 114 binaries and 437 lines (12 fewer binaries compile, above); this round's sets add 37 binaries and 281 lines: `family` 20 and 176, `spec3` 4 and 82, `ghost-lets3` 12 and 16, `two-plain` 1 and 7. Every binary that compiled both ways printed the same lines from both. The stack's listing shows where the moved calls go: `Stack::new::{closure#0}:036:071 => the definition at 25:15 | Stack::__logic_room with []`.

## What it means

1. **One implementation for each type works in a macro,** from the tokens of the declaration alone, for 132 declarations, with rustc doing all of the normalization. What made it possible is a snapshot type that is generic over its fields' snapshots: its declaration needs no bound and names no lifetime. The price is recursion, which needs the projection written in the declaration, and is not done for a generic type.
2. **The written bound gives what the fallback could not, and rustc says what to write.** Generic and concrete code agree on every type; a missing bound is one message with the fix in its help. The limit is where the bound cannot be written: a foreign trait's generic method.
3. **Capture, always, removes both of round 2's choices and costs one spelling.** Arguments need nothing written, and `model(x)` of a place is gone (5 places fewer in the specifications). A logic function over a snapshot is `Stack::room(snap(s))`, which is the longest thing the new rules make a specification write: every `snap(..)` in the four specifications is one of these.
4. **The sharp edge of the identity default is real and can be silent.** Capture-then-project and select-then-capture differ in type only while a declared model's type differs from the snapshot's. `a14` is a program where they are the same type and mean different values, and nothing is reported.
5. **In real specifications the family itself costs little; the collection costs more.** 11 of 34 places are bounds, parameters and moved calls (16 with their `snap`); 18 are Locus's `Vec`. The litmus corpus of this spike changes by one line, but it does not hold the 19 examples that declare runtime-typed parameters.
6. **A bound on a type appears as soon as an invariant reads a parameter.** `Shelf<T: Model>` is the first, and Rust then wants it on every `impl`. That is the same cost that the ghost field of `Vec` avoids with a nominal type.
7. **`unsafe trait` plus a mark on generated tokens is enough to list forgeries, and nothing more.** rustc refuses the careless one; the driver names the deliberate one, across crates. It is a listing, by crate name, made only when the driver runs.
8. **The plain form costs nothing in what exists.** No declaration of the earlier sets needs a mark. The conditional form is worse than experimental: it refuses every declared model that reads a field, because the clause that defers the check also hides the field's snapshot type inside the implementation.
9. **A ghost field without a bound has to be nominal,** and is then not a logical value: the library converts at the field. And `IndexMut` cannot carry a contract; `get_mut` can, and its contract types.
10. **Validity as a trait member types for every shape tried,** and costs each hand-written implementation one more function.

## Where the translation guesses

Round 1's and round 2's guesses stand, less the three this round removed (which type parameters need `Snapshot`; whether a method call's receiver is a place of a type that declares the method; which logic functions take `self`). Added in this round, each from the tokens alone:

| What it decides | How | What it would need |
|---|---|---|
| Which `impl` declares a model, and of which type | the trait's name is `Model`; the type by the last name of its path, among the structs and enums of this invocation | name resolution |
| That a field is opaque | an attribute spelled `#[opaque]` | nothing more |
| Which lifetimes an opaque field's snapshot must not keep | every lifetime the tokens show is written `'static`; a type parameter's own cannot be seen, and rustc then asks for `'static` (`g2`) | the types |
| Which type parameters the generated implementation bounds by `Model` | a parameter that a field's type names, where the field is not opaque and not a proof | nothing more, if every such field is read |
| That a field's type is an associated type of a parameter | the parameter followed by `::Name`, or inside `<T as Trait>::Name` | the types |
| That a type contains itself | a field's type holds the type's own name, or `Self`; through another type it is not seen | the declarations |
| How a declared model's arguments stand for the declaration's parameters | by position, replacing each parameter's name in the field types | the types |
| **That a logic function's parameter has a runtime type** | it starts with `&`, `*` or `[`; or is a machine integer, `bool`, `char`, a float, `Vec`, `Box`, `Option`, `String` or `str` by name; or a struct or enum of this invocation that is not logical; `self` in an `impl` of such a type. **A type parameter, and a type of another invocation or crate, are taken to be logical** | Locus's own typing |
| That a name inside a logic function is a logical value | every name there; a name in capitals is a static and is captured | name resolution |
| That `X::f(..)` is a type's logic function | a capital on the segment before the last (round 1's), so `<u8>::valid(..)` is written with brackets | name resolution |
| That a pattern's path names a runtime enum's variant | the owner is an enum of this invocation; its snapshot's number of parameters is its number of non-recursive fields | name resolution; another invocation's enum is not matched |
| That an enum is declared at module level | `self::Name` inside `const _`, for a payload that names the enum | the item's scope |
| `snap`, `model`, `final!` | by name | name resolution |
| A proof field's snapshot | `Proof`, and it is left out of `valid` | nothing more |
| (the driver) which implementations lowering wrote | the span's expansion is a macro named `locus` of a crate named `locus_macros` | the crate's identity |

## What failed, and what was not done

- **`snap(s).room()` is not built;** only the path form works (`a13e`).
- **A generic type that contains itself gets no implementation,** and a union none. No set has either.
- **A marker for a model declared outside its type's invocation is not built** (described under A).
- **The conditional form fails as described,** and its failure for a field with a declaration's lifetime was not looked into (`E0308` at the struct).
- **`*final!(self)` on Locus's `Vec` silently read the standard vector** in the first draft of its contracts, through `Deref`. Nothing warns of it.
- **An opaque field cannot be read by selecting it on the place** (`a06f`): only through `snap(s)`. The message is about the foreign type and does not say so.
- **The message at a declaration names the field's type, not the field,** and carries a note about type parameters that does not apply.
- **The standard `Vec` still has a model by default,** against the design, so that 252 files could be compared. The count without it is one line per set, above.
- **Round 2's seven-policy matrix was not rerun** for the new specifications, and `spec-omit/` was not regenerated from them: both are still round 2's text, which now fails on `logic fn model(x: &X)`.
- **`tools/spec3-table.py`'s causes are assigned by hand.** The tool checks only that the listed lines are exactly the changed code lines.
- The reviewer's adversarial cases about typing that have a file: a wrapper with no model whose field has one (`a02`, `a03`), an inner model that loses information (`a14`), representation the same in generic and concrete use (`d1`, `d6`, `d7`), two foreign snapshots (`a06e`), a value behind a trait object (`e2`). The rest are about proofs, memory or permissions and have none.
- Round 2's `probes2/`, its `expansions` section, and its diagnostics comparison were not run again.

## Not tested

- **Locus's own typing and checking.** Nothing here types logical code but rustc, and nothing is proved. In particular nothing checks that a logic function's parameter is a logical type beyond the syntactic test, that `valid` is used, or that a declared model is a function of the snapshot.
- **Equal transparent snapshots of two references or boxes say nothing about addresses,** and nothing here tests an address-sensitive operation: pointer equality, `ptr::eq`, an address used as a key, or a function that raw-pointer code makes sensitive to where a value is.
- A type that contains itself through another type; a generic recursive type; a union; three or more crates; a re-export of a type with a generated implementation; a model declared for a type of another crate (lowering refuses; rustc's orphan rule was not reached).
- `Result`, maps, sets, `Rc`, `Arc`, locks, atomics: the library declares nothing for them. Tuples of more than three.
- A `match` on a runtime enum of another invocation or crate; a generic runtime enum in a pattern beyond `v1`'s; a runtime struct pattern in logical code.
- `#[opaque]` on a field of a tuple struct and of an enum variant (written in lowering; no case).
- Whether rustc means to keep not checking a higher-ranked `where` clause at a declaration. The default no longer rests on it.
- A trait object given a model; `dyn Any`; a proof hidden behind a callback.
- The listing of implementations under a build that does not use the driver, and a forger's crate named as Locus's.
- Build time and the size of generated code (one snapshot type and five implementations for each declared type); rust-analyzer; rustfmt; release builds; lints.
