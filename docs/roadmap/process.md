+++
summary = "Engineering gates and proof storage are delivered; benchmark observation remains in progress."
id = "p310"
name = "Engineering process and proof storage"
status = "active"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/process.html"
order = 2
kind = "project"
+++

# Engineering process and proof storage

Delivered engineering gates, diagnostics, trusted-site audit, checked documentation, the written formal core and version-2 proof lockfiles. The only unfinished task here is [LOC-204](process.md#LOC-204): its benchmark infrastructure works, but the required two-week same-epoch observation window is real elapsed evidence and remains open. Lean mechanization is separately deferred under [LOC-57](assurance.md#LOC-57).

Historical Process acceptance: complete tools/check.sh --extended passed with a 221s fast suite against the advisory 120s target. P12 later passed the full fast gate and its migration checks; its task retains the exact historical measurements and does not claim a new extended run. Current counts and freshness belong to Generated status. The specification source is now Markdown; the website is derived from it.

<a id="LOC-194"></a>
## LOC-194 · P1 · Paragraph IDs, categories, and a traceability gate for the Kernel contract and Language, as built
<!-- task: {"id": "t311", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

Current documentation source: versioned Markdown loaded by tools/content.py; the static website is generated output. Traceability, known-bug validation and checked fences read that canonical content, not a hand-maintained HTML snapshot.

When: now. After: nothing.

Scope. Every paragraph of the Kernel contract and of Language, as built carries an identifier in the form chapter.section:paragraph and a category: normative, legality-rule, syntax, dynamic-semantics, informative, example. A paragraph without a category is informative; normativity is opted into, never inherited. Tests cite paragraphs: a corpus file with //~ spec: 7.2:3 or a Rust test with a spec attribute. A gate in the suite fails when a normative paragraph has no focused test citing it (a focused test is a corpus file under forty lines or one Rust test), when a citation names a paragraph that no longer exists, or when a behaviour-asserting test cites only an informative paragraph. The existing contract test, which checks that every kernel name appears in the contract, becomes one of the gate's rules.

Why. The document a reviewer reads and the tests the machine runs become the same artifact; today the kernel-contract test covers names only, and R4 had to rewrite Language, as built by hand because nothing tied its sentences to the code. Rue's spec (docs/spec, crates/rue-spec) does this with a rule() marker per paragraph and a 100 percent coverage gate.

Tests. The gate itself on a scratch copy of the atlas with a paragraph removed, a citation dangling, and an informative paragraph cited by an asserting test; the count of normative paragraphs and of citations printed by the extended gate.

Done when. tools/check.sh fails on an uncovered normative paragraph, and the atlas shows the IDs in the rendered documents.

Implementation accepted. Stable rendered paragraph IDs, local categories, strict focused citations, dangling/informative-only checks, rule inventories and adversarial traceability tests implemented. tools/spec.py is integrated in the gate.

<a id="LOC-195"></a>
## LOC-195 · P2 · Executable known-bug markers in the corpus
<!-- task: {"id": "t312", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

Current documentation source: versioned Markdown loaded by tools/content.py; the static website is generated output. Traceability, known-bug validation and checked fences read that canonical content, not a hand-maintained HTML snapshot.

When: now. After: nothing.

Scope. A corpus file may carry //~ known: LOC-NN with a reason. The runner expects the file to fail as pinned; if it passes, the suite fails, naming the marker to remove. The same marker on a Rust test through a small attribute or helper. The task number must be an open task in the atlas, checked by the meta test that reads atlas.html.

Why. A skipped or ignored test outlives its bug. Rue's known_bug = "RUE-NN" on CLI cases is an executable xfail: an unexpected pass is a failure, so a fixed bug cannot leave a stale marker.

Done when. One real known bug from the discussion note (the fold! of a struct literal that projection reduction does not match, found by E9) is pinned this way, and removing the workaround in the code makes the marker fail loudly.

Implementation accepted. Known-bug directives and Rust helper implemented with pinned failure, reason and open-task checks; changed failures and unexpected passes fail loudly. The specified E9 struct-literal fold bug was already fixed by Reconciliation, so its real reproducer is a passing regression. Synthetic harness tests cover the executable known-bug lifecycle; no stale real xfail is retained.

<a id="LOC-196"></a>
## LOC-196 · P3 · A reason on every trusted site, and locus audit
<!-- task: {"id": "t313", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

When: now, and again when trusted declarations arrive. After: nothing.

Scope. locus audit lists, for a file or a crate, everything a reviewer must take on trust: every function that does not promise terminates (a source of divergence), every panic ending and operator without evidence in a function without no_panic, every use of classical reasoning in the theory and, when the surface can invoke it, in source, and later every foreign or trusted declaration. Where the language gives the programmer a way to assert something the compiler does not check (trusted declarations in the Interop project, a lemma admitted without proof if that is ever allowed), the reason is a mandatory string literal in the grammar between the keyword and the item, kept in the tree so that a formatter cannot separate it and the audit prints it beside the site.

Why. The one part of the language the compiler cannot check is where a stated reason earns a reviewer's trust, and for code written by an agent the cost of stating it is nothing. Rue's checked "reason" { } (ADR-0095) makes it grammar rather than a lint. Positioning already promises the audit command.

Tests. The audit over the examples is a golden; a trusted site without a reason is a rejected corpus file once the grammar exists.

Done when. locus audit exists and its output for the target examples is committed as a golden; the reason clause is designed in the Vision for the declarations that will need it.

Implementation accepted. locus audit checks sorted files/directories and reports divergence, panic/allocation, classical dependencies and trusted reason strings. Target/foreign-site goldens and multi-file discovery/rejection regressions pass.

<a id="LOC-197"></a>
## LOC-197 · P4 · An appendix of every limit, each with its diagnostic
<!-- task: {"id": "t314", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

When: now. After: P1.

Scope. One appendix of the Kernel contract lists every count the checker and the elaborator enforce: nesting depth and steps in the kernel, evaluation steps and depth, the certificate limits, the arithmetic procedure's budgets, the parser's depth, the stored-proof reader's limits, the diagnostic and source-size ceilings, with the rule that exceeding a limit is a diagnosable failure that names the limit and never wraps, truncates, or drops silently. A test reads the appendix and compares the names and values with the constants in the code, as the contract test does for axioms.

Why. Limits as counts are a principle of the language; today they are scattered through source and task notes. Rue's Appendix C states each limit and the rule against silent wrapping.

Done when. The appendix exists, the test passes, and every limit constant in src has a row.

Implementation accepted. All counted implementation limits centralized with a checked Kernel appendix. Boundary tests cover source/diagnostic/parser/store/arithmetic/normalization limits; exhausted heuristics report the relevant ceiling without proving or refuting the unresolved claim.

<a id="LOC-198"></a>
## LOC-198 · P5 · The gate says that it ran
<!-- task: {"id": "t315", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T00:03:12.000Z"} -->

When: now. After: nothing.

Scope. tools/check.sh and its extended form end with one line that says the whole gate ran and how long the fast suite took; tests/acceptance.rs asserts on that line rather than on counts, and a run cut short by a resource limit or a killed process cannot print it. The fast-suite time is recorded on each run so that the two-minute criterion is a series and not a single measurement.

Why. A harness killed by memory or a timeout can still print a plausible summary. Rue's testing doc calls this "read the banner, not the tally".

Done when. Killing the suite midway leaves no banner, and the acceptance test fails.

Implemented as Reconciliation T2 prerequisite. tools/check.sh emits LOCUS GATE COMPLETE only after the selected fast/extended gate and a timing-history append succeed. tools/test_gate.py runs the real script with substitute build tools; failure with a plausible passing tally and SIGTERM interruption both leave no receipt or history record. tests/acceptance.rs executes these checks. Timing samples append to target/gate-history.jsonl; P9 consumes them.

<a id="LOC-199"></a>
## LOC-199 · P6 · Preview gates: incomplete features merge behind a flag
<!-- task: {"id": "t316", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T00:38:48.000Z"} -->

When: with the reconciliation, before its first cross-cutting commit. After: nothing.

Scope. locus check --preview name enables a named, unfinished feature; the elaborator gates each new construct with a call that reports "x requires preview feature name" without the flag; corpus files declare //~ preview: name and are expected to fail without the flag and to pass with it (or to fail as pinned with it, while the feature is partial); the golden runner and the meta test understand the directive; stabilisation removes the gate and the directive in one commit. The list of previews is a closed enum, each naming the atlas task that owns it.

Why. The Vision migration (logic fn, Bool, named-arm props, the removal of Ghost) cuts across the whole elaborator, unlike the core build's 41 independent commits. Gates let the new language grow beside the built one on main while every commit keeps the suite green. Rue's ADR-0005.

Done when. The first reconciliation commit lands behind a gate with its corpus files marked, and the built language's tests are untouched.

Implemented during Reconciliation. Closed task-owned registry, missing-preview diagnostics, corpus directives and lifecycle inventory tests are in place. All four Reconciliation previews are now stabilized; obsolete flags/directives are rejected.

<a id="LOC-200"></a>
## LOC-200 · P7 · Diagnostics as a versioned JSON surface, and locus explain
<!-- task: {"id": "t317", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

When: with the reconciliation. After: nothing.

Scope. locus check --error-format json prints one JSON array per diagnostic on stderr with a fixed key set and a schema version: severity, code, message, spans (with one primary), notes, helps, suggestions, and Locus's own fields: the claim, the claim after computing, the facts considered, the counterexample, and the suggested explicit form, each null when absent. Diagnostics are sorted by file and offset before rendering, so the order is deterministic. locus explain L0230 prints a long-form explanation of a code with its causes, an example, and the paragraph of Language, as built it rests on (P1). Error codes get ranges by origin: lexer and parser, elaborator, lowering and checker, driver, internal.

Why. Positioning asks for diagnostics as data with the claim, the facts, the gap, and the failing case; the consumer is an agent writing the implementation file. Changing a field is then a consumer-visible break, documented as such. Rue's --error-format json and rue explain.

Tests. The JSON for every rejected corpus file is a golden beside the text golden; a schema test checks the key set; explain has a golden per code and the meta test requires one.

Done when. Every code has a JSON golden and an explanation.

Implementation accepted. Versioned JSON diagnostic schema, structured proof fields, mapped/sorted source spans, every-code explanations and text/JSON goldens implemented. L0300 separates checker-origin rejection from elaborator errors. Schema and evolution policy documented.

<a id="LOC-201"></a>
## LOC-201 · P8 · Code blocks in the atlas are checked by the suite
<!-- task: {"id": "t318", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

Current documentation source: versioned Markdown loaded by tools/content.py; the static website is generated output. Traceability, known-bug validation and checked fences read that canonical content, not a hand-maintained HTML snapshot.

When: with the reconciliation. After: P6.

Scope. Every fenced code block in the Now documents (Language, as built; Overview; Architecture where it shows source; the Kernel contract where it shows terms) carries a mode: check, run with expected values, reject with a code, or prose. A tool exports the blocks with their modes, and a test checks each against the compiler, with previews declared where a block shows a gated feature. Vision documents are exempt until their feature lands, when the block moves to Now or gains a mode.

Why. The atlas is the only documentation; R4 had to rewrite Language, as built because nothing had tied its examples to the code, and Target examples were prose until the build made them tests. Rue checks every tutorial fence (check, compile-fail E####, skip) in CI.

Done when. Language, as built has no unchecked code block, and editing one to lie fails the suite.

Implementation accepted. Every Now fence has an explicit check/run/reject/prose mode. Compiler/interpreter/compiled-Rust checks and adversarial documentation tests are integrated. Vision remains exempt until implemented.

<a id="LOC-202"></a>
## LOC-202 · P9 · The status numbers are generated, not typed
<!-- task: {"id": "t319", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

Current documentation source: versioned Markdown loaded by tools/content.py; the static website is generated output. Traceability, known-bug validation and checked fences read that canonical content, not a hand-maintained HTML snapshot.

When: with the reconciliation. After: P5.

Scope. The counts the atlas shows (tests, corpus files, obligations by tier for the target examples, kernel rules and axioms, trusted-base lines by file, stored-proof hits, the fast-suite time) are produced by tools/check.sh --extended as one JSON record, and a tool writes them into the atlas status section from that record; the Overview cites them from there. Nothing in those numbers is typed by hand.

Why. Numbers typed by a person drift; the core build's were typed by me at each landing. Rue's homepage field report is derived from the repository at build time.

Done when. The extended gate updates the numbers and a stale number is impossible.

Implementation accepted. Extended gate produces the JSON measurement record and Generated status directly from complete test logs, proof-tier/replay samples, spec inventory and trusted-file lines. Current numbers are linked from Overview, not duplicated. Fingerprints include compiler/embedded inputs, tests, targets, libraries, examples, tools, editor sources and Now docs; stale display hides obsolete counts. Source changes during a gate reject publication.

<a id="LOC-203"></a>
## LOC-203 · P10 · A formal core for the check IR, lowering, and erasure, with three-way agreement
<!-- task: {"id": "t320", "status": "done", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

Completed scope: the written calculus, rule inventory and executable agreement checks. Lean mechanization is intentionally deferred and remains [LOC-57](assurance.md#LOC-57); no mechanized erasure theorem is claimed.

When: later, after the reconciliation. After: P1.

Scope. A document in the Now group states the trusted parts outside the kernel as a small calculus: the typing rules of the check IR (lets, facts, calls, matches, loops with state, return and panic endings, operations that may panic, promises), the lowering of the typed tree to it (versions, joins, loop state, write-backs), and erasure, each rule citing the paragraph of Language, as built it implements (P1) and the Architecture section that motivated it. The erased interpreter is named as the executable oracle of the calculus, the check-IR interpreter as the reference for the IR, and the rule of agreement is stated: prose, calculus, and the two interpreters must agree, a disagreement is a defect in whichever is wrong, and a report names the pair that disagrees, never a winner. The Lean deliverables are listed for the day mechanisation starts: a fixed axiom set, a trust report with the sorry count and printed axioms, a generated index from rule to Lean declaration to paragraph, and a validation procedure a reader can run in half an hour.

Why. Positioning names lowering, the checker, and erasure as trusted and says a sound kernel cannot establish that the program checked and the program run agree; the erasure theorem is the long-term answer and this document is its statement. Rue keeps a prose spec, a core calculus, a Lean mechanisation, and the compiler as four views (docs/formal, ADR-0097).

Done when. The calculus covers every form of the check IR and every lowering rule in Architecture, and the differential tests are described as its oracle.

Implementation accepted. Formal core covers check-IR constructors and static rules, lowering/versions/joins/state/writebacks/permissions, erasure/export and the agreement obligation. Inventory and focused citations checked. Mechanized Lean work intentionally deferred by user instruction; future fixed-axiom, sorry/axiom-report, rule-index and reproducible-validation requirements are documented.

<a id="LOC-204"></a>
## LOC-204 · P11 · Measurement: locus bench, with epochs and flags against noise
<!-- task: {"id": "t321", "status": "doing", "priority": 3, "created": "2026-09-23T00:03:12.000Z", "updated": "2026-09-23T04:35:37.000Z"} -->

Status: tooling delivered after [LOC-202](process.md#LOC-202); real observation period still in progress.

Scope. locus bench runs the corpus and records, per obligation, the search time by tier, the proof size in nodes, the kernel checking time, the certificate size, and the stored-proof hit; per file the elaboration, lowering, and erasure times and the fast-suite time; all as raw integer samples in one record per run, content-addressed and appended to a data branch, never overwritten. An epoch pins the workload set, the machine class, and the toolchain; a change to a pinned input turns the epoch. A workload is flagged when its median moves by more than k times the pooled dispersion of the run and the trailing window, and a series that stops advancing for more than a few commits fails the gate. The generated status page shows the per-workload sparklines and the headline index, derived at build time from the raw records (P9).

Why. Principles says to measure proof construction time, proof size, checking time, and rechecking cost before large proofs are normal; the one exit criterion the core build missed was a time bound measured once by hand. Rue's ADR-0067 and ADR-0072 give the method: raw samples kept, pins made explicit, flags against noise rather than percentages, and a real program as the workload.

Done when. Two weeks of records exist for the target examples and one deliberate slowdown is flagged.

Tooling accepted; observation period still open. locus bench records search/replay, per-obligation tier time/nodes/certificate size/kernel recheck, file phases and gate timing on the append-only content-addressed locus-bench-data branch. Epochs pin workload/library roles/order/content, actual build toolchain/settings, machine class and options; actual binary/source identities are recorded. Target-series freshness is enforced, robust slowdown detection has a deliberate-slowdown regression, and the status page derives sparklines/headline index from real records. The required two weeks of same-epoch observations have not elapsed and are not claimed. No historical records fabricated; no recurring automation created.

<a id="LOC-232"></a>
## LOC-232 · P12 · Locus.lock: directory lockfiles with named proof steps
<!-- task: {"id": "t350", "status": "done", "priority": 2, "created": "2026-09-23T00:51:44.000Z", "updated": "2026-09-23T04:53:57.000Z"} -->

When: now. After: nothing.

Scope. The stored proofs of E11 move from one sidecar per source to one Locus.lock at the root of what locus check was given (the crate root, or the directory of a single file), written and read with the toml crate, the first dependency of the compiler. The file has version = 2, then [[file]] tables in path order, each with path and [[file.obligation]] tables in order of position with key (the hash of the obligation as E11 defines it), at (the function and ordinal, reader-only), claim (the conclusion the stored proof proves, in the text form), and steps, a multi-line literal string of named steps: tN = term and sN = proof, one per line, a line may refer to earlier names, shared subterms and subproofs written once (hash-consed by the writer), the last line the conclusion. The reader parses each line, resolves names, expands the DAG into the tree the kernel checks, and hands it to the kernel as before: nothing trusted changes. A stale entry now reports "the stored proof concludes X, the obligation wants Y" using the claim field. The writer numbers steps in first-use order and rewrites the file whole; comments are not preserved. Migration: a run that finds version-1 sidecars reads them, writes Locus.lock, and deletes them, once. --locked, --no-store, LOCUS_PROOFS=off, LOCUS_SEARCH=none, --holes and --stats keep their meaning.

Why. Fifty sidecars in a crate are clutter, and the tree form repeats every shared subproof; a DAG of named steps is smaller, readable, and diff-friendly, and a single lockfile is what a reviewer and CI expect. Decided with Nikhil on 22 September; recorded under Found proofs are stored.

Tests.
- E11's round trip, reformatting, hostile-input, --locked, and weakened-search tests pass over the new file; the file is byte-identical on a second run; sharing is asserted on the lock's run entry (a subproof written once, referred to twice).
- The size of Locus.lock for the examples is at most that of the eight sidecars together.
- Migration from the committed sidecars, and the sidecars gone from the repository.
- The toml reader fed random bytes and mutated files never panics and is a miss or an error; a step naming a later or unknown step, a cycle, and a wrong claim are misses.

Accepted scope. Each source directory has a committed Locus.lock, and every stored corpus proof rechecks through it. A shared crate root is deferred to [LOC-41](interop.md#LOC-41), [LOC-42](interop.md#LOC-42); the original single-repository-file criterion was revised to match the actual single-file driver.

Implementation accepted. P12 is integrated with the reconciled language, bounded readers, JSON diagnostics, explicit library bundles and in-memory benchmark replay. All 36 committed v1 sidecars were migrated through successful checking to three directory lockfiles (examples, acceptance corpus and target corpus): 123522 bytes became 104046 bytes. All 36 sources then passed --locked with LOCUS_SEARCH=none and byte-identical lockfiles. The current single-file check command uses its source directory as the lockfile root; a shared crate root remains for future multi-file checking, so this repository intentionally has three lockfiles rather than one. Claim text is display metadata; authority comes only from checking the stored certificate against the current obligation. The expanded-tree and cumulative named-step node budgets are centralized and tested, including a 1 MiB stack boundary. tools/check.sh passed: 857 tests, 0 ignored, fmt and all-target clippy clean; fast tests took 203s against the advisory 120s target. Historical full-suite measurements are correctly marked stale after the source change; no new full extended-gate result is claimed.
