# Locus workloads

These are future compiler workloads, not decided litmus outcomes or implementations
of the standard library. They retain open proof obligations and explicit primitive
interfaces. Nothing here imports executable code from `spikes/`; the Rust mirrors
are independent experiments, not implementations used by these Locus modules.

- [Standard library study](stdlib/README.md): logical Seq/Map, permissions,
  native and wrapped collections, ownership/interior-state interfaces and clients.
- [Mutable-loan study](mutable-loans/README.md): the worked invariant-preserving
  native Vec acquisition and its support interface.

Each directory has a Cargo manifest with a `.lc` library root, following the
intended crate shape. The compiler and required library support do not exist yet;
ordinary Cargo/rustc cannot compile these sources. Complete the recorded OPEN
obligations and replace interface-only declarations with checked implementations
before claiming a runnable verified crate. `check_layout.py` checks only paths
and module wiring; it never reports proof acceptance.
