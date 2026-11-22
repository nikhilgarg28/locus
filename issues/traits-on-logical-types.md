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

### 2026-11-16, proposal made at the owner's request

The owner asked for an audit of the decisions that are still open, with a proposal for each. This is the proposal for this issue. Nothing in it is decided.

**The question that decisions 80 to 82 of RFC 0002 make urgent:** a trait method has the signature its trait gives it, so it cannot return a proof. On Locus's own types, `v[i]`, `*rc`, `for x in &v`, `a == b`, and `v.clone()` run and tell Locus nothing.

Options:
1. **Leave it.** A named method gives the fact: `v.get(i, p)`, `v.view()`.
2. **Views.** Each of Locus's types has one method that returns it as a type the language understands, with a proof that the two are the same: a `Vec` as a slice, a `String` as a `str`. After one binding, `len` and indexing are the language's.
3. **Transparent functions.** A function may declare that its result is a stated logical expression of its arguments. Locus verifies the body against it once, and from then on a call is an expression it understands, as it understands `a + b`. The declaration is on the method's implementation and does not change its Rust signature, so it works for operators and trait methods, and `if h: i < v.len()` works with no binding at all.

Proposal: option 2 now, and option 3 as the next design after proof construction. The strongest argument against option 3 is that it is a second way to learn a fact from a call, beside a proof in the result, and the owner has just turned one such way down. The difference is that nothing is assumed: the fact is the function's own declared meaning, checked against its body. It does not help a wrapper whose body cannot be verified, which is why it waits for the verified collections.

For the rest of this issue: logical traits as type classes, following Lean, and generic code checked once against them, in place of checking at each instantiation.

### 2026-11-17, found while writing examples of values with invariants

A sorted vector over `T: Ord` cannot state its invariant. A call of `cmp` tells Locus nothing ([RFC 0004](../design/rfcs/0004-runtime-code.md)), and no trait method has a contract. RFC 0002's examples of values with invariants hold `u32`s for this reason.

### 2026-11-22, what the library's contracts are stated on

A question about Locus's library is to be decided with this issue's design. `Vec::view` and the three signatures of `Vec` that RFC 0002 proposes speak of models with no `Model` bound, which `models.no-default` does not allow. Offered contracts on snapshots, with `v.len()` read as the snapshot's length, or contracts on models with a `Model` bound on each method, the owner said:

> Re: library contracts - I'm not following but will defer to you.

No decision is recorded from that reply. The first, which was the recommendation, cannot be written as it was put without changing what decided examples rest on. RFC 0002 lists the question, the three ways out, and what each changes in those examples, as item 27 of what is still for the owner, and [standard-library](standard-library.md) has the record. What a contract on a snapshot can say is part of this issue's design, so the owner was advised to settle the two together.
