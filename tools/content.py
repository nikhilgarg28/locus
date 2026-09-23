#!/usr/bin/env python3
"""Canonical Markdown documents and project tasks for the Locus website.

TOML front matter identifies a document; its body is ordinary Markdown. Project
files retain every LOC number and the original internal ID in task comments.
HTML is an output and is never read as the current specification.
"""
from __future__ import annotations
import json
import datetime
import math
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parent.parent
TASK = re.compile(
    r'^(?:<a id="LOC-\d+"></a>\n)?## LOC-(\d+) · (.+)\n<!-- task: (\{[^\n]+\}) -->(?:\n|$)',
    re.M,
)


def read_markdown(path: Path) -> dict:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("+++\n") or "\n+++\n" not in text[4:]:
        raise ValueError(f"{path}: expected TOML front matter between +++ lines")
    header, body = text[4:].split("\n+++\n", 1)
    metadata = tomllib.loads(header)
    metadata["body"] = body.removeprefix("\n").rstrip("\n").split("\n")
    metadata["body_line"] = len(header.splitlines()) + 3 + int(body.startswith("\n"))
    return metadata


def toml_key(key):
    return (
        key
        if re.fullmatch(r"[A-Za-z0-9_-]+", key)
        else json.dumps(key, ensure_ascii=False)
    )


def toml_value(value):
    """Serialize front matter values, including nested inline tables."""
    if isinstance(value, dict):
        return (
            "{ "
            + ", ".join(
                toml_key(key) + " = " + toml_value(item) for key, item in value.items()
            )
            + " }"
        )
    if isinstance(value, list):
        return "[" + ", ".join(toml_value(item) for item in value) + "]"
    if isinstance(value, (datetime.datetime, datetime.date, datetime.time)):
        return value.isoformat()
    if isinstance(value, float) and not math.isfinite(value):
        return "nan" if math.isnan(value) else "inf" if value > 0 else "-inf"
    if isinstance(value, (str, bool, int, float)):
        return json.dumps(value, ensure_ascii=False, allow_nan=False)
    raise ValueError(f"unsupported TOML metadata value: {type(value).__name__}")


def markdown(metadata: dict, body: str) -> str:
    header = "\n".join(
        f"{toml_key(key)} = {toml_value(value)}"
        for key, value in metadata.items()
        if value is not None
    )
    return f"+++\n{header}\n+++\n\n{body.rstrip()}\n"


def write_if_changed(path: Path, text: str):
    if not path.exists() or path.read_text(encoding="utf-8") != text:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


def load(root: Path = ROOT) -> dict:
    root = Path(root)
    state = json.loads((root / "docs/data/state.json").read_text())
    docs, projects, tasks = [], [], []
    seen_docs, seen_projects, seen_tasks, seen_numbers, seen_routes = (
        set(),
        set(),
        set(),
        set(),
        set(),
    )
    for path in sorted((root / "docs").rglob("*.md")):
        if not path.read_text(encoding="utf-8").startswith("+++\n"):
            continue  # Diagnostic explanations keep their existing format.
        item = read_markdown(path)
        item["source"] = path.relative_to(root).as_posix()
        route = item.get("route")
        if route:
            if (
                route in seen_routes
                or route.startswith("/")
                or ".." in Path(route).parts
                or not route.endswith(".html")
            ):
                raise ValueError(f"{path}: duplicate or unsafe route {route!r}")
            seen_routes.add(route)
        if item.get("kind") != "project":
            if item["id"] in seen_docs:
                raise ValueError(f'duplicate document {item["id"]}')
            seen_docs.add(item["id"])
            docs.append(item)
            continue
        if item["id"] in seen_projects:
            raise ValueError(f'duplicate project {item["id"]}')
        seen_projects.add(item["id"])
        body = "\n".join(item.pop("body"))
        matches = list(TASK.finditer(body))
        description = body[: matches[0].start()] if matches else body
        item["description"] = re.sub(r"^# [^\n]+\n*", "", description).strip()
        projects.append(item)
        for index, match in enumerate(matches):
            number, title, raw = match.groups()
            task = json.loads(raw)
            if int(number) in seen_numbers:
                raise ValueError(f"{path}: duplicate task LOC-{number}")
            seen_tasks.add(task["id"])
            seen_numbers.add(int(number))
            end = matches[index + 1].start() if index + 1 < len(matches) else len(body)
            task.update(
                n=int(number),
                title=title,
                project=item["id"],
                notes=body[match.end() : end].strip(),
            )
            tasks.append(task)
        # A typo must not silently remove a task from the public roadmap.
        if len(matches) != len(
            re.findall(r'^(?:<a id="LOC-\d+"></a>\n)?## LOC-', body, re.M)
        ):
            raise ValueError(
                f"{path}: malformed LOC task heading or missing task metadata"
            )
    state.update(
        docs=sorted(docs, key=lambda d: d.get("order", 999)),
        projects=sorted(projects, key=lambda p: p.get("order", 999)),
        tasks=sorted(tasks, key=lambda t: t["n"]),
    )
    return state


def save(data: dict, root: Path = ROOT):
    root = Path(root)
    for doc in data["docs"]:
        source = doc.get("source", f'docs/{doc["id"]}.md')
        metadata = {
            k: v for k, v in doc.items() if k not in ("source", "body", "body_line")
        }
        write_if_changed(root / source, markdown(metadata, "\n".join(doc["body"])))
    for project in data["projects"]:
        metadata = {
            k: v
            for k, v in project.items()
            if k not in ("source", "description", "body_line")
        }
        metadata["kind"] = "project"
        body = f'# {project["name"]}\n\n{project.get("description", "")}\n'
        for task in sorted(
            (t for t in data["tasks"] if t["project"] == project["id"]),
            key=lambda t: t["n"],
        ):
            fields = {
                k: v
                for k, v in task.items()
                if k not in ("n", "title", "project", "notes")
            }
            body += f'\n<a id="LOC-{task["n"]}"></a>\n## LOC-{task["n"]} · {task["title"]}\n<!-- task: {json.dumps(fields, ensure_ascii=False)} -->\n\n{task.get("notes", "")}\n'
        source = project.get("source", f'docs/roadmap/{project["id"]}.md')
        write_if_changed(root / source, markdown(metadata, body))
    state = {k: v for k, v in data.items() if k not in ("docs", "projects", "tasks")}
    write_if_changed(
        root / "docs/data/state.json",
        json.dumps(state, indent=2, ensure_ascii=False) + "\n",
    )
