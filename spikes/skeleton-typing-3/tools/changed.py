#!/usr/bin/env python3
"""Round 2's suite under this round's library and lowering: every binary
whose outcome (rustc's error codes in each expansion) or printed lines
differ from round 2's, with a cause read from rustc's messages.

    changed.py NEW_REPORT.json BASELINE.json...
"""
import json, re, sys

CAUSES = [
    ("param", "a logic function declares a parameter with a runtime type: now an error from lowering"),
    ("snapshot-trait", "the source names round 2's trait `Snapshot`, which is gone"),
    ("unbounded", "a type parameter, or an associated type of one, is read with no `Model` bound: none is inserted"),
    ("no-model", "a type that nothing declares a model for is read: there is no opaque fallback"),
    ("names", "compiles in both rounds and prints other type names: an integer's snapshot is a `Nat` or an `Int`, and a declared type's snapshot has its fields"),
    ("now-ok", "now compiles"),
    ("other-errors", "rejected in both rounds, with other errors"),
    ("other", "other"),
]


def messages(e):
    return [x["message"] for x in e["errors"]["runtime"]]


def cause(o, e):
    new = messages(e)
    ok_o, ok_e = o["got"] == ["ok", "ok"], e["got"] == ["ok", "ok"]
    if ok_o and ok_e:
        return "names"
    if ok_e:
        return "now-ok"
    if any("a logic function's parameters are logical types" in m for m in new):
        return "param" if ok_o or not any("a logic function's parameters" in m for m in messages(o)) else "other-errors"
    if any("`Snapshot`" in m for m in new):
        return "snapshot-trait"
    read = [re.match(r"`(.*)` cannot be read in logical code", m) for m in new]
    read = [m.group(1) for m in read if m]
    if read and ok_o:
        return "unbounded" if any(re.fullmatch(r"[A-Z]\w?|<\w+ as \w+>::\w+", t) for t in read) else "no-model"
    if not ok_o:
        return "other-errors"
    return "other"


def main():
    new = json.load(open(sys.argv[1]))["packages"]
    old = {}
    for path in sys.argv[2:]:
        for package, p in json.load(open(path))["packages"].items():
            old.setdefault(package, p["bins"])
    by_cause = {}
    totals = []
    for package, p in new.items():
        if package not in old:
            continue
        changed = both_o = both_e = 0
        for name in sorted(p["bins"]):
            if name not in old[package]:
                continue
            o, e = old[package][name], p["bins"][name]
            both_o += o["got"] == ["ok", "ok"]
            both_e += e["got"] == ["ok", "ok"]
            pro = (o.get("prints") or [[], []])[0][1:]
            pre = (e.get("prints") or [[], []])[0][1:]
            if o["got"] == e["got"] and pro == pre:
                continue
            changed += 1
            c = cause(o, e)
            def cell(x):
                return "ok" if x["got"] == ["ok", "ok"] else (x["got"][0] if x["got"][0] == x["got"][1] else " / ".join(x["got"]))
            first = next(iter(e["errors"]["runtime"]), None)
            was = next(iter(o["errors"]["runtime"]), None)
            detail = ""
            if first:
                where = f"{first['place'][1]}:{first['place'][2]}" if first["place"] else "?"
                detail = f" | now {where}: {first['message'][:120]}"
            elif was:
                detail = f" | round 2 said: {was['message'][:120]}"
            elif pro != pre:
                detail = f" | prints {' | '.join(pre)[:170]}"
            by_cause.setdefault(c, []).append(f"{package}/{name}: {cell(o)} -> {cell(e)}{detail}")
        totals.append(f"{package}: {len(p['bins'])} files; compile in both expansions: round 2 {both_o}, round 3 {both_e}; "
                      f"outcome or output differs in {changed}")
    for key, title in CAUSES:
        if key in by_cause:
            print(f"### {len(by_cause[key])}: {title}")
            for line in by_cause[key]:
                print("  " + line)
    print("\n".join(totals))
    print(f"all: outcome or output differs in {sum(len(v) for v in by_cause.values())}: "
          + "; ".join(f"{k} {len(by_cause[k])}" for k, _ in CAUSES if k in by_cause))


main()
