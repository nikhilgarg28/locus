#!/usr/bin/env python3
"""Builds the four specifications under each conversion policy, in both
expansions, and prints the table of what each policy makes the source write.

    spec-matrix.py [--driver]

A cell is counted only if the file compiles in both expansions under that
policy with exactly what the cell counts; a file that does not is `FAILS`.
"""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
subprocess.run([sys.executable, f"{HERE}/tools/gen-spec.py", "--quiet"], check=True)
data = json.load(open(f"{HERE}/spec/master/counts.json"))
causes = ["as Int", "a literal's type", "=== true", "Bool and Prop bridge", "model(x)"]
ok = {}
for config, c in data["configs"].items():
    cmd = [sys.executable, f"{HERE}/tools/measure.py", f"spec-{config}", "--quiet"]
    if "--driver" in sys.argv:
        cmd += ["--driver", "--calls"]
    if c["features"]:
        cmd += ["--features", c["features"]]
    for k, v in c["env"].items():
        cmd += ["--env", f"{k}={v}"]
    cmd.append(c["package"])
    out = subprocess.run(cmd, capture_output=True, text=True, cwd=HERE).stdout
    report = json.load(open(f"{HERE}/out/spec-{config}/report-{c['package']}.json"))
    bins = report["packages"][c["package"]]["bins"]
    for name, e in bins.items():
        good = e["got"] == ["ok", "ok"] and "DIFFERENT" not in e["verdict"] and "EXITED" not in str(e.get("prints"))
        ok[(name, config)] = good
        if not good:
            first = (e["errors"]["runtime"] or e["errors"]["check"] or [{}])[0]
            where = first.get("place") or ["", "?", "?"]
            print(f"  {name} under {config}: {e['got']} {e['verdict']}: {where[1]}:{where[2]}: {first.get('message', '')[:110]}")
    for line in out.splitlines():
        if "listed" in line:
            print(f"  {config}: {line}")
print()
print("What the source writes that RFC 0002's implicit rules do not, in each specification:")
print("`as Int` / a literal's type / `=== true` / a Bool-and-Prop bridge / `model(x)` = total")
configs = list(data["configs"])
print(f"{'':18}" + "".join(f"{c:>17}" for c in configs))
totals = {c: [0] * len(causes) for c in configs}
for name, per in data["counts"].items():
    cells = []
    for c in configs:
        k = [len(per[c][x]) for x in causes]
        totals[c] = [a + b for a, b in zip(totals[c], k)]
        cells.append(f"{'/'.join(map(str, k))}={sum(k)}" if ok.get((name, c)) else "FAILS")
    print(f"{name:18}" + "".join(f"{x:>17}" for x in cells))
print(f"{'all four':18}" + "".join(f"{'/'.join(map(str, totals[c]))}={sum(totals[c])}".rjust(17) for c in configs))
print()
for name, per in data["counts"].items():
    for c in ("p2x-v3", "p2x-v2"):
        lines = {cause: sorted(set(v)) for cause, v in per[c].items() if v}
        print(f"{name}, {c}: " + ("; ".join(f"{cause}: lines {v}" for cause, v in lines.items()) or "nothing"))
