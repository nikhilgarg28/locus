# Checked Locus libraries

Library declarations use the ordinary Locus grammar and proof checker. They add no axioms. Generic declarations are checked for each concrete instantiation; this compiler does not claim that an unused generic body has been checked for every type.

Include files explicitly with the repeatable `--library` option:

```sh
locus check app.lc --library library/logical.lc
locus check app.lc --library library/logical.lc --library library/finite_map.lc
```

The same option works for `run`, `rust`, and `build`. Included declarations share the entry file's namespace; duplicate declarations are errors. Diagnostics and proof locations point to the original input file and line. This is an explicit compilation-unit facility, not a module or package system.

- `logical.lc`: recursive library `Peano`, `Maybe<T>`, and `Seq<T>`, with checked length, lookup, append, map, nonnegative length, the append-length induction theorem, and Peano/native-Int correspondence.
- `finite_map.lc`: generic entries and maps whose `UniqueKeys` proof field certifies their representation. `map_prepend` requires separate evidence that its key is absent. Equality here is representation equality; extensional map equality is a separate proposition.
- `relations.lc`: index-witness membership, recursive reachability, and an induction proof that the example transition relation preserves ordering.
- `buffer_model.lc`: generic finite observations from readable physical storage to `Seq<M>`, using each element's checked Model implementation and proving the model length.
- `runtime_list.lc`: physical recursive boxed lists observed as logical `Seq<Peano>` values; the observation retains no runtime borrow.
- `integer.lc`: a signed integer representation built from library Peano, with checked correspondence to native Int on a stated fragment. Native Int remains the arithmetic backend.

The other libraries depend on `logical.lc` so supply both files. No imports are implicit. A logical representation is erased; a proof attached to it is still checked. Ordinary effects in the calling program remain ordinary effects.

Model definitions can be opened in abstract proofs with
`unfold!(model!(value), evidence)` and closed with
`fold!(model!(value), evidence)`. The observation selects its canonical,
checked model definition. Its source observation must occur in the relevant
claim, and the selector cannot run ordinary effects. Primitive model laws
are separate from source definitions.

A `logic fn` template may observe a borrowed physical `T`; its instantiated
body must still be total, have no runtime effects, and return a Logical
value. An operation such as `element as M` observes the canonical model and then requires a permitted logical conversion to M. Logical aggregates require `Logical` bounds for
type parameters stored in their fields. Unused generic observer bodies do
not provide a theorem that every possible element type has such a model.

The buffer model length theorem applies to arbitrary readable input. A
caller can keep `before = values as Seq<Int>`, establish its length, push a
runtime value, and establish `seq_length(after) == seq_length(before) + 1`.
The before snapshot remains a description of the old contents. The
`library_buffer_model.lc` corpus example also checks the complete contents
of a concrete push against `seq_append` and executes the result in both
interpreters and generated Rust. Equality on closed, proof-free logical
data may be checked by composing kernel evaluation equations; this does
not assume equality for arbitrary symbolic inputs.

The built-in `Nat` is the efficient checked nonnegative-Int representation used for unsigned machine models. `Peano` is the separate inductive teaching example; its conversion lemmas illustrate user-defined logical data.
