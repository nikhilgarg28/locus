# Kernel contract

This document states every rule the proof kernel implements, with exact premises and conclusions. The kernel lives in `src/kernel/` and is independent of the parser. The code and this document change together: a rule is not in the kernel unless it is written here, and nothing here is in force until a kernel test exercises it.

It covers all six kernel gates, **K1** to **K6**, of [the core plan](core-plan.md). Tests are in `tests/kernel.rs` (K1), `tests/kernel_products.rs` (K2), `tests/kernel_functions.rs` (K3), `tests/kernel_cases.rs` (K4), `tests/kernel_numbers.rs` (K5), `tests/kernel_loops.rs` (K6), `tests/kernel_evaluation.rs` (evaluation and the range successor axiom), and `tests/kernel_depth.rs` (the depth bound). The intended full rule inventory is section 12.2 of [the specification](core-language-spec.md); rules are added here as each gate is built.

## Representation

Types:

~~~
Type ::= bool | u8 | Prop
       | Nat                      internal natural numbers
       | @P                       the proofs of the proposition P
       | (A_0, A_1, ..., A_n)     a telescope: A_i may mention fields 0..i-1
       | struct S                 a declared struct, by identity
       | enum E                   a declared enum, by identity
       | math fn(A_0, ..., A_n) -> R     parameters form a telescope; R may mention all of them
~~~

`Prop`, `@P`, and `Nat` are ghost types: they have no runtime representation. `Nat` is internal to the kernel: it is the model of `u8` and the domain of induction, and a source program cannot name it. A function type is ghost when its result type is: such a function is a predicate or a proof. Only total functions have kernel types; an ordinary `fn` never reaches the kernel. A term occurs inside a type only inside some `@P`. A value can therefore change what a later proof field says, and never what data a product holds.

In a telescope, field `i` is under `i` binders: `#0` in it is field `i - 1`, `#1` is field `i - 2`, and so on. `A_i[v_0, ..., v_{i-1}]` below means field `i`'s type with each earlier field replaced by the given term.

Declarations. A struct declaration is a closed telescope. An enum declaration is a list of variants, each a closed payload telescope. A proposition declaration is described under Declared propositions below. A math function declaration is a closed function type and a body. Each declaration is checked against the declarations that precede it, so a struct, enum, or proposition cannot mention itself and a function cannot call itself, directly or indirectly. Kernel terms contain no loop, so every declared function is total by construction. Structs are nominal: two declarations with identical fields are different types. Tuple types are structural, and the names a source program gives tuple fields do not exist here.

Terms. A proposition is a term of type `Prop`; there is no separate syntactic class.

