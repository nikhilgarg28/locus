#!/usr/bin/env python3
"""Writes spec-omit/: each of the four specifications (the default policy's
text) with ONE written conversion left out, two omissions for each file, to
see what rustc tells the programmer."""
import os

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OMISSIONS = [
    ("o1a", "s1-stack", "s.room() == cap as Int", "s.room() == cap", "`as Int` left out of a comparison"),
    ("o1b", "s1-stack", "r <==> self.len() == 0", "r == (self.len() == 0)",
     "the `bool` result compared with `==`, as RFC 0002's `Bool` comparison would be written"),
    ("o2a", "s2-sorted-vec", "            0 as Nat\n        } else {", "            0\n        } else {",
     "a literal result with no type, where the function returns a `Nat`"),
    ("o2b", "s2-sorted-vec", "below(xs.skip(1 as Nat), x)", "below(xs.skip(1), x)",
     "a literal argument with no type, where the parameter is a `Nat`"),
    ("o3a", "s3-binary-search", "@(mid as Int == lo as Int + (hi - lo) / 2)", "@(mid == lo as Int + (hi - lo) / 2)",
     "`as Int` left out on one side of `==`"),
    ("o3b", "s3-binary-search", "-> @(lo as Int + (hi - lo) / 2 >= lo as Int &&", "-> @(lo + (hi - lo) / 2 >= lo as Int &&",
     "`as Int` left out of an addition"),
    ("o4a", "s4-container", "total_of_pushed(before, model(x))", "total_of_pushed(before, x)",
     "`model(..)` left out at an argument whose parameter's type is a type parameter"),
    ("o4b", "s4-container", "shelf.items.take(0 as Nat)", "shelf.items.take(0)",
     "a literal argument with no type, where the parameter is a `Nat`"),
]
os.makedirs(f"{HERE}/spec-omit/src/bin", exist_ok=True)
manifest = open(f"{HERE}/skel/Cargo.toml").read().replace('name = "skel"', 'name = "spec-omit"')
open(f"{HERE}/spec-omit/Cargo.toml", "w").write(manifest)
for name, spec, old, new, what in OMISSIONS:
    text = open(f"{HERE}/spec/src/bin/{spec}.rs").read()
    assert text.count(old) == 1, (name, old)
    text = text.replace(old, new)
    lines = text.split("\n")
    assert lines[0].startswith("//! expect:") and lines[1].startswith("//! prints:")
    lines[0] = "//! expect: runtime=reject check=reject"
    lines[1] = f"//! OMISSION in {spec}.rs: {what}."
    path = f"{HERE}/spec-omit/src/bin/{name}-{spec[3:]}.rs"
    text = "\n".join(lines)
    if not os.path.exists(path) or open(path).read() != text:
        open(path, "w").write(text)
