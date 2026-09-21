#!/usr/bin/env python3
"""Keeps atlas.html and the markdown files in step.

atlas.html holds everything: documents, the language status table, projects
and tasks. A document may name the markdown file it mirrors.

    python3 tools/atlas.py status   which mirrors differ from the atlas
    python3 tools/atlas.py export   write the markdown files from the atlas,
                                    and docs/roadmap.md from the projects
    python3 tools/atlas.py import   read the markdown files into the atlas

Only the data block of atlas.html is rewritten; the program around it is
left byte for byte as it was, in the format the page itself saves.
"""
import datetime
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ATLAS = ROOT / "atlas.html"
BLOCK = re.compile(r'(<script type="application/json" id="atlas-data">\n)(.*?)(\n</script>)', re.S)
ROADMAP = "docs/roadmap.md"
STATUS_MARK = {"done": "x", "canceled": "-"}


def load():
    text = ATLAS.read_text(encoding="utf-8")
    match = BLOCK.search(text)
    if not match:
        sys.exit("atlas.html has no data block")
    return text, match, json.loads(match.group(2))


def store(text, match, data):
    # The same layout as JSON.stringify(data, null, 1) in the page.
    dumped = json.dumps(data, indent=1, ensure_ascii=False).replace("<", "\\u003c")
    ATLAS.write_text(text[: match.start(2)] + dumped + text[match.end(2):], encoding="utf-8")


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")


def roadmap(data):
    lines = [
        "# Roadmap",
        "",
        "Generated from atlas.html by `python3 tools/atlas.py export`. The projects and tasks are edited there.",
        "",
    ]
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


def mirrors(data):
    for doc in data["docs"]:
        if doc.get("path"):
            yield doc, ROOT / doc["path"], "\n".join(doc["body"])
    yield None, ROOT / ROADMAP, roadmap(data)


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else "status"
    text, match, data = load()
    if command == "status":
        clean = True
        for doc, path, body in mirrors(data):
            on_disk = path.read_text(encoding="utf-8") if path.exists() else None
            if on_disk != body:
                clean = False
                print(("missing  " if on_disk is None else "differs  ") + str(path.relative_to(ROOT)))
        print("everything is in step" if clean else "run export to write the files, or import to read them")
    elif command == "export":
        for doc, path, body in mirrors(data):
            if not path.exists() or path.read_text(encoding="utf-8") != body:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(body, encoding="utf-8")
                print("wrote    " + str(path.relative_to(ROOT)))
    elif command == "import":
        changed = False
        for doc, path, body in mirrors(data):
            if doc is None or not path.exists():
                continue
            on_disk = path.read_text(encoding="utf-8")
            if on_disk != body:
                doc["body"] = on_disk.split("\n")
                doc["updated"] = now()
                changed = True
                print("read     " + str(path.relative_to(ROOT)))
        if changed:
            store(text, match, data)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
