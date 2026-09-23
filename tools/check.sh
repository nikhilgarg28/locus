#!/usr/bin/env bash
# The checks every commit must pass: formatting, clippy, and the tests, whose
# randomized parts use fixed seeds and small counts.
#
#   tools/check.sh              the fast form, for every commit; its tests
#                               are timed against FAST_LIMIT_SECONDS, the
#                               bound the Build plan asks of them, and a run
#                               over it is reported, not failed
#   tools/check.sh --extended   before a milestone: the fast form, then the
#                               whole suite again in release with
#                               LOCUS_EXTENDED=1, under which the randomized
#                               tests run a hundred times as many cases (the
#                               random programs run 10,000), and then one
#                               line per exit criterion of the Build plan,
#                               taken from what tests/acceptance.rs and the
#                               long runs printed
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

FAST_LIMIT_SECONDS=120

extended=
for argument in "$@"; do
    case "$argument" in
        --extended) extended=1 ;;
        -h | --help)
            echo "usage: tools/check.sh [--extended]"
            exit 0
            ;;
        *)
            echo "usage: tools/check.sh [--extended]" >&2
            exit 2
            ;;
    esac
done

# The fast gate. Building is not timed: the tests are, from a warm build.
# The editor and browser highlighters: the copy embedded in the atlas is the
# module's source, and both the module and the VS Code grammar are run over
# every .lc file when Node is present (the grammar test skips itself
# without its npm dependencies).
python3 tools/metrics.py invalidate
source_fingerprint=$(python3 tools/metrics.py fingerprint)
python3 tools/highlight.py check
python3 tools/spec.py check
if command -v node >/dev/null 2>&1; then
    node editors/highlight/test.js
    node editors/vscode/locus/test/tokenize.js
fi
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo test --locked --offline --no-run
mkdir -p target
started=$(date +%s)
cargo test --locked --offline 2>&1 | tee target/check-fast.log
elapsed=$(( $(date +%s) - started ))
echo "LOCUS TEST RUN COMPLETE: fast" >> target/check-fast.log
fast="fast tests: ${elapsed}s, limit ${FAST_LIMIT_SECONDS}s"
if (( elapsed > FAST_LIMIT_SECONDS )); then
    fast="$fast: OVER THE LIMIT"
fi
echo "$fast"
fast_elapsed=$elapsed
if [[ -z "$extended" ]]; then
    python3 tools/bench.py check
    python3 tools/metrics.py check
    python3 tools/gate.py fast "$fast_elapsed" "$FAST_LIMIT_SECONDS"
    exit 0
fi

# The extended runs, in release so that the counts are feasible. Everything
# the tests print is kept, so that the summary below can quote it.
export LOCUS_EXTENDED=1
mkdir -p target
log=target/check-extended.log
started=$(date +%s)
cargo test --locked --offline --release -- --nocapture 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
echo "LOCUS TEST RUN COMPLETE: extended" >> "$log"
elapsed=$(( $(date +%s) - started ))

# One line per exit criterion of the Build plan. A criterion the acceptance
# tests measure prints `criterion: ...`; the three that rest on the long
# runs are quoted from those runs' own summaries.
summary() {
    local line
    line=$(grep -m 1 -E "$1" "$log" || true)
    if [[ -z "$line" ]]; then
        echo "  $2: no summary line found (pattern $1)"
    else
        echo "  $line"
    fi
}
echo
echo "extended suite: ${elapsed}s, exit status ${status}"
echo "exit criteria of the Build plan:"
summary "^criterion: the target examples run" "the target examples run"
summary "^criterion: the proofs are the ones predicted" "the proofs are the ones predicted"
summary "^criterion: a Rust caller is held at the boundary" "a Rust caller is held at the boundary"
summary "^criterion: an author is told why" "an author is told why"
summary "^criterion: a crate can be checked by the kernel alone" "a crate can be checked by the kernel alone"
summary "^criterion: checking is deterministic" "checking is deterministic"
summary "^criterion: Locus is never more permissive than rustc" "Locus is never more permissive than rustc"
summary "^random programs \(extended\): [0-9]+ generated" "what is checked is what runs"
summary "^criterion: the trusted base is written down" "the trusted base is written down and tested as such"
summary "^hand-built: [0-9]+ triples" "  kernel soundness, hand-built proofs"
summary "^theory: [0-9]+ triples" "  kernel soundness, the theory"
summary "^corpus: [0-9]+ triples" "  kernel soundness, the corpus"
summary "^criterion: the parser is robust" "the parser is robust, and total for stated reasons"
summary "^test parser_fuzz::|^test random_token_sequences_give_an_ast_or_a_diagnostic_without_a_panic \.\.\. ok" "  parser fuzz, token sequences"
summary "^test edited_examples_give_an_ast_or_a_diagnostic_without_a_panic \.\.\. ok" "  parser fuzz, edited files"
summary "^criterion: the legacy is gone" "the legacy is gone"
summary "^criterion: the suites are usable" "the suites are usable"
echo "  $fast; extended suite ${elapsed}s in release"
if (( status == 0 )); then
    LOCUS_FAST_SECONDS="$fast_elapsed" target/release/locus bench --samples 5 > target/bench-latest.json
    python3 tools/bench.py check
    python3 tools/metrics.py collect --fast-log target/check-fast.log --extended-log "$log" --fast-seconds "$fast_elapsed" --source-fingerprint "$source_fingerprint" > target/status-record.json
    python3 tools/metrics.py check --fresh
    python3 tools/gate.py extended "$fast_elapsed" "$FAST_LIMIT_SECONDS"
fi
exit "$status"
