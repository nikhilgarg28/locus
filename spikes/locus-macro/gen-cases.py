#!/usr/bin/env python3
"""Writes cases/src/bin/<name>.rs for question 2: each litmus example's full
text, unchanged, inside `locus! { ... }`, and a `main` that prints each of its
`//~ run:` calls as `<call> => <Debug of the value>`.

    python3 -I gen-cases.py          # write the files
    python3 -I gen-cases.py --check  # exit 1 if a file differs from what it would write
"""
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
LITMUS = HERE.parent.parent / "litmus"

# (bin name, litmus path): chosen for the syntax each exercises.
CASES = [
    ("ghost-parameters", "logical-layer/ghost-parameters.accept.lc"),
    ("checked-indices", "contracts/checked-indices.accept.lc"),
    ("returned-borrow", "contracts/returned-borrow.accept.lc"),
    ("final-value-at-return", "contracts/final-value-at-return.accept.lc"),
    ("derive-eq-with-proof-field", "logical-layer/derive-eq-with-proof-field.accept.lc"),
    ("match-every-arm", "proofs/match-every-arm.accept.lc"),
    ("logical-fields", "logical-layer/logical-fields.accept.lc"),
    ("same-is-a-proposition", "logical-layer/same-is-a-proposition.accept.lc"),
    ("logic-calls-logic-function", "logical-layer/logic-calls-logic-function.accept.lc"),
    ("wrapped-precondition", "export/wrapped-precondition.accept.lc"),
    ("logical-closure-in-body", "observation/logical-closure-in-body.accept.lc"),
    ("snapshot-survives-assignment", "proofs/snapshot-survives-assignment.accept.lc"),
    ("reproved-after-assignment", "proofs/reproved-after-assignment.accept.lc"),
    ("locus-macro-inside-rust-macro", "compatibility/locus-macro-inside-rust-macro.accept.lc"),
    ("tokens-in-macro-input", "compatibility/tokens-in-macro-input.accept.lc"),
    ("underscore-in-rust-positions", "compatibility/underscore-in-rust-positions.accept.lc"),
    ("pub-locus-as-tuple-field-type", "compatibility/pub-locus-as-tuple-field-type.accept.lc"),
    ("at-in-patterns", "compatibility/at-in-patterns.accept.lc"),
    ("raw-identifier-logic", "compatibility/raw-identifier-logic.accept.lc"),
    ("keywords-as-names", "compatibility/keywords-as-names.accept.lc"),
    ("after-move", "observation/after-move.reject.lc"),
    ("during-mutable-borrow", "observation/during-mutable-borrow.reject.lc"),
]

HEADER = """//! Spike case for question 2: `litmus/{path}`, its full text copied
//! unchanged inside `locus! {{ ... }}`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + {offset} here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {{
"""


def render(name, path):
    text = (LITMUS / path).read_text()
    header = HEADER.format(path=path, offset=7)
    runs = []
    for line in text.splitlines():
        if line.startswith("//~ run:"):
            call = line[len("//~ run:"):].split("=>")[0].strip()
            runs.append(f'    println!("{{}} => {{:?}}", r#"{call}"#, {call});')
    main = ["", "}", "", "fn main() {", '    println!("expansion: {}", ::locus::__expansion!());']
    main += runs
    main += ["}", ""]
    rust = header + text.rstrip("\n") + "\n" + "\n".join(main)
    # rustfmt formats only the harness: it leaves a `locus! { }` block alone.
    return subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout", "-q"], input=rust,
                          capture_output=True, text=True, check=True).stdout


def main():
    check = "--check" in sys.argv
    out = HERE / "cases" / "src" / "bin"
    out.mkdir(parents=True, exist_ok=True)
    stale = []
    for name, path in CASES:
        want = render(name, path)
        # The example's text must reach `locus!` byte for byte.
        assert (LITMUS / path).read_text().rstrip("\n") in want, path
        f = out / f"{name}.rs"
        if check:
            if not f.exists() or f.read_text() != want:
                stale.append(f.name)
        else:
            f.write_text(want)
    if check:
        if stale:
            print("stale copies of litmus examples:", ", ".join(stale))
            sys.exit(1)
        print(f"all {len(CASES)} case files hold their litmus example's text verbatim, and are up to date")


if __name__ == "__main__":
    main()
