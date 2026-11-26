#!/bin/sh
# Run after installing nightly with miri and rust-src. No dependency downloads
# other than Miri's sysroot setup are required by this package itself.
set -eu
spike_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$spike_root"
export CARGO_TARGET_DIR="$spike_root/target/miri"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$spike_root/target/cache}"
cargo +nightly miri test --bin mutable-loans
