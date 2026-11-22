# Spike: a Rust type of its own for each logical type

A throwaway experiment for the draft of RFC 0006 on traits, and for [traits-on-logical-types](../../issues/traits-on-logical-types.md). Nothing outside `spikes/` may depend on it.

## Question

[RFC 0001](../../design/rfcs/0001-compiler-pipeline.md) lowers every logical value that is not a proof to one marker, `LogicalData`. rustc then cannot tell `Int` from `Nat`, so it cannot resolve a trait implemented for a logical type.

**If each logical type were a zero-sized Rust type of its own, could rustc do all of the trait resolution for logical types, so that Locus does none?**

1. Does rustc resolve implementations, bounds, and associated types on such types, for the program's traits, the library's, and the standard library's?
2. What breaks, given that lowering is syntactic and that a macro cannot name the type of the logical code it carries?
3. Logical code is never given to rustc as code. Can Locus still get rustc's answer for a trait used there?
4. With one marker, as today, where does a single set of traits for both kinds of type get stuck?

## Setup

Plain Rust, edition 2021, rustc 1.97.0. No lowering and no Locus macro is involved: every file is the Rust that a lowering would have to produce, written by hand.

- `lib/lg.rs` stands in for Locus's library with one type for each logical type: `Int`, `Nat`, `Bool`, `Prop`, a generic `Seq<T>`, `Snap<T>` in two encodings, and `Erased` for every proof, as today.
- `lib/lu.rs` stands in for the library as it is today: `Int` and `Nat` are aliases of `LogicalData`.
- `cases/` holds twenty programs. The first line of each says whether it must compile, or which error it must give. `c` cases use `lg`, and `r` cases use one marker.
- `oracle/` is a rustc driver of about 120 lines, built against the compiler's own crates as [rustc-facts](../rustc-facts/README.md) is. After rustc has checked `oracle/sample.rs`, it builds types that appear nowhere in that file and asks rustc's trait solver about them.

`./run.sh` runs all of it. The oracle needs `rustup component add rustc-dev llvm-tools --toolchain stable`, and is skipped without it.

## Results

Run on 2026-11-22. All twenty cases gave the outcome their first line states.

**With a type for each logical type:**

| Case | Outcome | What it shows |
|---|---|---|
| `c01_family` | compiles | An associated type that is `Int` at `Int` and a proof at `Nat`, used in a signature and stored in a runtime struct. The struct is 4 bytes: the markers cost nothing |
| `c02_user_types` | compiles | The library's trait for a program's logical type; a program's trait for `Int` and `Seq<Nat>`; the standard `Add` and `From`; an implementation with a bound; a trait with a type parameter; logical types that contain themselves. A member name shared by two traits is not ambiguous on a type that implements one |
| `c03_mismatch` | E0308 | rustc rejects a `Vec<Nat>` used as a `Vec<Seq<Int>>`. Today it accepts, and only Locus's own typing rejects |
| `c04_orphan` | E0117 | The orphan rule applies as in Rust |
| `c10_both_kinds` | compiles | One trait implemented for `u32` and for `Nat`. The standard `From` used at runtime types and at logical types through one generic function. `Logical` as an ordinary bound |
| `c11_only_ghost_runtime` | E0277 | That bound keeps runtime types out |
| `c12_arithmetic` | compiles | `a + b` on two ghost values type-checks as Rust code, through `Add` on the markers |
| `c13_comparison` | E0308 | A comparison does not: Rust fixes the result of `<` as the runtime `bool` |

**What breaks:**

| Case | Outcome | What it shows |
|---|---|---|
| `c05_infer_backward` | compiles | `let g = ghost();` followed by a call that takes a `Nat`: rustc infers `Nat`, backward from the later use |
| `c06_infer_unconstrained` | E0283 | The same `let` when only later logical code mentions `g`: "type annotations needed" |
| `c07_infer_annotated` | compiles | With `let before: Nat = ..` |
| `c14_cast` | compiles | Ghost locals with no logical type for rustc, and a conversion at each use in runtime code: a declared parameter type, or a known type argument, decides |
| `c15_cast_generic` | E0283 | That conversion has nothing to go on where the use is generic |
| `c08_snap_typed_borrow` | E0499 | A `Snap<T>` that keeps `T` keeps `T`'s lifetimes. A ghost snapshot of a `&mut` then keeps the borrow alive, and Rust code that is fine today is rejected |
| `c09_snap_uniform_borrow` | compiles | The same program with one marker for every snapshot, through a projection that forgets `T` |

