# Exporting to Rust

Locus has two modes. In build mode, the default, Locus builds the program itself, and checks every crate that depends on a Locus crate, so a crate's whole interface, contracts included, is available to the crates that use it. In export mode, Locus writes the crate's runtime program as Rust source that Rust users build without Locus. This chapter says what an exported crate's interface may contain. [RFC 0001](../design/rfcs/0001-compiler-pipeline.md) describes the two modes, and [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the rules.

What other crates can reach is every item and field that the source declares plain `pub` and that other crates can reach from the crate's root, together with the public fields and methods of the types among them. An item or field declared `pub(locus)` is not part of it, and neither is a `pub` item that no other crate can reach. Inside the crate, every other chapter applies unchanged.

r[export.locus-visibility]
An item or field declared `pub(locus)` is public when its crate is built, and visible only within the crate when the crate is exported, so the rules of this chapter do not apply to it. When a crate is exported, a plain `pub use` of a `pub(locus)` item is rejected.

r[export.plain-interface]
When a crate is exported, nothing that other crates can reach exposes logical data ([`logic.boundary.logical-data`](logical-layer.md#the-boundary-with-unverified-code)): no function or method has a proof parameter or a proof in its result, no parameter, result, constant, static, or type alias has a type that exposes logical data, no field that other crates can reach exposes logical data, and no function has a `Model` bound. Because Rust makes every field of an enum's variants public, an enum that other crates can reach has no field that exposes logical data. A sealed type may appear in each of these places: as a parameter or a result, and as the type of a field that other crates can reach, in a struct or in a variant of an enum.

r[export.invariants-closed]
When a crate is exported, no field that a proof field reads is reachable from other crates, and other crates cannot construct a struct that has a field which exposes logical data: that field is not reachable to them (`export.plain-interface`). So a type with proof fields that other crates can reach stays closed against them: they hold its values and call its functions, and can neither build one nor change what its proofs read. A struct or an enum that holds logical data only inside fields of sealed types proves nothing of its own, so other crates may build one, and assign those fields, with values of those types.

r[export.macros-direct]
When a crate is exported, every invocation of one of Locus's macros, and every call that passes an argument for a parameter of logical type, is written directly in the crate's own code: not in any macro's output, and not in the input of a function-like macro. An attribute macro or a derive may take an item that contains them. A Locus macro may be invoked by its name or an imported alias.

r[export.function-values]
When a crate is exported, a function with ghost parameters or a ghost result may be used as a value wherever it may be when the crate is built, which is within verified code ([`logic.boundary.calls`](logical-layer.md#the-boundary-with-unverified-code)).

r[export.traits]
When a crate is exported, no item or implementation that other crates can reach has a bound on a Locus trait ([`traits.members`](traits.md#what-a-trait-holds)), in a parameter list or in a `where` clause, on a type parameter or on an associated type. An implementation that a law implementation generates ([`traits.laws.provided`](traits.md#law-traits)) is held to this too. Code that uses an exported crate is built without Locus, so nothing checks an implementation of a Locus trait that it writes, and no evidence could be given for it. Inside the crate, and on what is declared `pub(locus)`, such a bound stands as it does when the crate is built.
