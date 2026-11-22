#!/usr/bin/env bash
# Reproduces every result in README.md. Needs stable rustc 1.97 and Cargo,
# and rustfmt for `expansions`.
#
#   ./run.sh [all|cases|corpus|ghostlets|typed|intrait|expansions] [name-prefix]
#
# Everything is built under this spike's own target/. The runtime expansion
# is stock `cargo build`; the checking expansion is the same command with
# RUSTFLAGS="--cfg __locus_check". Each lowering and each expansion has a
# target directory of its own, so no rebuild evicts another.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"
T="$here/target"
section="${1:-all}"
only="${2:-}"
want() { [ "$section" = all ] || [ "$section" = "$1" ]; }
export RUST_BACKTRACE=0 CARGO_TERM_COLOR=never
unset RUSTFLAGS LOCUS_SPIKE_ALL_TYPED LOCUS_SPIKE_IN_TRAIT LOCUS_SPIKE_EXPANSION

# build WORKSPACE TARGET MODE PACKAGE BIN: prints rustc's diagnostics, short form.
build() {
    local flags=""
    [ "$3" = check ] && flags="--cfg __locus_check"
    rm -f "$2/$3/debug/$5"
    (cd "$1" && RUSTFLAGS="$flags" cargo build -q -p "$4" --bin "$5" --target-dir "$2/$3" --message-format=short 2>&1)
}
# The error codes (or `error` for one without a code) in diagnostics on stdin.
codes() {
    local c
    c="$(grep -oE ': error(\[E[0-9]+\])?:' | sed -E 's/^: error(\[(E[0-9]+)\])?:/\2/; s/^$/error/' | sort -u | paste -sd, -)"
    echo "${c:-ok}"
}
# run TARGET MODE BIN: its output, one line, `|` between lines.
run() {
    [ -x "$1/$2/debug/$3" ] && "$1/$2/debug/$3" 2>&1 | paste -sd'|' -
}
# rest OUTPUT: a binary's output after its first line, which names the expansion.
rest() {
    case "$1" in *"|"*) echo "${1#*|}" ;; *) echo "" ;; esac
}
# matches EXPECTED GOT: `reject` is any error, and `annotate` is rustc's
# "type annotations needed", which is E0282 or E0283.
matches() {
    [ "$1" = "$2" ] || { [ "$1" = reject ] && [ "$2" != ok ]; } ||
        { [ "$1" = annotate ] && { [ "$2" = E0282 ] || [ "$2" = E0283 ]; }; }
}
# The first error lines of diagnostics on stdin, without the directory.
errors() {
    grep -E ': error' | sed "s|$here/||" | head -"${1:-3}"
}

echo "rustc: $(rustc --version)"
echo "cargo: $(cargo --version)"
start=$SECONDS

