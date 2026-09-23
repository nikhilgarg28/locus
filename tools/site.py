#!/usr/bin/env python3
"""Build, validate, or preview the static public website.

  npm ci --prefix website
  python3 tools/site.py build
  python3 tools/site.py check
  python3 tools/site.py serve --port 8765 --no-open

The build reads Markdown and repository metadata, then emits target/site.
No browser-side Markdown renderer or write-capable web API is involved.
"""
from __future__ import annotations
import argparse
from html.parser import HTMLParser
import http.server
import json
from pathlib import Path
import subprocess
import sys
from urllib.parse import unquote, urlsplit
import webbrowser

ROOT = Path(__file__).resolve().parent.parent


class Page(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.duplicates = []
        self.links = []
        self.lang = False
        self.title = False
        self.main = False

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if "id" in attrs:
            if attrs["id"] in self.ids:
                self.duplicates.append(attrs["id"])
            self.ids.add(attrs["id"])
        if tag == "html":
            self.lang = bool(attrs.get("lang"))
        if tag == "title":
            self.title = True
        if tag == "main":
            self.main = True
        for name in ("href", "src", "poster", "action"):
            if attrs.get(name):
                self.links.append(attrs[name])


def validate(directory: Path):
    directory = directory.resolve()
    pages = {}
    errors = []
    links = 0
    for file in sorted(directory.rglob("*.html")):
        parser = Page()
        parser.feed(file.read_text())
        pages[file.resolve()] = parser
        if parser.duplicates:
            errors.append(
                f"{file.relative_to(directory)}: duplicate IDs: {parser.duplicates}"
            )
        if not parser.lang or not parser.title:
            errors.append(f"{file.name}: missing document language or title")
        if file.name != "atlas.html" and not parser.main:
            errors.append(f"{file.name}: missing main landmark")
    for file, page in pages.items():
        for link in page.links:
            url = urlsplit(link)
            if url.scheme or url.netloc:
                if url.scheme and url.scheme not in ("http", "https", "mailto", "tel"):
                    errors.append(f"{file.name}: unsafe link {link}")
                continue
            if not url.path:
                target = file
            else:
                target = (
                    directory / url.path.lstrip("/")
                    if url.path.startswith("/")
                    else file.parent / unquote(url.path)
                ).resolve()
            if target.is_dir():
                target = target / "index.html"
            links += 1
            if not target.is_relative_to(directory) or not target.exists():
                errors.append(
                    f"{file.relative_to(directory)}: missing local target {link}"
                )
            elif (
                url.fragment
                and target in pages
                and unquote(url.fragment) not in pages[target].ids
            ):
                errors.append(f"{file.relative_to(directory)}: missing fragment {link}")
    index = directory / "data/spec-index.json"
    if index.exists():
        data = json.loads(index.read_text())
        seen = set()
        for rule in data["rules"]:
            if rule["id"] in seen:
                errors.append(f'duplicate rule {rule["id"]}')
            seen.add(rule["id"])
            url = urlsplit(rule["url"])
            file = (directory / url.path).resolve()
            if file not in pages or url.fragment not in pages[file].ids:
                errors.append(f'rule {rule["id"]} is absent from rendered HTML')
            if rule["category"] not in ("informative", "example") and not any(
                t["focused"] for t in rule["tests"]
            ):
                errors.append(f'operative rule {rule["id"]} has no focused test')
    if errors:
        raise ValueError("\n".join(errors))
    if not pages:
        raise ValueError("no HTML pages were built")
    return f"Site checked: {len(pages)} HTML pages, {links} local links, no broken fragments or duplicate IDs."


def build(output: Path):
    result = subprocess.run(
        ["node", str(ROOT / "website/build.mjs"), "--out", str(output)], cwd=ROOT
    )
    if result.returncode:
        raise SystemExit(result.returncode)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "check", "serve", "validate"))
    parser.add_argument("--out", type=Path, default=ROOT / "target/site")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--no-open", action="store_true")
    args = parser.parse_args()
    args.out = args.out.resolve()
    try:
        if args.command == "validate":
            print(validate(args.out))
            return 0
        build(args.out)
        if args.command == "check":
            return 0  # The build always validates before publishing output.
        if args.command == "serve":
            handler = lambda *a, **kw: http.server.SimpleHTTPRequestHandler(
                *a, directory=str(args.out), **kw
            )
            server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), handler)
            address = f"http://127.0.0.1:{args.port}/"
            print(
                f"Locus website: {address}\nEdit Markdown, then run tools/site.py build to refresh.",
                flush=True,
            )
            if not args.no_open:
                webbrowser.open(address)
            try:
                server.serve_forever()
            except KeyboardInterrupt:
                pass
            finally:
                server.server_close()
    except (ValueError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
