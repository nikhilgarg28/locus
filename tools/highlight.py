#!/usr/bin/env python3
"""Compatibility entry point for the shared Locus highlighter tests.

    python3 tools/highlight.py check

The website imports editors/highlight/locus.js directly. There is no embedded
copy to synchronize. The old `put` command is a deprecated no-op.
"""
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", nargs="?", default="check", choices=("check", "put"))
    args = parser.parse_args(argv)
    if args.command == "put":
        print("Deprecated: the website imports editors/highlight/locus.js directly; no files were changed.")
        return 0
    try:
        return subprocess.run(["node", str(ROOT / "editors/highlight/test.js")], cwd=ROOT).returncode
    except FileNotFoundError:
        print("Highlighter checks require Node.js; install Node.js 20 or newer.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
