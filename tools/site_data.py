#!/usr/bin/env python3
"""Prepare repository-derived website inputs; never invent runtime measurements."""
from dataclasses import asdict
import json
from pathlib import Path
import re
import subprocess
import content
import spec
import metrics
import bench

ROOT = content.ROOT


def source_excerpt(citation):
    lines = (ROOT / citation.source).read_text().splitlines()
    if citation.source.endswith(".rs"):
        start = next(
            (
                i
                for i, line in enumerate(lines)
                if re.search(r"\bfn\s+" + re.escape(citation.test) + r"\s*\(", line)
            ),
            0,
        )
        end = next(
            (i for i in range(start + 1, len(lines)) if lines[i].startswith("#[test]")),
            len(lines),
        )
        end = min(end, start + 55)
    else:
        start, end = 0, min(len(lines), 55)
    return {
        **asdict(citation),
        "line": start + 1,
        "end": end,
        "excerpt": "\n".join(lines[start:end]).rstrip(),
        "truncated": end < len(lines)
        and citation.source.endswith(".lc")
        or end == start + 55,
    }


def prepare():
    data = content.load(ROOT)
    paragraphs = spec.inventory(data)
    uses = spec.citations(ROOT)
    coverage = spec.validate(paragraphs, uses)
    spec.fences(data)
    spec.validate_known(data, spec.known_markers(ROOT))
    citations = [source_excerpt(c) for c in uses]
    documents = []
    for doc in data["docs"]:
        doc = dict(doc)
        doc["publish"] = doc.get("publish", doc.get("group") == "Now")
        if doc["id"] == "generated-status":
            doc["body"] = metrics.expected_body(
                data.get("measurements"), metrics.fingerprint(ROOT)
            )
        documents.append(doc)
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    dirty = bool(
        subprocess.check_output(
            ["git", "status", "--porcelain"], cwd=ROOT, text=True
        ).strip()
    )
    history = bench.records()
    if not history and bench.git(
        "rev-parse", "--verify", "refs/remotes/origin/locus-bench-data", check=False
    ):
        history = bench.records("refs/remotes/origin/locus-bench-data")
    performance = {"summary": bench.summary(history), "runs": []}
    if history:
        epoch = history[-1]["epoch"]
        for record in history:
            if record["epoch"] != epoch:
                continue
            performance["runs"].append(
                {
                    k: record[k]
                    for k in (
                        "recorded_at",
                        "source_commit",
                        "dirty",
                        "samples",
                        "pins",
                        "comparisons",
                    )
                }
            )
    return {
        **data,
        "docs": documents,
        "rules": [asdict(p) for p in paragraphs],
        "citations": citations,
        "coverage": coverage,
        "performance": performance,
        "revision": revision,
        "dirty": dirty,
        "repository": "https://github.com/nikhilgarg28/locus",
        "specimen": (ROOT / "examples/increment.lc")
        .read_text()
        .split("\nfn consume")[0]
        .strip(),
    }


if __name__ == "__main__":
    print(json.dumps(prepare(), ensure_ascii=False))
