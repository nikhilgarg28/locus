+++
id = "language-types"
title = "Values and types"
group = "Now"
spec_chapter = 1
order = 101
route = "specification/types.html"
description = "Physical data, logical data, products, enums, and dependent evidence fields."
+++

# Values and types

<!-- spec: 1.0:3 informative -->
Locus programs combine runtime data with logical values: propositions describe claims, and evidence establishes them. This chapter introduces the types that hold those values, including tuples and structs whose later fields can state facts about earlier ones.

## Types

<!-- spec: 1.3:1 legality-rule -->
| Type | Runtime form | Notes |
|---|---|---|
| `bool` | `bool` | Physical boolean. |
| `u8` through `u64`, `i8` through `i64` | Same machine type | No `usize`/`isize` surface type yet. |
| `()` and tuples | Same product shape | Optional names in tuple types bind later field types; access stays positional. |
| Ordinary structs/enums | Rust structs/enums | A physical aggregate may hold erased fields; an ordinary enum retains its tag. |
| `Int`, `Bool`, `Prop`, `@P` | `Erased` | Logical integers, booleans, propositions and evidence. |
| `#[derive(Logical)]` aggregates | `Erased` | Every field must be Logical. |
| `logic Fn(x: T) -> U` | `Erased` | Inputs and result are Logical, including dependent evidence results. |
| `&T`, `&mut T` parameters | Rust borrows | Shared references may also be stored or returned with checked lifetimes; mutable loans are call-scoped. |
| `Box<T>` | `Box<T>` | Always physical, even when its payload is Logical. Recursive runtime enums need Box indirection. |
| `[T; n]`, `Vec<T>`, borrowed `[T]` | Corresponding Rust storage | Runtime or Logical elements; literal array lengths. Logical elements erase, physical storage remains. |
| `!` | Diverging expression | A declared never-returning function has no reachable result; its proof representation is erased. |

<!-- spec: 1.3:2 legality-rule -->
Types may occur on parameters, bindings, fields, constants and results. Later fields and results may mention earlier values inside propositions and evidence types. Such dependencies do not select a runtime layout. Tuple field names exist only as binders in later types. Named structs are nominal.

<!-- spec: 1.3:3 legality-rule -->
Generic structs, enums, propositions and functions are checked templates. Every concrete instance is elaborated and kernel checked. An unused generic body is not asserted to be universally verified. `T: Logical` is the supported classification bound; general trait definitions and implementations are not supported; the built-in Model bridge is described in [Models](11-models.md). Generic methods inside inherent impls are not yet supported.

<!-- spec: 1.3:4 legality-rule -->
`Ghost<T>` and `snapshot!` are rejected migration syntax. Observe runtime data using its logical model (`x as Int`, or a checked user-defined Model implementation). A logical value's classification belongs to its type, not a `logic let` binding qualifier.

## Products, enums, and match

<!-- spec: 1.4:1 legality-rule -->
A tuple is built `(a, b)`, a one-field tuple `(n,)`, and a struct `S { f: e, g }` with `g` short for `g: g`, in any field order; every field is given once and the names are those declared. A tuple field is read `t.0`, and nested `pair.1.0`; a struct field by name. A tuple type's field names are binders only: `t.name` is refused. Each field of a product is checked against its type with the earlier fields substituted, so `(next, within_limit::Bounds @ evidence)` is checked with `next` standing for the first field in the second's claim, and a `let (next, still) = step(...)` opens the tuple the same way: `still` has the type `@within_limit(next.failures as Int)`, about the `next` just bound, which is what lets evidence returned by a call be passed on.

<!-- spec: 1.4:2 legality-rule -->
An enum declares its variants with no fields, positional fields, or named fields, and a variant's later field may mention an earlier one in a proposition. A variant is built by path, `E::V(x)`, `E::V { f: x, g }`, and inside an `impl` block of `E` also `Self::V`. `match` on an enum has one arm per variant, in any order, or a `_` arm; the pattern of an arm is `E::V`, `E::V(names)`, `E::V { f, g: other, .. }`, with a name or `_` for each field, and nothing deeper. Each arm knows that the scrutinee is the variant it names, applied to the names it bound. `match` takes apart an enum or evidence: `bool` is branched on with `if`, and an integer is compared.

<!-- spec: 1.4:3 legality-rule -->
The ordinary patterns of a `let` are a name, `_`, and a tuple of those; a runtime struct or variant pattern in a `let` is refused. A name in a pattern may be `mut`.