# ------------------------------------------------------------------- cases
# Each file in cases/src/bin says what it must do:
#   //! expect: runtime=<ok|reject|annotate|codes> check=<ok|reject|annotate|codes>
#   //! prints: <a line both binaries must print>      (any number)
# `expect` is what the draft's rules say, not what this toy was seen to do.
if want cases; then
    echo
    echo "## Cases, with the lowering of this spike"
    total=0 same=0 differ=0
    for f in cases/src/bin/"$only"*.rs; do
        bin="$(basename "$f" .rs)"
        want_r="$(sed -n 's|^//! expect: runtime=\([^ ]*\) check=.*|\1|p' "$f")"
        want_c="$(sed -n 's|^//! expect: runtime=[^ ]* check=\([^ ]*\).*|\1|p' "$f")"
        r="$(build "$here" "$T/new" runtime cases "$bin")"
        c="$(build "$here" "$T/new" check cases "$bin")"
        got_r="$(codes <<< "$r")"
        got_c="$(codes <<< "$c")"
        verdict="as expected"
        matches "$want_r" "$got_r" && matches "$want_c" "$got_c" || verdict="DIFFERS"
        out=""
        if [ "$got_r" = ok ] && [ "$got_c" = ok ]; then
            ro="$(run "$T/new" runtime "$bin")"
            co="$(run "$T/new" check "$bin")"
            expected="$(sed -n 's|^//! prints: ||p' "$f" | paste -sd'|' -)"
            if [ "${ro%%|*}" != "expansion: runtime" ] || [ "${co%%|*}" != "expansion: checking" ]; then
                verdict="DIFFERS"
                out="  a binary did not say which expansion it is: [$ro] [$co]"
            elif [ "$(rest "$ro")" != "$(rest "$co")" ]; then
                verdict="DIFFERS"
                out="  the two binaries print different lines: [$ro] [$co]"
            elif [ -n "$expected" ] && [ "$(rest "$ro")" != "$expected" ]; then
                verdict="DIFFERS"
                out="  printed [$(rest "$ro")], expected [$expected]"
            else
                out="  both binaries print: $(rest "$ro")"
            fi
        fi
        total=$((total + 1))
        if [ "$verdict" = "as expected" ]; then same=$((same + 1)); else differ=$((differ + 1)); fi
        echo "== $bin: expect runtime=$want_r check=$want_c; got runtime=$got_r check=$got_c; $verdict"
        errors 2 <<< "$r" | sed 's/^/  runtime: /'
        errors 2 <<< "$c" | sed 's/^/  check:   /'
        [ -n "$out" ] && echo "$out"
    done
    echo "cases: $total, as expected: $same, differ: $differ"
    if [ -z "$only" ]; then
        echo
        echo "### Three of them with the existing spike's lowering (one marker), for comparison"
        for bin in g13-copied g14-tuple-component g20-logic-where-runtime; do
            r="$(build "$here/baseline" "$T/old" runtime cases "$bin")"
            c="$(build "$here/baseline" "$T/old" check cases "$bin")"
            echo "== $bin: old runtime=$(codes <<< "$r") check=$(codes <<< "$c")"
            errors 1 <<< "$r" | sed 's/^/  old runtime: /'
        done
        echo
        echo "### rustc's whole message for g16, in the runtime expansion"
        touch cases/src/bin/g16-single-implementation.rs
        cargo build -q -p cases --bin g16-single-implementation --target-dir "$T/new/runtime" 2>&1 |
            sed -n '/^error\[E0283\]/,/^$/p'
    fi
fi

# ------------------------------------------------------------------ corpus
# compare PACKAGE: each file of the package, compiled with the existing
# spike's lowering (baseline/, one marker) and with this one. Sets the
# counts that the summaries print.
compare() {
    local pkg="$1" f bin line low ws r c cr cc lit ro co expected
    accepted=0 old_both=0 new_both=0 old_rej=0 new_rej=0
    for f in $pkg/src/bin/"$only"*.rs; do
        bin="$(basename "$f" .rs)"
        line="== $bin:"
        for low in old new; do
            ws="$here"
            [ "$low" = old ] && ws="$here/baseline"
            r="$(build "$ws" "$T/$low" runtime $pkg "$bin")"
            c="$(build "$ws" "$T/$low" check $pkg "$bin")"
            cr="$(codes <<< "$r")"
            cc="$(codes <<< "$c")"
            line="$line $low runtime=$cr check=$cc;"
            eval "${low}_r=\$cr ${low}_c=\$cc ${low}_errs=\"\$(errors 2 <<< \"\$r\" | sed 's/^/  '$low' runtime: /'; errors 2 <<< \"\$c\" | sed 's/^/  '$low' check:   /')\""
        done
        echo "$line"
        [ "$old_r/$old_c" != "$new_r/$new_c" ] && echo "  CHANGED"
        [ -n "$new_errs" ] && echo "$new_errs"
        if [ "$old_r/$old_c" != "$new_r/$new_c" ] && [ -n "$old_errs" ]; then echo "$old_errs"; fi
        lit="$(grep -oE -m1 'litmus/[^`]+\.lc' "$f" || true)"
        case "$lit" in
            *.accept.lc)
                accepted=$((accepted + 1))
                [ "$old_r/$old_c" = ok/ok ] && old_both=$((old_both + 1))
                [ "$new_r/$new_c" = ok/ok ] && new_both=$((new_both + 1))
                ;;
            *.reject.lc)
                [ "$old_r" = ok ] && [ "$old_c" != ok ] && old_rej=$((old_rej + 1))
                [ "$new_r" = ok ] && [ "$new_c" != ok ] && new_rej=$((new_rej + 1))
                ;;
        esac
        if [ "$new_r/$new_c" = ok/ok ]; then
            ro="$(run "$T/new" runtime "$bin")"
            co="$(run "$T/new" check "$bin")"
            expected="$(sed -n 's|^//~ run: ||p' "$f" | paste -sd'|' -)"
            if [ "$(rest "$ro")" != "$(rest "$co")" ]; then
                echo "  new: the two binaries print different lines: [$ro] [$co]"
            elif [ -n "$expected" ] && [ "$(rest "$ro")" = "$expected" ]; then
                echo "  new: both binaries print the example's run lines"
            elif [ -n "$expected" ] && [ -n "$(rest "$ro")" ]; then
                echo "  new: MISMATCH with the example's run lines: [$(rest "$ro")] expected [$expected]"
            elif [ -n "$(rest "$ro")" ]; then
                echo "  new: both binaries print: $(rest "$ro")"
            fi
            if [ "$old_r/$old_c" = ok/ok ] && [ "$(run "$T/old" runtime "$bin")" != "$ro" ]; then
                echo "  the old and new runtime binaries print different lines"
            fi
        fi
    done
}

