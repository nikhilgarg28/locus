> Repository import: I01 was subsequently adopted by the owner (RFC 0002,
> decision 125). The experimental report below is preserved as evidence, not a
> claim that the compiler exists. The worked Locus source moved to
> [studies/mutable-loans](../../studies/mutable-loans/README.md); negative and
> incomplete boundary fragments remain in `locus/`. The prior stdlib Rust
> implementation is [beside this spike](../stdlib/README.md), and its target
> Locus modules are in [studies/stdlib](../../studies/stdlib/README.md).

# Mutable-loan spike for Locus

This spike tests I01: lending an invariant-read field when the acquisition
contract proves the invariant at the loan's final value. It incorporates Claude's
latest proposal to keep only `final!(r)` and introduce a named acquisition
reference with one runtime use. It does not adopt a language decision.

**Result: the narrow rule has a viable concrete candidate.** The initialized-prefix
obligation holds for arbitrary element writes under the modeled permission
contract. Handwritten Rust lowering works, including the previous stdlib spike's
raw-memory Region. Sortedness, unrelated enclosing invariants, deferred
acquisition and separate ghost-cache restoration do not follow automatically.
Several lowering and source-rule details still need specification.

**No Locus program was compiled or kernel-checked.** The `.lc` sources are open
design inputs, with explicit interface/lemma obligations and proof holes.
SMT conditions are hand-translated. Rust, ASan and Miri exercise the Rust model,
not a Locus compiler. No repository spec, decided litmus or trust entry changed.

## Evidence and reproduction

| Layer | Observed result | What it establishes |
|---|---|---|
| Rust unit tests | 15 pass in debug, optimized and ASan builds | Loan endpoints, aliasing restrictions, shape, native allocation and cleanup behavior of these examples |
| Miri | The same 15 tests pass | No detected undefined behavior in these interpreted executions, including the vendored permission kernel |
| Process scenarios | 17 expected outcomes in each of debug, optimized and ASan builds | Catch versus abort, argument/call/result boundaries, cleanup ordering, deferred and forget counterexamples |
| rustc acceptance/rejection probes | 6 compile and run; 6 reject with the expected diagnostic codes | Real Rust borrow/typing boundaries, including the named-binding and final-placeholder cases |
| Z3 queries | 17 expected outcomes: 6 UNSAT, 11 SAT | Four positive preservation/frame obligations and two deliberate inconsistent/circular controls; countermodels for the other obligations |
| Locus inputs | 24 annotated cases plus support interface | Proposed expectations and remaining obligations only; zero compiler acceptance results |

The successful recorded command is `python3 -u run.py --asan`.
The successful Miri invocation used `cargo +nightly miri test --bin mutable-loans`
with writable cargo-target and XDG-cache directories. The runner includes a local
toolchain fallback specific to this workspace; `--rustc /path/to/rustc` overrides it.

```sh
python3 -m pip install --target .deps -r requirements.txt
python3 run.py --asan
# Optional; install once if unavailable:
rustup toolchain install nightly --profile minimal --component miri --component rust-src
sh run_miri.sh
```

ASan uses Rust's experimental `-Zsanitizer=address` through `RUSTC_BOOTSTRAP=1` in
this throwaway harness. The sysroot is not rebuilt with ASan. The Miri run used
its default settings and ran the 15 unit tests, not the aborting subprocess suite.
Intentional process aborts disable core dumps in the Python runner.

Versions: rustc 1.100.0 (`0ef1bc386`, 2026-11-09); nightly rustc 1.102.0
(`739adecd4`, 2026-11-24); z3-solver 4.15.4.0. Machine-readable outcomes and full
logs are in `results/`; solver inputs are in `results/vcs/`.

## Main findings

### 1. The useful native Vec case works under the stated contract

`vc01_prefix_arbitrary_element` proves the initialized-prefix preservation
condition for arbitrary capacity, length, valid index and final element value,
using an array model with separate initialization and contents. It is not bounded
enumeration. The relation updates an initialized slot to another initialized
value and frames other slots. The Rust permission-kernel version lends `&mut T`
while retaining initialized ownership of that slot.

