#!/usr/bin/env bash
# Reproduces the results in README.md. Builds the driver and the versions of
# Locus's library, then runs each experiment on a fresh copy of a workspace.
# Nothing in this directory is modified.
#
# Usage: ./run.sh [q1 q2 q3 q4 q5 q6]    (default: all of them)
# Needs: rustup component add rustc-dev llvm-tools --toolchain stable
# Questions 4 and 5 also run nightly Cargo, if a nightly toolchain is installed.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
[ $# -gt 0 ] || set -- q1 q2 q3 q4 q5 q6

drv="$work/driver/debug/locus-driver"
(cd "$here/driver" && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$work/driver" cargo +stable build -q) || exit 1

# The versions of Locus's library, one directory per option of question 1.
rc=(rustc +stable --edition 2024 --crate-type rlib)
lib() { "${rc[@]}" "$@" || exit 1; }
f="$here/flavors"
# (i) two crates named __locus, each with its own markers.
lib --crate-name __locus -C metadata=check -C extra-filename=-check "$f/i_check.rs" --out-dir "$work/lib-i"
lib --crate-name __locus -C metadata=run -C extra-filename=-run "$f/i_run.rs" --out-dir "$work/lib-i"
# (ii) one marker crate, re-exported by two crates named __locus.
lib --crate-name locus_core "$f/core.rs" --out-dir "$work/lib-ii"
for v in check run; do
    lib --crate-name __locus -C metadata=$v -C extra-filename=-$v "$f/ii_$v.rs" \
        --extern "locus_core=$work/lib-ii/liblocus_core.rlib" --out-dir "$work/lib-ii"
done
# (iii) one __locus crate and a procedural macro that reads LOCUS_FLAVOR.
rustc +stable --edition 2024 --crate-type proc-macro --crate-name locus_macros "$f/iii_macros.rs" \
    --extern proc_macro --out-dir "$work/lib-iii" || exit 1
lib --crate-name __locus "$f/iii_lib.rs" --extern "locus_macros=$work/lib-iii/liblocus_macros.so" --out-dir "$work/lib-iii"
# (iv) one __locus crate whose prove! expands to code under cfg(__locus_check). The default.
lib --crate-name __locus "$f/iv_lib.rs" --out-dir "$work/lib-iv"
lib_iv="$work/lib-iv/lib__locus.rlib"
# The driver links stable's rustc_driver, which nightly Cargo's environment does not find.
stable_lib="$(rustc +stable --print sysroot)/lib"

# Shortens paths, Cargo's hashes, and Cargo's timings.
tidy() {
    sed -E -e "s|$work|\$WORK|g" -e "s|$here/||g" -e 's/-[0-9a-f]{16}([^0-9a-f]|$)/-HASH\1/g' \
        -e 's/ in [0-9.]+m?s$//' -e 's/ \([0-9.]+s, [0-9.]+[a-z]* (after|before) last build at [^)]*\)//'
}

# A fresh copy of a workspace (and the crates beside it) under $work/NAME.
copy() {
    rm -rf "${work:?}/$1"
    mkdir -p "$work/$1"
    for src in "${@:2}"; do cp -r "$here/$src" "$work/$1/"; done
}

# Runs Cargo in DIR as build mode would: with the driver as its wrapper.
# WRAP picks the wrapper variable; NOWRAP=1 runs plain Cargo; LIB and CHECK_LIB
# pick the versions of Locus's library; TC picks Cargo's toolchain.
locus_cargo() {
    local dir=$1
    shift
    mkdir -p "$dir/target/locus"
    if [ -n "${NOWRAP:-}" ]; then
        (cd "$dir" && cargo +"${TC:-stable}" "$@")
    else
        (cd "$dir" && env LD_LIBRARY_PATH="$stable_lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
            LOCUS_SPIKE_DIR="$dir/target/locus" LOCUS_SPIKE_LIB="${LIB:-$lib_iv}" \
            LOCUS_SPIKE_CHECK_LIB="${CHECK_LIB:-${LIB:-$lib_iv}}" \
            "${WRAP:-RUSTC_WORKSPACE_WRAPPER}=$drv" cargo +"${TC:-stable}" "$@")
    fi
}

logpos() { cat "$1/target/locus/log" 2>/dev/null | wc -l; }
lognew() {
    tail -n +$(($2 + 1)) "$1/target/locus/log" 2>/dev/null | { if [ -n "${STAMPS:-}" ]; then cat; else sed -E 's/^\[[0-9]+\] //'; fi; } | tidy
}

# One Cargo run: prints what Cargo did, its exit status, and what the driver logged.
step() {
    local title=$1 dir=$2 before out status
    shift 2
    before=$(logpos "$dir")
    echo "--- $title: cargo $*"
    out=$(locus_cargo "$dir" "$@" 2>&1)
    status=$?
    printf '%s\n' "$out" | grep -E '^ *(Compiling|Dirty|Fresh) |^error|^warning|locus: error|^ *--> |^test result|^ *Running [a-z]|^ *Doc-tests|panicked at|^assertion failed' |
        grep -vE 'aborting due|could not compile|generated [0-9]+ warning' | tidy | sed -E 's/^ */  cargo: /'
    echo "  exit status $status"
    lognew "$dir" "$before" | grep -v 'rustc -vV' | sed 's/^/  driver: /'
}

# Like step, with VAR=VALUE assignments first.
envstep() {
    local vars=()
    while [[ $1 == *=* ]]; do vars+=("$1"); shift; done
    (
        [ ${#vars[@]} -gt 0 ] && export "${vars[@]}"
        step "$@"
    )
}

q1() {
    echo "== Q1: versions of Locus's library across crates, with plain rustc"
    "$here/flavors/flavors.sh" "$work/flavors" | tidy
    local name
    for name in i i-remap ii iii iv; do
        local vars=()
        case $name in
            i) vars=(LIB="$work/lib-i/lib__locus-run.rlib" CHECK_LIB="$work/lib-i/lib__locus-check.rlib") ;;
            i-remap) vars=(LIB="$work/lib-i/lib__locus-run.rlib" CHECK_LIB="$work/lib-i/lib__locus-check.rlib"
                LOCUS_SPIKE_REMAP=1) ;;
            ii) vars=(LIB="$work/lib-ii/lib__locus-run.rlib" CHECK_LIB="$work/lib-ii/lib__locus-check.rlib") ;;
            iii) vars=(LIB="$work/lib-iii/lib__locus.rlib") ;;
            iv) vars=(LIB="$lib_iv") ;;
        esac
        echo "== Q1: option ($name) through Cargo"
        copy "q1-$name" ws outside
        local d="$work/q1-$name/ws"
        envstep "${vars[@]}" "build" "$d" build
        [ -x "$d/target/debug/app" ] || continue
        echo "  app prints: $("$d/target/debug/app")"
        cat >>"$d/app/src/main.rs" <<'EOF'
