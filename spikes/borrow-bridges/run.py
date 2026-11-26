#!/usr/bin/env python3
"""Reproduce Rust compile/runtime probes and hand-written SMT queries offline."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
TARGET = ROOT / "target"
OUT = ROOT / "results"
TARGET.mkdir(exist_ok=True)
OUT.mkdir(exist_ok=True)
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rustc", default=shutil.which("rustc") or
                    "/workspace/locus-review/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc")
args = parser.parse_args()
records = []


def run(name, command, error=None, tokens=()):
    result = subprocess.run(list(map(str, command)), cwd=ROOT, text=True,
                            capture_output=True, timeout=120)
    output = result.stdout + result.stderr
    # Normalize only trailing whitespace so recorded logs pass git diff checks.
    (OUT / f"{name}.log").write_text(output.rstrip() + "\n" if output else "")
    passed = ((result.returncode != 0 and f"error[{error}]" in output)
              if error else (result.returncode == 0 and all(t in output for t in tokens)))
    records.append({"name": name, "command": list(map(str, command)),
                    "expected_error": error, "returncode": result.returncode,
                    "passed": passed})
    (OUT / "runs.json").write_text(json.dumps(records, indent=2) + "\n")
    print(f"{name}: {'PASS' if passed else 'FAIL'}", flush=True)
    if not passed:
        print(output)
        raise SystemExit(1)


run("rustc-version", [args.rustc, "--version", "--verbose"])
common = [args.rustc, "--edition=2024"]
lib = TARGET / "libborrow_bridge_probe.rlib"
run("library", [*common, "-Dwarnings", "--crate-type=rlib", "--crate-name=borrow_bridge_probe",
                "rust/lib.rs", "-o", lib])
link = ["--extern", f"borrow_bridge_probe={lib}"]
for name, flags in [("debug", []), ("optimized", ["-O"])]:
    binary = TARGET / f"tests-{name}"
    run(f"build-tests-{name}", [*common, *flags, "-Dwarnings", "--test", "rust/tests.rs", *link,
                              "-o", binary])
    run(f"tests-{name}", [binary, "--test-threads=1"], tokens=["14 passed; 0 failed"])

errors = {
    "reject_identity_provider": "E0119",
    "reject_foreign_blanket": "E0210",
    "reject_arbitrary_forwarding": "E0119",
    "reject_duplicate_provider": "E0119",
    "reject_reference_outlives_owner": "E0515",
    "reject_mutation_with_live_query": "E0502",
    "reject_forged_adapter": "E0277",
    "reject_generated_registration": "E0277",
}
for source in sorted((ROOT / "cases").glob("*.rs")):
    binary = TARGET / source.stem
    error = errors.get(source.stem)
    run(source.stem, [*common, source, *link, "-o", binary], error=error)
    if not error:
        run(source.stem + "-run", [binary])
run("laws", [sys.executable, ROOT / "check_laws.py"], tokens=["LAW QUERIES: 17 matched"])
print("PASS: 14 runtime tests in two profiles; 2 compile/run cases; 8 compiler rejections; 17 SMT queries")
