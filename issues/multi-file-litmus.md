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

### 2026-11-09, fourth review of PR #10

Two more rules wait for examples that span crates. RFC 0002's decision 52 takes unchecked code to be sound crate by crate: a crate with `unsafe` code that the project has not named is treated as foreign code, which an example can show only with a second crate. And RFC 0001's decision 39 holds what other crates can reach, in the first stage, to export mode's interface rules. Neither has a spec rule yet.

### 2026-11-09, conversation with the owner about PR #24

The rule that waits for a second crate has changed with RFC 0002's decision 54: a function that calls into a crate with `unsafe` code that the project has not named is unverified, so it can state no proof. It still has no spec rule, for the same reason.

### 2026-11-22, two more rules that one file cannot show

RFC 0002's decision 94 has an interface record, for each type parameter of a generic function, how its values reach unverified code, so that a caller in another crate checks a loan at its own type without the body. An example of that needs two crates. `logical-layer/own-generic-lends-proof-holder.accept.lc` and `logical-layer/own-generic-lends-proof-cell.reject.lc` show the same check within one crate, where the body is at hand.

Decision 90 says what a function's result and the program's result list among the assumptions. An example states whether a program is accepted and what it returns, and has no way to state what a result lists, in one file or in several.

### 2026-11-22, the record of which crates contain `unsafe` code

RFC 0002's decision 95 has the driver record an unverified crate as one that contains `unsafe` code when it has a function that foreign code can call, even when the crate writes no `unsafe`. The case behind it takes two crates: one that defines a sealed type, and one that Locus does not verify, which puts `#[no_mangle]` on a function that takes that type. And what it changes is what is recorded and listed, which an example has no way to state. So it has no rule and no example, as decision 52's record has none.

### 2026-11-22, where a model is declared, and a type declared in plain Rust

Two clauses of RFC 0002's decisions on models need two files to show. `models.declared-by-owner` says that a model is declared in the `locus!` block or the `.lc` file that declares its type, so a model in another file of the type's own crate is rejected; an example is one `.lc` file, and shows only a model for a type of another crate. And `models.snapshots.seen` gives a struct declared in a plain `.rs` file no snapshot.

### 2026-11-23, an implementation of `Model` in plain Rust

One more rule has a part that one file cannot show. `compat.names.model-impls` says that only Locus writes an implementation of `Model`, and that one written in plain Rust is refused: in a `.rs` file, outside a `locus!` block, or in a crate that Locus does not lower, verified or not ([RFC 0001](../design/rfcs/0001-compiler-pipeline.md), decision 51). Its two examples show what a `.lc` file can: an `unsafe impl Model` written in Locus code, which is rejected, and a crate that forbids `unsafe` code and declares a model, which is accepted. The refusal in another file or crate, which is the driver's, has no example.

### 2026-11-23, three things in RFC 0006 that one file cannot show

[RFC 0006](../design/rfcs/0006-traits.md), a draft, states three things without a rule, in [the chapter on traits](../spec/traits.md#what-has-no-rule), because each needs a second crate or a second file:

- that a Locus trait is implemented only in crates that Locus verifies, and that an implementation of one in another crate of a build is an error;
- that the orphan rule applies to a logical type as to any type: a crate implements its own trait for a logical type of Locus's library, and the library's trait for its own logical type, and not the library's trait for the library's type. One file shows the first two (`traits/two-logical-types-one-trait.accept.lc`, `traits/operator-on-own-logical-type.accept.lc`);
- that a trait of another crate is read as its crate declared it: which members are logic functions, and which methods run. The rule that a logical type implements no trait with a method that runs is judged after rustc for this reason, and its example has the trait in the same file (`traits/provided-method-on-logical-type.reject.lc`).

The fourth draft of the RFC had the first as a rule, `traits.impls.checked`. It becomes one when an example can pin it.