fn moved() { let v = vec![1]; drop(v); let _ = b::nonzero!(v); }
EOF
        envstep "${vars[@]}" "app observes a moved value through b::nonzero!, a macro of a's" "$d" build
    done
}

# Prints the wall time of a command in milliseconds.
ms() {
    local t0 t1
    t0=$(date +%s%N)
    "$@" >/dev/null 2>&1
    t1=$(date +%s%N)
    echo $(((t1 - t0) / 1000000))
}

# One quiet build of DIR, timed; then VAR=VALUE assignments.
build_ms() {
    local d=$1
    shift
    (
        [ $# -gt 0 ] && export "$@"
        ms locus_cargo "$d" build -q
    )
}

# Times clean and incremental builds of a copy, with and without the check.
timings() {
    local d=$1 mode
    for mode in check skip; do
        local vars=(X=1) label="with the check   "
        [ $mode = skip ] && vars=(LOCUS_SPIKE_SKIP_CHECK=1) && label="without the check"
        local clean=() body=() leaf=() i
        rm -rf "$d/target"
        build_ms "$d" "${vars[@]}" >/dev/null # warm-up, not counted
        for i in 1 2 3; do
            rm -rf "$d/target"
            clean+=("$(build_ms "$d" "${vars[@]}")")
        done
        # A proof in a's body changes, so Cargo rebuilds a, b, and app.
        for i in 1 2 3; do
            if [ $((i % 2)) = 1 ]; then sed -i 's/prove!(d);/prove!(d, n);/' "$d/a/src/lib.lc"; else sed -i 's/prove!(d, n);/prove!(d);/' "$d/a/src/lib.lc"; fi
            body+=("$(build_ms "$d" "${vars[@]}")")
        done
        # A line of app's main changes, so Cargo rebuilds app only.
        for i in 1 2 3; do
            if [ $((i % 2)) = 1 ]; then sed -i 's/let n = 10;/let n = 12;/' "$d/app/src/main.rs"; else sed -i 's/let n = 12;/let n = 10;/' "$d/app/src/main.rs"; fi
            leaf+=("$(build_ms "$d" "${vars[@]}")")
        done
        echo "  $label: clean ${clean[*]}; a's body changed ${body[*]}; app's body changed ${leaf[*]}"
    done
}

q2() {
    echo "== Q2: two compiles in one wrapper invocation"
    copy q2 ws outside
    local d="$work/q2/ws"
    envstep LOCUS_SPIKE_ARGS=b "build, recording b's argument lists" "$d" build
    echo "  diff: Cargo's arguments for b, and the runtime program's"
    diff "$d"/target/locus/args/b-*.cargo "$d"/target/locus/args/b-*.run | tidy | sed 's/^/    /'
    echo "  diff: Cargo's arguments for b, and the checking program's"
    diff "$d"/target/locus/args/b-*.cargo "$d"/target/locus/args/b-*.check | tidy | sed 's/^/    /'

    echo "== Q2: wall time of cargo build, small workspace (milliseconds; three runs each)"
    copy q2t ws outside
    timings "$work/q2t/ws"
    echo "== Q2: wall time of cargo build, a with 2000 more functions, each with a proof (milliseconds)"
    copy q2b ws outside
    local bulk="$work/q2b/ws/a/src/bulk.lc"
    for i in $(seq 1 2000); do
        echo "pub fn f$i(x: u32, _h: ::__locus::Erased) -> u32 { let _p = ::__locus::prove!(x); x.wrapping_mul($i).rotate_left(3) ^ $i }"
    done >"$bulk"
    echo "mod bulk;" >>"$work/q2b/ws/a/src/lib.lc"
    timings "$work/q2b/ws"

    echo "== Q2: pipelining, with a logical pass that takes 1.5 s (log lines stamped in ms)"
    copy q2p ws outside
    d="$work/q2p/ws"
    envstep LOCUS_SPIKE_SLOW_PASS=1500 STAMPS=1 "clean build" "$d" build
    rm -rf "$d/target"
    envstep LOCUS_SPIKE_KEEP_ARTIFACT_JSON=1 LOCUS_SPIKE_SLOW_PASS=1500 STAMPS=1 \
        "clean build, the checking program's artifact notification passed to Cargo" "$d" build
    echo "== Q2: the same in q4ws, where d depends on c and has no build script"
    copy q2p4 q4ws
    d="$work/q2p4/q4ws"
    envstep LOCUS_SPIKE_KEEP_ARTIFACT_JSON=1 LOCUS_SPIKE_SLOW_PASS=1500 STAMPS=1 \
        "clean build, the checking program's artifact notification passed to Cargo" "$d" build
}

q3() {
    echo "== Q3: invalidation"
    copy q3 ws outside
    local d="$work/q3/ws"
    step "clean build" "$d" build -v
    step "no change" "$d" build -v
    sed -i 's/contract double: v1/contract double: v2/' "$d/a/src/util.lc"
    envstep LOCUS_SPIKE_SEEN=1 "a's contract changed: util.lc says double: v2" "$d" build -v
    echo "  after the failed check, in target/debug: $(cd "$d/target/debug" && ls app deps/libb-* 2>/dev/null | tidy | tr '\n' ' ')"
    envstep LOCUS_SPIKE_SEEN=1 "no change, after the failed check" "$d" build -v
    sed -i 's/uses a::double: v1/uses a::double: v2/' "$d/b/src/lib.lc"
    step "b now relies on double: v2" "$d" build -v
    local iface before
    iface=$(ls "$d"/target/debug/deps/liba-*.lcif | head -1)
    before=$(stat -c %Y.%y "$iface")
    sed -i 's/prove!(d);/prove!(d, n);/' "$d/a/src/lib.lc"
    step "only a proof in a's body changed" "$d" build -v
    [ "$before" = "$(stat -c %Y.%y "$iface")" ] && echo "  a's interface file was not rewritten (same mtime)"
    sed -i 's/x \* 2/x + x/' "$d/a/src/util.lc"
    step "util.lc's body changed" "$d" build -v
    echo "  a's dep-info, as rewritten:"
    tidy <"$(ls "$d"/target/debug/deps/a-*.d | head -1)" | sed 's/^/    /'
    echo "  b's dep-info, as rewritten:"
    tidy <"$(ls "$d"/target/debug/deps/b-*.d)" | sed 's/^/    /'
    for f in "$d"/target/debug/deps/liba-*.lcif; do echo "contract extra: v1" >>"$f"; done
    step "a's interface files changed, and nothing else" "$d" build -v

    echo "== Q3: the same with rustc's own dep-info, not rewritten"
    copy q3raw ws outside
    d="$work/q3raw/ws"
    envstep LOCUS_SPIKE_RAW_DEPINFO=1 "clean build" "$d" build -v
    echo "  a's dep-info, as rustc wrote it:"
    tidy <"$(ls "$d"/target/debug/deps/a-*.d | head -1)" | sed 's/^/    /'
    envstep LOCUS_SPIKE_RAW_DEPINFO=1 "no change" "$d" build -v
    envstep LOCUS_SPIKE_RAW_DEPINFO=1 "no change" "$d" build -v
    sed -i 's/contract double: v1/contract double: v2/' "$d/a/src/util.lc"
    envstep LOCUS_SPIKE_RAW_DEPINFO=1 "a's contract changed: util.lc says double: v2" "$d" build -v
}

# Plain Cargo, then build mode, then each again, on a fresh copy of q4ws.
# Arguments: NAME, then VAR=VALUE assignments for the build-mode runs.
q4case() {
    local name=$1
    shift
    copy "q4-$name" q4ws
    local d="$work/q4-$name/q4ws"
    NOWRAP=1 step "plain" "$d" build -v
    envstep "$@" "build mode" "$d" build -v
    NOWRAP=1 step "plain again" "$d" build -v
    envstep "$@" "build mode again" "$d" build -v
    echo "  libraries in target: $(find "$d/target" -path "$d/target/locus" -prune -o -name 'lib[cd]-*.rlib' -printf '%f\n' | sort | tr '\n' ' ')"
}

q4() {
    echo "== Q4: no stale success. d's check fails; plain rustc builds c and d."
    echo "=== RUSTC_WORKSPACE_WRAPPER"
    q4case wsw X=1
    echo "=== RUSTC_WRAPPER"
    q4case rw WRAP=RUSTC_WRAPPER
    echo "=== RUSTC_WRAPPER, and the driver adds a line to rustc -vV"
    q4case rw-stamp WRAP=RUSTC_WRAPPER LOCUS_SPIKE_VV_STAMP=spike-1
    echo "=== RUSTC_WRAPPER, and the line added to rustc -vV changes (a copy of the driver)"
    copy q4-rw-stamp2 q4ws
    local d="$work/q4-rw-stamp2/q4ws"
    mkdir -p "$work/driver3"
    cp "$drv" "$(dirname "$drv")/locus" "$work/driver3/"
    local d3="$work/driver3/locus-driver"
    drv=$d3 envstep WRAP=RUSTC_WRAPPER LOCUS_SPIKE_VV_STAMP=spike-1 "build mode, stamp spike-1" "$d" build -v
    drv=$d3 envstep WRAP=RUSTC_WRAPPER LOCUS_SPIKE_VV_STAMP=spike-2 "build mode, stamp spike-2" "$d" build -v
    printf 'x' >>"$d3"
    drv=$d3 envstep WRAP=RUSTC_WRAPPER LOCUS_SPIKE_VV_STAMP=spike-2 "build mode, stamp spike-2, a byte appended to the driver" "$d" build -v
    printf 'x' >>"$d3"
    drv=$d3 envstep WRAP=RUSTC_WRAPPER LOCUS_SPIKE_VV_STAMP=spike-2 "build mode, stamp spike-2, another byte appended" "$d" build -v
    echo "=== RUSTC_WORKSPACE_WRAPPER: the wrapper's path and the wrapper's file"
    copy q4-wpath q4ws
    d="$work/q4-wpath/q4ws"
    mkdir -p "$work/driver2"
    cp "$drv" "$(dirname "$drv")/locus" "$work/driver2/"
    step "build mode" "$d" build -v
    drv="$work/driver2/locus-driver" step "build mode, the same driver at another path" "$d" build -v
    printf 'x' >>"$work/driver2/locus-driver"
    drv="$work/driver2/locus-driver" step "build mode, a byte appended to that driver's file" "$d" build -v
    printf 'x' >>"$work/driver2/locus-driver"
    drv="$work/driver2/locus-driver" envstep LOCUS_SPIKE_VV_STAMP=spike-2 \
        "build mode, another byte appended, and the driver adds a line to rustc -vV" "$d" build -v
    echo "=== RUSTC_WRAPPER, and RUSTFLAGS=--cfg=__locus_build"
    q4case rw-flags WRAP=RUSTC_WRAPPER RUSTFLAGS=--cfg=__locus_build
    echo "=== RUSTC_WRAPPER, and __CARGO_DEFAULT_LIB_METADATA=locus (Cargo-internal)"
    q4case rw-meta WRAP=RUSTC_WRAPPER __CARGO_DEFAULT_LIB_METADATA=locus
    echo "=== RUSTC_WRAPPER, and a separate target directory"
    copy q4-rw-dir q4ws
    d="$work/q4-rw-dir/q4ws"
    NOWRAP=1 step "plain" "$d" build -v
    envstep WRAP=RUSTC_WRAPPER CARGO_TARGET_DIR="$d/target/locus/cargo" "build mode" "$d" build -v
    envstep WRAP=RUSTC_WRAPPER CARGO_TARGET_DIR="$d/target/locus/cargo" "build mode again" "$d" build -v
    if rustup run nightly cargo -V >/dev/null 2>&1; then
        local stable_rustc
        stable_rustc=$(rustup which --toolchain stable rustc)
        echo "=== nightly Cargo ($(rustup run nightly cargo -V)), stable rustc"
        echo "=== nightly Cargo: RUSTC_WORKSPACE_WRAPPER"
        TC=nightly RUSTC="$stable_rustc" q4case n-wsw X=1
        echo "=== nightly Cargo: RUSTC_WRAPPER"
        TC=nightly RUSTC="$stable_rustc" q4case n-rw WRAP=RUSTC_WRAPPER
    else
        echo "=== nightly Cargo: not installed, skipped"
    fi
}

q5() {
    echo "== Q5: kinds of crates"
    copy q5 ws outside
    local d="$work/q5/ws"
    envstep LOCUS_SPIKE_ARGS=b LOCUS_SPIKE_SEEN=1 "build" "$d" build
    echo "  the build script's cfg in b's arguments (Cargo's, checking program's):"
    grep -c '^b_fast$' "$d"/target/locus/args/b-*.cargo "$d"/target/locus/args/b-*.check | tidy | sed 's/^/    /'
    envstep LOCUS_SPIKE_SEEN=1 "test" "$d" test -v
    step "test, without doctests" "$d" test --lib --bins --tests
    step "outside dependency, RUSTC_WORKSPACE_WRAPPER" "$d" build --features outside
    envstep WRAP=RUSTC_WRAPPER LOCUS_SPIKE_SEEN=1 "outside dependency, RUSTC_WRAPPER" "$d" build --features outside
    echo "  app prints: $("$d/target/debug/app" | tr '\n' ' ')"
    if rustup run nightly cargo -V >/dev/null 2>&1; then
        echo "== Q5: the same workspace with nightly Cargo ($(rustup run nightly cargo -V)), stable rustc"
        copy q5n ws outside
        d="$work/q5n/ws"
        local stable_rustc
        stable_rustc=$(rustup which --toolchain stable rustc)
        envstep TC=nightly RUSTC="$stable_rustc" WRAP=RUSTC_WRAPPER "build" "$d" build --features outside
        echo "  interface files: $(find "$d/target" -name '*.lcif' | tidy | sort | tr '\n' ' ')"
        sed -i 's/contract double: v1/contract double: v2/' "$d/a/src/util.lc"
        envstep TC=nightly RUSTC="$stable_rustc" WRAP=RUSTC_WRAPPER "a's contract changed" "$d" build --features outside
    fi
}

q6() {
    echo "== Q6: identities, from the facts of a fresh build"
    copy q6 ws outside
    local d="$work/q6/ws"
    locus_cargo "$d" build -q >/dev/null 2>&1
    local c
    for c in a b app; do
        echo "  facts of $c:"
        local facts
        facts=$(ls "$d"/target/locus/facts/$c-*.jsonl | head -1)
        grep -E '"kind":"(crate|item|call)"' "$facts" | tidy | sed 's/^/    /'
    done
    echo "  runtime programs:"
    grep 'runtime program [a-z_]*:' "$d/target/locus/log" | sed -E 's/^\[[0-9]+\] /    /' | tidy
    echo "  a's impls after swapping the order of its two impls:"
    python3 - "$d/a/src/lib.lc" <<'EOF'
import sys
p = sys.argv[1]
s = open(p).read()
inherent = "impl Meters {\n    pub fn get(&self) -> u32 {\n        self.0\n    }\n}\n"
clone = "impl Clone for Meters {\n    fn clone(&self) -> Self {\n        Meters(self.0)\n    }\n}\n"
assert inherent in s and clone in s
s = s.replace(inherent, "@@INHERENT@@").replace(clone, inherent).replace("@@INHERENT@@", clone)
open(p, "w").write(s)
EOF
    locus_cargo "$d" build -q >/dev/null 2>&1
    grep -E '"def_kind":"(Impl|AssocFn)' "$(ls -t "$d"/target/locus/facts/a-*.jsonl | head -1)" | tidy | sed 's/^/    /'
    echo '#[cfg(__locus_check)] pub fn only_in_checking() {}' >>"$d/a/src/lib.lc"
    step "a defines an item in its checking program only" "$d" build
}

for q in "$@"; do "$q"; done
