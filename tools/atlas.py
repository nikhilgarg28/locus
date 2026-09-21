#!/usr/bin/env python3
"""Reads and writes the documents inside atlas.html from the command line.

atlas.html is the only copy of the documents, the language status table, and
the projects and tasks. This tool is for working on it outside a browser.

    python3 tools/atlas.py list               the documents, with their names
    python3 tools/atlas.py show NAME          print a document as markdown
    python3 tools/atlas.py put NAME FILE      replace a document's text ("-" reads standard input)
    python3 tools/atlas.py dump DIR           write every document, and the roadmap, as markdown files
    python3 tools/atlas.py tasks              the projects and their tasks
    python3 tools/atlas.py serve [PORT]       open the atlas from a local address, where it can save
                                              itself as changes are made, in any browser

Add --file PATH to work on a copy other than atlas.html, and --no-open to serve without opening a browser.

Only the data block of atlas.html is rewritten, in the layout the page itself
saves, and its revision is raised so that an open page notices the change.
"""
import datetime
import http.server
import json
import os
import pathlib
import re
import sys
import webbrowser

ROOT = pathlib.Path(__file__).resolve().parent.parent
ATLAS = ROOT / "atlas.html"
BLOCK = re.compile(r'(<script type="application/json" id="atlas-data">\n)(.*?)(\n</script>)', re.S)
STATUS_MARK = {"done": "x", "canceled": "-"}


def load():
    text = ATLAS.read_text(encoding="utf-8")
    match = BLOCK.search(text)
    if not match:
        sys.exit("atlas.html has no data block")
    return text, match, json.loads(match.group(2))


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")


def store(text, match, data):
    data["meta"]["rev"] = data["meta"].get("rev", 0) + 1
    data["meta"]["savedAt"] = now()
    # The same layout as JSON.stringify(data, null, 1) in the page.
    dumped = json.dumps(data, indent=1, ensure_ascii=False).replace("<", "\\u003c")
    ATLAS.write_text(text[: match.start(2)] + dumped + text[match.end(2):], encoding="utf-8")


def find(data, name):
    for doc in data["docs"]:
        if doc["id"] == name or doc["title"].lower() == name.lower():
            return doc
    sys.exit("no document named %r; try: %s" % (name, ", ".join(d["id"] for d in data["docs"])))


def roadmap(data):
    lines = ["# Roadmap", ""]
    for project in data["projects"]:
        lines += ["## " + project["name"], "", "Status: " + project.get("status", "planned") + ".", ""]
        if project.get("description"):
            lines += [project["description"].rstrip(), ""]
        tasks = [t for t in data["tasks"] if t.get("project") == project["id"]]
        for task in sorted(tasks, key=lambda t: t["n"]):
            mark = STATUS_MARK.get(task["status"], " ")
            suffix = "" if task["status"] in ("done", "backlog", "canceled") else " (" + task["status"] + ")"
            lines.append("- [%s] %s-%d %s%s" % (mark, data["meta"].get("taskPrefix", "LOC"), task["n"], task["title"], suffix))
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def revision_of(text):
    match = BLOCK.search(text)
    if not match:
        return None
    try:
        return json.loads(match.group(2))["meta"]["rev"]
    except (ValueError, KeyError):
        return None


def serve(port, open_browser):
    """Serves the atlas on this machine only, and writes what the page sends back."""
    allowed_hosts = {"localhost:%d" % port, "127.0.0.1:%d" % port}

    class Handler(http.server.BaseHTTPRequestHandler):
        def reply(self, status, body, kind="application/json"):
            data = body.encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", kind + "; charset=utf-8")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(data)

        def ours(self):
            # A page on another site cannot set this header without asking first, and is not answered.
            return self.headers.get("Host") in allowed_hosts and self.headers.get("X-Atlas") == "1"

        def do_GET(self):
            path = self.path.split("?")[0]
            if path == "/":
                self.send_response(302)
                self.send_header("Location", "/atlas.html")
                self.end_headers()
            elif path == "/atlas.html":
                self.reply(200, ATLAS.read_text(encoding="utf-8"), "text/html")
            elif path == "/__atlas/info" and self.ours():
                self.reply(200, json.dumps({"atlas": True, "rev": revision_of(ATLAS.read_text(encoding="utf-8"))}))
            else:
                self.reply(404, "{}")

        def do_POST(self):
            if self.path != "/__atlas/save" or not self.ours():
                return self.reply(404, "{}")
            text = self.rfile.read(int(self.headers.get("Content-Length", "0"))).decode("utf-8")
            rev = revision_of(text)
            if rev is None or not text.startswith("<!DOCTYPE html>"):
                return self.reply(400, "this is not an atlas")
            scratch = ATLAS.with_name(ATLAS.name + ".saving")
            scratch.write_text(text, encoding="utf-8")
            os.replace(scratch, ATLAS)
            self.reply(200, json.dumps({"rev": rev}))

        def log_message(self, *args):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    address = "http://localhost:%d/atlas.html" % port
    print("serving %s at %s (Ctrl+C to stop)" % (ATLAS, address))
    if open_browser:
        webbrowser.open(address)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


def main():
    global ATLAS
    args = sys.argv[1:]
    if "--file" in args:
        at = args.index("--file")
        ATLAS = pathlib.Path(args[at + 1]).resolve()
        del args[at:at + 2]
    open_browser = "--no-open" not in args
    args = [a for a in args if a != "--no-open"]
    command = args[0] if args else "list"
    if command == "serve":
        return serve(int(args[1]) if len(args) > 1 else 8765, open_browser)
    text, match, data = load()
    if command == "list":
        for doc in data["docs"]:
            print("%-18s %-12s %5d lines  %s" % (doc["id"], doc.get("group", ""), len(doc["body"]), doc["title"]))
    elif command == "show" and len(args) == 2:
        sys.stdout.write("\n".join(find(data, args[1])["body"]))
    elif command == "put" and len(args) == 3:
        doc = find(data, args[1])
        body = sys.stdin.read() if args[2] == "-" else pathlib.Path(args[2]).read_text(encoding="utf-8")
        doc["body"] = body.split("\n")
        doc["updated"] = now()
        store(text, match, data)
        print("replaced %s; atlas.html is at revision %d" % (doc["id"], data["meta"]["rev"]))
    elif command == "dump" and len(args) == 2:
        out = pathlib.Path(args[1])
        out.mkdir(parents=True, exist_ok=True)
        for doc in data["docs"]:
            (out / (doc["id"] + ".md")).write_text("\n".join(doc["body"]), encoding="utf-8")
        (out / "roadmap.md").write_text(roadmap(data), encoding="utf-8")
        print("wrote %d files to %s" % (len(data["docs"]) + 1, out))
    elif command == "tasks":
        sys.stdout.write(roadmap(data))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
