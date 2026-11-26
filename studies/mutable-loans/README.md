# Mutable-loan workload

`src/native_get_mut.lc` contains the worked native Vec acquisition and enclosing
Nonempty case. `src/support.lc` names the required permission interface and pure
sequence lemmas. `src/lib.lc` and Cargo.toml give them a future library-crate shape.

The owner adopted the named-acquisition rule in RFC 0002, decision 125. The
implementation and lemma bodies remain OPEN: declarations are not new trusted
axioms, proof holes are not checked, and there is no Locus compiler to build this
crate today. The `T: Snapshot` declaration bound remains isolated as the existing
conditional proof-storage issue rather than a new container API decision.

Partial/negative fragments remain [under the spike](../../spikes/mutable-loans/locus/README.md).
Small decided examples belong to `litmus/contracts/`. The independent executable
Rust lowering and [its report](../../spikes/mutable-loans/README.md) are experiment
inputs and evidence; this crate does not depend on them.
