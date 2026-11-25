#!/usr/bin/env python3
"""Builds every binary of a package in one expansion and reports, for each
binary, rustc's errors. Usage:

    build.py PACKAGE MODE TARGET_DIR [--json OUT] [--features F] [name-prefix]

MODE is `runtime` or `check` (the same build with `--cfg __locus_check`).
Prints one line for each binary: `name<TAB>codes`, where codes is `ok` or the
error codes, and writes every error, with its place, to OUT as JSON.
"""
import json, os, subprocess, sys

def main():
    args = sys.argv[1:]
    out = None
    if "--json" in args:
        i = args.index("--json")
        out = args[i + 1]
        del args[i:i + 2]
    features = None
    if "--features" in args:
        i = args.index("--features")
        features = args[i + 1]
        del args[i:i + 2]
    package, mode, target = args[:3]
    prefix = args[3] if len(args) > 3 else ""
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    bins = sorted(f[:-3] for f in os.listdir(f"{here}/{package}/src/bin")
                  if f.endswith(".rs") and f.startswith(prefix))
    env = dict(os.environ)
    env["RUSTFLAGS"] = "--cfg __locus_check" if mode == "check" else ""
    env["CARGO_TERM_COLOR"] = "never"
    cmd = ["cargo", "build", "-q", "-p", package, "--keep-going", "--target-dir", target,
           "--message-format=json"]
    for b in bins:
        cmd += ["--bin", b]
    if features:
        cmd += ["--features", features]
    p = subprocess.run(cmd, cwd=here, env=env, capture_output=True, text=True)
    errors = {b: [] for b in bins}
    for line in p.stdout.splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("reason") != "compiler-message":
            continue
        d = m["message"]
        if d["level"] != "error" or d["message"].startswith("aborting due to"):
            continue
        name = m["target"]["name"]
        spans = [s for s in d["spans"] if s["is_primary"]]
        # Where the user wrote it: the outermost expansion site in the binary's file.
        place = None
        for s in spans:
            t = s
            while t.get("expansion") and not t["file_name"].endswith(f"{name}.rs"):
                t = t["expansion"]["span"]
            place = (t["file_name"], t["line_start"], t["column_start"], t["line_end"], t["column_end"])
            break
        errors.setdefault(name, []).append({
            "code": (d.get("code") or {}).get("code") or "error",
            "message": d["message"],
            "place": place,
            "rendered": d.get("rendered", ""),
        })
    for b in bins:
        codes = sorted({e["code"] for e in errors[b]})
        print(f"{b}\t{','.join(codes) if codes else 'ok'}")
    if out:
        with open(out, "w") as f:
            json.dump(errors, f, indent=1)

main()
