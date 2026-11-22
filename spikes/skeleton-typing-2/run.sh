#!/usr/bin/env bash
# Reproduces every result in README.md. Needs stable rustc 1.97, Cargo and
# python3; rustfmt for `expansions`; and `rustup component add rustc-dev
# llvm-tools` for the driver (`agree`).
#
#   ./run.sh [all|probes|policy|shapes|snapshots|suite|agree|diagnostics|expansions]
#
# Everything is built under this spike's own target/, one directory for each
# configuration and each expansion, and reports are written under out/. The
# runtime expansion is stock `cargo build`; the checking expansion is the
# same command with RUSTFLAGS="--cfg __locus_check".
#
# The DEFAULT is the owner's conversion policy: `Nat` to `Int` is written
# (`n as Int`), a comparison in logical code is a `Prop`, a `Bool` is a
# proposition where the grammar wants one, and a literal that nothing stands
# beside is an `Int`. The other policies are switches:
#
#   --features locus/mixed-ops     P2 implicit inside arithmetic and comparisons
#   --features locus/p3-v2         V2: no implicit `Bool` to `Prop` (`b === true`)
#   --features locus/p3-v0         V0: a comparison gives a `Bool`; `Bool` to `Prop` implicit
#   --features locus/p3-v1         V1: a comparison gives a `Bool`; `b === true` is written
#   LOCUS_SPIKE_LITERALS=expected  a literal argument takes the parameter's type
#   LOCUS_SPIKE_LITERALS=inferred  the first round's literals (rustc's inference)
#   LOCUS_SPIKE_SNAP_BOUNDS=off|mentions|calls   where lowering adds `T: Snapshot`
#   LOCUS_SPIKE_SNAP_FALLBACK=off  `snap(x)` has no opaque fallback
#   LOCUS_SPIKE_HIDDEN_TRAIT=1     a trait's stubs in a hidden trait beside it (round 1)
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"
section="${1:-all}"
want() { [ "$section" = all ] || [ "$section" = "$1" ]; }
export RUST_BACKTRACE=0 CARGO_TERM_COLOR=never
unset RUSTFLAGS LOCUS_SPIKE_ALL_TYPED LOCUS_SPIKE_HIDDEN_TRAIT LOCUS_SPIKE_EXPANSION \
    LOCUS_SPIKE_NO_SKELETON LOCUS_SPIKE_FALLBACK_UNTYPED LOCUS_SPIKE_LITERALS \
    LOCUS_SPIKE_SNAP_BOUNDS LOCUS_SPIKE_SNAP_FALLBACK LOCUS_SPIKE_PIECES LOCUS_CHECK_UNTYPED LOCUS_CALLS_DIR
OLD="corpus ghost-lets cases skel more"          # the first round's five sets
NEW="spec policy two-app corpus-explicit"        # this round's
report() { echo "out/$1/report-$(echo "$2" | tr ' ' '-').json"; }
# The switches that are nearest to the first round's defaults, and to RFC
# 0002's implicit rules.
R1LIKE="--features locus/mixed-ops,locus/p3-v0 --env LOCUS_SPIKE_LITERALS=inferred --env LOCUS_SPIKE_SNAP_BOUNDS=off --env LOCUS_SPIKE_SNAP_FALLBACK=off"

echo "rustc: $(rustc --version)"
echo "cargo: $(cargo --version)"
start=$SECONDS
python3 tools/gen-spec.py --quiet
python3 tools/gen-omissions.py
python3 tools/make-explicit.py > /dev/null

# ----------------------------------------------------------------- probes
# The encodings of this round, written by hand in plain Rust.
if want probes; then
    echo
    echo "## Hand probes of the encodings (probes2/)"
    mkdir -p target/probes2
    probe() { # FILE [--cfg X]: the first lines of rustc's errors, or what the binary prints
        local f="$1"
        shift
        local out
        out="$(rustc --edition 2024 "$@" "probes2/$f.rs" -o "target/probes2/$f" 2>&1 | grep -E '^error' -A 5 | grep -vE '^--$|^ *\|$' | head -"${LINES_SHOWN:-6}")"
        if [ -n "$out" ]; then echo "$out"; else "target/probes2/$f"; fi
    }
    echo "### q1: a logical enum's variants reached through the type alone"
    probe q1
    LINES_SHOWN=4 probe q1 --cfg missing
    echo "### q2 (P4): the callee takes each argument through a bound; the model wins for a type parameter"
    probe q2
    for c in backward mismatch widen; do LINES_SHOWN=5 probe q2 --cfg $c; done
    echo "### q3 (P5): a logic function on the runtime type is found first, then one on its model"
    probe q3
    LINES_SHOWN=3 probe q3 --cfg nomethod
    echo "### q5: making the message for an omitted conversion say what to write"
    for c in "op" "op --cfg dummy" "lit"; do echo "-- --cfg $c"; LINES_SHOWN=5 probe q5 --cfg $c; done
    echo "### s1 to s5: an opaque snapshot for a type that nothing declares one for"
    for p in s1 s2; do echo "-- $p"; LINES_SHOWN=12 probe $p | grep -E '^error|conflicting|first impl'; done
    echo "-- s3"; probe s3 && echo "s3: the fallback alone compiles"; LINES_SHOWN=6 probe s3 --cfg both
    echo "-- s4"; probe s4; LINES_SHOWN=9 probe s4 --cfg held
    echo "-- s5"; probe s5
