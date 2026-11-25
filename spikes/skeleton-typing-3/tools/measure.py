#!/usr/bin/env python3
"""Builds packages of this spike in both expansions under one configuration
and reports, for each binary, what rustc said, and, for each piece of logical
code that `locus!` recorded, whether it has a typing skeleton and whether
rustc reported an error inside it.

    measure.py CONFIG [--features F] [--driver] [--calls] [--workspace DIR] [--quiet]
               [--env K=V]... [--prefix P] PACKAGE...

Everything is built under target/CONFIG/{runtime,check}. The report goes to
out/CONFIG/report.txt and out/CONFIG/report.json, and to stdout.
"""
import json, os, re, subprocess, sys, time

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def build(package, mode, target, features, env_extra, prefix, pieces, calls=None, workspace=HERE):
    bins = sorted(f[:-3] for f in os.listdir(f"{HERE}/{package}/src/bin")
                  if f.endswith(".rs") and f.startswith(prefix))
    now = time.time()
    for b in bins:  # so that the macro runs again and records its pieces
        os.utime(f"{HERE}/{package}/src/bin/{b}.rs", (now, now))
    env = dict(os.environ)
    env.update(env_extra)
    env["RUSTFLAGS"] = "--cfg __locus_check" if mode == "check" else ""
    env["CARGO_TERM_COLOR"] = "never"
    env["LOCUS_SPIKE_PIECES"] = pieces
    os.makedirs(pieces, exist_ok=True)
    if calls:
        env["LOCUS_CALLS_DIR"] = calls
        os.makedirs(calls, exist_ok=True)
    cmd = ["cargo", "build", "-q", "-p", package, "--keep-going", "--target-dir", f"{HERE}/{target}",
           "--message-format=json"]
    for b in bins:
        cmd += ["--bin", b]
    if features:
        cmd += ["--features", features]
    started = time.time()
    p = subprocess.run(cmd, cwd=workspace, env=env, capture_output=True, text=True)
    seconds = time.time() - started
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
        if name not in errors and m["target"]["kind"] != ["bin"]:
            # an error in a library that the binaries depend on: every
            # binary of the package fails with it
            for b in bins:
                errors[b].append({"code": (d.get("code") or {}).get("code") or "error",
                                  "message": f"in the crate `{name}`: " + d["message"], "place": None,
                                  "rendered": d.get("rendered", "")})
            continue
        place = None
        for s in d["spans"]:
            if not s["is_primary"]:
                continue
            t = s
            # Where the user wrote it: the outermost expansion site in the binary's file.
            while t.get("expansion") and not t["file_name"].endswith(f"{name}.rs"):
                t = t["expansion"]["span"]
            place = [t["file_name"], t["line_start"], t["column_start"], t["line_end"], t["column_end"]]
            break
        errors.setdefault(name, []).append({
            "code": (d.get("code") or {}).get("code") or "error",
            "message": d["message"],
            "place": place,
            "rendered": d.get("rendered", ""),
        })
    return bins, errors, seconds


def codes(errs):
    c = sorted({e["code"] for e in errs})
    return ",".join(c) if c else "ok"


def expectation(path):
    text = open(path).read()
    m = re.search(r"^//! expect: runtime=(\S+) check=(\S+)", text, re.M)
    if m:
        return m.group(1), m.group(2), re.findall(r"^//! prints: (.*)$", text, re.M)
    m = re.search(r"litmus/[^`]+\.(accept|reject|open)\.lc", text)
    runs = re.findall(r"^//~ run: (.*)$", text, re.M)
    return (("litmus " + m.group(1)) if m else "none"), None, runs


def matches(want, got):
    return (want == got or (want == "reject" and got != "ok")
            or (want == "annotate" and got in ("E0282", "E0283")))


def run(target, mode, name):
    exe = f"{HERE}/{target}/{mode}/debug/{name}"
    try:
        p = subprocess.run([exe], capture_output=True, text=True, timeout=20)
        lines = p.stdout.splitlines()
        if p.returncode != 0:
            # a stub that ran panics
            lines.append(f"EXITED WITH {p.returncode}: {p.stderr.strip()[:200]}")
        return lines
    except Exception as e:  # noqa: BLE001
        return [f"did not run: {e}"]


