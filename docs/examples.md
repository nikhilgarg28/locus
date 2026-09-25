+++
id = "examples"
title = "Examples"
group = "Now"
route = "examples.html"
order = 51
+++

# Useful code. Specific guarantees.

Each example answers three questions: what runs, what is promised, and where the evidence comes from. All executable examples are checked. Run directives exercise the interpreters and generated Rust; they are test annotations, not language syntax.

| Start here | What it demonstrates |
|---|---|
| [Choose an algorithm with optional evidence](#choose-an-algorithm-with-optional-evidence) | Scoped proof transport and a runtime choice. |
| [Return a value and its proof](#return-a-value-and-its-proof) | The smallest complete contract. |
| [Compute a midpoint without overflow](#compute-a-midpoint-without-overflow) | A specification that catches a familiar arithmetic bug. |
| [Validate once, then carry the invariant](#validate-once-then-carry-the-invariant) | Evidence across a constructor, a type, and a consumer. |
| [Read only an available element](#read-only-an-available-element) | A bounds proof obtained from control flow. |
| [Keep a counter bounded](#keep-a-counter-bounded) | A loop invariant maintained as evidence. |
| [Prove a sequence operation](#prove-a-sequence-operation) | Structural recursion and an induction proof. |

## Choose an algorithm with optional evidence

Suppose `Sorted(items)` claims that the input’s contents are in ascending order. `@Sorted(items)` is its proof type; `Option<@Sorted(items)>` lets the caller supply that evidence when available. The functions return an optional index for the requested key.

~~~rust run
// docs:hide
logic fn ordered_prefix(items: &[u32], count: Int, bounds: @(0 <= count && count <= items.len())) -> Bool {
    if count <= 1 { true } else {
        let remaining = count - 1;
        let smaller: @(0 <= remaining && remaining < count) = And::Intro(prove!(0 <= remaining), prove!(remaining < count));
        let within: @(0 <= remaining && remaining <= items.len()) = And::Intro(prove!(0 <= remaining), prove!(remaining <= items.len()));
        let prefix = recurse!(smaller, ordered_prefix(items, remaining, within));
        prefix && items.get(count - 2) <= items.get(count - 1)
    }
}
logic fn Sorted(items: &[u32]) -> Prop {
    let count = items.len();
    let within: @(0 <= count && count <= items.len()) = And::Intro(prove!(0 <= count), prove!(count <= items.len()));
    prop!(ordered_prefix(items, count, within))
}
// docs:show
#[no_panic]
fn binary_search(items: &[u32], key: u32, sorted: @Sorted(items)) -> Option<usize> {
    let mut lo: usize = 0;
    let mut hi = items.len();
    let mut bounded: @(lo <= hi && hi <= items.len()) = And::Intro(prove!(lo <= hi), prove!(hi <= items.len()));
    loop {
        if lo < hi {
            let mid = lo + (hi - lo) / 2;
            let value = items.get(mid);
            if value == key { break Option::<usize>::Some(mid); } else {
                if value < key {
                    lo = mid + 1;
                    bounded = And::Intro(prove!(lo <= hi), prove!(hi <= items.len()));
                } else {
                    hi = mid;
                    bounded = And::Intro(prove!(lo <= hi), prove!(hi <= items.len()));
                }
            }
        } else { break Option::<usize>::None; }
    }
}
// docs:hide
#[no_panic]
fn linear_search(items: &[u32], key: u32) -> Option<usize> {
    let mut i: usize = 0;
    loop {
        if i < items.len() {
            if items.get(i) == key { break Option::<usize>::Some(i); } else { i = i + 1; }
        } else { break Option::<usize>::None; }
    }
}
// docs:show
#[no_panic]
fn search(items: &[u32], key: u32, sorted: Option<@Sorted(items)>) -> Option<usize> {
    match sorted {
        Option::Some(proof) => binary_search(&items, key, proof),
        Option::None => linear_search(&items, key),
    }
}
// docs:hide
fn example() -> Option<usize> {
    let xs: [u32; 3] = [1, 3, 5];
    let sorted: @Sorted(xs) = fold!(Sorted, prove!(ordered_prefix(xs, 3, And::Intro(prove!(0 <= 3), prove!(3 <= xs.len())))));
    search(&xs, 3, Some(sorted))
}
// Sorted means every adjacent pair is nondecreasing. Empty/singleton inputs
// satisfy it. Its computation and evidence erase; Some/None does not.
// The checked contract here is sorted input plus panic-free index arithmetic
// and reads. Search-result completeness and termination are tested, not yet
// stated as return proofs.
//~ run: example() => Some(1)
// docs:show
~~~

- `Some(proof)` forwards evidence for this particular input to binary search. That function’s signature makes sortedness a required precondition.
- `None` leaves sortedness unknown. Linear search accepts the input without that precondition.
- The `Option` tag controls the runtime branch. Its proof payload is checked before execution and erased; dispatch does not inspect a proof or recheck the list’s order.

A caller could obtain the evidence from a verified sort or a validator, then reuse it for searches of the same unchanged contents. Evidence about an earlier snapshot does not certify a list after its contents change.

**What is proved:** the caller must supply sortedness evidence to enter binary search, and both algorithms satisfy `no_panic`, including arithmetic and index bounds. The tracked `bounded` proof maintains `lo <= hi <= items.len()` through the loop. Sortedness means adjacent elements are nondecreasing; its recursive logical definition and a checked concrete caller are in the complete source.

**What is tested:** search results on empty, singleton, duplicate, unsorted and boundary-valued lists, through both dispatch paths. This example does not yet state or prove result completeness or termination as a function contract. [Full source](../examples/optional_search.lc) and `tests/scoped_generics.rs` provide the implementation and differential tests.

## Return a value and its proof

The name `out` in the result type binds the returned byte for the later proof field. `_` requests evidence; the kernel checks it. Wrapping addition includes the boundary case where 255 becomes zero.

~~~rust run
fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    let out = n.wrapping_add(1);
    (out, _)
}
// docs:hide
fn consume(n: u8) -> u8 {
    let (value, correct) = increment(n);
    value
}
//~ run: consume(41) => 42
//~ run: increment(255) => (0, Erased)
// docs:show
~~~

A caller can forward `correct` as a proof argument. It cannot inspect the proof to choose a runtime branch. The returned byte executes normally; the proof becomes an erased position. See [proofs](spec/10-proofs.md) and [erasure](spec/12-erasure.md).

## Compute a midpoint without overflow

The mathematical specification is `(lo + hi) / 2`. Computing that sum in `u32` can overflow even when both inputs are valid. The implementation subtracts first, halves the difference, and adds it back. The input proof establishes that subtraction is safe; `no_panic` makes the compiler check every arithmetic safety condition.

~~~rust run
#[terminates] #[no_panic] #[no_alloc] #[no_io]
fn midpoint(lo: u32, hi: u32, ordered: @(lo <= hi))
    -> (mid: u32, @(mid == (lo + hi) / 2))
{
    let half = (hi - lo) / 2;
    let mid = lo + half;
    (mid, prove!(mid == (lo + hi) / 2))
}
// docs:hide
//~ run: midpoint(3, 10, Erased) => (6, Erased)
//~ run: midpoint(4294967294, 4294967295, Erased) => (4294967294, Erased)
// docs:show
~~~

Arithmetic inside the proof type uses `Int` models, so the specification’s sum does not overflow. This tempting implementation fails its safety obligation:

~~~rust reject L0235
#[no_panic]
fn midpoint(lo: u32, hi: u32, ordered: @(lo <= hi)) -> u32 {
    (lo + hi) / 2
}
~~~

The failure is useful: `lo <= hi` does not imply that their sum fits. See [machine arithmetic](spec/03-integers.md#machine-arithmetic).

## Validate once, then carry the invariant

A percentage is more useful than an unconnected proof of a number’s range. Its private proof field certifies its own value field. A constructor checks untrusted input at runtime; a consumer obtains the bound from the type without repeating the check.

~~~rust run
pub struct Percent {
    value: u32,
    valid: @(value <= 100),
}
impl Percent {
    #[no_panic]
    pub fn checked(value: u32) -> Option<Percent> {
        if value <= 100 {
            Some(Percent { value, valid: prove!(value <= 100) })
        } else { None }
    }
}
#[no_panic]
fn remaining(percent: &Percent) -> (out: u32, @(out + model!(percent.value) == 100)) {
    let out = 100 - percent.value;
    (out, _)
}
#[no_panic]
fn handle_input(input: u32) -> Option<u32> {
    let checked = Percent::checked(input);
    match checked {
        Option::Some(percent) => {
            let (left, correct) = remaining(&percent);
            Some(left)
        }
        Option::None => None,
    }
}
//~ run: handle_input(75) => Some(25)
//~ run: handle_input(100) => Some(0)
//~ run: handle_input(101) => None
~~~

The proof travels through three layers: validation, the `Percent` value, and `remaining`. The consumer’s subtraction is safe because every valid `Percent` carries the bound. The public constructor is usable from safe Rust; private fields prevent a caller from manufacturing `Percent { value: 101, ... }`. This proves a range property, not any application-specific meaning of a percentage. See [the export boundary](spec/13-rust-interop.md).

## Read only an available element

A bounds check can produce the fact required by an access. The runtime branch handles an empty slice; the other branch has evidence that index zero exists. The returned `Option` is ordinary runtime data.

~~~rust run
fn first(bytes: &[u8]) -> Option<u8> {
    if bytes.len() > 0 {
        Some(bytes[0])
    } else { None }
}
// docs:hide
fn demo() -> Option<u8> {
    let bytes: [u8; 2] = [42, 99];
    first(&bytes)
}
//~ run: first(&[]) => None
//~ run: demo() => Some(42)
// docs:show
~~~

For a caller that already knows the slice is nonempty, the same condition can be a proof parameter:

~~~rust check
fn first_known(bytes: &[u8], available: @(0 < bytes.len())) -> u8 {
    bytes[0]
}
~~~

The current compiler requires evidence for collection access. A function can establish it by checking at runtime, as `first` does, or require it in its interface. A missing proof is a compile-time error. See [collection models](spec/11-models.md#collections).

## Keep a counter bounded

`bounded` is the loop invariant: a proof about the current counter. Initial construction establishes it. Increment invalidates it; the next assignment supplies evidence for the updated value. It must hold on every loop back edge.

~~~rust run
#[no_panic]
fn count_to(limit: u8) -> (count: u8, @(count <= limit)) {
    let mut count: u8 = 0;
    let mut bounded: @(count <= limit) = _;
    while count < limit {
        count = count + 1;
        bounded = _;
    }
    (count, bounded)
}
//~ run: count_to(0) => (0, Erased)
//~ run: count_to(255) => (255, Erased)
~~~

The signature guarantees the bound on normal return, and `no_panic` guarantees the increment cannot overflow. It does not state exact equality with the limit or certify termination. Tests show concrete executions; they do not strengthen the declared theorem. The [verified lock](spec/16-walkthrough.md) uses this pattern across a separate transition function.

## Prove a sequence operation

A logical sequence is a finite value defined by constructors. `append` follows the first sequence’s structure. The theorem follows that same structure: the empty case needs no recursive hypothesis; the nonempty case reuses the theorem for the tail.

~~~rust check
#[derive(Logical)]
enum Seq { Empty, Cons { head: Int, tail: Seq } }

logic fn length(xs: Seq) -> Int {
    match xs {
        Seq::Empty => 0,
        Seq::Cons { head, tail } => 1 + length(tail),
    }
}
logic fn append(xs: Seq, ys: Seq) -> Seq {
    match xs {
        Seq::Empty => ys,
        Seq::Cons { head, tail } => Seq::Cons { head, tail: append(tail, ys) },
    }
}
logic fn append_length(xs: Seq, ys: Seq)
    -> @(length(append(xs, ys)) == length(xs) + length(ys))
{
    match xs {
        Seq::Empty => {
            let base: @(length(ys) == length(Seq::Empty) + length(ys)) =
                fold!(length, prove!(length(ys) == 0 + length(ys)));
            fold!(append, base)
        }
        Seq::Cons { head, tail } => {
            let induction = append_length(tail, ys);
            let step: @(length(Seq::Cons { head, tail: append(tail, ys) })
                == length(Seq::Cons { head, tail }) + length(ys)) =
                fold!(length, prove!(1 + length(append(tail, ys))
                    == 1 + length(tail) + length(ys)));
            fold!(append, step)
        }
    }
}
~~~

`induction` supplies the equation for the smaller `tail`. Arithmetic adds one to both sides. `fold!(length, ...)` closes the constructor’s length equation, then `fold!(append, ...)` closes the append equation. Every step produces evidence checked by the kernel. There is no axiom claiming that append is correct and no enumeration of all sequences.

This sequence is Logical and erases. A runtime vector needs a separately checked model and operation contracts. The [generic library version](../library/logical.lc) adds reusable `Seq<T>` and other operations; the [buffer example](../tests/corpus/target/library_buffer_model.lc) connects a runtime push to sequence observations.

## Read and run more

| Example | Next concept |
|---|---|
| [Explicit proof steps](../examples/proofs.lc) | Unfold, rewrite, and fold evidence. |
| [Named propositions](../examples/propositions.lc) | Constructor alternatives and proof matching. |
| [Verified lock](spec/16-walkthrough.md) | State transitions, mutation, and loop evidence. |
| [Finite maps](../library/finite_map.lc) | A logical representation carrying uniqueness evidence. |
| [Relations](../library/relations.lc) | Membership witnesses and reachability induction. |
| [Boxed lists](../library/runtime_list.lc) | A physical recursive type with a logical model. |

~~~sh prose shell-commands
cargo run -- check tests/corpus/target/midpoint.lc --holes
cargo run -- run tests/corpus/target/lock.lc run 300 9
cargo run -- build tests/corpus/target/percent.lc --out /tmp/percent_crate
~~~

The teaching sequence takes inspiration from [Lean’s evidence-based indexing](https://lean-lang.org/functional_programming_in_lean/Interlude___-Propositions___-Proofs___-and-Indexing/) and [induction material](https://lean-lang.org/functional_programming_in_lean/Interlude___-Tactics___-Induction___-and-Proofs/), and [Verus’s loop-invariant tutorial](https://verus-lang.github.io/verus/guide/while.html). These programs use Locus syntax and its current checked contracts; they do not imply parity with either tool.

## One implementation, two return interfaces

This function gives Locus callers an explicit proof slot. When selected by an export entry, it also gives Rust callers a data-only result:

~~~rust run
pub fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    let out = n.wrapping_add(1);
    (out, _)
}
fn checked_use(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    let (out, evidence) = increment(n);
    (out, evidence)
}
//~ run: checked_use(41) => (42, Erased)
//~ run: checked_use(255) => (0, Erased)
~~~

The project exporter generates `increment(n: u8) -> u8` for Rust and retains the pair-returning implementation privately. [Build and share a component](packages.md#export-a-function-that-returns-evidence) explains how to select the export. The complete source example above exercises Locus proof transport; filesystem and compiled-Rust tests exercise the separate facade.
