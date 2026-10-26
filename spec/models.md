# Models

A model maps a runtime type to the logical type that describes it, such as a `u32` to the natural number it holds. Every runtime type has exactly one model: declared with `impl Model for T`, derived with `#[derive(Model)]`, declared by Locus's library for the standard library's types, or, by default, an opaque identity. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

Logical code reaches runtime data only through places: locals, visible fields, and the targets of references and `Box`. It reads a value's visible fields as places whatever the value's model is. Logical code cannot read inside an `UnsafeCell`, whose contents are in a private field, or through a raw pointer, so no model reads data that changes while the value is shared.

r[models.observation]
In logical code, a runtime value used as a logical value is its type's model. The model of an unsigned integer is a `Nat`, and that of a signed integer an `Int`, so arithmetic on them in logical code never overflows.

r[models.integer-range]
The model of a machine integer lies within the range of its type.

r[models.one-per-type]
A type has at most one model.

r[models.default-identity]
A type with no declared, derived, or library model has the model `Opaque<T>`: an opaque identity of a value at a version. Two values have equal identities when they are the same value, as two versions that no write separates are, and nothing else is known about them. In particular, two values with equal fields need not have equal identities.

r[models.derived]
`#[derive(Model)]` on a struct or enum that its crate defines, with no type parameters, gives it a model with the same fields and variants, each field taken to its own model, and its proof fields left out. It is rejected unless every field's type has a model that is declared, derived, or given by Locus's library. Two values have equal derived models exactly when they are the same variant and their fields' models are equal.

r[models.bounds-on-functions]
A `Model` bound appears only on a function's type parameters, never on an `impl` or a trait.

r[models.declared-by-owner]
A type's model is declared or derived by the crate that defines the type, or, for the standard library's types, declared by Locus's library.

r[models.own-definition]
A hand-written model's definition never depends on the model itself: inside it, and inside every logic function it calls, a value of the type is read through its fields and never taken to the model being defined.

r[models.frozen-data]
A model reads only data that cannot change while the value is shared: not the contents of an `UnsafeCell`, and not the target of a raw pointer.
