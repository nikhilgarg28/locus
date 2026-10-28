#!/usr/bin/env bash
# Reproduces every result in README.md. Needs stable rustc 1.97 and Cargo,
# python3, and for question 4 rustfmt and rust-analyzer
# (`rustup component add rustfmt rust-analyzer`).
#
#   ./run.sh [all|q1|q2|q3|q4|q5|expansions]
#
# Everything is built under this spike's own target/. The runtime expansion
# is stock `cargo build`; the checking expansion is the same command with
# RUSTFLAGS="--cfg __locus_check". They use separate target directories,
# target/runtime and target/check, so neither rebuild evicts the other.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"
litmus="$(cd ../../litmus && pwd)"
T="$here/target"
section="${1:-all}"
want() { [ "$section" = all ] || [ "$section" = "$1" ]; }
export RUST_BACKTRACE=0 CARGO_TERM_COLOR=never
unset RUSTFLAGS

# build MODE PACKAGE BIN: prints rustc's diagnostics, short form.
build() {
    local flags=""
    [ "$1" = check ] && flags="--cfg __locus_check"
    rm -f "$T/$1/debug/$3"
    RUSTFLAGS="$flags" cargo build -q -p "$2" --bin "$3" --target-dir "$T/$1" --message-format=short 2>&1
}
# The error codes (or `error` for one without a code) in diagnostics on stdin.
codes() {
    local c
    c="$(grep -oE ': error(\[E[0-9]+\])?:' | sed -E 's/^: error(\[(E[0-9]+)\])?:/\2/; s/^$/error/' | sort -u | paste -sd, -)"
    echo "${c:-ok}"
}
# run MODE BIN: its output, one line, `|` between lines.
run() {
    [ -x "$T/$1/debug/$2" ] && "$T/$1/debug/$2" 2>&1 | paste -sd'|' -
}

echo "rustc: $(rustc --version)"
echo "cargo: $(cargo --version)"
start=$SECONDS

# ---------------------------------------------------------------- question 1
# lex_rustc OUT ROOT FILE...: each FILE's full text, inside an invocation of
# a procedural macro, through rustc's own lexer (edition 2024). The macro,
# __lexdump!, writes the tokens it was given to OUT/<flat name>.rustc.txt.
lex_rustc() {
    local out="$1" root="$2" so f rel flat ok=0 fail=0
    shift 2
    so="$(cargo build -q -p locus-macros --message-format=json | python3 -I -c '
import json, sys
for line in sys.stdin:
    m = json.loads(line)
    if m.get("reason") == "compiler-artifact" and m["target"]["name"] == "locus_macros":
        print([f for f in m["filenames"] if f.endswith((".so", ".dylib", ".dll"))][0])')"
    for f in "$@"; do
        rel="${f#"$root"/}"
        flat="${rel//\//__}"
        { printf 'locus_macros::__lexdump! { "%s";\n' "$out/$flat.rustc.txt"; cat "$f"; printf '\n}\n'; } > "$out/$flat.rs"
        if rustc --edition 2024 --crate-type lib --crate-name lexcheck --emit metadata -o "$out/$flat.rmeta" \
            --extern locus_macros="$so" "$out/$flat.rs" 2> "$out/$flat.err"; then
            ok=$((ok + 1))
        else
            fail=$((fail + 1))
            echo "  fail $rel: $(grep -m1 '^error' "$out/$flat.err")"
            # rustc's lexer recovered and still called the macro: keep that dump apart.
            [ -f "$out/$flat.rustc.txt" ] && mv "$out/$flat.rustc.txt" "$out/$flat.recovered.txt"
        fi
    done
    echo "rustc lexer, inside a proc macro invocation: $ok of $((ok + fail)) tokenize, $fail fail"
}
compare() { python3 -I compare-dumps.py "$1"; }
if want q1; then
    echo
    echo "## Question 1: does each litmus file tokenize as a Rust token stream?"
    cargo build -q -p tokenize -p locus-macros
    out="$T/q1"
    rm -rf "$out"
    mkdir -p "$out/litmus" "$out/edge"
    mapfile -t files < <(find "$litmus" -name '*.lc' | sort)
    echo "litmus files: ${#files[@]}"
    "$T/debug/tokenize" "$out/litmus" "$litmus" "${files[@]}"
    lex_rustc "$out/litmus" "$litmus" "${files[@]}"
    compare "$out/litmus"
    echo
    echo "### Edge cases, not from the corpus (tokenize/edge/)"
    mapfile -t edge < <(find "$here/tokenize/edge" -name '*.txt' | sort)
    "$T/debug/tokenize" "$out/edge" "$here/tokenize/edge" "${edge[@]}"
    lex_rustc "$out/edge" "$here/tokenize/edge" "${edge[@]}"
    compare "$out/edge"