def pieces_of(directory, name):
    path = f"{directory}/{name}.tsv"
    out = []
    if not os.path.exists(path):
        return out
    for line in open(path).read().splitlines():
        f = line.split("\t")
        if len(f) < 7:
            continue
        # proc_macro's lines and columns start at 1, as rustc's do
        p = {"kind": f[0], "from": [int(f[1]), int(f[2])], "to": [int(f[3]), int(f[4])],
             "status": f[5], "detail": f[6], "errors": []}
        if p not in out:
            out.append(p)
    return out


def inside(place, piece):
    if not place:
        return False
    at = [place[1], place[2]]
    return piece["from"] <= at <= piece["to"]


def main():
    args = sys.argv[1:]
    config = args.pop(0)
    features, prefix, env_extra, calls, workspace, quiet = None, "", {}, False, HERE, False
    packages = []
    while args:
        a = args.pop(0)
        if a == "--features":
            features = args.pop(0)
        elif a == "--prefix":
            prefix = args.pop(0)
        elif a == "--driver":
            sysroot = subprocess.run(["rustc", "--print", "sysroot"], capture_output=True,
                                     text=True).stdout.strip()
            env_extra["RUSTC_WORKSPACE_WRAPPER"] = f"{HERE}/target/driver/debug/locus-skeleton-driver"
            env_extra["LOCUS_SYSROOT"] = sysroot
            env_extra["LD_LIBRARY_PATH"] = f"{sysroot}/lib"
        elif a == "--calls":
            calls = True
        elif a == "--workspace":
            workspace = f"{HERE}/{args.pop(0)}"
        elif a == "--quiet":
            quiet = True
        elif a == "--env":
            k, v = args.pop(0).split("=", 1)
            env_extra[k] = v
        else:
            packages.append(a)
    target = f"target/{config}"
    out_dir = f"{HERE}/out/{config}"
    stamp = time.strftime("%H%M%S")
    os.makedirs(out_dir, exist_ok=True)
    report = {"config": config, "features": features, "env": env_extra, "packages": {}}
    lines = []
    for package in packages:
        result = {}
        seconds = {}
        for mode in ("runtime", "check"):
            pieces = f"{out_dir}/pieces-{stamp}/{package}/{mode}"
            listing = f"{out_dir}/calls-{stamp}/{package}/{mode}" if calls else None
            bins, errors, seconds[mode] = build(package, mode, f"{target}/{mode}", features,
                                                env_extra, prefix, pieces, listing, workspace)
            result[mode] = (errors, pieces, listing)
        both, differing, listed = 0, 0, 0
        entries = {}
        for b in bins:
            want_r, want_c, prints = expectation(f"{HERE}/{package}/src/bin/{b}.rs")
            if package == "more":
                # Written for the first spike's second mode: its `expect` and
                # `prints` lines say what that mode should do, not this one.
                want_r, want_c, prints = "none", None, []
            er, ec = result["runtime"][0][b], result["check"][0][b]
            got_r, got_c = codes(er), codes(ec)
            pieces = pieces_of(result["runtime"][1], b)
            same_pieces = pieces == pieces_of(result["check"][1], b)
            for mode, errs in (("runtime", er), ("check", ec)):
                for e in errs:
                    hit = [p for p in pieces if p["kind"] != "ghost let" and inside(e["place"], p)]
                    if hit:
                        # the innermost piece
                        hit.sort(key=lambda p: (p["to"][0] - p["from"][0], p["to"][1] - p["from"][1]))
                        hit[0]["errors"].append([mode, e["code"], e["message"]])
                    for p in pieces:
                        if p["kind"] == "ghost let" and e["place"] and (
                                inside(e["place"], p) or e["place"][1] == p["from"][0]):
                            p["errors"].append([mode, e["code"], e["message"]])
            entry = {"expect": [want_r, want_c], "got": [got_r, got_c], "pieces": pieces,
                     "same_pieces": same_pieces,
                     "errors": {"runtime": er, "check": ec}}
            if want_c is None:
                verdict = ""
                if want_r == "litmus accept":
                    verdict = "compiles both ways" if (got_r, got_c) == ("ok", "ok") else "DOES NOT COMPILE"
                elif want_r == "litmus reject":
                    verdict = "rustc accepts" if (got_r, got_c) == ("ok", "ok") else "rustc rejects"
            else:
                verdict = "as expected" if matches(want_r, got_r) and matches(want_c, got_c) else "DIFFERS"
            if (got_r, got_c) == ("ok", "ok"):
                ro, co = run(target, "runtime", b), run(target, "check", b)
                entry["prints"] = [ro, co]
                if ro[:1] != ["expansion: runtime"] or co[:1] != ["expansion: checking"]:
                    verdict += "; A BINARY DID NOT NAME ITS EXPANSION"
                elif ro[1:] != co[1:]:
                    verdict += "; THE TWO BINARIES PRINT DIFFERENT LINES"
                elif prints and (want_c is not None or len(ro) > 1):
                    want = prints if want_c is not None else [
                        f"{a} => {b_}" for a, b_ in (p.split(" => ", 1) for p in prints)]
                    if ro[1:] != want:
                        verdict += f"; PRINTS {ro[1:]}, EXPECTED {want}"
                    else:
                        verdict += "; prints as expected"
            if calls and (got_r, got_c) == ("ok", "ok"):
                name = b.replace("-", "_")
                try:
                    lr = open(f"{result['runtime'][2]}/{name}.calls").read().splitlines()
                    lc = open(f"{result['check'][2]}/{name}.calls").read().splitlines()
                except OSError as e:
                    lr, lc = [f"no listing: {e}"], []
                both += 1
                listed += len([x for x in lr if x])
                entry["calls"] = {"runtime": lr, "check": lc}
                if lr != lc:
                    differing += 1
                    verdict += "; THE TWO EXPANSIONS RESOLVE A CALL DIFFERENTLY"
            entry["verdict"] = verdict
            entries[b] = entry
            exp = want_r if want_c is None else f"runtime={want_r} check={want_c}"
            lines.append(f"== {b}: expect {exp}; got runtime={got_r} check={got_c}; {verdict}")
            for mode, errs in (("runtime", er), ("check", ec)):
                for e in errs[:2]:
                    where = f"{e['place'][1]}:{e['place'][2]}" if e["place"] else "?"
                    lines.append(f"  {mode}: {where}: error[{e['code']}]: {e['message'][:150]}")
            if entry.get("prints"):
                lines.append(f"  both print: {' | '.join(entry['prints'][0][1:])}"
                             if entry["prints"][0][1:] == entry["prints"][1][1:]
                             else f"  runtime prints {entry['prints'][0][1:]}, checking prints {entry['prints'][1][1:]}")
            for p in pieces:
                if p["status"] == "not translated":
                    lines.append(f"  {p['kind']} at {p['from'][0]}:{p['from'][1]}: no skeleton: {p['detail']}")
        report["packages"][package] = {"bins": entries, "seconds": seconds}
        if calls:
            lines.append(f"{package}: binaries that both expansions accept: {both}; calls, method calls "
                         f"and constants listed in each: {listed}; binaries with one resolved differently: {differing}")
        lines.append(f"{package}: built in {seconds['runtime']:.1f}s (runtime) and {seconds['check']:.1f}s (checking)")
    text = "\n".join(lines)
    if quiet:
        print("\n".join(x for x in lines if not x.startswith(("==", "  "))))
    else:
        print(text)
    with open(f"{out_dir}/report.txt", "a") as f:
        f.write(text + "\n")
    name = "-".join(packages)
    with open(f"{out_dir}/report-{name}.json", "w") as f:
        json.dump(report, f, indent=1)


main()
