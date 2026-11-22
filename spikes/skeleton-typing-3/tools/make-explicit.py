#!/usr/bin/env python3
"""Writes corpus-explicit/: the corpus with what the default conversion
policy makes its source write, and counts the changed lines by cause.

Each edit is listed here with its cause. corpus/ itself is RFC 0002's text
(the litmus examples, copied unchanged), and compiles with the switches
that are nearest to RFC 0002's implicit rules.
"""
import os, sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EDITS = [
    # (file, old, new, cause, how many conversions)
    ("logical-closure-in-body.rs",
     "let all: @(forall |n: Int| n + x === x + n) = logic! { |n: Int| prove!(n + x === x + n) };",
     "let all: @(forall |n: Int| n + x as Int === x as Int + n) = logic! { |n: Int| prove!(n + x as Int === x as Int + n) };",
     "as Int", 4),
    ("logical-fields.rs", "Tree::Leaf => 0,", "Tree::Leaf => 0 as Nat,", "a literal's type", 1),
    ("match-every-arm.rs", "logic fn one_is_not_even(h: @Even(1)) -> @False {",
     "logic fn one_is_not_even(h: @Even(1 as Nat)) -> @False {", "a literal's type", 1),
    ("x-joined-logical-type.rs", "let d = logic! { double(3) };", "let d = logic! { double(3 as Nat) };",
     "a literal's type", 1),
    ("x-split-logical-type.rs", "let d = logic! { double(3) };", "let d = logic! { double(3 as Nat) };",
     "a literal's type", 1),
]


def main():
    src, dst = f"{HERE}/corpus/src/bin", f"{HERE}/corpus-explicit/src/bin"
    os.makedirs(dst, exist_ok=True)
    manifest = open(f"{HERE}/corpus/Cargo.toml").read().replace('name = "corpus"', 'name = "corpus-explicit"')
    open(f"{HERE}/corpus-explicit/Cargo.toml", "w").write(manifest)
    lines, conversions, files = {}, {}, set()
    for name in sorted(os.listdir(src)):
        text = open(f"{src}/{name}").read()
        for f, old, new, cause, n in EDITS:
            if f == name:
                assert text.count(old) == 1, (name, old)
                text = text.replace(old, new)
                lines[cause] = lines.get(cause, 0) + 1
                conversions[cause] = conversions.get(cause, 0) + n
                files.add(name)
        path = f"{dst}/{name}"
        if not os.path.exists(path) or open(path).read() != text:
            open(path, "w").write(text)
    total = len(os.listdir(src))
    print(f"corpus: {total} files; {len(files)} files and {sum(lines.values())} lines changed for the default policy")
    for cause in ("as Int", "a literal's type", "=== true", "Bool and Prop bridge"):
        print(f"  {cause}: {lines.get(cause, 0)} lines, {conversions.get(cause, 0)} places")


main()
