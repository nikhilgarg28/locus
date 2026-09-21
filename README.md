# Locus

*Review the guarantees, check the implementation, and use the result from ordinary Rust.*

Locus is a Rust-like language in which propositions and proofs are ordinary parts of a program. Explicit proof evidence is checked by a small kernel, proofs are erased, and the rest is emitted as plain Rust.

```rust
fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
    let out = n.wrapping_add(1);
    (out, _)
}
```

**Everything else is in [atlas.html](atlas.html).** Open it in a browser: it holds the design notes, the language specification, the architecture, the kernel contract, a table of what is built and what is left, and the projects and tasks, and it is where they are edited. `python3 tools/atlas.py serve` opens it from a local address, where it saves itself as changes are made, in any browser; the same tool reads and writes the documents from the command line.

```sh
cargo run -- check examples/lock.lc --holes    # types and proofs; every `_` and how it was filled
cargo run -- run examples/lock.lc attempts_left 5 9
cargo run -- rust examples/lock.lc             # the generated Rust
cargo test
```
