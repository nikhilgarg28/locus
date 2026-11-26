# Borrowed-key bridges: I08b spike

Status: experiment, 2026-11-26, based on PR26 at `1899466`. The owner requested
a spike before settling the proposed borrowed-lookup API. This records results
and recommendations; it adopts no new language semantics or trusted facts.
Nothing outside this spike may depend on its implementation.

## Result

The proposed projection/equivalence/hash/order architecture survives these
tests, with an explicit projection-validity obligation. Concrete providers and
existing standard adapters have viable Rust shapes. **One uniform, extensible
implementation-bound certificate is still a compiler/lowering gate.** A correct
independent method and a `Borrow<Q>` bound do not certify the native method.

On `rustc 1.100.0 (0ef1bc386 2026-11-09)`:

- 14 runtime tests pass in debug and optimized builds (28 executions).
- 2 separate downstream programs compile and run.
- 8 negative compiler probes fail with their expected error codes.
- 17 hand-written Z3 queries match expectations: 8 unsatisfiable negated goals
  and 9 satisfiable weakened-law countermodels.

See [results/runs.json](results/runs.json), [runtime output](results/tests-debug.log),
[law-query output](results/laws.log) and [models](results/laws.json).
No Locus compiler, kernel proof checker or source-to-VC translation ran. Rust
tests are observations at selected values; solver results concern only the
explicit formulas supplied. These are not proofs of a HashMap/BTreeMap, all
strings, all hashers, a compiler translation, or the standard library.

## What the code actually does

[rust/lib.rs](rust/lib.rs) implements proof-erased reference operations, an
ordinary macro instantiating `Borrow<str>` for a named local type, and a small
closed adapter registry. No macro infers semantic laws or checks Locus proofs.
[rust/tests.rs](rust/tests.rs) exercises real standard maps and instrumented
hash calls, including deliberately unlawful native implementations.

The recorder observes selected primitive calls on this toolchain. It is not
the full I04 semantic-event implementation; equal sample recordings do not
prove compatibility with every hasher. Its `finish` is deliberately constant
and unused for the trace assertions. Existing I04 adapter requirements remain.

## Runtime results

| Probe | Observed result | Consequence |
|---|---|---|
| Standard String/str | Both native maps find a separately allocated query; sample traces agree | Existing adapter route is viable |
| Generated Name/String provider | Both native maps find a str query | Concrete local code generation fits Rust |
| Reflexive borrowing, including str and slices | Existing blanket works, without another Borrow impl | Reuse the standard identity implementation |
| Owned<T> -> T | Works for str and byte slices | Some useful generic concrete provider shapes work |
| Vec<u8>/[u8] | Both maps find a slice query; sample native batch traces agree | Unsized queries matter; full slice certification remains I04 later work |
| Correct independent operation, wrong native Borrow | Operation returns alice; native tree lookup finds mallory instead | The actual selected native implementation must be bound |
| Case-insensitive key, raw str projection | Owned equality holds but borrowed equality disagrees; native lookup misses | Equality compatibility is independently necessary |
| Canonical lowercase str projection | Lowercase query succeeds; uppercase query misses | Normalizing stored projection can be lawful, but does not normalize queries |
| Correct projection/equality, salted key hash | Native key and query traces differ | BorrowEquiv does not imply BorrowHash |
| Reversed owned order, ordinary str order | Equal outcomes agree but directions differ; native tree lookup misses | Require the entire comparison result |
| ID-only key with display label | u32 lookup works; equivalent update retains original label | Projection may forget fields while preserving selected equivalence |
| Query outside stored projection image | Ordinary miss | Laws cannot assume every query was produced by borrowing |
| Hidden mutable Cell hash state | Same Unit observation, different traces | Snapshot stability/adequacy cannot be skipped |
| Panicking operation | No successful result leaves the call | Contracts remain normal-return claims; no new abort rule |

The deliberately bad keys violate Rust's documented collection laws. Their
behavior is permitted to be wrong; these tests do not report Rust bugs or
claim Locus would accept them. Native failure observations are pinned to this
toolchain; the underlying conflicting equations are the design counterexamples.

## Compiler boundaries

| File in cases/ | Rust result | Interpretation |
|---|---|---|
| accept_native_adapters | Compiles/runs | Existing String/str and unsized identity routes |
| accept_downstream_provider | Compiles/runs | Concrete provider emitted in type-owning crate |
| reject_identity_provider | E0119 | New Borrow<T> for T collides with core |
| reject_foreign_blanket | E0210 | Cannot install a global foreign blanket from the extension trait |
| reject_arbitrary_forwarding | E0119 | Even a local Wrapper<T> cannot forward arbitrary Q without potential identity overlap |
| reject_duplicate_provider | E0119 | Existing and generated Borrow do not coexist |
| reject_reference_outlives_owner | E0515 | Returned query cannot outlive owner |
| reject_mutation_with_live_query | E0502 | Owner cannot be changed while a shared query remains live |
| reject_forged_adapter | E0277 | Closed private registry blocks forged evidence |
| reject_generated_registration | E0277 | That same registry blocks a legitimate downstream generated provider |

