# Rust features and Locus

This file is the explicit record of how the Locus core relates to Rust: what is supported, what is supported differently, what is not supported, what Locus adds, and where the two currently conflict. It tracks the target fragment in [the core language specification](core-language-spec.md), not the implemented frontend; [grammar.md](grammar.md) describes what the parser accepts today.

Keep this file current. Any change to the specification that adds, removes, or alters a Rust-visible construct must update the matching row here.

Status values:

| Status | Meaning |
|---|---|
| Supported | Same surface form and the same meaning as Rust, within the fragment. |
| Differs | Present, but with a different form or a restricted meaning. The note says how. |
| Deferred | Not in the core. Intended later; the note gives the README milestone when one exists. |
| Unplanned | Not in the core and with no current plan. |

## 1. Supported, or supported with differences

| Rust feature | Status | Notes |
|---|---|---|
| `bool`, `true`, `false` | Supported | |
| `u8`, decimal literals | Differs | No literal suffixes, hex, octal, or binary forms. Underscore separators are accepted. |
| Unit `()` and tuples | Differs | A tuple type may name its fields, `(value: u8, @[value != 0])`. The names are binders for later field types only; access, construction, and patterns stay positional. |
| `.0`, `.1` tuple projection | Supported | Erased fields still occupy a source position. |
| Named-field `struct` | Differs | Fields are ordered, may be unnamed, and may be accessed positionally as well as by name. A later field's type may mention an earlier field inside a proposition. No field-init shorthand, no reordering, no `..base` update. |
| `enum` | Differs | Unit and tuple-style variants only. Non-recursive. Variants are always written `Enum::Variant`. No struct-style variants, explicit discriminants, or `as` casts. |
| `match` | Differs | Exhaustive, ordered arms; name, wildcard, literal, tuple, struct, and variant patterns, nested. No guards, `\|` alternatives, ranges, `@` bindings, or `ref`. Each arm also receives checked evidence about the scrutinee. |
| `if` / `else` | Differs | `else` is required. Defined as `match` on `bool`; each branch receives evidence of the condition's value. |
| `let` and shadowing | Supported | Bindings are immutable. Irrefutable patterns only. |
| Block expressions, trailing expression | Supported | |
| `fn` items | Differs | Parameter and result types are always explicit. Result types may name their fields and depend on parameters. Top level only. No recursion. |
| Function pointer types `fn(A) -> B` | Differs | Inhabited by the names of declared functions. A `math fn(A) -> B` type also exists. |
| `const` items | Differs | The initializer is a total logical expression. |
| Method-call syntax `x.wrapping_add(1)` | Differs | Only for primitive operations: `wrapping_add`, `wrapping_sub`. No user methods. |
| Comparison operators `== != < <= > >=` | Differs | Defined on `u8` and `bool` only. No chaining. No equality on products, enums, or functions at runtime. |
| `!`, `&&`, `\|\|` | Supported | Short-circuit on `bool`. The same tokens build propositions when the operands are `Prop`. |
| `loop` | Differs | Carries explicit state: `loop (s: T = init) -> R { ... }`. There is no bare `loop { }`. |
| `break value` | Supported | Targets the nearest `loop`. No labels. |
| `continue` | Differs | Takes the next state: `continue(next_1, next_2)`. No labels. |
| `for` | Differs | Only `for i in lo..hi (state) { ... }` over a `u8` range, with explicit state and no `break` (proposed form). Requires evidence that `lo <= hi`; a reversed range is rejected, where Rust treats it as empty. No iterators. |
| Line and nested block comments | Supported | Doc comments have no special meaning. |
| Attributes `#[...]`, `#![...]` | Deferred | Reserved; currently rejected with a diagnostic. |

## 2. Not supported

### 2.1 Types

| Rust feature | Status | Notes |
|---|---|---|
| Other integer types: `u16` to `u128`, `i8` to `i128`, `usize`, `isize` | Deferred | Milestone 2. Added by the same rules as `u8`. |
| Arithmetic and bit operators: `+ - * / %`, `& \| ^ << >>`, unary `-`, compound assignment | Deferred | Milestone 2. Needs the overflow policy: an obligation over the mathematical value. |
| `as` casts and numeric conversions | Deferred | Milestone 2. |
| `f32`, `f64` | Unplanned | |
| `char`, `str`, `String`, string, char, and byte literals | Deferred | With collections, milestone 10. |
| Arrays `[T; N]`, slices `[T]`, indexing `a[i]`, array literals | Deferred | Milestones 9 and 10. The bracket syntax conflicts with proposition literals; see section 4. |
| References `&T`, `&mut T`, lifetimes | Deferred | Milestone 9. |
| Raw pointers | Unplanned | Listed in the README as following the foundation. |
| `Box`, `Rc`, `Arc` | Deferred | With recursion and ownership. In the logic they are transparent wrappers around `T`. |
| `Vec`, maps, sets, the standard library | Deferred | Milestone 10. |
| Recursive types | Deferred | Milestone 4, with structural recursion. |
| Tuple structs `struct P(u8, u8);` and unit structs | Deferred | Named-field struct syntax covers unnamed fields meanwhile. |
| Struct-style enum variants, explicit discriminants | Deferred | |
| `union` | Unplanned | |
| Never type `!` | Deferred | A match with no arms on a proof of `false` plays this role inside verified code. |
| Type aliases `type` | Deferred | |
| Generics and const generics | Deferred | Milestone 3. |
| Traits, `impl` blocks, user methods, associated items, trait objects `dyn`, `impl Trait` | Deferred | Milestone 12. |
| Closures and the `Fn`, `FnMut`, `FnOnce` traits | Deferred | Logic-only lambdas come first; runtime closures follow the foundation. |

