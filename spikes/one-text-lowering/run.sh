#!/usr/bin/env bash
# Reproduces every result in README.md. Needs stable rustc 1.97 with the
# rustc-dev component (for the facts driver), Cargo, and python3.
#
#   ./run.sh [all|cases|q2|q3|spans|facts]
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
lib="$work/lib"
mkdir -p "$work/bin" "$work/diag"
"$here/build-lib.sh" "$lib" || exit 1
section="${1:-all}"
want() { [ "$section" = all ] || [ "$section" = "$1" ]; }

# compile FILE FLAVOR [rustc args...]: prints `ok` or the error codes, then
# each diagnostic. The checking flavor runs with lints capped, as in RFC 0001.
# LOCUS_ROOTS and LOCUS_SCOPES, when the caller sets them, reach the
# library's proc macros (see library/macros.rs).
compile() {
    local file="$1" flavor="$2"
    shift 2
    local name caps=()
    name="$(basename "$file" .rs)"
    [ "$flavor" = check ] && caps=(--cap-lints allow)
    rm -f "$work/bin/$name.$flavor"
    rustc +stable --edition 2024 --crate-name case "${caps[@]}" --error-format=json \
        --extern __locus="$lib/$flavor/lib__locus.rlib" -L dependency="$lib/$flavor" \
        "$@" "$file" -o "$work/bin/$name.$flavor" 2> "$work/diag/$name.$flavor.json"
    python3 -I "$here/diag.py" "$work/diag/$name.$flavor.json" "$file"
}

# case_ KEY FILE [rustc args...]: compiles one text against both flavors,
# compares with its `// expect[KEY]:` line, or else its `// expect:` line,
# and runs both binaries when both compiled. With BRIEF=1, prints errors only.
case_() {
    local key="$1" file="$2"
    shift 2
    local name expect c r got verdict label
    name="$(basename "$file" .rs)"
    expect=""
    [ -n "$key" ] && expect="$(sed -n "s|^// expect\[$key\]: ||p" "$file" | head -1)"
    [ -z "$expect" ] && expect="$(sed -n 's|^// expect: ||p' "$file" | head -1)"
    c="$(compile "$file" check "$@")"
    r="$(compile "$file" run "$@")"
    norm() { tr ',' '\n' <<< "$1" | sort | paste -sd, -; }
    got="check=$(norm "$(head -1 <<< "$c")") run=$(norm "$(head -1 <<< "$r")")"
    local want_c want_r
    want_c="$(sed -E 's/^check=([^ ]*) .*/\1/' <<< "$expect")"
    want_r="$(sed -E 's/.* run=([^ ]*)$/\1/' <<< "$expect")"
    [ "$got" = "check=$(norm "$want_c") run=$(norm "$want_r")" ] && verdict=as-expected || verdict="UNEXPECTED, expected $expect"
    label="$name"
    [ -n "$key" ] && label="$name [$key]"
    echo "== $label: $got ($verdict)"
    if [ "${BRIEF:-0}" = 1 ]; then
        tail -n +2 <<< "$c" | grep '^    error' | sed 's/^    /  check: /'
        tail -n +2 <<< "$r" | grep '^    error' | sed 's/^    /  run:   /'
    else
        echo "  checking flavor:"
        tail -n +2 <<< "$c"
        echo "  runtime flavor:"
        tail -n +2 <<< "$r"
    fi
    if [ -x "$work/bin/$name.check" ] && [ -x "$work/bin/$name.run" ]; then
        echo "  output, checking flavor: $("$work/bin/$name.check" 2>/dev/null | paste -sd'|' -)"
        echo "  output, runtime flavor:  $("$work/bin/$name.run" 2>/dev/null | paste -sd'|' -)"
    fi
}

# one FILE FLAVOR [rustc args...]: compiles one text against one flavor and
# runs it if it compiled.
one() {
    local file="$1" flavor="$2"
    shift 2
    local name out
    name="$(basename "$file" .rs)"
    out="$(compile "$file" "$flavor" "$@")"
    echo "== $name, $flavor flavor: $out"
    if [ -x "$work/bin/$name.$flavor" ]; then
        echo "  output: $("$work/bin/$name.$flavor" 2>&1 | paste -sd'|' -)"
    fi
}

