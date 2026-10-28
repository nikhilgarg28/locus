# Spike: build mode's Cargo graph

> The marker for logical data is called `ErasedData` here, as it was when this spike ran. The RFCs now call it `LogicalData`.

A throwaway experiment for [RFC 0001](../../design/rfcs/0001-compiler-pipeline.md), the compiler pipeline, whose acceptance waits for it. Nothing outside `spikes/` may depend on it.

## Question

In build mode, one Cargo run checks and builds every crate, with `locus-driver` as Cargo's compiler wrapper. For each crate Locus checks, one invocation of the driver compiles the checking program without code generation, hands its facts to a separate `locus` process for the logical pass, and only then compiles the runtime program as the artifact Cargo asked for. Both programs are one lowered text, compiled against two versions of Locus's library. Each checked crate writes an interface file that its dependents' logical passes read. Does this survive a real Cargo graph?

1. **Library versions across crates.** When `a`'s runtime artifact was built against the runtime library and `b`'s checking program is compiled against the checking library, do the markers unify? Compare (i) two crates of the same name, swapped through `--extern`; (ii) one marker crate shared by both versions; (iii) one procedural macro whose output depends on an environment variable. What breaks, for example a dependency's `macro_rules!` that calls a Locus macro through `$crate`?
2. **Two compiles in one invocation.** What does it cost in wall time, clean and incremental?
3. **Invalidation.** Does a change to `a`'s interface re-check `b` and `app`? Does a change to a body alone? Can dep-info make Cargo track `.lc` modules and interface files?
4. **No stale success.** Can Cargo ever reuse an artifact as if it had passed a check: after a plain `cargo build`, after a failed check? Does Cargo's fingerprint include the wrapper, and if not, what makes it safe?
5. **Kinds of crates.** What does the wrapper see for a build script, a procedural macro, an integration test, and `cargo test`'s harnesses? `RUSTC_WORKSPACE_WRAPPER` or `RUSTC_WRAPPER` for a Locus dependency outside the workspace?
6. **Semantic identities.** What can the driver put in facts to identify items by definition across crates?

## Setup

Spike syntax, in `.lc` files: a line `contract NAME: VERSION` states a contract, a line `uses DEP::NAME: VERSION` states a contract the crate relies on, `::__locus::prove!(places)` is a proof that observes places, and `::__locus::Erased` is a ghost marker. Lowering blanks the two kinds of line and keeps the rest, so line numbers do not move.

- `ws/` is a workspace of its own:
  - `a`, a Locus library: `src/lib.lc` declares `mod util;`, which resolves to `src/util.lc`. It has a contract, a proof, a function taking `Erased`, an inherent and a trait impl, and `macro_rules! nonzero`, which calls `$crate::__l::prove!` through `a`'s own re-export of Locus's library.
  - `b`, a Locus library depending on `a` and `pm`. It relies on `a::div: v1` and `a::double: v1`, and re-exports `a::nonzero`. Its build script calls `a::double` (a build-dependency) and sets `--cfg b_fast`, which gates a function that uses `pm::answer!()`.
  - `pm`, a procedural macro crate.
  - `app`, plain Rust: a binary depending on `b`, with a unit test, an integration test `tests/it.rs`, and an optional dependency on `outside`.
- `outside/` is a Locus library outside the workspace, standing in for a registry crate.
- `q4ws/` is a second workspace, `c` and `d`, whose Locus lines are written `//@ contract ...` so that plain rustc also builds them. `d`'s check fails: it relies on `c::f: v1`, and `c` states `f: v2`.
- `driver/` builds two programs:
  - `locus`, the fake: `locus lower` writes each `.lc` file as a `.rs` file under `target/locus/lower/<crate>-<hash>/`, and `locus check` is the logical pass. The pass fails when a `uses` line does not match a `contract` line in the dependency's interface file, and writes the crate's own interface, its `contract` lines, beside its artifact as `lib<crate>-<hash>.lcif`.
  - `locus-driver`, a rustc driver of about 600 lines. A crate is checked when its root is a `.lc` file or when one of its `--extern` crates has an interface file. For a checked crate, one invocation:
    1. runs `locus lower`;
    2. compiles the checking program: the lowered text, `--emit=metadata` into a side directory, `--cfg __locus_check`, `LOCUS_FLAVOR=check`, lints capped, the `artifacts` JSON notification removed, an incremental directory of its own; it writes facts after analysis;
    3. runs `locus check`;
    4. compiles the runtime program: Cargo's arguments, the lowered root, and Locus's library. It stops the compile, before code generation, unless the program defines the same items, by DefPathHash, as the checking program: a stand-in for validation;
    5. rewrites rustc's dep-info so that it lists the `.lc` files, the dependencies' interface files, Locus's library, and `locus`.

    Every other invocation is plain rustc. A crate that must be checked fails, rather than compiling unchecked, when the driver is not configured.
- `flavors/` holds Locus's library in four versions, and `a.rs` and `b.rs`, the lowered text of two libraries for the plain-rustc comparison in `flavors.sh`. Each `__locus` has `Erased`, `ErasedData`, and `prove!`; in the checking version `prove!(p)` expands to `{ observe(&p); Erased }`, in the runtime version to `Erased`.
  - (i) `i_check.rs` and `i_run.rs`: two crates named `__locus`, each with its own markers, built with different `-C metadata`.
  - (ii) `core.rs`, a marker crate `locus_core`, and `ii_check.rs` and `ii_run.rs`, two crates named `__locus` that re-export its markers and define their own `prove!`.
  - (iii) `iii_lib.rs` and `iii_macros.rs`: one `__locus` whose `prove!` calls a procedural macro, which reads `LOCUS_FLAVOR`.
  - (iv), not in the brief: `iv_lib.rs`, one `__locus` whose `prove!` expands to `{ #[cfg(__locus_check)] { observe(&p); } Erased }`. A `cfg` is evaluated in the crate where the macro is expanded, so the driver passes `--cfg __locus_check` to checking compiles only, and `--check-cfg cfg(__locus_check)` to runtime compiles. The driver uses (iv) unless told otherwise.

Run `./run.sh`, or `./run.sh q3` for one question. It needs `rustup component add rustc-dev llvm-tools --toolchain stable`, and uses a nightly toolchain's Cargo for parts of questions 4 and 5 if one is installed. It works on fresh copies in a temporary directory and changes nothing here.

## Results

Stable rustc 1.97.0 and Cargo 1.97.0, the release installed here, and nightly Cargo 1.101.0-nightly (cad07cf8e 2026-10-23) with stable rustc, on 2026-10-26, in a 4-core Linux container. `./run.sh` ran in 1 minute 13 seconds and exited 0. In its output, `$WORK` is its temporary directory, `HASH` stands for Cargo's hashes, `cargo:` lines are the lines of Cargo's output that `run.sh` keeps (status, errors, warnings, and test results), and `driver:` lines are the driver's log. Each excerpt below is copied from that run; where lines were left out, the text says which.

### 1. Library versions across crates

With plain rustc (`flavors/flavors.sh`): `a` is built as a runtime rlib and as checking metadata, then `b`'s checking program is compiled against each. Lines 7 to 9 of `b.rs` pass markers to and from `a`. Lines 13 to 15 observe `v` after it has moved, and must be rejected in the checking program: line 13 calls `::__locus::prove!` directly, line 14 calls `a::assert_dollar!`, which expands to `$crate::__l::prove!`, and line 15 calls `a::assert_abs!`, which expands to `::__locus::prove!`.