~~~
t ::= x                     a context variable, by identity
    | #i                    a bound variable, by de Bruijn index
    | true | false | 0..255
    | 0n, 1n, ...           Nat literals, of arbitrary size
    | wrapping_add(t, t) | wrapping_sub(t, t)
    | u8_eq(t, t) | u8_lt(t, t) | u8_le(t, t)     the runtime comparisons, of type bool
    | to_nat(t) | of_nat(t) | succ(t) | nat_add(t, t)
    | t ==[A] t             equality at type A
    | t => t
    | forall (#: A) { t }   binds #0 in its body
    | (t_0, ..., t_n) : (A_0, ..., A_n)     a tuple value carries its telescope
    | S { t_0, ..., t_n }
    | t.i                   positional projection
    | proof(p)              a proof used as a value
    | f                     a declared math function, as a value
    | t(t_0, ..., t_n)      application
    | E::i(t_0, ..., t_n)   variant i of a declared enum
    | case t : R { arm_0, ..., arm_n }     arm_i binds the payload of variant i
    | N(t_0, ..., t_n)      a declared proposition applied to arguments
    | exists (#: A) { t }   binds #0 in its body
    | absurd(p) : A         a value of any type from a proof of an empty proposition
    | for # in t..t (S = t) { t }     range iteration; see below
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
    | definition(t)         computation axiom
    | case_step(t)          computation axiom
    | construct(N, i, params, payload)     variant i of a declared proposition
    | case_proof(p, G, arms)    case analysis on a proof
    | case_data(t, G, arms)     case analysis on data
    | exists_intro(P, t, p)
    | exists_elim(p, G, arm)
    | excluded_middle(P)
    | for_empty(t)          computation axiom
    | for_step(t, p, p)     computation axiom
    | evaluate(t)           big-step evaluation of a closed term
    | evaluate_all(t)       a claim about every byte, by 256 evaluations; t binds #0
    | omitted               left by the evaluator; proves nothing
    | axiom(...)            an axiom of Nat or of the u8 model
    | nat_induction(M, p, arm, t)
~~~

An arm of a proof-level case binds some term variables and some hypotheses, and states how many of each.

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
| `enum E` | `E` is declared |
| `math fn(A_0, ..., A_n) -> R` | `(A_0, ..., A_n, R)` is a well-formed telescope |

## Term typing: `ctx |- t : A` in a mode

| Term | Premises | Type |
|---|---|---|
| `x` | `x : A` or `ghost x : A` is in `ctx`. In `Executable` mode, `x` is not ghost. | `A` |
| `#i` | none: a bound index outside any binder is rejected | |
| `true`, `false` | | `bool` |
| `0..255` | | `u8` |
| `wrapping_add(a, b)`, `wrapping_sub(a, b)` | exactly two arguments; `a : u8` and `b : u8` in the same mode | `u8` |
| `u8_eq(a, b)`, `u8_lt(a, b)`, `u8_le(a, b)` | `a : u8` and `b : u8` in the same mode | `bool` |
| `0n, 1n, ...` | mode is `Logical` | `Nat` |
| `to_nat(a)` | mode is `Logical`; `a : u8` | `Nat` |
| `of_nat(n)` | `n : Nat`, which is only possible in `Logical` mode | `u8` |
| `succ(n)`, `nat_add(n, m)` | mode is `Logical`; the arguments have type `Nat` | `Nat` |
| `a ==[A] b` | mode is `Logical`; `A` is a type and not a proof type; `a : A` and `b : A` in `Logical` mode | `Prop` |
| `P => Q` | mode is `Logical`; `P : Prop` and `Q : Prop` in `Logical` mode | `Prop` |
| `forall (#: A) { P }` | mode is `Logical`; `A` is a type; for a fresh ghost `x : A`, `P[x] : Prop` in `Logical` mode | `Prop` |
| `(t_0, ..., t_n) : (A_0, ..., A_n)` | the telescope is a type; the field rule below holds | `(A_0, ..., A_n)` |
| `S { t_0, ..., t_n }` | `S` is declared with fields `(A_0, ..., A_n)`; the field rule below holds | `struct S` |
| `t.i` | `t : (A_0, ..., A_n)` or `t : struct S` in the same mode; `i <= n`. In `Executable` mode the result type is not ghost. | `A_i[t.0, ..., t.(i-1)]`, or, when `t` is literally a product value with fields `v_0, ..., v_n`, `A_i[v_0, ..., v_{i-1}]` |
| `proof(p)` | mode is `Logical`; `p` proves `P` | `@P` |
| `E::i(t_0, ..., t_n)` | `E` is declared and has a variant `i` with payload `(A_0, ..., A_n)`; the field rule holds for the payload, in the same mode | `enum E` |
| `case t : R { arms }` | `t : bool` or `t : enum E` in the same mode; `R` is a type, is not a proof type, and in `Executable` mode is not ghost; there is exactly one arm per variant, in order (`false`, `true` for `bool`); arm `i` binds exactly the payload of variant `i` as variables `ys`, together with the hypothesis `t ==[T] variant_i(ys)` where `T` is the scrutinee's type, and with those added its body has type `R` in the same mode | `R` |
| `N(t_0, ..., t_n)` | mode is `Logical`; `N` is declared with parameters `(A_0, ..., A_n)`; each `t_j : A_j` in `Logical` mode | `Prop` |
| `exists (#: A) { P }` | as `forall` | `Prop` |
| `absurd(p) : A` | `p` proves `N(...)` where `N` is declared with no variants; `A` is a type; in `Executable` mode `A` is not ghost | `A` |
| `f` | `f` is declared with type `F`. In `Executable` mode `F` is not ghost. | `F` |
| `t(t_0, ..., t_n)` | `t : math fn(A_0, ..., A_n) -> R` in the same mode; the field rule below holds for the arguments against the parameters. In `Executable` mode the result type is not ghost. | `R[t_0, ..., t_n]` |

Each arm's hypothesis is the same fact `case_data` gives its arms, so a branch of a math function knows what a branch of executable code knows, and a proof field in an arm can use it. It is a hypothesis, so it can occur only inside proofs, and it has no runtime content.

In an executable `case`, a payload variable is executable when its field has a runtime representation and ghost otherwise; in a logical `case` every payload variable is ghost. The result type of a `case` does not depend on the scrutinee. A `case` cannot scrutinize a proof and cannot have a proof type as its result: case analysis that inspects or produces proofs is a proof rule, below. `absurd` is the match with no arms used for its value; it marks a point that is never reached.

Function declaration. `math fn f(x_0: A_0, ..., x_n: A_n) -> R { body }` is accepted when its function type is well formed with no variables in scope, and, with fresh ghost variables for the parameters, `body : R[x_0, ..., x_n]` in `Logical` mode. A lemma is a function whose result type is a proof type; its body has the form `proof(p)`.

The field rule. It applies to the fields of a product value and to the arguments of a call. There are exactly as many values as fields. For each `i` in order, let `E = A_i[t_0, ..., t_{i-1}]`:

- if `E` is `@P`, then `t_i` must have the form `proof(p)` and `p` must prove `P`;
- if `E` is another ghost type, `t_i : E` in `Logical` mode;
- otherwise `t_i : E` in the product's own mode.

So a ghost field is a logical position even inside an executable product: a ghost variable may be mentioned by a proof field's proposition, or stored in a `Prop` field, but not stored in a data field of executable data.

Requiring the literal form `proof(p)` in every proof field is what makes proof irrelevance a syntactic check. A projection or variable of proof type is used in such a position as `proof(of_term(t))`.

A proposition former, a `proof(p)`, or a projection of a ghost field in `Executable` mode is rejected: a term of ghost type has no runtime value. Inside a proposition former the mode is always `Logical`, which is the upgrade rule of specification section 2.4.

Equality may be formed at any type except a proof type, including `Prop`, product types, and function types. There is no extensionality rule: nothing concludes `f == g` from `forall x, f(x) == g(x)`, and nothing concludes `P == Q` from `P => Q` and `Q => P`.

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
| `projection(c.i)` | `c` is literally a tuple or struct value with fields `v_0, ..., v_n`; `c.i : A` in `Logical` mode; `A` is not a proof type | `c.i ==[A] v_i` |
| `definition(f(t_0, ..., t_n))` | the callee is literally a declared function `f` with parameters `x_0, ..., x_n` and body `b`; `f(t_0, ..., t_n) : A` in `Logical` mode; `A` is not a proof type | `f(t_0, ..., t_n) ==[A] b[t_0, ..., t_n]` |
| `case_step(case c : R { arms })` | `c` is literally `false`, `true`, or `E::i(v_0, ..., v_n)`; the case is well typed in `Logical` mode | `case c ... ==[R] arm_i[v_0, ..., v_n]`, with the arm's hypothesis replaced by `refl(c)` |
| `literal(op(args))` | `op` is a primitive and every argument is a literal of the right type | `op(args) ==[A] r`, where `r` is the literal the native evaluation below produces and `A` is the primitive's result type |

### Declared propositions

`prop N(x_0: A_0, ..., x_n: A_n) { variants }` is accepted when each `A_j` is a type that is not a proof type, and each variant has one of two shapes:

- A variant without a stated conclusion has a payload telescope that may mention the parameters. It proves `N` at whatever parameters it is given.
- A variant with a stated conclusion has a closed payload telescope `(B_0, ..., B_m)` and arguments `c_0, ..., c_n` under it, with each `c_j : A_j`. It proves exactly `N(c_0, ..., c_n)`. The parameters are not in scope in it.

A parameter cannot be a proof, so no parameter type depends on an earlier parameter, and index equations are independent. A conclusion is a list of arguments for `N`, not a proposition, so a variant cannot conclude anything but `N`; the surface rule that says so is enforced by the representation. A payload cannot mention `N`, because `N` has no identity until its declaration is accepted.

| Proof | Premises | Conclusion |
|---|---|---|
| `construct(N, i, (t_0..t_n), payload)`, variant `i` without a stated conclusion | the field rule holds for `(t_0, ..., t_n, payload...)` against the parameters followed by the payload telescope | `N(t_0, ..., t_n)` |
| `construct(N, i, (), payload)`, variant `i` with a stated conclusion | the field rule holds for the payload | `N(c_0[payload], ..., c_n[payload])` |
| `case_proof(p, G, arms)` | `p` proves `N(a_0, ..., a_n)`; `G : Prop` in the current context; one arm per variant, in order; each arm proves `G` under the bindings below | `G` |
| `case_data(t, G, arms)` | `t : bool` or `t : enum E` in `Logical` mode, of type `T`; `G : Prop` in the current context; one arm per variant; arm `i` binds fresh ghost payload variables `ys` and the hypothesis `t ==[T] variant_i(ys)`, and proves `G` | `G` |

What an arm of `case_proof` binds:

- for a variant without a stated conclusion: payload variables whose types are the payload telescope with the parameters replaced by `a_0, ..., a_n`, and no hypotheses;
- for a variant with a stated conclusion: payload variables `ys`, then one hypothesis per parameter, `a_j ==[A_j] c_j[ys]`. These are the index equations. There is no equation about the scrutinee itself: under proof irrelevance it would say nothing.

`G` is checked before anything is bound, so it cannot mention a payload variable, and it does not depend on the proof being analyzed. A proposition with no variants has a `case_proof` with no arms, which proves any `G`.

The result of `case_proof` is a proof by construction, and every arm is a kernel proof, hence total. The specification's restrictions on a match over a proof, that its result is a proof and that its arms are total logical computations, therefore cannot be violated in kernel terms; rejecting surface programs that try is the elaborator's job.

The prelude declares `True`, `False`, `And`, and `Or` by this mechanism, before anything else, and records their identities. `!P` is `P => False`.

### Exists

| Proof | Premises | Conclusion |
|---|---|---|
| `exists_intro(P, t, p)` | `P` is `exists (#: A) { B }` and `P : Prop`; `t : A` in `Logical` mode; `p` proves `B[t]` | `P` |
| `exists_elim(p, G, arm)` | `p` proves `exists (#: A) { B }`; `G : Prop` in the current context; the arm binds a fresh ghost `w : A` and the hypothesis `B[w]`, and proves `G` | `G` |

`G` cannot mention the witness, and the result is a proof: an existential cannot be mined for data.

### Excluded middle

| Proof | Premises | Conclusion |
|---|---|---|
| `excluded_middle(P)` | the declarations include the prelude; `P : Prop` | `Or(P, P => False)` |

`proof_is_classical` reports whether a proof uses this rule, directly or through a declared function, and each function declaration records the same about its body. This is bookkeeping for auditing, not a check.

### Range iteration

`for i in lo..hi (s : S(i) = init) { body }` is the term-level recursion rule. `S(i)` is a tuple telescope under one binder, the index. `body` binds the index `i`, the current state `s`, and two hypotheses.

| Term | Premises | Type |
|---|---|---|
| `for i in lo..hi (s = init) { body }` | the declarations include the prelude; `lo : u8` and `hi : u8` in the same mode; the attached proof proves `u8_le(lo, hi)`; `init : S(lo)` in the same mode; for a fresh `i : u8`, `S(i)` is a type; with `i : u8`, `s : S(i)`, and the hypotheses `u8_le(lo, i)` and `u8_lt(i, hi)` added, `body : S(wrapping_add(i, 1))` in the same mode | `S(hi)` |

In `Executable` mode `i` and `s` are executable variables; in `Logical` mode they are ghost. The ghost fields of `s` are ghost either way, by the projection rule.

The index may change what the state's proofs say and never what data the state holds, because a term occurs in a type only inside `@P`. So the erasure of `S(i)` does not depend on `i`, and a loop whose invariant varies with the index erases to a plain loop over one fixed state type. That is what makes this rule erasable, and it holds by construction.

Ordered bounds make the final index `hi`, so the result type needs no case distinction. A reversed range is not a type error in itself; it is rejected because no proof of `u8_le(lo, hi)` can be supplied. Since `i < hi`, the successor in the body's type never wraps. A half-open byte range cannot have 255 as an index.

Two loops that differ only in the proof that their bounds are ordered are the same term.

| Proof | Premises | Conclusion |
|---|---|---|
| `for_empty(f)` | `f` is a `for` whose `lo` and `hi` are the same term, and is well typed | `f ==[S(lo)] init` |

| `for_step(f, lower, upper)` | `f` is a well-typed `for` over `lo..wrapping_add(h, 1)`, with the successor written exactly so; `lower` proves `u8_le(lo, h)`; `upper` proves `u8_lt(h, wrapping_add(h, 1))`; the unrolled right side below is well typed at the type of `f` | `f ==[S(wrapping_add(h, 1))] body[h, g, lower, upper]`, where `g` is the same `for` over `lo..h` with `lower` as its ordering proof |

Stating the successor case with the bound in successor form is what makes it work for index-dependent state: the body at index `h` has type `S(wrapping_add(h, 1))`, which is the type of `f`, so no transport between types is needed. `upper` says the successor does not wrap. The loop `g` reuses the body under the hypothesis `i < h`; a body whose proofs depend on the particular upper bound does not type-check there, and then there is no step. `for_empty` and `for_step` compute a loop symbolically; on literals, `evaluate` below is the practical route, because a literal bound cannot be rewritten into successor form inside a `for` that carries a proof about it.

Soundness of the typing rule is by induction on `hi - lo` in the intended model. It is a separate rule from `nat_induction` on purpose: one recursion rule computes and erases to a loop, the other only proves and is erased.

### Evaluation

| Proof | Premises | Conclusion |
|---|---|---|
| `evaluate(t)` | `t : A` in `Logical` mode; `A` is plain data: `bool`, `u8`, `Nat`, or a tuple, struct, or enum built only from those; `t` has no free variable; evaluation finishes within the step budget with value `v` | `t ==[A] v` |
| `evaluate_all(b)` | for a fresh ghost `x : u8`, `b[x] : bool`; for each byte `k`, `b[k]` evaluates to `true`, all within one step budget | `forall (#: u8) { b ==[bool] true }` |

Evaluation is big-step and call-by-value: primitives by native evaluation, a call by instantiating the function's body, `case` by choosing the arm of the evaluated scrutinee, projection from the evaluated product, and `for` by running the body for each index from `lo` to `hi`. It is a shortcut for a chain of the computation axioms and is the one place the kernel computes. It takes no part in comparing terms.

Proofs are never evaluated. The evaluator replaces each `proof(p)` it meets by `proof(omitted)`, so a loop state that carries proofs about the previous state does not grow. `omitted` proves nothing: checking it is an error. Because `evaluate` offers only plain data, no omitted proof can appear in a conclusion. To evaluate the data of a proof-carrying value, project it first: `evaluate(count_up(200).0)`.

A failing case of `evaluate_all` is reported with the byte that refutes it. With `reflect`, a proved `forall (x: u8) { u8_le(x, 255) == true }` becomes a fact about the ordering of any byte.

The budget is 2,000,000 evaluation steps, counted in steps and never in time, so that acceptance does not depend on the machine. Exceeding it is an error distinct from refutation.

### Nat

`Nat` has the literals, `succ`, and `nat_add`. Its rules are Peano's, with addition:

| Axiom | Premises | Conclusion |
|---|---|---|
| `nat_add_zero(a)` | `a : Nat` | `a + 0 == a` |
| `nat_add_succ(a, b)` | `a, b : Nat` | `a + succ(b) == succ(a + b)` |
| `nat_succ_injective(a, b)` | `a, b : Nat` | `succ(a) == succ(b) => a == b` |
| `nat_succ_not_zero(a)` | `a : Nat` | `succ(a) == 0 => False` |

| Proof | Premises | Conclusion |
|---|---|---|
| `nat_induction(M, base, step, t)` | for a fresh ghost `x : Nat`, `M[x] : Prop`; `t : Nat`; `base` proves `M[0]`; `step` binds a fresh ghost `n : Nat` and the hypothesis `M[n]`, and proves `M[succ(n)]` | `M[t]` |

Literals connect to `succ` through `literal`: `succ(3n) == 4n`. Induction is the proof-level recursion rule. It has no runtime content. There is no term-level recursion over `Nat`.

The orderings are not primitive. The prelude defines them as ordinary math functions, and the kernel knows their identities only because the axioms below are stated with them:

~~~
nat_le(a, b) := exists k { a + k == b }
nat_lt(a, b) := nat_le(succ(a), b)
u8_le(a, b)  := nat_le(to_nat(a), to_nat(b))
u8_lt(a, b)  := nat_lt(to_nat(a), to_nat(b))
~~~

### The model of u8

A `u8` is a natural number below 256. `to_nat` is the inclusion and `of_nat` is reduction modulo 256. Six axioms say so, and each is true in that reading:

| Axiom | Premises | Conclusion |
|---|---|---|
| `to_nat_bound(x)` | `x : u8` | `nat_lt(to_nat(x), 256)` |
| `of_to_nat(x)` | `x : u8` | `of_nat(to_nat(x)) == x` |
| `to_of_nat(n)` | `n : Nat` | `nat_lt(n, 256) => to_nat(of_nat(n)) == n` |
| `of_nat_wrap(n)` | `n : Nat` | `of_nat(n + 256) == of_nat(n)` |
| `wrapping_add_model(a, b)` | `a, b : u8` | `wrapping_add(a, b) == of_nat(to_nat(a) + to_nat(b))` |
| `wrapping_sub_model(a, b)` | `a, b : u8` | `wrapping_add(wrapping_sub(a, b), b) == a` |

`to_of_nat` and `of_nat_wrap` together pin `of_nat` down as reduction modulo 256 without a `mod` operator, and `wrapping_sub` is characterized as the inverse of `wrapping_add` without subtraction on `Nat`. Injectivity of `to_nat` follows from `of_to_nat` by congruence.

Reflection connects a runtime comparison to the proposition it decides:

| Axiom | Premises | Conclusion |
|---|---|---|
| `reflect(c, true)` | `c` is `u8_eq(a, b)`, `u8_lt(a, b)`, or `u8_le(a, b)`, and `c : bool` | `c == true => P` |
| `reflect(c, false)` | the same | `c == false => (P => False)` |

where `P` is `a ==[u8] b`, `u8_lt(a, b)`, or `u8_le(a, b)` respectively. With `case_data` on the comparison, these give each branch of an `if` its fact, and the two together decide `P` without excluded middle. The converse directions are derivable from them.

All of these axioms need the prelude, because they are stated with `False` and the orderings.

`src/kernel/theory.rs` begins the kernel-level prelude: associativity of addition, `0 + a == a`, and `succ(a) + b == succ(a + b)` by induction; every natural is zero or a successor, which serves as case analysis on `Nat`; reflexivity, `0 <= a`, transitivity, and monotonicity of `succ` for `nat_le`; the corresponding facts about `u8_le`; and the two lemmas the specification's `bounded_walk` uses, `u8_le(i, limit) => (i == limit => False) => u8_lt(i, limit)` and `u8_lt(i, limit) => u8_le(wrapping_add(i, 1), limit)`. The second shows through the model of `wrapping_add` that the successor of a byte below another does not wrap. These are declared lemmas, checked by the kernel when they are declared, and not trusted. A fact about the ordering of three byte variables is obtained by reasoning about their models, never by enumerating bytes.

### Notes on the rules

Using a lemma needs no rule of its own: a call to a lemma is a term of proof type, so `of_term(lemma(args))` proves the instantiated conclusion. A lemma has no defining equation, because equality at its proof type cannot be formed; nothing is lost, since proofs are irrelevant.

`projection`, `literal`, `definition`, and `case_step` are the computation axioms of specification section 12.2 that exist so far; the `let` axiom is a hypothesis, as described under Context. Each is a single step validated by matching the shape of its term. They are used through `transport`, and the elaborator will insert them silently.

Projection from a literal product is typed by the product's own field values, which is the type the constructor rule checked field `i` against. So `v_i : A` always holds in `projection`, including for a nested dependent product whose inner type mentions an outer field.

`definition` is the only way a function body ever becomes visible. At type `Prop` it is how a predicate is unfolded, and at a function type it gives equations such as `select() == successor`. The call must name the declared function directly: a call through a variable or through another call has no defining equation until the callee has been rewritten to a function name.

`literal` evaluates natively: `u8::wrapping_add`, `u8::wrapping_sub`, `==`, `<`, `<=` on bytes; widening for `to_nat`; the low byte for `of_nat`; and arbitrary-precision addition for `succ` and `nat_add` (`src/kernel/nat.rs`), so those always have a step. This evaluation must agree with the model below, and `tests/kernel_numbers.rs` checks that it does.

`check(ctx, p, P)` requires `P : Prop`, infers the proposition `p` proves, and accepts when the two are the same term.

## Comparison

"The same term" means equal up to two things, and nothing else:

- renaming of bound variables, which the locally nameless representation makes structural equality;
- proof irrelevance: any two terms of the form `proof(p)` are the same, whatever `p` is.

The same relation on types compares the propositions inside `@P` and the fields of telescopes with it.

There is no unfolding, no evaluation, and no normalization. A hypothesis `is_three(n)` does not prove `n == 3`; transport along `definition(is_three(n))` does. `refl(2)` does not prove `wrapping_add(1, 1) == 2`; `literal(wrapping_add(1, 1))` does. `refl(3)` does not prove `(3, true).0 == 3`; `projection((3, true).0)` does.

Proof irrelevance is sound as a syntactic check because of the field rule: in a well-typed term, every position of proof type inside a product value holds a `proof(p)`, equality at a proof type cannot be formed, and no other term former has a proof-typed argument. Two product values with the same data and different proofs are therefore the same term, and `refl` proves them equal with no proof step.

Constructor disjointness and injectivity are not rules. `tests/kernel_cases.rs` derives `Red == Green => False` and `Byte(a) == Byte(b) => a == b` from `case_step`, `transport`, and a `case` that sends the constructors to different results.

## Derived forms

`src/kernel/derive.rs` builds proofs out of the rules above and is not trusted: `symm`, `trans`, `rewrite`, `unfold`, and `fold`. Each computes a transport template by abstracting occurrences of a term, and the kernel checks the result like any other proof. These are the elaborator forms of specification section 8.4. Their loops are bounded by a step count.

`unfold` and `fold` reach a call that mentions a bound variable, as in `forall x { is_three(x) }`, by going under `forall` and `exists` and into the conclusion of an implication, and rebuilding the binder around the rewritten body. `fold` follows the shape of its goal and unfolds an implication's premise on the way in. A call inside the arguments of a declared proposition, or under a binder within the premise of an implication, is not reached. `rewrite` needs no descent, because the term it replaces is closed. `Chain` builds an equational chain a link at a time.

## Depth bound

Checking, comparison, and substitution are recursive. Every public entry point (`check_type`, `infer_term`, `infer_proof`, `check_proof`, `Context::declare`, `assume`, `define`, and the declaration functions) first measures its input with an explicit work list, without recursion, and rejects input nested more than `MAX_DEPTH` = 256 levels deep, counting types, terms, and proofs together.

The number comes from measurement in an unoptimized build on a 2 MiB thread stack: nested arithmetic and nested quantifiers check at depth 800, and the worst shape found, a chain of transports, at 500 but not 600. To get there, the two judgments and the binder traversals are written as small dispatchers that call one function per rule or per variant; as single large matches, an unoptimized build reserved stack for every arm at once and overflowed near depth 150. A long equational argument should be a balanced tree of transitivity steps, or separate lemmas, not one chain.

The bound is on input. A term built by substitution during checking can be deeper than any input, by at most the input depth for each substitution a proof performs. Dropping a very deep term is recursive in Rust itself; the parser's own nesting limit keeps such terms from arising from source text.

## Invariants the implementation maintains

- Every proposition returned by proof inference is well formed in the context it was inferred in.
- Everything pushed while checking under a binder is removed before returning, on success and on failure.
- A fresh variable or hypothesis has an identity that occurs nowhere else, so generalizing over it cannot capture anything, and it cannot be referred to after its binder is left.
- Replacement terms used for `t[u]` are well formed in the context, hence contain no dangling index. For `forall_elim` this is established by typing the argument first.

## Deliberately absent

Symmetry, transitivity, and congruence of equality are not rules. They are derived from `refl` and `transport`, and `tests/kernel.rs` derives each of them.

All six kernel gates are implemented, and the gaps recorded when they were finished are closed. What remains, each noted where it arises above: `for_step` has no step for a body whose proofs depend on the particular upper bound; the derived forms do not reach inside a declared proposition's arguments or under a binder in an implication's premise; the depth bound covers input, not terms produced by substitution; and there is no transport between types, which nothing has needed so far because a dependent product can be rebuilt field by field.

One K2 acceptance condition is stated in the plan in terms of `NonZero`. When K2 was built the kernel had no `!=`, which needs `False` from K4, so the test uses a struct whose proof field is an equation, `a.wrapping_add(b) == 10`, to the same effect.

## Trusted base

`src/kernel/check.rs` (the rules and the comparison), `src/kernel/term.rs` (opening and closing binders, and telescope instantiation), `src/kernel/context.rs` (lookup and scoping), `src/kernel/defs.rs` (declarations and the prelude's definitions), the axioms of `Nat` and of the `u8` model, the native evaluation behind `literal` together with its agreement with that model, `src/kernel/nat.rs` (literal arithmetic), `src/kernel/eval.rs` (the evaluator), and `src/kernel/depth.rs` (the depth bound). `src/kernel/derive.rs`, `src/kernel/classical.rs`, and `src/kernel/theory.rs` are not part of it.

Agreement is tested exhaustively. For every pair of bytes, `tests/kernel_numbers.rs` has the kernel check that the native `wrapping_add` result equals `of_nat(to_nat(a) + to_nat(b))` evaluated step by step, that adding the subtrahend back to the native `wrapping_sub` result restores the minuend, and that whatever the native `<` answers, the matching fact about the models is provable. `of_nat` and `to_nat` are checked against their axioms on literals. The builder functions on `Term` and `Proof` that take closures are conveniences for constructing well-scoped terms; a term built any other way is checked just the same.

Known limit: checking is recursive and has no depth bound yet. Hand-written terms cannot exhaust the stack; this must be addressed before the kernel accepts terms produced from untrusted source text.
