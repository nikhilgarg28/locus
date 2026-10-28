#!/usr/bin/env python3
"""Question 1: compares the two lexers' token dumps in DIR, for the files
both accepted (DIR/<name>.fallback.txt beside DIR/<name>.rustc.txt). A
difference only in a punctuation token's Spacing is counted apart, with the
token and the one after it.

    python3 -I compare-dumps.py DIR
"""
import collections
import pathlib
import re
import sys

d = pathlib.Path(sys.argv[1])
same, spacing, other = 0, collections.Counter(), []
files_spacing = 0
for f in sorted(d.glob("*.fallback.txt")):
    r = f.with_name(f.name.replace(".fallback.txt", ".rustc.txt"))
    if not r.exists():
        continue
    a, b = f.read_text().splitlines(), r.read_text().splitlines()
    if a == b:
        same += 1
        continue
    strip = lambda ls: [re.sub(r" (Joint|Alone)$", "", x) for x in ls]
    if strip(a) != strip(b):
        other.append(f.name.replace(".fallback.txt", ""))
        continue
    files_spacing += 1
    for i, (x, y) in enumerate(zip(a, b)):
        if x != y:
            nxt = a[i + 1].strip() if i + 1 < len(a) else "(end)"
            spacing[f"`{x.split()[1]}` {x.split()[2]} (fallback), {y.split()[2]} (rustc), before {nxt}"] += 1
print(f"token dumps of files both lexers accept: {same} identical, {files_spacing} differ only in Spacing, {len(other)} differ otherwise")
for k, n in sorted(spacing.items()):
    print(f"  {n:3} x {k}")
for name in other:
    print(f"  differ otherwise: {name}")