### 2.2 Bindings, ownership, and state

| Rust feature | Status | Notes |
|---|---|---|
| `let mut`, assignment | Deferred | Milestone 7. |
| Moves, borrows, the borrow checker, `Copy` and `Clone` | Deferred | Milestone 9. Every core value is immutable and freely reusable. |
| `Drop` and destructors | Deferred | After the foundation. |
| Interior mutability: `Cell`, `RefCell`, `Mutex` | Deferred | After the foundation. Types that allow cycles are not inductive in the logic. |
| `static` items | Unplanned | |
| Threads, `Send`, `Sync`, atomics | Deferred | After the foundation. |
| `async` / `await` | Deferred | After the foundation. |
| `unsafe` blocks and functions | Unplanned | |

### 2.3 Control flow and expressions

| Rust feature | Status | Notes |
|---|---|---|
| Recursive and mutually recursive functions | Deferred | Milestone 4. Declarations are acyclic in the core. |
| `if` without `else` | Deferred | |
| `if let`, `let ... else`, `while let` | Deferred | |
| `while` | Deferred | Milestone 8, elaborating to state-passing loops. |
| `for` over iterators, ranges as values | Deferred | With traits. |
| Bare `loop { }`, loop labels, labelled `break` and `continue` | Deferred | Milestone 8. |
| `return` | Deferred | |
| `?` operator | Deferred | With generics. |
| Match guards, `\|` patterns, range patterns, `@` bindings, `ref` patterns, slice patterns | Deferred | Guards interact with the `=>` conflict in section 4. |
| Refutable patterns in `let` | Deferred | |
| Struct update `..base`, field-init shorthand | Deferred | |
| Panics, `panic!`, unwinding, `catch_unwind` | Deferred | Milestone 11. The core's only non-returning behavior is divergence. |
| Macros: `macro_rules!`, procedural macros, `println!` and other std macros | Unplanned | |
| Input and output, `fn main`, process entry | Deferred | Milestone 1 defines how generated Rust is invoked. |

### 2.4 Items and program structure

| Rust feature | Status | Notes |
|---|---|---|
| Modules `mod`, `use`, paths other than `Enum::Variant`, visibility `pub` | Deferred | Milestone 5. |
| Crates, Cargo dependencies, `extern crate` | Deferred | Milestone 11. |
| `extern` blocks, FFI, calling Rust from Locus | Deferred | Milestone 11. Requires marked trusted declarations. |
| Exporting Locus functions to Rust callers | Deferred | Milestone 11. Proof parameters need checked wrappers or validated types. |
| Items nested inside function bodies | Deferred | |
| `const fn`, const evaluation | Unplanned | `math fn` is the closest analogue: a restricted subset usable in a second context. |
| Type inference for function signatures | Unplanned | Signatures stay explicit. |

## 3. Locus constructs that are not Rust

| Construct | Purpose |
|---|---|
| `math fn` | A pure, total function, callable from both code and logic. Lemmas and predicates are math functions. |
| `Prop` | The type of logical claims. |
| `@P`, `@[formula]` | The type of proofs of a proposition. |
| `[formula]` | A proposition literal, with logical `==`, comparisons, connectives, `forall`, and `exists`. |
| `=>` between propositions | Implication. |
| `forall (x: A) { ... }`, `exists (x: A) { ... }` | Quantifiers inside a formula. |
| `prop Name(params) { variants }` | A user-declared proposition, given by its proof constructors. |
| `_` in expression position | A request for a checked proof of the expected proposition. |
| `rewrite(eq, h)`, `unfold(f, h)`, `fold(f, h)` | Built-in proof forms: transport along an equality or a function's defining equation. |
| Applying a proof, `all(n)`, `imp(hp)` | Instantiating a quantified or implicational proof. |
| Named tuple fields and dependent field types | Letting a proof field describe a data field. |
| `loop (state) -> R`, `continue(next)`, `for i in lo..hi (state)` | State-passing iteration for an immutable language. |
| Ghost bindings | Bindings that exist only for the checker. Implicit in the core; a `ghost` keyword is deferred. |

## 4. Known conflicts with Rust syntax

A Rust superset stays reachable only if Locus never gives valid Rust syntax a different meaning. Whether to adopt that as a rule is an open question in the specification (section 15.2). These are the current violations and near-misses.

| Locus form | Conflict | Proposed resolution |
|---|---|---|
| `[n > 0]` as a proposition literal | Valid Rust: a one-element `[bool; 1]` array. In the core a bracketed expression is always a proposition literal. Also collides with array types, slices, and indexing once those exist. | Drop the literal form. Elaborate an expression as a formula wherever a `Prop` is expected, and write the proof type as `@(n > 0)`. |
| `=>` as implication | Rust uses `=>` for match arms. Unambiguous only while guards are absent. | Spell implication `==>`. |
| `prop`, `math`, `in` as words | Valid Rust identifiers, except `in`. | Treat as contextual keywords, reserved only where the grammar expects them. |
| Erased fields in tuples and structs | Not a syntax conflict, but generated Rust has different field positions from the source. | Decide between renumbering and zero-sized placeholders (specification section 15.2). |

Forms that occupy positions where Rust has no valid syntax, and are therefore safe extensions: `math fn`, `@` in type position, `loop (...)`, `continue(...)`, `for ... (state) { }`, named tuple fields, and an enum or struct that refers to itself without `Box` (rejected by Rust as infinitely sized; not yet accepted by Locus either).
