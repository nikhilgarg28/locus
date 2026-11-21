# RFC 0004: What runtime code tells Locus

- **Status:** draft
- **Issue:** part of [runtime-semantics](../../issues/runtime-semantics.md)
- **Supersedes:** none

## Summary

Locus understands a fixed list of runtime operations: those that the language itself defines, on the types its table names, and a short list of library items beside them. Each has a value and a condition under which it returns, and once it has returned Locus knows both. Arithmetic means the wrapped result whichever way the crate is built, so nothing is owed where it is written. A fact that a loop keeps is a `let mut` proof variable that the program assigns again, and one rule, about where paths join, makes it an invariant. There is no invariant construct, and no way yet for a function of the program's own to join the list.

**[RFC 0005](0005-core-terms.md) restates part of this RFC,** after two reviews found gaps in it. Where the two differ, RFC 0005 is the later text. It defines wrapping exactly; says what a condition, a write, a join, a loop, a `for` range, and the end of a borrow each put in the context; and gives a kept proof variable its type. The table of understood operations below was revised after the same reviews, which found that a comparison on any type that stays Rust's would call user code.

## Motivation

A proof is about values, and runtime code is what makes the values. [RFC 0003](0003-proof-construction.md) says that a hole uses "what is known at that point", and nothing yet says what that is for real code. Every accepted example so far is a straight line of comparisons and small arithmetic.

```rust
fn binary_search(xs: &[u32], target: u32) -> Option<(i: usize, @(i < xs.len() && xs[i] == target))> {
    let mut lo = 0;
    let mut hi = xs.len();              // (1) what is known of `hi`?
    while lo < hi {                     // (2) what is known at the top, every time round?
        let mid = lo + (hi - lo) / 2;   // (3) is this the true midpoint, in a release build too?
        if xs[mid] < target {           // (4) what does it mean that the index returned?
            lo = mid + 1;
        } else if xs[mid] > target {
            hi = mid;
        } else {
            return Some((mid, _));      // (5) can the hole be filled?
        }
    }
    None
}
```

The numbered lines are the three questions of this RFC: which operations Locus understands, how a loop keeps a fact, and what arithmetic means when it can overflow.

## Design

### Understood operations

**An operation is understood when it is in one of two closed lists.** The first is what the language itself defines, on the types it names. The second is a short list of library items whose meaning Locus fixes. Everything else is a call, known through its signature.

An earlier revision said "when Rust gives it syntax and it is applied to a type that stays Rust's". That was too wide: `==` on an `Option`, a tuple, a slice, or a reference is syntax on a type that stays Rust's, and it calls the element's `PartialEq`, which may return anything. An operation is matched to a row by what rustc resolves it to, and by its operands' full types.

**What the language defines:**

