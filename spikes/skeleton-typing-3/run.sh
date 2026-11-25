#!/usr/bin/env bash
# Reproduces every result in README.md, and writes run.log when run as
#
#   ./run.sh 2>&1 | tee run.log
#
# Needs stable rustc 1.97, Cargo and python3; and `rustup component add
# rustc-dev llvm-tools` for the driver (`forgery`, `agree`).
#
#   ./run.sh [all|cases|specs|corpus|forgery|plain|suite|agree|messages]
#
# Everything is built under this spike's own target/, one directory for each
# configuration and each expansion, and reports are written under out/. The
# runtime expansion is stock `cargo build`; the checking expansion is the
# same command with RUSTFLAGS="--cfg __locus_check".
#
# Round 2's conversion policy and its switches are unchanged. This round's:
#
#   LOCUS_SPIKE_DEFERRED=1              the conditional form of a generated implementation
#                                       (capture-family c11): a field whose type has no model
#                                       is an error only when the struct is read
#   --features locus/no-std-vec-model   the standard library's own `Vec` has no model
#   LOCUS_MODELS_DIR=dir                (the driver) lists every implementation of `Model`
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"
section="${1:-all}"
want() { [ "$section" = all ] || [ "$section" = "$1" ]; }
export RUST_BACKTRACE=0 CARGO_TERM_COLOR=never
unset RUSTFLAGS LOCUS_SPIKE_ALL_TYPED LOCUS_SPIKE_HIDDEN_TRAIT LOCUS_SPIKE_EXPANSION \
    LOCUS_SPIKE_NO_SKELETON LOCUS_SPIKE_FALLBACK_UNTYPED LOCUS_SPIKE_LITERALS \
    LOCUS_SPIKE_DEFERRED LOCUS_SPIKE_PIECES LOCUS_CHECK_UNTYPED LOCUS_CALLS_DIR LOCUS_MODELS_DIR
OLD="corpus ghost-lets cases skel more"                  # the first round's five sets
R2="spec policy spec-omit two-app corpus-explicit"       # the second round's
NEW="family spec3 ghost-lets3 two-plain"                 # this round's
report() { echo "out/$1/report-$(echo "$2" | tr ' ' '-').json"; }
show() { python3 tools/show.py "$@" | grep -v '^$'; }

echo "rustc: $(rustc --version)"
echo "cargo: $(cargo --version)"
start=$SECONDS
python3 tools/gen-spec.py --quiet       # round 2's specifications, as round 2 wrote them
python3 tools/gen-omissions.py
python3 tools/make-explicit.py > /dev/null

# ------------------------------------------------------------------ cases
# This round's cases: each file's first line says what the two compilations
# must do. a: the family and capture; c: Locus's `Vec`; d: the reviewer's
# seven required cases; e: closures, function items, trait objects; g: the
# litmus example that regressed; v: validity.
if want cases; then
    echo
    echo "## The cases of this round (family/src/bin), both expansions"
    python3 tools/measure.py sk family | grep -E '^== |^  both print|^family' | cut -c1-520
    python3 - <<'EOF'
import json
b = json.load(open("out/sk/report-family.json"))["packages"]["family"]["bins"]
ok = sum(1 for e in b.values() if e["verdict"].startswith("as expected") and "PRINTS" not in e["verdict"] and "DIFFERENT" not in e["verdict"])
print(f"family: {len(b)} cases; as their first line says, in both expansions: {ok}; compile and run in both: {sum(1 for e in b.values() if e['got'] == ['ok', 'ok'])}")
EOF
    python3 tools/summary.py "$(report sk family)" | grep -E '^## |  all |family: ' | head -6
fi

# ------------------------------------------------------------------ specs
if want specs; then
    echo
    echo "## The four specifications"
    echo "### Round 2's text (spec/), unedited, under the new rules"
    python3 tools/measure.py sk spec | grep -E '^== |^  runtime' | cut -c1-300
    echo "### Rewritten for the new rules (spec3/), both expansions"
    python3 tools/measure.py sk spec3 | grep -E '^== |^  both print|^spec3' | cut -c1-300
    python3 tools/summary.py "$(report sk spec3)" | grep -E '  all '
    echo "### With the standard library's own Vec given no model (--features locus/no-std-vec-model)"
    python3 tools/measure.py novec --features locus/no-std-vec-model spec3 | grep -E '^== ' | cut -c1-200
    echo "### What the source now writes that round 2's did not (tools/spec3-table.py)"
    python3 tools/spec3-table.py