```text
(i) two flavor crates named __locus, different -C metadata
  flavor check                                         exit 0
  flavor run                                           exit 0
  a, runtime program (rlib)                            exit 0
  a, checking program (rmeta, side directory)          exit 0
  b, checking program against a's runtime rlib         exit 1
      b.rs:7:41: error[E0308]: mismatched types: expected `__locus::Erased`, found `__l::Erased`
      b.rs:8:33: error[E0308]: mismatched types: expected `__l::ErasedData`, found `__locus::ErasedData`
      b.rs:9:18: error[E0308]: mismatched types: expected `__l::Erased`, found `__locus::Erased`
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, checking program against a's checking rmeta       exit 1
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:14:57: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, runtime program against a's runtime rlib          exit 0
(i') the same, but both flavors built with the same -C metadata
  flavor check                                         exit 0
  flavor run                                           exit 0
  a, runtime program (rlib)                            exit 0
  a, checking program (rmeta, side directory)          exit 0
  b, checking program against a's runtime rlib         exit 1
      b.rs:14:57: error[E0463]: can't find crate for `a`: can't find crate
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, checking program against a's checking rmeta       exit 1
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:14:57: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, runtime program against a's runtime rlib          exit 0
(ii) one marker crate, re-exported by two flavor crates named __locus
  marker crate locus_core                              exit 0
  flavor check                                         exit 0
  flavor run                                           exit 0
  a, runtime program (rlib)                            exit 0
  a, checking program (rmeta, side directory)          exit 0
  b, checking program against a's runtime rlib         exit 1
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, checking program against a's checking rmeta       exit 1
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:14:57: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, runtime program against a's runtime rlib          exit 0
(iii) one __locus crate; a procedural macro reads LOCUS_FLAVOR
  proc macro locus_macros                              exit 0
  __locus                                              exit 0
  a, runtime program (rlib)                            exit 0
  a, checking program (rmeta, side directory)          exit 0
  b, checking program against a's runtime rlib         exit 1
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:14:57: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, checking program against a's checking rmeta       exit 1
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:14:57: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, runtime program against a's runtime rlib          exit 0
(iv) one __locus crate; prove! expands to code under cfg(__locus_check)
  __locus                                              exit 0
  a, runtime program (rlib)                            exit 0
      a.rs:6:10: warning: unexpected `cfg` condition name: `__locus_check`
  a, checking program (rmeta, side directory)          exit 0
  b, checking program against a's runtime rlib         exit 1
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:14:57: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, checking program against a's checking rmeta       exit 1
      b.rs:13:53: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:14:57: error[E0382]: borrow of moved value: `v`: value borrowed here after move
      b.rs:15:54: error[E0382]: borrow of moved value: `v`: value borrowed here after move
  b, runtime program against a's runtime rlib          exit 0
      b.rs:6:13: warning: unexpected `cfg` condition name: `__locus_check`
      b.rs:13:53: warning: unexpected `cfg` condition name: `__locus_check`
      b.rs:14:57: warning: unexpected `cfg` condition name: `__locus_check`
      b.rs:15:54: warning: unexpected `cfg` condition name: `__locus_check`
```

The `unexpected cfg` warnings of (iv) appear because `flavors.sh` does not pass `--check-cfg`; the driver does.

Through Cargo, `ws` was built with each option, and then `fn moved() { let v = vec![1]; drop(v); let _ = b::nonzero!(v); }` was added to `app`. `b::nonzero!` is `a`'s macro, which calls `$crate::__l::prove!`. A summary of the run's output for each option:

| Option | `cargo build` | observations in `app` | `app` with `moved` |
|---|---|---|---|
| (i) | exit 101: E0308 in `b`'s checking program at `lib.rs:10:18` (`a::div(n, 2, h)`) and at `19:6` and `19:20` (the calls of `a::witness()`) | not reached | not reached |
| (i), checking programs compiled against dependencies' checking metadata (`LOCUS_SPIKE_REMAP`) | exit 0 | 1 | exit 101, E0382 at `app/src/main.rs:20:48` |
| (ii) | exit 0 | **0** | **exit 0** |
| (iii) | exit 0 | 1 | exit 101, E0382 at `app/src/main.rs:20:48` |
| (iv) | exit 0 | 1 | exit 101, E0382 at `app/src/main.rs:20:48` |

Option (ii), the last step in full:

```text
--- app observes a moved value through b::nonzero!, a macro of a's: cargo build
  cargo: Compiling app v0.1.0 ($WORK/q1-ii/ws/app)
  cargo: warning: function `moved` is never used
  cargo: --> app/src/main.rs:20:4
  exit status 0
  driver: bin app-HASH: start, 1 dependency interfaces
  driver: locus check app: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: bin app-HASH: checked
  driver: runtime program app: stable_crate_id 50f95b187aba2425, svh 5f7fb56f3827d8f1167806d37471e6f4, 2 items, the same DefPathHashes as the checking program
  driver: bin app-HASH: lower 2ms, checking program 54ms, logical pass 3ms, runtime program 100ms
```

Every successful build of `app` printed `half 5 quadruple 12 answer 42`.

### 2. Two compiles in one invocation

Argument lists of `b`'s compiles, one argument per line, against Cargo's:

```text
  diff: Cargo's arguments for b, and the runtime program's
    4c4
    < b/src/lib.lc
    ---
    > $WORK/q2/ws/target/locus/lower/b-HASH/lib.rs
    35a36,41
    > --extern
    > __locus=$WORK/lib-iv/lib__locus.rlib
    > -L
    > dependency=$WORK/lib-iv
    > --check-cfg
    > cfg(__locus_check)
  diff: Cargo's arguments for b, and the checking program's
    4c4
    < b/src/lib.lc
    ---
    > $WORK/q2/ws/target/locus/lower/b-HASH/lib.rs
    6c6
    < --json=diagnostic-rendered-ansi,artifacts,future-incompat
    ---
    > --json=diagnostic-rendered-ansi
    9c9
    < --emit=dep-info,metadata,link
    ---
    > --emit=metadata
    23c23
    < $WORK/q2/ws/target/debug/deps
    ---
    > $WORK/q2/ws/target/locus/check
    25c25
    < incremental=$WORK/q2/ws/target/debug/incremental
    ---
    > incremental=$WORK/q2/ws/target/locus/incremental/b-HASH
    35a36,43
    > --cap-lints
    > allow
    > --cfg
    > __locus_check
    > --extern
    > __locus=$WORK/lib-iv/lib__locus.rlib
    > -L
    > dependency=$WORK/lib-iv
```

Per crate, in that clean build of `ws`:

```text
  driver: lib a-HASH: lower 7ms, checking program 49ms, logical pass 3ms, runtime program 80ms
  driver: build-script build_script_build-HASH: lower 1ms, checking program 53ms, logical pass 3ms, runtime program 110ms
  driver: lib b-HASH: lower 5ms, checking program 34ms, logical pass 2ms, runtime program 39ms
  driver: bin app-HASH: lower 1ms, checking program 32ms, logical pass 2ms, runtime program 112ms
```

Wall time of `cargo build`. "Without the check" is the same driver with steps 2 and 3 skipped (`LOCUS_SPIKE_SKIP_CHECK`): it still lowers `.lc` crates, and, since no interface files are written, compiles `app` and `b`'s build script as plain rustc. "a's body changed" alternates a proof between `prove!(d)` and `prove!(d, n)`, so Cargo rebuilds `a`, `b`'s build script, `b`, and `app`; "app's body changed" alternates one literal in `app`'s `main`. Each mode starts with one clean build that is not counted.

```text
== Q2: wall time of cargo build, small workspace (milliseconds; three runs each)
  with the check   : clean 765 707 733; a's body changed 587 601 638; app's body changed 217 195 206
  without the check: clean 337 415 388; a's body changed 149 179 159; app's body changed 83 81 84
== Q2: wall time of cargo build, a with 2000 more functions, each with a proof (milliseconds)
  with the check   : clean 2677 2439 2315; a's body changed 1506 1544 1494; app's body changed 266 234 242
  without the check: clean 1140 1043 1122; a's body changed 570 575 643; app's body changed 111 120 141
```

