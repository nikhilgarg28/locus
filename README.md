# Locus

*Review the guarantees, check the implementation, and use the result from ordinary Rust.*

Locus is a Rust-like language for the parts of a crate whose correctness matters most. Propositions and proofs compose with ordinary functions and data, and explicit proof evidence is checked, deterministically, by a small, auditable kernel. Locus is designed to verify systems implementations with precise arithmetic, ownership, and mutation semantics, then emit readable Rust without runtime proof machinery. Generated interfaces protect proven invariants at the boundary with safe Rust callers. A programmer or an AI supplies the implementation and evidence against a human-reviewed specification; proof construction can be automated, while acceptance rests on independent checking. That is the destination; the core implemented so far covers `bool`, `u8`, structs, enums, and loops without mutation.

```rust
fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
    let out = n.wrapping_add(1);
    (out, _)
}
```

`@[...]` is the type of evidence for a claim, and `_` asks the compiler to find that evidence. What it finds is an explicit proof, which a small kernel checks. Proofs are erased, and the rest is emitted as plain Rust.

## Status

A `.loc` file goes from text to a checked program, an interpreted result, and generated Rust. The kernel, the checker for executable code, erasure, a reference interpreter, the Rust printer, the parser, and the elaborator with its bounded proof search and diagnostics all exist. `examples/lock.loc` is the end-to-end example. There is no mutation, no arithmetic beyond a byte, and no protected export to Rust yet.

## Running it

Requires Rust 1.85 or newer. The only dependency is the diagnostic renderer.

```sh
cargo run -- check examples/lock.loc            # types and proofs
cargo run -- check examples/lock.loc --holes    # every `_`, how it was filled, and what it cost
cargo run -- check examples/lock.loc --stats    # per function: time to elaborate and to check
cargo run -- run examples/lock.loc attempts_left 5 9
cargo run -- rust examples/lock.loc             # the generated Rust
cargo run -- tokens examples/increment.loc
cargo run -- parse examples/increment.loc
cargo run -- ast examples/preserve.loc
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

`check`, `run`, and `rust` stop at the first stage that reports an error. Errors in the source exit with status 1, errors in the command line with status 2. Diagnostic color is enabled for terminals unless `NO_COLOR` is set. `run` takes `u8` and `bool` arguments.

## Documents

| | |
|---|---|
| [docs/notes.md](docs/notes.md) | The only document about the future: positioning, agreed directions, open questions, deferred features |
| [docs/roadmap.md](docs/roadmap.md) | Batches of work |
| [docs/language.md](docs/language.md) | The language as specified now, with its grammar and its relationship to Rust's features |
| [docs/architecture.md](docs/architecture.md) | How the compiler is built: the representations, the trusted base, the elaborator and its proof search |
| [docs/kernel-contract.md](docs/kernel-contract.md) | Every kernel rule, with exact premises and conclusion; it changes together with the kernel |

A fact about the future lives only in the notes. When something is built, its text moves from there into the document that describes what exists.
