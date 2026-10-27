# Traits and operators on logical types

- **Kind:** decision
- **Status:** open
- **Blocks:** Generic specifications such as "sorted by `T: Ord`", the operators that the library's types offer, and models of generic types the program defines

## Question

How logical code is generic over operations, and how operators on logical types are resolved. RFC 0002 (`design/rfcs/0002-logical-layer.md`, proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10)) fixes the arithmetic of `Int` and `Nat`. It also lets logic functions be generic over logical types or over `T: Model`. It does not yet say:

- **Logical traits:** whether there are any, like Lean's type classes, for arithmetic, order, and equality.
- **Operators on user-defined logical types:** how `+`, `<`, `==`, and `[]` are resolved, together with the laws that tie a comparison to `===` (Lean's `LawfulBEq`) and decidable equality. For now `==` and the orderings are built in only on `Int`, `Nat`, and `Bool`.
- **Rules:** `logic.ghost.no-trait-impls`, `models.bounds-on-functions`
- **Runtime traits and models:** how they relate. For example, does a type's `Ord` implementation agree with an order on its model, or `PartialEq` with equality of models?
- **Traits with logical values or contracts.** For now no trait method's signature holds logical values, and no logical type appears in an `impl` apart from `impl Model`. When such traits arrive, an implementation written in plain Rust cannot be checked, so in export mode they will not be reachable from other crates ([#7](distribution-and-export.md)).
- **Checking generic code once** for every instantiation with logical types, instead of again at each one.
- **Generic `#[derive(Model)]`.** RFC 0002 derives a model only for types without type parameters, because a derived generic model would need a `Model` bound on its `impl`, and `Model` bounds may appear only on functions.
- **The `?` operator** on values that hold proofs, which goes through the `Try` trait.

## What it blocks

Generic specifications such as "sorted by `T: Ord`", the operators that the library's types offer, and models of generic types the program defines.

## Options

To be laid out in an RFC.

## Log

### 2026-10-25, opened on GitHub as #12

Moved into the repository from [GitHub issue #12](https://github.com/nikhilgarg28/locus/issues/12) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.