With a logical pass that sleeps 1.5 s (`LOCUS_SPIKE_SLOW_PASS`), log lines stamped in milliseconds modulo 100 000; then the same with the checking program's artifact notification left in its `--json` (`LOCUS_SPIKE_KEEP_ARTIFACT_JSON`), in `ws` and in `q4ws`, where `d` depends only on `c`. Only the driver's `start`, summary, and failure lines are shown:

```text
--- clean build: cargo build
  cargo: Compiling a v0.1.0 ($WORK/q2p/ws/a)
  cargo: Compiling pm v0.1.0 ($WORK/q2p/ws/pm)
  cargo: Compiling b v0.1.0 ($WORK/q2p/ws/b)
  cargo: Compiling app v0.1.0 ($WORK/q2p/ws/app)
  exit status 0
  driver: [71341] lib a-HASH: start, 0 dependency interfaces
  driver: [72941] lib a-HASH: lower 4ms, checking program 40ms, logical pass 1503ms, runtime program 53ms
  driver: [72960] build-script build_script_build-HASH: start, 1 dependency interfaces
  driver: [74648] build-script build_script_build-HASH: lower 1ms, checking program 41ms, logical pass 1504ms, runtime program 142ms
  driver: [74680] lib b-HASH: start, 1 dependency interfaces
  driver: [76263] lib b-HASH: lower 4ms, checking program 42ms, logical pass 1503ms, runtime program 33ms
  driver: [76280] bin app-HASH: start, 1 dependency interfaces
  driver: [77938] bin app-HASH: lower 1ms, checking program 35ms, logical pass 1502ms, runtime program 119ms
--- clean build, the checking program's artifact notification passed to Cargo: cargo build
  cargo: Compiling a v0.1.0 ($WORK/q2p/ws/a)
  cargo: Compiling pm v0.1.0 ($WORK/q2p/ws/pm)
  cargo: thread 'main' (6491) panicked at src/tools/cargo/src/cargo/util/dependency_queue.rs:191:13:
  cargo: assertion failed: edges.remove(&key)
  exit status 101
  driver: [78102] lib a-HASH: start, 0 dependency interfaces
  driver: [79758] lib a-HASH: lower 4ms, checking program 70ms, logical pass 1503ms, runtime program 79ms
--- clean build, the checking program's artifact notification passed to Cargo: cargo build
  cargo: Compiling c v0.1.0 ($WORK/q2p4/q4ws/c)
  cargo: Compiling d v0.1.0 ($WORK/q2p4/q4ws/d)
  cargo: error: extern location for c does not exist: $WORK/q2p4/q4ws/target/debug/deps/libc-HASH.rmeta
  cargo: --> $WORK/q2p4/q4ws/target/locus/lower/d-HASH/lib.rs:3:5
  cargo: warning: build failed, waiting for other jobs to finish...
  cargo: thread 'main' (6604) panicked at src/tools/cargo/src/cargo/util/dependency_queue.rs:190:56:
  exit status 101
  driver: [79989] lib c-HASH: start, 0 dependency interfaces
  driver: [80020] lib d-HASH: start, 0 dependency interfaces
  driver: [80043] lib d-HASH: checking program FAILED to compile
  driver: [81541] lib c-HASH: lower 5ms, checking program 17ms, logical pass 1503ms, runtime program 26ms
```

### 3. Invalidation

All with `cargo build -v`. Cargo's `Compiling` lines, the lines for `pm`, which is always `Fresh`, and the driver's lines other than its checks and `b`'s are left out:

```text
--- clean build: cargo build -v
  exit status 0
  driver: locus check build_script_build: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: ok
  driver: locus check app: 1 observations, 1 dependency interfaces, 0 expectations: ok
--- no change: cargo build -v
  cargo: Fresh a v0.1.0 ($WORK/q3/ws/a)
  cargo: Fresh b v0.1.0 ($WORK/q3/ws/b)
  cargo: Fresh app v0.1.0 ($WORK/q3/ws/app)
  exit status 0
--- a's contract changed: util.lc says double: v2: cargo build -v
  cargo: Dirty a v0.1.0 ($WORK/q3/ws/a): the file `a/src/util.lc` has changed
  cargo: Dirty b v0.1.0 ($WORK/q3/ws/b): the dependency `a` was rebuilt
  cargo: locus: error: `b` relies on `a::double: v1`, but the interface of `a` states ["double: v2"]
  exit status 101
  driver: locus check build_script_build: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: sees lib b-HASH: root lib.lc, crate-type lib, --test no, emit dep-info,metadata,link, cfg ["b_fast"], features [], externs ["a", "pm"], cap-lints -, CARGO_PRIMARY_PACKAGE set, no rlib on disk yet
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: FAILED
  driver: lib b-HASH: logical pass FAILED; runtime program not compiled
  after the failed check, in target/debug: app deps/libb-HASH.lcif deps/libb-HASH.rmeta 
--- no change, after the failed check: cargo build -v
  cargo: Fresh a v0.1.0 ($WORK/q3/ws/a)
  cargo: Dirty b v0.1.0 ($WORK/q3/ws/b): couldn't read metadata for file `target/debug/deps/libb-HASH.rlib`
  cargo: locus: error: `b` relies on `a::double: v1`, but the interface of `a` states ["double: v2"]
  exit status 101
  driver: sees lib b-HASH: root lib.lc, crate-type lib, --test no, emit dep-info,metadata,link, cfg ["b_fast"], features [], externs ["a", "pm"], cap-lints -, CARGO_PRIMARY_PACKAGE set, no rlib on disk yet
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: FAILED
  driver: lib b-HASH: logical pass FAILED; runtime program not compiled
--- b now relies on double: v2: cargo build -v
  cargo: Fresh a v0.1.0 ($WORK/q3/ws/a)
  cargo: Dirty b v0.1.0 ($WORK/q3/ws/b): the precalculated components changed
  cargo: Dirty app v0.1.0 ($WORK/q3/ws/app): info of dependency `b` changed
  exit status 0
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: ok
  driver: locus check app: 1 observations, 1 dependency interfaces, 0 expectations: ok
--- only a proof in a's body changed: cargo build -v
  cargo: Dirty a v0.1.0 ($WORK/q3/ws/a): the file `a/src/lib.lc` has changed
  cargo: Dirty b v0.1.0 ($WORK/q3/ws/b): the dependency `a` was rebuilt
  cargo: Dirty app v0.1.0 ($WORK/q3/ws/app): the dependency `b` was rebuilt
  exit status 0
  driver: locus check build_script_build: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: ok
  driver: locus check app: 1 observations, 1 dependency interfaces, 0 expectations: ok
  a's interface file was not rewritten (same mtime)
--- util.lc's body changed: cargo build -v
  cargo: Dirty a v0.1.0 ($WORK/q3/ws/a): the file `a/src/util.lc` has changed
  cargo: Dirty b v0.1.0 ($WORK/q3/ws/b): the dependency `a` was rebuilt
  cargo: Dirty app v0.1.0 ($WORK/q3/ws/app): the dependency `b` was rebuilt
  exit status 0
  driver: locus check build_script_build: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: ok
  driver: locus check app: 1 observations, 1 dependency interfaces, 0 expectations: ok
--- a's interface files changed, and nothing else: cargo build -v
  cargo: Fresh a v0.1.0 ($WORK/q3/ws/a)
  cargo: Dirty b v0.1.0 ($WORK/q3/ws/b): the file `target/debug/deps/liba-HASH.lcif` has changed
  cargo: Dirty app v0.1.0 ($WORK/q3/ws/app): the dependency `b` was rebuilt
  exit status 0
  driver: locus check build_script_build: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: ok
  driver: locus check app: 1 observations, 1 dependency interfaces, 0 expectations: ok
```

The dep-info files, as the driver rewrote them, after the step "util.lc's body changed", without their blank lines:

