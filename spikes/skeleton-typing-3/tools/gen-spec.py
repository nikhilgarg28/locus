#!/usr/bin/env python3
"""Writes the four specifications under each conversion policy, from the
master texts in spec/master/, and counts what each policy makes the source
write that RFC 0002's implicit rules do not.

A master text marks each such place:

    [[P2:TEXT]]         written where `Nat` to `Int` is explicit (`as Int`)
    [[LIT:TEXT]]        written where a literal that nothing stands beside is an `Int`
    [[LITX:TEXT]]       the same, and also written where a literal takes the type that
                        is expected of an argument or a result: the literal is an
                        operand's branch, of which nothing is expected
    [[B2P:TEXT]]        written where a `Bool` is not a proposition implicitly (`=== true`)
    [[CMP:A||B]]        A where a comparison gives a `Prop`, B where it gives a `Bool`
    [[P4:TEXT]]         written under every policy: `model(..)` at an argument
                        whose parameter's type is a type parameter

Markers stay on their line, so line N is line N in every version.

    gen-spec.py            writes the packages and prints the table
"""
import json, os, re, sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# name: (package, P2 explicit, comparisons give Prop, Bool implicit, literals strict, features, env)
CONFIGS = {
    "p2x-v3": ("spec", True, True, True, True, "", {}),
    "p2x-v2": ("spec-p2x-v2", True, True, False, True, "locus/p3-v2", {}),
    "p2x-v0": ("spec-p2x-v0", True, False, True, True, "locus/p3-v0", {}),
    "p2i-v3": ("spec-p2i-v3", False, True, True, True, "locus/mixed-ops", {}),
    "p2i-v2": ("spec-p2i-v2", False, True, False, True, "locus/mixed-ops,locus/p3-v2", {}),
    "p2i-v0": ("spec-p2i-v0", False, False, True, True, "locus/mixed-ops,locus/p3-v0", {}),
    # RFC 0002's rules, as nearly as the switches give them.
    "rfc": ("spec-rfc", False, False, True, False, "locus/mixed-ops,locus/p3-v0",
            {"LOCUS_SPIKE_LITERALS": "expected"}),
}
MARK = re.compile(r"\[\[(P2|LITX|LIT|B2P|P4):((?:(?!\[\[|\]\]).)*)\]\]")
CMP = re.compile(r"\[\[CMP:((?:(?!\|\||\]\]|\[\[CMP).)*)\|\|((?:(?!\]\]|\[\[).)*)\]\]")


def render(text, p2x, prop, implicit, strict):
    counts = {"as Int": [], "a literal's type": [], "=== true": [], "Bool and Prop bridge": [],
              "model(x)": []}
    out = []
    for n, line in enumerate(text.split("\n"), 1):
        # innermost markers first, then the alternation around them
        def simple(m):
            kind, body = m.group(1), m.group(2)
            keep = {"P2": p2x, "LIT": strict, "LITX": True, "B2P": not implicit, "P4": True}[kind]
            return f"\x01{kind}\x02{body}\x03" if keep else ""
        line = MARK.sub(simple, line)
        def alt(m):
            a, b = m.group(1), m.group(2)
            return f"\x01CMP\x02{a}\x03" if prop else b
        line = CMP.sub(alt, line)
        # what the chosen alternative kept
        for kind, cause in (("P2", "as Int"), ("LIT", "a literal's type"), ("LITX", "a literal's type"),
                            ("B2P", "=== true"),
                            ("CMP", "Bool and Prop bridge"), ("P4", "model(x)")):
            hits = line.count(f"\x01{kind}\x02")
            if kind == "P4":
                hits //= 2  # `model(` and `)`
            counts[cause] += [n] * hits
        line = re.sub(r"\x01[A-Z0-9]+\x02", "", line).replace("\x03", "")
        out.append(line)
    return "\n".join(out), counts


def main():
    table = {}
    names = sorted(f for f in os.listdir(f"{HERE}/spec/master") if f.endswith(".rs"))
    for config, (package, p2x, prop, implicit, strict, features, env) in CONFIGS.items():
        os.makedirs(f"{HERE}/{package}/src/bin", exist_ok=True)
        manifest = open(f"{HERE}/skel/Cargo.toml").read().replace('name = "skel"', f'name = "{package}"')
        with open(f"{HERE}/{package}/Cargo.toml", "w") as f:
            f.write(manifest)
        for name in names:
            text, counts = render(open(f"{HERE}/spec/master/{name}").read(), p2x, prop, implicit, strict)
            path = f"{HERE}/{package}/src/bin/{name}"
            if not os.path.exists(path) or open(path).read() != text:
                open(path, "w").write(text)
            table.setdefault(name[:-3], {})[config] = counts
    with open(f"{HERE}/spec/master/counts.json", "w") as f:
        json.dump({"configs": {k: {"package": v[0], "features": v[5], "env": v[6]} for k, v in CONFIGS.items()},
                   "counts": table}, f, indent=1)
    if "--quiet" in sys.argv:
        return
    causes = ["as Int", "a literal's type", "=== true", "Bool and Prop bridge", "model(x)"]
    print("Places where the source writes what RFC 0002's implicit rules do not, by cause")
    print("(as Int / a literal's type / === true / Bool and Prop bridge / model(x)), then the total:")
    print(f"{'':18}" + "".join(f"{c:>16}" for c in CONFIGS))
    for name, per in table.items():
        cells = []
        for c in CONFIGS:
            k = [len(per[c][x]) for x in causes]
            cells.append(f"{'/'.join(map(str, k))}={sum(k)}")
        print(f"{name:18}" + "".join(f"{x:>16}" for x in cells))
    print()
    for name, per in table.items():
        for c in ("p2x-v3", "p2x-v2"):
            lines = {cause: sorted(set(v)) for cause, v in per[c].items() if v}
            print(f"{name}, {c}: lines " + "; ".join(f"{cause}: {v}" for cause, v in lines.items()))


main()