fi

# ----------------------------------------------------------------- policy
if want policy; then
    echo
    echo "## The conversion policy"
    echo "### The four specifications under each policy, in both expansions (tools/spec-matrix.py)"
    python3 tools/spec-matrix.py
    echo
    echo "### The litmus corpus: lines changed for the default policy (tools/make-explicit.py)"
    python3 tools/make-explicit.py
    python3 tools/measure.py sk --quiet corpus corpus-explicit ghost-lets > /dev/null
    python3 tools/summary.py "$(report sk "corpus corpus-explicit ghost-lets")" | grep -E '^## |litmus|  all '
    python3 tools/measure.py p3v2 --quiet --features locus/p3-v2 corpus-explicit ghost-lets > /dev/null
    echo "the changed corpus and the ghost-let examples under the strict variant (p3-v2), against the default:"
    python3 tools/compare.py default "$(report sk "corpus corpus-explicit ghost-lets")" strict-v2 "$(report p3v2 "corpus-explicit ghost-lets")"
    python3 tools/measure.py rfc --quiet $R1LIKE corpus ghost-lets > /dev/null
    echo "corpus/ and ghost-lets/ as written (RFC 0002's text) under the default, against the switches nearest to RFC 0002:"
    python3 tools/compare.py rfc-like "$(report rfc "corpus ghost-lets")" default "$(report sk "corpus corpus-explicit ghost-lets")"
    echo
    echo "### The policy's cases (policy/src/bin/p*, v*), default"
    python3 tools/measure.py sk policy | grep -E '^== [pv]|^policy' | cut -c1-240
    echo
    echo "### The same cases under each other policy, against the default"
    variant() { # NAME ARGS..
        local name="$1"
        shift
        python3 tools/measure.py "$name" --quiet "$@" policy > /dev/null
        echo "-- $name"
        python3 tools/compare.py default "$(report sk policy)" "$name" "$(report "$name" policy)" | grep -vE '^== [den]' | cut -c1-230
    }
    variant p3-v2 --features locus/p3-v2
    variant p3-v0 --features locus/p3-v0
    variant p3-v1 --features locus/p3-v1
    variant mixed-ops --features locus/mixed-ops
    variant literals-expected --env LOCUS_SPIKE_LITERALS=expected
    echo
    echo "### rustc's messages where a conversion is left out"
    python3 tools/show.py "$(report sk policy)" p2a-omitted-as-int runtime 4
    echo "(under p3-v2:)"
    python3 tools/show.py "$(report p3-v2 policy)" v7-mixed-connective runtime 2
    python3 tools/show.py "$(report p3-v2 policy)" v2-bool-field-and-local runtime 1
    echo "(under the default:)"
    python3 tools/show.py "$(report sk policy)" v8-bool-still-written runtime 3
    python3 tools/show.py "$(report sk policy)" v4b-returns-bool runtime 1
    python3 tools/show.py "$(report sk policy)" p4b-type-parameter-omitted runtime 1
    python3 tools/show.py "$(report sk policy)" p1a-literal-forward runtime 1
    python3 tools/show.py "$(report sk policy)" p1c-literal-argument runtime 1
    echo
    echo "### Two omissions in each specification (spec-omit/, from tools/gen-omissions.py)"
    python3 tools/measure.py sk spec-omit | grep -E '^== ' | cut -c1-200
    for b in o1a-stack o1b-stack o2a-sorted-vec o2b-sorted-vec o3a-binary-search o3b-binary-search \
        o4a-container o4b-container; do
        python3 tools/show.py "$(report sk spec-omit)" "$b" runtime 1 | grep -v '^$' | head -24
    done
fi

