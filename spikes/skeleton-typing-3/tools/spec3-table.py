#!/usr/bin/env python3
"""The four specifications, round 2's text (spec/src/bin, the default
policy) against round 3's (spec3/src/bin): every line of round 3's that is
not round 2's, with the cause of each place on it. The causes are written
here by hand, line by line; the tool checks that the list is complete, that
is, that it names exactly the code lines that `difflib` finds changed.

    spec3-table.py            prints the table and the lines
"""
import difflib, os, sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CAUSES = ["bound", "opaque", "snap", "model", "param", "method", "elements", "result", "import", "less"]
TITLE = {
    "bound": "a `T: Model` bound",
    "opaque": "an `#[opaque]` mark",
    "snap": "`snap(..)`",
    "model": "`model(..)`",
    "param": "a logic function's parameter, from a runtime type",
    "method": "a method call moved from a place to its snapshot",
    "elements": "other: `.elements()` on a vector's snapshot",
    "result": "other: a library method's result carries its fact (`.0`)",
    "import": "other: Locus's `Vec` is imported",
    "less": "(written no longer: `model(x)` of a place)",
}
# file: {line of round 3's text: [one cause for each place]}
PLACES = {
    "s1-stack": {
        9: ["import"],
        19: ["param", "elements"],
        25: ["param", "elements"],
        36: ["method", "snap"],
        41: ["result"],
        48: ["method", "snap"],
        49: ["method", "snap", "method", "snap"],
        51: ["less"],
        60: ["result"],
        65: ["result"],
        69: ["method", "snap"],
    },
    "s2-sorted-vec": {
        9: ["import"],
        44: ["param", "elements"],
        56: ["less"],
        60: ["result"],
        78: ["import"],
    },
    "s3-binary-search": {},
    "s4-container": {
        10: ["import"],
        52: ["param"],
        57: ["bound"],
        60: ["elements"],
        63: ["bound"],
        64: ["elements"],
        68: ["elements"],
        69: ["elements", "elements"],
        77: ["elements"],
        79: ["less"],
        82: ["less"],
        83: ["less"],
        103: ["result"],
    },
}


def code(line):
    s = line.strip()
    return bool(s) and not s.startswith("//")


def main():
    total = {c: 0 for c in CAUSES}
    rows, lines_out, ok = [], [], True
    all_lines = changed_lines = 0
    for name, places in PLACES.items():
        a = open(f"{HERE}/spec/src/bin/{name}.rs").read().split("\n")
        b = open(f"{HERE}/spec3/src/bin/{name}.rs").read().split("\n")
        changed = set()
        for tag, _, _, j1, j2 in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes():
            if tag in ("replace", "insert"):
                changed.update(j + 1 for j in range(j1, j2) if code(b[j]))
        if changed != set(places):
            ok = False
            print(f"{name}: THE LIST IS NOT THE DIFF: only in the diff {sorted(changed - set(places))}, "
                  f"only in the list {sorted(set(places) - changed)}")
        count = {c: 0 for c in CAUSES}
        for line, causes in places.items():
            for c in causes:
                count[c] += 1
                total[c] += 1
        all_lines += len(b) - 1
        changed_lines += len(places)
        rows.append((name, len(b) - 1, len(places), count))
        by = {}
        for line, causes in sorted(places.items()):
            for c in dict.fromkeys(causes):
                by.setdefault(c, []).append(line)
        lines_out.append(f"{name}: " + ("; ".join(f"{c}: lines {by[c]}" for c in CAUSES if c in by) or "nothing"))
    head = f"{'':<18}{'lines':>6}{'changed':>8}" + "".join(f"{c:>9}" for c in CAUSES)
    print(head)
    for name, n, ch, count in rows:
        print(f"{name:<18}{n:>6}{ch:>8}" + "".join(f"{count[c]:>9}" for c in CAUSES))
    print(f"{'all four':<18}{all_lines:>6}{changed_lines:>8}" + "".join(f"{total[c]:>9}" for c in CAUSES))
    written = sum(v for c, v in total.items() if c != "less")
    print(f"places where round 3 writes something round 2 did not: {written}; places where it writes less: {total['less']}")
    print("columns: " + "; ".join(f"{c} = {TITLE[c]}" for c in CAUSES))
    print("\n".join(lines_out))
    sys.exit(0 if ok else 1)


main()
