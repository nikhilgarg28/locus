# Spike: facts from rustc

A throwaway experiment for [RFC 0001](../../design/rfcs/0001-compiler-pipeline.md). Nothing outside `spikes/` may depend on it.

## Question

Can Locus hand a crate to rustc as a *checking program*, in which logical types are zero-sized placeholders and proofs become borrows of the places they read, and get back what its logical pass needs?

1. The types of the places proofs observe, including fields of types from other crates.
2. The items that paths in logical code resolve to.
3. rustc's borrow checker rejecting a proof that observes a moved value.
4. The typed body of a function whose behavior a proof is about, with everything implicit made explicit.
5. All of this inside a real Cargo build with a dependency, without changing the crate's files.
6. On stable Rust, or only on nightly?

## Setup

- `sample/src/lib.lc` is a Locus source in provisional syntax. It has a logic function, a proof parameter, two proofs that observe places, and one proof that observes `v` after it has moved.
- `sample/check/lib.rs` is the checking program Locus would generate for it, written by hand. Each line matches the same line of `lib.lc`, and synthesized items follow at the end:
  - logical types become `__locus::Proof` and `__locus::Nat`;
  - the logic function becomes a stub;
  - each proof becomes `__locus::observe(&place)` for every place it reads, plus `__locus::resolve(path)` for every item it names.
- `sample/src/lib.rs` is the runtime program Locus would emit, also by hand. Cargo builds this file normally. Its `pub struct Erased` is a shortcut of the spike: a real runtime program keeps its proof stand-in private to the crate, as RFC 0001 requires.
- `sample/dep` is a path dependency providing `Meters`.
- `driver/` is a rustc driver, about 150 lines. Cargo runs it as `RUSTC_WORKSPACE_WRAPPER`.
  - For the target crate, it replaces the crate root `src/lib.rs` with `check/lib.rs`.
  - After macro expansion, it asks rustc's type checker about every `observe` and `resolve` call and writes the answers as JSON lines.
  - For one function, `first`, it also writes each expression of rustc's typed body (THIR) with its type.
  - It then lets compilation continue, so rustc's own checks, including the borrow checker, run as usual.
  - For every other crate, it behaves exactly like rustc. If the crate name does not match, it silently compiles the crate as it is, which a real driver must not do: RFC 0001 requires it to fail instead.

Run `./run.sh`. It needs `rustup component add rustc-dev llvm-tools --toolchain stable`.

## Results

With stable rustc 1.97.0, the stable release installed here, run on 2026-10-25:

```text
cargo check exit status: 101
facts:
  {"kind":"thir","expr":"VarRef","type":"&std::vec::Vec<u32>"}
  {"kind":"thir","expr":"Deref","type":"std::vec::Vec<u32>"}
  {"kind":"thir","expr":"Borrow","type":"&std::vec::Vec<u32>"}
  {"kind":"thir","expr":"Scope","type":"&std::vec::Vec<u32>"}
  {"kind":"thir","expr":"Literal","type":"usize"}
  {"kind":"thir","expr":"Scope","type":"usize"}
  {"kind":"thir","expr":"ZstLiteral","type":"for<'a> fn(&'a std::vec::Vec<u32>, usize) -> &'a <std::vec::Vec<u32> as std::ops::Index<usize>>::Output {<std::vec::Vec<u32> as std::ops::Index<usize>>::index}"}
  {"kind":"thir","expr":"Call","type":"&u32","detail":"callee=for<'a> fn(&'a std::vec::Vec<u32>, usize) -> &'a <std::vec::Vec<u32> as std::ops::Index<usize>>::Output {<std::vec::Vec<u32> as std::ops::Index<usize>>::index}"}
  {"kind":"thir","expr":"Deref","type":"u32"}
  {"kind":"thir","expr":"Scope","type":"u32"}
  {"kind":"thir","expr":"Block","type":"u32"}
  {"kind":"thir","expr":"Scope","type":"u32"}
  {"kind":"resolution","at":"check/lib.rs:12:41: 12:47","item":"sample_dep::Meters","def_kind":"Ctor(Struct, Fn)"}
  {"kind":"observation","at":"check/lib.rs:13:52: 13:54","type":"std::vec::Vec<u32>"}
  {"kind":"resolution","at":"check/lib.rs:13:74: 13:79","item":"total","def_kind":"Fn"}
  {"kind":"observation","at":"check/lib.rs:15:64: 15:68","type":"u32"}
  {"kind":"observation","at":"check/lib.rs:17:51: 17:53","type":"std::vec::Vec<u32>"}
diagnostics (level, code, primary span, message):
  warning unused_variables check/lib.rs:4:10 unused variable: `xs`
  warning unused_variables check/lib.rs:6:28 unused variable: `nonempty`
  error E0382 check/lib.rs:17:51 borrow of moved value: `v`
  warning unused_variables check/lib.rs:15:9 unused variable: `kept`
  warning unused_variables check/lib.rs:17:9 unused variable: `late`
  failure-note None - For more information about this error, try `rustc --explain E0382`.
```