# ----------------------------------------------------------------- shapes
# Positions and shapes that the first round did not cover.
if want shapes; then
    echo
    echo "## Positions and shapes (policy/src/bin/d*, two-lib/, two-app/)"
    python3 tools/measure.py sk policy | grep -E '^== d' -A5 | grep -E '^== |^  (runtime|both)' | cut -c1-330
    python3 tools/summary.py "$(report sk policy)" | grep -E "field's type|signature|  all "
    python3 tools/measure.py sk two-app | cut -c1-330
    python3 tools/measure.py hidden --quiet --env LOCUS_SPIKE_HIDDEN_TRAIT=1 two-app > /dev/null
    echo "the two crates with the first round's hidden trait (LOCUS_SPIKE_HIDDEN_TRAIT=1):"
    python3 tools/compare.py default "$(report sk two-app)" hidden-trait "$(report hidden two-app)" | cut -c1-230
fi

# -------------------------------------------------------------- snapshots
if want snapshots; then
    echo
    echo "## Snapshots for every type"
    echo "### Where lowering adds the bound \`Snapshot\`, in the first round's five sets and in this round's"
    for mode in off mentions calls; do
        python3 tools/measure.py "snap-$mode" --quiet --env LOCUS_SPIKE_SNAP_BOUNDS=$mode --env LOCUS_SPIKE_SNAP_FALLBACK=off \
            $OLD policy spec > /dev/null
    done
    python3 tools/snapshot-bounds.py "$(report snap-off "$OLD policy spec")" "$(report snap-mentions "$OLD policy spec")" \
        "$(report snap-calls "$OLD policy spec")"
    echo
    echo "### The opaque fallback of \`snap(x)\`: the default against LOCUS_SPIKE_SNAP_FALLBACK=off"
    python3 tools/measure.py sk --quiet $OLD policy spec > /dev/null
    python3 tools/compare.py no-fallback "$(report snap-calls "$OLD policy spec")" default "$(report sk "$OLD policy spec")" | cut -c1-230
    echo
    echo "### The cases (policy/src/bin/n*), default"
    python3 tools/measure.py sk-n --prefix n policy | grep -E '^== |^  (runtime|both)' | cut -c1-300
    echo
    echo "### The first round's snapshot cases, and the litmus example that regressed"
    python3 tools/measure.py sk-c --prefix c skel | grep -E '^== ' | cut -c1-200
    python3 tools/measure.py sk-gl --prefix gl-snap ghost-lets | grep -E '^== ' | cut -c1-200
fi

# ------------------------------------------------------------------ suite
# The first round's whole suite under the new defaults, against the first
# round's own results (baseline/round1-sk.json, from running its run.sh).
if want suite; then
    echo
    echo "## The first round's suite"
    echo "### With the switches nearest to the first round's defaults: what the new lowering changed"
    python3 tools/measure.py r1like --quiet $R1LIKE $OLD > /dev/null
    python3 tools/compare.py round1 baseline/round1-sk.json r1like "$(report r1like "$OLD")" | cut -c1-230
    step() { # NAME DESCRIPTION ARGS..
        local name="$1" what="$2"
        shift 2
        echo
        echo "### From there, one change of policy at a time: $what"
        python3 tools/measure.py "$name" --quiet "$@" $OLD > /dev/null
        python3 tools/compare.py r1like "$(report r1like "$OLD")" "$name" "$(report "$name" "$OLD")" | cut -c1-230
    }
    step step-p1 "P1, a literal that nothing stands beside is an Int" --features locus/mixed-ops,locus/p3-v0 \
        --env LOCUS_SPIKE_SNAP_BOUNDS=off --env LOCUS_SPIKE_SNAP_FALLBACK=off
    step step-p2 "P2, Nat to Int is written" --features locus/p3-v0 --env LOCUS_SPIKE_LITERALS=inferred \
        --env LOCUS_SPIKE_SNAP_BOUNDS=off --env LOCUS_SPIKE_SNAP_FALLBACK=off
    step step-p3 "P3, a comparison is a Prop (V3)" --features locus/mixed-ops --env LOCUS_SPIKE_LITERALS=inferred \
        --env LOCUS_SPIKE_SNAP_BOUNDS=off --env LOCUS_SPIKE_SNAP_FALLBACK=off
    step step-snap "snapshot bounds and the opaque fallback" --features locus/mixed-ops,locus/p3-v0 \
        --env LOCUS_SPIKE_LITERALS=inferred
    echo
    echo "### The new defaults, all together, against the first round"
    python3 tools/measure.py sk --quiet $OLD > /dev/null
    python3 tools/summary.py "$(report sk "$OLD")" | grep -E '^## |litmus|  all |not translated|^    [a-z0-9-]+:[0-9]+:[0-9]+ \(.*\): '
    python3 tools/compare.py round1 baseline/round1-sk.json default "$(report sk "$OLD")" | grep -E '^[a-z-]+: '
