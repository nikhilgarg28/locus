#!/usr/bin/env python3
"""Writes ghost-lets/src/bin/gl-<name>.rs: every litmus example that has a
`let x = logic! { .. };` with no type annotation, its full text unchanged
inside `locus! { ... }`, and a `main` that prints which expansion it is.

    python3 -I gen-ghost-lets.py /path/to/locus/litmus

The files it wrote are kept in this directory, so nothing here reads the
repository when it is built. run.sh says whether they still match it.
"""
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
LET = re.compile(r"let (mut )?[a-z_0-9]+ = logic!")

HEADER = """//! `litmus/{path}`, its full text copied unchanged inside
//! `locus! {{ ... }}`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {{
"""
FOOTER = """
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
"""


def main():
    litmus = pathlib.Path(sys.argv[1]).resolve()
    check = "--check" in sys.argv
    out = HERE / "ghost-lets" / "src" / "bin"
    stale, count, lets = [], 0, 0
    for f in sorted(litmus.rglob("*.lc")):
        text = f.read_text()
        n = len(LET.findall(text))
        if n == 0:
            continue
        count += 1
        lets += n
        path = f.relative_to(litmus)
        want = HEADER.format(path=path.as_posix()) + text.rstrip("\n") + "\n" + FOOTER
        name = "gl-" + f.name.split(".")[0] + ".rs"
        if check:
            if not (out / name).exists() or (out / name).read_text() != want:
                stale.append(name)
        else:
            (out / name).write_text(want)
    if check and stale:
        print("ghost-lets/src/bin DIFFERS from the litmus corpus:", ", ".join(stale))
    elif check:
        print(f"ghost-lets/src/bin holds the {count} litmus examples with such a `let`, verbatim ({lets} `let`s)")
    else:
        print(f"wrote {count} files, {lets} `let`s")


if __name__ == "__main__":
    main()
