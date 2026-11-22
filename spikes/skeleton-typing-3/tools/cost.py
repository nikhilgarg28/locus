#!/usr/bin/env python3
"""Build time of a package's binaries, with and without typing skeletons.

    cost.py [--rounds N] [--check] PACKAGE...

With `--check` it times `cargo check`, which stops after rustc's analysis:
no code is generated and nothing is linked.

For each package and each expansion, the library is built first. Then, N
times, every binary's source is touched and `cargo build -j1` is timed: one
rustc at a time, so the figure is the sum of the binaries' compile and link
times and Cargo's own overhead. Printed: the median of the N wall times, for

- this spike's lowering, with skeletons;
- the same with `LOCUS_SPIKE_NO_SKELETON=1`, which lowers every piece of
  logical code as the first mode does (the library and the lowered types
  are still this spike's);
- the first mode itself: `first-mode/`, the library and macro of
  `spikes/marker-lowering` copied unchanged, over the same source files.

Binaries that fail to compile are counted in all three, and stop early.
"""
import os, statistics, subprocess, sys, time

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


CHECK = False


def timed(workspace, package, target, env_extra, mode, rounds):
    bins = sorted(f[:-3] for f in os.listdir(f"{HERE}/{package}/src/bin") if f.endswith(".rs"))
    env = dict(os.environ)
    env.update(env_extra)
    env["RUSTFLAGS"] = "--cfg __locus_check" if mode == "check" else ""
    cmd = ["cargo", "check" if CHECK else "build", "-q", "-j1", "-p", package, "--keep-going",
           "--target-dir", target, "--bins"]
    run = lambda: subprocess.run(cmd, cwd=workspace, env=env, capture_output=True, text=True)
    run()  # the library, the macros, and a first build of everything
    times = []
    built = 0
    for _ in range(rounds):
        now = time.time()
        for b in bins:
            os.utime(f"{HERE}/{package}/src/bin/{b}.rs", (now, now))
        started = time.time()
        run()
        times.append(time.time() - started)
        built = sum(1 for b in bins if os.path.exists(f"{target}/debug/{b}")
                    and os.path.getmtime(f"{target}/debug/{b}") >= started - 1)
    return len(bins), built, statistics.median(times), min(times), max(times)


def main():
    global CHECK
    args = sys.argv[1:]
    rounds = 5
    if "--check" in args:
        args.remove("--check")
        CHECK = True
    if args and args[0] == "--rounds":
        rounds = int(args[1])
        args = args[2:]
    for package in args:
        for mode in ("runtime", "check"):
            rows = [("with skeletons", HERE, {}, f"{HERE}/target/cost-sk/{mode}"),
                    ("without (LOCUS_SPIKE_NO_SKELETON=1)", HERE, {"LOCUS_SPIKE_NO_SKELETON": "1"},
                     f"{HERE}/target/cost-none/{mode}"),
                    ("the first mode", f"{HERE}/first-mode", {}, f"{HERE}/target/cost-first/{mode}")]
            for label, workspace, env, target in rows:
                n, built, median, low, high = timed(workspace, package, target, env, mode, rounds)
                what = "cargo check" if CHECK else "cargo build"
                done = "" if CHECK else f"; {built} of {n} binaries built"
                print(f"{package}, {mode} expansion, {what}, {label}: median {median:.2f}s of {rounds} "
                      f"(min {low:.2f}s, max {high:.2f}s){done}")


main()
