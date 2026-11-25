#!/usr/bin/env bash
# Reproduces every result in README.md. Needs stable rustc 1.97 and Cargo,
# and rustfmt for `expansions`.
#
#   ./run.sh [all|cases|corpus|ghostlets|typed|intrait|stubcases|stubcorpus|stubmore|three|splitcorpus|splitcalls|expansions] [name-prefix]
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

# ========================================================= the second mode
# "stub-self": program text keeps the one marker, and a distinct type, a
# stub, stands only where an `impl` names a logical type. Its workspace is
# stub-self/, whose library turns on the macro's feature `stub-self`.

# place LOWERING: the workspace of a lowering.
place() {
    case "$1" in old) echo "$here/baseline" ;; stub) echo "$here/stub-self" ;; split) echo "$here/split" ;; *) echo "$here" ;; esac
}
# versus PACKAGE A B: each file of the package under lowerings A and B (old:
# the existing spike's; new: this spike's first mode; stub: its second).
# Prints what B did, and marks a file whose outcome or output is not A's.
versus() {
    local pkg="$1" f bin low r c lit line same
    declare -A R C E O
    total=0 changed=0 accepted=0 a_both=0 b_both=0 a_rej=0 b_rej=0
    for f in $pkg/src/bin/"$only"*.rs; do
        bin="$(basename "$f" .rs)"
        line="== $bin:"
        for low in "$2" "$3"; do
            r="$(build "$(place "$low")" "$T/$low" runtime $pkg "$bin")"
            c="$(build "$(place "$low")" "$T/$low" check $pkg "$bin")"
            R[$low]="$(codes <<< "$r")"
            C[$low]="$(codes <<< "$c")"
            E[$low]="$(errors 2 <<< "$r" | sed "s/^/  $low runtime: /"; errors 2 <<< "$c" | sed "s/^/  $low check:   /")"
            O[$low]=""
            if [ "${R[$low]}/${C[$low]}" = ok/ok ]; then
                O[$low]="$(rest "$(run "$T/$low" runtime "$bin")")"
                [ "${O[$low]}" = "$(rest "$(run "$T/$low" check "$bin")")" ] || O[$low]="${O[$low]} (THE CHECKING BINARY PRINTS: $(rest "$(run "$T/$low" check "$bin")"))"
            fi
            line="$line $low runtime=${R[$low]} check=${C[$low]};"
        done
        total=$((total + 1))
        same=yes
        [ "${R[$2]}/${C[$2]}" = "${R[$3]}/${C[$3]}" ] && [ "${O[$2]}" = "${O[$3]}" ] || same=no
        if [ "$same" = no ]; then
            changed=$((changed + 1))
            line="$line CHANGED"
        fi
        echo "$line"
        [ -n "${E[$3]}" ] && echo "${E[$3]}"
        if [ "$same" = no ]; then
            [ -n "${O[$2]}" ] && echo "  $2 prints: ${O[$2]}"
            [ -n "${O[$3]}" ] && echo "  $3 prints: ${O[$3]}"
        fi
        lit="$(grep -oE -m1 'litmus/[^`]+\.lc' "$f" || true)"
        case "$lit" in
            *.accept.lc)
                accepted=$((accepted + 1))
                [ "${R[$2]}/${C[$2]}" = ok/ok ] && a_both=$((a_both + 1))
                [ "${R[$3]}/${C[$3]}" = ok/ok ] && b_both=$((b_both + 1))
                ;;
            *.reject.lc)
                [ "${R[$2]}" = ok ] && [ "${C[$2]}" != ok ] && a_rej=$((a_rej + 1))
                [ "${R[$3]}" = ok ] && [ "${C[$3]}" != ok ] && b_rej=$((b_rej + 1))
                ;;
        esac
    done
}

if want stubcases; then
    echo
    echo "## Second mode: every case of the first mode, under both modes"
    versus cases new stub
    echo "cases: $total; outcome or output differs between the modes: $changed"
fi

if want stubcorpus; then
    echo
    echo "## Second mode: the corpus and the ghost lets, with the existing spike's lowering and with this mode"
    versus corpus old stub
    echo "corpus: accepted litmus examples: $accepted; compile in both expansions: old $a_both, stub $b_both; files that differ: $changed"
    echo "corpus: rejected litmus examples that fail in the checking expansion alone: old $a_rej, stub $b_rej"
    versus ghost-lets old stub
    echo "ghost-lets: accepted litmus examples: $accepted; compile in both expansions: old $a_both, stub $b_both; files that differ: $changed"
fi

