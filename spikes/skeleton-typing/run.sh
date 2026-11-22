#!/usr/bin/env bash
# Reproduces every result in README.md. Needs stable rustc 1.97, Cargo and
# python3; rustfmt for `expansions`; and `rustup component add rustc-dev
# llvm-tools` for the driver (`fallback` and `agree`).
#
#   ./run.sh [all|probes|coverage|variants|fallback|agree|diagnostics|cost|expansions]
#
# Everything is built under this spike's own target/, one directory for each
# configuration and each expansion, and reports are written under out/. The
# runtime expansion is stock `cargo build`; the checking expansion is the
# same command with RUSTFLAGS="--cfg __locus_check".
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"
section="${1:-all}"
want() { [ "$section" = all ] || [ "$section" = "$1" ]; }
export RUST_BACKTRACE=0 CARGO_TERM_COLOR=never
unset RUSTFLAGS LOCUS_SPIKE_ALL_TYPED LOCUS_SPIKE_IN_TRAIT LOCUS_SPIKE_EXPANSION \
    LOCUS_SPIKE_NO_SKELETON LOCUS_SPIKE_FALLBACK_UNTYPED LOCUS_SPIKE_LITERAL_INT \
    LOCUS_SPIKE_PIECES LOCUS_CHECK_UNTYPED LOCUS_CALLS_DIR
SETS="corpus ghost-lets cases skel more"
report() { echo "out/$1/report-$(echo "$SETS" | tr ' ' '-').json"; }

echo "rustc: $(rustc --version)"
echo "cargo: $(cargo --version)"
start=$SECONDS

# ----------------------------------------------------------------- probes
# The encodings, written by hand in plain Rust, before any macro.
if want probes; then
    echo
    echo "## Hand probes of the encodings"
    mkdir -p target/probes
    for p in p1 p2 p4; do
        rustc --edition 2024 "probes/$p.rs" -o "target/probes/$p" 2>&1 | grep -E '^(error|warning: unused)' | head -5
        echo "probes/$p.rs: $("target/probes/$p" | paste -sd'|' -)"
    done
    for p in p3 p5 p6; do
        echo "probes/$p.rs: $(rustc --edition 2024 "probes/$p.rs" -o "target/probes/$p" 2>&1 |
            grep -E '^error\[|^[0-9 ]+\| .*(arg\(&b\)|Node\(n\))|^note: multiple' | sed 's/  */ /g' | paste -sd'|' -)"
    done
fi

# --------------------------------------------------------------- coverage
# The five sets, with this spike's lowering and with the first mode's:
# - corpus/ and ghost-lets/: the litmus examples of the first spike;
# - cases/ and more/: its own cases (more/ was written for its second mode,
#   and its `expect` lines say what that mode should do);
# - skel/: the cases of this spike.
if want coverage; then
    echo
    echo "## The first mode (first-mode/: its library and macro, unchanged)"
    python3 tools/measure.py first --workspace first-mode --quiet corpus ghost-lets cases
    python3 tools/summary.py out/first/report-corpus-ghost-lets-cases.json | grep -E '^## |litmus'
    echo
    echo "## This spike's lowering: every file"
    python3 tools/measure.py sk $SETS
    echo
    echo "## Pieces of logical code, and ghost lets"
    python3 tools/summary.py "$(report sk)"
    echo
    echo "## What changed from the first mode"
    python3 tools/compare.py first out/first/report-corpus-ghost-lets-cases.json \
        skeleton "$(report sk)"
fi

# --------------------------------------------------------------- variants
# One switch at a time, each against the default: what compiles differently.
if want variants; then
    [ -f "$(report sk)" ] || python3 tools/measure.py sk --quiet $SETS > /dev/null
    variant() { # NAME DESCRIPTION ARGS..
        local name="$1" what="$2"
        shift 2
        echo
        echo "## Variant: $what"
        python3 tools/measure.py "$name" --quiet "$@" $SETS > /dev/null
        python3 tools/compare.py default "$(report sk)" "$name" "$(report "$name")"
    }
    variant homog "no operator on an Int and a Nat (feature homogeneous-ops)" --features locus/homogeneous-ops
    variant args "a Nat place may be given where an Int is wanted (feature widen-args)" --features locus/widen-args
    variant litint "a lone literal in an unannotated let is an Int (LOCUS_SPIKE_LITERAL_INT=1)" --env LOCUS_SPIKE_LITERAL_INT=1
    variant intrait "a trait's stubs are items of the lowered trait (LOCUS_SPIKE_IN_TRAIT=1)" --env LOCUS_SPIKE_IN_TRAIT=1
fi

