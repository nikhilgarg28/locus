+++
id = "development-workflow"
title = "Development workflow"
group = "Now"
created = "2026-09-23T04:07:15.000Z"
updated = "2026-09-23T04:43:11.000Z"
route = "guide/development.html"
order = 16
+++

# Development workflow

The compiler, documentation, and measurement tools live in the same repository. Rust 1.91 or newer builds the compiler. The full development workflow also uses Python 3.11 or newer, Node.js 20 or newer, and npm 10 or newer. Fetch Cargo dependencies before running the offline compiler gate; install the website and editor dependencies for their checks.

## Build the documentation

Canonical content is Markdown with TOML front matter under `docs/`. Edit those files directly. `tools/content.py` loads their metadata and bodies; `tools/atlas.py` retains compatibility commands for the older authoring workflow. Generated HTML is output, not the source of the specification.

~~~sh prose shell-commands
npm ci --prefix website
python3 tools/site.py build
python3 tools/site.py check
python3 tools/site.py serve
~~~

The site is written to `target/site`. The published pages link the current specification, examples, correctness argument, performance records, and roadmap. Repository archives retain earlier plans and design discussions without presenting them as implemented language rules.

## Run the compiler gate

~~~sh prose shell-commands
cargo fetch --locked
npm install --prefix editors/vscode/locus
tools/check.sh
tools/check.sh --extended
~~~

The fast gate checks formatting, warnings-denied clippy, specification traceability, executable documentation, compiler tests, website rendering and links, and benchmark freshness. The extended gate also runs the release stress suite, benchmarks the target programs, and regenerates [status](generated-status.md). The website commands above also support focused checks during documentation work.

Only a successful complete selected gate emits `LOCUS GATE COMPLETE` as its final line. A successful Cargo tally followed by a killed or failed step is not completion. Timing history appends to `target/gate-history.jsonl`. The fast-suite target is 120 seconds after compilation warm-up; an overrun is reported but is not a correctness failure. No benchmark duration determines whether a proof is accepted.

## Connect specification and tests

The [language manual](specification.md), [kernel contract](reference/kernel.md), and [formal core](reference/formal-core.md) use permanent paragraph IDs. Preserve an ID when moving its paragraph. A rule's category is local; an omitted category means informative.

Tests cite IDs with `//~ spec: 1.2:3` in a corpus file or `#[doc = "spec: 1.2:3"]` on a Rust test. An operative paragraph needs a focused citation: one Rust test or a corpus file with fewer than forty physical lines. Review must still establish that the test exercises the rule; the gate checks the references and coverage, not semantic equivalence of prose and implementation.

Current documentation fences declare `check`, `run`, `reject CODE`, or `prose REASON`. Run fences include expected values. The harness compiles the complete blocks and compares runnable examples across both interpreters and Rust. The Website workflow runs it on every branch push and pull request before uploading an artifact. The internal `Now` metadata group identifies documents subject to this fence gate; archived design proposals do not become checked examples merely by appearing in the repository.

~~~sh prose shell-commands
python3 tools/spec.py check
python3 tools/spec.py fences --out target/doc-fences
~~~

For a shorter teaching excerpt, put setup or test-driver lines between `// docs:hide` and `// docs:show` inside a checked fence. The HTML offers the full program in a disclosure; CI still checks every line. Markers must balance within one fence and cannot nest. Keep important assumptions and proof steps visible. See the [authoring example](../website/README.md#checked-examples-and-excerpts).

A known-bug marker pins a failure and an open roadmap task. An unexpected pass, a changed pinned failure, or a closed task fails the suite. Harness panics and timeouts cannot stand in for the expected compiler rejection. When a fix lands, remove the marker and retain the reproducer as a passing regression.

## Inspect trust and diagnostics

`locus audit FILE_OR_DIRECTORY...` checks each discovered source unit and lists functions without termination promises, unchecked panic sites, native allocation, classical dependencies, and explicitly trusted adapters with their reason strings. Discovery is sorted and skips symlinks, `target`, and `.git`; it does not introduce module resolution. An audit reports assumptions rather than proving them.

`locus check FILE --error-format json` emits one schema-versioned, single-element diagnostic array per line on stderr. `locus explain L0230` gives a code-specific explanation and specification citation. The [diagnostic schema](diagnostics/schema.md) defines fields and compatibility; text and JSON have separate goldens. A bounded failed search is not a refutation of the requested proposition.

## Build modules and packages

See the [package guide](packages.md) for entry files, Cargo metadata, build.rs integration and reusable theorems. The module driver keeps original file locations in diagnostics. Its `check` command owns one host-package proof lockfile; dependency packages remain read-only.

## Reuse checked proofs

`locus check path/file.lc` records the proofs it uses in `path/Locus.lock`. Commit that TOML file alongside the sources. `locus check path/file.lc --locked` requires every obligation to replay through the kernel without search or writes. `--no-store` bypasses storage for search experiments. Checking one source preserves neighboring files' lockfile entries.

Older `<source>.proofs` files are read only when that source has no table in `Locus.lock`. A successful unlocked check writes canonical named proof steps into the lockfile before deleting the migrated sidecar. Locked checking can read a legacy sidecar but leaves migration for an unlocked check. Stored claims and labels never substitute for kernel checking.

## Record measurements

`cargo run --release -- bench` benchmarks `tests/corpus/target` by default. Pass files or directories to select workloads, `--library FILE` to supply declarations, `--samples N` to choose repetitions, or `--no-record` to inspect raw JSON without recording history. Benchmarking uses isolated in-memory stores and never changes source lockfiles. The [performance page](performance.md) defines the timing boundaries and comparison policy.

Records append to the local Git branch `locus-bench-data`, without changing the checked-out branch or index. Retain and share that branch separately when moving machines. `python3 tools/bench.py summary` reads the history. `python3 tools/bench.py check` requires release-build history for the current target corpus, machine, and toolchain on an ancestral revision no more than five source commits behind. An unrelated workload does not refresh that series.

The extended gate produces `target/status-record.json`, `target/status.json`, and [generated status](generated-status.md) from complete logs and measured data. Its source fingerprint covers compiler inputs, tests, tools, examples, editor files, website sources, and documentation, including the roadmap. A changed fingerprint makes the report stale; old counts must not be presented as current. `python3 tools/metrics.py check` checks the displayed report, and `--fresh` additionally requires a current complete measurement. The extended gate rejects a checkout changed while its tests were running.

A published static report describes its recorded revision. It cannot monitor subsequent repository changes. Trusted-base counts use the conservative whole-file inventory in [tools/trusted-base.json](../tools/trusted-base.json); they are distinct from the size of the proof kernel. The [correctness page](correctness.md) explains the remaining preservation and mechanization obligations.
