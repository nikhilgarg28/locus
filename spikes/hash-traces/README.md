# Hash traces and direct standard delegation

These nine Rust probes informed the owner-adopted I04 design in
[RFC 0007](../../design/rfcs/0007-provided-trait-implementations.md#adopted-i04-traces-and-the-standard-hash-bridge).
They test concrete protocol and interop claims, not a complete HashSink library,
Locus typing/lowering, proof erasure, or kernel-checked proofs. Nothing outside
this spike may depend on its code.

## Results

All nine tests pass on `rustc 1.102.0-nightly (739adecd4 2026-11-24)`.
[results.txt](results.txt) contains the recorded run. The code uses nightly's
`hasher_prefixfree_extras` solely to observe and distinguish the underlying
`write_str` and `write_length_prefix` operations. The adopted public sink does
not need to implement `Hasher` or expose those unstable methods.

| Probe | Observed result | Design implication |
| --- | --- | --- |
| Stable-only forwarding sink | Standard `str::hash` changes from `Str("abc")` to `Bytes([97,98,99]), U8(255)` when the sink inherits `write_str`'s default | The old fallback does not preserve an underlying override |
| Borrowed lookup through that sink | Owned lookup finds the key; borrowed `str` lookup misses | Content equality does not supply the hash-agreement part of `Borrow` |
| Byte-write boundaries | Two writes and one concatenated write yield different digests | Preserve call boundaries in primitive traces |
| `Hasher::finish` visibility | A sink implementing `Hasher` exposes `finish` through the trait | A structurally write-only sink must not implement `Hasher` |
| Mutation before panic | The hasher changes before the post-call event append runs | An old log cannot certify hasher state after recovery |
| `BuildHasher::hash_one` override | The override differs from build/hash/finish | Cover the actual helper if the table uses it |
| `Hash::hash_slice` override | Repeated slice hashing differs despite a lawful individual hash | An individual hash certificate does not certify an unchecked batch override |
| Direct delegation with shared `StdText` | A sink without `Hasher` preserves the override, records equal text events, and succeeds at borrowed lookup | The adopted direct-delegation route fixes this counterexample |
| Standard `u8` slice | `Length(2), Bytes([1,2])` differs from `Length(2), U8(1), U8(2)` | Standard batching needs a separate `slice_trace`/adapter contract |

Some adversarial implementations deliberately violate Rust's documented library
laws. In particular, the failing-lookup key's `Borrow` implementation violates
hash agreement, and the bad `hash_slice` depends on external state. The tests
show which semantic obligations a Locus certificate must exclude; they do not
claim Rust is wrong or that Locus would accept these implementations.

The recorders use runtime `Vec`/`String` logs so the tests can inspect calls.
Those are experiment instrumentation, not the runtime representation proposed
for Locus's ghost log. The direct-delegation source records a common enum
variant for `String` and `str`; it never assumes equality of distinct variants.
The hasher is deliberately simple and is not a production hashing algorithm.

## Reproduction

Install an appropriate nightly toolchain, then run:

```sh
python3 spikes/hash-traces/run.py
```

Or pass an existing compiler explicitly:

```sh
python3 spikes/hash-traces/run.py --rustc /path/to/nightly/rustc
```

The runner prints its compiler version, compiles the two standalone test files
into a temporary directory, runs seven and two tests respectively, and removes
the binaries. It downloads nothing and modifies no toolchain. The expected
panic in `normal_return_trace_update_needs_an_unwind_policy` is caught by the
test; its diagnostic is not a test failure.

## Limits and remaining work

- No Locus source was compiled or verified. The private interface, proof result
  contracts, generic trait evidence, and export restrictions still need real
  compiler tests.
- These finite examples are not a theorem about all hashers, strings, builders,
  or standard-library versions. Designated adapter registrations must identify
  the supported implementations, versions, targets, and relevant helpers.
- The direct-delegation probe uses one transparent text newtype. It does not
  derive a `Borrow` law for arbitrary user implementations or custom equality.
- No universal zero-cost or no-unwind claim follows. Erasure, panic recovery,
  and code-generation costs are separate checks.
- The first-stage library interface, concrete designated hasher/builder
  inventory, and later batch/container/user-hasher work remain recorded in
  [checked-hashing-followups](../../issues/checked-hashing-followups.md).
