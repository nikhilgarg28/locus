# Running the first increment

Requires Rust 1.85 or newer and Cargo. The only direct dependency is the diagnostic renderer; lexing and parsing are handwritten.

```sh
cargo run -- tokens examples/increment.loc
cargo run -- parse examples/increment.loc
cargo run -- ast examples/preserve.loc
cargo run -- parse examples/propositions.loc
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The proof kernel is a separate module with no command-line surface yet. `cargo test --test kernel` runs its acceptance tests, which build kernel terms by hand; see [the kernel contract](kernel-contract.md) for the rules implemented so far.

`tokens` prints token kinds, source spans, and original spellings. `parse` validates syntax and reports the declaration count. `ast` prints the source-located syntax tree. Syntax errors exit with status 1; command-line usage errors exit with status 2. Help exits successfully. Diagnostic color is enabled for terminals unless `NO_COLOR` is set.

The command-line frontend does **not** resolve names, check types or proofs, interpret programs, or generate code; nothing connects the parser to the kernel yet. In particular, `fn impossible() -> @[false] { _ }` can parse successfully, as can an unbracketed Boolean in a `Prop` annotation, a proof hole in a runtime-data position, or a proposition-valued `fn` signature. Those require semantic diagnostics in subsequent batches; parsing success does not mean they are accepted programs. There is deliberately no `check` or `run` command yet.

See [the core plan](core-plan.md) for the implementation order and [the grammar](grammar.md) for current syntax and limits.

The current proof spelling is `@claim` / `@[condition]` in types, `@{ ... }` for proof commands, and `_` for an automatic proof request. Bare `@` is an error. Hash-based proof forms are rejected with migration guidance; `#[...]` and `#![...]` remain reserved attribute syntax. In a pattern, `_` still means to ignore the matched value. The `@{ ... }` block is retired by the specification in favor of proofs written as ordinary expressions; the parser still accepts it until batch 0.3b.

Bracket expressions remain neutral in the AST until typing supplies context. The parser also preserves explicit array forms and array/slice type syntax, but no array execution is implemented. The `!`, `&&`, `||`, and `=>` nodes likewise need semantic checking to distinguish operations on proposition values from executable Boolean operations.

The frontend currently spells pure, total functions `def`; the specification spells them `math fn`, and `def` will be replaced in batch 0.3b. `fn` declares executable functions with optional proof parameters/results. The AST preserves the distinction; checking purity, totality, and ghost-to-runtime data flow is not yet implemented. The files in `examples/` follow the implemented grammar. See [the grammar](grammar.md) for the full list of differences from [the specification](core-language-spec.md), and [rust-features.md](rust-features.md) for the Rust features outside the core.
