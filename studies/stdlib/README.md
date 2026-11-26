# Standard library workload

Thirteen Locus modules from the standard-library spike now have one crate root,
`src/lib.lc`, and names matching their `crate::...` imports. `src/bin/interop.lc`
calls the native and wrapped stack examples. The manifest and module structure
are ready for a future Locus build; the algorithms and proofs are not certified.

`import-manifest.json` maps the original filenames and records their import hashes.
Only the two stack functions gained public visibility for the future executable.
The original report is [in the Rust spike](../../spikes/stdlib/REPORT.md).

The modules deliberately retain OPEN points: sequence lemmas and representation,
map extensionality, primitive permission contracts, native proof-storage bounds,
wrapped-state correspondence/restoration, recursive B-tree helper contracts,
Hash/Hasher bridges and shared/interior-state protocols. Interface declarations
ending in `;` are study requirements, not implementations or trusted axioms.
No Rust mirror test establishes that a Locus proof hole can be filled.

The native Vec file predates the adopted named-acquisition rule and does not yet
implement get_mut. Its separate worked successor is
[studies/mutable-loans](../mutable-loans/README.md); integrating that method after
its support lemmas are proved is tracked work, not a duplicated implementation.
