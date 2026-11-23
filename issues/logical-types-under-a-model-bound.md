# A logical type under a `Model` bound

- **Kind:** decision
- **Status:** open
- **Blocks:** `traits.model-bounds` of RFC 0006, and with it one specification trait for a logical type and a runtime type
- **Rules:** `models.snapshots.seen`, `collections.element-types`, `logic.fn.parameters`, `traits.model-bounds`

## Question

Does a logical type have a snapshot and a model, and so meet a `Model` bound?

Three sources do not agree.

- **`models.snapshots.seen` lists the types that have a snapshot, and no logical type is among them.** It ends: "Any other type has neither a snapshot nor a model". `logic.fn.parameters` says that a type parameter bounded by `Model` "stands for a runtime type".
- **Decided examples need a logical type to meet the bound.** `logical-layer/logical-values-in-vec.accept.lc` pushes a `Nat` into a `Vec` of Locus's library and takes its length, and `logical-layer/own-method-at-proof-holder.accept.lc` pops from a `Vec<Option<@(n > 0)>>`. `collections.element-types` declares those methods for element types that have a model, `T: Model`, and rejects a call at an element type with none.
- **RFC 0002's text gives a logical type a model.** Its list of the models of standard types has "a logical type, itself, so `Vec<Nat>`'s model is a `Seq<Nat>`", and `models.snapshots` gives a struct's snapshot a field for each field, "proof fields included", so a logical field is its own part of a snapshot. In the third round of the skeleton spike the library's `Logical` has `Model<Snap = Self, Logic = Self>` as a supertrait.

By [the specification's own rule](../spec/README.md#when-sources-disagree), neither the rule nor the examples win.

## What it blocks

[RFC 0006](../design/rfcs/0006-traits.md) writes the first option below as part of `traits.model-bounds`, because its worked case rests on it: `trait OrderSpec: Model { logic fn le(a: Snap<Self>, b: Snap<Self>) -> Prop; .. }` is implemented by a logical `Rational`, where `Snap<Rational>` is `Rational`, and by a runtime `Version`. `CloneEq`, which has `Model` as a supertrait, is built in for every logical type on the same footing. Nothing is blocked from being written. Accepting RFC 0006 with the rule accepts the first option.

## Options

1. **A logical type is its own snapshot and its own model,** and meets a `Model` bound. This is what RFC 0002's text and examples have, and what RFC 0006 writes. `models.snapshots.seen` then needs logical types in its list, and `logic.fn.parameters` needs "stands for a runtime type" reworded. RFC 0006 changes neither.
2. **A logical type has neither.** Then a `Vec<Nat>` of Locus's library has no method with a contract, and the two decided examples above are rejected or rewritten. A specification trait cannot be written over `Snap<Self>` for a logical type, and one specification for both kinds of type is stated on the model's type alone, with the code's trait bounded `Model<Logic: OrderSpec>`. A runtime type with the default model would then have to declare a model of a named logical type before it could have a specification, since no header names a snapshot type.

## Example

`traits/specification-and-code-in-two-traits.accept.lc` is accepted under the first option and rejected under the second, at `impl OrderSpec for Level`.

## Log

### 2026-11-23, raised while writing RFC 0006

Found while bringing the RFC on traits into the repository. Its fourth draft said: "Whether `Snap` of a logical type is that type is for RFC 0002 to say." RFC 0002's rules do not say it, its text says it of the model, and its examples need it of the bound. RFC 0006 states it as part of `traits.model-bounds` and lists it second among what is still for the owner.