```text
  a's dep-info, as rewritten:
    $WORK/q3/ws/target/debug/deps/a-HASH.d: $WORK/q3/ws/a/src/lib.lc $WORK/q3/ws/a/src/util.lc $WORK/lib-iv/lib__locus.rlib $WORK/driver/debug/locus
    $WORK/q3/ws/target/debug/deps/liba-HASH.rlib: $WORK/q3/ws/a/src/lib.lc $WORK/q3/ws/a/src/util.lc
    $WORK/q3/ws/target/debug/deps/liba-HASH.rmeta: $WORK/q3/ws/a/src/lib.lc $WORK/q3/ws/a/src/util.lc
    $WORK/q3/ws/a/src/lib.lc:
    $WORK/q3/ws/a/src/util.lc:
    $WORK/lib-iv/lib__locus.rlib:
    $WORK/driver/debug/locus:
  b's dep-info, as rewritten:
    $WORK/q3/ws/target/debug/deps/b-HASH.d: $WORK/q3/ws/b/src/lib.lc $WORK/q3/ws/target/debug/deps/liba-HASH.lcif $WORK/lib-iv/lib__locus.rlib $WORK/driver/debug/locus
    $WORK/q3/ws/target/debug/deps/libb-HASH.rlib: $WORK/q3/ws/b/src/lib.lc
    $WORK/q3/ws/target/debug/deps/libb-HASH.rmeta: $WORK/q3/ws/b/src/lib.lc
    $WORK/q3/ws/b/src/lib.lc:
    $WORK/q3/ws/target/debug/deps/liba-HASH.lcif:
    $WORK/lib-iv/lib__locus.rlib:
    $WORK/driver/debug/locus:
```

The same workspace with rustc's own dep-info, not rewritten (`LOCUS_SPIKE_RAW_DEPINFO`); only Cargo's `Dirty` and `Fresh` lines for `a`, `b`, and `app`, and the first line of `a`'s dep-info, are shown:

```text
== Q3: the same with rustc's own dep-info, not rewritten
--- clean build: cargo build -v
  exit status 0
  a's dep-info, as rustc wrote it:
    $WORK/q3raw/ws/target/debug/deps/a-HASH.d: $WORK/q3raw/ws/target/locus/lower/a-HASH/lib.rs $WORK/q3raw/ws/target/locus/lower/a-HASH/util.rs
--- no change: cargo build -v
  cargo: Dirty a v0.1.0 ($WORK/q3raw/ws/a): the file `target/locus/lower/a-HASH/lib.rs` has changed
  cargo: Dirty b v0.1.0 ($WORK/q3raw/ws/b): the dependency `a` was rebuilt
  cargo: Dirty app v0.1.0 ($WORK/q3raw/ws/app): the dependency `b` was rebuilt
  exit status 0
--- no change: cargo build -v
  cargo: Fresh a v0.1.0 ($WORK/q3raw/ws/a)
  cargo: Fresh b v0.1.0 ($WORK/q3raw/ws/b)
  cargo: Fresh app v0.1.0 ($WORK/q3raw/ws/app)
  exit status 0
--- a's contract changed: util.lc says double: v2: cargo build -v
  cargo: Fresh a v0.1.0 ($WORK/q3raw/ws/a)
  cargo: Fresh b v0.1.0 ($WORK/q3raw/ws/b)
  cargo: Fresh app v0.1.0 ($WORK/q3raw/ws/app)
  exit status 0
```

### 4. No stale success

In `q4ws`, each case runs plain `cargo build -v`, build mode, plain again, and build mode again, on a fresh copy. Plain rustc builds `c` and `d`; build mode must fail on `d`. The case that fails, in full:

```text
=== RUSTC_WRAPPER
--- plain: cargo build -v
  cargo: Compiling c v0.1.0 ($WORK/q4-rw/q4ws/c)
  cargo: Compiling d v0.1.0 ($WORK/q4-rw/q4ws/d)
  exit status 0
--- build mode: cargo build -v
  cargo: Fresh c v0.1.0 ($WORK/q4-rw/q4ws/c)
  cargo: Fresh d v0.1.0 ($WORK/q4-rw/q4ws/d)
  exit status 0
--- plain again: cargo build -v
  cargo: Fresh c v0.1.0 ($WORK/q4-rw/q4ws/c)
  cargo: Fresh d v0.1.0 ($WORK/q4-rw/q4ws/d)
  exit status 0
--- build mode again: cargo build -v
  cargo: Fresh c v0.1.0 ($WORK/q4-rw/q4ws/c)
  cargo: Fresh d v0.1.0 ($WORK/q4-rw/q4ws/d)
  exit status 0
  libraries in target: libc-62f20815b6793fe8.rlib libd-95048b273ed8193f.rlib
```

A summary of the run's output for every case:

| Case | build mode after plain | plain again | build mode again |
|---|---|---|---|
| `RUSTC_WORKSPACE_WRAPPER` | `c`, `d` compiled; `d`'s check fails, exit 101 | Fresh, exit 0 | `c` Fresh, `d` checked again, exit 101 |
| `RUSTC_WRAPPER` | **`c`, `d` Fresh, exit 0**: no check ran | Fresh, exit 0 | **Fresh, exit 0** |
| `RUSTC_WRAPPER`, and the driver adds a line to `rustc -vV` | as `RUSTC_WORKSPACE_WRAPPER` | Fresh, exit 0 | `c` Fresh, exit 101 |
| `RUSTC_WRAPPER`, `RUSTFLAGS=--cfg=__locus_build` | as `RUSTC_WORKSPACE_WRAPPER` | Fresh, exit 0 | `c` Fresh, exit 101 |
| `RUSTC_WRAPPER`, `__CARGO_DEFAULT_LIB_METADATA=locus` | as `RUSTC_WORKSPACE_WRAPPER` | Fresh, exit 0 | `c` Fresh, exit 101 |
| `RUSTC_WRAPPER`, a `CARGO_TARGET_DIR` of its own | as `RUSTC_WORKSPACE_WRAPPER` | not run | `c` Fresh, exit 101 |
| nightly Cargo, `RUSTC_WORKSPACE_WRAPPER` | as with stable Cargo | Fresh, exit 0 | `c` Fresh, exit 101 |
| nightly Cargo, `RUSTC_WRAPPER` | **Fresh, exit 0**: no check ran | Fresh, exit 0 | **Fresh, exit 0** |

The libraries on disk after each case, and `c`'s StableCrateId in build mode, which rustc derives from Cargo's `-C metadata`:

```text
=== RUSTC_WORKSPACE_WRAPPER
  libraries in target: libc-553a86f57af79731.rlib libc-62f20815b6793fe8.rlib libd-95048b273ed8193f.rlib 
=== RUSTC_WRAPPER
  libraries in target: libc-62f20815b6793fe8.rlib libd-95048b273ed8193f.rlib 
=== RUSTC_WRAPPER, and the driver adds a line to rustc -vV
  libraries in target: libc-3c7a5f96f8132c7c.rlib libc-62f20815b6793fe8.rlib libd-95048b273ed8193f.rlib 
=== RUSTC_WRAPPER, and the line added to rustc -vV changes (a copy of the driver)
=== RUSTC_WORKSPACE_WRAPPER: the wrapper's path and the wrapper's file
=== RUSTC_WRAPPER, and RUSTFLAGS=--cfg=__locus_build
  libraries in target: libc-62f20815b6793fe8.rlib libc-7372be51bd65e5b4.rlib libd-95048b273ed8193f.rlib 
=== RUSTC_WRAPPER, and __CARGO_DEFAULT_LIB_METADATA=locus (Cargo-internal)
  libraries in target: libc-0f8467aeeb004b48.rlib libc-62f20815b6793fe8.rlib libd-95048b273ed8193f.rlib 
=== RUSTC_WRAPPER, and a separate target directory
=== nightly Cargo (cargo 1.101.0-nightly (cad07cf8e 2026-10-23)), stable rustc
=== nightly Cargo: RUSTC_WORKSPACE_WRAPPER
  libraries in target: libc-553a86f57af79731.rlib libc-62f20815b6793fe8.rlib libd-95048b273ed8193f.rlib 
=== nightly Cargo: RUSTC_WRAPPER
  libraries in target: libc-62f20815b6793fe8.rlib libd-95048b273ed8193f.rlib
```