fi

# ------------------------------------------------------------------ agree
if want agree; then
    if (cd driver && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$here/target/driver" cargo +stable build -q); then
        echo
        echo "## Both programs agree: the call listings of the two expansions, every accepted binary"
        touch two-lib/src/lib.rs   # so that the library is compiled, and listed, in this run
        python3 tools/measure.py calls --driver --calls --quiet $OLD $NEW
        python3 - <<'EOF'
import json, glob
r = json.load(open(sorted(glob.glob("out/calls/report-corpus-ghost-lets-cases-skel-more-*.json"))[-1]))["packages"]
both = lines = differ = 0
for p in r.values():
    for e in p["bins"].values():
        c = e.get("calls")
        if c:
            both += 1
            lines += len([x for x in c["runtime"] if x])
            differ += c["runtime"] != c["check"]
print(f"all sets: binaries that both expansions accept: {both}; lines in each listing: {lines}; binaries with a difference: {differ}")
EOF
        dir="$(ls -d out/calls/calls-*/two-app | tail -1)"
        echo "### The two crates: the library's listing and the application's"
        for c in two_lib app; do
            cmp -s "$dir/runtime/$c.calls" "$dir/check/$c.calls" &&
                echo "$c: $(grep -c . "$dir/runtime/$c.calls") lines in each expansion's listing, identical"
        done
        grep -E 'two_lib::(max_of|OrderSpec|Rational::__logic_scaled|Bounded::__logic)' "$dir/runtime/app.calls" | head -8 | cut -c1-220
        dir="$(ls -d out/calls/calls-*/spec | tail -1)"
        echo "### The stack: where its method calls resolve (runtime expansion; the checking one is the same)"
        grep -E 'room|last' "$dir/runtime/s1_stack.calls" | head -6 | cut -c1-200
    else
        echo "the driver is not built (rustc-dev is not installed): agree is skipped"
    fi
fi

# ------------------------------------------------------------ diagnostics
if want diagnostics; then
    [ -f "$(report sk "$OLD")" ] || python3 tools/measure.py sk --quiet $OLD > /dev/null
    python3 tools/measure.py sk --quiet policy > /dev/null
    echo
    echo "## Diagnostics: five deliberate errors in logical code, rustc's whole messages, before and after"
    for bin in e01-nat-for-seq e02-unknown-field e03-missing-operator; do
        echo "#### BEFORE (the first round): $bin"
        python3 tools/show.py baseline/round1-sk.json "$bin" runtime
        echo "#### AFTER: $bin"
        python3 tools/show.py "$(report sk "$OLD")" "$bin" runtime
    done
    for bin in e04-bool-for-prop-parameter e05-missing-arm; do
        echo "#### BEFORE (the first round's code, built in a copy of it): $bin"
        python3 tools/show.py baseline/round1-e04-e05.json "$bin" runtime
        echo "#### AFTER: $bin"
        python3 tools/show.py "$(report sk policy)" "$bin" runtime
    done
    echo "## Some of the others, after"
    for bin in x05-generic-parameter x06-snapshot-expected x12-no-model after-move; do
        python3 tools/show.py "$(report sk "$OLD")" "$bin" runtime 1
    done
fi

# ------------------------------------------------------------- expansions
# What `locus!` writes, for the README. `LOCUS_SPIKE_EXPANSION=dir` makes it
# write each expansion to dir/<file>.<line>.rs.
if want expansions; then
    echo
    echo "## What locus! writes"
    mkdir -p out/expansions
    touch policy/src/bin/*.rs spec/src/bin/*.rs
    LOCUS_SPIKE_EXPANSION="$here/out/expansions" cargo build -q -p policy --keep-going --bin p5-method-order \
        --bin d3-nested-pattern --bin d2-trait-method-contract --bin d1-field-proposition --bin d5-models \
        --target-dir target/expansions > /dev/null 2>&1
    LOCUS_SPIKE_EXPANSION="$here/out/expansions" cargo build -q -p spec --bin s1-stack \
        --target-dir target/expansions > /dev/null 2>&1
    for f in p5-method-order d3-nested-pattern d2-trait-method-contract d1-field-proposition d5-models s1-stack; do
        echo "### $f"
        rustfmt --edition 2024 < "$(ls out/expansions/$f.*.rs | head -1)" | grep -v '^\s*#\[' |
            sed 's/:: locus :: //g; s/::locus:://g; s/::core::[a-z]*:://g'
    done
fi

echo
echo "run.sh $section: $((SECONDS - start)) seconds"
