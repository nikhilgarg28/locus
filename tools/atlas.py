#!/usr/bin/env python3
"""Compatibility commands for the Markdown documentation and roadmap.

    python3 tools/atlas.py list / show NAME / put NAME FILE / tasks / plan
    python3 tools/atlas.py dump DIRECTORY
    python3 tools/atlas.py serve [PORT]    build and preview the public site

Markdown under docs/ is authoritative. atlas.html is only a legacy entry point.
"""
import argparse
import datetime
import pathlib
import re
import subprocess
import sys
import content

ROOT = pathlib.Path(__file__).resolve().parent.parent
STATUS_MARK = {"done": "x", "canceled": "-"}


def load():
    """Keep the old three-result API for measurement publishers.

    Text and match are unused now that HTML is output, not storage.
    """
    return None, None, content.load(ROOT)


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")


def store(_text, _match, data):
    """Compatibility API for publishers that update Markdown and its metadata."""
    content.save(data, ROOT)


def find(data, name):
    if name == "language":
        chapters = [d for d in data["docs"] if d.get("spec_chapter") == 1]
        return {"id": "language", "title": "Language manual", "body": [line for d in chapters for line in d["body"] + [""]]}
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

# A planned project: its document in the Plan group, and its lanes in column order.
PLAN_PROJECTS = {
    "Core build": ("build-plan", PLAN_LANES),
    "Reconciliation": ("reconciliation-plan", ["Syntax", "Kernel", "Elaborator", "Generated", "Data", "References", "Robustness"]),
}


def plan(data, name="Core build"):
    """A planned project's tasks are the only record of what depends on what. A task's
    title starts with its key, and its notes have the lines "Lane: X." and
    "Depends on: K1 (LOC-n), ...". Everything else about the order is derived here."""
    doc_id, PLAN_LANES = PLAN_PROJECTS[name]
    project_id = {"Core build": "p266", "Reconciliation": "p322"}[name]
    project = next(p for p in data["projects"] if p["id"] == project_id)
    prefix = data["meta"].get("taskPrefix", "LOC")
    tasks = {}
    for task in data["tasks"]:
        if task["project"] == project["id"] and " · " in task["title"]:
            tasks[task["title"].split(" · ")[0]] = task
    lane, deps, lane_line = {}, {}, {}
    for key, task in tasks.items():
        lines = task["notes"].split("\n")
        at = next((i for i, l in enumerate(lines) if l.startswith("Lane: ")), None)
        found = re.match(r"Lane: (\w+)\.", lines[at]) if at is not None else None
        after = next((l for l in lines if l.startswith("Depends on:")), None)
        if not found or found.group(1) not in PLAN_LANES or after is None:
            sys.exit("%s: the notes must have a 'Lane: X.' line and a 'Depends on:' line" % key)
        lane_line[key] = at
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
        lines[lane_line[key]] = "Lane: %s. Wave %d." % (lane[key], wave[key])
        at = next(i for i, l in enumerate(lines) if l.startswith("Depends on:"))
        lines[at] = "Depends on: %s." % (", ".join(ref(d) for d in deps[key]) or "nothing")
        unblocks = [k for k in order if key in deps[k]]
        if unblocks:
            lines.insert(at + 1, "Unblocks: %s." % ", ".join(ref(k) for k in unblocks))
        tasks[key]["notes"] = "\n".join(lines)

    merged = "Harness" in PLAN_LANES and PLAN_LANES[-1] == "Robustness"
    columns = PLAN_LANES[:-1] if merged else PLAN_LANES
    heads = (["Harness and robustness"] + columns[1:]) if merged else columns
    table = ["| Wave | " + " | ".join(heads) + " |", "|---" * (len(columns) + 1) + "|"]
    for n in range(1, max(wave.values()) + 1):
        cells = []
        for column in columns:
            mine = [k for k in order if wave[k] == n and (lane[k] == column or (merged and column == "Harness" and lane[k] == "Robustness"))]
            cells.append(", ".join(mine))
        table.append("| %d | %s |" % (n, " | ".join(cells)))
    listing = ["## The commits", "", "One task each, in the %s project, where the scope, the tests, and the condition for done are written. This list and the table of waves are written by python3 tools/atlas.py plan from the tasks, which are the only record of the order." % name, ""]
    for name in PLAN_LANES:
        listing += ["### " + name, ""]
        for key in order:
            if lane[key] == name:
                title = tasks[key]["title"].split(" · ", 1)[1]
                listing.append("- **%s** %s-%d. %s. After: %s." % (key, prefix, tasks[key]["n"], title, ", ".join(deps[key]) or "nothing"))
        listing.append("")
    doc = find(data, doc_id)
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


