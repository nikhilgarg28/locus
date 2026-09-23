# Locus

Locus is a Rust-like language with propositions and explicit, kernel-checked proofs. Functions can return evidence alongside ordinary data; logical computation is erased, and executable code is emitted as readable Rust.

```rust
fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    let out = n.wrapping_add(1);
    (out, _)
}
```

Here `@P` is the type of evidence for proposition `P`, and `_` asks the compiler to construct a proof for the kernel to check. The proof states wrapping behavior, including at the byte boundary. More substantial examples carry invariants through mutation and protect their generated interface from safe Rust callers. See [examples](docs/examples.md) and [correctness](docs/correctness.md) for the guarantees, assumptions, and remaining proof obligations.

## Read the book

The public documentation is built from Markdown under `docs/`. It contains the [language specification](docs/specification.md), [examples](docs/examples.md), [correctness argument](docs/correctness.md), [performance records](docs/performance.md), and [roadmap](docs/roadmap.md).

The website needs Python 3.11 or newer, Node.js 20 or newer, and npm 10 or newer:

```sh
npm ci --prefix website
python3 tools/site.py serve
```

The server builds the site and opens it locally. After editing Markdown, rebuild to refresh the static output:

```sh
python3 tools/site.py build
python3 tools/site.py check
```

Generated pages live in `target/site`. Edit the Markdown sources, not the generated HTML. `atlas.html` is a compatibility entry point; it is no longer a writable, all-in-one document. `tools/atlas.py` retains command-line document and roadmap helpers, with `serve` forwarding to the site tool.

## Try the compiler

The compiler requires Rust 1.91 or newer.

```sh
cargo run -- check tests/corpus/target/lock.lc --holes
cargo run -- run tests/corpus/target/lock.lc run 300 9
cargo run -- rust tests/corpus/target/lock.lc
cargo run -- build tests/corpus/target/percent.lc --out /tmp/percent_crate
```

An unlocked check writes reusable, checked certificates to `Locus.lock` beside the source. Add `--locked` to require replay through the kernel without proof search or writes.

## Contribute

The [development guide](docs/development.md) covers dependencies, specification citations, executable examples, diagnostics, and measurement policy.

```sh
cargo fetch --locked
npm install --prefix editors/vscode/locus
cargo run --release -- bench
tools/check.sh
tools/check.sh --extended
```

The fast gate requires recent benchmark history for the current target corpus, machine, and toolchain. The extended gate runs the release stress suite and refreshes measured status. A completed test run is evidence about its recorded source revision, not a mechanized compiler-correctness theorem.
