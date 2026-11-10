# Changing a field that an invariant reads, through a borrow

- **Kind:** decision
- **Status:** open
- **Blocks:** Data structures with invariants that change in place, such as a `push` on a type whose proof field reads its vector; idea 2 of [external-review-ideas](external-review-ideas.md)
- **Rules:** `contracts.invariants.no-mutable-borrow`, `contracts.invariants.assignment`, `contracts.invariants.observed`

## Question

RFC 0002 forbids mutably borrowing a field that a proof field reads (`contracts.invariants.no-mutable-borrow`). So this is rejected, though it keeps the invariant:

```rust prose rejected today
struct NonEmpty {
    items: Vec<u32>,
    ok: @(items.len() > 0),
}

impl NonEmpty {
    fn push(&mut self, x: u32) {
        self.items.push(x); // borrows `items` mutably
    }
}
```

What works today is assigning the field a whole new value, which for a vector means building it again. Most data structures with invariants change their fields in place, so most of them cannot be written. How should a verified function change such a field through a borrow?

The rule is there for a reason. While the field is borrowed, the value may not satisfy its invariant, and RFC 0002 promises that every value of an invariant type satisfies it wherever verified code can observe it (`contracts.invariants.observed`). Three things could observe it in between:
- the code doing the update, if it used the value whole or read its proof fields;
- a caller, if the borrow outlived the function, as in `fn items_mut(&mut self) -> &mut Vec<u32>`;
- a destructor, a `catch_unwind`, or another thread, if a panic unwound out of the update. Unwinding cannot be made harmless by a rule about what is believed afterwards: a `RefCell<NonEmpty>` gives no sign that a panic interrupted an update, and neither does a lock without poisoning.

## What it blocks

Any in-place update of a field that an invariant reads: `push`, `pop`, `sort`, writing an element, and every update of two fields that an invariant relates, such as a length and a buffer. Until this is decided, such types rebuild the field and assign it.

## Options

### 1. An `open!` block, which is atomic. Recommended.

```rust prose proposed
impl NonEmpty {
    fn push(&mut self, x: u32) {
        open!(self => {
            self.items.push(x);
        });
    }
}
```

`open!(v => { ... })` opens the invariant of `v`, a local or the target of a `&mut` whose type has proof fields, for the length of the block.

- **Inside the block,** the fields that `v`'s proof fields read may be mutably borrowed, and they and the proof fields may be assigned without proof. Any code may run there, including code that Locus does not verify.
- **Nothing observes `v` while it is open.** In the block, `v` is not used as a whole, by value or by reference, and its proof fields are not read; only its fields are used. No borrow of a field outlives the block. The block is left only at its end: not by `return`, `break`, `continue`, `?`, or `.await`.
- **At the end of the block,** every proof field of `v` is proved for the values its fields then have, as if each were assigned a hole. A proof field that search cannot prove is assigned in the block's last statements.
- **A panic that would unwind out of the block aborts the process.** The block holds a guard, in both programs of build mode and in an exported program, that aborts if it is dropped by unwinding. So no destructor, `catch_unwind`, or other thread sees `v` open. This is what the `take_mut` and `replace_with` crates do for the same reason.
- **Outside a block, nothing changes.** `contracts.invariants.no-mutable-borrow` still holds there, and a single assignment that keeps the invariant needs no block.

In the reading of RFC 0002, the block takes `v` apart into its fields, runs, and builds `v` again, which is when its proofs are owed. The guard is what makes the program that runs agree with that reading.

What it costs:
- **A panic inside the block is an abort, not an unwind.** This is a second exception to commitment 2 of the vision, that erasure never changes what the program does, beside source locations. It touches only code that opens an invariant, never a plain Rust program, and nothing changes under `panic = "abort"`. A server that isolates requests with `catch_unwind` loses that isolation for a panic in the middle of such an update.
- **A new name,** `open!`, among Locus's macros, and a block at each place a field is changed in place.
- **A guard in the trusted library,** a few lines.

Later, without changing any accepted program: Locus can leave the guard out where it shows that the block cannot unwind.

### 2. No syntax: the invariant is open while the borrow lasts, and Locus reasons about unwinding.

`self.items.push(x)` is accepted as written. The invariant is open from the borrow of `items` until rustc's borrow facts say the borrow ends, where the proof fields are proved again. For unwinding, every operation in between that may panic must leave a state for which the invariant can be proved. That needs, for each call that is given the borrow, what the borrowed place holds if the call unwinds: "unchanged" for `Vec::push`, "some permutation" for `sort_by`, and "unknown" for code Locus does not verify.

- For: no new syntax, no change at run time, and the common single call just works.
- Against: every specification gains an outcome for unwinding, which is new trusted text in the library and a second postcondition on every verified function that takes `&mut`. Partial correctness, where a panic proves nothing, stops being true inside these regions. An update of two fields has no borrow that spans both, so it still needs a block or a trick to keep the borrow alive. And where the region ends is decided by rustc's borrow analysis, not by anything the reader sees.

### 3. Stay as it is.

Keep the ban, and rebuild fields. Sound and simple, and it rules out most data structures with invariants.

### Comparing them

| | 1. `open!`, atomic | 2. Implicit, unwind outcomes | 3. As it is |
|---|---|---|---|
| `self.items.push(x)` | In a block | As written | Rejected |
| Two fields updated together | Yes | Awkward | Only by replacing the value |
| Unverified code on the field | Yes; the invariant is proved after | Only if the invariant holds for any value | Rejected |
| Run-time change | A panic in the block aborts | None | None |
| New trusted text | The guard | An unwind outcome in every specification | None |
| Where the invariant is open | Where the source says | Where rustc's borrows say | Nowhere |

Option 1 can be followed by option 2 for single specified calls, as a way to leave the block out. The reverse order would commit the library's specifications to unwind outcomes first.

## Example

Decided as option 1 gives them, citing `contracts.invariants.open`:
- [`contracts/invariant-opened-for-push.accept.lc`](../litmus/contracts/invariant-opened-for-push.accept.lc): the `push` above.
- [`contracts/invariant-opened-two-fields.accept.lc`](../litmus/contracts/invariant-opened-two-fields.accept.lc): a count and a vector that an invariant relates.
- [`contracts/invariant-opened-not-restored.reject.lc`](../litmus/contracts/invariant-opened-not-restored.reject.lc): a block that ends with the invariant broken.
- [`contracts/invariant-opened-used-whole.reject.lc`](../litmus/contracts/invariant-opened-used-whole.reject.lc), [`invariant-opened-proof-read.reject.lc`](../litmus/contracts/invariant-opened-proof-read.reject.lc), and [`invariant-opened-left-early.reject.lc`](../litmus/contracts/invariant-opened-left-early.reject.lc): what a block may not do.

## Log

### 2026-11-09, fourth review of PR #10

Raised by the fourth review of [#10](https://github.com/nikhilgarg28/locus/pull/10) as the largest gap in what can be proved about ordinary code, and written up at the owner's request as a proposal. Option 1 is the review's recommendation. The choice is the owner's, because it changes what a program does when it panics and so needs a change to the vision.

### 2026-11-09, the owner's decision

Asked whether a panic inside the block may abort, the owner answered:

> Open block is fine

RFC 0002 proposes option 1 as its decision 51, with the rule `contracts.invariants.open`, and the vision's commitment 2 names the abort. The three open examples are decided, and three rejected ones are added. The litmus format cannot yet expect an abort, so no example shows that a panic in a block aborts. This issue stays open until RFC 0002 is accepted. Left for later, and recorded in the RFC: leaving the guard out where a block cannot unwind, and option 2 for single specified calls.
