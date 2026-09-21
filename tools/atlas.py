#!/usr/bin/env python3
"""Reads and writes the documents inside atlas.html from the command line.

atlas.html is the only copy of the documents, the language status table, and
the projects and tasks. This tool is for working on it outside a browser.

    python3 tools/atlas.py list               the documents, with their names
    python3 tools/atlas.py show NAME          print a document as markdown
    python3 tools/atlas.py put NAME FILE      replace a document's text ("-" reads standard input)
    python3 tools/atlas.py dump DIR           write every document, and the roadmap, as markdown files
    python3 tools/atlas.py tasks              the projects and their tasks

Only the data block of atlas.html is rewritten, in the layout the page itself
saves, and its revision is raised so that an open page notices the change.
"""
import datetime
import json
import pathlib
import re
import sys

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


def main():
    args = sys.argv[1:]
    command = args[0] if args else "list"
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
