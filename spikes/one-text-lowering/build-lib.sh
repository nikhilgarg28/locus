#!/usr/bin/env bash
# Builds both flavors of Locus's library into $1/check and $1/run:
# the proc-macro crate `locus_macros` and the library crate `__locus`.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="$1"
for flavor in check run; do
    cfg=()
    [ "$flavor" = check ] && cfg=(--cfg locus_check)
    mkdir -p "$out/$flavor"
    rustc +stable --edition 2024 --crate-type proc-macro --crate-name locus_macros "${cfg[@]}" \
        "$here/library/macros.rs" --out-dir "$out/$flavor"
    rustc +stable --edition 2024 --crate-type rlib --crate-name __locus "${cfg[@]}" \
        --extern locus_macros="$out/$flavor/liblocus_macros.so" \
        "$here/library/locus.rs" --out-dir "$out/$flavor"
done
