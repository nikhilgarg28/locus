# Runtime semantics of verified code

- **Kind:** decision
- **Status:** open
- **Blocks:** Any proof about what runtime code computes; freezing the facts format and the choice of THIR or MIR in RFC 0001; the planned semantic slice: verifying one small mutable function with a reborrow and a destructor end to end through the pipeline, with its certificate and a replay of the stored proof. It is meant to run alongside this RFC, to expose the pipeline's constraints early
- **Rules:** `proofs.versions`, `contracts.final`, `logic.boundary.escape`, `logic.unverified.functions`

## Question

How rustc's typed bodies become statements in the logic: the **runtime semantics** of verified code. RFCs 0001 and 0002 ([#10](https://github.com/nikhilgarg28/locus/pull/10)) fix the interface, and leave this to its own RFC.

An external review asked for this RFC early, before the pipeline's representations are frozen, because it decides whether several of their commitments can be implemented as stated. It needs to answer:

- **Versions.** RFC 0002 says every write makes a new version, as in SSA, and that propositions are about versions. This RFC decides how versions are formed from typed bodies:
  - at branch joins and in loops;
  - for partial moves and reborrows;
  - which facts relate one version to another.
- **Mutation and `&mut`,** for example through Aeneas's functional reading.
  - RFC 0002 fixes the meaning in signatures: a parameter means its value at entry, and `final!(r)` is a mutable borrow's final value, a prophecy.
  - This RFC decides how bodies are translated, including where each borrow ends, which is where prophecies are resolved.
  - rustc computes where borrows end on MIR. Drops, unwinding, and borrows pull toward MIR; source-shaped typed expressions pull toward THIR.
- **What remains true after an opaque call,** and what is forgotten:
  - a completable value lent mutably to unverified code comes back as any value of its type;
  - values read through raw pointers, or cast or transmuted, are forgotten.
- **Drops and unwinding.**
  - Which obligations hold on unwind.
  - How explicit and implicit drops affect postconditions, which RFC 0002 checks after the locals' drops.
  - Whether a type's invariant may be broken temporarily, inside an operation whose contract restores it on return and on unwind ([#18](external-review-ideas.md)).
