+++
id = "home"
title = "Locus"
group = "Now"
route = "index.html"
order = 0
+++

# Systems code. With something to prove.

Locus is a Rust-like language where propositions and kernel-checked proofs are part of the program. State a guarantee, carry the evidence, and compile the result to readable Rust.

<!-- component: home -->

## Use evidence when you have it.

A search can accept optional evidence that its input is sorted. With evidence, call binary search; otherwise, fall back to linear search. Binary search requires the proof explicitly.

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

`@Sorted(items)` is evidence about this input’s contents. The `Some`/`None` choice survives at runtime; the proof payload erases. `None` leaves sortedness unknown, so the fallback works on any input.

This is a checked excerpt. Binary search takes `sorted: @Sorted(items)`; the complete source includes its implementation and the sortedness definition. See [the example](examples.md#choose-an-algorithm-with-optional-evidence) for its guarantees.

## Three ideas, kept close together

### Write the code

Use familiar structs, enums, ownership, mutable bindings, and control flow. Locus aims at the part of a Rust crate whose contract deserves more than a comment.

### State the claim

A proposition describes what should hold. Its proof can be a parameter, a local binding, a return value, or a field beside the data it describes.

### Check the evidence

Proof construction can use explicit steps and bounded automation. Every accepted certificate passes through the kernel, including proofs loaded from `Locus.lock`.

## Try a small example

The compiler is a research implementation. Start with the checked examples and the current language manual; broader Rust interoperability is still being developed.

~~~sh prose shell-commands
git clone https://github.com/nikhilgarg28/locus.git
cd locus
cargo run -- check examples/increment.lc
cargo run -- run examples/increment.lc consume 41
cargo run -- rust examples/increment.lc
~~~

Requires Rust 1.91 or newer. The [examples](examples.md) explain the evidence; the [correctness chapter](correctness.md) explains what checking does—and where its assumptions remain.
