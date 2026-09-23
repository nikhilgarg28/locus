+++
id = "language-proofs"
title = "Proofs and evidence"
group = "Now"
spec_chapter = 1
order = 109
route = "specification/proofs.html"
description = "Constructing, transporting, composing, and checking evidence."
+++

# Proofs and evidence

<!-- spec: 1.0:11 informative -->
Proofs have explicit places in expressions, function signatures and aggregates. You can supply evidence already in scope, build it with a proof form, or leave a hole for the compiler’s bounded search. Every accepted result is checked against the required claim.

## Proofs and their forms

<!-- spec: 1.7:1 syntax -->
A proof type is `@name`, `@f(args)` for a call of the logic, `@N(args)` for a declared proposition, or `@(F)` for a formula; a leading `@` begins a proof type; between a named proposition constructor and its body evidence it separates the two expression operands. Evidence is an ordinary expression of a proof type:

<!-- spec: 1.7:2 syntax -->
| Written | What it is |
|---|---|
| `_` | a hole: the compiler finds the evidence, by the tiers of [What a hole finds](#what-a-hole-finds) |
| `prove!(F)` | a claim stated where it stands: as a statement it leaves `F` known to everything after it; as a value it is evidence of `F` |
| a name, a field, a call | evidence already held, a field of evidence, or evidence a function returns |
| `And::Intro(h, k)`, `Or::Left(h)`, `Or::Right(h)`, `True::Intro` | the constructors of the built-in connectives |
| `N::Arm(witnesses) @ evidence`, `N::Arm { witness: value } @ evidence` | a named-arm constructor with a separate body-evidence slot |
| `match h { N::Arm(x) @ body => ..., }` | Proof elimination; nonempty matches produce evidence, so witnesses cannot be extracted into data. Empty false elimination remains valid. |
| `rewrite!(eq, h)` | `h` carried across the equation `eq` proves, every occurrence of its left side replaced by its right |
| `unfold!(f, h)`, `fold!(f, h)` | `h` with a call of the function `f` of the logic opened to its body, or the body closed to the call; `f` is a name or a path such as `Counter::small` |
| `unfold!(x as M, h)`, `fold!(x as M, h)` | select the checked Model implementation for the observation `x as M` and open or close its defining equation |
| `general(p)(q)(h)` | evidence of a `forall` or an implication applied to an argument |
| `u32_le_trans(a, b, c, ab, bc)` | a lemma of the theory called by name, its premises as arguments |
| `f` | a function of the logic that returns evidence, named as evidence of its general claim |

<!-- spec: 1.7:3 syntax -->
`examples/proofs.lc`, `examples/propositions.lc`, `tests/corpus/accept/explicit_steps.lc`, and `tests/corpus/accept/forms.lc` show each. The forms take `!` as a Rust macro does; a bare `rewrite(...)` is an error with a fix.

<!-- spec: 1.7:4 syntax -->
The lemmas callable by name are the 162 the kernel's theory declares: six about `Int`, `int_le_of_lt`, `int_lt_of_le_of_ne`, `int_le_add_left`, `int_le_add_right`, `int_le_sub`, `int_mul_le_mul_nonneg`; and at each of the eight machine types `T`, written `<T>_<lemma>` as `u32_le_trans` and `i8_view_bounds`, `le_refl`, `le_trans`, `le_of_lt`, `lt_of_le_of_ne`, `lt_irrefl`, `le_antisymm`, `view_injective`, `view_bounds`, `le_of_cmp`, `cmp_of_le`, `lt_of_cmp`, `cmp_of_lt`, `eq_of_cmp`, `cmp_of_eq`, `lt_of_not_le`, `le_of_not_lt`, `succ_le_of_lt`, and `eq_symm`, with `zero_le`, `sub_le`, and `sub_le_sub` at the unsigned types. Their statements are in the [kernel contract](../reference/kernel.md#lemmas).

<!-- spec: 1.7:5 syntax -->
Evidence is accepted only for the claim wanted, after computing ([What a hole finds](#what-a-hole-finds)): evidence of `x <= 3` where `y <= 3` is wanted is a mismatch, whatever relates `x` and `y`.

## What a hole finds

<!-- spec: 1.8:1 legality-rule -->
A `_`, a `prove!`, the obligation of an operator under `no_panic`, and the evidence of a panic form under `no_panic` are filled by a fixed search whose tiers are tried in order. Every limit in it is a count of work and never a clock, and the proof it finds is checked by the kernel like any other.

<!-- spec: 1.8:2 legality-rule -->
| Tier | What it does |
|---|---|
| stored | the entry for this obligation in the source directory’s `Locus.lock` ([Found proofs are stored](14-tooling.md#found-proofs-are-stored)), checked again |
| exact | a fact in scope that is the claim: a branch or arm fact, a `let` equation, a range bound, evidence in scope, or an evidence field of a value in scope |
| computed | the same after computing: `let` names replaced by what they stand for, projections of written products, matches on written constructors, and arithmetic on literals; a definition is never unfolded on its own |
| evaluation | a closed comparison, run by the kernel: `0 <= 3`, `2 != 0` |
| arithmetic | the linear arithmetic procedure (`src/arith`): the claim and the facts read as linear constraints over `Int`, the ranges of every machine value in them, the exact results of operators already discharged, and division by a literal reduced to the kernel's decomposition; it emits a certificate the kernel's `linear` rule checks |

<!-- spec: 1.8:3 legality-rule -->
What a hole does not do: take a connective apart, unfold a definition, rewrite by an equation in scope, try every value of a byte, or bridge evidence of one claim to another. Each of those is a form of [Proofs and their forms](#proofs-and-their-forms), and the diagnostic for a failed hole names which. `locus check --holes` lists every hole with the tier that filled it and the size of its proof; `--stats` counts the obligations by tier.