if want cases; then
    echo "### Questions 1 and 5: hard cases and borrow cases"
    for f in "$here"/cases/*.rs; do
        case_ "" "$f"
    done
fi

if want q2; then
    echo
    echo "### Question 2: an attribute macro sensitive to ghost bindings"
    mkdir -p "$work/q2"
    rustc +stable --edition 2024 --crate-type proc-macro --crate-name token_sensitive \
        "$here/q2/token_sensitive.rs" --out-dir "$work/q2" || exit 1
    ts=(--extern token_sensitive="$work/q2/libtoken_sensitive.so")
    echo "Old design, two texts:"
    one "$here/q2/answer.old-check.rs" check "${ts[@]}"
    one "$here/q2/answer.old-run.rs" run "${ts[@]}"
    echo "New design, one text:"
    one "$here/q2/answer.rs" check "${ts[@]}"
    one "$here/q2/answer.rs" run "${ts[@]}"
fi

# The proc macro the respanned case uses.
q3_macros() {
    mkdir -p "$work/q3"
    rustc +stable --edition 2024 --crate-type proc-macro --crate-name respan \
        "$here/q3/respan.rs" --out-dir "$work/q3" || exit 1
}

if want q3; then
    echo
    echo "### Question 3: Locus macros that rustc expands"
    echo "Modes of the library's proc macros (LOCUS_ROOTS): precise = every root is"
    echo "runtime; whole = (a); table = (b); dispatch = (d); combined = (b) or (d)."
    echo "Files *.c.rs are (c)."
    q3_macros
    rs=(--extern respan="$work/q3/librespan.so")
    for f in "$here"/q3/*.rs; do
        case "$f" in
            */respan.rs) continue ;;
            *.c.rs)
                LOCUS_ROOTS=precise BRIEF=1 case_ c "$f" "${rs[@]}"
                continue ;;
        esac
        for mode in precise whole table dispatch combined; do
            LOCUS_ROOTS=$mode LOCUS_SCOPES="${f%.rs}.scopes" BRIEF=1 case_ "$mode" "$f" "${rs[@]}"
        done
    done
fi

if want spans; then
    echo
    echo "### Question 3 (b): proc_macro::Span position APIs on stable"
    mkdir -p "$work/spans"
    for api in "s.line()" "s.column()" "s.start()" "s.end()" "s.file()" "s.local_file()" \
        "s.source_text()" "s.byte_range()" "s.join(s)" "s.parent()" "s.source()" \
        'proc_macro::tracked::path("scopes")' 'proc_macro::tracked::env_var("LOCUS_SCOPES")'; do
        printf 'extern crate proc_macro;\nuse proc_macro::{Span, TokenStream};\n#[proc_macro]\npub fn m(i: TokenStream) -> TokenStream { let s = Span::call_site(); let _ = &s; let _ = %s; i }\n' \
            "$api" > "$work/spans/t.rs"
        if rustc +stable --edition 2024 --crate-type proc-macro "$work/spans/t.rs" \
            --out-dir "$work/spans" 2> "$work/spans/err.txt"; then
            echo "  stable:   $api"
        else
            echo "  unstable: $api: $(grep -m1 -o 'error\[E[0-9]*\].*' "$work/spans/err.txt")"
        fi
    done
    echo
    echo "### Question 3 (b): positions that roots arrive with (table mode, checking flavor)"
    q3_macros
    rs=(--extern respan="$work/q3/librespan.so")
    for n in snapshot-in-vec snapshot-through-own-macro snapshot-macro-body snapshot-respanned; do
        f="$here/q3/$n.rs"
        LOCUS_ROOTS=table LOCUS_SCOPES="${f%.rs}.scopes" LOCUS_SPIKE_DEBUG=1 compile "$f" check "${rs[@]}" > /dev/null
        echo "== $n"
        grep -h '^locus-root' "$work/diag/$n.check.json" | sed -E 's|file=[^ ]*/|file=|; s/^/  /'
    done
fi

if want facts; then
    echo
    echo "### Question 4: facts from the checking flavor"
    driver="$work/facts/debug/locus-lowering-facts"
    (cd "$here/facts" && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$work/facts" cargo +stable build -q) || exit 1
    sysroot="$(rustc +stable --print sysroot)"
    for n in 01-inferred-receiver 02-generic-identity 03-for-loop 04e-matching-types 07-method-order; do
        echo "== $n"
        status=0
        LD_LIBRARY_PATH="$sysroot/lib" "$driver" --sysroot "$sysroot" --edition 2024 --crate-name case \
            --cap-lints allow --error-format=json \
            --extern __locus="$lib/check/lib__locus.rlib" -L dependency="$lib/check" \
            "$here/cases/$n.rs" -o "$work/bin/$n.facts" 2> "$work/diag/$n.facts.json" || status=$?
        echo "  rustc exit status: $status; errors: $(python3 -I "$here/diag.py" "$work/diag/$n.facts.json" | head -1)"
    done | sed 's/^body /  body /'
fi
