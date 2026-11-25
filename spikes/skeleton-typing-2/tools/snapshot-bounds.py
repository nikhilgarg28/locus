#!/usr/bin/env python3
"""Counts where lowering adds the bound `Snapshot`, from reports of
measure.py made with LOCUS_SPIKE_SNAP_BOUNDS=off, mentions and calls.

    snapshot-bounds.py OFF.json MENTIONS.json CALLS.json
"""
import json, sys
from collections import Counter

off, mentions, calls = (json.load(open(p))["packages"] for p in sys.argv[1:4])


def needs(report):
    """Binaries where rustc says a type parameter or a type has no snapshot."""
    out = {}
    for package, data in report.items():
        for name, e in data["bins"].items():
            n = len({(x["place"][1], x["place"][2]) for x in e["errors"]["runtime"]
                     if "has no snapshot type" in x["message"] and x["place"]})
            if n:
                out[(package, name)] = n
    return out


def inserted(report):
    c, files = Counter(), Counter()
    cannot = []
    for package, data in report.items():
        for name, e in data["bins"].items():
            for p in e["pieces"]:
                if p["kind"] != "snapshot bound":
                    continue
                if p["status"] == "inserted":
                    c[(package, p["detail"])] += 1
                    files[package] += 0
                else:
                    cannot.append(f"{package}/{name}:{p['from'][0]} ({p['detail']})")
    return c, cannot


n_off, n_m, n_c = needs(off), needs(mentions), needs(calls)
print(f"with no bound added (off): {len(n_off)} files, {sum(n_off.values())} places where rustc reports "
      "that a type has no snapshot type:")
for (package, name), n in sorted(n_off.items()):
    after = "compiles" if calls[package]["bins"][name]["got"] == ["ok", "ok"] else (
        "still no snapshot type" if (package, name) in n_c else f"other errors: {calls[package]['bins'][name]['got'][0]}")
    m = "" if (package, name) not in n_m or (package, name) in n_c else " (not with `mentions` alone)"
    print(f"  {package}/{name}: {n} places; with the bounds added (calls): {after}{m}")
for label, report in (("mentions", mentions), ("calls", calls)):
    c, cannot = inserted(report)
    per = Counter()
    for (package, detail), n in c.items():
        per[package] += n
    kinds = Counter()
    for (package, detail), n in c.items():
        kinds[detail] += n
    print(f"{label}: bounds added in {sum(c.values())} places ({', '.join(f'{n} {k}' for k, n in sorted(kinds.items()))}); "
          f"by set: {', '.join(f'{p} {n}' for p, n in sorted(per.items()))}")
    print(f"  where lowering sees the need and cannot add the bound: {cannot or 'none'}")
    still = {k: v for k, v in needs(report).items()}
    print(f"  files where rustc still reports a missing snapshot type: {sorted(f'{p}/{n}' for p, n in still)}")
