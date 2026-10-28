# Snapshots and models

Logical code sees a runtime value in two ways: as what it is, its *snapshot*, and as what it means, its *model*. Every runtime type has a snapshot type, which Locus derives from the type's definition. A type has a model only when it is declared with `impl Model for T`, derived with `#[derive(Model)]`, or declared by Locus's library for the standard library's types. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

Logical code reaches runtime data only through places: locals, fields, and the targets of references and `Box`. Reading a place gives a snapshot. Using a runtime value as a value, as an operand, either side of `===`, a method receiver, or an argument of a logical parameter, takes it to its model. `snap(x)` gives `x`'s snapshot, of type `Snap<T>`.

## Snapshots

r[models.snapshots]
Every runtime type `T` has a snapshot type `Snap<T>`, whose values are exactly the values that `T` can have. For a struct or enum it has the same fields and variants, each of its field type's snapshot type, proof fields included. A machine integer's snapshot is an integer within its type's range, and a `bool`'s a `Bool`. `Box` and references are transparent: `Snap<&T>` is `Snap<T>`. The snapshots of arrays, slices, and `Vec<T>` are `Seq<Snap<T>>`. So that every snapshot is finite, a field is *cut* when its type reaches the type that declares the field again through a reference, following type arguments, and a cut field's snapshot follows no reference: each reference in it holds only its address. The cut belongs to the type that closes the cycle, so a type's snapshot is the same wherever it appears, and a generic type's has the same shape for every type argument. Logical code that reads through a cut field's references as places still observes their targets.

r[models.snapshot-privacy]
A snapshot holds every field of its value, private ones included, but logical code reads a field of a snapshot only where Rust code could read that field.

r[models.frozen-data]
A snapshot holds nothing that can change without a write that Locus sees: not the contents of an `UnsafeCell`, which are opaque, and not the target of a raw pointer, only its address. A model, being a function of the snapshot, reads neither.

## Models

r[models.observation]
In logical code, a runtime value used as a value is its type's model. The model of an unsigned integer is a `Nat`, and that of a signed integer an `Int`, so arithmetic on them in logical code never overflows.

r[models.no-default]
A type has a model only when its model is declared, derived, or given by Locus's library. A value of a type without a model cannot be used as a value in logical code; its snapshot can.

r[models.integer-range]
A machine integer's snapshot, and so its model, lies within the range of its type.

r[models.one-per-type]
A type has at most one model.

r[models.derived]
`#[derive(Model)]` on a struct or enum that its crate defines, with no type parameters, gives it a model with the same fields and variants, each field taken to its own model, and its bare proof fields, whose type is a proof type, left out. A field that holds a proof inside a runtime type, such as `Option<@P>` or `Vec<(usize, @(i < n))>`, keeps its runtime structure, taken to that type's model. It is rejected unless every field's type has a model, and rejected when the type reaches itself through a reference, following type arguments, since the model type would then have no values; a type that reaches itself only through `Box` may derive its model, whose type is then recursive. Two values have equal derived models exactly when they are the same variant and their fields' models are equal.

r[models.bounds-on-functions]
A `Model` bound appears only on a function's type parameters, never on an `impl` or a trait.

r[models.declared-by-owner]
A type's model is declared or derived by the crate that defines the type, or, for the standard library's types, declared by Locus's library.

r[models.own-definition]
A model's definition never depends on the model itself, directly or through other types' models: inside a hand-written model, and inside every logic function it calls, a value of the type is read through its fields and never taken to the model being defined, and the models that derived and hand-written models use form no cycle.
