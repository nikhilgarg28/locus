#!/usr/bin/env python3
"""From reports of measure.py made in the default (plain) form: the
declarations of structs and enums at which rustc reports that a field's type
cannot be read in logical code. Where that type is a type parameter the
declaration needs a bound, `T: Model`; otherwise it needs an `#[opaque]` mark
on the field, or a model for the field's type. Then the declarations that
lowering wrote the family for, by what it wrote.

    opaque-needed.py REPORT.json...
"""
import json, os, re, sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
total = bounds = 0
wrote = {}
for path in sys.argv[1:]:
    for package, p in json.load(open(path))["packages"].items():
        found, bound = [], []
        for e in p["bins"].values():
            for piece in e["pieces"]:
                if piece["kind"] == "family":
                    key = piece["status"] + (": " + piece["detail"] if piece["detail"] else "")
                    wrote[key] = wrote.get(key, 0) + 1
        for name, e in sorted(p["bins"].items()):
            src = f"{HERE}/{package}/src/bin/{name}.rs"
            if not os.path.exists(src):
                continue
            lines = open(src).read().split("\n")
            seen = set()
            for x in e["errors"]["runtime"]:
                m = re.match(r"`(.*)` cannot be read in logical code", x["message"])
                if not (m and x["place"]):
                    continue
                text = lines[x["place"][1] - 1] if x["place"][1] <= len(lines) else ""
                decl = re.search(r"\b(struct|enum)\s+(\w+)", text)
                if decl and (x["place"][1], m.group(1)) not in seen:
                    seen.add((x["place"][1], m.group(1)))
                    which = bound if re.fullmatch(r"[A-Z]\w?", m.group(1)) else found
                    which.append(f"{name}:{x['place'][1]} `{decl.group(2)}`: `{m.group(1)}`")
        total += len(found)
        bounds += len(bound)
        print(f"{package}: {len(found)} declarations need a mark" + (": " + "; ".join(found) if found else "")
              + (f"; {len(bound)} need a bound: " + "; ".join(bound) if bound else ""))
print(f"all: {total} declarations need an `#[opaque]` mark, and {bounds} a bound on the type")
print("declarations of structs and enums that lowering saw: " + "; ".join(f"{k} {v}" for k, v in sorted(wrote.items())))