def serve(port, open_browser=True):
    arguments = [sys.executable, str(ROOT/"tools/site.py"), "serve", "--port", str(port)]
    if not open_browser:
        arguments.append("--no-open")
    raise SystemExit(subprocess.call(arguments))


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    parser = argparse.ArgumentParser(description=__doc__)
    if any(arg == "--file" or arg.startswith("--file=") for arg in args):
        parser.error("--file has been retired: edit the Markdown sources under docs/; atlas.html is only a compatibility entry point")
    commands = parser.add_subparsers(dest="command")
    commands.add_parser("list", help="list Markdown documents")
    show = commands.add_parser("show", help="print one document body")
    show.add_argument("name")
    put = commands.add_parser("put", help="replace one Markdown body, preserving its metadata")
    put.add_argument("name")
    put.add_argument("file", help="Markdown body file, or - for standard input")
    dump = commands.add_parser("dump", help="export document bodies and a roadmap summary")
    dump.add_argument("directory", type=pathlib.Path)
    plans = commands.add_parser("plan", help="refresh a historical plan from its task dependencies")
    plans.add_argument("project", nargs="?", default="Core build", choices=tuple(PLAN_PROJECTS))
    commands.add_parser("tasks", help="print roadmap tasks")
    preview = commands.add_parser("serve", help="build and serve the public site")
    preview.add_argument("port", nargs="?", type=int, default=8765)
    preview.add_argument("--no-open", action="store_true")
    args = parser.parse_args(args)
    command = args.command or "list"
    if command == "serve":
        return serve(args.port, not args.no_open)
    text, match, data = load()
    if command == "list":
        for doc in data["docs"]:
            print("%-18s %-12s %5d lines  %s" % (doc["id"], doc.get("group", ""), len(doc["body"]), doc["title"]))
    elif command == "show":
        print("\n".join(find(data, args.name)["body"]))
    elif command == "put":
        if args.name == "language":
            parser.error("edit the individual docs/spec/*.md chapters; the combined language view is read-only")
        doc = find(data, args.name)
        body = sys.stdin.read() if args.file == "-" else pathlib.Path(args.file).read_text(encoding="utf-8")
        metadata = {k: v for k, v in doc.items() if k not in ("source", "body", "body_line")}
        metadata["updated"] = now()
        # Replacing one body must not rewrite unrelated documents or roadmap tasks.
        content.write_if_changed(ROOT / doc["source"], content.markdown(metadata, body))
        print("replaced " + doc["source"])
    elif command == "dump":
        args.directory.mkdir(parents=True, exist_ok=True)
        for doc in data["docs"]:
            (args.directory / (doc["id"] + ".md")).write_text("\n".join(doc["body"]) + "\n", encoding="utf-8")
        (args.directory / "roadmap.md").write_text(roadmap(data), encoding="utf-8")
        print("wrote %d files to %s" % (len(data["docs"]) + 1, args.directory))
    elif command == "plan":
        summary = plan(data, args.project)
        store(text, match, data)
        print(summary)
    elif command == "tasks":
        sys.stdout.write(roadmap(data))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
