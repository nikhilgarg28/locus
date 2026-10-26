# Exporting to Rust

Locus has two modes. In build mode, the default, Locus builds the program itself, and checks every crate that depends on a Locus crate, so a crate's whole interface, contracts included, is available to the crates that use it. In export mode, Locus writes the crate's runtime program as Rust source that Rust users build without Locus. This chapter says what an exported crate's interface may contain. [RFC 0001](../design/rfcs/0001-compiler-pipeline.md) describes the two modes, and [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the rules.

What other crates can reach is every item and field that the source declares plain `pub` and that other crates can reach from the crate's root, together with the public fields and methods of the types among them. An item or field declared `pub(locus)` is not part of it, and neither is a `pub` item that no other crate can reach. Inside the crate, every other chapter applies unchanged.

r[export.locus-visibility]
An item or field declared `pub(locus)` is public when its crate is built, and visible only within the crate when the crate is exported, so the rules of this chapter do not apply to it. When a crate is exported, a plain `pub use` of a `pub(locus)` item is rejected.

r[export.plain-interface]
When a crate is exported, nothing that other crates can reach mentions a logical type: no function or method has a proof parameter or a proof in its result, no parameter, result, field, constant, static, or type alias has a type that holds logical values, and no function has a `Model` bound. Because Rust makes every field of an enum's variants public, an enum that other crates can reach has no logical or proof fields.

r[export.invariants-closed]
When a crate is exported, no field that a proof field reads is reachable from other crates, and other crates cannot construct a struct that has proof fields.

r[export.macros-direct]
When a crate is exported, every invocation of one of Locus's macros is written directly in the crate's own code, by its name or an imported alias, and not inside another macro's input or output.
