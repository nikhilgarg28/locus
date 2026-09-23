+++
summary = "Mechanized soundness proofs and independent certificate checking beyond the implemented formal core."
id = "p69"
name = "Formal assurance"
status = "planned"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/assurance.html"
order = 8
kind = "project"
+++

# Formal assurance

The written formal core and three-way differential tests are implemented; they do not constitute a mechanized preservation theorem. [LOC-57](assurance.md#LOC-57) retains the user-deferred Lean work with a reproducible trust report and rule index. [LOC-58](assurance.md#LOC-58) covers genuinely independent certificate checking, distinct from replaying proofs in the current compiler kernel. These are assurance extensions, not prerequisites silently added to already completed release tasks.

<a id="LOC-56"></a>
## LOC-56 · Differential testing, which exists today
<!-- task: {"id": "t70", "status": "done", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-21T19:19:39.000Z"} -->

Delivered. tests/differential.rs, tests/random_programs.rs, tests/corpus.rs and the checked/erased interpreters compare generated Rust behavior, including panic and erasure. These are executable oracles and regression evidence; the formal theorem remains [LOC-57](assurance.md#LOC-57).

<a id="LOC-57"></a>
## LOC-57 · Mechanize the check/lower/erase correctness argument
<!-- task: {"id": "t71", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Deferred by the user, not completed by [LOC-203](process.md#LOC-203). The formal core and executable differential oracles are delivered; remaining work is a machine-checked preservation/agreement theorem. Before Lean implementation, freeze an axiom set, publish sorry/axiom reports, generate rule-to-declaration-to-paragraph links and provide a reproducible validation procedure of roughly half an hour. Cover effects, panic, provenance, mutation and logical erasure, not only pure expressions.

<a id="LOC-58"></a>
## LOC-58 · An independently checkable proof artifact
<!-- task: {"id": "t72", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Stored certificates currently replay in the same Locus kernel ([LOC-232](process.md#LOC-232)). Remaining: a separately distributable/minimal checker or export to an independent proof system, with a stated trust boundary, format version and reproducible verification. Kernel-only replay inside the existing compiler is evidence toward this, not completion.
