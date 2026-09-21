#!/usr/bin/env bash
# The checks every commit must pass: formatting, clippy, and the tests, whose
# randomized parts use fixed seeds and small counts.
#
#   tools/check.sh              the fast form, for every commit
#   tools/check.sh --extended   before a milestone: sets LOCUS_EXTENDED, under
#                               which the randomized tests run a hundred times
#                               as many cases, and tests in release mode so
#                               that those counts are feasible
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

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

cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
if [[ -n "$extended" ]]; then
    export LOCUS_EXTENDED=1
    cargo test --locked --offline --release
else
    cargo test --locked --offline
fi
