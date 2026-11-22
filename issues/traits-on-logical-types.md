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

### 2026-11-22, conversation with the owner after the spikes on lowering

A logical type can implement a trait only if rustc can tell one logical type from another, and [RFC 0001](../design/rfcs/0001-compiler-pipeline.md) lowers them all to one marker. Four spikes asked how to change that: [logical-markers](../spikes/logical-markers/README.md), [marker-lowering](../spikes/marker-lowering/README.md), [skeleton-typing](../spikes/skeleton-typing/README.md), and its [second round](../spikes/skeleton-typing-2/README.md). Two lowerings were left standing. In the first, each logical type is a zero-sized Rust type of its own, and lowering also writes each piece of logical code as a Rust expression over stubs, in a closure that is never called, so that rustc types logical code. In the second, there is one marker, and traits are split into those for runtime types and those for logical types. Shown both, with what each spike measured, the owner chose the first:

> Yes I'm aligned with option 1 now and conversion policy is okay. Maybe Nat -> Int conversion shouldn't be implicit but others are okay? Similarly, should Bool -> Prop conversion be explicit or implicit? Starting with explicit is better - can always revise it later unless of course ergonomics are too poor.

The conversion policy is the one the second round tested: what a literal's type is, when a `Nat` is used as an `Int`, when a `Bool` is used as a proposition, and what a runtime value is when it is given to a logic function.

On the names that lowering writes, which a programmer never writes and rustc's messages show, such as `Proof` for the one marker of every proof:

> Yes, using symbols to make error messages good is the right principle, please adopt it.

On `Bool` and `Prop`, shown Lean's arrangement, in which a comparison is a proposition and a `Bool` is only the model of a runtime `bool`:

> Re: booo/prop, Lean's model looks good so let's adopt that I guess?

and then, told that Lean also uses a `Bool` as a proposition with nothing written:

> fwiw, I'm also happy for Bool to convert to proposition implicitly if that ends up the being default and/or if lean does it too

Asked whether a call may ever be read as an expression, so that an operator or a standard method on a type with a law trait would give its fact ([pure-functions](pure-functions.md)):

> Fact is always proof in the result or small/finite list of facts language infers from some constructs (which we have already defined). Don't want to go back on that. And don't want to introduce pure functions either right now.

So in runtime code `a < b` on a type of the program's own still tells Locus nothing, and the fact comes from the law trait's own method, called by name.

None of this is in an RFC yet. Still for the owner, from the second round's measurements: whether a `Nat` beside an `Int` in one operator needs `as Int`, which is 7 of the 17 places where its four specifications write something that RFC 0002 lets a program omit; whether a literal given to a `Nat` parameter is a `Nat`, as RFC 0002 says now, which is 7 more; and how a contract ties a `bool` result to a proposition, for which the round tried `r <==> P` and `r === decide(P)`.

One question the owner raised is not decided. Logical code reads a runtime value as its model, and RFC 0002 records "snapshots by default, and models only through `model(x)`" as rejected at the owner's direction. Now that `Model` would be an ordinary trait, the owner asked:

> now that Model is just a regular trait, why does the language care/know about it? What if everything in signatures and all capture in logic blocks was based on snapshots by default and using model required separate explicit mapping from snapshot to model?

It would also answer what the library's contracts are stated on, the entry above. It rests on every type having a snapshot that rustc can name, which the second round shows holds for a value, `snap(x)`, and not for every type that a program can write.

### 2026-11-22, the owner's answers on conversions, and on snapshots and models

The entry above left three questions from the second round of the skeleton spike. Asked whether a `Nat` beside an `Int` in one operator may go without `as Int`, whether a literal stays a `Nat` where a `Nat` is expected, as RFC 0002 has it, and whether a contract ties a `bool` result to a proposition with a new connective, `<==>`, or with a library function, `decide(P)`, the owner said:

> No, Yes, and go with library function/macro vs one more symbol

So a `Nat` is written `as Int` wherever an `Int` is wanted, in an operator too; RFC 0002's rule for literals stands; and there is no `<==>`.

The same day the owner worked out a design for snapshots, models, and capture with a second reviewer, asked for a recommendation on the whole of it, and had the reviewer answer that. On integers the owner said:

> I'm not married to finite logical integers like Nat32 - totally happy to fall to single Nat/Int system, at least in common cases unless explicitly chosen.

What came of the exchange, recorded here until the RFCs say it:

- A runtime value used as a logical value is the model of its snapshot. The language supplies snapshots, a model may be declared, the default model is the snapshot, and `snap(x)` gives the snapshot. A snapshot never depends on a model.
- The snapshot of a machine integer is a `Nat` or an `Int`. A captured value is valid for its type, so a captured `u8` is at most 255; nothing of the kind is assumed of a value that is only quantified over.
- References and `Box` stay transparent in snapshots. Two equal snapshots say nothing about addresses.
- A logic function takes logical types only. A runtime value given to one is its capture, and a method call on a runtime value resolves on the captured value's type.
- A type parameter is read in logical code only under a bound the program writes, `T: Model`. A type that Locus cannot see has no model until a field of it is marked opaque or it is wrapped. This is a deliberate step back from "every runtime type has a snapshot".
- Locus's collections keep their contents in a logical field, and give no mutable access to the standard collection inside. Their contracts speak of the elements' snapshots.
- Permissions over memory, from which those collections would later be proved, are an RFC of their own ([permissioned-pointers](permissioned-pointers.md)).

The owner then asked for all of it to be written into the RFCs once the third round of the spike reports:

> Good - assuming spike succeeds, I think we should begin to now update PR with all the decisions that have been made across all RFCs but not yet persisted in github.

None of it is in an RFC yet. [capture-family](../spikes/capture-family/README.md) shows the encoding in plain Rust, and a third round of the skeleton spike is testing it in the lowering.