```text
=== RUSTC_WORKSPACE_WRAPPER
  driver: runtime program c: stable_crate_id a80c56f814b1a3a1, svh 9556017cfdabe929c2b4a74a9a2e8c01, 1 items, the same DefPathHashes as the checking program
=== RUSTC_WRAPPER
=== RUSTC_WRAPPER, and the driver adds a line to rustc -vV
  driver: runtime program c: stable_crate_id 4b721beffae8f71b, svh fdc6b16d8894e0e2ae85e4203af9c8c2, 1 items, the same DefPathHashes as the checking program
=== RUSTC_WRAPPER, and the line added to rustc -vV changes (a copy of the driver)
  driver: runtime program c: stable_crate_id 4b721beffae8f71b, svh 0862c026441f170938bfb091d17e30a0, 1 items, the same DefPathHashes as the checking program
  driver: runtime program c: stable_crate_id 935c503031e3bc43, svh af9d448932b2fcdaf825d9c717845c19, 1 items, the same DefPathHashes as the checking program
=== RUSTC_WORKSPACE_WRAPPER: the wrapper's path and the wrapper's file
  driver: runtime program c: stable_crate_id a80c56f814b1a3a1, svh 1e0794633636e8d18db3e1f266d3669c, 1 items, the same DefPathHashes as the checking program
  driver: runtime program c: stable_crate_id da9ef823c6d0d07d, svh 4134c99b8b1bc5701a7684a9217743f8, 1 items, the same DefPathHashes as the checking program
  driver: runtime program c: stable_crate_id d4aa7239599d8416, svh 7aa79896a7f0fe86419e5b8da1f2d6b4, 1 items, the same DefPathHashes as the checking program
=== RUSTC_WRAPPER, and RUSTFLAGS=--cfg=__locus_build
  driver: runtime program c: stable_crate_id 5e4d5f3d8bbbe85c, svh 6d4fd573e65d4e66e07c897e62edf13e, 1 items, the same DefPathHashes as the checking program
=== RUSTC_WRAPPER, and __CARGO_DEFAULT_LIB_METADATA=locus (Cargo-internal)
  driver: runtime program c: stable_crate_id 896bbad00286d281, svh 0a50e6be2875b340b6f6f447a9ad76e8, 1 items, the same DefPathHashes as the checking program
=== RUSTC_WRAPPER, and a separate target directory
  driver: runtime program c: stable_crate_id 5e4d5f3d8bbbe85c, svh 61edcb82f01cad76b180599441e82668, 1 items, the same DefPathHashes as the checking program
=== nightly Cargo (cargo 1.101.0-nightly (cad07cf8e 2026-10-23)), stable rustc
=== nightly Cargo: RUSTC_WORKSPACE_WRAPPER
  driver: runtime program c: stable_crate_id a80c56f814b1a3a1, svh cfbabe8ed6dd6c4bb06f23566aa77058, 1 items, the same DefPathHashes as the checking program
=== nightly Cargo: RUSTC_WRAPPER
```

What Cargo tracks of the wrapper, with copies of the driver so that their files can change. Only the lines for `c`, and exit statuses, are shown; `d` failed its check every time:

```text
=== RUSTC_WRAPPER, and the line added to rustc -vV changes (a copy of the driver)
--- build mode, stamp spike-1: cargo build -v
  cargo: Compiling c v0.1.0 ($WORK/q4-rw-stamp2/q4ws/c)
  exit status 101
  driver: runtime program c: stable_crate_id 4b721beffae8f71b, svh 0862c026441f170938bfb091d17e30a0, 1 items, the same DefPathHashes as the checking program
--- build mode, stamp spike-2: cargo build -v
  cargo: Fresh c v0.1.0 ($WORK/q4-rw-stamp2/q4ws/c)
  exit status 101
--- build mode, stamp spike-2, a byte appended to the driver: cargo build -v
  cargo: Compiling c v0.1.0 ($WORK/q4-rw-stamp2/q4ws/c)
  exit status 101
  driver: runtime program c: stable_crate_id 935c503031e3bc43, svh af9d448932b2fcdaf825d9c717845c19, 1 items, the same DefPathHashes as the checking program
--- build mode, stamp spike-2, another byte appended: cargo build -v
  cargo: Fresh c v0.1.0 ($WORK/q4-rw-stamp2/q4ws/c)
  exit status 101
=== RUSTC_WORKSPACE_WRAPPER: the wrapper's path and the wrapper's file
--- build mode: cargo build -v
  cargo: Compiling c v0.1.0 ($WORK/q4-wpath/q4ws/c)
  exit status 101
  driver: runtime program c: stable_crate_id a80c56f814b1a3a1, svh 1e0794633636e8d18db3e1f266d3669c, 1 items, the same DefPathHashes as the checking program
--- build mode, the same driver at another path: cargo build -v
  cargo: Compiling c v0.1.0 ($WORK/q4-wpath/q4ws/c)
  exit status 101
  driver: runtime program c: stable_crate_id da9ef823c6d0d07d, svh 4134c99b8b1bc5701a7684a9217743f8, 1 items, the same DefPathHashes as the checking program
--- build mode, a byte appended to that driver's file: cargo build -v
  cargo: Fresh c v0.1.0 ($WORK/q4-wpath/q4ws/c)
  exit status 101
--- build mode, another byte appended, and the driver adds a line to rustc -vV: cargo build -v
  cargo: Compiling c v0.1.0 ($WORK/q4-wpath/q4ws/c)
  exit status 101
  driver: runtime program c: stable_crate_id d4aa7239599d8416, svh 7aa79896a7f0fe86419e5b8da1f2d6b4, 1 items, the same DefPathHashes as the checking program
```

By hand, not in `run.sh`: after a build with `RUSTC_WRAPPER` and the line `locus-driver: spike-1`, `target/.rustc_info.json` held that line, and Cargo passed `d` `-C metadata=3e170fd0a72483a9`, where a plain build passes `-C metadata=2e7d9eb85b546c57`.

### 5. Kinds of crates

What the invocations of `cargo build` and `cargo test` told the driver (`LOCUS_SPIKE_SEEN`), the first line for each crate and kind; the last is from the `RUSTC_WRAPPER` build with `outside`:

```text
  driver: sees lib a-HASH: root lib.lc, crate-type lib, --test no, emit dep-info,metadata,link, cfg [], features [], externs [], cap-lints -, CARGO_PRIMARY_PACKAGE set, no rlib on disk yet
  driver: sees proc-macro pm-HASH: root lib.rs, crate-type proc-macro, --test no, emit dep-info,link, cfg [], features [], externs [], cap-lints -, CARGO_PRIMARY_PACKAGE set
  driver: sees build-script build_script_build-HASH: root b/build.rs, crate-type bin, --test no, emit dep-info,link, cfg [], features [], externs ["a"], cap-lints -, CARGO_PRIMARY_PACKAGE set
  driver: sees lib b-HASH: root lib.lc, crate-type lib, --test no, emit dep-info,metadata,link, cfg ["b_fast"], features [], externs ["a", "pm"], cap-lints -, CARGO_PRIMARY_PACKAGE set, no rlib on disk yet
  driver: sees bin app-HASH: root main.rs, crate-type bin, --test no, emit dep-info,link, cfg [], features [], externs ["b"], cap-lints -, CARGO_PRIMARY_PACKAGE set
  driver: sees test-harness a-HASH: root lib.lc, crate-type , --test yes, emit dep-info,link, cfg [], features [], externs [], cap-lints -, CARGO_PRIMARY_PACKAGE set
  driver: sees test-harness pm-HASH: root lib.rs, crate-type , --test yes, emit dep-info,link, cfg [], features [], externs [], cap-lints -, CARGO_PRIMARY_PACKAGE set
  driver: sees test-harness b-HASH: root lib.lc, crate-type , --test yes, emit dep-info,link, cfg ["b_fast"], features [], externs ["a", "pm"], cap-lints -, CARGO_PRIMARY_PACKAGE set
  driver: sees test-harness it-HASH: root app/tests/it.rs, crate-type , --test yes, emit dep-info,link, cfg [], features [], externs ["b"], cap-lints -, CARGO_PRIMARY_PACKAGE set
  driver: sees test-harness app-HASH: root main.rs, crate-type , --test yes, emit dep-info,link, cfg [], features [], externs ["b"], cap-lints -, CARGO_PRIMARY_PACKAGE set
  driver: sees lib outside-HASH: root lib.lc, crate-type lib, --test no, emit dep-info,metadata,link, cfg [], features [], externs [], cap-lints -, CARGO_PRIMARY_PACKAGE unset, no rlib on disk yet
```

The build script's `cfg` in `b`'s arguments, counted in Cargo's argument list and in the checking program's:

```text
  the build script's cfg in b's arguments (Cargo's, checking program's):
    $WORK/q5/ws/target/locus/args/b-HASH.cargo:1
    $WORK/q5/ws/target/locus/args/b-HASH.check:1
```

`cargo test -v`, then `cargo test --lib --bins --tests`, Cargo's lines only:

```text
--- test: cargo test -v
  cargo: Compiling pm v0.1.0 ($WORK/q5/ws/pm)
  cargo: Compiling a v0.1.0 ($WORK/q5/ws/a)
  cargo: Compiling b v0.1.0 ($WORK/q5/ws/b)
  cargo: Compiling app v0.1.0 ($WORK/q5/ws/app)
  cargo: test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished
  cargo: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished
  cargo: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished
  cargo: test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished
  cargo: test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished
  cargo: Doc-tests a
  cargo: error: expected one of `!` or `::`, found `div`
  cargo: --> a/src/lib.lc:6:10
  cargo: error: doctest failed, to rerun pass `-p a --doc`
  exit status 1
--- test, without doctests: cargo test --lib --bins --tests
  exit status 0
```

`app` with `--features outside`, a path dependency outside the workspace, with each wrapper:

```text
--- outside dependency, RUSTC_WORKSPACE_WRAPPER: cargo build --features outside
  cargo: error: expected one of `!` or `::`, found `triple`
  cargo: --> $WORK/q5/outside/src/lib.lc:2:10
  exit status 101
--- outside dependency, RUSTC_WRAPPER: cargo build --features outside
  exit status 0
  driver: locus check outside: 0 observations, 0 dependency interfaces, 0 expectations: ok
  driver: locus check a: 1 observations, 0 dependency interfaces, 0 expectations: ok
  driver: locus check build_script_build: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: ok
  driver: locus check app: 2 observations, 2 dependency interfaces, 0 expectations: ok
  app prints: half 5 quadruple 12 answer 42 triple 30
```

The same workspace with nightly Cargo, stable rustc, and `RUSTC_WRAPPER`; only the checks, failures, and interface files are shown:

```text
== Q5: the same workspace with nightly Cargo (cargo 1.101.0-nightly (cad07cf8e 2026-10-23)), stable rustc
--- build: cargo build --features outside
  exit status 0
  driver: locus check outside: 0 observations, 0 dependency interfaces, 0 expectations: ok
  driver: locus check a: 1 observations, 0 dependency interfaces, 0 expectations: ok
  driver: build-script build_script_build: start, 1 dependency interfaces
  driver: locus check build_script_build: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: ok
  driver: bin app: start, 2 dependency interfaces
  driver: locus check app: 2 observations, 2 dependency interfaces, 0 expectations: ok
  interface files: $WORK/q5n/ws/target/debug/build/a/bf0221db6abc0d14/out/liba-HASH.lcif $WORK/q5n/ws/target/debug/build/b/390c8484d72593fa/out/libb-HASH.lcif $WORK/q5n/ws/target/debug/build/outside/b1c8c07b930d9812/out/liboutside-HASH.lcif 
--- a's contract changed: cargo build --features outside
  cargo: locus: error: `b` relies on `a::double: v1`, but the interface of `a` states ["double: v2"]
  exit status 101
  driver: locus check a: 1 observations, 0 dependency interfaces, 0 expectations: ok
  driver: build-script build_script_build: start, 1 dependency interfaces
  driver: locus check build_script_build: 0 observations, 1 dependency interfaces, 0 expectations: ok
  driver: locus check b: 2 observations, 1 dependency interfaces, 2 expectations: FAILED
  driver: lib b-HASH: logical pass FAILED; runtime program not compiled
```

### 6. Semantic identities

From the facts of a clean build, with `def_path_hash` printed as the StableCrateId and then the hash local to the crate:

