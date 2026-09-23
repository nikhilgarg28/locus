#!/usr/bin/env python3
"""Keeps the Locus highlighter embedded in atlas.html in step with its source.

    python3 tools/highlight.py put      copy editors/highlight/locus.js into atlas.html
    python3 tools/highlight.py check    fail if the two differ (run by tools/check.sh)

The embedded copy sits between the lines `// highlight:begin` and
`// highlight:end` inside the atlas's script, so that the page stays one
self-contained file while the module has one source of truth.
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ATLAS = ROOT / "atlas.html"
MODULE = ROOT / "editors" / "highlight" / "locus.js"
BEGIN, END = "// highlight:begin", "// highlight:end"


def split(text):
    start = text.index(BEGIN) + len(BEGIN) + 1
    end = text.index(END)
    return text[:start], text[start:end], text[end:]


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else "check"
    text = ATLAS.read_text()
    if BEGIN not in text or END not in text:
        sys.exit("atlas.html has no highlight markers")
    head, embedded, tail = split(text)
    module = MODULE.read_text()
    if command == "put":
        if embedded != module:
            ATLAS.write_text(head + module + tail)
            print("embedded editors/highlight/locus.js into atlas.html")
        else:
            print("atlas.html already has the current highlighter")
    elif command == "check":
        if embedded != module:
            sys.exit("atlas.html's embedded highlighter differs from editors/highlight/locus.js; run python3 tools/highlight.py put")
        print("highlighter in atlas.html is current")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
