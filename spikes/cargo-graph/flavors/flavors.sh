#!/usr/bin/env bash
# Question 1, with plain rustc: `a` is built once, as a runtime artifact, and
# `b`'s checking program is compiled against it. Do the ghost markers unify,
# and does a macro from `a` expand to the checking flavor inside `b`?
#
# Usage: flavors.sh OUT_DIR
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="${1:?usage: flavors.sh OUT_DIR}"
rustc=(rustc +stable --edition 2024 --crate-type rlib)

# Runs one compile and prints its exit status and its errors and warnings.
show() {
    local label=$1 log status
    shift
    log=$("$@" --error-format=short 2>&1)
    status=$?
    printf '  %-52s exit %s\n' "$label" "$status"
    grep -E '(error|warning)(\[|:)' <<<"$log" | grep -vE 'aborting due|warnings? emitted' |
        sed -e "s|$here/||g" -e "s|$out/||g" -e 's/^/      /'
}

# Builds a's runtime artifact, a's checking metadata, and b three ways.
# $1: dir; $2: extern for the checking flavor; $3: extern for the runtime flavor;
# $4, $5: extra flags for checking and runtime compiles (a cfg) or "".
pair() {
    local d=$1 check=$2 run=$3 cflag=$4 rflag=$5
    mkdir -p "$d/run" "$d/check" "$d/b"
    show "a, runtime program (rlib)" \
        env LOCUS_FLAVOR=run "${rustc[@]}" --crate-name a -C metadata=a -C extra-filename=-a $rflag \
        "$here/a.rs" --extern "__locus=$run" -L "dependency=$d/lib" --out-dir "$d/run" \
        --check-cfg 'cfg(docsrs,test)' --check-cfg 'cfg(feature, values())'
    show "a, checking program (rmeta, side directory)" \
        env LOCUS_FLAVOR=check "${rustc[@]}" --crate-name a -C metadata=a -C extra-filename=-a $cflag \
        --emit=metadata "$here/a.rs" --extern "__locus=$check" -L "dependency=$d/lib" --out-dir "$d/check"
    show "b, checking program against a's runtime rlib" \
        env LOCUS_FLAVOR=check "${rustc[@]}" --crate-name b $cflag --emit=metadata "$here/b.rs" \
        --extern "__locus=$check" --extern "a=$d/run/liba-a.rlib" -L "dependency=$d/lib" --out-dir "$d/b"
    show "b, checking program against a's checking rmeta" \
        env LOCUS_FLAVOR=check "${rustc[@]}" --crate-name b $cflag --emit=metadata "$here/b.rs" \
        --extern "__locus=$check" --extern "a=$d/check/liba-a.rmeta" -L "dependency=$d/lib" --out-dir "$d/b"
    show "b, runtime program against a's runtime rlib" \
        env LOCUS_FLAVOR=run "${rustc[@]}" --crate-name b $rflag "$here/b.rs" \
        --extern "__locus=$run" --extern "a=$d/run/liba-a.rlib" -L "dependency=$d/lib" --out-dir "$d/b" \
        --check-cfg 'cfg(docsrs,test)' --check-cfg 'cfg(feature, values())'
}

lib() { mkdir -p "$1/lib"; }

echo "(i) two flavor crates named __locus, different -C metadata"
d="$out/i"; lib "$d"
show "flavor check" "${rustc[@]}" --crate-name __locus -C metadata=check -C extra-filename=-check "$here/i_check.rs" --out-dir "$d/lib"
show "flavor run" "${rustc[@]}" --crate-name __locus -C metadata=run -C extra-filename=-run "$here/i_run.rs" --out-dir "$d/lib"
pair "$d" "$d/lib/lib__locus-check.rlib" "$d/lib/lib__locus-run.rlib" "" ""

echo "(i') the same, but both flavors built with the same -C metadata"
d="$out/i-same"; lib "$d"
show "flavor check" "${rustc[@]}" --crate-name __locus -C metadata=same -C extra-filename=-check "$here/i_check.rs" --out-dir "$d/lib"
show "flavor run" "${rustc[@]}" --crate-name __locus -C metadata=same -C extra-filename=-run "$here/i_run.rs" --out-dir "$d/lib"
pair "$d" "$d/lib/lib__locus-check.rlib" "$d/lib/lib__locus-run.rlib" "" ""

echo "(ii) one marker crate, re-exported by two flavor crates named __locus"
d="$out/ii"; lib "$d"
show "marker crate locus_core" "${rustc[@]}" --crate-name locus_core "$here/core.rs" --out-dir "$d/lib"
for f in check run; do
    show "flavor $f" "${rustc[@]}" --crate-name __locus -C metadata=$f -C extra-filename=-$f "$here/ii_$f.rs" \
        --extern "locus_core=$d/lib/liblocus_core.rlib" --out-dir "$d/lib"
done
pair "$d" "$d/lib/lib__locus-check.rlib" "$d/lib/lib__locus-run.rlib" "" ""

echo "(iii) one __locus crate; a procedural macro reads LOCUS_FLAVOR"
d="$out/iii"; lib "$d"
show "proc macro locus_macros" rustc +stable --edition 2024 --crate-type proc-macro --crate-name locus_macros \
    "$here/iii_macros.rs" --extern proc_macro --out-dir "$d/lib"
show "__locus" "${rustc[@]}" --crate-name __locus "$here/iii_lib.rs" \
    --extern "locus_macros=$d/lib/liblocus_macros.so" --out-dir "$d/lib"
pair "$d" "$d/lib/lib__locus.rlib" "$d/lib/lib__locus.rlib" "" ""

echo "(iv) one __locus crate; prove! expands to code under cfg(__locus_check)"
d="$out/iv"; lib "$d"
show "__locus" "${rustc[@]}" --crate-name __locus "$here/iv_lib.rs" --out-dir "$d/lib"
pair "$d" "$d/lib/lib__locus.rlib" "$d/lib/lib__locus.rlib" "--cfg __locus_check" ""