fi

# ---------------------------------------------------------------- question 2
if want q2; then
    echo
    echo "## Question 2: two expansions from one macro"
    python3 -I gen-cases.py --check || exit 1
    for f in cases/src/bin/*.rs; do
        bin="$(basename "$f" .rs)"
        r="$(build runtime cases "$bin")"
        c="$(build check cases "$bin")"
        echo "== $bin: runtime=$(codes <<< "$r") check=$(codes <<< "$c")"
        grep -E ': error' <<< "$r" | sed 's/^/  runtime: /' | head -4
        grep -E ': error' <<< "$c" | sed 's/^/  check:   /' | head -4
        grep -E ': warning' <<< "$r" | grep -v 'generated [0-9]* warning' | sed 's/^/  runtime: /' | head -4
        if [ -x "$T/runtime/debug/$bin" ] && [ -x "$T/check/debug/$bin" ]; then
            ro="$(run runtime "$bin")"
            co="$(run check "$bin")"
            echo "  runtime binary: $ro"
            echo "  check binary:   $co"
            lit="$(grep -oE -m1 'litmus/[^`]+\.lc' "$f" || true)"
            if [ -n "$lit" ]; then
                expected="$(sed -n 's|^//~ run: ||p' "../../$lit" | paste -sd'|' -)"
                if [ -z "$expected" ]; then
                    echo "  no run lines"
                elif [ "${ro#*|}" = "$expected" ] && [ "${co#*|}" = "$expected" ]; then
                    echo "  both match the example's run lines"
                else
                    echo "  MISMATCH with the example's run lines: $expected"
                fi
            fi
        fi
    done
    echo
    echo "### unexpected_cfgs, in a crate that does not declare the checking cfg (cross/user_c)"
    touch cross/user_c/src/bin/c-marker-hole.rs
    build runtime user_c c-marker-hole | grep -A1 'warning' | head -4
fi

# Shows what `locus!` wrote for some cases, formatted by rustfmt.
if [ "$section" = expansions ]; then
    out="$T/expansion"
    rm -rf "$out"
    mkdir -p "$out"
    touch cases/src/bin/*.rs
    LOCUS_SPIKE_EXPANSION="$out" cargo build -q -p cases --bins --keep-going --target-dir "$T/runtime" > /dev/null 2>&1
    for f in "$out"/*.rs; do
        echo "== $(basename "$f")"
        rustfmt --edition 2024 --emit stdout -q "$f" | grep -v '^$\|^/home'
    done
fi

# ---------------------------------------------------------------- question 3
if want q3; then
    echo
    echo "## Question 3: diagnostics inside a locus! block"
    for bin in plain-error locus-position-error check-only-error; do
        for mode in runtime check; do
            flags=""
            [ "$mode" = check ] && flags="--cfg __locus_check"
            echo "== $bin, $mode expansion"
            rm -f "$T/$mode/debug/$bin"
            RUSTFLAGS="$flags" cargo build -q -p diag --bin "$bin" --target-dir "$T/$mode" 2>&1 \
                | grep -v '^For more information\|^error: could not compile\|^$'
            [ -x "$T/$mode/debug/$bin" ] && echo "  (compiled)"
        done
    done
fi

# ---------------------------------------------------------------- question 4
if want q4; then
    echo
    echo "## Question 4: tooling"
    echo "### cargo fmt --check --all"
    cargo fmt --check --all > "$T/fmt.diff" 2>&1
    echo "exit status: $?"
    grep '^Diff in' "$T/fmt.diff" | sed "s|$here/||"
    echo "### rustfmt on a copy of diag/src/bin/fmt-probe.rs, then diff against the original"
    mkdir -p "$T/q4"
    cp diag/src/bin/fmt-probe.rs "$T/q4/fmt-probe.rs"
    rustfmt --edition 2024 "$T/q4/fmt-probe.rs"
    echo "rustfmt exit status: $?"
    diff diag/src/bin/fmt-probe.rs "$T/q4/fmt-probe.rs"
    echo "### rust-analyzer"
    if ! rust-analyzer --version > /dev/null 2>&1; then
        echo "rust-analyzer is not installed: skipped"
    else
        rust-analyzer --version
        ra() { CARGO_TARGET_DIR="$T/ra" rust-analyzer "$@"; }
        clean() { tr '\r\010' '\n\n' | grep -a -v 'perf counter\|^ *$' | sed "s|$here/||g"; }
        ra -v -v analysis-stats . > "$T/ra-stats.out" 2> "$T/ra-stats.err"
        echo "--- analysis-stats -v -v, exit status $?: workspace totals"
        clean < "$T/ra-stats.err" | grep -a -E '^ *(exprs|pats):'
        echo "--- bodies it analyzed in the cases crate other than main, all written inside locus! blocks"
        clean < "$T/ra-stats.out" | grep -a -E '^In [a-z-]+::[A-Za-z_]+: [0-9]+ exprs' | grep -a -v '::main:' \
            | grep -a -c -F -f <(ls cases/src/bin | sed 's/\.rs$/::/; s/^/In /')
        echo "--- every function with an unknown type or a type mismatch, and each such expression"
        clean < "$T/ra-stats.out" | grep -a -E 'Expected|Unknown type|^In ' | grep -a -v ' 0 unknown, 0 partial\|pats,'
        echo "--- the same, with RUSTFLAGS=\"--cfg __locus_check\": the checking expansion"
        RUSTFLAGS="--cfg __locus_check" CARGO_TARGET_DIR="$T/ra-check" rust-analyzer -v -v analysis-stats . \
            > "$T/ra-stats-check.out" 2> "$T/ra-stats-check.err"
        clean < "$T/ra-stats-check.err" | grep -a -E '^ *(exprs|pats):'
        clean < "$T/ra-stats-check.out" | grep -a -E 'Expected|Unknown type|^In ' | grep -a -v ' 0 unknown, 0 partial\|pats,'
        echo "--- diagnostics"
        ra diagnostics . 2>&1 | clean | grep -a '^at crate'
        echo "--- diagnostics, with RUSTFLAGS=\"--cfg __locus_check\""
        RUSTFLAGS="--cfg __locus_check" CARGO_TARGET_DIR="$T/ra-check" rust-analyzer diagnostics . 2>&1 | clean | grep -a '^at crate'
        echo "--- unresolved-references"
        ra unresolved-references . > "$T/ra-unresolved.out" 2>&1
        echo "exit status: $?"
        clean < "$T/ra-unresolved.out" | grep -a -E 'processing crate|panicked|db is attached' | head -4
    fi
fi

# ---------------------------------------------------------------- question 5
if want q5; then
    echo
    echo "## Question 5: visibility across crates"
    for pkg in user_b user_c; do
        for f in cross/$pkg/src/bin/*.rs; do
            bin="$(basename "$f" .rs)"
            line="$bin:"
            for mode in runtime check; do
                d="$(build $mode $pkg "$bin")"
                c="$(codes <<< "$d")"
                if [ "$c" = ok ]; then
                    line="$line $mode=ok, prints [$(run $mode "$bin" | sed 's/thread .main. ([0-9]*) //')]"
                else
                    line="$line $mode=$c [$(grep -m1 ': error' <<< "$d" | sed -E 's/^[^ ]+ error(\[E[0-9]+\])?: //')]"
                fi
                line="$line;"
            done
            echo "$line"
        done
    done
fi

echo
echo "wall time: $((SECONDS - start)) s"