The boundary matters: lending `&mut Option<T>` also permits clearing the slot,
which has a counterexample (`vc04`). Keeping the other dependencies, especially
length, fixed matters too (`vc17`). The existing invariant-assignment restriction
must not disappear as a side effect of allowing this loan.

### 2. Named acquisition is simpler, with a measurable ergonomic constraint

The main candidate is:

```rust
let before = snap(self.storage);
let shape = self.valid;
// Compute arguments and their evidence before lending the dependency.
let r = &mut self.storage;
let (x, loan) = r.borrow_mut_checked(i, initialized);
// Discharge preservation using before, shape, loan, final!(r), final!(x).
```

Only one runtime use of `r` is permitted; proof mentions of `final!(r)` do not
count as runtime uses. The association must remain tied to the original loan
through reborrows, movement or swaps of the returned reference.

An explicit `&mut` binding activates immediately. `r.method(self.storage.len())`
therefore rejects in Rust, whereas the direct method form may use two-phase
borrowing. `r.method(r.len())` compiles in Rust but violates the candidate's strict
single-runtime-use rule. Precomputing arguments before binding `r` works. The
named form removes the place-form ambiguity; it is more than a cost-free extra
line in every source context.

### 3. Freeze proof evidence, and check every enclosing dependency

Rust accepts a sibling proof-marker read while another field is lent. Therefore
Rust alone will not enforce suspension of an affected Locus proof field. The
logical pass must reject that read and use only evidence from before acquisition.
`vc07` produces the real counterexample; `vc08` deliberately assumes the desired
future invariant and shows how circular premises make a broken case look proved.

An enclosing invariant may fail even if the inner invariant survives (`vc11`).
Two disjoint loans can also each satisfy a check against old sibling state yet
jointly break the invariant (`vc12`). Restricting to one acquisition per affected
proof field is a defensible initial scope. It is not a general ban on all
disjoint Rust borrows.

### 4. The unwind policy works, but its boundary is observable

The subprocess tests distinguish:

- Panic evaluating arguments: caught normally, with a valid owner, when the
  guard starts at call entry. This also works with an already-active named `r`
  that has not been used to mutate the owner.
- Panic escaping acquisition: abort, whether or not the callee changed storage.
- Panic after acquisition returns: caught normally; the owner remains valid.
- Panic dropping a returned result: outside the acquisition boundary.
- Callee-local destructors: execute before the call exits and its boundary aborts.
  The suspended owner is not dropped before that abort in the tested lowering.

A guard surrounding the entire source call expression changes an argument panic
from catchable to aborting. A handwritten method shim preserves two-phase
receiver behavior and starts protection after argument evaluation. A naive
free-function rewrite can instead cause E0502. A generic lowering needs an
evaluation-order/MIR solution; this spike supplies examples, not that compiler.

The native Region tests include a caller panic after a successful loan, exactly-once
element drops, an element destructor that panics while remaining elements are
cleaned up, forgotten references, and zero-sized elements. ASan and Miri passed
these tests. This is finite execution evidence, not a proof of the memory core.

### 5. Synchronous construction is not enough for deferred acquisition

Both a returned closure and a returned future can mutate after the guard was
discharged. The future probe yields Pending with an invalid owner and cancellation
does not repair it. Both are runnable counterexamples to relying only on a
synchronous constructor and an abort guard.

The preservation obligation must be established immediately, before further
runtime work. The initial rule may explicitly exclude deferred acquisition.
This does not show that every API returning a future is inherently unsound: a
different API could provide sufficient immediate evidence and a suitable protocol.

### 6. Typed final placeholders need their own lowering treatment

Calling a placeholder on `&*r` after acquisition causes E0502 while `x` is live.
`final!` must not read the suspended parent even for type inference.

A checking skeleton that creates an internal typed token before the call and
uses that token in later proof placeholders compiles. This proves a small encoding
possibility only: the logical pass still owns loan identity, snapshot-family
normalization, proof validity and the core-term prohibition on future data.
User code must not gain a general-purpose future-value constructor from this token.

