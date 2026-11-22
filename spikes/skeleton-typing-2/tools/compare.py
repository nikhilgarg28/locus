#!/usr/bin/env python3
"""Compares two reports of measure.py, binary by binary.

    compare.py LABEL_A REPORT_A.json LABEL_B REPORT_B.json [--all]

Prints each binary whose outcome (rustc's error codes in each expansion) or
printed lines differ between the two, with the first error of each, and then
counts. With `--all`, prints every binary.
"""
import json, sys


def main():
    args = [a for a in sys.argv[1:] if a != "--all"]
    show_all = "--all" in sys.argv
    la, pa, lb, pb = args
    a, b = json.load(open(pa)), json.load(open(pb))
    for package in a["packages"]:
        if package not in b["packages"]:
            continue
        ba, bb = a["packages"][package]["bins"], b["packages"][package]["bins"]
        same = changed = 0
        both_a = both_b = 0
        for name in sorted(ba):
            if name not in bb:
                continue
            ea, eb = ba[name], bb[name]
            both_a += ea["got"] == ["ok", "ok"]
            both_b += eb["got"] == ["ok", "ok"]
            pra = (ea.get("prints") or [[], []])[0][1:]
            prb = (eb.get("prints") or [[], []])[0][1:]
            differs = ea["got"] != eb["got"] or pra != prb
            if differs:
                changed += 1
            else:
                same += 1
            if differs or show_all:
                def cell(e, pr):
                    got = "ok" if e["got"] == ["ok", "ok"] else (
                        e["got"][0] if e["got"][0] == e["got"][1] else " / ".join(e["got"]))
                    return got + (f" prints {' | '.join(pr)}" if pr else "")
                print(f"== {name}: {la}: {cell(ea, pra)}; {lb}: {cell(eb, prb)}")
                for label, e in ((la, ea), (lb, eb)):
                    for mode in ("runtime", "check"):
                        errs = e["errors"][mode]
                        if errs and (mode == "runtime" or e["got"][0] == "ok"):
                            x = errs[0]
                            where = f"{x['place'][1]}:{x['place'][2]}" if x["place"] else "?"
                            print(f"  {label}, {mode}: {where}: error[{x['code']}]: {x['message'][:140]}")
        print(f"{package}: {same + changed} files; compile in both expansions: {la} {both_a}, {lb} {both_b}; "
              f"outcome or output differs in {changed}")


main()