The full run took 2.2 seconds of wall time, including building the driver from scratch: rustc's own crates come prebuilt in `rustc-dev`. `cargo +stable check` on the same crate without the wrapper builds the runtime program cleanly.

## Findings

1. **Types and resolutions: yes.**
   - The driver read the type of every observed place, including `m.0`, a field of a struct defined in the dependency.
   - Paths resolved to local items (`total`, the stubbed logic function) and to items in other crates (`sample_dep::Meters`).
2. **Ownership of proofs: yes, from rustc's own borrow checker.** The proof that observes `v` after `let moved = v;` was rejected with E0382 at line 17, the same line as in `lib.lc`. The observations before the move were accepted. Locus needed no permission checker of its own for this case.
3. **Typed bodies: yes.** For `v[0]` with `v: &Vec<u32>`, rustc's typed body (THIR) makes every implicit step explicit:
   - the dereference of `v` and the borrow passed on;
   - the literal `0` typed `usize`;
   - indexing resolved to the trait method `<Vec<u32> as Index<usize>>::index`, returning `&u32`, then dereferenced.

   A translation of runtime code into the logic would start from exactly this. This spike only prints the body; it does not translate it.
4. **Cargo integration: yes.**
   - The workspace wrapper received Cargo's exact invocation: edition, cfgs, `--extern` paths, output directories. It compiled the dependency unchanged.
   - Swapping the crate root was one argument substitution; the crate's files on disk were untouched.
   - v0 captured Cargo invocations through `RUSTC_WRAPPER` for the same reason.
5. **Stable: yes, through `RUSTC_BOOTSTRAP=1`.**
   - The driver built against stable 1.97's `rustc-dev` component and ran under stable Cargo.
   - `RUSTC_BOOTSTRAP` is needed only to build the driver, not when it runs, and the crate being checked never sees it.
   - The Rust project does not support `RUSTC_BOOTSTRAP` for this use. In practice it means the driver is tied to one exact rustc release, just as a nightly driver is tied to one nightly.
6. **API churn: small here, but real.**
   - The same source built against nightly 2026-10-20 (1.101), four releases later, after one change: `rustc_driver::run_compiler` is now `rustc_driver::compiler_entrypoint`.
   - Everything else this spike uses was unchanged: `after_expansion`, `hir_body_owners`, `typeck`, `qpath_res`, the HIR visitor, and `thir_body`.
   - A driver that translates whole THIR or MIR bodies would touch a much larger surface, which changes faster.
7. **The checking program produces spurious warnings.** rustc warned about unused names: a stub's parameter (`xs`), a proof parameter (`nonempty`), and two proof bindings (`kept`, `late`). All four spans are in copied text, so span position alone cannot tell spurious warnings from real ones. RFC 0001 therefore compiles the checking program with lints off and takes lints from the runtime program.
8. **Mapping is needed even when lines align.** Lines match between `lib.lc` and `check/lib.rs` here, but columns do not: `v` is at column 51 in the checking program and column 17 in the source. Real lowering needs an origin map from generated spans to source spans.

## Not tested

- Translating typed bodies: the spike prints one function's THIR, nothing more.
- Proc macros, build scripts, and `cfg` handling beyond what Cargo passes through.
- An in-process diagnostic emitter, as opposed to rustc's JSON output.
- Generic functions, traits, and closures in proofs.
- Real lowering from `.lc`: the checking program and the runtime program were written by hand.
