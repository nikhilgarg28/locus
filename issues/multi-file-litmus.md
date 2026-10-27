# Litmus examples that span several files or crates

- **Kind:** task
- **Status:** later
- **Blocks:** spec rules for the claims this issue lists, which CI requires to have a decided example

Each litmus example is one `.lc` file, so nothing in `litmus/` can show a claim that involves more than one file or crate. Reviews of [#10](https://github.com/nikhilgarg28/locus/pull/10) found these claims with no example:

- `.rs` files may not name logical items (RFC 0001, still-for-the-owner item 1), which therefore has no spec rule yet;
- every crate that depends on a Locus crate is checked, including procedural macro crates, while doctests are refused;
- `pub(locus)` items are public to Locus crates in build mode and `pub(crate)` in export mode, as seen from another crate;
- dependents are checked against interfaces, which name a dependency's items by its identity and interface hash, so two versions of one crate stay apart;
- export mode's guard tests every `cfg` name in the crate's sources, `.rs` files included;
- build scripts and integration tests are checked as crates of their own;
- `#[path]` for modules beneath a `.lc` module.

**Question:** what format should an example spanning several files or crates have? For instance, a directory with a manifest, or one file with `//~ file:` sections. The tooling (`xtask/src/litmus.rs`, `pr-check`) and `litmus/README.md` would change with it.

**Blocks:** spec rules for the claims above, which CI requires to have a decided example.

## Log

### 2026-10-27, opened on GitHub as #19

Moved into the repository from [GitHub issue #19](https://github.com/nikhilgarg28/locus/issues/19) on 2026-10-27. The text above is the issue's as it was then.
