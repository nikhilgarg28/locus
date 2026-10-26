# Models

A model maps a runtime type to the logical type that describes it, such as a `u32` to the natural number it holds. A type declares its model with `impl Model for T`, and Locus's library declares the models of the standard library's types. [RFC 0002](../design/rfcs/0002-logical-layer.md) explains the design.

The *view* of a runtime value, of type `Opaque<T>`, is the part of it that cannot change while it is shared. A runtime value enters logical code as its view, whose visible fields logical code can read. A model is a logic function of the view. A value whose type `T` has no model stays its view: it can be compared with `===` and passed on, and its visible fields can be read.

r[models.observation]
In logical code, a runtime value used as a logical value is its type's model. The model of an unsigned integer is a `Nat`, and that of a signed integer an `Int`, so arithmetic on them in logical code never overflows.

r[models.integer-range]
The model of a machine integer lies within the range of its type.

r[models.one-per-type]
A type has at most one model.

r[models.bounds-on-functions]
A `Model` bound appears only on a function's type parameters, never on an `impl` or a trait.

r[models.declared-by-owner]
A type's model is declared by the crate that defines the type, or, for the standard library's types, by Locus's library.

r[models.from-view]
Inside a type's model function, its parameter is the value's view, and is never taken to the model being defined.

r[models.frozen-data]
A model reads only data that cannot change while the value is shared: not the contents of an `UnsafeCell`, and not the target of a raw pointer.
