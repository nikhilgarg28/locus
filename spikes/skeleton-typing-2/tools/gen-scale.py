#!/usr/bin/env python3
"""Writes scale/src/bin/scale.rs: N functions, each with four pieces of
logical code (two `logic!`, a proposition in a `let` annotation, a `prove!`),
in forms that the first mode lowers too. For the cost of typing skeletons.

    gen-scale.py [N]
"""
import os, sys

n = int(sys.argv[1]) if len(sys.argv) > 1 else 500
here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
out = ["//! Written by tools/gen-scale.py: %d functions, %d pieces of logical code." % (n, 4 * n + 1),
       "use ::locus::locus;", "", "locus! {", "    logic fn double(n: Nat) -> Nat { n + n }", ""]
for i in range(n):
    out += [
        f"    fn f{i}(v: &Vec<u32>, n: u32) -> usize {{",
        "        let before = logic! { v.len() };",
        "        let twice = logic! { double(before) + n };",
        "        let p: @(before + n >= n) = _;",
        "        let q = prove!(twice == double(v.len()) + n && before <= twice);",
        f"        v.len() + {i}",
        "    }",
    ]
out += ["}", "", "fn main() {", '    println!("expansion: {}", ::locus::__expansion!());',
        "    let v = vec![1u32];",
        "    let mut total = 0;"]
out += [f"    total += f{i}(&v, 1);" for i in range(n)]
out += ['    println!("{total}");', "}", ""]
with open(f"{here}/scale/src/bin/scale.rs", "w") as f:
    f.write("\n".join(out))
