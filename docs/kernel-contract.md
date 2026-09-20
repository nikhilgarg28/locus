# Kernel contract

This document states every rule the proof kernel implements, with exact premises and conclusions. The kernel lives in `src/kernel/` and is independent of the parser. The code and this document change together: a rule is not in the kernel unless it is written here, and nothing here is in force until a kernel test exercises it.

It covers kernel gates **K1** and **K2** of [the core plan](core-plan.md). Tests for K1 are in `tests/kernel.rs` and for K2 in `tests/kernel_products.rs`. The intended full rule inventory is section 12.2 of [the specification](core-language-spec.md); rules are added here as each gate is built.

## Representation

Types:

~~~
Type ::= bool | u8 | Prop
       | @P                       the proofs of the proposition P
       | (A_0, A_1, ..., A_n)     a telescope: A_i may mention fields 0..i-1
       | struct S                 a declared struct, by identity
~~~

`Prop` and `@P` are ghost types: they have no runtime representation. A term occurs inside a type only inside some `@P`. A value can therefore change what a later proof field says, and never what data a product holds.

In a telescope, field `i` is under `i` binders: `#0` in it is field `i - 1`, `#1` is field `i - 2`, and so on. `A_i[v_0, ..., v_{i-1}]` below means field `i`'s type with each earlier field replaced by the given term.

Declarations. A struct declaration is a closed telescope. It is checked against the declarations that precede it, so a struct cannot mention itself, directly or indirectly. Structs are nominal: two declarations with identical fields are different types. Tuple types are structural, and the names a source program gives tuple fields do not exist here.

Terms. A proposition is a term of type `Prop`; there is no separate syntactic class.

