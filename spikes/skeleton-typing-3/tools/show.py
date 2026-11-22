#!/usr/bin/env python3
"""Prints rustc's whole messages for one binary from a report of measure.py.

    show.py REPORT.json BINARY [runtime|check] [MAX]
"""
import json, sys


def main():
    report = json.load(open(sys.argv[1]))
    name = sys.argv[2]
    modes = [sys.argv[3]] if len(sys.argv) > 3 and sys.argv[3] in ("runtime", "check") else ["runtime", "check"]
    limit = int(sys.argv[-1]) if sys.argv[-1].isdigit() else 99
    for package in report["packages"].values():
        if name in package["bins"]:
            for mode in modes:
                errs = package["bins"][name]["errors"][mode]
                print(f"### {name}, the {'checking' if mode == 'check' else 'runtime'} expansion: {len(errs)} errors")
                for e in errs[:limit]:
                    print(e["rendered"].rstrip())
                    print()


main()
