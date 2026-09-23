+++
id = "examples"
title = "Examples"
group = "Now"
route = "examples.html"
order = 51
+++

# Small programs. Explicit guarantees.

These examples introduce the relationship between executable code, propositions, and evidence. The compiler checks the examples on this page as part of the documentation test suite.

## Return a value and its proof

The name `out` in the tuple type binds the first component for use in the second component's claim. `_` asks proof construction to fill the evidence slot; the kernel checks the result. Wrapping addition has the same meaning at the byte boundary as it does in Rust.

~~~locus run
fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    let out = n.wrapping_add(1);
    (out, _)
}

fn consume(n: u8) -> u8 {
    let (value, _) = increment(n);
    value
}
//~ run: consume(41) => 42
//~ run: increment(255) => (0, Erased)
~~~

The caller can transport the proof to another Locus function, retain it in a product, or ignore it. It cannot inspect the proof to choose a runtime branch. See the [proof rules](spec/10-proofs.md) and the full [source example](../examples/increment.lc).

## Name a proposition and supply its evidence

A proposition declaration gives names to the ways it can be established. In this example, `Below` needs evidence of its body. The expression after `@` supplies it.

~~~locus check
prop Small(n: Int) {
    Below => { prop!(n < 10) }
}

logic fn five_is_small() -> @Small(5) {
    Small::Below @ prove!(5 < 10)
}
~~~

`logic fn` is checked logical computation. It has no runtime implementation. An ordinary `fn` can also return evidence, but its executable work remains. See [logical computation](spec/08-logic.md).

## Carry an invariant through mutation

The [verified lock](spec/16-walkthrough.md) is a complete worked example. It limits failed attempts to three, returns evidence from each transition, and refreshes tracked evidence after assignment in a loop.

The example connects [snapshots and mutation](spec/07-mutation.md), [ownership](spec/06-ownership.md), and the [Rust export boundary](spec/13-rust-interop.md). Its [source file](../tests/corpus/target/lock.lc) is also compiled and run by the acceptance suite.

## Continue in the repository

| Example | What it exercises |
|---|---|
| [Explicit proof steps](../examples/proofs.lc) | Construct, rewrite, and transport evidence. |
| [Propositions](../examples/propositions.lc) | Logical predicates, named arms, and proof matching. |
| [A checked percentage](../tests/corpus/target/percent.lc) | A validated type usable from safe Rust. |
| [An overflow-safe midpoint](../tests/corpus/target/midpoint.lc) | Machine arithmetic justified by integer reasoning. |
| [A modeled buffer](../tests/corpus/target/library_buffer_model.lc) | Collection models and evidence across operations. |

~~~sh prose shell-commands
cargo run -- check tests/corpus/target/lock.lc --holes
cargo run -- check tests/corpus/target/lock.lc --locked
cargo run -- build tests/corpus/target/percent.lc --out /tmp/percent_crate
~~~
