# Checked hashing after the designated-leaf stage

- **Kind:** design
- **Status:** later
- **Blocks:** heterogeneous verified lookup, standard container adapters, and user-proved hashers

## Scope

[RFC 0007](../design/rfcs/0007-provided-trait-implementations.md#adopted-i04-traces-and-the-standard-hash-bridge)
records the owner-adopted I04 architecture. This issue retains the later work;
it does not reopen the trace/sink/adapter decision or claim an implementation.

- **Borrowed lookup (I08).** For the actual `Borrow<Q>` implementation, connect
  the returned reference to the specified projection, prove agreement of the
  selected equalities, and prove equal key/query traces on the table's actual
  routes. `String`/`str` is the first case. A fieldwise-hashed newtype does not
  certify an arbitrary borrow implementation; automatic derivation must supply
  all three relationships.
- **Batch hashing.** Add `slice_trace` or an equivalent explicit batch contract.
  The default `Hash::hash_slice` concatenates individual traces, but standard
  numeric overrides write a byte slice. Account for call boundaries, target
  widths/encoding, and each selected override. Do not assume a length prefix
  followed by individual elements describes every standard slice hash.
- **Containers and tuples.** Supply adapters for `[T]`, `Vec<T>`, `Option<T>`,
  `Box<T>`, tuples and the other chosen types only under the laws of the methods
  their actual standard implementation calls. Tuples already have standard
  `Hash` implementations; no competing generated implementation is allowed.
  `Vec<T>`/`[T]` must exercise borrowed lookup after batch hashing is covered.
  Container snapshot support is a separate prerequisite, not supplied by an
  adapter name.
- **User-proved hashers.** Expose checked construction, primitive writes,
  library-operation interpretation, finish and state-equivalence laws. A
  versioned fact about an inherited `write_str` default may help, but inspect
  actual resolution/forwarding rather than assume all implementations use it.
- **Recovery and closures.** No certified sink reuse after unwind in the first
  stage. [The closures issue](closures-async-packed-macros.md) retains the rule
  for future callback and catch_unwind contracts.

## First-stage implementation gates

The architecture is adopted, but the first implementation still needs a
concrete inventory of designated hasher/builder pairs and leaf adapters,
source API and derive spellings, validity premises, per-adapter trust reporting,
export/private-interface checks, and kernel-checked library proofs. The nine
[Rust probes](../spikes/hash-traces/README.md) validate selected runtime facts,
not any of those unimplemented Locus checks. Broader provider questions remain
in [provided-trait-implementations](provided-trait-implementations.md).

## Log

### 2026-11-25, owner adopts I04; PR #26

Created while recording the owner's request to adopt the restricted hashing
architecture and update its RFC and PR. The deferred capabilities above were
explicitly retained so that implementing the first stage does not silently
certify standard batching, arbitrary Borrow bodies, or arbitrary hashers.
