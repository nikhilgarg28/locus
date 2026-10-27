# Configurations: corner cases of cfg and compiler flags

- **Kind:** decision
- **Status:** open
- **Blocks:** how configurations are declared ([#7](distribution-and-export.md)), whether the guard is sound and usable on real crates, and whether build mode checks every crate that uses a Locus crate

## Question

RFC 0001 (`design/rfcs/0001-compiler-pipeline.md`, section *Configurations*, proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10)) treats each build configuration as a different program, and a proof as holding only in the configurations it was checked in. In outline:

- **In build mode,** Locus checks exactly the configuration being built, and builds what it checked. So most of the cases below arise only in export mode.
- **In export mode,** a crate declares the configurations it is checked in. By default these are dev, release and test builds, with default features and 64-bit pointers. The default does not depend on the machine that runs the check.
- **Erasure removes code that no checked configuration enables.** In export mode, it drops a `cfg` attribute that every checked configuration satisfies, and keeps the others as written.
- **A guard in an exported crate's generated module** turns every unchecked configuration into a `compile_error!`. It tests every `cfg` name that appears in checked code, plus `target_pointer_width` and `debug_assertions`.
- **The runtime program is validated against the checking program** in both modes, by comparing their items and typed bodies.
- **What neither the guard nor validation can cover becomes an assumption in the trust model.** For exported crates, that is the compiler, dependency versions and features, and the inputs of macros that read the environment or files when Rust users build them.

This issue collects the corner cases that outline does not settle. Each needs either a decision or a test showing it is already handled.

**Blocks:** how configurations are declared ([#7](distribution-and-export.md)), whether the guard is sound and usable on real crates, and whether build mode checks every crate that uses a Locus crate. It does not block RFC 0001, which records these as open.

## Corner cases

**Flags that ordinary tools add**
1. **`cfg(test)` and `--test`.** `cargo test --release` builds `test` without `debug_assertions`, which the export default does not check. Doc tests compile against the library without `cfg(test)`, and `cfg(doctest)` exists too.
2. **docs.rs and rustdoc.** These set `cfg(doc)` and `--cfg docsrs`, and many crates gate attributes on them, such as `#[cfg_attr(docsrs, feature(doc_cfg))]`. rustdoc expands `compile_error!`, so a guard that tests `docsrs` breaks documentation builds. Option: the guard ignores `cfg`s that gate only attributes unable to change checked code, such as docs, lint levels and feature gates.
3. **Tool `cfg`s:** `clippy`, `miri`, `rust_analyzer`. Running Clippy or Miri on a Locus crate must not hit the guard unless checked code tests these names.
4. **`-C panic=abort`** and `cfg(panic = "...")`. Proofs already say nothing after a panic, but `catch_unwind` and `Drop` behave differently under the two strategies.
5. **`-C target-cpu` and `-C target-feature`** (including `native`), which change `cfg(target_feature = ...)`. Is the default "the target's baseline features", and how is that stated without depending on the host?

**Settings that are not `cfg`s at all**
6. **`overflow-checks`**, set per profile independently of `debug_assertions`. `cfg(overflow_checks)` is unstable (E0658 on stable 1.97), so no guard can test it. The runtime semantics ([#6](runtime-semantics.md)) must make proofs hold either way. Are there other profile settings with the same property?
7. **`opt-level`, `lto`, `codegen-units`** should not change meaning. Confirm that nothing a proof relies on depends on them.

**Where `cfg` values come from**
8. **`RUSTFLAGS --cfg` and build scripts' `cargo:rustc-cfg`.** Build scripts that probe the compiler or the environment (for example with `autocfg`) can set different `cfg`s on the machine that checks than on the machine that builds.
9. **Compiler-version detection** (`rustversion`, build-script probes). This selects code by the compiler that builds an exported program, which ties into the trust assumption about that compiler.
10. **`check-cfg`.** The guard must not trigger `unexpected_cfgs` warnings that the user's own code would not. Using only names that appear in checked code should ensure this, but it needs a test, including for the names the guard always tests.

**Target properties**
11. **Other target `cfg`s in checked code** (`target_os`, `unix`, `windows`, `target_arch`, `target_endian`, `target_has_atomic`). The machine-independent default cannot pick their values. Options:
    - (a) a fixed reference target;
    - (b) require declared configurations whenever checked code tests these names (RFC 0001 does this for now);
    - (c) the host's values, which makes the default depend on the host.
12. **Checking a target other than the host** needs that target's standard library installed, even without code generation. A 32-bit host checking the default 64-bit configuration hits this. Early fixtures for cross-compilation and for host-versus-target builds are pinned in [#18](external-review-ideas.md), item 6, with `no_std`, `alloc`, and `#[no_implicit_prelude]`.
13. **32-bit and 16-bit targets:** what declaring them looks like, and how proofs about `usize` arithmetic carry across pointer widths.

**Features and dependencies**
14. **Cargo feature unification.** A downstream build can enable dependency features that the check did not. This changes the dependencies' code, their types (fields, trait implementations) and their `cfg`s, and no guard in this crate can test it. Options:
    - (a) an assumption in the trust model (RFC 0001 does this for now);
    - (b) record the dependency features seen at check time in the package metadata and have a Locus downstream check them;
    - (c) restrict proofs to facts about dependencies that hold under every feature set.
15. **The number of feature combinations.** Checking all 2ⁿ is infeasible. Is there a useful default beyond default features, such as defaults, no defaults and all features, as `cargo hack` does?
16. **`cfg`s inside other crates' macros.** A `macro_rules!` from a dependency can expand to `#[cfg(feature = "x")]`, which is then evaluated against *this* crate's features, and that name does not appear in this crate's source. Does the guard need the names after expansion, from the driver, rather than from the source?

**Interactions with erasure, the guard, and validation**
17. **`cfg!()` in expressions.** Both branches are compiled and type-checked, but the value is fixed per configuration. The runtime semantics must know it, and erasure keeps it.
18. **`cfg_attr`** on types that carry proofs or logical fields, for example `#[cfg_attr(feature = "serde", derive(Serialize))]`. The derive sees logical fields as markers: `ErasedData` implements only `Clone`, `Copy`, and `Debug`, so serde's derive fails on a logical data field unless told to skip it.
19. **Removing code no checked configuration enables.** This changes what an unchecked configuration would see. The guard fires there anyway, but rustc may also report errors caused by the removed code, which makes the guard's message harder to find. Can the guard's error be made to come first, or alone?
20. **The guard's key set must come from the source, not the output.** Once erasure removes a branch, the output no longer mentions its `cfg` name.
21. **Doctests in build mode.** rustdoc compiles doctests outside Cargo's `RUSTC_WRAPPER`, so build mode's rule that every crate depending on a Locus crate is checked does not reach them. Options: run rustdoc's compilations through Locus's driver, or do not run the doctests of Locus crates.
22. **Validating an exported program in every declared configuration.** Validation compiles the exported program and compares its typed bodies with the checked ones, so export mode does it once per declared configuration. Is that cost acceptable, or can configurations that differ only in code no proof relies on share one validation?

## What would help

- `.open` litmus examples for the cases that are about meaning: 6, 11, 14 and 17.
- A small test crate per case for the mechanical ones: 1–3, 8, 10, 19–22.

## Log

### 2026-10-25, opened on GitHub as #9

Moved into the repository from [GitHub issue #9](https://github.com/nikhilgarg28/locus/issues/9) on 2026-10-27. The text above is the issue's as it was then.
