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

The current proof spelling is `@claim` / `@[condition]` in types and `_` for an automatic proof request. `@` never begins an expression. Hash-based proof forms are rejected with migration guidance; `#[...]` and `#![...]` remain reserved attribute syntax. In a pattern, `_` still means to ignore the matched value. The `@{ ... }` block is retired in favor of proofs written as ordinary expressions, and receives a migration diagnostic, as do `def`, `+`, and array syntax.

Bracket expressions remain neutral in the AST until typing supplies context. The parser also preserves explicit array forms and array/slice type syntax, but no array execution is implemented. The `!`, `&&`, `||`, and `=>` nodes likewise need semantic checking to distinguish operations on proposition values from executable Boolean operations.

`fn` declares executable functions with optional proof parameters and results; `math fn` declares pure, total functions usable in propositions. The AST preserves the distinction; checking purity, totality, and ghost-to-runtime data flow belongs to the elaborator, which is not yet implemented. The files in `examples/` follow the specification's grammar. `examples/lock.loc` is the end-to-end example the milestone is judged by. See [the grammar](grammar.md) for the implemented syntax, [the specification](core-language-spec.md) for its meaning, and [rust-features.md](rust-features.md) for the Rust features outside the core.
