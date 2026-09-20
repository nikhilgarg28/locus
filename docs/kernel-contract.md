# Kernel contract

This document states every rule the proof kernel implements, with exact premises and conclusions. The kernel lives in `src/kernel/` and is independent of the parser. The code and this document change together: a rule is not in the kernel unless it is written here, and nothing here is in force until a test in `tests/kernel.rs` exercises it.

It covers kernel gate **K1** of [the core plan](core-plan.md). The intended full rule inventory is section 12.2 of [the specification](core-language-spec.md); rules are added here as each gate is built.

## Representation

Types in K1:

~~~
Type ::= bool | u8 | Prop
~~~

`Prop` is a ghost type: it has no runtime representation.

Terms. A proposition is a term of type `Prop`; there is no separate syntactic class.

~~~
t ::= x                     a context variable, by identity
    | #i                    a bound variable, by de Bruijn index
    | true | false | 0..255
    | wrapping_add(t, t) | wrapping_sub(t, t)
    | t ==[A] t             equality at type A
    | t => t
    | forall (#: A) { t }   binds #0 in its body
~~~

Binding is locally nameless. Variables of the context carry globally unique identities that are never reused. Variables bound inside a term are indices. Two consequences are relied on throughout:

- Terms that differ only in the names of bound variables are equal as data.
- Substituting a term that is well formed in the context under a binder needs no shifting, because such a term contains no dangling index.

`t[u]` below means: replace the outermost bound variable of `t` by `u`. `close(t, x)` means: turn the context variable `x` into the outermost bound variable of `t`.

Proofs:

~~~
p ::= h                     a context hypothesis, by identity
    | refl(t)
    | transport(p, T, p)    T is a template: a term binding #0 as its hole
    | implies_intro(t, p)   binds a hypothesis in p
    | implies_elim(p, p)
    | forall_intro(A, p)    binds a term variable in p
    | forall_elim(p, t)
~~~

Hypotheses bound by `implies_intro` are indexed separately from term variables.

## Context

A context is an ordered list of three kinds of entry:

~~~
x : A            an executable variable
ghost x : A      a ghost variable
h : P            a hypothesis, where P is a proposition well formed in the preceding context
~~~

A variable whose type is a ghost type is ghost however it was declared.

Terms are typed in one of two modes. `Logical` is the upgraded reading of the context: ghost variables are ordinary variables. `Executable` means the term's value is required at runtime.

## Term typing: `ctx |- t : A` in a mode

| Term | Premises | Type |
|---|---|---|
| `x` | `x : A` or `ghost x : A` is in `ctx`. In `Executable` mode, `x` is not ghost. | `A` |
| `#i` | none: a bound index outside any binder is rejected | |
| `true`, `false` | | `bool` |
| `0..255` | | `u8` |
| `wrapping_add(a, b)`, `wrapping_sub(a, b)` | exactly two arguments; `a : u8` and `b : u8` in the same mode | `u8` |
| `a ==[A] b` | mode is `Logical`; `a : A` and `b : A` in `Logical` mode | `Prop` |
| `P => Q` | mode is `Logical`; `P : Prop` and `Q : Prop` in `Logical` mode | `Prop` |
| `forall (#: A) { P }` | mode is `Logical`; for a fresh ghost `x : A`, `P[x] : Prop` in `Logical` mode | `Prop` |

A proposition former in `Executable` mode is rejected: a term of ghost type has no runtime value. Inside a proposition former the mode is always `Logical`, which is the upgrade rule of specification section 2.4.

Equality may be formed at any type, including `Prop`.

## Proof checking: `ctx |- p proves P`

The kernel reads the proposition off the proof. It never searches.

| Proof | Premises | Conclusion |
|---|---|---|
| `h` | `h : P` is in `ctx` | `P` |
| `refl(t)` | `t : A` in `Logical` mode | `t ==[A] t` |
| `transport(e, T, p)` | `e` proves `a ==[A] b`; for a fresh ghost `x : A`, `T[x] : Prop`; `p` proves `T[a]` | `T[b]` |
| `implies_intro(P, p)` | `P : Prop`; with a fresh hypothesis `h : P` added, `p[h]` proves `Q` | `P => Q` |
| `implies_elim(f, p)` | `f` proves `P => Q`; `p` proves `P` | `Q` |
| `forall_intro(A, p)` | with a fresh ghost `x : A` added, `p[x]` proves `P` | `forall (#: A) { close(P, x) }` |
| `forall_elim(f, t)` | `f` proves `forall (#: A) { P }`; `t : A` in `Logical` mode | `P[t]` |

`check(ctx, p, P)` requires `P : Prop`, infers the proposition `p` proves, and accepts when the two are the same term.

## Comparison

"The same term" is structural equality of the locally nameless representation, which is equality up to renaming of bound variables. There is no other comparison anywhere in the kernel: no unfolding, no evaluation, no normalization. `refl(2)` does not prove `wrapping_add(1, 1) == 2`.

Proof irrelevance is part of comparison in the specification. It is vacuous in K1, because no K1 term can contain a proof. It becomes a real clause at K2, when data can carry proof fields.

## Invariants the implementation maintains

- Every proposition returned by proof inference is well formed in the context it was inferred in.
- Everything pushed while checking under a binder is removed before returning, on success and on failure.
- A fresh variable or hypothesis has an identity that occurs nowhere else, so generalizing over it cannot capture anything, and it cannot be referred to after its binder is left.
- Replacement terms used for `t[u]` are well formed in the context, hence contain no dangling index. For `forall_elim` this is established by typing the argument first.

## Deliberately absent

Symmetry, transitivity, and congruence of equality are not rules. They are derived from `refl` and `transport`, and `tests/kernel.rs` derives each of them.

Not yet present, by gate:

| Gate | Adds |
|---|---|
| K2 | Tuples, structs, dependent proof fields; the let, projection, and literal computation axioms; proof irrelevance in comparison |
| K3 | Math functions and defining equations; `unfold`, `fold`, `rewrite` as transports; function values and types; `Prop` fields |
| K4 | Enums and the case rule with arm evidence; declared props and the index-equation case rule; `Exists`; excluded middle with dependency recording |
| K5 | Internal `Nat` with induction; the `u8` model, reflection lemmas, native evaluation |
| K6 | The range-iteration rule |

## Trusted base at K1

`src/kernel/check.rs` (the rules and the comparison), `src/kernel/term.rs` (opening and closing binders), and `src/kernel/context.rs` (lookup and scoping). The builder functions on `Term` and `Proof` that take closures are conveniences for constructing well-scoped terms; a term built any other way is checked just the same.

Known limit: checking is recursive and has no depth bound yet. Hand-written terms cannot exhaust the stack; this must be addressed before the kernel accepts terms produced from untrusted source text.
