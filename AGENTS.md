# Working on Locus

Locus is a Rust-like language with explicit, kernel-checked proofs and logical computation that erases when emitting Rust. Treat the compiler, specification, tests, and public documentation as one product.

## Read before starting

**Before starting a task, read the entire project documentation, not only the section that appears relevant.** Read:

- This file and the root [README.md](README.md).
- Every Markdown file under [docs/](docs/), including the language manual, reference contracts, diagnostics, roadmap, vision, historical plans, and archives.
- [website/README.md](website/README.md), [library/README.md](library/README.md), and the README files under [editors/](editors/).

Use `rg --files docs -g '*.md' | sort` to inventory the documentation. Read the full contents; a file listing, heading scan, or generated summary is not a substitute. After reading, inspect the implementation and tests relevant to the task. For an ongoing task, retain that context and reread documents that have changed.

Distinguish current contracts from proposals. The manual and reference contracts describe the implemented language. Vision and archived plans explain intent and history; they do not grant additional syntax or semantics. If implementation, tests, and documentation disagree, identify the discrepancy and resolve it with evidence. Do not silently rewrite a contract to conceal a compiler defect.

## Documentation map

| Location | Responsibility |
|---|---|
| `docs/specification.md`, `docs/spec/*.md` | Current language manual, organized into numbered conceptual chapters |
| `docs/reference/kernel.md` | Proof kernel contract and accepted proof rules |
| `docs/reference/formal-core.md` | Checking, lowering, erasure, and preservation obligations |
| `docs/architecture.md` | Compiler representations, boundaries, and implementation structure |
| `docs/examples.md`, `docs/overview.md` | Checked examples and getting started |
| `docs/correctness.md` | What establishes confidence, the trusted base, and remaining obligations |
| `docs/performance.md`, `docs/generated-status.md` | Measurement policy and generated, freshness-aware results |
| `docs/development.md` | Contributor commands, gates, diagnostics, and proof storage |
| `docs/diagnostics/*.md` | Compiler explanations and the diagnostic schema |
| `docs/roadmap.md`, `docs/roadmap/*.md` | Current project ownership and permanent `LOC-N` tasks |
| `docs/vision/` | Future language design and unresolved proposals |
| `docs/plans/`, `docs/archive/` | Historical plans and decisions; preserve their context |
| `docs/data/state.json` | Retained metadata and measurement receipts, not a second prose store |
| `website/` | Static renderer, assets, dependency lockfile, and publishing instructions |

Markdown is authoritative. `tools/content.py` loads it; `tools/site_data.py` prepares checked repository data; `website/build.mjs` renders it; `tools/site.py` validates and serves the output. `target/site/` is generated and must not be committed. Root `atlas.html` is a compatibility entry point, not an editable source database. `tools/atlas.py` retains command-line compatibility helpers.

## Keep documentation coherent

