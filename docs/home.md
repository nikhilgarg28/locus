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

## A familiar program. A precise promise.

An ordinary function can return a value together with evidence about that value. The proof is checked before execution; the generated Rust keeps the computation and erases the evidence.

<!-- component: specimen -->

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
