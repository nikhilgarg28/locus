#!/usr/bin/env bash
# Builds the driver against stable rustc internals and runs it as Cargo's
# workspace wrapper on the sample crate. Prints the facts the driver
# reported and the diagnostics rustc produced.
#
# Needs: rustup component add rustc-dev llvm-tools --toolchain stable
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

(cd "$here/driver" && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$work/driver" cargo +stable build -q)

cd "$here/sample"
status=0
RUSTC_WORKSPACE_WRAPPER="$work/driver/debug/locus-facts-driver" \
LOCUS_SPIKE_CRATE=sample \
LOCUS_SPIKE_ROOT_FROM=src/lib.rs \
LOCUS_SPIKE_ROOT_TO=check/lib.rs \
LOCUS_SPIKE_FACTS="$work/facts.jsonl" \
LOCUS_SPIKE_BODY=first \
CARGO_TARGET_DIR="$work/sample" \
cargo +stable check -q --message-format=json > "$work/messages.json" || status=$?

echo "cargo check exit status: $status"
echo "facts:"
sed 's/^/  /' "$work/facts.jsonl"
echo "diagnostics (level, code, primary span, message):"
python3 -I - "$work/messages.json" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    m = json.loads(line)
    if m.get("reason") != "compiler-message":
        continue
    d = m["message"]
    primary = [s for s in d["spans"] if s["is_primary"]]
    at = f"{primary[0]['file_name']}:{primary[0]['line_start']}:{primary[0]['column_start']}" if primary else "-"
    print(" ", d["level"], (d.get("code") or {}).get("code"), at, d["message"])
PY
