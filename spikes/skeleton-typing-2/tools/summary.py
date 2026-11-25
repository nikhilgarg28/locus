#!/usr/bin/env python3
"""Counts the pieces of logical code in one or more reports of measure.py.

    summary.py REPORT.json...

For each package: how many pieces of each kind there are, how many have a
typing skeleton, in how many of those rustc reported an error (in either
expansion), and which. Then the ghost `let`s, by where their type comes from.
"""
import json, sys
from collections import Counter, defaultdict


def main():
    for path in sys.argv[1:]:
        report = json.load(open(path))
        for package, data in report["packages"].items():
            kinds = defaultdict(lambda: Counter())
            failed, untranslated, lets = [], [], Counter()
            let_errors = []
            files = Counter()
            for name, entry in sorted(data["bins"].items()):
                files["files"] += 1
                if entry["got"] == ["ok", "ok"]:
                    files["compile in both expansions"] += 1
                if not entry["same_pieces"]:
                    print(f"  NOTE {name}: the two expansions recorded different pieces")
                for p in entry["pieces"]:
                    at = f"{name}:{p['from'][0]}:{p['from'][1]}"
                    if p["kind"] == "ghost let":
                        source = p["status"]
                        if p["detail"] == "annotated":
                            source += " (the `let` has an annotation)"
                        else:
                            source += " (no annotation)"
                        lets[source] += 1
                        errs = sorted({(e[1], e[2]) for e in p["errors"]})
                        need = [e for e in errs if e[0] in ("E0282", "E0283")]
                        if need:
                            lets["... of which rustc asks for a type"] += 1
                            let_errors.append((at, source, need[0]))
                        continue
                    k = kinds[p["kind"]]
                    k["pieces"] += 1
                    if p["status"] == "translated":
                        k["translated"] += 1
                        if p["errors"]:
                            k["with an error"] += 1
                            modes = sorted({e[0] for e in p["errors"]})
                            first = p["errors"][0]
                            failed.append((at, p["kind"], "+".join(modes), first[1], first[2]))
                        else:
                            k["no error"] += 1
                    elif p["status"] == "not translated":
                        untranslated.append((at, p["kind"], p["detail"]))
                    elif p["status"] == "not attempted":
                        k["pieces"] -= 1
                        k["not attempted"] += 1
                    else:
                        k[p["status"]] += 1
            print(f"## {package} ({report['config']}): {files['files']} files, "
                  f"{files['compile in both expansions']} compile in both expansions")
            litmus = Counter()
            for entry in data["bins"].values():
                kind, got = entry["expect"][0], entry["got"]
                if kind == "litmus accept":
                    litmus["accepted"] += 1
                    litmus["accepted, both"] += got == ["ok", "ok"]
                elif kind == "litmus reject":
                    litmus["rejected"] += 1
                    litmus["rejected, both"] += got[0] != "ok" and got[1] != "ok"
                    litmus["rejected, checking alone"] += got[0] == "ok" and got[1] != "ok"
            if litmus:
                print(f"  accepted litmus examples: {litmus['accepted']}; compile in both expansions: "
                      f"{litmus['accepted, both']}")
                print(f"  rejected litmus examples: {litmus['rejected']}; rustc rejects them in both "
                      f"expansions: {litmus['rejected, both']}; in the checking expansion alone: "
                      f"{litmus['rejected, checking alone']}")
            total = Counter()
            for kind, k in sorted(kinds.items()):
                if k["not attempted"]:
                    print(f"  {kind:28} not attempted {k['not attempted']:3}")
                    continue
                total.update(k)
                print(f"  {kind:28} pieces {k['pieces']:3}  translated {k['translated']:3}  "
                      f"no error in either expansion {k['no error']:3}  with an error {k['with an error']:3}")
            print(f"  {'all':28} pieces {total['pieces']:3}  translated {total['translated']:3}  "
                  f"no error in either expansion {total['no error']:3}  with an error {total['with an error']:3}")
            if untranslated:
                print("  not translated:")
                for at, kind, why in untranslated:
                    print(f"    {at} ({kind}): {why}")
            if failed:
                print("  translated, and rustc reports an error in it:")
                for at, kind, modes, code, message in failed:
                    print(f"    {at} ({kind}; {modes}): {code}: {message[:140]}")
            print("  ghost `let`s initialized by a `logic!`, by where their type comes from:")
            for source, n in sorted(lets.items()):
                print(f"    {source}: {n}")
            for at, source, (code, message) in let_errors:
                print(f"    {at} ({source}): {code}: {message[:120]}")


main()
