# Spike: translation validation

- **Kind:** design
- **Status:** open

RFC 0001 (decision 29 in [#10](https://github.com/nikhilgarg28/locus/pull/10)) makes translation validation compare a precise description of each item, as well as every typed body:

- signature, generics, and bounds;
- ABI;
- exported symbol and linkage (`#[no_mangle]`, `#[export_name]`, `#[link_section]`, `#[used]`);
- effective visibility;
- representation and layout attributes;
- target features;
- which impls exist;
- the values of constants and statics that the logic reads.

Only the transformations that export mode states are exempt. Nothing has tested this yet. The Cargo-graph spike compared only sets of items.

**Question:** can the driver compute this description and the typed-body comparison for both programs, from rustc's typeck results and metadata, cheaply enough? Does it catch each kind of difference?

**What the spike should try**, each as a deliberate difference between the checking and the runtime program, which validation must reject:
- a changed body;
- a changed callee;
- an attribute that only one program has, such as `#[cfg_attr(not(__locus_check), export_name = "...")]` emitted by a procedural macro;
- a layout change, such as `#[repr]` under a `cfg`;
- a changed symbol;
- a changed visibility;
- an impl that only one program has.

Measure the cost against the Cargo-graph spike's builds.

**Order:** this comes before the semantic slice ([#6](runtime-semantics.md)).

## Log

### 2026-10-27, opened on GitHub as #20

Moved into the repository from [GitHub issue #20](https://github.com/nikhilgarg28/locus/issues/20) on 2026-10-27. The text above is the issue's as it was then.
