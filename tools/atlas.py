#!/usr/bin/env python3
"""Reads and writes the documents inside atlas.html from the command line.

atlas.html is the only copy of the documents, the language status table, and
the projects and tasks. This tool is for working on it outside a browser.

    python3 tools/atlas.py list               the documents, with their names
    python3 tools/atlas.py show NAME          print a document as markdown
    python3 tools/atlas.py put NAME FILE      replace a document's text ("-" reads standard input)
    python3 tools/atlas.py dump DIR           write every document, and the roadmap, as markdown files
    python3 tools/atlas.py tasks              the projects and their tasks
    python3 tools/atlas.py plan               check the order of the Core build tasks, and rewrite from them
                                              the waves and the list of commits in the Build plan
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


PLAN_LANES = ["Harness", "Syntax", "Kernel", "Elaborator", "Mutation", "Ownership", "Robustness"]


def plan(data):
    """The Core build tasks are the only record of what depends on what. A task's
    title starts with its key, and its notes have the lines "Lane: X." and
    "Depends on: K1 (LOC-n), ...". Everything else about the order is derived here."""
    project = next(p for p in data["projects"] if p["name"] == "Core build")
    prefix = data["meta"].get("taskPrefix", "LOC")
    tasks = {}
    for task in data["tasks"]:
        if task["project"] == project["id"] and " · " in task["title"]:
            tasks[task["title"].split(" · ")[0]] = task
    lane, deps = {}, {}
    for key, task in tasks.items():
        lines = task["notes"].split("\n")
        found = re.match(r"Lane: (\w+)\.", lines[0])
        after = next((l for l in lines if l.startswith("Depends on:")), None)
        if not found or found.group(1) not in PLAN_LANES or after is None:
            sys.exit("%s: the notes must start with 'Lane: X.' and have a 'Depends on:' line" % key)
        lane[key] = found.group(1)
        deps[key] = re.findall(r"\b([A-Z]\d+)\b", after.split(":", 1)[1])
        for dep in deps[key]:
            if dep not in tasks:
                sys.exit("%s depends on %s, which is not a task" % (key, dep))
    wave, visiting = {}, set()

    def level(key):
        if key in visiting:
            sys.exit("the dependencies make a cycle through " + key)
        if key not in wave:
            visiting.add(key)
            wave[key] = 1 + max([level(d) for d in deps[key]], default=0)
            visiting.discard(key)
        return wave[key]

    for key in tasks:
        level(key)
    ref = lambda k: "%s (%s-%d)" % (k, prefix, tasks[k]["n"])
    order = sorted(tasks, key=lambda k: tasks[k]["n"])
    for key in order:
        lines = [l for l in tasks[key]["notes"].split("\n") if not l.startswith("Unblocks:")]
        lines[0] = "Lane: %s. Wave %d." % (lane[key], wave[key])
        at = next(i for i, l in enumerate(lines) if l.startswith("Depends on:"))
        lines[at] = "Depends on: %s." % (", ".join(ref(d) for d in deps[key]) or "nothing")
        unblocks = [k for k in order if key in deps[k]]
        if unblocks:
            lines.insert(at + 1, "Unblocks: %s." % ", ".join(ref(k) for k in unblocks))
        tasks[key]["notes"] = "\n".join(lines)

    columns = PLAN_LANES[:-1]
    table = ["| Wave | Harness and robustness | " + " | ".join(columns[1:]) + " |", "|---" * (len(columns) + 1) + "|"]
    for n in range(1, max(wave.values()) + 1):
        cells = []
        for column in columns:
            mine = [k for k in order if wave[k] == n and (lane[k] == column or (column == "Harness" and lane[k] == "Robustness"))]
            cells.append(", ".join(mine))
        table.append("| %d | %s |" % (n, " | ".join(cells)))
    listing = ["## The commits", "", "One task each, in the Core build project, where the scope, the tests, and the condition for done are written. This list and the table of waves are written by python3 tools/atlas.py plan from the tasks, which are the only record of the order.", ""]
    for name in PLAN_LANES:
        listing += ["### " + name, ""]
        for key in order:
            if lane[key] == name:
                title = tasks[key]["title"].split(" · ", 1)[1]
                listing.append("- **%s** %s-%d. %s. After: %s." % (key, prefix, tasks[key]["n"], title, ", ".join(deps[key]) or "nothing"))
        listing.append("")
    doc = find(data, "build-plan")
    body = doc["body"]
    start = next(i for i, l in enumerate(body) if l.startswith("| Wave |"))
    end = start
    while end < len(body) and body[end].startswith("|"):
        end += 1
    body[start:end] = table
    start = body.index("## The commits")
    end = next(i for i in range(start + 1, len(body)) if body[i].startswith("## "))
    body[start:end] = listing
    doc["updated"] = now()

    def chain(key):
        return (chain(max(deps[key], key=lambda d: wave[d])) if deps[key] else []) + [key]

    last = max(order, key=lambda k: wave[k])
    return "%d commits in %d waves; the longest chain is %s" % (len(order), wave[last], ", ".join(chain(last)))


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
    elif command == "plan":
        summary = plan(data)
        store(text, match, data)
        print(summary)
    elif command == "tasks":
        sys.stdout.write(roadmap(data))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