# The cases written for the second mode, and a reviewer's counterexamples,
# in more/src/bin. `expect` and `prints` are what this mode is meant to do.
#   //! ask: QUESTION => EXPECTED     a question for the driver (stub-self/oracle),
#                                    asked once the checking expansion type-checks
# The driver is rustc with one more step. It is built against the compiler's
# own crates: rustup component add rustc-dev llvm-tools --toolchain stable
if want stubmore; then
    echo
    echo "## Second mode: its own cases, and the driver's answers"
    oracle=""
    if rustup component list --installed 2> /dev/null | grep -q rustc-dev; then
        (cd stub-self/oracle && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$T/oracle" cargo +stable build -q) &&
            oracle="$T/oracle/debug/locus-stub-oracle"
    fi
    [ -n "$oracle" ] || echo "the driver is not built (rustc-dev is not installed): no question is asked"
    sysroot="$(rustc --print sysroot)"
    total=0 same=0 differ=0 asked=0 answered=0
    for f in more/src/bin/"$only"*.rs; do
        bin="$(basename "$f" .rs)"
        want_r="$(sed -n 's|^//! expect: runtime=\([^ ]*\) check=.*|\1|p' "$f")"
        want_c="$(sed -n 's|^//! expect: runtime=[^ ]* check=\([^ ]*\).*|\1|p' "$f")"
        r="$(build "$here/stub-self" "$T/stub" runtime more "$bin")"
        c="$(build "$here/stub-self" "$T/stub" check more "$bin")"
        got_r="$(codes <<< "$r")"
        got_c="$(codes <<< "$c")"
        verdict="as expected"
        matches "$want_r" "$got_r" && matches "$want_c" "$got_c" || verdict="DIFFERS"
        out=""
        if [ "$got_r" = ok ] && [ "$got_c" = ok ]; then
            ro="$(run "$T/stub" runtime "$bin")"
            co="$(run "$T/stub" check "$bin")"
            expected="$(sed -n 's|^//! prints: ||p' "$f" | paste -sd'|' -)"
            if [ "$(rest "$ro")" != "$(rest "$co")" ]; then
                verdict="DIFFERS"
                out="  the two binaries print different lines: [$ro] [$co]"
            elif [ -n "$expected" ] && [ "$(rest "$ro")" != "$expected" ]; then
                verdict="DIFFERS"
                out="  printed [$(rest "$ro")], expected [$expected]"
            elif [ -n "$(rest "$ro")" ]; then
                out="  both binaries print: $(rest "$ro")"
            fi
        fi
        total=$((total + 1))
        if [ "$verdict" = "as expected" ]; then same=$((same + 1)); else differ=$((differ + 1)); fi
        echo "== $bin: expect runtime=$want_r check=$want_c; got runtime=$got_r check=$got_c; $verdict"
        errors 2 <<< "$r" | sed 's/^/  runtime: /'
        errors 2 <<< "$c" | sed 's/^/  check:   /'
        [ -n "$out" ] && echo "$out"
        if [ -n "$oracle" ] && [ "$got_c" = ok ] && grep -q '^//! ask: ' "$f"; then
            answers="$T/answers/$bin.txt"
            mkdir -p "$T/answers"
            : > "$answers"
            touch "$f"
            (cd stub-self && LOCUS_ASK_CRATE="${bin//-/_}" LOCUS_ASK_FILE="$here/$f" LOCUS_ASK_OUT="$answers" \
                LOCUS_SYSROOT="$sysroot" LD_LIBRARY_PATH="$sysroot/lib" RUSTC_WORKSPACE_WRAPPER="$oracle" \
                RUSTFLAGS="--cfg __locus_check" cargo check -q -p more --bin "$bin" --target-dir "$T/stub/ask" > /dev/null 2>&1)
            while IFS= read -r q; do
                asked=$((asked + 1))
                a="$(grep -F -x -m1 "ask: $q" "$answers" || true)"
                if [ -n "$a" ]; then
                    answered=$((answered + 1))
                    echo "  $a"
                else
                    echo "  ask: ${q%% => *} => $(grep -F -m1 "ask: ${q%% => *} => " "$answers" | sed 's/.* => //') (EXPECTED: ${q##* => })"
                fi
            done < <(sed -n 's|^//! ask: ||p' "$f")
        fi
    done
    echo "its own cases: $total, as expected: $same, differ: $differ"
    echo "questions to the driver: $asked, answered as expected: $answered"
    if [ -z "$only" ]; then
        echo
        echo "### rustc's note for s11, in the runtime expansion"
        touch more/src/bin/s11-hand-blanket-helper.rs
        (cd stub-self && cargo build -q -p more --bin s11-hand-blanket-helper --target-dir "$T/stub/runtime" 2>&1) |
            grep -E '^error\[E0119\]|= note: upstream'
    fi
fi

# ========================================================= the third variant
# "split": the two expansions differ. The checking expansion is the first
# mode's, and the runtime expansion keeps one marker. Its workspace is split/.

# Every case under every variant: one line for each, with what each of the
# two expansions did and, where both compiled, what the binaries print.
if want three; then
    echo
    echo "## Every case under the three variants: first mode | stub-self | split"
    for pkg in cases more; do
        for f in $pkg/src/bin/"$only"*.rs; do
            [ -f "$f" ] || continue
            bin="$(basename "$f" .rs)"
            line="== $bin"
            for low in new stub split; do
                r="$(build "$(place "$low")" "$T/$low" runtime $pkg "$bin")"
                c="$(build "$(place "$low")" "$T/$low" check $pkg "$bin")"
                cr="$(codes <<< "$r")"
                cc="$(codes <<< "$c")"
                cell="runtime=$cr check=$cc"
                if [ "$cr/$cc" = ok/ok ]; then
                    ro="$(rest "$(run "$T/$low" runtime "$bin")")"
                    co="$(rest "$(run "$T/$low" check "$bin")")"
                    if [ "$ro" = "$co" ]; then
                        cell="$cell prints [$ro]"
                    else
                        cell="$cell RUNTIME PRINTS [$ro] CHECKING PRINTS [$co]"
                    fi
                fi
                line="$line | $cell"
            done
            echo "$line"
        done
    done
fi

if want splitcorpus; then
    echo
    echo "## Third variant: the corpus and the ghost lets, with the existing spike's lowering and with this variant"
    versus corpus old split
    echo "corpus: accepted litmus examples: $accepted; compile in both expansions: old $a_both, split $b_both; files that differ: $changed"
    echo "corpus: rejected litmus examples that fail in the checking expansion alone: old $a_rej, split $b_rej"
    versus ghost-lets old split
    echo "ghost-lets: accepted litmus examples: $accepted; compile in both expansions: old $a_both, split $b_both; files that differ: $changed"
fi

# The correspondence check that "both expansions compile" does not give. The
# driver lists, for each call the source wrote, the item rustc resolved it
# to. The two expansions of one source correspond only if the listings name
# the same item at every call. The type arguments, after ` with `, differ by
# design and are not compared, and neither is the item's path, which can
# name a logical type: a definition of the crate is compared by where the
# source writes it.
if want splitcalls; then
    echo
    echo "## Third variant: the calls of each expansion, for every case that both expansions accept"
    oracle=""
    if rustup component list --installed 2> /dev/null | grep -q rustc-dev; then
        (cd stub-self/oracle && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$T/oracle" cargo +stable build -q) &&
            oracle="$T/oracle/debug/locus-stub-oracle"
    fi
    sysroot="$(rustc --print sysroot)"
    mkdir -p "$T/calls"
    compared=0 diverge=0
    if [ -z "$oracle" ]; then
        echo "the driver is not built (rustc-dev is not installed): nothing is listed"
    else
        for pkg in cases more; do
            for f in $pkg/src/bin/"$only"*.rs; do
                [ -f "$f" ] || continue
                bin="$(basename "$f" .rs)"
                [ -x "$T/split/runtime/debug/$bin" ] && [ -x "$T/split/check/debug/$bin" ] || continue
                for mode in runtime check; do
                    flags=""
                    [ "$mode" = check ] && flags="--cfg __locus_check"
                    : > "$T/calls/$bin.$mode.txt"
                    touch "$f"
                    (cd split && LOCUS_ASK_CRATE="${bin//-/_}" LOCUS_CALLS_OUT="$T/calls/$bin.$mode.txt" \
                        LOCUS_SYSROOT="$sysroot" LD_LIBRARY_PATH="$sysroot/lib" RUSTC_WORKSPACE_WRAPPER="$oracle" \
                        RUSTFLAGS="$flags" cargo check -q -p $pkg --bin "$bin" --target-dir "$T/split/calls-$mode" > /dev/null 2>&1)
                done
                compared=$((compared + 1))
                # Compared by what is before ` | `: the call and its definition.
                d="$(diff <(sed 's/ | .*//' "$T/calls/$bin.runtime.txt") <(sed 's/ | .*//' "$T/calls/$bin.check.txt") | grep '^[<>]' | sed 's/^[<>] //; s/ => .*//' | sort -u || true)"
                if [ -n "$d" ]; then
                    diverge=$((diverge + 1))
                    echo "== $bin: the two expansions resolve a call differently"
                    while IFS= read -r call; do
                        grep -F "$call => " "$T/calls/$bin.runtime.txt" | sed 's/^/  runtime:  /'
                        grep -F "$call => " "$T/calls/$bin.check.txt" | sed 's/^/  checking: /'
                    done <<< "$d"
                fi
            done
        done
        echo "cases that both expansions accept: $compared; with a call resolved differently: $diverge"
        if [ -s "$T/calls/r10-inherent-beats-trait.runtime.txt" ]; then
            echo
            echo "### The whole listing for r10, runtime expansion and then checking expansion"
            cat "$T/calls/r10-inherent-beats-trait.runtime.txt"
            echo "--"
            cat "$T/calls/r10-inherent-beats-trait.check.txt"
        fi
    fi
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