| Operation | On | Its value | It returns only if |
|---|---|---|---|
| `a + b`, `a - b`, `a * b`, `-a` | Machine integers | The true result, wrapped to the type | Always, as far as a proof may assume ([below](#arithmetic)) |
| `a / b`, `a % b` | Machine integers | Rust's division and remainder | `b != 0`, and for a signed type the quotient is in range |
| `==`, `!=`, `<`, `<=`, `>`, `>=` | Two integers of one type, or two `bool`s | The comparison of their values | Always |
| `&&`, `\|\|`, `!`, and `&`, `\|`, `^` | `bool` | The connective. `&&` and `\|\|` evaluate their second operand only when it is needed | Always |
| `x as T` | An integer or a `bool`, to an integer type | The value wrapped to `T`; for a `bool`, 0 or 1 | Always |
| A literal, a constant | | Its value | Always |
| `s.f`, `t.0` | A struct, a tuple | That part of the value | Always |
| Building a tuple, a struct, or an enum | Any, the program's own included | By structure | Always |
| `&x`, `&mut x`, `*r`, and `*b` for a `Box` | | The value itself. A mutable borrow is [RFC 0005](0005-core-terms.md)'s | Always |
| `xs[i]` | An array or a slice, with a `usize` | The element at `i` | `i < xs.len()` |
| `=`, and `+=` and the like | A place | A write. A compound form is the operation and then the write | As the operation |
| `if`, `match`, `if let`, `let` with a pattern | | [Below](#patterns) | |

**Library items whose meaning Locus fixes:**

| Item | On | Its value | It returns only if |
|---|---|---|---|
| `xs.len()` | An array or a slice | The length | Always |
| `Box::new(x)` | | A box that holds `x` | Always |
| `a.checked_add(b)`, `checked_sub`, `checked_mul` | Machine integers | `Some` of the true result when it is in range, and `None` otherwise | Always |
| `xs[a..b]` and the other range forms | An array or a slice | That part of the sequence | A condition for each form, below |
| `for i in a..b`, `for i in a..=b` | Machine integers | The range fact of [RFC 0005](0005-core-terms.md) | |
| `e?` | An `Option`, a `Result` | The payload, where it continues | Always |

The split is real in Rust. The first list is built into the compiler for those types. The second is trait implementations and functions of `core`, which Locus trusts to mean what their rows say (`trusted.obligations`).

- **One rule for all of them.** Once an understood operation has returned, Locus knows that its condition held and that its result is its value. After `let x = xs[i];` it knows `i < xs.len()`, and that `x` is the element at `i`. A panic proves nothing, and a return proves that the check passed.
- **Each form of range indexing has its own condition.** `a..b` needs `a <= b && b <= xs.len()`. `a..=b` needs `a <= b + 1 && b < xs.len()`. `a..` needs `a <= xs.len()`, `..b` needs `b <= xs.len()`, `..=b` needs `b < xs.len()`, and `..` needs nothing.
- **`e?` continues with the payload** of a `Some` or an `Ok`, and that `e` was one is known afterwards. Otherwise the function returns: a `None` as it is, and an `Err` with the same error when the two error types are the same. When they differ, the error passes through a conversion, which is a call, so the error returned is known by its type. `?` on a value that holds a proof is still deferred (RFC 0002).
- **`assert!` needs no rule.** It is an `if` whose other branch does not return, so its condition holds afterwards.
- **Operands are read in Rust's order of evaluation,** and an operation with no row gives no fact.

#### Patterns

In an arm of a `match`, in the body of an `if let`, and after a `let` with a pattern, the scrutinee is known to have matched the pattern, for a pattern built from bindings, wildcards, literals, ranges of integers, references, and the constructors of tuples, structs, and enums. In an arm of a `match`, each earlier arm is also known to have failed: its pattern did not match, or its guard did not hold. So in `match n { 0..=9 => A, _ => B }`, `n > 9` is known in `B`.

A slice pattern gives no fact about the sequence. The loans it makes are [RFC 0005](0005-core-terms.md)'s.

#### What is not understood

Each of these is a call, or gives a result that is known by its type:

- **A comparison on any other type:** an `Option`, a tuple, an array, a slice, a reference, a `Box`, a `char`, and every type of the program's own. `&Number(0) == &Number(1)` can be `true`. `char` can join when it has a model, and references, tuples, and arrays of integers can each be added as a row.
- **Shifts, and `&`, `|`, `^`, and `!` on integers.** Each needs an exact meaning first: a signed `>>` rounds down where `/` truncates, and `<<` drops high bits.
- **Floating-point numbers,** whose `==` is not equality, and **casts** from a `char` or an enum, and to or from a pointer.
- **`opt.unwrap()`, `xs.get(i)`, `xs.iter()`,** an operator on a type of the program's own, and every method of `Option` and `Result`. The owner on the methods: "it's always possible to add more later".

#### `debug_assert!` and build profiles

`cfg!(debug_assertions)` is a `bool` of which Locus knows nothing, whichever way the crate is built. `debug_assert!(c)` is `if cfg!(debug_assertions) { assert!(c); }`, so its condition is established on one branch only, and is not known afterwards. The same holds for a statement or a block under `#[cfg(debug_assertions)]` or its negation: it gives no fact to the code after it.

- **What a program proves does not depend on whether it is built for release.** Arithmetic is read the same way in both for this reason.
- **Every other `cfg` is as [RFC 0001](0001-compiler-pipeline.md#configurations) has it,** checked for the configuration being built. `debug_assertions` is still part of the configuration, and decides which code exists.
- **The other reading is not unsound.** With facts for each configuration, a program could pass every check in the build it is written in, and be rejected when built for release.

So in the motivating example, line 1 gives `hi === xs.len()`. Line 4 gives `mid < xs.len()`, the element, and, in each branch, how it compares with `target`. The hole on line 5 is filled from those, and needs nothing that the loop keeps.

### Arithmetic

Rust defines the overflow of `+`, `-`, and `*` in both kinds of build, and never as undefined behavior. With overflow checks, which a debug build has by default, it panics. Without them, the result wraps. One statement is true in both: **if the operation returned, its result is the wrapped value.**

- **`a + b` means the wrapped sum, whichever way the crate is built,** and nothing is owed where it is written. So a proof holds in a debug build and in a release build, as the [vision](../../VISION.md)'s first commitment asks, and plain Rust compiles unchanged.
- **The true result follows when it is in range.** Locus knows the range of every machine integer, and that wrapping does nothing to a value in range. `linear` knows these two facts about wrapping, whose exact definition is [RFC 0005](0005-core-terms.md#wrapping)'s. An earlier revision called wrapping "a remainder by a literal", which is wrong for Locus's `%`: that one truncates toward zero.
- **So overflow shows only where a proof needs the true value and the facts do not give the range:**

  ```rust
  fn next(i: u32) -> (j: u32, @(j > i)) {
      (i + 1, _) // error: cannot prove `j > i`: `i + 1` may overflow, so `j` is the wrapped sum
  }

  fn next_below(i: u32, room: @(i < 100)) -> (j: u32, @(j > i)) {
      (i + 1, _) // accepted: `i + 1` is in range
  }
  ```

- **In a proposition, arithmetic is on models and never overflows** (RFC 0002). `@(j == i + 1)` states the true sum.
- **Division is different:** it panics on a zero divisor in every build, so having returned is a fact.
- **`checked_add`, `checked_sub`, and `checked_mul`** are how a program learns at run time that a result is in range, beside comparing first.
- **The strict rule belongs to `#[no(panic)]`** ([no-panic-claims](../../issues/no-panic-claims.md)): there, each operation is proved to be in range, since a build with overflow checks could panic. It is asked for one function at a time, so it rejects no plain Rust.

In the motivating example, line 3 needs no proof. Inside the loop `lo < hi` is known, so `hi - lo` is in range, the sum is at most `hi`, and a hole proves `lo <= mid && mid < hi`.

### Loops, and proofs that are kept

A loop's body runs many times with different values. Anything known of a variable that the loop changes must hold every time control reaches the top.

**There is no invariant construct.** A fact that a loop keeps is a proof variable, declared with `let mut` before the loop and assigned again in the body:

```rust
fn count_below(xs: &[u32], limit: u32) -> (n: usize, @(n <= xs.len())) {
    let mut n = 0;
    let mut i = 0;
    let mut fits: @(n <= i && i <= xs.len()) = _;
    while i < xs.len() {
        if xs[i] < limit {
            n += 1;
            i += 1;
            fits = _;
        } else {
            i += 1;
            fits = _;
        }
    }
    (n, _)
}
```

- **`mut` on a proof variable means what it means in Rust:** the variable may be assigned. Its proposition is written at its declaration, and each assignment is checked against it at the values of that moment.
- **A proof is about the values it was made from, and that is all.** After `n += 1`, `fits` still holds a proof, about the earlier `n`. It does not fit where a fact about the current `n` is needed, by the rule that every proof already obeys ([`proofs.versions`](../../spec/proofs.md)). There is no state in which a proof cannot be read.
- **Assigning it again is always written.** Locus does not try a hole by itself.
- **Where paths join, Locus knows what it knows on every one of them.** So a proof variable proves its proposition for the current values after an `if` only if both branches leave it so.
- **The top of a loop is such a place.** Its paths are the way in, and the end of each way through the body. There, a variable that the loop writes is known by its type and by the proof variables that are current on every one of those paths. A variable that the loop does not write keeps what was known of it.
- **So nothing is special about `break`, `continue`, or a loop inside a loop.** Each is paths that join.
- **`for i in a..b` gives `a <= i && i < b` in its body.** A loop whose kept fact mentions the index is written as a `while` with a counter of the program's own, as above, since `i` is not in scope where the proof would be declared. In any other `for`, the loop variable is known by its type.
- **Termination is not asked for.** A loop that never ends proves nothing ([RFC 0002](0002-logical-layer.md)).

A loop means what the recursive function means that takes the variables it writes as parameters, and the kept proofs as proof parameters: the way in is the first call, the end of the body is the call of itself, and leaving is a return.

### What is not here

- **A way for a function of the program's own to mean something,** so that a call of it is read as these operations are. The owner looked at two forms and chose "nothing for now" ([pure-functions](../../issues/pure-functions.md)). The one case with no way round it is an operator or a trait method on a type of the program's own, which cannot return a proof.
- **Methods of `Option` and `Result`,** `get`, `first`, and `last` on slices, and the other methods of the integer types.
- **Iterators.** `for x in xs`, `enumerate`, and `step_by` are calls.

## Rules

The rules below are a new chapter of the specification, [Runtime code](../../spec/runtime.md).

| Rule | Examples |
|---|---|
| `runtime.operations` | `runtime/bool-cast-is-zero-or-one.accept.lc`, `runtime/bool-connectives.accept.lc`, `runtime/checked-sum-with-question-mark.accept.lc`, `runtime/checked-sum.accept.lc`, `runtime/comparison-of-options-with-no-claim.accept.lc`, `runtime/length-of-slice.accept.lc`, `runtime/comparison-of-options.reject.lc`, `runtime/comparison-through-a-reference.reject.lc`, `runtime/for-over-an-iterator.reject.lc`, `runtime/method-that-is-a-call.reject.lc`, `runtime/shift-is-not-understood.reject.lc` |
| `runtime.operations.returned` | `proofs/given-what-an-index-returned.accept.lc`, `runtime/assert-gives-its-condition.accept.lc`, `runtime/binary-search-found.accept.lc`, `runtime/division-returned.accept.lc`, `runtime/inclusive-range-index-returned.accept.lc`, `runtime/index-returned.accept.lc`, `runtime/midpoint-in-range.accept.lc`, `runtime/index-not-yet-returned.reject.lc` |
| `runtime.operations.conditions` | `runtime/bool-cast-is-zero-or-one.accept.lc`, `runtime/checked-sum.accept.lc`, `runtime/division-returned.accept.lc`, `runtime/inclusive-range-index-returned.accept.lc`, `runtime/index-returned.accept.lc` |
| `runtime.patterns` | `runtime/later-arm-knows-earlier-arms-failed.accept.lc`, `runtime/slice-pattern-gives-no-fact.reject.lc` |
| `runtime.try` | `runtime/checked-sum-with-question-mark.accept.lc` |
| `runtime.debug-assertions` | `runtime/debug-assert-with-no-claim.accept.lc`, `runtime/branch-on-debug-assertions.reject.lc`, `runtime/debug-assert-gives-nothing.reject.lc` |
| `runtime.arith.wrapping` | `contracts/cursor-replaced-whole.accept.lc`, `core/cast-in-range.accept.lc`, `core/wrap-written-out.accept.lc`, `runtime/midpoint-in-range.accept.lc`, `runtime/sum-in-range.accept.lc`, `core/cast-wraps.reject.lc`, `core/difference-may-wrap.reject.lc`, `runtime/sum-may-overflow.reject.lc` |
| `runtime.proofs.assigned` | `runtime/kept-proof-assigned-again.accept.lc`, `runtime/kept-proof-on-every-path.accept.lc`, `runtime/loop-invariant-by-induction.accept.lc`, `runtime/kept-proof-borrowed.reject.lc`, `runtime/kept-proof-left-behind.reject.lc`, `runtime/proof-holder-keeps-its-proposition.reject.lc` |
| `runtime.joins` | `runtime/assert-gives-its-condition.accept.lc`, `runtime/clamp-after-a-branch.accept.lc`, `runtime/kept-proof-on-every-path.accept.lc`, `runtime/loop-keeps-a-fact.accept.lc`, `runtime/branch-on-debug-assertions.reject.lc`, `runtime/debug-assert-gives-nothing.reject.lc`, `runtime/fact-from-one-branch.reject.lc`, `runtime/kept-proof-on-one-path.reject.lc` |
| `runtime.loops` | `runtime/binary-search-found.accept.lc`, `runtime/element-write-keeps-length.accept.lc`, `runtime/loop-invariant-by-induction.accept.lc`, `runtime/loop-keeps-a-fact.accept.lc`, `runtime/loop-invariant-missed-at-continue.reject.lc`, `runtime/loop-loses-a-fact.reject.lc` |
| `runtime.loops.for-range` | `runtime/for-over-a-range.accept.lc`, `runtime/for-over-an-iterator.reject.lc`, `runtime/range-bound-written-while-built.reject.lc`, `runtime/range-bounds-built-once.reject.lc` |

## Interactions

| Area | Effect |
|---|---|
| Types | None. Every understood operation keeps its Rust type. |
| Propositions and proofs | Defines "what is known at a point", which [RFC 0003](0003-proof-construction.md)'s hole and `linear` use. A `let mut` proof variable may be assigned; `proofs.versions` already says what a proof is about. |
| Ownership and mutation | A write gives a place a new version, as RFC 0002 says. This RFC adds what is known where paths join, and at the top of a loop. |
| Erasure and runtime behavior | None. No operation is added, removed, or checked differently, and arithmetic is read as it runs in each build. |
| Rust interoperability | Plain Rust compiles unchanged: no operation owes a proof. A call of the standard library is a call, as RFC 0002 says. |
| Diagnostics | A proof that needs a true result reports that the operation may overflow. A proof left behind by a write reports the write, as `proofs/stored-proof-after-clear.reject.lc` does. |
| Trust model | None added. That Locus reads each understood operation as Rust defines it is part of `trusted.obligations`. |

## Alternatives

**An invariant written on the loop,** as an attribute, in the way of Dafny, Verus, and Creusot. Proposed first, and replaced by the owner's `let mut` proof variable. An attribute is a new construct with a rule for each of `break`, `continue`, and the end of the body, and it proves the invariant again in a place that the source does not show. A proof variable is assigned where the author puts it, and one rule about joins covers every case. The attribute's one advantage is a `for` loop whose kept fact mentions the index, and it can be added later as shorthand.

**A state in which a proof cannot be read,** after a write to what it is about. Proposed with the proof variable, and removed by the owner: "what if we just said that if the proof is stale, it can not be used eventually unless the variable has current version? That may keep fewer concepts?" A proof about earlier values is still a proof. That it does not fit a later use is the rule Locus already has.

**Trying a hole when a kept proof has been left behind.** Rejected: the error would move away from the write that caused it.

**A proof that each arithmetic operation is in range,** as Verus, SPARK, and WUFFS ask. Rejected as the general rule, since every Rust function with no `unsafe` code is verified, and plain Rust would stop compiling. It is the rule under `#[no(panic)]`.

**Reading arithmetic as a build with overflow checks runs it,** where a return means no overflow, as Aeneas does. Rejected: a release build wraps, so a proof would be false there, unless Locus forced the checks on and so changed what a release build does.

**Checking each build configuration by itself.** Rejected: a proof would hold in one profile and fail in another.

**A longer list of understood methods:** those of `Option` and `Result`, `get`, `is_empty`, `min`, and `max`. Left out by the owner for now.

**A function that declares what its result is.** Two forms were put to the owner: a mark under which a function's body is its meaning, and a declared value that the body proves or assumes. The owner chose neither for now. Both are in [pure-functions](../../issues/pure-functions.md).

## Prior art

- **Aeneas** gives each primitive operation of Rust a value and a way to fail, which is where the two columns of the table come from. It turns each loop into a recursive function, which is this RFC's reading of a loop.
- **Dafny, Verus, and Creusot** write an invariant on the loop, and take every variable that the loop writes to be unknown but for it. This RFC keeps the second half.
- **Lean**'s fixed-width integers wrap, with theorems that relate the result to the true one modulo a power of two. That is this RFC's arithmetic.
- **Rust's RFC 560** defines integer overflow as a panic or the wrapped result, and never as undefined behavior, which is what makes one reading true in every build.

## Unresolved questions

**Decided by the owner,** in the session of 2026-11-17:

1. **An operation is understood when Rust gives it syntax and it is applied to a type that stays Rust's; each has a value and a condition under which it returns, both known once it has returned.** "I like your recommendation."
2. **`len` on arrays and slices is understood, and no method of `Option` or `Result` is, for now.** "Yes, let's do it and go to B."
3. **A fact that a loop keeps is a `let mut` proof variable that the program assigns again.** "what if we didn't have invariants but mut proof variables which are reestablished by the programmer on each iteration?" Assigning it is always written, and a loop whose kept fact mentions a `for` index is a `while`: "Yes all three decisions for B make sense."
4. **A proof left behind by a write is still a proof, and there is no state in which it cannot be read.** The owner's simplification, quoted under [Alternatives](#alternatives).
5. **Arithmetic means the wrapped result in every build, the strict rule belongs to `#[no(panic)]`, and `checked_add`, `checked_sub`, and `checked_mul` are understood.** "Yes all three rules make sense. We can lock this in".
6. **No way yet for a function of the program's own to mean something.** "Let's stay with nothing for now."

**Decided by the owner on 2026-11-21,** after the reviews of PR #25. Three choices and a list of confirmations were put together, and the owner answered "Yes to all":

7. **An operation is understood when it is in one of two closed lists,** what the language defines and a short list of library items, and comparisons are understood on integers and `bool` only.
8. **Shifts and bitwise operations on integers are left out for now,** and on `bool` they are the connectives.
9. **`debug_assert!` and a branch on `cfg!(debug_assertions)` give no fact.** A second reviewer preferred facts for each configuration, which RFC 0001 already has; the owner chose that what a program proves should not depend on the build profile.
10. **Confirmed with them:** `?` on an `Option`, and on a `Result` of one error type; rows for literals, constants, compound assignment, and patterns; what an arm of a `match` knows of the earlier arms; a condition for each form of range indexing; and that an operation with no row gives no fact.

**Still for the owner.** Each is written into the rules and examples as proposed:

1. **`Box::new` is understood.** RFC 0002 already lists a `Box`'s allocation among what the language defines, and six of its examples build one.
2. **What is known where paths join is what is known on every path, and no more.** So a variable that two branches assign differently is known afterwards by its type and by the proofs kept about it. Knowing it as an `if` of the two values would say more, and can be added.
3. **Details of the two lists that the owner did not see one by one:** a cast from a `bool`; that a reference pattern gives a fact, and a slice pattern none; that a statement under `#[cfg(debug_assertions)]` gives no fact, as a `debug_assert!` does; and that `&`, `|`, and `^` on `bool` do not short-circuit.
4. **A `let mut` proof variable's proposition must be written** at its declaration.
5. **`a..=b`** gives `a <= i && i <= b`.

**Left to later work:**

- The rest of the runtime semantics: `&mut` parameters and reborrows, partial moves, drops, and how much of this is trusted ([runtime-semantics](../../issues/runtime-semantics.md)).
- A function that declares what its result is ([pure-functions](../../issues/pure-functions.md)), and contracts on a trait's methods ([traits-on-logical-types](../../issues/traits-on-logical-types.md)).
- A claim that a loop ends ([termination-claims](../../issues/termination-claims.md)), and that a function cannot panic ([no-panic-claims](../../issues/no-panic-claims.md)).
- Naming the fact of a `while` condition and of a `match` arm ([naming-facts-from-control-flow](../../issues/naming-facts-from-control-flow.md)).

**Said since, in [RFC 0005](0005-core-terms.md):** what a write to a field or to an element tells Locus, and what is known after a `while` ends.

**Withdrawn.** An earlier revision of this list proposed that a slice's length is at most `isize::MAX`. That is false for an element type of size zero, which a proof is: `vec![(); usize::MAX]` gives a slice longer than that. Rust's bound is on a slice's size in bytes, so it bounds the length only for an element type known not to be zero-sized. What holds of every runtime slice is that its length fits a `usize`.