- **Loops and loop invariants,** including tracked evidence (`let mut h: @P`), which RFC 0002 defers here.
- **Integer overflow,** where proofs must hold under every build profile. The `overflow-checks` setting is not a stable `cfg`, so no configuration guard can test it ([#9](cfg-corner-cases.md)).
- **Panics, and calls Locus cannot translate.**
- **Narrower observations:** whether reading a proof field can observe only the fields its proposition reads, rather than the whole value.
- **Obligation generation.** Turning claims and typed bodies into the obligations the kernel checks is trusted (`trusted.obligations` in `design/TRUST.md`). This RFC decides how much of it is, and how that trust is reduced. The kernel establishes the theorem it is given; the translation must establish that the theorem describes the program.

## What it blocks

- Any proof about what runtime code computes.
- Freezing the facts format and the choice of THIR or MIR in RFC 0001.
- The planned semantic slice: verifying one small mutable function with a reborrow and a destructor end to end through the pipeline, with its certificate and a replay of the stored proof. It is meant to run alongside this RFC, to expose the pipeline's constraints early.

## Options

To be laid out in the RFC. The choice between THIR and MIR as the source is part of it.

## Example

None yet. The stale-proof examples in `litmus/proofs/` and `litmus/contracts/`, and `litmus/observation/proof-before-move.accept.lc`, rely on the minimum facts that RFC 0002 requires of whatever this RFC decides.

## Log

### 2026-10-25, opened on GitHub as #6

Moved into the repository from [GitHub issue #6](https://github.com/nikhilgarg28/locus/issues/6) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.

### 2026-11-09, fourth review of PR #10

RFC 0002 now states the reading that this RFC must define precisely ([What a checked program means](../design/rfcs/0002-logical-layer.md#what-a-checked-program-means)), and its decision 45 says what a checked function's summary records. The review also named the largest gap in what can be proved about ordinary code: a field that an invariant reads cannot be borrowed mutably, so `self.items.push(x)` is rejected. [opening-invariants](opening-invariants.md) lays out the options for the owner; what a drop or a caught panic may observe while such a field is borrowed is part of the reading that this RFC defines ([soundness-model](soundness-model.md)).

### 2026-11-09, conversation with the owner about PR #24

RFC 0002's decision 54 makes a function verified whole or not at all: one that contains `unsafe` code or handles a raw pointer is unverified. So this RFC translates only functions with no `unsafe` code and no raw pointers, and treats every unverified function as an unknown function of its Rust type. A verified function's summary records only what it may write; escape, and the assumption about Rust's aliasing rules, are gone. The words are now verified and unverified, where earlier entries of this log say checked and unchecked.

### 2026-11-16, proposal made at the owner's request

The owner asked for an audit of the decisions that are still open, with a proposal for each. This is the proposal for this issue. Nothing in it is decided.

Stated as semantics only; which of rustc's representations Locus reads is an implementation choice that the owner has set aside for now.
- **The reading:** a verified function is a pure function of values, as RFC 0002 says, with each place given a new version at each write, in the manner of Aeneas. A `&mut` parameter is a value in and a value out, which `final!` names.
- **The first slice:** straight-line code, `if` and `match`, calls of verified functions, and `&mut` parameters. No loops. This is enough for every accepted example that has no loop.
- **Loops** need an invariant: a proposition that holds on entry and that each iteration keeps. It is carried as tracked evidence, a `let mut h: @P` that the loop body re-establishes, so that a loop adds no new kind of annotation.
- **Overflow:** an arithmetic operation means the same operation on the models when it does not overflow. A proof that uses its result needs a proof that it did not, in every build profile; where the result is not used in a proof, nothing is owed, so Rust code compiles unchanged.
- **Panics and drops:** a panic proves nothing, and since a value with an invariant is valid at every point (RFC 0002, decision 67), unwinding needs no rule of its own. A drop is a call.
- **What the language defines** on Rust's built-in types (RFC 0002, decision 82) is listed and given its meaning here.

### 2026-11-17, conversation with the owner on what runtime code tells Locus

The owner took up this issue as one question, what Locus knows about the value of a runtime expression, in three parts: which operations it understands, loops, and overflow. The first part is decided. On the proposal: "I like your recommendation." On keeping the list of methods to one: "Yes, let's do it and go to B."

**Decided:**

- **An operation is understood when Rust gives it syntax and it is applied to a type that stays Rust's** (RFC 0002, decision 82): arithmetic, division and remainder, comparisons and the boolean operators, casts between integer types, indexing and range indexing of arrays and slices, building and matching tuples, structs, and enums, `?`, taking and following a reference, following a `Box`, and `for` over a range of integers.
- **One method is understood: `len`, on arrays and slices.** No method of `Option` or `Result` is, for now; the owner: "it's always possible to add more later". The methods of the integer types, such as `checked_add`, are decided with overflow.
- **Each understood operation has a value and a condition under which it returns, and once it has returned Locus knows both.** After `let x = xs[i];`, Locus knows `i < xs.len()`, and that `x` is the element at `i`. A panic proves nothing, and a return proves that the check passed. So `assert!` needs no rule of its own: it is an `if` whose other branch does not return. `debug_assert!` gives nothing, since a release build leaves it out.
- **Everything else is a call,** known through its signature: `opt.unwrap()`, `xs.get(i)`, `xs.iter()`, an operator on a type of the program's own, and `==` on a type with a derived `PartialEq`.

**In the proposal the owner accepted, and still this issue's to write down as rules:** each operation's value and condition; that floats are left out; that shifts and bit operations are named functions which `compute` evaluates and `linear` takes as atoms; and `Box::new`, which RFC 0002 already lists as defined by the language, and without which a `Box` could neither keep its value nor hold a proof.

**Deferred by the owner:** a way for a function of the program's own to declare what its result is, so that a call of it is understood as these operations are ([pure-functions](pure-functions.md)). "We will come back and examine pure functions later after B and C".

Nothing here is in an RFC or the specification yet. It will be written with the other two parts.

### 2026-11-17, conversation with the owner on loops

The second part, loops, is decided. The reply had proposed an invariant written on the loop as an attribute. The owner proposed another way:

> what if we didn't have invariants but mut proof variables which are reestablished by the programmer on each iteration?

and, on the three questions that followed, "Yes all three decisions for B make sense." The owner then removed a rule from the proposal, under which a proof left behind by a write could not be read at all:

> I'm not sure if we even need to have rules that stale proofs can't be used - what if we just said that if the proof is stale, it can not be used eventually unless the variable has current version? That may keep fewer concepts?

**Decided:**

- **There is no invariant construct.** A fact that a loop keeps is a proof variable declared `let mut` before the loop, which the body assigns again after it changes what the fact is about.
- **Assigning it is always written.** Locus does not try a hole by itself when a proof has been left behind.
- **A proof is about the values it was made from, and that is all.** There is no state in which a proof cannot be read. A proof that a write has left behind is still a proof, about the earlier values, and the rule that already exists does the rest: a proof must prove what its use requires, so it cannot stand where a fact about the current values is needed. The error is the one `proofs/stored-proof-after-clear.reject.lc` already shows.
- **`mut` on a proof variable means what it means in Rust:** the variable may be assigned. Its proposition is written at its declaration, and each assignment is checked against that proposition at the values of that moment.
- **Where paths join, a proof variable proves its proposition for the current values only if it does on every path.** The top of a loop is such a place: the paths are the entry and the end of each way through the body. This one rule is what makes the variable an invariant, and it covers `break`, `continue`, and nested loops with nothing more.
- **At the top of a loop, a variable that the loop writes is known by its type, and by such proofs.** A variable that the loop does not write keeps what was known of it.
- **`for i in a..b` gives `a <= i && i < b` in its body.** A loop whose kept fact mentions the index is written as a `while` with a counter of the program's own, for now, since `i` is not in scope where the proof would be declared.
- **Termination is not asked for.**

```rust prose the shape the owner chose; not yet a rule
let mut bounds: @(lo <= hi && hi <= xs.len()) = _;
while lo < hi {
    let mid = lo + (hi - lo) / 2;
    if xs[mid] < target {
        lo = mid + 1;
        bounds = _;
    } else if xs[mid] > target {
        hi = mid;
        bounds = _;
    } else {
        return Some((mid, _));
    }
}
```

This is the tracked evidence that RFC 0002 had deferred to this issue, in a simpler form than it was sketched there: nothing follows a version, and a proof is assigned.

The third part, overflow, is next.

### 2026-11-17, conversation with the owner on overflow

The third part, overflow, is decided: "Yes all three rules make sense. We can lock this in".

**Decided:**

- **`a + b`, `a - b`, and `a * b` on machine integers mean the wrapped result, in every build, and nothing is owed at the operation.** Rust defines overflow in both kinds of build: with overflow checks it panics, and without them the result wraps. One statement is true in both: if the operation returned, its result is the wrapped value. So a proof holds whichever way the crate is built, as the vision's first commitment asks, and plain Rust compiles unchanged.
- **The true result follows when it is in range.** Locus knows the range of every machine integer, and that wrapping does nothing to a value in range. `linear` does this itself, since wrapping is a remainder by a literal. So overflow shows only where a proof needs the true value and the facts do not give the range.
- **Division and remainder return only if the divisor is not zero,** in every build, so that fact is given. A cast between integer types is always defined, as wrapping.
- **The strict rule, that each operation is proved to be in range, belongs to `#[no(panic)]`** ([no-panic-claims](no-panic-claims.md)), since a build with overflow checks could panic there. It is asked for one function at a time, so it does not reject plain Rust.
- **`checked_add`, `checked_sub`, and `checked_mul` join the understood methods,** beside `len`: each gives `Some` of the true result when it is in range, and `None` otherwise. No other method of the integer types does, for now. `wrapping_add` means what `+` means under the first rule, and matters only once `#[no(panic)]` exists.

**In the proposal, and still this issue's to write down:** that shifts mean the shift by the amount reduced to the type's width, which is also true in both kinds of build; and negation, which wraps as the other operations do.

With this, the three parts of the question are decided: understood operations, loops, and overflow. None is in an RFC or the specification yet. The owner next wants to explore whether a program needs a way to give a function of its own a meaning, which is [pure-functions](pure-functions.md), and that may change how the first part is written.

### 2026-11-17, conversation with the owner on what runtime code tells Locus, concluded

The owner asked for the three decided parts to be written down: "I suppose we can lock in our decisions here. Please file them in an RFC".

[RFC 0004](../design/rfcs/0004-runtime-code.md) is the result, as a draft: it has had no adversarial review. It adds a chapter to the specification, [Runtime code](../spec/runtime.md), with eight rules and their examples. It lists what the owner decided, and what it proposes beyond that: that `Box::new` is understood, that what is known where paths join is what is known on every path and no more, the rows of its table that were not discussed one by one, that a kept proof's proposition is written, and `a..=b`.

What stays in this issue is the rest of the runtime semantics: `&mut` parameters and reborrows, where borrows end, partial moves, drops and unwinding, calls that Locus cannot translate, and how much of obligation generation is trusted.