# The 26 files that the existing spike's run.sh builds for its question 2
# (22 litmus examples and 4 others), copied.
if want corpus; then
    echo
    echo "## Corpus: the existing spike's cases, with its lowering and with this one"
    if [ -d /home/user/locus/spikes/locus-macro/cases/src/bin ]; then
        if diff -rq /home/user/locus/spikes/locus-macro/cases/src/bin corpus/src/bin > /dev/null; then
            echo "corpus/src/bin is identical to spikes/locus-macro/cases/src/bin"
        else
            echo "corpus/src/bin DIFFERS from spikes/locus-macro/cases/src/bin"
        fi
        if diff -rq /home/user/locus/spikes/locus-macro/locus/src baseline/locus/src > /dev/null &&
            diff -rq /home/user/locus/spikes/locus-macro/locus-macros/src baseline/locus-macros/src > /dev/null; then
            echo "baseline/ holds the library and macro of spikes/locus-macro, unchanged"
        else
            echo "baseline/ DIFFERS from the library and macro of spikes/locus-macro"
        fi
    fi
    compare corpus
    echo "accepted litmus examples: $accepted; compile in both expansions: old $old_both, new $new_both"
    echo "rejected litmus examples that fail in the checking expansion alone: old $old_rej, new $new_rej"
fi

# --------------------------------------------------------------- ghost lets
# Every litmus example with a `let x = logic! { .. };` that has no type
# annotation, copied by gen-ghost-lets.py. Their `main` prints nothing, so
# no run lines are compared.
if want ghostlets; then
    echo
    echo "## Ghost lets: every litmus example with an unannotated \`let x = logic! { .. };\`"
    [ -d /home/user/locus/litmus ] && python3 -I gen-ghost-lets.py /home/user/locus/litmus --check
    compare ghost-lets
    echo "accepted litmus examples: $accepted; compile in both expansions: old $old_both, new $new_both"
fi