~~~
t ::= x                     a context variable, by identity
    | #i                    a bound variable, by de Bruijn index
    | true | false | 0..255
    | wrapping_add(t, t) | wrapping_sub(t, t)
    | t ==[A] t             equality at type A
    | t => t
    | forall (#: A) { t }   binds #0 in its body
    | (t_0, ..., t_n) : (A_0, ..., A_n)     a tuple value carries its telescope
    | S { t_0, ..., t_n }
    | t.i                   positional projection
    | proof(p)              a proof used as a value
~~~

A tuple value carries its type because a dependent telescope cannot be inferred from the values alone.

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
    | of_term(t)            a term of proof type, used as a proof
    | projection(t)         computation axiom
    | literal(t)            computation axiom
~~~

Hypotheses bound by `implies_intro` are indexed separately from term variables.

## Context

A context is an ordered list of three kinds of entry:

~~~
x : A            an executable variable
ghost x : A      a ghost variable
h : P            a hypothesis, where P is a proposition well formed in the preceding context
~~~

A variable whose type is a ghost type is ghost however it was declared. Declaring a variable requires its type to be well formed.

An immutable logical `let x = e` reaches the kernel as `x : A` followed by the hypothesis `x ==[A] e`, where `e : A`. `Context::define` does exactly that and is not a rule: it is `declare` followed by `assume`. The same fact is derivable inside a closed statement by instantiating `forall x, x == e => P(x)` at `e` and discharging the premise with `refl(e)`; `tests/kernel_products.rs` does so.

Terms are typed in one of two modes. `Logical` is the upgraded reading of the context: ghost variables are ordinary variables. `Executable` means the term's value is required at runtime.

## Type formation: `ctx |- A type`

| Type | Premises |
|---|---|
| `bool`, `u8`, `Prop` | none |
| `@P` | `P : Prop` in `Logical` mode |
| `(A_0, ..., A_n)` | for each `i`, with fresh ghost variables `x_0 : A_0, ..., x_{i-1} : A_{i-1}[...]` added, `A_i[x_0, ..., x_{i-1}]` is a type |
| `struct S` | `S` is declared |

## Term typing: `ctx |- t : A` in a mode

| Term | Premises | Type |
|---|---|---|
| `x` | `x : A` or `ghost x : A` is in `ctx`. In `Executable` mode, `x` is not ghost. | `A` |
| `#i` | none: a bound index outside any binder is rejected | |
| `true`, `false` | | `bool` |
| `0..255` | | `u8` |
| `wrapping_add(a, b)`, `wrapping_sub(a, b)` | exactly two arguments; `a : u8` and `b : u8` in the same mode | `u8` |
| `a ==[A] b` | mode is `Logical`; `A` is a type and not a proof type; `a : A` and `b : A` in `Logical` mode | `Prop` |
| `P => Q` | mode is `Logical`; `P : Prop` and `Q : Prop` in `Logical` mode | `Prop` |
| `forall (#: A) { P }` | mode is `Logical`; `A` is a type; for a fresh ghost `x : A`, `P[x] : Prop` in `Logical` mode | `Prop` |
| `(t_0, ..., t_n) : (A_0, ..., A_n)` | the telescope is a type; the field rule below holds | `(A_0, ..., A_n)` |
| `S { t_0, ..., t_n }` | `S` is declared with fields `(A_0, ..., A_n)`; the field rule below holds | `struct S` |
| `t.i` | `t : (A_0, ..., A_n)` or `t : struct S` in the same mode; `i <= n`. In `Executable` mode the result type is not ghost. | `A_i[t.0, ..., t.(i-1)]` |
| `proof(p)` | mode is `Logical`; `p` proves `P` | `@P` |

The field rule. There are exactly as many values as fields. For each `i` in order, let `E = A_i[t_0, ..., t_{i-1}]`:

- if `E` is `@P`, then `t_i` must have the form `proof(p)` and `p` must prove `P`;
- if `E` is another ghost type, `t_i : E` in `Logical` mode;
- otherwise `t_i : E` in the product's own mode.

So a ghost field is a logical position even inside an executable product: a ghost variable may be mentioned by a proof field's proposition, or stored in a `Prop` field, but not stored in a data field of executable data.

Requiring the literal form `proof(p)` in every proof field is what makes proof irrelevance a syntactic check. A projection or variable of proof type is used in such a position as `proof(of_term(t))`.

A proposition former, a `proof(p)`, or a projection of a ghost field in `Executable` mode is rejected: a term of ghost type has no runtime value. Inside a proposition former the mode is always `Logical`, which is the upgrade rule of specification section 2.4.

Equality may be formed at any type except a proof type, including `Prop` and product types.

## Proof checking: `ctx |- p proves P`

The kernel reads the proposition off the proof. It never searches.

| Proof | Premises | Conclusion |
|---|---|---|
| `h` | `h : P` is in `ctx` | `P` |
| `refl(t)` | `t : A` in `Logical` mode; `A` is not a proof type | `t ==[A] t` |
| `transport(e, T, p)` | `e` proves `a ==[A] b`; for a fresh ghost `x : A`, `T[x] : Prop`; `p` proves `T[a]` | `T[b]` |
| `implies_intro(P, p)` | `P : Prop`; with a fresh hypothesis `h : P` added, `p[h]` proves `Q` | `P => Q` |
| `implies_elim(f, p)` | `f` proves `P => Q`; `p` proves `P` | `Q` |
| `forall_intro(A, p)` | with a fresh ghost `x : A` added, `p[x]` proves `P` | `forall (#: A) { close(P, x) }` |
| `forall_elim(f, t)` | `f` proves `forall (#: A) { P }`; `t : A` in `Logical` mode | `P[t]` |
| `of_term(t)` | `t : @P` in `Logical` mode | `P` |
| `projection(c.i)` | `c` is literally a tuple or struct value with fields `v_0, ..., v_n`; `c.i : A` in `Logical` mode; `A` is not a proof type; `v_i : A` | `c.i ==[A] v_i` |
| `literal(op(a, b))` | `op` is `wrapping_add` or `wrapping_sub`; `a` and `b` are `u8` literals | `op(a, b) ==[u8] r`, where `r` is the result modulo 256 |

`projection` and `literal` are the computation axioms of specification section 12.2 that exist so far; the `let` axiom is a hypothesis, as described under Context. Each is a single step validated by matching the shape of its term. They are used through `transport`, and the elaborator will insert them silently.

The premise `v_i : A` of `projection` matters only for a nested dependent product: the projection's type names earlier fields as `c.j`, while the value's own type names them as `v_j`. The step is offered only when the two types are already the same. Bridging them would need transport between types, which the kernel does not have.

`literal` evaluates with Rust's `u8::wrapping_add` and `u8::wrapping_sub`. Gate K5 adds the model of `u8` that this evaluation must agree with.

`check(ctx, p, P)` requires `P : Prop`, infers the proposition `p` proves, and accepts when the two are the same term.

## Comparison

"The same term" means equal up to two things, and nothing else:

- renaming of bound variables, which the locally nameless representation makes structural equality;
- proof irrelevance: any two terms of the form `proof(p)` are the same, whatever `p` is.

The same relation on types compares the propositions inside `@P` and the fields of telescopes with it.

There is no unfolding, no evaluation, and no normalization. `refl(2)` does not prove `wrapping_add(1, 1) == 2`; `literal(wrapping_add(1, 1))` does. `refl(3)` does not prove `(3, true).0 == 3`; `projection((3, true).0)` does.

Proof irrelevance is sound as a syntactic check because of the field rule: in a well-typed term, every position of proof type inside a product value holds a `proof(p)`, equality at a proof type cannot be formed, and no other term former has a proof-typed argument. Two product values with the same data and different proofs are therefore the same term, and `refl` proves them equal with no proof step.

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
| K3 | Math functions and defining equations; `unfold`, `fold`, `rewrite` as transports; function values and types; `Prop` fields |
| K4 | Enums and the case rule with arm evidence; declared props and the index-equation case rule; `Exists`; excluded middle with dependency recording |
| K5 | Internal `Nat` with induction; the `u8` model, reflection lemmas, native evaluation |
| K6 | The range-iteration rule |

One K2 acceptance condition is stated in the plan in terms of `NonZero`. The kernel has no `!=` until K4 brings `False`, so the test uses a struct whose proof field is an equation, `a.wrapping_add(b) == 10`, to the same effect.

## Trusted base

`src/kernel/check.rs` (the rules and the comparison), `src/kernel/term.rs` (opening and closing binders, and telescope instantiation), `src/kernel/context.rs` (lookup and scoping), `src/kernel/defs.rs` (declarations), and Rust's wrapping `u8` arithmetic behind `literal`. The builder functions on `Term` and `Proof` that take closures are conveniences for constructing well-scoped terms; a term built any other way is checked just the same.

Known limit: checking is recursive and has no depth bound yet. Hand-written terms cannot exhaust the stack; this must be addressed before the kernel accepts terms produced from untrusted source text.