```text
== Q6: identities, from the facts of a fresh build
  facts of a:
    {"kind":"crate","name":"a","stable_crate_id":"2dbe4ad014a64de1","svh":"fddb749c1d02e6d6de815a52b9998e35"}
    {"kind":"item","def_kind":"Fn","path":"util::double","def":"a[2dbe]::util::double","def_path_hash":"2dbe4ad014a64de1:b7910b979539b937"}
    {"kind":"item","def_kind":"Fn","path":"div","def":"a[2dbe]::div","def_path_hash":"2dbe4ad014a64de1:db17f934e20acacf"}
    {"kind":"item","def_kind":"Fn","path":"witness","def":"a[2dbe]::witness","def_path_hash":"2dbe4ad014a64de1:c4964da8dde31fa9"}
    {"kind":"item","def_kind":"Struct","path":"Meters","def":"a[2dbe]::Meters","def_path_hash":"2dbe4ad014a64de1:c847e01f9eddc67a"}
    {"kind":"item","def_kind":"Impl { of_trait: false }","path":"Meters","def":"a[2dbe]::{impl#0}","def_path_hash":"2dbe4ad014a64de1:ba5f2692c6cd4118","self_type":"Meters","self_type_hash":"2dbe4ad014a64de1:c847e01f9eddc67a"}
    {"kind":"item","def_kind":"Impl { of_trait: true }","path":"<Meters as std::clone::Clone>","def":"a[2dbe]::{impl#1}","def_path_hash":"2dbe4ad014a64de1:de0dbde71701d8f1","self_type":"Meters","self_type_hash":"2dbe4ad014a64de1:c847e01f9eddc67a","trait":"std::clone::Clone","trait_hash":"940e698046960fd5:cb4fba115d0fb7c1"}
    {"kind":"item","def_kind":"AssocFn","path":"Meters::get","def":"a[2dbe]::{impl#0}::get","def_path_hash":"2dbe4ad014a64de1:240c1b7befd40b7a"}
    {"kind":"item","def_kind":"AssocFn","path":"<Meters as std::clone::Clone>::clone","def":"a[2dbe]::{impl#1}::clone","def_path_hash":"2dbe4ad014a64de1:ae8870a3e47d24e5"}
  facts of b:
    {"kind":"crate","name":"b","stable_crate_id":"98ec2375e644f924","svh":"6f4b6d636a9de12336c2bd74d4d424dd"}
    {"kind":"item","def_kind":"Fn","path":"half","def":"b[98ec]::half","def_path_hash":"98ec2375e644f924:fa8bc78ec25663a5"}
    {"kind":"item","def_kind":"Fn","path":"quadruple","def":"b[98ec]::quadruple","def_path_hash":"98ec2375e644f924:80424bfd9625be20"}
    {"kind":"item","def_kind":"Fn","path":"witnesses","def":"b[98ec]::witnesses","def_path_hash":"98ec2375e644f924:23ed584869f72bdc"}
    {"kind":"item","def_kind":"Fn","path":"answer","def":"b[98ec]::answer","def_path_hash":"98ec2375e644f924:cb8d647af8181e9b"}
    {"kind":"call","at":"$WORK/q6/ws/target/locus/lower/b-HASH/lib.rs:10:5: 10:20","path":"a::div","def":"a[2dbe]::div","def_path_hash":"2dbe4ad014a64de1:db17f934e20acacf"}
    {"kind":"call","at":"$WORK/q6/ws/target/locus/lower/b-HASH/lib.rs:15:5: 15:34","path":"a::double","def":"a[2dbe]::util::double","def_path_hash":"2dbe4ad014a64de1:b7910b979539b937"}
    {"kind":"call","at":"$WORK/q6/ws/target/locus/lower/b-HASH/lib.rs:15:15: 15:33","path":"a::double","def":"a[2dbe]::util::double","def_path_hash":"2dbe4ad014a64de1:b7910b979539b937"}
    {"kind":"call","at":"$WORK/q6/ws/target/locus/lower/b-HASH/lib.rs:15:25: 15:32","path":"a::Meters::get","def":"a[2dbe]::{impl#0}::get","def_path_hash":"2dbe4ad014a64de1:240c1b7befd40b7a"}
    {"kind":"call","at":"$WORK/q6/ws/target/locus/lower/b-HASH/lib.rs:19:6: 19:18","path":"a::witness","def":"a[2dbe]::witness","def_path_hash":"2dbe4ad014a64de1:c4964da8dde31fa9"}
    {"kind":"call","at":"$WORK/q6/ws/target/locus/lower/b-HASH/lib.rs:19:20: 19:32","path":"a::witness","def":"a[2dbe]::witness","def_path_hash":"2dbe4ad014a64de1:c4964da8dde31fa9"}
  facts of app:
    {"kind":"crate","name":"app","stable_crate_id":"50f95b187aba2425","svh":"603f72f568135b417c4da5c082d5de94"}
    {"kind":"item","def_kind":"Fn","path":"main","def":"app[50f9]::main","def_path_hash":"50f95b187aba2425:546638b5f14b7508"}
    {"kind":"call","at":"app/src/main.rs:5:13: 5:25","path":"b::Meters","def":"a[2dbe]::Meters::{constructor#0}","def_path_hash":"2dbe4ad014a64de1:b1ac028d177c0d9e"}
    {"kind":"call","at":"app/src/main.rs:6:48: 6:58","path":"b::half","def":"b[98ec]::half","def_path_hash":"98ec2375e644f924:fa8bc78ec25663a5"}
    {"kind":"call","at":"app/src/main.rs:6:60: 6:76","path":"b::quadruple","def":"b[98ec]::quadruple","def_path_hash":"98ec2375e644f924:80424bfd9625be20"}
    {"kind":"call","at":"app/src/main.rs:6:78: 6:89","path":"b::answer","def":"b[98ec]::answer","def_path_hash":"98ec2375e644f924:cb8d647af8181e9b"}
  runtime programs:
    runtime program a: stable_crate_id 2dbe4ad014a64de1, svh 41fbe45cc227c5f72d61421234a2e3f4, 8 items, the same DefPathHashes as the checking program
    runtime program build_script_build: stable_crate_id 466134134f869b51, svh 38bfb14de434b689cf33055980d4b286, 1 items, the same DefPathHashes as the checking program
    runtime program b: stable_crate_id 98ec2375e644f924, svh 5326ff8ee1b982981cc4e82c99761483, 4 items, the same DefPathHashes as the checking program
    runtime program app: stable_crate_id 50f95b187aba2425, svh 5cd148fb1396b74ffb9aad5e03c29f1f, 1 items, the same DefPathHashes as the checking program
  a's impls after swapping the order of its two impls:
    {"kind":"item","def_kind":"Impl { of_trait: true }","path":"<Meters as std::clone::Clone>","def":"a[2dbe]::{impl#0}","def_path_hash":"2dbe4ad014a64de1:ba5f2692c6cd4118","self_type":"Meters","self_type_hash":"2dbe4ad014a64de1:c847e01f9eddc67a","trait":"std::clone::Clone","trait_hash":"940e698046960fd5:cb4fba115d0fb7c1"}
    {"kind":"item","def_kind":"Impl { of_trait: false }","path":"Meters","def":"a[2dbe]::{impl#1}","def_path_hash":"2dbe4ad014a64de1:de0dbde71701d8f1","self_type":"Meters","self_type_hash":"2dbe4ad014a64de1:c847e01f9eddc67a"}
    {"kind":"item","def_kind":"AssocFn","path":"<Meters as std::clone::Clone>::clone","def":"a[2dbe]::{impl#0}::clone","def_path_hash":"2dbe4ad014a64de1:fd950378bb96d067"}
    {"kind":"item","def_kind":"AssocFn","path":"Meters::get","def":"a[2dbe]::{impl#1}::get","def_path_hash":"2dbe4ad014a64de1:1ae2effbe98468b4"}
--- a defines an item in its checking program only: cargo build
  cargo: Compiling a v0.1.0 ($WORK/q6/ws/a)
  cargo: error: locus: the runtime program differs from the checking program: 1 items missing, 0 extra
  exit status 101
  driver: lib a-HASH: start, 0 dependency interfaces
  driver: locus check a: 1 observations, 0 dependency interfaces, 0 expectations: ok
  driver: lib a-HASH: checked
  driver: runtime program a: stable_crate_id 2dbe4ad014a64de1, svh 9fade8dc0fd22160865849fcf21ad4ad, 8 items, DIFFERENT from the checking program (1 missing, 0 extra)
  driver: lib a-HASH: lower 4ms, checking program 43ms, logical pass 2ms, runtime program 27ms, runtime program FAILED
```

The last step added `#[cfg(__locus_check)] pub fn only_in_checking() {}` to `a`. In question 5, the library `a` had StableCrateId `2dbe4ad014a64de1`, its test harness `d411d1156c2ad591`, and `a` built with `RUSTC_WRAPPER` `d3623f8d7c5f2898`.

## Findings

1. **Library versions: (iv) is the simplest option that works, and (iii) also works. (ii) silently loses proofs, and (i) needs every checked dependency compiled twice and kept twice.**
   - (i), two crates of the same name: the markers do not unify. `b`'s checking program against `a`'s runtime rlib fails with E0308 wherever it passes a marker to or from `a`, and there `a`'s `$crate` macro (line 14) expanded to the runtime version too. It works only when every checking program is compiled against its dependencies' checking metadata, which the driver did by keeping them in a side directory and remapping `--extern` and `-L` for each checking compile. With the same `-C metadata` for both versions, rustc failed instead with E0463 at the `$crate` macro.
   - (ii), one marker crate: the markers unify, and a checking program compiles against its dependencies' runtime metadata. But a dependency's macro that calls Locus's macro through `$crate` expands to the dependency's runtime version: `b::nonzero!` produced no observation in `app`, and a proof that observes a moved value was accepted. A macro that names `::__locus` directly resolves in the caller's crate and works.
   - (iii), a procedural macro that reads `LOCUS_FLAVOR`: works, including through `$crate`, with one library and no remapping. It costs a procedural macro crate, and an input that rustc does not track.
   - (iv), one library whose macros expand under `#[cfg(__locus_check)]`: works in the same cases, with one rlib and no procedural macro. The `cfg` is evaluated where the macro expands, so `$crate` paths do not matter. The runtime compile needs `--check-cfg cfg(__locus_check)`, or every crate that expands a proof warns.
   - RFC 0001 says "two versions of Locus's library". Option (iv) is one library with two expansions, so whether it qualifies is the owner's decision, not this spike's.