fi

# ----------------------------------------------------------------- corpus
if want corpus; then
    echo
    echo "## The litmus corpus"
    python3 tools/measure.py sk --quiet corpus corpus-explicit ghost-lets ghost-lets3 > /dev/null
    echo "### Unedited (corpus/, ghost-lets/) and with round 2's edits (corpus-explicit/), against round 2"
    python3 tools/compare.py round2 baseline/round2-corpus-explicit.json round3 "$(report sk "corpus corpus-explicit ghost-lets ghost-lets3")" | cut -c1-230
    echo "### The edited copy of this round (ghost-lets3/: one line of one file), against ghost-lets/"
    diff <(cd ghost-lets/src/bin && grep -n '' *.rs) <(cd ghost-lets3/src/bin && grep -n '' *.rs) | grep -E '^[<>]' | cut -c1-200
    python3 tools/summary.py "$(report sk "corpus corpus-explicit ghost-lets ghost-lets3")" | grep -E '^## |litmus|  all '
    echo "### The same example with the field marked #[opaque] instead (family/g2, g2w)"
    python3 tools/measure.py sk family | grep -E '^== g2' -A1 | grep -E '^== |^  runtime' | cut -c1-220
    echo "### With the standard library's own Vec given no model"
    python3 tools/measure.py novec --quiet --features locus/no-std-vec-model corpus-explicit ghost-lets3 > /dev/null
    python3 tools/compare.py default "$(report sk "corpus corpus-explicit ghost-lets ghost-lets3")" no-std-vec-model "$(report novec "corpus-explicit ghost-lets3")" | cut -c1-200
    echo "### The repository's whole litmus tree, from its text: logic functions with a runtime-typed parameter (tools/litmus-scan.py)"
    python3 tools/litmus-scan.py /home/user/locus/litmus 2>/dev/null | cut -c1-1400 || echo "(the repository's litmus tree is not at /home/user/locus/litmus)"
fi

