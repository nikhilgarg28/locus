"""Summarizes rustc's JSON diagnostics.

First line: `ok`, or the sorted error codes, comma-separated.
Then one indented line per error and per warning: code or lint, line:column,
and the message.
"""
import json
import sys

errors, details = set(), []
case = sys.argv[2] if len(sys.argv) > 2 else None
for line in open(sys.argv[1]):
    line = line.strip()
    if not line.startswith("{"):
        continue
    d = json.loads(line)
    if d.get("$message_type", "diagnostic") != "diagnostic" or d["level"] not in ("error", "warning"):
        continue
    if d["message"].startswith(("aborting due to", "Some errors have")) or "warning emitted" in d["message"] or "warnings emitted" in d["message"]:
        continue
    code = (d.get("code") or {}).get("code") or ("error" if d["level"] == "error" else "warning")
    primary = [s for s in d["spans"] if s["is_primary"]]
    at = "-"
    if primary:
        s = primary[0]
        where = "" if s["file_name"] == case else s["file_name"].rsplit("/", 1)[-1] + ":"
        at = f"{where}{s['line_start']}:{s['column_start']}"
    if d["level"] == "error":
        errors.add(code)
    details.append(f"    {d['level']} {code} {at} {d['message']}")
print(",".join(sorted(errors)) if errors else "ok")
for line in details:
    print(line)
