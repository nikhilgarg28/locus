#!/bin/sh
# Runs the spike: the tests in both builds, then the programs that must not
# compile. Prints one line per result.
set -eu
cd "$(dirname "$0")"

echo "== unchecked build: Perm<T> is zero-sized"
cargo test --quiet -- --nocapture 2>&1 | grep -E "^sizes|test result: .* [1-9][0-9]* passed|FAILED|panicked" || true

echo "== checked build: every primitive asserts its requires"
RUSTFLAGS="--cfg checked" cargo test --quiet --target-dir target/checked -- --nocapture 2>&1 \
    | grep -E "^checked|test result: .* [1-9][0-9]* passed|FAILED" || true

echo "== programs rustc must reject"
cargo build --quiet
lib=$(ls target/debug/libpermissioned_pointers_spike.rlib)
expect() {
    out=$(rustc --edition 2024 --crate-type lib --emit metadata -o target/case.rmeta \
        --extern permissioned_pointers_spike="$lib" -L target/debug/deps "cases/$1.rs" 2>&1) && {
        echo "ACCEPTED (unexpected): $1"
        return
    }
    if echo "$out" | grep -q "error\[$2\]\|$2"; then
        echo "rejected with $2: $1"
    else
        echo "rejected, but not with $2: $1"
        echo "$out" | head -5
    fi
}
expect use-after-free E0382
expect copy-permission E0382
expect two-mutable-borrows E0499
expect write-while-shared E0502
expect drop-consumes-permission E0507
expect destructure-type-with-drop E0509
expect not-send E0277
expect unsafe-in-a-collection "usage of an \`unsafe\` block"

echo "== unsafe outside src/perm.rs"
grep -n "unsafe" src/pbox.rs src/pvec.rs | grep -v "forbid(unsafe_code)\|no \`unsafe\` code" || echo "none"
