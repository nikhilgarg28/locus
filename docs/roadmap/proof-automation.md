+++
summary = "More proof automation, reusable lemmas and logical models checked by the existing kernel."
id = "p61"
name = "Proof ergonomics and model libraries"
status = "planned"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/proof-automation.html"
order = 6
kind = "project"
+++

# Proof ergonomics and model libraries

One proof-development backlog replaces the separate stronger-inference, proof automation, logic-growth and recursion lists. Additional hole procedures ([LOC-51](proof-automation.md#LOC-51)), checked lemmas ([LOC-52](proof-automation.md#LOC-52)) and explicit hints ([LOC-84](proof-automation.md#LOC-84)) are distinct tasks. Seq/Nat/Maybe, a finite-map representation, single logical self-recursion and quantifiers are delivered. Remaining map/set laws, congruence, library proof APIs and mutual/runtime function recursion build on that core without weakening kernel checking. Runtime collections are tracked by [LOC-55](memory-layout.md#LOC-55).

<a id="LOC-26"></a>
## LOC-26 · Logical Set and a fuller finite-map interface
<!-- task: {"id": "t31", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Seq, Maybe, Nat and a representation-based FiniteMap are delivered ([LOC-223](reconciliation.md#LOC-223), [LOC-227](reconciliation.md#LOC-227); library/logical.lc and library/finite_map.lc). Remaining: Set plus map lookup/update/removal and their laws, extensional equality where desired, and reusable model interfaces. The present FiniteMap guarantees unique keys and checked prepend; it is not a complete map library.

<a id="LOC-51"></a>
## LOC-51 · Additional deterministic proof-hole procedures
<!-- task: {"id": "t62", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Exact facts, checked normalization, closed evaluation, linear arithmetic and stored replay are already implemented ([LOC-167](core-build.md#LOC-167), [LOC-173](core-build.md#LOC-173), [LOC-232](process.md#LOC-232)). Remaining: identify useful missed goals, select deterministic certificate-producing procedures and expose the attempted facts/gap. Keep heuristic failure distinct from falsehood; every result still goes through the kernel. Library lemmas and explicit hints belong to [LOC-52](proof-automation.md#LOC-52), [LOC-84](proof-automation.md#LOC-84).

<a id="LOC-52"></a>
## LOC-52 · Grow the proved lemma library
<!-- task: {"id": "t63", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Integer/machine lemmas and initial Nat/Seq/map/relations lemmas exist ([LOC-166](core-build.md#LOC-166), [LOC-223](reconciliation.md#LOC-223), [LOC-227](reconciliation.md#LOC-227)). Remaining: reusable model and collection laws driven by real programs, with documented prerequisites and proof-cost measurements. Prefer ordinary checked library code to new axioms. Distinct from changing hole search ([LOC-51](proof-automation.md#LOC-51)).

<a id="LOC-53"></a>
## LOC-53 · Recursion beyond single logical self-recursion
<!-- task: {"id": "t65", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Delivered: recursive logical data including mutual data groups, logical structural recursion, Int-measured recurse! and proof induction ([LOC-221](reconciliation.md#LOC-221), [LOC-222](reconciliation.md#LOC-222)), plus physical Box-recursive types. Remaining: mutually recursive functions and ordinary runtime recursive calls/contracts; distinguish those from already-supported mutual data declarations. Use deterministic termination certificates for total claims.

<a id="LOC-67"></a>
## LOC-67 · Callable congruence and proof automation
<!-- task: {"id": "t83", "status": "backlog", "priority": 0, "created": "2026-09-21T19:19:39.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Logical function values/captures are delivered ([LOC-225](reconciliation.md#LOC-225)). Remaining: additional congruence reasoning for symbolic applications, with explicit certificates and measured proof size; do not assume function extensionality. Coordinate [LOC-51](proof-automation.md#LOC-51) rather than creating a separate general solver backlog.

<a id="LOC-79"></a>
## LOC-79 · Move suitable proof forms into checked libraries
<!-- task: {"id": "t172", "status": "backlog", "priority": 0, "created": "2026-09-21T19:37:49.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Remaining design/implementation experiment: explicit predicates for equality transport, nameable function-definition equations and organized logic lemma APIs. Generics and logical closures now exist ([LOC-219](reconciliation.md#LOC-219), [LOC-225](reconciliation.md#LOC-225)), so compare library spellings with current rewrite!/fold!/unfold! without adding unchecked power. It is not a promise that all forms can cease being compiler primitives.

<a id="LOC-84"></a>
## LOC-84 · Explicit hints for prove!
<!-- task: {"id": "t178", "status": "backlog", "priority": 0, "created": "2026-09-21T19:41:53.000Z", "updated": "2026-09-23T05:30:08.935417+00:00"} -->

Still a design choice. Earlier candidate: prove!(P, using lemma, fact). Decide how hints name/apply checked lemmas and how failures report the missing proposition. Coordinate [LOC-51](proof-automation.md#LOC-51) and [LOC-52](proof-automation.md#LOC-52); do not conflate an explicit hint API with implicit unbounded lemma search.