### 7. The guard is not universally zero-cost on the normal path

For an unknown Rust callee that may unwind, optimized x86-64 assembly gives the
unguarded wrapper a tail jump. The guarded wrapper saves/restores a register,
uses call/return and has unwind metadata. No normal-path Boolean guard test
remains in that example, but the generated code is not identical.

`rust/cost.rs` reproduces this; the assembly is archived in `results/cost.s`.
This is a counterexample to a universal zero-cost claim, not a performance
benchmark. Inlining or compiler-known non-unwind behavior can change the result.
No new trusted no-unwind assumption is needed for the proposed initial rule.

### 8. What the rule still does not solve

- SortedVec: an unrestricted element write can break sortedness. Neighbor bounds
  on the *final* element suffice mathematically (`vc06`), but obtaining them needs
  another API, a restricted mutation interface, or the proposed later pledge work.
- Wrapped Vec: its independently stored ghost sequence is not automatically
  updated by proving a property of native memory (`vc16`). I03 remains open.
- Native declaration bounds: `T: Snapshot` remains in the Locus candidate to
  isolate I01; I10's conditional proof-storage encoding remains separate.
- Final values as data: admitting future-dependent writes can create an
  inconsistent fixed point (`vc15`). Keep the core-term pending-final restriction.
- Loans retained through output parameters: one runtime use of `r` does not imply
  the loan ends at call return. The Rust test demonstrates this signature shape.
  The initial compiler must track it or explicitly reject it.

## Remaining gates before adopting or implementing the rule

1. Define how the immediately following proof region supplies the acquisition's
   restoration certificate, how it closes, and which runtime operation is first.
   `kept` in the candidate is evidence for an obligation, not an assignment into
   a suspended field. No Locus elaborator currently implements this association.
2. Define the named binding and single-runtime-use check on elaborated code,
   including macros, moves, reborrows, output-parameter escape and dead branches.
   Retain all affected enclosing-invariant and sibling-dependency restrictions.
3. Elaborate `final!` without reading the borrowed place; tie it to loan identity
   and reject every pending-data escape, including inferred and higher-order ones.
4. Specify guard scope, argument evaluation and cleanup ordering and implement
   them faithfully in the chosen Rust lowering. No postcondition implies no panic.
5. Prove the Seq lemmas and replay the corresponding generated obligations in the
   real kernel. SMT results and handwritten Rust lowering are not substitutes.

**Recommendation:** keep the named-binding candidate, frozen evidence,
immediate proof discharge, and abort-on-acquisition-unwind as the next design to
specify. The spike found concrete restrictions and implementation work, but no
counterexample to the initialized-prefix case under those restrictions. This is
not a soundness theorem or an owner decision. Keep I01 current until discussed.

## Files and provenance

- `locus/native_get_mut.open.lc`: worked native Vec and enclosing Nonempty cases.
- `locus/support.lc`: assumed Region interface and unimplemented pure lemma obligations.
- Other `locus/*.open.lc`: annotated boundary fragments M03-M24; not standalone
  complete programs, and not decided litmus.
- `rust/probes.rs`: runnable observations plus native adapter and unit tests.
- `rust/permission_kernel.rs`: unchanged copy of the prior stdlib spike's
  `src/permission.rs`, SHA-256
  `116ba1b304df79155ea75cf3427029a6352201de66568c8e966133bb5ed3d3b3`.
- `rust/compile_pass`, `rust/compile_fail`: actual rustc boundary probes.
- `logic/check_vcs.py`: inspectable formulas and expected counterexamples.
- `baseline/`: contracts and collections read at GitHub `d826ec3`; current rule
  prohibitions are retained there. The previous stdlib workload used `c2818cd`.

Development failures are recorded rather than counted as successful tests: the
initial repair-guard rejection expected E0515, but rustc correctly reported E0713
because Drop needs exclusive access; the expectation was corrected. Initial Miri
setup failed on a read-only default cache; setting a writable XDG cache fixed it.
The initial setup log is retained. No permission escalation was needed.