The last row is a **limitation of the prototype**, not a proposed Locus
rejection. Sealing a Rust marker alone is not the solution to open certification.
Making it public again would let unverified clients claim arbitrary native
correspondence. Locus must export checked, implementation-bound metadata or use
another genuinely checked registration mechanism already required by its
provider design. This spike implements neither mechanism.

## Logical contract and checks

Let K and Q below denote frozen observations. For source runtime types, their
admissibility conditions must also be carried. Let p: K -> Q be the total pure
projection. Proposed obligations are:

1. **Validity:** validK(a) implies validQ(p(a)). An actual returned &Q is valid,
   but generic theorems over the logical projection also need this evidence.
2. **Native projection:** a normal return from the actual Borrow<Q>::borrow
   has observation p(entry), with Rust's shared-reference lifetime and existing
   snapshot framing. Independent verified code is not evidence about this body.
3. **Equivalence:** EK(a,b) iff EQ(p(a),p(b)) for valid source observations.
4. **Hash:** traceK(a) === traceQ(p(a)), connected to actual hashing paths.
5. **Order:** cmpK(a,b) === cmpQ(p(a),p(b)), connected to actual comparisons.

Hash collections additionally need lawful equivalence and EQ-compatible
hashing on queries, and compatible table builder states/interpreters. Tree
collections need lawful total query ordering, as well as cross-type agreement.
Ordinary Borrow alone promises none of these as machine-checked evidence.

Lookup is defined for every valid query q by EQ(p(k),q). The SMT queries check:

- a matching arbitrary query has the stored key's trace;
- two matching keys belong to one EK class (uses reflection);
- replacing a key by an EK-equivalent representative preserves matching (uses
  preservation);
- native lookup needs the actual implementation bound to p;
- comparison equality alone does not fix tree direction;
- compatible ordering plus query-order transitivity justifies one tree-pruning
  step for an arbitrary query;
- validity and builder-state premises cannot be omitted.

These are small local derivations with explicit law instances, not a complete
lookup proof or a proof that the trait definitions are consistent for all types.
Admissibility is an uninterpreted predicate in this model, not a new Locus axiom.

## Candidate Locus source and recommendation

[locus/interfaces.open.lc](locus/interfaces.open.lc) spells out the projection,
validity lemma, proof-bearing operation, provider and separate laws.
[generated-name](locus/generated-name.open.lc),
[existing-native](locus/existing-native.open.lc),
[mismatched-native](locus/mismatched-native.open.lc) and
[opaque-projection](locus/opaque-projection.open.lc) make the desired uses and
failures concrete. These are annotated sketches outside normative litmus:
`admissible`, text adapter APIs and native evidence packaging remain schematic.

Recommended direction, **not yet adopted**:

- Keep the pure projection and a separately usable proof-bearing operation.
- Bind native behavior through a concrete generated provider or a designated
  existing-implementation adapter. Do not equate the independent operation
  trait with native certification.
- Reuse the standard identity blanket; reject duplicate providers normally.
- Keep equivalence, hashing and ordering laws separate, explicitly naming the
  selected policies and preserving the query's own meaning.
- Add validity transport and preserve the existing stability/framing conditions.
- A wrapped snapshot must retain enough information for its projection. Trust
  cannot make a function distinguish two identical Unit-only snapshots.

Remaining gates before the generic native API is finished: cross-crate native
implementation evidence and provider/adapter unification; concrete admissibility
API; unsized snapshot marker lowering; actual Locus proof checking; and the
previously deferred I04 container/batch and designated-adapter registrations.
No new capture default, snapshot override, blanket impl rule, user hasher,
runtime body unfolding or unchecked certificate constructor is adopted.

## Reproduce

Use Python with the version in requirements.txt installed, then:

```sh
python3 spikes/borrow-bridges/run.py --rustc /path/to/rustc
```

The runner downloads nothing. It writes binaries to ignored target/ and logs,
commands, models and SMT-LIB queries to results/. If rustc is not on PATH, its
fallback is the existing toolchain used by this workspace. It exits nonzero
for a missing dependency, unexpected compiler error, wrong runtime assertion,
solver timeout/unknown, or mismatched solver result. Optimized runs optimize
the test crate; the small support rlib is built once without -O.
