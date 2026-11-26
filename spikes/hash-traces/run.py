#!/usr/bin/env python3
"""Run the I04 Rust probes; no Locus compiler or proof checker is involved."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", help="path to a nightly rustc executable")
    args = parser.parse_args()
    if args.rustc:
        compiler = [args.rustc]
    elif shutil.which("rustup"):
        compiler = ["rustup", "run", "nightly", "rustc"]
    else:
        parser.error("pass --rustc /path/to/nightly/rustc, or install nightly via rustup")

    root = Path(__file__).resolve().parent
    print("I04 hashing probes: Rust runtime evidence only", flush=True)
    subprocess.run([*compiler, "--version"], check=True)
    with tempfile.TemporaryDirectory(prefix="locus-hash-traces-") as temporary:
        for stem in ("protocol_counterexamples", "direct_delegation"):
            binary = Path(temporary) / stem
            print(f"\nCompiling and running {stem}.rs", flush=True)
            subprocess.run(
                [*compiler, "--edition=2024", "--test", str(root / f"{stem}.rs"),
                 "-o", str(binary)],
                check=True,
            )
            subprocess.run([str(binary), "--nocapture"], check=True)
    print("\nPASS: 7 adversarial probes and 2 delegation/batch probes", flush=True)


if __name__ == "__main__":
    main()
