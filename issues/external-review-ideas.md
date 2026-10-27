# Design ideas from the external review of RFCs 0001 and 0002

- **Kind:** design
- **Status:** later

An external review of [#10](https://github.com/nikhilgarg28/locus/pull/10) proposed seven ideas that go beyond fixing what is there. The owner agreed with all of them in spirit and asked to pin them for now. Each needs its own design work when its time comes. None blocks RFCs 0001 or 0002.

1. **Proof transport across versions.** When two versions of a value are proved equal, proofs about one could carry over to the other without a visible re-proof. The elaborator would build the transport, and the kernel would check it.
   - Adopting it would flip some stale-proof examples in `litmus/proofs/` and `litmus/contracts/`, so it is an expectation change.
   - It changes nothing that is trusted.
2. **Mutable invariants under contracts.** An operation could break a type's invariant temporarily, if its contract restores the invariant on normal return and on unwind. That allows more useful operations than requiring that nothing in between can panic.
   - It depends on the runtime semantics of unwinding and drops ([#6](runtime-semantics.md)).
   - It relates to the ban on mutably borrowing proof fields.
3. **Generated export adapters.** Explicit, generated wrappers at the export boundary, with signatures you can review: "check the precondition at run time" for a function with a precondition, and "drop the postcondition" for one with a postcondition. Authors would no longer write that boilerplate by hand. Belongs with [#7](distribution-and-export.md).
4. **Proof storage by goal identity.** Match stored proofs to normalized goals and dependency fingerprints, rather than to source locations, and invalidate a certificate when what it depends on changes.
   - This is about robustness and speed, not trust, since the kernel re-checks every stored proof.
   - The identity must include the hypotheses in context. Belongs with [#13](proof-search-and-storage.md).
5. **Compiler organization.** Start most of RFC 0001's ten proposed crates as modules, and split them when a dependency or testing boundary earns it.
   - Some boundaries are structural from day one:
     - the driver, which alone links rustc's internals;
     - the facts format, shared by the driver and everything else;
     - Locus's library, which user programs link;
     - the kernel, kept apart so that nothing untrusted can reach into it;
     - the `locus` command.
   - So the floor is about five crates, not two.
6. **Compatibility fixtures.** Early fixtures for `no_std`, `alloc`, `#[no_implicit_prelude]`, cross-compilation, and host versus target builds. The prelude that Locus injects must respect the crate's actual Rust environment.
7. **Minimal erasure in export mode.** Keep ordinary Rust item names, imports and private items wherever possible, and defer cosmetic cleanup, because each rename or deletion is more trusted transformation. If build mode adopts the one-lowered-text design now proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10), build mode has no erasure step at all, and this applies to export mode only.

## Log

### 2026-10-26, opened on GitHub as #18

Moved into the repository from [GitHub issue #18](https://github.com/nikhilgarg28/locus/issues/18) on 2026-10-27. The text above is the issue's as it was then.
