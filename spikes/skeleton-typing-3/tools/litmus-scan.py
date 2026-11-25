#!/usr/bin/env python3
"""Counts, in a tree of litmus examples (`*.lc`), the logic functions that
declare a parameter with a runtime type, by the test that this round's
lowering applies (`runtime_parameter` in locus-macros/src/lib.rs), from the
text alone. Nothing is compiled. It reads the tree and writes nothing.

    litmus-scan.py DIR
"""
import os, re, sys

RUNTIME = {"u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
           "bool", "char", "f32", "f64", "Vec", "Box", "Option", "String", "str"}


def split_top(s):
    parts, depth, cur = [], 0, ""
    for i, c in enumerate(s):
        if c in "<([{":
            depth += 1
        elif c in ">)]}" and not (c == ">" and i > 0 and s[i - 1] in "-="):
            depth -= 1
        if c == "," and depth <= 0:
            parts.append(cur)
            cur = ""
        else:
            cur += c
    if cur.strip():
        parts.append(cur)
    return parts


def runtime(ty, adts):
    ty = ty.strip()
    if ty[:1] in "&*[":
        return True
    if ty.startswith("("):
        return any(runtime(p, adts) for p in split_top(ty[1:-1]))
    m = re.match(r"(\w+)", ty)
    return bool(m) and (m.group(1) in RUNTIME or m.group(1) in adts)


def main():
    root = sys.argv[1]
    files = fns = with_fn = 0
    kinds = {}
    hits = []
    for d, _, names in sorted(os.walk(root)):
        for n in sorted(names):
            if not n.endswith(".lc"):
                continue
            files += 1
            text = open(os.path.join(d, n)).read()
            # structs and enums that are not `#[derive(Logical)]`
            adts = set()
            for m in re.finditer(r"((?:#\[[^\]]*\]\s*)*)(?:pub(?:\([a-z]+\))?\s+)?(?:struct|enum)\s+(\w+)", text):
                if "Logical" not in m.group(1):
                    adts.add(m.group(2))
            found = 0
            for m in re.finditer(r"\blogic\s+fn\s+(\w+)\s*(?:<[^>(]*>)?\s*\(", text):
                depth, k = 1, m.end()
                while k < len(text) and depth:
                    depth += text[k] == "("
                    depth -= text[k] == ")"
                    k += 1
                for p in split_top(text[m.end():k - 1]):
                    if ":" in p and not p.strip().startswith(("self", "&self", "mut self")):
                        if runtime(p.split(":", 1)[1], adts):
                            found += 1
                            break
                    elif "self" in p.split(":")[0] and (p.strip().startswith("&") or ":" in p and runtime(p.split(":", 1)[1], adts)):
                        found += 1
                        break
            if found:
                with_fn += 1
                fns += found
                kind = n.split(".")[-2]
                kinds[kind] = kinds.get(kind, 0) + 1
                hits.append(f"{os.path.relpath(os.path.join(d, n), root)} ({found})")
    print(f"{files} examples; {fns} logic functions in {with_fn} files declare a parameter with a runtime type "
          f"(by outcome: {', '.join(f'{k} {v}' for k, v in sorted(kinds.items()))})")
    print("  " + "; ".join(hits))


main()