- Update the relevant manual, reference contract, examples, and roadmap in the same change as the implementation. Only describe behavior as implemented when the code accepts it and the appropriate checks pass. Mark partial support and limitations explicitly.
- Give each concept one authoritative explanation and link to it elsewhere. Use short orientation text where helpful; avoid copying long rules, task lists, or test inventories across pages.
- Organize the manual by language concepts, not implementation milestones. Keep research proposals and historical notes out of the current specification. Preserve useful design rationale without presenting abandoned choices as active work.
- Preserve TOML front matter, stable document IDs, and rule IDs. Routes must be unique relative `.html` paths. Use real relative Markdown links; the renderer maps them to published routes. Wrap syntax such as `Vec<T>` in backticks.
- A current page normally has `group = "Now"`; its code fences are checked. Historical and vision pages remain distinct. Do not change a page's group or a rule's category merely to bypass a gate.
- Every specification block needs a permanent marker such as `<!-- spec: 1.2:3 legality-rule -->`. IDs survive editorial moves and are not current chapter numbers. Do not renumber or reuse them. Treat changes to rule meaning as semantic changes, with corresponding tests and reviewable rationale.
- Every operative rule needs a focused test citation: `//~ spec: ID` in a corpus file of fewer than forty physical lines, or `#[doc = "spec: ID"]` on a Rust test after `#[test]`. A large corpus file may provide context but does not replace focused coverage. Verify the test actually exercises the rule; a syntactically valid citation is insufficient.
- Current documentation fences declare a language and mode: `rust check`, `rust run`, `rust reject Lxxxx`, or a non-executable form such as `text prose kernel-grammar`. The `rust` label enables GitHub highlighting; the mode makes it a Locus example for CI and the website. Run examples need `//~ run:` expectations; rejected examples need diagnostic codes; prose exemptions need an honest reason. Do not exempt an executable example just because it fails.
- Teaching excerpts may hide setup or test drivers between balanced whole-line `// docs:hide` and `// docs:show` comments inside a checked fence. The complete program must remain in Markdown and pass the same checks; never hide assumptions or weaken proof obligations to simplify the display. The renderer provides a complete-source disclosure. See [website authoring](website/README.md#checked-examples-and-excerpts).
- The website generates test panels from citations. Keep prose about behavior and rationale; avoid repeating lists of test filenames already available in those panels.

## Maintain one roadmap

Use `docs/roadmap/*.md` as the owning task records. Search existing tasks before adding a project or task; extend or consolidate overlapping work rather than creating another copy.

- Preserve public `LOC-N` numbers, metadata comments, and adjacent task anchors when moving tasks. Never reuse a number.
- Describe a concrete scope and completion condition. Split partial work from genuinely completed work; parsing a construct alone is not semantic implementation.
- Mark a task done only when its implementation and required validation are complete. Cite relevant code, tests, or measured results in its notes.
- Mark abandoned or superseded tasks explicitly and link to their replacement. Retain historical identifiers so references still resolve.
- Known-bug exemptions must name an open task and pin the expected failure. When the bug is fixed, remove the exemption and keep the reproducer as a passing regression. An unexpected pass, panic, or timeout must not count as the expected rejection.

## Protect the correctness boundary

- Proof search, elaboration, and stored certificates are not authorities. Accepted evidence must pass kernel checking in its actual context. Do not add axioms, trusted stubs, or permissive fallbacks merely to make a proof or test pass.
- Logical erasure must preserve ordinary evaluation and effects. A runtime function does not disappear because its result is logical. Logical observations still require valid ownership and borrowing permissions; erased information cannot control runtime behavior.
- Preserve snapshot/version identity through mutation, joins, calls, and loops. Do not make stale evidence usable by dropping dependencies or weakening the checker.
- Keep the checking IR, erased representation, interpreters, and Rust emission in agreement. Update their inventories and the formal core when adding forms. Differential disagreement is a defect to investigate; resource exhaustion is inconclusive.
- Preserve the safe-Rust export boundary for invariant-bearing values. Explicitly trusted native contracts remain assumptions and must retain their audit information.
- If a change affects the trusted base, update `tools/trusted-base.json` and the correctness documentation. Distinguish kernel soundness assumptions, whole-compiler assumptions, testing evidence, and mechanized proofs. Do not describe the compiler as formally verified when the relevant preservation proof remains deferred.

## Validate the change

Follow [docs/development.md](docs/development.md) for prerequisites and measurement policy. The standard development gate is `tools/check.sh`; use `tools/check.sh --extended` for milestone/stress work as specified there. Only its final `LOCUS GATE COMPLETE` receipt establishes that the selected gate completed. Report failures, skipped checks, and advisory timing overruns accurately.

For documentation or website changes, run:

```sh
npm ci --prefix website
python3 tools/spec.py check
python3 tools/site.py check
python3 tools/test_site.py
```

Run `cargo test --locked --offline --test atlas_fences` when executable documentation changes. For compiler changes, add focused positive and negative regressions and run the affected suites before the full gate. Use deterministic seeds and retain minimal reproducers. Inspect diagnostic and proof-output changes before updating goldens; do not broadly bless failures without understanding each difference.

For visible site changes, inspect desktop and narrow/mobile layouts, navigation, code blocks, search, and test disclosures in a real browser. Keep the site readable without JavaScript and working below a path prefix such as `/locus/`. Validate links and source escaping. Keep dependencies pinned and avoid unnecessary client-side frameworks or external assets.

## Test new language machinery across its boundaries

For every new language construct, write a coverage matrix before calling the work complete. Include accepted and rejected syntax, name/type resolution, interactions with existing control flow and data shapes, ownership and snapshot failures, erasure/effects, export/import boundaries, diagnostics, and source-file/package boundaries where relevant. Explain any deferred part in the owning plan or task.

Exercise the independent checking IR and erased checker with malformed inputs as well as compiler-produced inputs. For executable behavior, compare both interpreters and warning-denied Rust, including overflow modes, panics, evaluation order and deterministic generated cases where relevant. Proof-related changes need both valid evidence and near-miss/stale evidence tests, plus stored-proof replay when affected. A parser test or one happy-path example is not sufficient evidence for a new feature.

Prefer tests of observable behavior and soundness boundaries over tests mirroring helper implementations. Keep minimal regressions for discovered bugs; do not convert failures to expected errors just to make the suite pass. Use isolated temporary projects for CLI/build tests so they cannot alter a developer’s proof lockfiles. Report the coverage and material omissions, not merely a test count.

## Keep measured claims honest

Never invent timings, test counts, or historical samples. Do not hand-edit generated measurements to make status look current. The compiler benchmarks use the `locus-bench-data` branch; local gate timing history is separate. Compare only compatible benchmark epochs and disclose revision, date, and dirty status.

Freshness tracking must include every input checked by the gate, including website code and roadmap content, while excluding generated measurement output to avoid cycles. A stale report must hide obsolete current claims. Regenerate full status through the documented extended gate; a successful subset does not establish a full-suite result.

## Work cleanly

Inspect `git status` before editing. Preserve unrelated user work and review exactly what is staged before committing. Keep changes scoped and reviewable; use `rg` for repository searches and update existing tools rather than adding competing sources of truth. Commit relevant `Locus.lock` changes when proof obligations change; never commit `target/`, `node_modules/`, local logs, or fabricated receipts.

A repository push and a website deployment are distinct operations. The Website workflow builds on pushes and pull requests; publication is a separate manual action. Follow the user's requested scope and report which actions actually completed. Finish with a concise account of what changed, what was validated, and any remaining limitation.
