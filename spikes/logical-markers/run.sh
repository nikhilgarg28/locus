#!/bin/sh
# Compiles the two stand-in libraries and every case, and compares each outcome with its first line.
set -u
cd "$(dirname "$0")"
OUT=target
rm -rf "$OUT"; mkdir -p "$OUT"
rustc --version
for lib in lg lu; do
    rustc --edition 2021 --crate-type lib --crate-name "$lib" "lib/$lib.rs" --out-dir "$OUT" -A warnings || exit 1
done
fail=0
for f in cases/*.rs; do
    name=$(basename "$f" .rs)
    expect=$(sed -n '1s,^// expect: ,,p' "$f")
    if rustc --edition 2021 "$f" -L "$OUT" --out-dir "$OUT" -A warnings 2>"$OUT/$name.err"; then
        got=ok
    else
        got=$(grep -o 'error\[E[0-9]*\]' "$OUT/$name.err" | head -1 | tr -d 'error[]')
    fi
    if [ "$got" = "$expect" ]; then verdict=as-expected; else verdict=UNEXPECTED; fail=1; fi
    printf '%-26s expect %-6s got %-6s %s\n' "$name" "$expect" "$got" "$verdict"
    if [ "$got" = ok ]; then "$OUT/$name" | sed 's/^/    | /'; else grep -m1 '^error' "$OUT/$name.err" | sed 's/^/    | /'; fi
done

# The oracle: a compiler driver that asks rustc about types it builds itself.
# Needs: rustup component add rustc-dev llvm-tools --toolchain stable
if rustup component list --installed 2>/dev/null | grep -q rustc-dev; then
    work=$(mktemp -d)
    (cd oracle && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$work" cargo +stable build -q) || exit 1
    sysroot=$(rustc --print sysroot)
    LD_LIBRARY_PATH="$sysroot/lib" "$work/debug/locus-oracle-spike" oracle/sample.rs \
        --crate-type lib --edition 2021 --sysroot "$sysroot" -A warnings || fail=1
    rm -rf "$work"
else
    echo "oracle: skipped, rustc-dev is not installed"
fi
exit $fail