# ------------------------------------------------------------------- typed
# How many ghost `let`s would need a written type if no ghost local could be
# left without a logical type: LOCUS_SPIKE_ALL_TYPED=1 makes every
# `let x = logic! { .. };` one typed binding. Cargo does not track the
# variable, so these builds have a target directory of their own.
if want typed; then
    echo
    echo "## Ghost locals initialized by \`logic!\` with no annotation, by what this spike's lowering gave them"
    # Counted in what `locus!` wrote: `untyped!` for a local that only logical
    # code mentions, `data!` for one whose type rustc infers.
    for pkg in corpus ghost-lets cases; do
        dump="$T/census/$pkg"
        rm -rf "$dump"
        mkdir -p "$dump"
        touch $pkg/src/bin/*.rs
        LOCUS_SPIKE_EXPANSION="$dump" cargo build -q -p $pkg --bins --keep-going --target-dir "$T/new/runtime" > /dev/null 2>&1
        lets="$(cat "$dump"/*.rs | tr -s '[:space:]' ' ' | grep -oE 'let (mut )?[a-z_0-9]+ = :: locus :: (untyped|data) ?!' | grep -c .)"
        untyped="$(cat "$dump"/*.rs | tr -s '[:space:]' ' ' | grep -oE 'let (mut )?[a-z_0-9]+ = :: locus :: untyped ?!' | grep -c .)"
        echo "$pkg: $lets such \`let\`s; $untyped hold the untyped marker, $((lets - untyped)) are typed by rustc's inference"
    done
    echo
    echo "## Every ghost local typed: which \`let\`s need an annotation the source does not have"
    export LOCUS_SPIKE_ALL_TYPED=1
    for pkg in corpus ghost-lets cases; do
        files=0 failing=0 lets=0
        for f in $pkg/src/bin/"$only"*.rs; do
            bin="$(basename "$f" .rs)"
            # Only programs that this spike's lowering compiles both ways.
            [ -x "$T/new/runtime/debug/$bin" ] && [ -x "$T/new/check/debug/$bin" ] || continue
            files=$((files + 1))
            r="$(build "$here" "$T/typed" runtime $pkg "$bin")"
            cr="$(codes <<< "$r")"
            [ "$cr" = ok ] && continue
            failing=$((failing + 1))
            echo "== $bin: runtime=$cr"
            while IFS= read -r e; do
                at="$(sed -E 's/^([^:]+):([0-9]+):.*/\1 \2/' <<< "$e")"
                lets=$((lets + 1))
                echo "  $(sed "s|$here/||" <<< "$e" | cut -c1-110)"
                echo "      $(sed -n "${at##* }p" "${at%% *}" | sed 's/^ *//')"
            done < <(grep -E ': error\[E028[23]\]' <<< "$r")
        done
        echo "$pkg: $files programs that compile both ways with the draft's rule; with every ghost local typed, $failing fail, at $lets \`let\`s"
    done
    unset LOCUS_SPIKE_ALL_TYPED
fi

# ----------------------------------------------------------------- in trait
# Every case again with the other placement of a trait's logic functions:
# LOCUS_SPIKE_IN_TRAIT=1 keeps their stubs as items of the lowered trait, in
# every invocation of `locus!`, the library's included. Cargo does not track
# the variable, so these builds have a target directory of their own.
if want intrait; then
    echo
    echo "## Every case with the stubs of a trait's logic functions kept in the lowered trait"
    export LOCUS_SPIKE_IN_TRAIT=1
    total=0 same=0 differ=0
    for f in cases/src/bin/"$only"*.rs; do
        bin="$(basename "$f" .rs)"
        want_r="$(sed -n 's|^//! expect: runtime=\([^ ]*\) check=.*|\1|p' "$f")"
        want_c="$(sed -n 's|^//! expect: runtime=[^ ]* check=\([^ ]*\).*|\1|p' "$f")"
        r="$(build "$here" "$T/intrait" runtime cases "$bin")"
        c="$(build "$here" "$T/intrait" check cases "$bin")"
        got_r="$(codes <<< "$r")"
        got_c="$(codes <<< "$c")"
        total=$((total + 1))
        if matches "$want_r" "$got_r" && matches "$want_c" "$got_c"; then
            same=$((same + 1))
        else
            differ=$((differ + 1))
            echo "== $bin: expect runtime=$want_r check=$want_c; got runtime=$got_r check=$got_c; DIFFERS"
            errors 1 <<< "$r" | sed 's/^/  runtime: /'
            errors 1 <<< "$c" | sed 's/^/  check:   /'
        fi
        if [ "$got_r/$got_c" = ok/ok ] && [ "$(rest "$(run "$T/intrait" runtime "$bin")")" != "$(rest "$(run "$T/intrait" check "$bin")")" ]; then
            echo "== $bin: the two binaries print different lines"
        fi
    done
    echo "cases: $total, as expected: $same, differ: $differ"
    unset LOCUS_SPIKE_IN_TRAIT
fi

# Shows what `locus!` wrote for the cases named by the prefix, formatted by rustfmt.
if [ "$section" = expansions ]; then
    out="$T/expansion"
    rm -rf "$out"
    mkdir -p "$out"
    for f in cases/src/bin/"$only"*.rs corpus/src/bin/"$only"*.rs; do
        [ -f "$f" ] || continue
        touch "$f"
        LOCUS_SPIKE_EXPANSION="$out" cargo build -q -p "$(cut -d/ -f1 <<< "$f")" --bin "$(basename "$f" .rs)" --target-dir "$T/new/runtime" > /dev/null 2>&1
    done
    for f in "$out"/*.rs; do
        echo "== $(basename "$f")"
        rustfmt --edition 2024 --emit stdout -q "$f" | grep -v '^$\|^/'
    done
fi

echo
echo "wall time: $((SECONDS - start)) s"