2. **Two compiles in one invocation work, at 1.9 to 3.8 times the wall time of the same builds without the check (medians of three).**
   - The checking program's arguments differ from Cargo's only in root, `--emit`, `--out-dir`, incremental directory, `--json`, lints, the `cfg`, and the library; the runtime program's only in root, library, and `--check-cfg`.
   - Small workspace: clean 707 to 765 ms against 337 to 415 without the check; after a body change in `a`, 587 to 638 against 149 to 179; after a change in `app`, 195 to 217 against 81 to 84. With 2000 more functions in `a`: clean 2315 to 2677 against 1043 to 1140; after a body change in `a`, 1494 to 1544 against 570 to 643; after a change in `app`, 234 to 266 against 111 to 141.
   - The baseline is not plain Cargo: Locus crates are still lowered and compiled through the driver, and plain-Rust dependents are compiled unchecked.
   - The logical pass is on the critical path. Cargo starts a dependent only on the runtime program's metadata notification, which comes after the pass. With a 1.5 s pass, the clean build ran `a`, `b`'s build script, `b`, and `app` one after another, 6.6 s from `a`'s start to `app`'s end.
   - The checking program must not emit an artifact notification. When it did, Cargo started `d` 31 ms after `c`, before `c`'s pass had finished and before `c` had an interface, and then panicked. In `ws`, where `b` also waits for its build script, Cargo panicked (`assertion failed: edges.remove(&key)`) after `a`'s runtime program had finished.
3. **Invalidation is conservative, and rewritten dep-info is required.**
   - A change to `a`'s contract rebuilt `a` and re-checked `b`, which failed. `app` was re-checked once `b` passed.
   - A change to a body alone re-checked `b`, `b`'s build script, and `app` too. Cargo rebuilds every dependent of a rebuilt crate ("the dependency `a` was rebuilt"), although `a`'s interface file was not rewritten.
   - The rewritten dep-info lists the `.lc` module files, the dependencies' interface files, the library, and `locus`, and Cargo acted on it: a change to `util.lc` rebuilt `a`, and a change to `a`'s interface file alone rebuilt `b` and `app`.
   - With rustc's own dep-info, which lists only the lowered files, Cargo rebuilt once for nothing, and then **missed a change to `util.lc`: every crate stayed Fresh with a contract its dependent no longer met.**
4. **Stale success: `RUSTC_WRAPPER` is not part of Cargo's fingerprint. The driver's own line in `rustc -vV`, or a target directory of its own, makes it safe.**
   - `RUSTC_WORKSPACE_WRAPPER` is part of it: its path enters `-C metadata` of workspace members, so build mode's artifacts have other file names and StableCrateIds than plain Cargo's, and plain and checked builds alternate without reusing each other's artifacts.
   - Only its path is. Appending a byte to the driver left `c` Fresh, so a new driver installed at the same path would reuse results that the old one checked.
   - `RUSTC_WRAPPER` is not part of it, with stable Cargo 1.97 and with nightly Cargo 1.101: after a plain build, build mode was entirely Fresh, and `d`, whose check fails, built with exit 0.
   - The wrapper's `rustc -vV` output is part of it, for either wrapper. Cargo keeps it in `target/.rustc_info.json` and asks again when the wrapper's file changes, so a driver that adds its own identity to `-vV` gets artifacts of its own, for every crate, and for every version of itself. This worked best here: it is the driver's own, and it covers `RUSTC_WRAPPER`. A new line from an unchanged driver file was not noticed, so the line must identify the driver's file.
   - A target directory of its own also works. `RUSTFLAGS` changed file names but not `-C metadata` (the StableCrateId was the same as in a target directory of its own), and would compete with the user's own flags. `__CARGO_DEFAULT_LIB_METADATA` worked, but it is Cargo's internal variable.
   - After a failed check, Cargo had removed the crate's rlib before running the wrapper, and invalidated its fingerprint, so the next run checked again. The old `.rmeta` and interface file stayed on disk; nothing used them.
5. **Kinds of crates:**
   - A build script is crate `build_script_build`, type `bin`, root `build.rs`. It is checked when it depends on a Locus crate, and Cargo compiled it only after that crate's whole compile. Cargo runs the script itself, without the wrapper; its `cargo::rustc-cfg` reaches the crate's invocation as `--cfg`, and the driver copies it to the checking program.
   - A procedural macro is `--crate-type proc-macro`, compiled here as plain rustc, and loaded by both programs of `b`.
   - An integration test, and every harness of `cargo test`, is `--test` with no `--crate-type` and a `-C metadata` of its own, so it is another crate to check, with no interface. A library's harness is checked separately from the library.
   - Doctests are compiled by rustdoc from the `.lc` source, outside any wrapper, and failed ([cfg-corner-cases](../../issues/cfg-corner-cases.md)).
   - `RUSTC_WORKSPACE_WRAPPER` does not wrap a path dependency outside the workspace, so `outside` reached plain rustc and failed. `RUSTC_WRAPPER` wraps it, and it was checked from its `.lc` sources. Build mode therefore needs `RUSTC_WRAPPER`, and with it the `-vV` identity of finding 4.
   - Nightly Cargo's directory per unit did not disturb interface files kept beside each artifact.
6. **Identities: a DefPathHash names an item within one build configuration; interfaces need an identity that survives edits.**
   - A DefPathHash is the crate's StableCrateId followed by a hash of the item's definition path. It was identical in the checking and runtime programs of every crate in every run, so the two programs can be paired by it, as the spike's validation does. Calls through re-exports resolved to the defining item (`b::Meters` to `a[2dbe]::Meters::{constructor#0}`).
   - The StableCrateId comes from Cargo's `-C metadata`, which depends on the wrapper's path, the `-vV` output, and whether the crate is a test harness. The SVH differs between the two programs, so it is not an identity.
   - Impls are positional: swapping `a`'s two impls swapped their DefPathHashes and changed their methods'. Their trait and self type, by DefPathHash, stayed the same, and identify a trait impl. An inherent impl needs more than its self type, since a type may have several.
   - So facts can use DefPathHashes within one build, and an interface, which dependents rely on and hash, should name items by crate name and definition path, and impls by trait and self type, so that reordering a file does not change it.
   - The spike's validation compares only sets of items. It caught an item that only the checking program had.

## Open problems

- **The logical pass is on the critical path.** A crate's dependents wait for its whole pass. Overlapping them needs the interface before the pass ends; nothing here tried it.
- **Which version of the library.** Options (iv) and (iii) work, but they are not "two versions of Locus's library" as RFC 0001 words it. With either, user code that writes `cfg(__locus_check)`, or a user's procedural macro that reads `LOCUS_FLAVOR`, makes the two programs differ, and comparing items catches only differences in items.
- **The driver's identity.** Cargo re-reads `-vV` only when the wrapper's file changes, so the identity must cover Locus's library and `locus` too, or they must ship inside the driver's file. Only the driver's file and an environment variable were tried.
- **Keys of the side directories.** The spike keys lowered text, checking metadata, and facts by crate name and `-C extra-filename`. Nightly Cargo passes no `-C extra-filename` for binaries and build scripts, so a binary and its test harness would share a key. No case here collided.
- **Untested:** cross-compilation with `--target` (host and target units of one crate; procedural macros and build scripts on the host), a procedural macro that depends on a Locus crate, `.rs` modules in a Locus crate (lowering writes only `.lc` files), registry and git dependencies (`--cap-lints allow`), workspaces with many crates, release builds, `cargo check` and `cargo clippy`, and the cost of `RUSTC_WRAPPER` for crates that are not checked.
- **Measurements** come from one 4-core container, debug builds, small crates, and a logical pass that does nothing.
- **Doctests** stay [cfg-corner-cases](../../issues/cfg-corner-cases.md).