**The oracle,** on types the sample never mentions:

```text
which implementation a member comes from:
  <Int as OrdLogic>::le  is the `le` on line 24 of the sample, at []
  <Version as OrdLogic>::le  is the `le` on line 26 of the sample, at []
  <Seq<Version> as OrdLogic>::le  is the `le` on line 27 of the sample, at [Version]
  <Seq<Seq<Nat>> as OrdLogic>::le  is the `le` on line 27 of the sample, at [Seq<Nat>]
whether a bound holds:
  Version: OrdLogic  is  true
  Seq<Version>: OrdLogic  is  true
  Seq<Seq<Nat>>: OrdLogic  is  true
  Bool: OrdLogic  is  false
  Seq<Bool>: OrdLogic  is  false
what an associated type is:
  <Int as Family>::Out  is  Int
  <Nat as Family>::Out  is  Erased
  <Seq<Seq<Nat>> as Family>::Out  is  Seq<Seq<Erased>>
under a bound, in `fn generic<T: OrdLogic>()`:
  Seq<T>: OrdLogic  is  true
  T: Family  is  false
```

**With one marker, as today:**

| Case | Outcome | What it shows |
|---|---|---|
| `r01_two_impls` | E0119 | Two implementations of a trait, for `Int` and for `Nat`, conflict: both are for `LogicalData` |
| `r05_assoc_types` | E0119 | So an associated type cannot differ between two logical types |
| `r02_from_reflexive` | compiles | `Int::from(nat)` already resolves, to the standard library's `impl<T> From<T> for T`. rustc takes the conversion to be the identity |
| `r03_from_own` | E0119 | And the library cannot write its own conversion, which overlaps that one |
| `r04_foreign_trait` | E0117 | A program cannot give a standard trait to its logical type, because that type is the library's marker |

## What it means

1. **rustc can do all of it for anything whose type is written.** Implementations, bounds, associated types, type parameters on traits, implementations with bounds, and the orphan rule all behave as they do for any Rust type. One set of traits serves both kinds of type, and `Logical` is one bound among others. Locus resolves nothing.
2. **Logical code needs no typing by rustc.** Locus can type it as it does today and ask rustc afterwards, through the driver, which implementation a member comes from, whether a bound holds, and what an associated type is. The types need not appear in the program.
3. **The obstacle is a ghost value whose type nobody wrote.** The expansion of `logic! { .. }` cannot name the logical type of its contents, and the runtime program is built by plain rustc with no driver, so the type must come from rustc's inference in both programs. Where a ghost local is mentioned only by later logical code, which is the common case, there is nothing to infer from. Three ways through, none tried beyond what the cases show:
   - **Annotate.** `let before: Nat = logic! { .. };`. RFC 0002 does not ask for this today.
   - **Untyped ghost locals, and a conversion at each use in runtime code.** Lowering already knows which bindings are ghost. It fails only where a ghost local is passed to something generic, which then needs an annotation.
   - **A typing skeleton,** in which lowering mirrors simple logical code as calls of stubs. Not tried.
4. **`Snap<T>` must stay one marker.** Otherwise ghost snapshots carry lifetimes and change what the borrow checker accepts. The cost is that no implementation can tell `Snap<A>` from `Snap<B>`.
5. **Comparisons and `&&` stay inside the library's macro,** and arithmetic need not. Their result is always `Bool`, so nothing has to be inferred for them.
6. **rustc would infer logical types backward** (`c05`), where RFC 0002 says Locus never does. Locus's own typing would have to agree with what rustc inferred, or be checked against it.
7. **With one marker, a single set of traits gets stuck at once.** Every implementation for a logical type is an implementation for the same Rust type. Lowering would have to erase them all and stand a stub in their place, which it cannot do for a trait of the standard library or of another crate. Associated types cannot differ. And the standard library's reflexive `From` already answers, wrongly.

## Not tested

- No lowering was written. Whether the two programs RFC 0001 builds from one source stay in step under this change is not shown.
- The oracle asked about one crate's own types. Types and implementations from other crates, and the cost of asking, were not tried.
- Export, error messages, dependent tuples, and what replaces the name `LogicalData` in rustc's messages.
- What a trait's method means at a logical type. It is a runtime method to rustc, and logical code may not call runtime functions, so it would have to be a logic function there. That is a design question, not one for rustc.