# ---------------------------------------------------------------- forgery
# `Model` is an `unsafe trait`. Two lowered crates, one that is not, and the
# driver's listing of every implementation.
if want forgery; then
    echo
    echo "## Forgery and authorization: two lowered crates and one that is not"
    python3 tools/measure.py sk two-app two-plain | grep -E '^== |^  (both print|runtime)' | cut -c1-520
    show "$(report sk "two-app two-plain")" forge-without-unsafe runtime 1
    if (cd driver && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$here/target/driver" cargo +stable build -q); then
        rm -rf out/models-listing && mkdir -p out/models-listing
        touch two-lib/src/lib.rs   # so that the library crate is compiled, and listed, in this run
        python3 tools/measure.py models --driver --quiet --env LOCUS_MODELS_DIR="$here/out/models-listing" two-app two-plain family > /dev/null
        echo "### The driver's listing: every implementation of Model each crate sees (crate | type | origin)"
        for c in two_lib app forge_with_unsafe; do
            echo "-- $c.models: $(grep -c "| Locus's library" out/models-listing/$c.models) of Locus's library, and:"
            grep -v "| Locus's library" out/models-listing/$c.models
        done
        echo "-- over every crate of two-app, two-plain and family that compiled: implementations outside Locus's library, by origin"
        cat out/models-listing/*.models | grep -v "| Locus's library" | sort -u | sed 's/.* | //' | sort | uniq -c
        echo "-- those not written by lowering:"
        cat out/models-listing/*.models | grep "BY HAND" | sort -u
    else
        echo "the driver is not built (rustc-dev is not installed): the listing is skipped"
    fi
fi

# ------------------------------------------------------------------ plain
# The default is the plain form. The conditional form is behind a switch.
if want plain; then
    echo
    echo "## A field whose type has no model: the plain form (default) and the conditional form (LOCUS_SPIKE_DEFERRED=1)"
    python3 tools/measure.py sk --quiet $OLD $R2 $NEW > /dev/null
    python3 tools/measure.py deferred --quiet --env LOCUS_SPIKE_DEFERRED=1 $OLD $R2 $NEW > /dev/null
    echo "### Declarations at which the plain form reports a field's type (tools/opaque-needed.py)"
    python3 tools/opaque-needed.py "$(report sk "$OLD $R2 $NEW")"
    echo "### Every file whose outcome differs between the two forms"
    python3 tools/compare.py plain "$(report sk "$OLD $R2 $NEW")" deferred "$(report deferred "$OLD $R2 $NEW")" | cut -c1-230
    echo "### The message of the plain form, at the declaration"
    show "$(report sk "$OLD $R2 $NEW")" a07-foreign-field-unmarked runtime 1
    echo "### The message of the conditional form, where the struct is first read"
    show "$(report deferred "$OLD $R2 $NEW")" a07r-foreign-field-unmarked-read runtime 1
fi

# ------------------------------------------------------------------ suite
# Round 2's whole suite under the new library and lowering, against round
# 2's own results (baseline/round2-*.json, copied from its out/).
if want suite; then
    echo
    echo "## Round 2's suite under the new library and lowering"
    python3 tools/measure.py sk --quiet $OLD $R2 > /dev/null
    python3 tools/changed.py "$(report sk "$OLD $R2")" baseline/round2-old.json baseline/round2-policy.json \
        baseline/round2-two-app.json baseline/round2-spec-omit.json baseline/round2-corpus-explicit.json baseline/round2-calls.json
    echo "### The same suite with the standard library's own Vec given no model"
    python3 tools/measure.py novec --quiet --features locus/no-std-vec-model $OLD $R2 > /dev/null
    python3 tools/compare.py default "$(report sk "$OLD $R2")" no-std-vec-model "$(report novec "$OLD $R2")" | grep -E '^[a-z0-9-]+: '
fi

# ------------------------------------------------------------------ agree
if want agree; then
    if (cd driver && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$here/target/driver" cargo +stable build -q); then
        echo
        echo "## Both programs agree: the call listings of the two expansions, every accepted binary"
        touch two-lib/src/lib.rs
        python3 tools/measure.py calls --driver --calls --quiet $OLD $R2 $NEW
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
        dir="$(ls -d out/calls/calls-*/spec3 | tail -1)"
        echo "### The stack: where its calls resolve (runtime expansion; the checking one is the same)"
        grep -E 'room|__logic_model|elements' "$dir/runtime/s1_stack.calls" | head -6 | cut -c1-200
    else
        echo "the driver is not built (rustc-dev is not installed): agree is skipped"
    fi
fi

# --------------------------------------------------------------- messages
if want messages; then
    [ -f "$(report sk family)" ] || python3 tools/measure.py sk --quiet family > /dev/null
    echo
    echo "## rustc's messages, whole"
    for bin in a08-missing-bound-fn a09-missing-bound-impl a10-missing-bound-trait-method; do
        show "$(report sk family)" $bin runtime 1
    done
    for bin in a05e-associated-type-field-unbounded a03-wrapper-confused a06e-two-foreign-snapshots-mixed \
        a06f-opaque-field-selected-first a11-model-in-another-invocation a12-logic-fn-runtime-parameter f1e-forged-inside-locus \
        a13e-method-on-snapshot d2b-foreign-trait-generic-method d2c-foreign-trait-bound-added d6e-second-implementation \
        e1e-closure-where-model-is-asked e2e-trait-object-read e2f-trait-object-field-unmarked \
        c3e-index-mut-contract c4e-vec-push-without-model c5e-vec-standard-mutation \
        c6e-ghost-field-of-the-sequence-type c7e-bound-on-the-struct; do
        show "$(report sk family)" $bin runtime 1 | head -34
    done
    echo "### \`model(s)\` with no type written"
    show "$(report sk family)" m1e-model-without-a-type runtime 1 | head -30
fi

echo
echo "run.sh $section: $((SECONDS - start)) seconds"