# The driver, for the two sections that need it.
driver=""
if want fallback || want agree; then
    if (cd driver && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$here/target/driver" cargo +stable build -q); then
        driver="$here/target/driver/debug/locus-skeleton-driver"
    else
        echo "the driver is not built (rustc-dev is not installed): fallback and agree are skipped"
    fi
fi

# --------------------------------------------------------------- fallback
# A ghost local whose initializer has no skeleton keeps the first mode's
# lowering, and the driver reports every use of a local left untyped that
# is not logical code.
if want fallback && [ -n "$driver" ]; then
    echo
    echo "## Fallback, checked: initializers with no skeleton (skel/f*), the first mode's guess"
    python3 tools/measure.py checked --driver --env LOCUS_CHECK_UNTYPED=1 --prefix f skel
    echo
    echo "## The same, every such local left untyped (LOCUS_SPIKE_FALLBACK_UNTYPED=1)"
    python3 tools/measure.py checked-untyped --driver --env LOCUS_CHECK_UNTYPED=1 \
        --env LOCUS_SPIKE_FALLBACK_UNTYPED=1 --prefix f skel
    echo
    echo "## g13, g14, g15 of the first spike with no skeleton at all (LOCUS_SPIKE_NO_SKELETON=1), the first mode's guess"
    python3 tools/measure.py checked-none --driver --env LOCUS_CHECK_UNTYPED=1 \
        --env LOCUS_SPIKE_NO_SKELETON=1 --prefix g1 cases | grep -A4 -E '^== g1[345]'
    echo
    echo "## The same, every such local left untyped"
    python3 tools/measure.py checked-none-untyped --driver --env LOCUS_CHECK_UNTYPED=1 \
        --env LOCUS_SPIKE_NO_SKELETON=1 --env LOCUS_SPIKE_FALLBACK_UNTYPED=1 --prefix g1 cases |
        grep -A4 -E '^== g1[345]'
    echo
    echo "## The check on every file, with skeletons: what it adds to rustc's errors"
    python3 tools/measure.py checked-all --driver --env LOCUS_CHECK_UNTYPED=1 --quiet $SETS > /dev/null
    python3 tools/compare.py default "$(report sk)" checked "$(report checked-all)"
fi

# ------------------------------------------------------------------ agree
# The driver lists, for each call, method call and associated constant that
# the source wrote, the item rustc resolved it to. The two expansions of a
# file correspond only if their listings are the same.
if want agree && [ -n "$driver" ]; then
    echo
    echo "## Both programs agree: the call listings of the two expansions"
    python3 tools/measure.py calls --driver --calls --quiet $SETS
    echo
    echo "### The nine cases of the first spike that the brief names"
    python3 - <<'EOF'
import json
r = json.load(open("out/calls/report-corpus-ghost-lets-cases-skel-more.json"))["packages"]["more"]["bins"]
for name in sorted(r):
    if name.split("-")[0] in ("r01", "r02", "r03", "r04", "r09", "r10", "r11", "r13", "r14"):
        e = r[name]
        calls = e.get("calls")
        if calls:
            same = "the same" if calls["runtime"] == calls["check"] else "DIFFERENT"
            print(f"== {name}: both expansions compile; {len(calls['runtime'])} lines in each listing, {same}; prints {e['prints'][0][1:]}")
        else:
            print(f"== {name}: got runtime={e['got'][0]} check={e['got'][1]}: no listing to compare")
EOF
    echo
    echo "### r02 and r04 with the feature widen-args, under which they compile"
    python3 tools/measure.py calls-args --driver --calls --features locus/widen-args --prefix r0 more |
        grep -E '^== r0[24]|^more:'
    echo
    echo "### The listing of r10, runtime expansion and then checking expansion"
    dir="$(ls -d out/calls/calls-*/more | tail -1)"
    cat "$dir/runtime/r10_inherent_beats_trait.calls"
    echo "--"
    cat "$dir/check/r10_inherent_beats_trait.calls"
    echo
    echo "### The listing of t01, runtime expansion (the checking expansion's is the same)"
    dir="$(ls -d out/calls/calls-*/cases | tail -1)"
    cat "$dir/runtime/t01_two_traits.calls"
    cmp -s "$dir/runtime/t01_two_traits.calls" "$dir/check/t01_two_traits.calls" && echo "(identical)"
fi

# ------------------------------------------------------------ diagnostics
if want diagnostics; then
    [ -f "$(report sk)" ] || python3 tools/measure.py sk --quiet $SETS > /dev/null
    echo
    echo "## Diagnostics: three deliberate type errors in logical code, rustc's whole messages"
    for bin in e01-nat-for-seq e02-unknown-field e03-missing-operator; do
        python3 tools/show.py "$(report sk)" "$bin"
    done
    echo "## Some of the others"
    for bin in x01-literal-alone x05-generic-parameter x06-snapshot-expected x09-missing-arm \
        x12-no-model x14-view-imported c06-unbounded-parameter c07-two-snap-headers g18-snap-mismatch \
        c03-snap-in-header after-move; do
        python3 tools/show.py "$(report sk)" "$bin" runtime 2
    done
fi

# ------------------------------------------------------------------- cost
if want cost; then
    echo
    echo "## Cost: build time with and without skeletons"
    python3 tools/cost.py corpus cases
    python3 tools/gen-scale.py 500
    echo "scale/src/bin/scale.rs: $(head -1 scale/src/bin/scale.rs | sed 's|^//! ||')"
    python3 tools/cost.py --check scale
    python3 tools/cost.py scale
fi

# ------------------------------------------------------------- expansions
# What `locus!` writes, for the README. `LOCUS_SPIKE_EXPANSION=dir` makes it
# write each expansion to dir/<file>.<line>.rs.
if want expansions; then
    echo
    echo "## What locus! writes"
    mkdir -p out/expansions
    touch corpus/src/bin/*.rs cases/src/bin/t01-two-traits.rs skel/src/bin/a0*.rs
    LOCUS_SPIKE_EXPANSION="$here/out/expansions" cargo build -q -p corpus --keep-going --bins \
        --target-dir target/expansions > /dev/null 2>&1
    LOCUS_SPIKE_EXPANSION="$here/out/expansions" cargo build -q -p skel --keep-going --bin a04-match \
        --bin a02-places --bin a05-operators --target-dir target/expansions > /dev/null 2>&1
    for f in logical-fields returned-borrow checked-indices match-every-arm a02-places a05-operators; do
        echo "### $f"
        rustfmt --edition 2024 < "$(ls out/expansions/$f.*.rs | head -1)" | grep -v '^\s*#\[' |
            sed 's/:: locus :: //g; s/::locus:://g; s/::core::[a-z]*:://g'
    done
fi

echo
echo "run.sh $section: $((SECONDS - start)) seconds"
