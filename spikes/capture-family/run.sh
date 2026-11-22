#!/usr/bin/env bash
# Compiles each case with stable rustc and compares the outcome with its first line.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
out="$(mktemp -d)"; ok=0; n=0
for f in cases/*.rs; do
    n=$((n + 1)); name="$(basename "$f" .rs)"
    want="$(sed -n '1s#^// expect: \([A-Za-z0-9]*\)\..*#\1#p' "$f")"
    if rustc --edition 2024 -A warnings "$f" -o "$out/$name" 2> "$out/$name.err"; then
        got="ok"; printed="$(cd "$out" && timeout 5 "./$name" 2>&1 | head -1)"
    else
        got="$(grep -o -E '^error\[E[0-9]+\]' "$out/$name.err" | sort -u | tr -d 'error[]' | paste -sd, -)"; printed=""
    fi
    case ",$got," in *",$want,"*) verdict="as expected"; ok=$((ok + 1));; *) verdict="DIFFERS (wanted $want)";; esac
    echo "$name: $got; $verdict${printed:+; prints $printed}"
    grep -E "^(error|help: consider|  = note: for)" "$out/$name.err" 2>/dev/null | grep -v aborting | head -4 | sed 's/^/    /' | cut -c1-200
done
echo "cases: $n, as expected: $ok"
rm -rf "$out"
