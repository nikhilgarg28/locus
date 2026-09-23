#!/usr/bin/env python3
"""Write a successful gate measurement, then emit its completion receipt."""
import datetime
import json
import pathlib
import subprocess
import sys

BANNER = "LOCUS GATE COMPLETE"


def complete(mode, seconds, limit, root=None):
    root = pathlib.Path(root or pathlib.Path(__file__).resolve().parent.parent)
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, text=True,
                          capture_output=True, check=False).stdout.strip() or None
    record = {"schema_version": 1, "mode": mode, "fast_seconds": seconds,
              "limit_seconds": limit, "over_limit": seconds > limit,
              "commit": head,
              "recorded_at": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    history = root / "target" / "gate-history.jsonl"
    history.parent.mkdir(parents=True, exist_ok=True)
    # One append, only after every command in the selected gate succeeded.
    with history.open("a", encoding="utf-8") as output:
        output.write(json.dumps(record, sort_keys=True) + "\n")
    print(f"{BANNER}: {mode}; fast suite {seconds}s; limit {limit}s", flush=True)


if __name__ == "__main__":
    if len(sys.argv) != 4 or sys.argv[1] not in ("fast", "extended"):
        sys.exit("usage: gate.py <fast|extended> <seconds> <limit>")
    complete(sys.argv[1], int(sys.argv[2]), int(sys.argv[3]))
