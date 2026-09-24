+++
id = "kernel-contract"
title = "Kernel contract"
group = "Now"
created = "2026-09-21T19:19:39.000Z"
updated = "2026-09-23T04:48:55.000Z"
route = "reference/kernel.html"
order = 3
description = "The implemented logical kernel: syntax, checked rules, primitive meanings, and limits."
+++

# Kernel contract

<!-- spec: 2.0:1 informative -->
This document states every rule the proof kernel implements, with exact premises and conclusions. The kernel lives in `src/kernel/` and is independent of the parser. The code and this document change together: a rule is not in the kernel unless it is written here, and nothing here is in force until a kernel test exercises it.

<!-- spec: 2.0:2 informative -->
Read this as a technical reference alongside the [language manual](../spec/01-introduction.md). The representation and basic checking rules come first; later sections give logical comparisons, checked recursion, library quantifiers, physical snapshots, permissions and resource limits. Internal forms are not automatically source syntax: for example, the kernel retains native quantifier certificates and bounded range terms while source quantifiers use checked library predicates and source loops use the check IR. `tests/kernel_int.rs` checks that every implemented primitive, axiom and proof rule is named in this Markdown contract, using `Prim::name`, `Axiom::name` and `Proof::rule_name`. Focused suites cover each rule family. The historical implementation gates are listed separately below.

## Representation

<!-- spec: 2.1:1 informative -->
Types:

<!-- spec: 2.1:2 syntax -->
~~~text prose kernel-grammar
Type ::= bool | Prop
       | u8 | u16 | u32 | u64 | i8 | i16 | i32 | i64     the machine integer types
       | Int                      the integers of the logic
       | @P                       the proofs of the proposition P
       | (A_0, A_1, ..., A_n)     a telescope: A_i may mention fields 0..i-1
       | struct S                 a declared struct, by identity
       | enum E                   a declared enum, by identity
       | box_type(A)              immutable contents of a physical Box
       | buffer_type(A)           immutable contents of an array, slice, or Vec
       | fn(A_0, ..., A_n) -> R     the type of a function of the logic: parameters form a telescope; R may mention all of them
~~~

<!-- spec: 2.1:3 syntax -->
`Prop`, `@P` and `Int` are intrinsically erased types. Checked nominal declarations may also be marked logical, and a function type is erased when its result is. The kernel has one bool representation; source bool/Bool classification and nested physical layouts require the separate checked layout layer. Box and Buffer are physical containers even when their payloads erase. The eight machine integer types are data, with `u8` represented by Type::U8 and the other seven by Type::Machine(T); their integer observations use the primitive view schema. Kernel function types describe total logical terms, never ordinary source fn execution. Term dependencies in types occur inside propositions of proof types, so they may alter an evidence claim but not select a physical layout. In this contract, “ghost” is an internal erasure classification, not a source qualifier or a supported Ghost<T> type.

<!-- spec: 2.1:4 syntax -->
In a telescope, field `i` is under `i` binders: `#0` in it is field `i - 1`, `#1` is field `i - 2`, and so on. `A_i[v_0, ..., v_{i-1}]` below means field `i`'s type with each earlier field replaced by the given term.

<!-- spec: 2.1:5 syntax -->
A struct declaration is a closed telescope; an enum declaration is a list of payload telescopes. A function declaration supplies a closed dependent function type and a checked body. The basic declaration APIs accept references only to earlier declarations. Dedicated APIs additionally admit atomic recursive enum groups, strictly positive inductive predicates, and functions whose self-calls satisfy structural or nonnegative-Int descent. They publish no unchecked candidate declarations. The bounded For term is a separate total iteration rule. These are the supported recursion principles; ordinary runtime calls and unbounded source loops live outside the logical kernel. Structs and enums are nominal, while tuples are structural and discard source field names.

<!-- spec: 2.1:6 syntax -->
Terms. A proposition is a term of type `Prop`; there is no separate syntactic class.

<!-- spec: 2.1:7 syntax -->
~~~text prose kernel-grammar
t ::= x                     a context variable, by identity
    | #i                    a bound variable, by de Bruijn index
    | true | false | 0..255
    | 0u16, -1i8, ...       a literal of another machine integer type, within its range
    | 0i, 1i, -1i, ...      Int literals, of arbitrary size and either sign
    | int_add(t, t) | int_sub(t, t) | int_mul(t, t) | int_neg(t)
    | int_div(t, t) | int_rem(t, t)     quotient and remainder, truncated toward zero
    | int_le(t, t)          the order of Int, of type Prop
    | int_eq_b(t, t) | int_lt_b(t, t) | int_le_b(t, t)     logical Int comparisons, of type bool
    | view[T](t)            T -> Int: the value of a machine integer, for a machine type T
    | wrap[T](t)            Int -> T: reduction into the range of T
    | cast[S, T](t)         S -> T: what `as` between machine types compiles to
    | op[T](t, t) | op[T](t)     a row of the table of primitive operations at a machine type T: op is one of
                            add, sub, mul, div, rem, neg, wrapping_add, wrapping_sub, wrapping_mul, wrapping_neg
    | eq[T](t, t) | lt[T](t, t) | le[T](t, t)     the runtime comparisons at a machine type T, of type bool
    | t ==[A] t             equality at type A
    | t => t
    | forall (#: A) { t }   binds #0 in its body
    | (t_0, ..., t_n) : (A_0, ..., A_n)     a tuple value carries its telescope
    | S { t_0, ..., t_n }
    | t.i                   positional projection
    | proof(p)              a proof used as a value
    | f                     a declared function of the logic, as a value
    | lambda(A_0, ..., A_n) -> R { t }     dependent logical callable
    | t(t_0, ..., t_n)      application
    | boxed(t)              immutable Box payload snapshot
    | buffer[op, A](t_0, ..., t_n)     Literal, Length, Get, Set, or Push
    | E::i(t_0, ..., t_n)   variant i of a declared enum
    | case t : R { arm_0, ..., arm_n }     arm_i binds the payload of variant i
    | N(t_0, ..., t_n)      a declared proposition applied to arguments
    | exists (#: A) { t }   binds #0 in its body
    | absurd(p) : A         a value of any type from a proof of an empty proposition
    | for # in t..t (S = t) { t }     range iteration; see below
~~~

<!-- spec: 2.1:8 syntax -->
A tuple value carries its type because a dependent telescope cannot be inferred from the values alone.

<!-- spec: 2.1:9 syntax -->
Binding is locally nameless. Variables of the context carry globally unique identities that are never reused. Variables bound inside a term are indices. Two consequences are relied on throughout:

<!-- spec: 2.1:10 syntax -->
- Terms that differ only in the names of bound variables are equal as data.
- Substituting a term that is well formed in the context under a binder needs no shifting, because such a term contains no dangling index.

<!-- spec: 2.1:11 syntax -->
`t[u]` below means: replace the outermost bound variable of `t` by `u`. `close(t, x)` means: turn the context variable `x` into the outermost bound variable of `t`.

<!-- spec: 2.1:12 informative -->
Proofs:

<!-- spec: 2.1:13 syntax -->
~~~text prose kernel-grammar
p ::= h                     a context hypothesis, by identity; the rule is named hyp
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
    | case_known(t, p)      computation under a checked scrutinee equation
    | buffer_step(t)        a checked structural buffer equation
    | buffer_lower(t) | buffer_upper(t)     bounds on snapshot length
    | construct(N, i, params, payload)     variant i of a declared proposition
    | case_proof(p, G, arms)    case analysis on a proof
    | case_data(t, G, arms)     case analysis on data
    | exists_intro(P, t, p)
    | exists_elim(p, G, arm)
    | excluded_middle(P)
    | for_empty(t)          computation axiom
    | for_step(t, p, p)     computation axiom
    | evaluate(t)           big-step evaluation of a closed term
    | omitted               left by the evaluator; proves nothing
    | axiom(...)            an axiom of Int, of a machine integer type, of the table of primitive operations, or of the comparisons
    | data_induction(t, motives, arms)     induction over finite enum values
    | prop_induction(p, motive, arms)      induction over an inductive predicate
    | int_induction(M, p, arm, t)
    | linear(G, c, [(p, c), ...])     a certificate of linear arithmetic; each c is an Int literal
~~~

<!-- spec: 2.1:14 syntax -->
An arm of a proof-level case binds some term variables and some hypotheses, and states how many of each.

<!-- spec: 2.1:15 syntax -->
Hypotheses bound by `implies_intro` are indexed separately from term variables.

## Context

<!-- spec: 2.2:1 informative -->
A context is an ordered list of three kinds of entry:

<!-- spec: 2.2:2 syntax -->
~~~text prose kernel-grammar
x : A            an executable variable
ghost x : A      a ghost variable
h : P            a hypothesis, where P is a proposition well formed in the preceding context
~~~

<!-- spec: 2.2:3 normative -->
A variable whose type is a ghost type is ghost however it was declared. Declaring a variable requires its type to be well formed.

<!-- spec: 2.2:4 normative -->
An immutable logical binding `x = e` introduces a typed variable and, when its type admits equality, the defining hypothesis `x ==[A] e`. Context::define validates this context extension; it is not an additional proof rule. The same equality-based reasoning can be derived inside a closed statement by instantiating `forall x, x == e => P(x)` at e and discharging the premise with refl(e). Proof-valued bindings instead carry their established proposition; equality between proof objects is forbidden. `tests/kernel_products.rs` exercises the dependent-product and context rules.

<!-- spec: 2.2:5 normative -->
Terms are typed in one of two modes. `Logical` is the upgraded reading of the context: ghost variables are ordinary variables. `Executable` means the term's value is required at runtime.

## Type formation: `ctx |- A type`

<!-- spec: 2.3:1 syntax -->
| Type | Premises |
|---|---|
| `bool`, `u8`, `Int`, `Prop` | none |
| `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64` | none; the form `Type::Machine(U8)` is rejected, because `u8` is `Type::U8` |
| `@P` | `P : Prop` in `Logical` mode |
| `(A_0, ..., A_n)` | for each `i`, with fresh ghost variables `x_0 : A_0, ..., x_{i-1} : A_{i-1}[...]` added, `A_i[x_0, ..., x_{i-1}]` is a type |
| `struct S` | `S` is declared |
| `enum E` | `E` is declared |
| `box_type(A)`, `buffer_type(A)` | `A` is a well-formed type; physical layouts and permissions are checked separately |
| `fn(A_0, ..., A_n) -> R` | `(A_0, ..., A_n, R)` is a well-formed telescope |

## Term typing: `ctx |- t : A` in a mode

<!-- spec: 2.4:1 legality-rule -->
| Term | Premises | Type |
|---|---|---|
| `x` | `x : A` or `ghost x : A` is in `ctx`. In `Executable` mode, `x` is not ghost. | `A` |
| `#i` | none: a bound index outside any binder is rejected | |
| `true`, `false` | | `bool` |
| `0..255` | | `u8` |
| a literal `n` of another machine type `T` | `min(T) <= n <= max(T)`, from the table under The machine integer types; the form `Term::Machine(U8, n)` is rejected | `T` |
| `0i, 1i, -1i, ...` | mode is `Logical` | `Int` |
| `int_add(a, b)`, `int_sub(a, b)`, `int_mul(a, b)` | mode is `Logical`; exactly two arguments; `a : Int` and `b : Int` | `Int` |
| `int_div(a, b)`, `int_rem(a, b)` | mode is `Logical`; exactly two arguments; `a : Int` and `b : Int`; no condition on `b` | `Int` |
| `int_neg(a)` | mode is `Logical`; exactly one argument; `a : Int` | `Int` |
| `int_le(a, b)` | mode is `Logical`; exactly two arguments; `a : Int` and `b : Int` | `Prop` |
| `view[T](x)` | mode is `Logical`; exactly one argument; `x : T`, for a machine type `T` | `Int` |
| `wrap[T](n)` | exactly one argument; `n : Int`, which is only possible in `Logical` mode | `T` |
| `cast[S, T](x)` | exactly one argument; `x : S` in the same mode, for machine types `S` and `T` | `T` |
| `op[T](a, b)`, `op[T](a)` | `op[T]` is a row of the table under The primitive operations, so `neg` and `wrapping_neg` need a signed `T`; exactly as many arguments as the row's arity, two, or one for the two negations; each `a : T` in the same mode | `T` |
| `eq[T](a, b)`, `lt[T](a, b)`, `le[T](a, b)` | exactly two arguments; `a : T` and `b : T` in the same mode, for a machine type `T` | `bool` |
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
| `f` | `f` is declared with type `F`. In `Executable` mode `F` is not ghost and `f` is executable, as defined under function declaration below. | `F` |
| `t(t_0, ..., t_n)` | `t : fn(A_0, ..., A_n) -> R` in the same mode; the field rule below holds for the arguments against the parameters. In `Executable` mode the result type is not ghost. | `R[t_0, ..., t_n]` |

<!-- spec: 2.4:2 legality-rule -->
In an arm's fact, a payload variable `y` standing in a proof field appears as `proof(of_term(y))`, so that the fact is itself well formed under the field rule. Each arm's hypothesis is the same fact `case_data` gives its arms, so a branch of a logical function knows what a branch of executable code knows, and a proof field in an arm can use it. It is a hypothesis, so it can occur only inside proofs, and it has no runtime content.

<!-- spec: 2.4:3 legality-rule -->
In an executable `case`, a payload variable is executable when its field has a runtime representation and ghost otherwise; in a logical `case` every payload variable is ghost. The result type of a `case` does not depend on the scrutinee. A `case` cannot scrutinize a proof and cannot have a proof type as its result: case analysis that inspects or produces proofs is a proof rule, below. `absurd` is the match with no arms used for its value; it marks a point that is never reached.

<!-- spec: 2.4:4 legality-rule -->
Function declaration. `fn f(x_0: A_0, ..., x_n: A_n) -> R { body }`, a function of the logic, is accepted when its function type is well formed with no variables in scope, and, with fresh ghost variables for the parameters, `body : R[x_0, ..., x_n]` in `Logical` mode. A lemma is a function whose result type is a proof type; its body has the form `proof(p)`.

<!-- spec: 2.4:5 legality-rule -->
The basic declaration API initially classifies a checked total function as executable only if its result is not erased and its body also checks in Executable mode, with erased parameters unavailable to executable computation. A signature alone cannot decide this: the kernel-only definition `fn narrow(n: Int) -> u8 { wrap[u8](n) }` checks logically but cannot produce runtime bytes. Calling a logical-only definition makes a body logical-only as well. Definitions::restrict_to_logic may remove executable permission from an accepted declaration, never add it; source logic fn uses this restriction even for a Bool result whose kernel representation is bool. Definitions::is_executable reports the resulting classification. This internal permission is distinct from the source fn/logic fn distinction.

<!-- spec: 2.4:6 legality-rule -->
The field rule. It applies to the fields of a product value and to the arguments of a call. There are exactly as many values as fields. For each `i` in order, let `E = A_i[t_0, ..., t_{i-1}]`:

<!-- spec: 2.4:7 legality-rule -->
- if `E` is `@P`, then `t_i` must have the form `proof(p)` and `p` must prove `P`;
- if `E` is another ghost type, `t_i : E` in `Logical` mode;
- otherwise `t_i : E` in the product's own mode.

<!-- spec: 2.4:8 legality-rule -->
So a ghost field is a logical position even inside an executable product: a ghost variable may be mentioned by a proof field's proposition, or stored in a `Prop` field, but not stored in a data field of executable data.

<!-- spec: 2.4:9 legality-rule -->
Requiring the literal form `proof(p)` in every proof field is what makes proof irrelevance a syntactic check. A projection or variable of proof type is used in such a position as `proof(of_term(t))`.

<!-- spec: 2.4:10 legality-rule -->
A proposition former, a `proof(p)`, or a projection of a ghost field in `Executable` mode is rejected: a term of ghost type has no runtime value. Inside a proposition former the mode is always `Logical`, as specified by the context modes above.

<!-- spec: 2.4:11 legality-rule -->
Equality may be formed at any type except a proof type, including `Prop`, product types, and function types. There is no extensionality rule: nothing concludes `f == g` from `forall x, f(x) == g(x)`, and nothing concludes `P == Q` from `P => Q` and `Q => P`.

## Proof checking: `ctx |- p proves P`

<!-- spec: 2.5:1 legality-rule -->
The kernel reads the proposition off the proof. It never searches.

<!-- spec: 2.5:2 legality-rule -->
| Proof | Premises | Conclusion |
|---|---|---|
| `h` (the rule `hyp`) | `h : P` is in `ctx` | `P` |
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

<!-- spec: 2.6:1 legality-rule -->
`prop N(x_0: A_0, ..., x_n: A_n) { variants }` is accepted when each `A_j` is a type that is not a proof type, and each variant has one of two shapes:

<!-- spec: 2.6:2 legality-rule -->
- A variant without a stated conclusion has a payload telescope that may mention the parameters. It proves `N` at whatever parameters it is given.
- A variant with a stated conclusion has a closed payload telescope `(B_0, ..., B_m)` and arguments `c_0, ..., c_n` under it, with each `c_j : A_j`. It proves exactly `N(c_0, ..., c_n)`. The parameters are not in scope in it.

<!-- spec: 2.6:3 legality-rule -->
A parameter cannot be a proof, so no parameter type depends on an earlier parameter, and index equations are independent. A conclusion is a list of arguments for `N`, not a proposition, so a variant cannot conclude anything but `N`; the representation enforces that rule. This basic declaration API does not give `N` an identity until acceptance, so its payloads cannot mention `N`. Recursive predicates use the separately checked inductive declaration API described below, with strict positivity and induction rules.

<!-- spec: 2.6:4 legality-rule -->
| Proof | Premises | Conclusion |
|---|---|---|
| `construct(N, i, (t_0..t_n), payload)`, variant `i` without a stated conclusion | the field rule holds for `(t_0, ..., t_n, payload...)` against the parameters followed by the payload telescope | `N(t_0, ..., t_n)` |
| `construct(N, i, (), payload)`, variant `i` with a stated conclusion | the field rule holds for the payload | `N(c_0[payload], ..., c_n[payload])` |
| `case_proof(p, G, arms)` | `p` proves `N(a_0, ..., a_n)`; `G : Prop` in the current context; one arm per variant, in order; each arm proves `G` under the bindings below | `G` |
| `case_data(t, G, arms)` | `t : bool` or `t : enum E` in `Logical` mode, of type `T`; `G : Prop` in the current context; one arm per variant; arm `i` binds fresh ghost payload variables `ys` and the hypothesis `t ==[T] variant_i(ys)`, and proves `G` | `G` |

<!-- spec: 2.6:5 informative -->
What an arm of `case_proof` binds:

<!-- spec: 2.6:6 legality-rule -->
- for a variant without a stated conclusion: payload variables whose types are the payload telescope with the parameters replaced by `a_0, ..., a_n`, and no hypotheses;
- for a variant with a stated conclusion: payload variables `ys`, then one hypothesis per parameter, `a_j ==[A_j] c_j[ys]`. These are the index equations. There is no equation about the scrutinee itself: under proof irrelevance it would say nothing.

<!-- spec: 2.6:7 legality-rule -->
`G` is checked before anything is bound, so it cannot mention a payload variable, and it does not depend on the proof being analyzed. A proposition with no variants has a `case_proof` with no arms, which proves any `G`.

<!-- spec: 2.6:8 legality-rule -->
The result of `case_proof` is a proof by construction, and every arm is a kernel proof, hence total. The specification's restrictions on a match over a proof, that its result is a proof and that its arms are total logical computations, therefore cannot be violated in kernel terms; rejecting surface programs that try is the elaborator's job.

<!-- spec: 2.6:9 legality-rule -->
The prelude declares `True`, `False`, `And`, and `Or` by this mechanism, before anything else, and records their identities. `!P` is `P => False`.

### Exists

<!-- spec: 2.7:1 legality-rule -->
| Proof | Premises | Conclusion |
|---|---|---|
| `exists_intro(P, t, p)` | `P` is `exists (#: A) { B }` and `P : Prop`; `t : A` in `Logical` mode; `p` proves `B[t]` | `P` |
| `exists_elim(p, G, arm)` | `p` proves `exists (#: A) { B }`; `G : Prop` in the current context; the arm binds a fresh ghost `w : A` and the hypothesis `B[w]`, and proves `G` | `G` |

<!-- spec: 2.7:2 legality-rule -->
`G` cannot mention the witness, and the result is a proof: an existential cannot be mined for data.

### Excluded middle

<!-- spec: 2.8:1 legality-rule -->
| Proof | Premises | Conclusion |
|---|---|---|
| `excluded_middle(P)` | the declarations include the prelude; `P : Prop` | `Or(P, P => False)` |

<!-- spec: 2.8:2 legality-rule -->
`proof_is_classical` reports whether a proof uses this rule, directly or through a declared function, and each function declaration records the same about its body. This is bookkeeping for auditing, not a check.

### Range iteration

<!-- spec: 2.9:1 legality-rule -->
`for i in lo..hi (s : S(i) = init) { body }` is the term-level recursion rule. The bounds and the index have one machine integer type `T`, read off `lo`, and the rule is stated over the model of `T` in `Int`: `v(x)` below is `view[T](x)`, `<=` is `int_le`, `<` is `int_lt`, and `succ(i)` is the row `wrapping_add[T](i, 1)` at the literal `1` of `T`. `S(i)` is a tuple telescope under one binder, the index. `body` binds the index `i`, the current state `s`, and two hypotheses.

<!-- spec: 2.9:2 legality-rule -->
| Term | Premises | Type |
|---|---|---|
| `for i in lo..hi (s = init) { body }` | the declarations include the prelude; `lo : T` for a machine type `T`, and `hi : T`, in the same mode; the attached proof proves `v(lo) <= v(hi)`; `init : S(lo)` in the same mode; for a fresh `i : T`, `S(i)` is a type; with `i : T`, `s : S(i)`, and the hypotheses `v(lo) <= v(i)` and `v(i) < v(hi)` added, `body : S(succ(i))` in the same mode | `S(hi)` |

<!-- spec: 2.9:3 legality-rule -->
In Executable mode the loop index and state are executable variables; in Logical mode they are ghost. Erased state fields remain logical positions in either mode. A bound of another machine type is rejected. This rule is for the kernel’s bounded For term, not the source language’s ordinary for loop. Source loops lower to exec::ForStmt, permit reversed ranges as empty, and are not callable inside propositions; see the [formal core](formal-core.md#spec-3.2:1).

<!-- spec: 2.9:4 legality-rule -->
The index may change what the state's proofs say and never what data the state holds, because a term occurs in a type only inside `@P`. So the erasure of `S(i)` does not depend on `i`, and a loop whose invariant varies with the index erases to a plain loop over one fixed state type. That is what makes this rule erasable, and it holds by construction.

<!-- spec: 2.9:5 legality-rule -->
Ordered bounds make the final index `hi`, so the result type needs no case distinction. A reversed range is not a type error in itself; it is rejected because no proof of `v(lo) <= v(hi)` can be supplied. Since `v(i) < v(hi)`, the successor in the body's type never wraps: `op_model` and `view_wrap` give `v(succ(i)) == v(i) + 1` under the range facts, which is how the lemma `succ_le_of_lt` below proves it. A half-open range cannot have `max(T)` as an index.

<!-- spec: 2.9:6 legality-rule -->
Two loops that differ only in the proof that their bounds are ordered are the same term.

<!-- spec: 2.9:7 legality-rule -->
| Proof | Premises | Conclusion |
|---|---|---|
| `for_empty(f)` | `f` is a `for` whose `lo` and `hi` are the same term, and is well typed | `f ==[S(lo)] init` |

<!-- spec: 2.9:8 legality-rule -->
| `for_step(f, lower, upper)` | `f` is a well-typed `for` over `lo..succ(h)`, with the successor written exactly so, `wrapping_add[T](h, 1)` at the literal `1` of the index type; `lower` proves `v(lo) <= v(h)`; `upper` proves `v(h) < v(succ(h))`; the unrolled right side below is well typed at the type of `f` | `f ==[S(succ(h))] body[h, g, lower, upper]`, where `g` is the same `for` over `lo..h` with `lower` as its ordering proof |

<!-- spec: 2.9:9 legality-rule -->
Stating the successor case with the bound in successor form is what makes it work for index-dependent state: the body at index `h` has type `S(succ(h))`, which is the type of `f`, so no transport between types is needed. `upper` says the successor does not wrap. The loop `g` reuses the body under the hypothesis `i < h`; a body whose proofs depend on the particular upper bound does not type-check there, and then there is no step. `for_empty` and `for_step` compute a loop symbolically; on literals, `evaluate` below is the practical route, because a literal bound cannot be rewritten into successor form inside a `for` that carries a proof about it.

<!-- spec: 2.9:10 legality-rule -->
Soundness of the typing rule is by induction on `hi - lo` in the intended model. It is a separate rule from `int_induction` on purpose: one recursion rule computes and erases to a loop, the other only proves and is erased. Since M3 no surface loop lowers to this term; the checker of the check IR has a bounded `for` statement of its own, and the term remains a rule of the kernel exercised by `tests/kernel_loops.rs`.

### Evaluation

<!-- spec: 2.10:1 legality-rule -->
| Proof | Premises | Conclusion |
|---|---|---|
| `evaluate(t)` | `t : A` in `Logical` mode; `A` is plain data: `bool`, a machine integer type, `Int`, or a tuple, struct, or enum built only from those; `t` has no free variable; evaluation finishes within the step budget with value `v` | `t ==[A] v` |
| `evaluate(c)`, where `c` is literally `int_le(a, b)` or `a ==[Int] b` | `c : Prop` in `Logical` mode; `a` and `b` have no free variable and evaluate, within one step budget for the two, to the literals `m` and `n` | `c` when `m <= n`, respectively `m == n`, holds of the two numbers; `c => False` when it does not, which needs the prelude |

<!-- spec: 2.10:2 legality-rule -->
Evaluation is big-step and call-by-value: primitives by native evaluation, a call by instantiating the function's body, `case` by choosing the arm of the evaluated scrutinee, projection from the evaluated product, and `for` by running the body for each index from `lo` to `hi`, the two bounds being literals of one machine type. It is a shortcut for a chain of the computation axioms and is the one place the kernel computes. It takes no part in comparing terms. On `Int` the primitives are computed with `kernel::Integer` (`src/kernel/int.rs`) and with nothing else, so nothing overflows.

<!-- spec: 2.10:3 legality-rule -->
Proofs are never evaluated. The evaluator replaces each `proof(p)` it meets by `proof(omitted)`, so a loop state that carries proofs about the previous state does not grow. `omitted` proves nothing: checking it is an error. Because `evaluate` offers only plain data, no omitted proof can appear in a conclusion. To evaluate the data of a proof-carrying value, project it first: `evaluate(count_up(200).0)`.

<!-- spec: 2.10:4 legality-rule -->
A comparison of integers is a proposition, so it has no value for `evaluate` to state an equation with. The second row decides it instead: both sides are evaluated, and the rule proves the comparison when it holds of the two numbers and its negation when it does not. `int_lt` is an abbreviation of `int_le`, so it is decided by the same row. No other proposition is decided: `evaluate(1 ==[u8] 1)` is still rejected as not plain data, and so is an equation at any machine type; `view` takes such a fact into `Int`. Inside a value, `int_le(a, b)` is an opaque proposition like any other and is not evaluated.

<!-- spec: 2.10:5 legality-rule -->
Evaluation has two budgets, both counted and neither timed, so that acceptance does not depend on the machine: 2,000,000 steps, and a nesting depth of 200. Exceeding either is an error distinct from refutation. The depth budget exists because a call evaluates the callee's body: a chain of a thousand small functions, each calling the next, is shallow as input and a thousand levels deep to evaluate, which would otherwise exhaust the stack long before the step budget. Iterations of a `for` run one after another, so a long loop costs steps and not depth. `int_mul` is the one primitive whose result can be twice the size of its operands, so a term of a hundred nodes that squares a number again and again names a number of `2^100` bits. A multiplication is therefore charged what the schoolbook product costs, one step for each pair of 32-bit digits of its operands, before it is carried out. That bounds the size of every number evaluation can reach, and the work done on it, by the step budget; the other primitives add at most one bit for each step. `int_div` and `int_rem` make nothing larger, but the long division in `src/kernel/nat.rs` is bit by bit: for each bit of the dividend it doubles a remainder and subtracts the divisor from it when it can, and each of those touches every digit of the divisor. A quotient or a remainder is therefore charged one step for each bit of the dividend times each 32-bit digit of the divisor, again before it is carried out. A division whose charge alone exceeds the budget is refused without doing any of its work. Measured in an unoptimized build on a 2 MiB thread stack, evaluation alone nests 500 levels in every shape tried and overflows at 700 in the worst; the bound is well under half of that because evaluation can begin at the bottom of a proof that is itself nested up to the input depth bound, and the two share one stack. A test does exactly that.

### Int

<!-- spec: 2.11:1 normative -->
`Int` is the integers of the logic. It enters the kernel by axioms and native evaluation, not by a construction from natural numbers. `kernel::Natural` in `src/kernel/nat.rs` supplies the magnitude of `kernel::Integer`; it is an implementation type, distinct from the userland logical `Nat` enum. `Int` has literals of any size and either sign, the primitives `int_add`, `int_sub`, `int_mul`, `int_neg`, `int_div`, and `int_rem`, and one primitive order proposition, `int_le(a, b)`. Boolean-valued comparisons are described under Logical Boolean comparisons. Below, `a + b`, `a - b`, `a * b`, `-a`, `a / b`, `a % b`, and `a <= b` stand for the integer operations and order proposition; a literal is written without its `i`, and `==` is `==[Int]`.

<!-- spec: 2.11:2 normative -->
The strict order is not primitive and is not a definition to unfold either. `int_lt(a, b)` is the term `int_le(int_add(a, 1i), b)`, built by `Term::int_lt`, so `a < b` and `a + 1 <= b` are the same term and each proves the other with no step. `>=` and `>` are the same two with the sides exchanged.

<!-- spec: 2.11:3 informative -->
The ring axioms say that `Int` is a commutative ring:

<!-- spec: 2.11:4 normative -->
| Axiom | Premises | Conclusion |
|---|---|---|
| `int_add_assoc(a, b, c)` | `a, b, c : Int` | `(a + b) + c == a + (b + c)` |
| `int_add_comm(a, b)` | `a, b : Int` | `a + b == b + a` |
| `int_add_zero(a)` | `a : Int` | `a + 0 == a` |
| `int_add_neg(a)` | `a : Int` | `a + (-a) == 0` |
| `int_sub_def(a, b)` | `a, b : Int` | `a - b == a + (-b)` |
| `int_mul_assoc(a, b, c)` | `a, b, c : Int` | `(a * b) * c == a * (b * c)` |
| `int_mul_comm(a, b)` | `a, b : Int` | `a * b == b * a` |
| `int_mul_one(a)` | `a : Int` | `a * 1 == a` |
| `int_mul_add(a, b, c)` | `a, b, c : Int` | `a * (b + c) == a * b + a * c` |

<!-- spec: 2.11:5 informative -->
The order axioms say that the ring is totally and discretely ordered:

<!-- spec: 2.11:6 normative -->
| Axiom | Premises | Conclusion |
|---|---|---|
| `int_le_refl(a)` | `a : Int` | `a <= a` |
| `int_le_trans(a, b, c)` | `a, b, c : Int` | `a <= b => (b <= c => a <= c)` |
| `int_le_antisymm(a, b)` | `a, b : Int` | `a <= b => (b <= a => a == b)` |
| `int_le_add(a, b, c)` | `a, b, c : Int` | `a <= b => a + c <= b + c` |
| `int_le_mul(a, b)` | `a, b : Int` | `0 <= a => (0 <= b => 0 <= a * b)` |
| `int_le_total(a, b)` | `a, b : Int` | `Or(a <= b, b + 1 <= a)` |
| `int_lt_irrefl(a)` | `a : Int` | `a + 1 <= a => False` |

<!-- spec: 2.11:7 normative -->
Every premise is checked in `Logical` mode, and every axiom needs the prelude. An axiom is an instance at the given terms, which may be any terms of type `Int` in the context; the general statement is obtained with `forall_intro`.

<!-- spec: 2.11:8 normative -->
An ordered ring need not be the integers, because its order need not be discrete. Here it is: `int_le_total` gives `b + 1 <= a`, which is `b < a`, in its second case, where the total order of a ring would give only `b <= a`. With `int_lt_irrefl` it follows that nothing lies strictly between `n` and `n + 1`. `tests/kernel_int.rs` derives `n + 1 <= m => (m + 1 <= n + 1 => False)` from `int_le_add`, `int_le_trans`, and `int_lt_irrefl`, and also `a <= a + 1`, `a <= b => a < b + 1`, `a <= b => (b < c => a < c)`, and `a + c <= b + c => a <= b`, from the axioms alone. `0 <= 1` is not an axiom: `evaluate(int_le(0i, 1i))` proves it.

<!-- spec: 2.11:9 normative -->
| Proof | Premises | Conclusion |
|---|---|---|
| `int_induction(M, base, step, t)` | for a fresh ghost `x : Int`, `M[x] : Prop`; `t : Int`; `base` proves `M[0i]`; `step` binds a fresh ghost `n : Int` and two hypotheses, `int_le(0i, n)` and then `M[n]`, and proves `M[int_add(n, 1i)]` | `int_le(0i, t) => M[t]` |

<!-- spec: 2.11:10 normative -->
Induction is over the non-negative integers, which are the ones reached from `0` by adding `1`, so the conclusion carries the premise `0 <= t`. The successor in the step is written exactly `int_add(n, 1i)`. The step binds one variable and two hypotheses. It is what makes the axioms describe the integers and not some larger discretely ordered ring, and it has no runtime content.

<!-- spec: 2.11:11 normative -->
Quotient and remainder. `a / b` is the quotient truncated toward zero and `a % b` the remainder that goes with it, exactly Rust's `/` and `%` on the signed integer types, with no overflow because nothing overflows here. Both are total: `a / 0 == 0` and `a % 0 == a`. Eight axioms fix them. In the table `a < b` is the term `a + 1 <= b`, as everywhere on `Int`, so each conclusion is a single `int_le` or a single equation, usable as a linear fact as it stands:

<!-- spec: 2.11:12 normative -->
| Axiom | Premises | Conclusion |
|---|---|---|
| `int_div_rem(a, b)` | `a, b : Int` | `a == (a / b) * b + a % b` |
| `int_div_zero(a)` | `a : Int` | `a / 0 == 0` |
| `int_rem_lower_pos(a, b)` | `a, b : Int` | `0 < b => -b < a % b` |
| `int_rem_upper_pos(a, b)` | `a, b : Int` | `0 < b => a % b < b` |
| `int_rem_lower_neg(a, b)` | `a, b : Int` | `b < 0 => b < a % b` |
| `int_rem_upper_neg(a, b)` | `a, b : Int` | `b < 0 => a % b < -b` |
| `int_rem_nonneg(a, b)` | `a, b : Int` | `0 <= a => 0 <= a % b` |
| `int_rem_nonpos(a, b)` | `a, b : Int` | `a <= 0 => a % b <= 0` |

<!-- spec: 2.11:13 normative -->
The decomposition is stated in exactly one orientation, with `a` alone on the left, the quotient's product before the remainder, and the quotient before the divisor in the product: `a == int_add(int_mul(int_div(a, b), b), int_rem(a, b))`. Any other arrangement is a different term, reached from this one by the ring axioms.

<!-- spec: 2.11:14 normative -->
Each axiom is true of truncating division at every `a` and every `b`, a zero divisor included. For `b != 0`, write `q = a / b` and `r = a % b`: truncation toward zero means `|q|` is `|a| / |b|` rounded down and `q` has the sign of `a * b`, so `r = a - q * b` has the sign of `a`, or is zero, and satisfies `|r| < |b|`. The decomposition is `r`'s definition; the four bounds say `|r| < |b|` split by the sign of `b`, since for `0 < b` the bound is `-b < r < b` and for `b < 0` it is `b < r < -b`; and the two sign axioms say `r` has the sign of `a`. At `b == 0` the unconditional axioms hold as well: `int_div_rem` reads `a == 0 * 0 + a`, which is true because `a / 0 == 0` and `a % 0 == a`; `int_div_zero` is the definition of `a / 0`; and `int_rem_nonneg` and `int_rem_nonpos` say that `a % 0`, which is `a`, has the sign of `a`. The four bounds are false at `b == 0` without their conditions, since `a % 0 == a` is bounded by nothing, which is why they carry them: `0 < b` and `b < 0` exclude zero exactly. The bounds are strict, and `0 <= b` in place of `0 < b` would let `b == 0` in, so `tests/kernel_int_div.rs` rejects each of these near misses and refutes the unconditional bounds at `b == 0` by evaluation.

<!-- spec: 2.11:15 normative -->
For `b != 0` the axioms determine `q` and `r` uniquely, so they pin down truncating division and not flooring or Euclidean division. If `a == q * b + r == q' * b + r'` with both remainders bounded by `|b|` in magnitude and both of the sign of `a`, then `r - r'` is a multiple of `b` whose magnitude is below `|b|`, because two numbers of one sign and each below `|b|` in magnitude differ by less than `|b|`; so it is zero, and then `q == q'` because `b != 0`. Flooring division, `-7 / 2 == -4` with remainder `1`, breaks `int_rem_nonpos`, and Euclidean division, `-7 / -2 == 4` with remainder `1`, breaks it too. `tests/kernel_int_div.rs` has evaluation refute both and prove the truncating values in each of the four sign combinations: `-7 / 2 == -3` and `-7 % 2 == -1`, `7 / -2 == -3` and `7 % -2 == 1`, `-7 / -2 == 3` and `-7 % -2 == -1`, and `7 / 2 == 3` and `7 % 2 == 1`.

<!-- spec: 2.11:16 normative -->
`a % 0 == a` is not an axiom. It follows from the two unconditional axioms and the ring: `int_div_rem(a, 0)` gives `a == (a / 0) * 0 + a % 0`, `int_div_zero` turns `a / 0` into `0`, `0 * 0 == 0` comes from the ring axioms, and `0 + a % 0 == a % 0` from `int_add_comm` and `int_add_zero`. `tests/kernel_int_div.rs` derives it inside the kernel in thirty-three nodes, twenty-one of them the ring derivation of `x * 0 == 0`.

<!-- spec: 2.11:17 normative -->
Computation. `literal` computes one primitive on literals, as `int_add(2i, 3i) == 5i` and `int_div(-7i, 2i) == -3i`; `int_le` is a proposition and has no `literal` step. `evaluate` computes a closed term of type `Int` to a literal, and decides a closed `int_le(a, b)` or `a ==[Int] b`, as described under Evaluation. `int_div` and `int_rem` are computed by `Integer::div` and `Integer::rem` in `src/kernel/int.rs`, which truncate toward zero and give `0` and `a` at a zero divisor, and by nothing else; `tests/kernel_int_div.rs` checks them against Rust's `/` and `%` on random `i128` pairs with a nonzero divisor, against the formulas at a zero divisor, and on the one pair where Rust overflows, `i128::MIN / -1`, checks that the kernel gives `2^127`. A quotient or a remainder is charged steps in proportion to its operands, as described under Evaluation. There is no other connection between the literals and the axioms: that `2 + 3` is `5` is known by computing, not from the ring laws.

<!-- spec: 2.11:18 normative -->
A proof from these axioms alone takes one transport for each rearrangement, as the derived facts in `tests/kernel_int.rs` show. The rule `linear`, under Linear arithmetic below, checks a certificate of linear arithmetic instead, and takes instances of the bounds and sign axioms above as its constraints, which is why each of them concludes a single comparison rather than a conjunction.

### The machine integer types

<!-- spec: 2.12:1 normative -->
The machine integer types are `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, and `i64`. They are runtime data. Each is modelled over `Int` by the same three primitives and six axioms, instantiated at the type, and compared at runtime by the same three comparisons and one more axiom, under Comparisons below. This is the one model of a machine type the kernel has, and `u8` is a case of it like any other. The kernel holds one table, `MachineInt` in `src/kernel/machine.rs`, and nothing else is per type:

<!-- spec: 2.12:2 normative -->
| Type | `bits` | `min` | `max` |
|---|---|---|---|
| `u8` | 8 | `0` | `255` |
| `u16` | 16 | `0` | `65535` |
| `u32` | 32 | `0` | `4294967295` |
| `u64` | 64 | `0` | `18446744073709551615` |
| `i8` | 8 | `-128` | `127` |
| `i16` | 16 | `-32768` | `32767` |
| `i32` | 32 | `-2147483648` | `2147483647` |
| `i64` | 64 | `-9223372036854775808` | `9223372036854775807` |

<!-- spec: 2.12:3 normative -->
An unsigned type has `min = 0` and `max = 2^bits - 1`; a signed one has `min = -2^(bits - 1)` and `max = 2^(bits - 1) - 1`. Either way the range holds exactly `2^bits` consecutive integers. In the kernel `u8` is `Type::U8` with literals `Term::U8`, as before; the other seven are `Type::Machine(T)` with literals `Term::Machine(T, n)`, and `Type::machine` and `Term::machine` build the right form for any `T`. `Type::Machine(U8)` and `Term::Machine(U8, n)` are rejected wherever a type or a term is checked, so that `u8` has one spelling. A literal `Term::Machine(T, n)` is a term only when `min(T) <= n <= max(T)`; one built outside the range is rejected by typing wherever it occurs, inside a primitive, an equation, or an axiom included, and native evaluation gives it no value, so `literal` has no step on it either. Literals of two types are never the same term.

<!-- spec: 2.12:4 normative -->
view[T] has type T -> Int and is logical-only; it represents a source model observation such as x as Int. A source ordering uses the logical Bool comparison of these observations lifted through holds; cmp_reflect connects it to int_le or its strict abbreviation. wrap[T] has type Int -> T and denotes modular reduction inside the kernel’s machine model. It has no executable use and does not authorize an Int-to-machine cast in source: the source checker rejects that extraction. cast[S,T] has type S -> T and implements machine-to-machine source as in either mode, matching Rust’s truncation and extension semantics. Each primitive carries its machine types; Prim::name omits those parameters and the stored text prints them in brackets.

<!-- spec: 2.12:5 normative -->
The schema. For each type `T`, with `min(T)`, `max(T)`, and `2^bits` written as `Int` literals, `<=` being `int_le` and `+` being `int_add`:

<!-- spec: 2.12:6 normative -->
| Axiom | Premises | Conclusion |
|---|---|---|
| `view_lower[T](x)` | `x : T` | `min(T) <= view[T](x)` |
| `view_upper[T](x)` | `x : T` | `view[T](x) <= max(T)` |
| `wrap_view[T](x)` | `x : T` | `wrap[T](view[T](x)) ==[T] x` |
| `view_wrap[T](n)` | `n : Int` | `min(T) <= n => (n <= max(T) => view[T](wrap[T](n)) ==[Int] n)` |
| `wrap_period[T](n)` | `n : Int` | `wrap[T](n + 2^bits) ==[T] wrap[T](n)` |
| `cast_def[S, T](x)` | `x : S` | `cast[S, T](x) ==[T] wrap[T](view[S](x))` |

<!-- spec: 2.12:7 normative -->
Each is an instance at a term of the context, checked in `Logical` mode, and needs the prelude as every axiom does. `Axiom::name` gives the name without the type; the type, or the pair of types, is carried by the axiom, as `Axiom::ViewLower(T, x)` and `Axiom::CastDef(S, T, x)`. The bounds and the period are the numbers of the table, not terms to unfold: `view_lower[i8](x)` is `int_le(-128i, view[i8](x))`, and `wrap_period[u16](n)` has `int_add(n, 65536i)` inside. `view_lower` and `view_upper` conclude a single comparison each, usable as a linear fact as the bounds on `Int` are, and the premises of `view_wrap` are the two comparisons that place `n` in the range.

<!-- spec: 2.12:8 normative -->
Why the schema is true. In the standard model a value of `T` is an integer in `[min, max]`, `view` is the inclusion, `wrap(n)` is the one integer in `[min, max]` congruent to `n` modulo `2^bits`, which exists and is unique because the range holds exactly `2^bits` consecutive integers, and `cast` is `wrap` after `view`. `view_lower` and `view_upper` say the inclusion lands in the range. `wrap_view`: a value in the range is congruent to itself and is the only member of the range that is, so `wrap` sends its view back to it. `view_wrap`: an `n` in the range is itself the unique member congruent to `n`, so `view(wrap(n)) == n`; the premises are needed, because outside the range `wrap(n)` differs from `n` by a nonzero multiple of `2^bits`, and `tests/kernel_machine.rs` refutes the unconditional statement by evaluation at `max + 1`, `min - 1`, `2^bits`, and `-2^bits` at every type. `wrap_period`: `n` and `n + 2^bits` are congruent, so they have the same reduction; this holds for a signed type as for an unsigned one, because reduction is into a window of `2^bits` consecutive integers wherever the window sits, and a window of any other width would break it. `cast_def` is the definition of `cast`. So every axiom holds of every value, at every type, and at every pair `S`, `T`.

<!-- spec: 2.12:9 normative -->
What the schema pins down. `view` is injective, by `wrap_view` and congruence, so a value of `T` is known by its view, which lies in the range. For an `n` in the range, `view_wrap` fixes `wrap(n)` as the value whose view is `n`. Any other `n` reaches the range by adding or subtracting `2^bits` some number of times, and `wrap_period` says each such step leaves `wrap(n)` unchanged. So `wrap` is reduction modulo `2^bits` into `[min, max]`, and `cast` is fixed by `cast_def`: the axioms determine the three primitives up to the choice of which value of `T` is which, which nothing in the logic can observe. `cast[T, T](x) == x` is not an axiom; it is `cast_def[T, T]` followed by `wrap_view[T]`, and `tests/kernel_machine.rs` derives it inside the kernel at every `T`. Sign extension, zero extension, and truncation are not separate rules either: `cast[i8, i16](-1i8) == -1i16`, `cast[u16, i8](300u16) == 44i8`, and `cast[i8, u64](-1i8) == 18446744073709551615u64` are each `wrap` after `view`, computed by `literal` or proved from the schema.

<!-- spec: 2.12:10 normative -->
Computation. `literal` computes each primitive on a literal: `view[T]` of a literal of `T` is the `Int` literal of the same number; `wrap[T]` of an `Int` literal is the literal of `T` at its reduction; `cast[S, T]` of a literal of `S` is `wrap[T]` of its value, so that `cast_def` holds of the computed values by construction. `evaluate` computes the same on closed terms, and a machine type is plain data, so a tuple, struct, or enum holding machine values evaluates too. `wrap` is computed by `MachineInt::wrap` in `src/kernel/machine.rs`: the remainder of the argument modulo `2^bits` by `Integer::rem`, which has the sign of the argument, is made non-negative by one addition of `2^bits`, and is then moved down by `2^bits` when it exceeds `max`, which only a signed type has room for. It is written over `Integer` so that no width is a special case: `u64` and `i64` go through the same three steps as `u8`. It is not charged steps beyond the one every primitive costs: the divisor has at most three 32-bit digits, so the long division is linear in the size of the argument, as `int_add` is. `tests/kernel_machine.rs` checks the three primitives against Rust. For every value of `u8` and `i8`: `view` is the value, `wrap` of the value is the literal, both round trips close, and `cast` to each of the eight types agrees with Rust's `as`. For every type, at a boundary set: the ends of the range and their neighbours, zero and its neighbours, and every power of two in range with its neighbours, for `view`, `wrap`, the round trips, and `cast` to every type; and for `wrap` also the values just outside the range, `2^bits`, `-2^bits`, and `2^(bits + 1)` with their neighbours, against Rust's truncating `as` from `i128`. At random: `wrap` of integers of up to two hundred bits, at every type, against a reduction that uses only Rust's `as` on the lowest 64-bit limb of the magnitude, negated in two's complement for a negative argument, which never touches `Integer`; and `cast` at every pair of types on random values against Rust's `as`.

<!-- spec: 2.12:11 normative -->
The operators and the wrapping methods, at every type, are the section after the next; the comparisons, and the lemmas proved about the machine types, follow here.

#### Comparisons

<!-- spec: 2.13:1 normative -->
The runtime comparisons at a machine type are three primitives, `eq[T]`, `lt[T]`, and `le[T]`, which are `Prim::Cmp(op, T)` with `op` one of `CmpOp::Eq`, `CmpOp::Lt`, and `CmpOp::Le`, each typed `T, T -> bool` in either mode: runtime data in, a `bool` out, so an executable term may contain one, and it is what `==`, `<`, and `<=` on two values of `T` compile to. `!=`, `>`, and `>=` are not primitives of their own: `a > b` is `lt[T](b, a)`, `a >= b` is `le[T](b, a)`, and `a != b` is a `case` on `eq[T](a, b)` with the two branches exchanged, as the lowering writes them for every supported machine type. Three forms rather than six keep the axiom below to three cases and the evaluator to three comparisons. The type is part of the term, as for every primitive at a machine type: `le[u8]` and `le[u16]` are different primitives, and a comparison at `T` of an operand of another type is rejected by typing. `Prim::name` and `CmpOp::name` give `eq`, `lt`, and `le` without the type, and the display form adds it, as `le[i32](a, b)`. These are the only runtime comparisons: a source comparison of two machine values compiles to one of them, and `==` and `!=` on two `bool` compile to a `case`.

<!-- spec: 2.13:2 normative -->
Computation. `literal` computes a comparison of two literals of `T` by comparing their numbers with `kernel::Integer`, which is `CmpOp::holds`, and `evaluate` computes the same on closed terms: `lt[i8](-1, 0)` is `true`, `le[u64](18446744073709551615, 0)` is `false`, and `eq[u16](0, 0)` is `true`. The number of a literal is its view, so this is the comparison of the views. A comparison costs the one step every primitive costs. `tests/kernel_lemmas.rs` checks it against Rust's `==`, `<`, and `<=` on every pair of values of `u8` and of `i8`, through the native evaluation `literal` and `evaluate` are computed by and through the two rules at a sample of the pairs, and at a boundary set of every type crossed with itself, the ends of the range, zero, and their neighbours.

<!-- spec: 2.13:3 normative -->
The schema. One axiom, `Axiom::CmpReflect(c, flag)`: `c` is a comparison `op[T](a, b)`, and `P` is the proposition of the same name about the views, as `CmpOp::claim` builds it: `view[T](a) ==[Int] view[T](b)` for `eq`; `view[T](a) < view[T](b)` for `lt`, which is the term `int_le(int_add(view[T](a), 1i), view[T](b))` as everywhere on `Int`; and `int_le(view[T](a), view[T](b))` for `le`.

<!-- spec: 2.13:4 normative -->
| Axiom | Premises | Conclusion |
|---|---|---|
| `cmp_reflect(c, true)` | `c` is `eq[T](a, b)`, `lt[T](a, b)`, or `le[T](a, b)`, and `c : bool`, so that `a : T` and `b : T` | `c == true => P` |
| `cmp_reflect(c, false)` | the same | `c == false => (P => False)` |

<!-- spec: 2.13:5 normative -->
The type is read off the comparison, so the axiom carries none of its own, and `Axiom::name` gives `cmp_reflect`. An instance at a term that is not one of the three comparisons, `true` for one, is rejected as having no computation step. Typing the comparison types its operands at `T`, so an operand of another type is rejected there. With `case_data` on the comparison, the two flags give each branch of an `if` its fact about the views, and the two together decide `P` without excluded middle; the converse directions, from `P` to `c == true`, are derivable, and the lemmas below derive them. This is the bridge the elaborator's hole solver uses: a claim that is a comparison of two machine values is read as the runtime test that decides it, and a branch that knows the test's outcome gives the claim through this axiom, with `view_injective` below where the claim is an equality at `T` rather than of the views.

<!-- spec: 2.13:6 normative -->
Why the schema is true. In the standard model `view` is the inclusion of the values of `T` into the integers, so it is injective and preserves the order: two values are equal exactly when their views are, and one lies below another exactly when its view does. Rust's `==`, `<`, and `<=` on a machine integer type compare the values, signed at a signed type, which are the views; and the kernel computes `eq[T]`, `lt[T]`, and `le[T]` by comparing the numbers of the literals, which are the views. So `c == true` holds exactly when `P` does, and both flags are true at every `a`, every `b`, and every `T`. The injectivity of `view` is not an axiom: `view[T](a) ==[Int] view[T](b) => a ==[T] b` follows from `wrap_view` and congruence, `a` is `wrap[T](view[T](a))`, which is `wrap[T](view[T](b))`, which is `b`, and `src/kernel/theory.rs` proves it at every `T` as the lemma `view_injective` below.

<!-- spec: 2.13:7 normative -->
Tests. `tests/kernel_lemmas.rs` uses each instance of `cmp_reflect` in both directions at every type and every comparison, checks the statement of each, and near-misses it: the wrong flag claimed, the strict comparison claimed for the weak one and the weak for the strict, the claim about the views at another type, the axiom at a comparison whose operands have another type, and at a term that is not a comparison; and it reflects an evaluated comparison at a grid of literal pairs of `u8` and `i8`, checking that evaluation decides the same about the views. `tests/kernel_soundness.rs` attacks proofs that use `cmp_reflect`, with hand-built triples at `u16` and `i32` in both directions and mutants that flip the flag, move a comparison to its sibling, `le` to `lt`, `lt` to `le`, and `eq` to `lt`, or to a neighbouring type, and exchange the operands.

#### Lemmas

<!-- spec: 2.14:1 normative -->
`src/kernel/theory.rs` declares six lemmas about `Int` and one family of eighteen lemmas about each machine type `T`, with three more at each unsigned type, the families stated over the views. They are declared through the ordinary checker when the theory is built and are not trusted: were a proof wrong, `declare` would fail, and `tests/kernel_soundness.rs` attacks each of them with mutants. Each is a checked kernel function returning evidence, with its premises as proof parameters, so a call proves the instantiated conclusion through `of_term` and takes proofs of the premises as arguments. `Theory::lemma_names` lists them under the names the source language calls them by, `int_le_of_lt` and `u16_le_trans`, and `tests/kernel_lemmas.rs` lists the same names in full, so that a rename is deliberate; the elaborator exposes exactly this table to source, 162 names, so a lemma added here is callable by name at once. Below, `a < b` is `int_lt(a, b)`, `!P` is `P => False`, `<=` is `int_le`, `v(x)` is `view[T](x)`, and `succ(a)` is `wrapping_add[T](a, 1)`.

<!-- spec: 2.14:2 informative -->
The lemmas about `Int`:

<!-- spec: 2.14:3 normative -->
| Lemma | Parameters | Conclusion |
|---|---|---|
| `int_le_of_lt` | `a, b : Int`; `a < b` | `a <= b` |
| `int_lt_of_le_of_ne` | `a, b : Int`; `a <= b`; `!(a ==[Int] b)` | `a < b` |
| `int_le_add_left` | `a, b, c : Int`; `a <= b` | `c + a <= c + b` |
| `int_le_add_right` | `a, b, c : Int`; `a <= b` | `a + c <= b + c` |
| `int_le_sub` | `a, b, c : Int`; `a <= b` | `a - c <= b - c` |
| `int_mul_le_mul_nonneg` | `a, b, c : Int`; `a <= b`; `0 <= c` | `a * c <= b * c` |

<!-- spec: 2.14:4 normative -->
The family at `T`, named `<T>_<lemma>`, as `u32_le_trans` and `i8_view_bounds`:

<!-- spec: 2.14:5 normative -->
| Lemma | Parameters | Conclusion |
|---|---|---|
| `le_refl` | `a : T` | `v(a) <= v(a)` |
| `le_trans` | `a, b, c : T`; `v(a) <= v(b)`; `v(b) <= v(c)` | `v(a) <= v(c)` |
| `le_of_lt` | `a, b : T`; `v(a) < v(b)` | `v(a) <= v(b)` |
| `lt_of_le_of_ne` | `a, b : T`; `v(a) <= v(b)`; `!(v(a) ==[Int] v(b))` | `v(a) < v(b)` |
| `lt_irrefl` | `a : T` | `!(v(a) < v(a))` |
| `le_antisymm` | `a, b : T`; `v(a) <= v(b)`; `v(b) <= v(a)` | `a ==[T] b` |
| `view_injective` | `a, b : T`; `v(a) ==[Int] v(b)` | `a ==[T] b` |
| `view_bounds` | `a : T` | `And(min(T) <= v(a), v(a) <= max(T))` |
| `le_of_cmp` | `a, b : T`; `le[T](a, b) == true` | `v(a) <= v(b)` |
| `cmp_of_le` | `a, b : T`; `v(a) <= v(b)` | `le[T](a, b) == true` |
| `lt_of_cmp` | `a, b : T`; `lt[T](a, b) == true` | `v(a) < v(b)` |
| `cmp_of_lt` | `a, b : T`; `v(a) < v(b)` | `lt[T](a, b) == true` |
| `eq_of_cmp` | `a, b : T`; `eq[T](a, b) == true` | `a ==[T] b` |
| `cmp_of_eq` | `a, b : T`; `a ==[T] b` | `eq[T](a, b) == true` |
| `lt_of_not_le` | `a, b : T`; `le[T](a, b) == false` | `v(b) < v(a)` |
| `le_of_not_lt` | `a, b : T`; `lt[T](a, b) == false` | `v(b) <= v(a)` |
| `succ_le_of_lt` | `a, b : T`; `v(a) < v(b)` | `v(succ(a)) <= v(b)` |
| `eq_symm` | `a, b : T`; `a ==[T] b` | `b ==[T] a` |

<!-- spec: 2.14:6 normative -->
And at each unsigned type only, where `min(T)` is `0`, so that a difference under its minuend stays in the range, which fails at a signed type, as `127i8 - -128i8` shows:

<!-- spec: 2.14:7 normative -->
| Lemma | Parameters | Conclusion |
|---|---|---|
| `zero_le` | `a : T` | `v(0) <= v(a)`, at the literal `0` of `T` |
| `sub_le` | `a, b : T`; `v(b) <= v(a)` | `v(wrapping_sub[T](a, b)) <= v(a)` |
| `sub_le_sub` | `a, b, c : T`; `v(b) <= v(c)`; `v(c) <= v(a)` | `v(wrapping_sub[T](a, c)) <= v(wrapping_sub[T](a, b))` |

<!-- spec: 2.14:8 normative -->
The proofs are short, and `tests/kernel_lemmas.rs` bounds them. `le_refl`, `lt_irrefl`, `le_trans`, and `int_le_add_right` are the axioms of the order at the views; `view_bounds` packs `view_lower` and `view_upper` into an `And`; `view_injective` is the chain above; `le_antisymm` and `eq_of_cmp` go through it. `lt_of_le_of_ne` and `int_lt_of_le_of_ne` are case analysis on `int_le_total`, with antisymmetry refuting the case the order does not want. From a comparison, `cmp_reflect` is the whole proof, `le_of_cmp` and `lt_of_cmp`; towards one, `cmp_of_le`, `cmp_of_lt`, and `cmp_of_eq` decide the boolean by `case_data` and refute the wrong case through `cmp_reflect`, so no excluded middle is used anywhere in the theory. Seven lemmas are linear certificates, the rule under Linear arithmetic: `int_le_of_lt`, `int_le_add_left`, `int_le_sub`, `le_of_lt`, `lt_of_not_le`, and `le_of_not_lt` each from the one premise, the last two taking `P => False` from `cmp_reflect` as a constraint; and `int_mul_le_mul_nonneg`, which is `int_le_mul` at `b + -a` and `c`, whose conclusion `0 <= (b + -a) * c` a certificate combines with four ring axioms read as linear equations in the products, `int_mul_comm` and `int_mul_add` twice each, the product `c * (a + -a)` reading as the constant `0` because its right side is a form with no atoms. `succ_le_of_lt`, `sub_le`, and `sub_le_sub` go through the table: `op_model` says the row is `wrap[T]` of the exact result, `view_wrap` under two linear certificates from the premises and the range axioms says the view of that is the exact result, and one more certificate closes the goal; `eq_symm` is one transport; `zero_le` is `view_lower` carried to the literal `0` by its literal step. `tests/kernel_lemmas.rs` uses every lemma at every type once at variables, with the premises as hypotheses, and once at literals, with the premises decided by evaluation, and checks that a lemma is refused at the wrong conclusion, at a premise of the wrong shape, and at an argument of another type.

<!-- spec: 2.14:9 normative -->
The names `u8_le_refl`, `u8_le_trans`, `u8_lt_of_le_of_ne`, `u8_succ_le_of_lt`, `u8_zero_le`, `u8_sub_le`, `u8_sub_le_sub`, and `u8_eq_symm`, which the examples call, instantiate the machine-lemma family at `u8`. Their statements use the integer views in the table above.

### Linear arithmetic

<!-- spec: 2.15:1 normative -->
The rule `linear` checks a certificate of linear arithmetic over `Int`. A certificate is a goal `G`, a literal coefficient `c0` for it, and a list of pairs `(p_i, c_i)` of a proof and a literal coefficient, and nothing else. Every constraint enters as the conclusion of a proof the kernel checks in the ordinary way, in the current context: a hypothesis, an instance of a range axiom of a machine type, an instance of a division axiom with its condition discharged, a lemma. Nothing enters on the word of whoever built the certificate, and a constraint with no proof behind it is refused as the bad proof it is, with that proof's error. What the rule trusts is three steps, stated here in full: the reading of a term as a linear form, the reading of a conclusion or the goal as a constraint, and the sum. It does no search, divides nothing, and rounds nothing.

<!-- spec: 2.15:2 normative -->
| Proof | Premises | Conclusion |
|---|---|---|
| `linear(G, c0, [(p_1, c_1), ..., (p_n, c_n)])` | `G : Prop` in `Logical` mode and has one of the two goal shapes below; each `p_i` proves, in the current context, a proposition of one of the three constraint shapes below, and `c_i` is admissible for that shape; in the sum of the negated goal times `c0` and each constraint times `c_i`, no atom is left and the constant is negative | `G` |

<!-- spec: 2.15:3 normative -->
The first step reads a term of type `Int` as a linear form, a constant and a coefficient for each atom:

<!-- spec: 2.15:4 normative -->
| Term | Read as |
|---|---|
| an `Int` literal `k` | the constant `k` |
| `int_add(s, t)`, `int_sub(s, t)` | the sum, respectively the difference, of the forms of `s` and `t` |
| `int_neg(s)` | the form of `s` negated |
| `int_mul(s, t)`, where the form of `s` has no atoms | the form of `t` times the constant of `s`; symmetrically when the form of `t` has no atoms |
| anything else | an atom with coefficient 1: a variable, a `view`, an `int_div` or `int_rem`, a product whose two sides both have atoms, a call, a projection |

<!-- spec: 2.15:5 normative -->
Two atoms are one atom exactly when they are the same term by the kernel's comparison, `same`. Nothing is commuted, unfolded, or evaluated inside an atom: `x * y` and `y * x` are two atoms, `view[u32](x)` in one constraint and `view[u32](x)` in another are one, `2 * x`, `x * 2`, and `x + x` read to the same form, and `(D / 2) * 2` is twice the atom `D / 2`. An atom whose coefficient becomes zero is dropped, so `x - x` reads as the constant `0`, and so does `(x - x) * y`.

<!-- spec: 2.15:6 normative -->
The second step reads a proposition as a constraint. A conclusion is read as the form that is non-negative, or zero, when the conclusion holds. `s - t` below is the difference of the forms of `s` and `t`, and `False` is the prelude's:

<!-- spec: 2.15:7 normative -->
| Conclusion of `p_i` | Read as | `c_i` |
|---|---|---|
| `int_le(s, t)` | `t - s >= 0` | not negative |
| `int_le(s, t) => False` | `s - t - 1 >= 0` | not negative |
| `s ==[Int] t` | `t - s == 0` | either sign |

<!-- spec: 2.15:8 normative -->
Any other conclusion is refused: an implication whose premise is not discharged, a conjunction, an equation at another type, a comparison of bytes. The conclusion read is the one the ordinary checker infers for `p_i`; a certificate states no conclusions of its own. The negated goal is read the same way, and its coefficient `c0` must be positive:

<!-- spec: 2.15:9 normative -->
| Goal `G` | Contributes |
|---|---|
| `int_le(s, t)` | `s - t - 1 >= 0`, times `c0` |
| `False` | nothing; `c0` is ignored |

<!-- spec: 2.15:10 normative -->
No other goal is accepted. `s < t` is the term `s + 1 <= t`, so a strict goal needs nothing of its own. An equation is proved as two inequalities and `int_le_antisymm`, and a negated inequality by `implies_intro` and a certificate for `False`.

<!-- spec: 2.15:11 normative -->
The third step adds the contributions up and accepts when no atom is left and the constant is negative. Why that proves the goal: when a constraint holds, its contribution is a non-negative number if it is an inequality, because its coefficient is not negative, and zero if it is an equation, whatever its coefficient; every constraint holds, because a kernel proof says so; and if the negation of the goal held as well, its contribution would be non-negative too, so the sum would be non-negative. The sum is a negative literal. So the negation of the goal fails, and by `int_le_total` the goal holds; for the goal `False`, the constraints alone are in contradiction. Reading `int_le(s, t) => False` as `s - t - 1 >= 0` is where the discreteness of `Int` enters, once, in the same place `int_le_total` puts it, and it is what lets an integer argument such as "twice a whole number between -1 and 1 is zero" be a certificate rather than a case analysis. Reasoning that needs a constraint divided by a common factor and rounded is not covered: the rule never divides.

<!-- spec: 2.15:12 normative -->
Limits are counts, so that acceptance does not depend on the machine, and each has a test at it in `tests/kernel_linear.rs`: at most `MAX_LINEAR_PAIRS` = 256 pairs; at most `MAX_LINEAR_ATOMS` = 256 atoms in any form, checked after every addition, so the sum is bounded too; and at most `MAX_LINEAR_BITS` = 512 bits in any literal the rule reads, which are the coefficients and the `Int` literals inside the goal and the conclusions. The sums are not limited; they are bounded by those three counts. A pair's proof is checked with the depth bound and the evaluation budgets of the ordinary checker. There is no clock. Refusals by the rule itself are `KernelError::Linear` with a `LinearError`: `NotAGoal`, `NotAConstraint`, `GoalCoefficient`, `NegativeCoefficient`, `TooManyPairs`, `TooManyAtoms`, `LiteralTooLarge`, `Uncancelled`, naming an atom the sum keeps, and `NotNegative`, naming the constant. A pair whose proof fails is refused with that proof's own error, and never with one of these.

<!-- spec: 2.15:13 normative -->
`CertificateText` provides a diagnostic summary, version 1 (`LINEAR_TEXT_VERSION`): `linear v1 G ; c0 ; [p_1 * c_1, ..., p_n * c_n]`, with `G` as the kernel prints terms. Each `p_i` is only its rule name, with an axiom name for an axiom; this summary is not a replayable proof serialization. Complete proofs are serialized by `src/store/text.rs`, stored as step DAGs in `Locus.lock`, expanded and checked against the current obligation when replayed. `tests/kernel_linear.rs` checks the four certificates below and certificates for the target examples, decides every one-step change of the four with an independent `i128` checker, and rejects unproved range facts, wrong proof types, quotient constraints for another divisor and remainder bounds without their conditions. It also checks random certificates against small integer assignments and verifies the specified merging of atoms.

#### The last obligation of midpoint

<!-- spec: 2.16:1 normative -->
This certificate isolates the final arithmetic obligation of midpoint. In `tests/kernel_linear.rs`, the exact results of `hi - lo`, `(hi - lo) / 2`, and `lo + half` are hypotheses about the views of three `u32` variables; the bridge from `lo <= hi` to `L <= H` is also a hypothesis. The test checks linear-certificate validation independently of the comparison-reflection and operation-model proofs that supply those facts in an elaborated program.

<!-- spec: 2.16:2 example -->
~~~text prose mathematical-derivation-fragment
let half = (hi - lo) / 2;
let mid = lo + half;
(mid, prove!(mid as Int == (lo as Int + hi as Int) / 2))
~~~

<!-- spec: 2.16:3 normative -->
Write L, H, D, F, M for the views of lo, hi, hi - lo, half, and mid; these are atoms. Write q and r for the terms `D / 2` and `D % 2`, and Q and R for `(L + H) / 2` and `(L + H) % 2`; these are atoms too. MAX is the literal 4294967295. The facts available, each with the proof that stands behind it:

<!-- spec: 2.16:4 normative -->
| Name | Constraint | Where it comes from |
|---|---|---|
| ord | `L <= H` | the hypothesis ordered, through the model's bridge from `lo <= hi` |
| lo0 | `0 <= L` | the range axiom of u32 at lo |
| hi1 | `H <= MAX` | the range axiom of u32 at hi |
| sub | `D == H - L` | the exact result of `hi - lo`, known because its obligation was met (below) |
| div | `F == q` | the meaning of `/` on u32, with `2 != 0` by evaluation |
| add | `M == L + F` | the exact result of `lo + half`, known because its obligation was met (below) |
| d1 | `D == q * 2 + r` | the decomposition axiom at D and 2 |
| d2 | `r + 1 <= 2` | the remainder bound at D and 2, its condition `0 < 2` by evaluation |
| d3 | `0 <= r` | the sign axiom at D and 2, its condition `0 <= D` being the range axiom of u32 at hi - lo |
| e1 | `L + H == Q * 2 + R` | the decomposition axiom at L + H and 2 |
| e2 | `R + 1 <= 2` | the remainder bound at L + H and 2 |
| e3 | `0 <= R` | the sign axiom at L + H and 2, its condition `0 <= L + H` by a certificate of its own, `linear(0 <= L + H, 1, [(lo0, 1), (hi0, 1)])`, where hi0 is `0 <= H`, the range axiom of u32 at hi |

<!-- spec: 2.16:5 normative -->
The obligation of `hi - lo` is `0 <= H - L`. Its negation is `-(H - L) - 1 >= 0`. The certificate is `[(ord, 1)]` with `c0 = 1`: `(L - H - 1) + (H - L) = -1`.

<!-- spec: 2.16:6 normative -->
The obligation of `lo + half` is `L + F <= MAX`, and its negation is `L + F - MAX - 1 >= 0`. The orientation of each constraint matters to the signs, so here is the table as the rule reads it, `t - s` for `s == t` and for `s <= t`:

<!-- spec: 2.16:7 normative -->
| Name | Read as |
|---|---|
| ord | `H - L >= 0` |
| lo0 | `L >= 0` |
| hi1 | `MAX - H >= 0` |
| sub | `H - L - D == 0` |
| div | `q - F == 0` |
| add | `L + F - M == 0` |
| d1 | `2q + r - D == 0` |
| d2 | `1 - r >= 0` |
| d3 | `r >= 0` |
| e1 | `2Q + R - L - H == 0` |
| e2 | `1 - R >= 0` |
| e3 | `R >= 0` |

<!-- spec: 2.16:8 normative -->
For `L + F <= MAX`, with `c0 = 2`: (div, 2), (d1, -1), (sub, 1), (ord, 1), (hi1, 2), (d3, 1).

<!-- spec: 2.16:9 example -->
~~~text prose mathematical-derivation-fragment
2 (L + F - MAX - 1)  =  2L + 2F           - 2MAX - 2
2 (q - F)            =      - 2F + 2q
-1 (2q + r - D)      =           - 2q - r + D
1 (H - L - D)        =  -L + H              - D
1 (H - L)            =  -L + H
2 (MAX - H)          =      - 2H                + 2MAX
1 (r)                =                 + r
sum                  =  -2
~~~

<!-- spec: 2.16:10 normative -->
Every atom cancels and the constant is -2, so the sum of lo and half fits.

<!-- spec: 2.16:11 normative -->
The claim is `M == Q`, proved as `M <= Q` and `Q <= M` and `int_le_antisymm`.

<!-- spec: 2.16:12 normative -->
For `M <= Q`, negation `M - Q - 1 >= 0`, with `c0 = 2`: (add, 2), (div, 2), (d1, -1), (sub, 1), (e1, 1), (e2, 1), (d3, 1).

<!-- spec: 2.16:13 example -->
~~~text prose mathematical-derivation-fragment
2 (M - Q - 1)        =  2M - 2Q                          - 2
2 (L + F - M)        = -2M      + 2L + 2F
2 (q - F)            =                - 2F + 2q
-1 (2q + r - D)      =                     - 2q - r + D
1 (H - L - D)        =           - L            + H - D
1 (2Q + R - L - H)   =      + 2Q - L            - H      + R
1 (1 - R)            =                                   - R + 1
1 (r)                =                          + r
sum                  =  -1
~~~

<!-- spec: 2.16:14 normative -->
For `Q <= M`, negation `Q - M - 1 >= 0`, with `c0 = 2`: (add, -2), (div, -2), (d1, 1), (sub, -1), (e1, -1), (d2, 1), (e3, 1). It is the mirror image: the equations change sign, and the two bounds used are `1 - r >= 0` and `R >= 0`. The sum is again -1.

<!-- spec: 2.16:15 normative -->
What the example shows. Four certificates, the longest of seven pairs, against proofs of hundreds of steps from the ring axioms. Every line of a table is a proof the kernel checks on its own. The integer reasoning, that twice a whole number lying between -1 and 1 is zero, needs nothing beyond reading a strict inequality as one with a 1 added. And the atoms must be the same terms wherever they occur, which is the elaborator's work: it must present D as the same term in sub, d1, and the range axiom, by computing let names away before it asks.

### The primitive operations

<!-- spec: 2.17:1 legality-rule -->
The operators of the machine integer types, `+`, `-`, `*`, `/`, `%`, and unary minus, and the methods `wrapping_add`, `wrapping_sub`, `wrapping_mul`, and `wrapping_neg`, are one table, `src/kernel/ops.rs`, with a row for each operation at each type it exists at: every operation at every type, except the two negations, which exist at the signed types only, as in Rust. A row is a primitive of the kernel, `op[T]`, which is `Prim::Op(op, T)`, typed `T, T -> T`, or `T -> T` for a negation, in either mode: runtime data in, runtime data out, so an executable term may contain one, and it is what the operator compiles to. The operations are named by Rust's method names, `add`, `sub`, `mul`, `div`, `rem`, `neg`, `wrapping_add`, `wrapping_sub`, `wrapping_mul`, and `wrapping_neg`; `Op::name` and `Prim::name` give the name without the type, and the display form adds it, as `add[u16](a, b)`. A row that does not exist, `neg[u8]` or `wrapping_neg[u32]`, is rejected wherever a term or an axiom is checked (`KernelError::NoRow`), and native evaluation gives it no value. The rows are the only arithmetic on machine values: the source methods `wrapping_add`, `wrapping_sub`, `wrapping_mul`, and `wrapping_neg` compile to them at the receiver's type, and the bounded `for` steps its index by `wrapping_add[T](i, 1)`.

<!-- spec: 2.17:2 legality-rule -->
Each row is stated over the model of its type. Write `e` for the exact result: a term of `Int` formed from the views of the operands by the primitive of `Int` of the same name, `int_add(view[T](a), view[T](b))` for `add` and for `wrapping_add`, and likewise `int_sub`, `int_mul`, `int_div`, `int_rem`, and `int_neg`. Then the table, with `T` the type of the row and `a` and `b` its operands:

<!-- spec: 2.17:3 legality-rule -->
| Operation | Types | Arity | Panics when | Total kernel value | Exact meaning when safe | Plain Rust may wrap |
|---|---|---|---|---|---|---|
| `add[T](a, b)`, `a + b` | all eight | 2 | `e` is outside `[min(T), max(T)]`, for `e = view(a) + view(b)` | `wrap[T](e)` | `view(add(a, b)) == e`, when it fits | yes; Locus inserts a check |
| `sub[T](a, b)`, `a - b` | all eight | 2 | the same, for `e = view(a) - view(b)` | `wrap[T](e)` | `view(sub(a, b)) == e`, when it fits | yes |
| `mul[T](a, b)`, `a * b` | all eight | 2 | the same, for `e = view(a) * view(b)` | `wrap[T](e)` | `view(mul(a, b)) == e`, when it fits | yes |
| `neg[T](a)`, `-a` | the four signed | 1 | the same, for `e = -view(a)`, which is exactly when `a` is `min(T)` | `wrap[T](e)`, which is `min(T)` at `min(T)` | `view(neg(a)) == e`, when it fits | yes |
| `div[T](a, b)`, `a / b` | all eight | 2 | `view(b) == 0`; and at a signed type also when `view(a) == min(T)` and `view(b) == -1` | `wrap[T](view(a) / view(b))`, the quotient truncated toward zero | the meaning is already exact wherever the row does not panic | no: panics in every build |
| `rem[T](a, b)`, `a % b` | all eight | 2 | the same as `div` | `wrap[T](view(a) % view(b))`, the remainder with the sign of `a` | the same | no: panics in every build |
| `wrapping_add[T](a, b)` | all eight | 2 | never | `wrap[T](view(a) + view(b))` | none beyond the meaning | never panics |
| `wrapping_sub[T](a, b)` | all eight | 2 | never | `wrap[T](view(a) - view(b))` | none | never panics |
| `wrapping_mul[T](a, b)` | all eight | 2 | never | `wrap[T](view(a) * view(b))` | none | never panics |
| `wrapping_neg[T](a)` | the four signed | 1 | never | `wrap[T](-view(a))`, which is `min(T)` at `min(T)` | none | never panics |

<!-- spec: 2.17:4 legality-rule -->
`Row::fits_at` decides whether a checked operation succeeds on concrete operands. `Row::fits` states the corresponding logical premises: range bounds for addition, subtraction, multiplication and negation; a nonzero divisor and signed overflow exclusion for division/remainder. `Row::compute` gives the total wrapped mathematical operation used by the kernel. `Row::rust_can_wrap` describes plain Rust without overflow checks, not Locus execution. Both Locus interpreters panic when `fits_at` fails, and generated Rust enforces the same rule. Only proof-checked safety permits emitting a plain Rust operator. The kernel's total meaning and the execution checker's normal-return rule are distinct.

<!-- spec: 2.17:5 legality-rule -->
The kernel evaluates a total mathematical operation and never panics. This is its value on successful Locus execution; a failing runtime check has no returning value. `literal` computes a row on literals of its type by `Row::compute`, and `evaluate` computes the same on closed terms: `add[u8](200, 100)` is `44`, `sub[u16](0, 1)` is `65535`, `div[i64](-7, 2)` is `-3` and `rem[i64](-7, 2)` is `-1`. A kernel term is a mathematical object, so a row has a value at every pair of operands, the pairs where Rust panics included: `div[i8](-128, -1)` is `wrap[i8](128)`, which is `-128`, what Rust's `wrapping_div` gives; `rem[i8](-128, -1)` is `0`; `neg[i16](-32768)` is `-32768`; and at a zero divisor, where `Int` has `a / 0 == 0` and `a % 0 == a`, `div[T](a, 0)` is `0` and `rem[T](a, 0)` is `a`. Panicking is the interpreters' business. A row is charged no steps beyond the one every primitive costs: its operands are at most 64 bits, so no product exceeds 128 bits.

<!-- spec: 2.17:6 legality-rule -->
The schema. Two axioms, instantiated at the row the axiom carries, an operation and a type, and at the operands, `Axiom::OpModel(op, T, xs)` and `Axiom::OpExact(op, T, xs)`, where `xs` are as many terms as the row's arity, each checked to have type `T` in `Logical` mode, and `e` is the exact result of their views as above:

<!-- spec: 2.17:7 legality-rule -->
| Axiom | Premises | Conclusion |
|---|---|---|
| `op_model[op, T](xs)` | `op[T]` is a row; `xs` are as many as its arity, each `x : T` | `op[T](xs) ==[T] wrap[T](e)` |
| `op_exact[op, T](xs)` | `op[T]` is a row of `add`, `sub`, `mul`, or `neg`; the same on `xs` | `min(T) <= e => (e <= max(T) => view[T](op[T](xs)) ==[Int] e)` |

<!-- spec: 2.17:8 legality-rule -->
`Axiom::name` gives `op_model` and `op_exact` without the row. An instance with the wrong number of operands is rejected as an arity error, one at a row that does not exist as `KernelError::NoRow`, and `op_exact` at a wrapping method or at `div` or `rem` as `KernelError::NoOverflow`: those rows have no overflow condition, and their meaning in every build is already the exact one wherever they do not panic, so `op_model` is all there is to say about them. `op_model[add, u16](a, b)` is `add[u16](a, b) ==[u16] wrap[u16](int_add(view[u16](a), view[u16](b)))`, and `op_exact[neg, i8](a)` is `int_le(-128i, int_neg(view[i8](a))) => (int_le(int_neg(view[i8](a)), 127i) => view[i8](neg[i8](a)) ==[Int] int_neg(view[i8](a)))`. The premises of `op_exact` are the two comparisons `fits` builds for the row, so a certificate that discharges the obligation of a `no_panic` promise discharges the premises too.

<!-- spec: 2.17:9 legality-rule -->
Why the schema is true. In the standard model, `op[T]` is the total function that sends operands of `T` to `wrap[T]` of their exact result; this is how the kernel computes every row, and `op_model` states exactly that, so it holds at every row and every operands. What has to be checked is that this function is Rust's operation wherever Rust has a value, since that is what makes the kernel's facts facts about the program. For `add`, `sub`, `mul`, and `neg`, the Rust Reference says a build without overflow checks gives the result reduced modulo `2^bits`, which is `wrap`, and a build with them gives the same value where it lies in the range and panics otherwise; the wrapping methods are defined as that reduction at every pair. For `div` and `rem`, Rust's quotient truncates toward zero, as `int_div` does, and its remainder has the sign of the dividend, as `int_rem` has; where Rust does not panic the quotient lies in the range, because `|a / b| <= |a|`, so the quotient can leave the range only when `|b| == 1`, and the one such pair that does, `min(T) / -1`, is excluded by the condition; and the remainder always lies in the range, because its magnitude is below `|b|` and its sign is that of `a`. So `wrap` changes nothing there, and the meaning is the exact one. `op_exact` follows from `op_model` and `view_wrap`: `view[T](op[T](xs)) == view[T](wrap[T](e))` by transport along `op_model`, and `view[T](wrap[T](e)) == e` by `view_wrap[T](e)` under the same two premises. It is an axiom for convenience, so that the exact result is one step rather than three, and adds nothing to what `op_model` and the model of the type already say; `tests/kernel_ops.rs` derives it inside the kernel at four rows, and checks the derived proof against the claim the axiom proves.

<!-- spec: 2.17:10 legality-rule -->
`op_model` at a pair where Rust panics. `op_model[div, i8](-128i8, -1i8)` states `div[i8](-128, -1) ==[i8] wrap[i8](int_div(-128i, -1i))`, and the right-hand side evaluates to `-128`: the axiom says the wrapped quotient is `min`, which is true of the wrapped meaning, though Rust panics there. The same holds of `op_model` at a zero divisor, where the meaning is the total value `Int` gives. This is harmless. A program that reaches an operation that panics does not continue, so no fact about that operation's result is ever used by what follows; what the checker of the check IR learns after `a / b` is that the condition held, `b != 0`, never a value at a pair that panics; and `op_exact` is stated only under `fits`. Everything the kernel proves is true in the model in which `op[T]` is the total function above, and Rust's panic is a refinement of that model that the interpreters and the check IR enforce by consulting `fits` and `fits_at`, not a fact the kernel is asked to know.

<!-- spec: 2.17:11 legality-rule -->
Tests. `tests/kernel_ops.rs` checks the table against Rust row by row, with Rust's side a macro over the concrete types. At 8 bits, for every pair of operands of `u8` and of `i8` and every row: `fits_at` holds exactly when Rust's checked operation returns `Some`, and `compute` equals Rust's wrapping operation wherever there is one, and Rust's plain result wherever it does not panic; at a zero divisor, where Rust has neither, `compute` gives the total values above. The kernel's `evaluate` of the applied row, and the premises `fits` builds, decided by the kernel one comparison at a time, are checked against the same at a sample of values crossed with itself, and at every pair under `LOCUS_EXTENDED`. At the wider types the same is done at a boundary set crossed with itself, the ends of the range, zero, and a few powers of two with their neighbours, and at random pairs, a third of them pulled to the boundary so that they overflow. `min(T) / -1` and `min(T) % -1` at every signed type, negation of `min(T)`, division by zero at every type, and the pairs just past each end of the range for `+`, `-`, and `*` have named tests, each also checking what `rust_can_wrap` says of the row. Each schema is stated at every row and misused: an operand of another type, the row at a neighbouring type, the wrong number of operands, a row that does not exist, the sibling operation, the operands exchanged, the exact equation claimed without its premises or with one or with them exchanged or strict, and `op_exact` at a row that cannot overflow. `tests/kernel_soundness.rs` attacks proofs that use both schemas, with mutants that move a row to its sibling operation or to a neighbouring type.

### Notes on the rules

<!-- spec: 2.18:1 normative -->
Using a lemma needs no rule of its own: a call to a lemma is a term of proof type, so `of_term(lemma(args))` proves the instantiated conclusion. A lemma has no defining equation, because equality at its proof type cannot be formed; nothing is lost, since proofs are irrelevant.

<!-- spec: 2.18:2 normative -->
`projection`, `literal`, `definition`, and `case_step` each expose one checked computation step as an equation; the `let` equation is a hypothesis, as described under Context. The later `case_known` and buffer rules handle additional checked reductions. Every step validates its term and the rule-specific premises. Equations are used through `transport`; the elaborator can construct these certificates without a corresponding source annotation.

<!-- spec: 2.18:3 normative -->
Projection from a literal product is typed by the product's own field values, which is the type the constructor rule checked field `i` against. So `v_i : A` always holds in `projection`, including for a nested dependent product whose inner type mentions an outer field.

<!-- spec: 2.18:4 normative -->
`definition` exposes one function application as the body instantiated with its arguments. At type `Prop` it unfolds a predicate; at a function type it gives equations such as `select() == successor`. The callee must be a declared function name or a checked `Lambda`, whose step is beta reduction. A call through a variable or another call needs its callee rewritten to one of those forms first. Proof-returning applications have no defining equality at their proof type; their conclusions are available through `of_term`.

<!-- spec: 2.18:5 normative -->
`literal` evaluates natively: arbitrary-precision `add`, `sub`, `mul`, `neg`, `div`, and `rem` of `kernel::Integer` (`src/kernel/int.rs`) for `int_add`, `int_sub`, `int_mul`, `int_neg`, `int_div`, and `int_rem`, where `div` and `rem` truncate toward zero and are total; the table and `MachineInt::wrap` of `src/kernel/machine.rs` for `view`, `wrap`, and `cast`; `Row::compute` of `src/kernel/ops.rs` for each row `op[T]`, which is `wrap` of the exact result and never panics; and the comparison of two `kernel::Integer` values, `CmpOp::holds`, for `eq[T]`, `lt[T]`, and `le[T]`. `int_le` has no literal result. On `Int` this evaluation must agree with the integers, which the axioms of `Int` describe; on the machine types it must agree with the machine integers, and `tests/kernel_machine.rs` checks it against Rust, and `tests/kernel_numbers.rs` checks, on every pair of bytes, that what it computes for `wrapping_add[u8]` is what `op_model` states, step by step, and that what it answers for a comparison is what `cmp_reflect` proves of the views; on the rows of the table it must agree with Rust wherever Rust has a value, and `tests/kernel_ops.rs` checks that it does; on the comparisons it must agree with Rust's `==`, `<`, and `<=`, and `tests/kernel_lemmas.rs` checks that it does.

<!-- spec: 2.18:6 normative -->
`check(ctx, p, P)` requires `P : Prop`, infers the proposition `p` proves, and accepts when the two are the same term.

## Comparison

<!-- spec: 2.19:1 informative -->
"The same term" means equal up to two things, and nothing else:

<!-- spec: 2.19:2 legality-rule -->
- renaming of bound variables, which the locally nameless representation makes structural equality;
- proof irrelevance: any two terms of the form `proof(p)` are the same, whatever `p` is.

<!-- spec: 2.19:3 legality-rule -->
The same relation on types compares the propositions inside `@P` and the fields of telescopes with it.

<!-- spec: 2.19:4 legality-rule -->
There is no unfolding, no evaluation, and no normalization. A hypothesis `is_three(n)` does not prove `n == 3`; transport along `definition(is_three(n))` does. `refl(2)` does not prove `wrapping_add[u8](1, 1) == 2`; `literal(wrapping_add[u8](1, 1))` does. `refl(3)` does not prove `(3, true).0 == 3`; `projection((3, true).0)` does. Two `Int` literals are the same term exactly when they are the same number, because a number has one representation: zero has no sign, and a magnitude has no leading zero digit. Two literals of machine types are the same term exactly when their types and their numbers agree, and `view[T]`, `wrap[T]`, `cast[S, T]`, each row `op[T]`, and the comparisons `eq[T]`, `lt[T]`, and `le[T]` carry their types, so the same operation at two types is two different terms.

<!-- spec: 2.19:5 legality-rule -->
Proof irrelevance is sound as a syntactic check because of the field rule: in a well-typed term, every position of proof type inside a product value holds a `proof(p)`, equality at a proof type cannot be formed, and no other term former has a proof-typed argument. Two product values with the same data and different proofs are therefore the same term, and `refl` proves them equal with no proof step.

<!-- spec: 2.19:6 legality-rule -->
Constructor disjointness and injectivity are not rules. `tests/kernel_cases.rs` derives `Red == Green => False` and `Byte(a) == Byte(b) => a == b` from `case_step`, `transport`, and a `case` that sends the constructors to different results.

## Derived forms

<!-- spec: 2.20:1 normative -->
`src/kernel/derive.rs` builds proofs out of the rules above and is not trusted: `symm`, `trans`, `rewrite`, `unfold`, and `fold`. Each computes a transport template by abstracting occurrences of a term, and the kernel checks the result like any other proof. Their source forms are described in [Proofs and evidence](../spec/10-proofs.md). Their loops are bounded by a step count.

<!-- spec: 2.20:2 normative -->
`unfold` and `fold` reach a call that mentions a bound variable, as in `forall x { is_three(x) }`, by going under `forall` and `exists` and into the conclusion of an implication, and rebuilding the binder around the rewritten body. `fold` follows the shape of its goal and unfolds an implication's premise on the way in. A call inside the arguments of a declared proposition, or under a binder within the premise of an implication, is not reached. `rewrite` needs no descent, because the term it replaces is closed. `Chain` builds an equational chain a link at a time.

## Depth bound

<!-- spec: 2.21:1 legality-rule -->
Checking, comparison, and substitution are recursive. Every public entry point (`check_type`, `infer_term`, `infer_proof`, `check_proof`, `Context::declare`, `assume`, `define`, and the declaration functions) first measures its input with an explicit work list, without recursion, and rejects input nested more than `MAX_DEPTH` = 256 levels deep, counting types, terms, and proofs together.

<!-- spec: 2.21:2 legality-rule -->
The number comes from measurement in an unoptimized build on a 2 MiB thread stack: nested arithmetic and nested quantifiers check at depth 800, and the worst shape found, a chain of transports, at 500 but not 600. To get there, the two judgments and the binder traversals are written as small dispatchers that call one function per rule or per variant; as single large matches, an unoptimized build reserved stack for every arm at once and overflowed near depth 150. A long equational argument should be a balanced tree of transitivity steps, or separate lemmas, not one chain.

<!-- spec: 2.21:3 legality-rule -->
The input-depth guard is not a bound on every intermediate term: substitution can increase depth. Checking, substitution and Rust destruction still use recursion, so the guard is a resource policy rather than a proof of total resource safety for arbitrary compositions. Source and stored-text parsers also bound nesting; the stored-text reader separately bounds bytes, expanded nodes and cumulative retained named-step nodes. These limits and their failure meanings are collected in [Implementation limits](#implementation-limits).

## Invariants the implementation maintains

<!-- spec: 2.22:1 normative -->
- Every proposition returned by proof inference is well formed in the context it was inferred in.
- Everything pushed while checking under a binder is removed before returning, on success and on failure.
- A fresh variable or hypothesis has an identity that occurs nowhere else, so generalizing over it cannot capture anything, and it cannot be referred to after its binder is left.
- Replacement terms used for `t[u]` are well formed in the context, hence contain no dangling index. For `forall_elim` this is established by typing the argument first.

## Deliberately absent

<!-- spec: 2.23:1 normative -->
Symmetry, transitivity, and congruence of equality are not rules. They are derived from `refl` and `transport`, and `tests/kernel.rs` derives each of them.

<!-- spec: 2.23:2 normative -->
The remaining limitations of the core APIs, also noted alongside their rules, are: the evaluator is recursive and bounded by a depth budget rather than written with an explicit stack, so a legitimate computation nested more than 200 deep is refused; `for_step` has no step for a body whose proofs depend on the particular upper bound; the derived forms do not reach inside a declared proposition's arguments or under a binder in an implication's premise; the depth bound covers input, not terms produced by substitution; and there is no general transport between types; dependent products are rebuilt field by field.

<!-- spec: 2.23:3 normative -->
The dependent-product regression in `tests/kernel_products.rs` uses a struct whose proof field states `wrapping_add[u8](a, b) == 10`. It exercises the same relationship between data fields and dependent evidence as a `NonZero` wrapper: construction must supply evidence for the stored data, and equality ignores the evidence field.

## Trusted base

<!-- spec: 2.24:1 normative -->
The logical trusted base comprises the acceptance and binding machinery in check.rs, term.rs, context.rs and defs.rs; declaration validation in generics.rs, recursive.rs, measured.rs and quantifiers.rs; primitive collection meanings in buffer.rs; literal arithmetic in nat.rs and int.rs; machine ranges and operation schemas in machine.rs and ops.rs; checked evaluation in eval.rs; arithmetic-certificate validation in linear.rs; and input-depth admission in depth.rs. Its admitted axioms include Int, machine models and operations, Boolean reflection/reification and optional excluded middle. The contract gives their exact premises. Derived proof construction in derive.rs, checked theory construction in theory.rs, and classical-dependency discovery in classical.rs add no authority. The end-to-end compiler boundary is larger: checked lowering/layouts/permissions, erasure/cleanup, native contracts and Rust export privacy are listed in tools/trusted-base.json and explained in [Scope and trust](../spec/15-scope.md).

<!-- spec: 2.24:2 normative -->
Agreement is tested exhaustively. For every pair of bytes, `tests/kernel_numbers.rs` has the kernel check that the native `wrapping_add[u8]` result equals `wrap[u8]` of the sum of the views, as `op_model` states it, evaluated step by step, that adding the subtrahend back to the native `wrapping_sub[u8]` result restores the minuend, and that whatever the native `<` answers, the matching fact about the views is proved by `cmp_reflect` and decided the same way by evaluating the views. `wrap[u8]` and `view[u8]` are checked against their axioms on literals. For every value of `u8` and `i8` and every target type, `tests/kernel_machine.rs` checks `view`, `wrap`, and `cast` against Rust; the wider machine types are too large to exhaust, and are checked at their boundaries and at random, as the section on them describes. For every pair of operands of `u8` and of `i8`, `tests/kernel_ops.rs` checks every row of the table of primitive operations against Rust's checked and wrapping operations, and the wider types at a boundary set crossed with itself and at random pairs, as the section on the table describes; `tests/kernel_lemmas.rs` does the same for the three comparisons, against Rust's `==`, `<`, and `<=`. That `cmp_reflect` is consistent rests on `view` being the inclusion, which is injective and preserves the order, as the subsection on the comparisons argues. That the schema of the machine types is consistent rests on the machine integers being a model of it, which that section argues axiom by axiom. `Int` is too large to exhaust. That the axioms of `Int` are consistent rests on the integers being a model of them, and on the list being short and standard: a discretely ordered commutative ring with induction, and truncating division stated by its decomposition, its bounds, and the sign of its remainder, which the section on `Int` checks against that model at every divisor. That evaluation agrees with that model rests on `kernel::Integer` computing the integers: `tests/kernel_integers.rs` compares it with Rust's arithmetic up to 128 bits, division and remainder included, and checks the ring identities beyond, and `tests/kernel_int.rs` and `tests/kernel_int_div.rs` check `evaluate` against it on random closed terms, in both directions: the true claim is proved and the false one is refused. The builder functions on `Term` and `Proof` that take closures are conveniences for constructing well-scoped terms; a term built any other way is checked just the same.

## Historical acceptance gates

<!-- spec: 2.25:1 informative -->
The kernel was built in six acceptance gates, from hand-written terms and independently of the parser. Each was small, and none was started before the ones ahead of it passed. The tests are named after them.

<!-- spec: 2.25:2 informative -->
| Gate | Scope | Passes when |
|---|---|---|
| K1 (done) | Terms, contexts with the three kinds of entry and upgrade, comparison up to renaming and proof irrelevance, `Eq` with reflexivity and transport, internal `Forall` and `Implies`. | A true equality checks; a false one is rejected; a fact in scope discharges an identical goal; a ghost variable is rejected in an executable term. |
| K2 (done) | Tuples, structs, dependent proof fields, and the `let`, projection, and literal computation axioms. | A dependent data/proof result checks; returned data instantiates a later proof field; two `NonZero` values with equal bytes are equal without a proof step. |
| K3 (done) | Functions of the logic, defining equations, `unfold`/`fold`/`rewrite`, function values, `Prop` as a value. | An explicit unfolding step checks; `select() == successor` is provable at a function type while pointwise agreement does not prove function equality; `Claim { proposition: [true] }` and `Claim { proposition: [false] }` are distinguishable and their projections compute. |
| K4 (done) | Enums and the case rule with arm evidence; declared props, the index-equation case rule, `Exists`, excluded middle with dependency recording. | An indexed proposition match supplies its index equation; a match on a proof with a non-proof result or a non-total arm is rejected; a variant concluding another proposition is rejected; constructor disjointness is derived. |
| K5 (done) | The `u8` model, reflection lemmas, and native evaluation, at first over an internal `Nat` with induction. Since E5 the model of `u8` is the one every machine type has, over `Int`, `tests/kernel_numbers.rs` states the gate over it, and R4 removed `Nat`. | A `u8` ordering lemma over three variables is proved from the model, not by enumeration; native evaluation agrees with the model exhaustively for one-byte operations. |
| K6 (done) | The range-iteration rule. | The bounded-count example checks with index-dependent state; empty range, rejected reversed bounds, `hi == 255`, and nested iteration are covered. |

<!-- spec: 2.25:3 informative -->
After the gates, the kernel grows by the commits of the build plan, which are named differently: `Int` was added by the plan's commit K2, which is not the gate K2 above, its quotient and remainder by the plan's commit K3, the machine integer types by K4, and the linear arithmetic rule by K6.

## Logical Boolean comparisons

<!-- spec: 2.26:1 legality-rule -->
The kernel has one `bool` type. Surface `Bool` is checked in logical mode; surface `bool` is checked in executable mode. This does not introduce an additional kernel type, conversion axiom, or proposition former. `holds(b)` abbreviates `b ==[bool] true`.

<!-- spec: 2.26:2 legality-rule -->
`int_eq_b(a, b)`, `int_lt_b(a, b)`, and `int_le_b(a, b)` each have type `Int, Int -> bool`. Their computation on integer literals returns respectively equality, strict order, and non-strict order of the arbitrary-precision integer values. An operand of another type or an incorrect arity is rejected. Boolean connectives retain their existing case-expression meaning.

<!-- spec: 2.26:3 legality-rule -->
For a comparison `c`, let `P` be its proposition: equality at `Int`, `int_lt(a,b)` (the abbreviation `int_le(a+1,b)`), or `int_le(a,b)`. For machine comparisons the operands of `P` are their existing `view` terms. Both kinds share these schemas, after checking the comparison and its operand types:

<!-- spec: 2.26:4 legality-rule -->
- `cmp_reflect(c, true) : c ==[bool] true => P`
- `cmp_reflect(c, false) : c ==[bool] false => (P => False)`
- `cmp_reify(c, true) : P => c ==[bool] true`
- `cmp_reify(c, false) : (P => False) => c ==[bool] false`

<!-- spec: 2.26:5 legality-rule -->
The schemas accept only a comparison primitive; they do not interpret arbitrary Boolean terms as propositions. Evaluation and reflection are deterministic, and proof equality remains syntactic rather than normalizing.

## Named-arm proposition declarations

<!-- spec: 2.27:1 legality-rule -->
A named-arm proposition declaration supplies independent header parameter types and zero or more arms. Each arm supplies a telescope consisting of those header parameters followed by its witness types, and a body of type `Prop` scoped over the entire telescope. Witnesses cannot themselves be proof-typed parameters. The body is checked in logical mode against declarations already accepted; thus it may contain connectives, Boolean holds, equalities, declared predicates, and calls of logical functions. Local lets are substituted by elaboration; logical case expressions retain the kernel's existing checked case form.

<!-- spec: 2.27:2 legality-rule -->
The constructor's checked payload telescope consists of the witnesses followed by exactly one proof of the body. Header arguments instantiate the common parameters, supplied witnesses instantiate the arm binders, and the final evidence must establish that precise instantiated body. Missing or additional evidence, wrong witness types, or evidence of a different body is rejected. Named versus positional witnesses affect only elaboration; the kernel stores positions.

<!-- spec: 2.27:3 legality-rule -->
`Construct` and `CaseProof` retain their existing representations and checking rules: case elimination introduces the witness fields and the body-evidence field belonging to the matched arm. There is no axiom equating the enclosing proposition with its body, nor any computation rule that unfolds an inductively declared proposition.

<!-- spec: 2.27:4 legality-rule -->
Declarations only refer to earlier accepted declarations. Self-recursive declarations use the positivity and induction rules of Reconciliation D5 below; unguarded or negative occurrences are refused. Primitive bootstrap declarations (`True`, `False`, `And`, `Or`) retain their existing constructor representations; implication and negation retain their current forms. During migration the old `Params` and `Indexed` declaration API remains available for existing clients and the prelude; new surface declarations use the checked arm-body API.

## Generic templates and monomorphic instances

<!-- spec: 2.28:1 legality-rule -->
A generic declaration is an unchecked template, separate from accepted kernel declarations. Its type parameters are private placeholder identities. The ordinary checker has no type-variable rule and rejects an unsubstituted placeholder as an unknown type. A template may describe a struct telescope, enum payload telescopes, function signature and body, or proposition header and arms.

<!-- spec: 2.28:2 legality-rule -->
Instantiation supplies exactly one well-formed, closed type per parameter. It checks each parameter's bound, substitutes each placeholder simultaneously through every type occurrence (including types inside terms and proofs), then submits the resulting monomorphic declaration to the same declaration checker used without generics. An instance is accepted only after this complete check. Repeated requests for the same template and types reuse the accepted identity; a failed instantiation adds no declaration and is not cached. Templates themselves introduce no assumptions or proof rules.

<!-- spec: 2.28:3 legality-rule -->
`Logical` is a checked bound on eligible kernel representations: Int, logical-mode bool, Prop, proofs and logical functions, and nominal aggregates explicitly registered logical after checking every field. Machine integers and unregistered runtime aggregates fail this bound. Surface classification distinguishes runtime bool from logical Bool before calling this API. Logical aggregate registration checks all field types; it is not an unsafe user assertion. Runtime erasure still uses the surface/IR classification checks.

<!-- spec: 2.28:4 legality-rule -->
Substitution preserves all term and hypothesis binders: type arguments contain no free term variables, and replacing a type parameter does not introduce or remove a term binder. Nominal declaration references in terms remain nominal; using a placeholder as a value constructor or leaking it into a referenced declaration cannot forge an instance and is rejected by ordinary checking.

## Finite logical enums and structural induction

<!-- spec: 2.29:1 legality-rule -->
A logical enum group is checked atomically: names for every group member are reserved in a temporary definition environment, all payload telescopes are checked, and all declarations become available together only on success. Every payload field is logical. The initial positivity check admits occurrences of a group member only as a direct payload field; an occurrence inside a function, proof, tuple, or another type expression is rejected. This is a conservative strictly positive fragment, sufficient for finite lists, naturals, and directly mutually recursive logical trees. There is no runtime allocation or Box representation for these values.

<!-- spec: 2.29:2 legality-rule -->
A structurally recursive logical function designates one enum-typed parameter as its decreasing argument. Its body is checked in a temporary environment containing its own signature. Every self-reference must be a direct call; the decreasing actual argument must be a field obtained by matching that parameter or an already known proper descendant. Matching through any member of its recursive enum group may establish further descendants. A whole original argument, a reconstructed value, an arbitrary function result, or an escaped self-function value is not accepted as structural descent. Every call is type-checked independently. This API admits only structural descent; Int-measured recursion uses its separate checked API below. Acceptance adds the checked declaration atomically and makes it logical-only.

<!-- spec: 2.29:3 legality-rule -->
`data_induction(target, motives, arms)` has one motive for each member of the target's enum group. Each motive is a proposition with one bound value of its member type. The arms, in group and variant declaration order, bind each constructor's payload and receive an induction hypothesis for every directly recursive payload field. The required conclusion is the corresponding motive at that constructor applied to its fields. The final conclusion is the target member's motive at target. Missing, duplicate, or foreign motives, incorrect payload or hypothesis counts, or a proof of a different motive instance are rejected. This rule expresses induction over finite values; it grants neither proof inspection nor runtime logical data.

<!-- spec: 2.29:4 legality-rule -->
Evaluation of recursive logical calls retains the existing deterministic step and nesting budgets. Exhausting those budgets rejects the evaluation certificate; it does not license partial logical computation. Structural descent establishes termination independently of the evaluator budget.

## Inductive named predicates

<!-- spec: 2.30:1 legality-rule -->
An inductive predicate reserves its own name in a temporary environment, checks its header and named-arm telescopes, checks recursive occurrences for strict positivity, and publishes the declaration only on success. The initial API supports a single recursive predicate; mutually recursive predicate groups are rejected as unsupported. A recursive occurrence is admitted directly as P(args), under primitive And or Or, under forall/exists, or in an implication conclusion when its premise does not mention P. Recursive occurrences in witness types, arguments of P itself, negation, an implication premise, equality, cases, or calls to helper functions are refused. This deliberately does not assume that an arbitrary helper preserves positivity.

<!-- spec: 2.30:2 legality-rule -->
The constructors still take witnesses and exactly one body proof. CaseProof exposes a constructor's original body evidence; it does not add induction hypotheses.

<!-- spec: 2.30:3 legality-rule -->
`prop_induction(scrutinee, motive, arms)` proves a motive over the header arguments from a proof of the inductive predicate. The motive has exactly one binder per header parameter and must form a proposition. Each induction arm binds fresh generic header parameters followed by that constructor's witnesses and original body evidence. It receives one additional hypothesis: the strengthened arm body obtained by replacing each positive recursive P(args) with And(P(args), motive(args)), preserving surrounding conjunction, disjunction, quantifiers, and implication. The arm must establish motive(header). The conclusion is the motive instantiated with the scrutinee claim's actual arguments.

<!-- spec: 2.30:4 legality-rule -->
Strengthening preserves branch choices and quantifier witnesses instead of flattening recursive occurrences into unrelated assumptions. Quantifier binders are opened with fresh identities before motive substitution and closed again, avoiding capture. Invalid motive arity, an arm with incorrect binders/hypotheses, or an incorrect conclusion is rejected. Induction is proof-only and does not permit inspecting erased evidence to select runtime data.

<!-- spec: 2.30:5 legality-rule -->
Source structural recursion uses an ordinary direct self call, e.g. length(tail). The source name is temporarily a fresh logical variable of the closed dependent function type; this variable is unavailable while elaborating the public signature. Source elaboration cannot add a global recursive definition or an unchecked defining equation. Final checked lowering substitutes the actual candidate FnId for this local callable and checks the resulting entire body with the kernel structural recursion API. Logical-enum parameters are tried in signature order; the first parameter for which all self calls descend structurally is selected. If none works, the declaration fails with L0203. Calls returning evidence remain subject to the same structural check, so recursive lemmas are induction. For Int measures, recurse! supplies explicit checked descent evidence under the rule below.

## Logical lambda values

<!-- spec: 2.31:1 legality-rule -->
A lambda contains a dependent parameter telescope, a result type under all parameters, and a body under those same binders. Its type is the corresponding Fn telescope. Formation checks the entire signature in the surrounding context, opens each parameter with a fresh identity, and checks the body against the instantiated result in logical mode. Captures are the free variables already in that context; substitution is binder-aware in parameters, result, and body. Lambda values are admitted only in logical mode and have no runtime closure or environment. The surface restricts their results and parameters to its logical-callable rules.

<!-- spec: 2.31:2 legality-rule -->
Application uses the ordinary checked function application rule. `definition(lambda(args))` supplies the beta equation between the well-typed application and the body with its parameters simultaneously replaced by those arguments. Equality of proof values remains forbidden, so beta for a proof-returning call is used by applying/checking the proof, never by equating proof objects. No function extensionality or unchecked callable is introduced. The evaluator reduces closed lambda applications under its existing step/depth limits. Stored certificates encode the telescope, result, and body explicitly and are rechecked after loading.

## Recursion with a nonnegative Int measure

<!-- spec: 2.32:1 legality-rule -->
A logical function may designate one Int parameter as its measure. Every recursive self-call must occur as the second value of a two-field tuple projected at position 1; the first field holds checked evidence of `0 <= next_measure && next_measure < current_measure`. The proposition uses logical Bool comparisons lifted by Holds and the existing And constructor. The current measure is the designated argument at invocation entry; the next measure is the actual argument supplied to this recursive call. The source form is `recurse!(evidence, function(next_args))`; the tuple is only its explicit kernel certificate encoding.

<!-- spec: 2.32:2 legality-rule -->
The complete body, tuple types, proof objects, and applications are checked first against the candidate signature. A separate syntactic walk verifies the guard has exactly the required proposition, scans its evidence and every actual argument for further recursive uses, and rejects every unguarded self-call or escaped self-function value. A proof cannot justify itself by making an unchecked recursive call. The function is committed atomically only after both checks succeed and has no runtime form. All other uses of tuples remain ordinary. A call on a negative measure cannot be justified; a branch may return without recursion on any Int. Each recursive edge enters the nonnegative integers and strictly decreases, which is the trusted well-foundedness principle. This does not introduce a general termination oracle or a proof axiom.

## Case computation under a checked branch equation
<!-- spec: 2.33:1 legality-rule -->
`CaseKnown { term, equation }` generalizes CaseStep. The term must be a well-typed data Case expression in the current logical context. The equation must establish that its original scrutinee equals a concrete constructor of the same type (including Bool false/true). The selected arm is opened with that constructor's checked payload and its case hypothesis is replaced by the supplied equation. The result is equality at the original Case result type between the original Case and that opened arm; the opened arm is checked at that result type. No other arm is rewritten, and a different scrutinee, wrong type, impossible variant, ill-typed payload, or invalid equation is rejected. CaseStep is the reflexive special case. Elaboration may attempt this rule only from existing checked branch equations and remains bounded.

## Library quantifier construction and elimination

<!-- spec: 2.34:1 legality-rule -->
For each closed element type T the library declares two ordinary propositions, checked through the same declaration API as application predicates. A predicate is a logical function T -> Prop. Exists<T>(P) has a Witness(value:T) arm whose body is P(value). ForAll<T>(P) has an Each(prove_each: Fn(x:T)->@P(x)) arm whose body is True. Each arm therefore uses the regular explicit body-evidence slot. The declaration registry is metadata holding the actual checked identifiers and T; no proposition is recognized from its name or an assumed numeric ID.

<!-- spec: 2.34:2 legality-rule -->
Construction is ordinary Construct. Universal specialization and existential elimination are ordinary CaseProof. The recovered function/witness is scoped to the proof-producing case arm. Applying prove_each at t gives P(t); matching Exists permits using value and its evidence to prove a result independent of the scoped witness. No extraction returning T is admitted. Predicate closures are checked Lambda terms and any beta conversion used by helpers is replayed with Definition/Transport. These derived builders add no axiom and require no primitive quantifier rule.

<!-- spec: 2.34:3 legality-rule -->
All source forall/exists syntax and theorem lifting now use the checked library declarations. Native quantifier terms and certificate forms remain a low-level kernel/store representation for existing certificates and dependent-argument congruence; they are not an alternative source-language lowering. Their existing trusted rules and hostile-input tests remain part of the contract. Removing that internal representation requires replacing its uses in dependent proof transport and migrating stored certificates, and is not needed for source quantification to be library-defined. Acceptance checks explicit constructors, keyword sugar, proof-only use of witnesses, rejected witness extraction, wrong-proof application, and absence of native quantifier nodes in source-generated proposition types.

## Physical collection snapshots

<!-- spec: 2.35:1 legality-rule -->
Physical collections use the following immutable content semantics; runtime storage and permissions are separate judgments.

<!-- spec: 2.35:2 legality-rule -->
A buffer value is an immutable finite sequence of element snapshots. It
does not contain addresses, allocation capacity, borrows, or a Rust
layout. Arrays, parameter slices, and Vec values have different physical
layouts but may use the same content snapshot. Buffer identity is never
sufficient to authorize a borrow: the source permission checker tracks
the storage root, alias path, lifetime, and its current SSA version.

<!-- spec: 2.35:3 legality-rule -->
`Buffer<T>` contains at most `u64::MAX` elements. Physical storage is
further bounded by the Rust target's `usize::MAX`; the mathematical bound
works on every supported target with at most 64-bit addresses. `length(b)` is an Int
in that range. A literal checks every entry against T. A read or update
requires kernel-checked evidence of `0 <= i && i < length(b)`. A push
requires evidence of `length(b) < u64::MAX`; ordinary executable push
supplies that fact only on normal return, after its capacity/allocator
failure paths have been accounted for by the exec checker. The buffer
language contains no unchecked read and no source annotation is allowed
to manufacture those facts.

<!-- spec: 2.35:4 legality-rule -->
The reduction equations are structural: the length of a literal is its
entry count; updating keeps length; pushing adds one; reading a literal
selects its checked entry; reading an updated index yields the supplied
element; reading the pushed index yields the appended element. Rewrites
at different symbolic indices require the appropriate equality/ordering
evidence. Each update creates a new snapshot; old snapshots remain true
descriptions of old contents and cannot authorize access to new contents.

<!-- spec: 2.35:5 legality-rule -->
Runtime indexing, update, allocation, push, and reference acquisition
have explicit exec IR operations. Their arguments run left to right.
Allocation/panic behavior remains runtime behavior; postconditions hold
only after normal return. Erasing a model observation never erases an
effectful argument. Rust foreign operations are admitted only by trusted
declarations recording an explicit reason, signature, promises, and
specification, all included in the audit.

<!-- spec: 2.35:6 legality-rule -->
A library Seq model is related by element order and length. It is not
identified with Buffer by representation or assumed equal to an address.
Constructing/composing a model is logical computation, while observing
an existing physical value requires the short shared permission check
and records the current root version without retaining a Rust reference.

<!-- spec: 2.35:7 legality-rule -->
Rule inventory for these additions: `case_known` is the branch-equation computation rule above. `buffer_step` checks one structural buffer equation; `buffer_lower` and `buffer_upper` establish the immutable length bounds 0 and u64::MAX. The checker validates the buffer term and its element type before admitting a bound. Native execution operations must separately justify indexing bounds and introduce push capacity facts only after normal return.

## Physical Box and finite recursive runtime data

<!-- spec: 2.36:1 legality-rule -->
Box<T> is always physical, including Box<T> when T is logical. Its immutable snapshot is a box containing a snapshot of T; allocation identity and addresses are not part of mathematical equality. Interior mutation is excluded. A snapshot never grants permission to move, borrow, or mutate physical storage. Source ownership and reference checks remain separate obligations.

<!-- spec: 2.36:2 legality-rule -->
Kernel Type::Boxed(T) checks T. Term::Boxed(value) is logical-only snapshot construction; projection at index zero observes the payload, and the existing checked Projection rule reduces projection of a literal box. All other indices fail. Executable IR allocation is explicit and preserves argument effects, allocator failure, and Box layout. Erasure maps Box<logical> to Box<Erased>, not Erased. Logical payload observation cannot influence a runtime branch/result.

<!-- spec: 2.36:3 legality-rule -->
Runtime recursive enums are declared atomically with all candidate schemas present before dependent field checking. Recursive references must be strictly positive and guarded by Box, so Rust has finite layout. Direct unboxed recursion and negative recursion are rejected. Logical structural model functions may descend through a boxed recursive payload; this is finite-tree recursion, not arbitrary pointer traversal.

<!-- spec: 2.36:4 legality-rule -->
Tests cover Box allocation/read, always-physical classification, logical-payload erasure retaining allocation and effects, rejection of logical extraction and Copy, direct versus boxed recursive layout, malformed projection and dependent candidate schemas, RuntimeList model, and interpreter/generated Rust agreement.

## Checked read preservation under push

<!-- spec: 2.37:1 legality-rule -->
BufferStep may additionally check a proposed equality between two Get terms. The left source must be Push(original, appended, room), the right source must be original, and the element types and indices must coincide. Both reads are independently checked, including their bounds evidence against their respective snapshots. Thus the right read supplies the essential old-bound certificate; merely being in range after push does not establish preservation. The conclusion is exactly that equality. No bound is inferred by guessing or by an untrusted identifier.

## Persistent shared references

<!-- spec: 2.38:1 legality-rule -->
A shared reference has two descriptions. Its kernel description is the immutable
value read at its creation; its physical description is `&'a T`, including the
referent layout and lifetime. They must not be confused with an owned T, even
though the kernel represents both by T. There is no interior mutability in this
tier, and no stored mutable reference. Snapshot equality never authorizes an
alias or supplies a lifetime.

<!-- spec: 2.38:2 legality-rule -->
Physical forms must support shared borrow creation, copying/transporting the
reference, dereference, reference fields, and a returned reference. These carry
ErasureLayout::Shared { lifetime, inner } and corresponding EType::Ref, with
explicit erased borrow/deref expressions. The kernel checks the unchanged value
expression; lowering/erasure retain physical reference identity and Rust syntax.
Parameter passing of the existing tier remains a separate concern: &T lends a
value for the call; a result or stored reference is a value with Shared layout.

<!-- spec: 2.38:3 legality-rule -->
Locus maintains provenance for each shared reference leaf of a local/product:
which storage root and field/index path was borrowed, at which version, and the
source span. Moving/copying the reference transports that provenance. A borrow
of a parameter may return only under the corresponding declared lifetime;
borrows of local owned storage cannot escape that storage. A stored reference's
lifetime belongs to its referent, not to the local/struct currently carrying it.
A function's lifetime signature maps each reference result leaf to input leaves
bearing that lifetime. Reborrowing may shorten a lifetime; it cannot extend one.

<!-- spec: 2.38:4 legality-rule -->
References may live through unrelated writes. An overlapping write or move
invalidates the earlier borrow for subsequent access. The error is emitted when
an invalidated reference is used again, permitting mutation after its last use
(NLL), rather than conservatively pinning a root until lexical scope ends. A
logical observation counts as a use even though it is erased, and thus receives
the same permission check. Within one call existing overlap rules remain in
force; a reference argument cannot outlive a simultaneous conflicting &mut lend.
An index borrow conservatively overlaps writes anywhere in its collection in
this tier, matching Rust's indexing behavior without split_at_mut machinery.

<!-- spec: 2.38:5 legality-rule -->
Shared references cannot provide mutable access, be moved-from as owned data,
or be converted to raw addresses. Dereferencing for an owned result requires
Copy; logical observation may inspect without moving. Stored &mut is rejected.
The physical function/struct lifetime binders are printed in Rust, and rustc is
an independent oracle for rejected escape/alias cases. Its acceptance alone is
insufficient: Locus also checks uses inside erased observations and models.

<!-- spec: 2.38:6 legality-rule -->
The typed permission checker carries provenance through structs, tuple fields, branches, calls, and returns before lowering. The independent erased checker validates physical shapes; the three-way tests include lifetime rejection checked against rustc.

## Positivity and induction through registered quantifier schemas
<!-- spec: 2.39:1 legality-rule -->
An ordinary proposition is recognized as a library quantifier only after its complete parameter and constructor telescope is checked against the Exists/ForAll schemas above. The registry records those checked IDs and the closed logical element type; a matching name or merely carrying a function is insufficient. In an inductive predicate body, a registered quantifier applied to an explicit one-argument Prop-returning Lambda is a positive context precisely when the lambda parameter/result types do not mention the recursive predicate and the lambda body passes the existing positivity check. Opaque predicate helpers and negative occurrences remain rejected. Induction strengthens the explicit lambda body pointwise using the same quantifier declaration, preserving binder scopes. This is the library representation of the already supported positive native quantifier case.

<!-- spec: 2.39:2 legality-rule -->
Source mutual logical data is inferred from the declaration dependency graph. A strongly connected component containing only `#[derive(Logical)] enum` declarations is checked as one atomic group using the existing kernel `declare_logical_enum_group` API. Every external dependency is declared before the group. All source members and constructor names become visible together only after the complete group passes. No source planning placeholder is declared in a kernel context. As in the kernel's conservative positivity rule, any group member may occur directly as a payload field; occurrences inside function, proof, tuple, Box, or other type expressions are rejected. Generic templates are specialized before grouping, so mutually recursive logical generic trees are checked per concrete instance. Structural computation and proof recursion may match across group members and then recurse on a proper same-typed descendant. Mutually recursive function declarations are still rejected; this change introduces no new recursion principle or trusted kernel rule.

<!-- spec: 2.39:3 legality-rule -->
Runtime containers may carry erased logical payloads. Their length, tags, storage operations and eager argument effects remain physical; Rust uses `Vec<Erased>` or `[Erased; N]`, including Rust's zero-sized-element allocation behavior. The exec operation carries `logical_payload`, derived from the source erasure layout by the checked native factory. It may be true only for an intrinsically logical type or the kernel Bool (which also models source runtime bool). Element inputs then check in logical mode; storage and indices still check in executable mode. This flag never grants a logical value permission to control runtime computation: source layout checking and the independently checked erased helper enforce that boundary. The checked interpreter projects logical elements to the same marker as the erased interpreter while retaining physical buffer structure.

## Implementation limits

<!-- spec: 2.40:1 legality-rule -->
All limits are counted, never elapsed-time decisions. An oversized source is rejected before tokenization (the driver checks file metadata and assembled library size before reading); the library API checks the assembled source again. Each compilation phase retains at most MAX_DIAGNOSTICS diagnostics. The first excess replaces the last retained diagnostic with L0011 and an explicit suppression notice; this is an error, never a successful truncated program. Speculative diagnostic rollback restores the ceiling state. Generated boundary tests in tests/limits.rs cover L0010/L0011 without giant checked-in fixtures.

<!-- spec: 2.40:2 normative -->
| Name | Value | Counted scope | On exhaustion |
|---|---:|---|---|
| `MAX_SOURCE_BYTES` | 67108864 | one source unit, including assembled libraries | L0010; no tokenization or elaboration |
| `MAX_DIAGNOSTICS` | 1000 | retained diagnostics in one compilation phase | L0011 replaces the final diagnostic; explicit suppression notice |
| `MAX_DIAGNOSTIC_FACTS` | 6 | distinct facts printed for one failed obligation | explicit omission note; proof search still receives all facts |
| `MAX_PARSER_DEPTH` | 64 | nested guarded parser productions | L0108; parsing rejects the construct |
| `MAX_EXPRESSION_CHAIN` | 128 | postfix/binary expression chain and cast lookahead | L0108; parsing rejects an excessive chain |
| `MAX_KERNEL_DEPTH` | 256 | kernel input terms, types and proofs; stored-text nesting | KernelError::TooDeep or named text parse error |
| `MAX_EVALUATION_STEPS` | 2000000 | one kernel evaluation | KernelError::EvaluationStepLimit |
| `MAX_EVALUATION_DEPTH` | 200 | nested kernel evaluation calls | KernelError::EvaluationTooDeep |
| `MAX_DERIVED_STEPS` | 10000 | one derived fold/unfold construction | KernelError::StepLimit |
| `MAX_LINEAR_PAIRS` | 256 | certificate pairs; default arithmetic pairs | LinearError::TooManyPairs or arithmetic Budget(pairs) |
| `MAX_LINEAR_ATOMS` | 256 | linear certificate atoms; default arithmetic atoms | LinearError::TooManyAtoms or arithmetic Budget(atoms) |
| `MAX_LINEAR_BITS` | 512 | linear certificate literal magnitude bits | LinearError::LiteralTooLarge |
| `MAX_ARITHMETIC_ELIMINATIONS` | 1024 | default arithmetic eliminations shared by nested runs | arithmetic Budget(eliminations), surfaced in L0230 notes |
| `MAX_ARITHMETIC_DERIVED` | 16384 | default arithmetic derived constraints | arithmetic Budget(derived), surfaced in L0230 notes |
| `MAX_ARITHMETIC_BITS` | 256 | default arithmetic coefficient/multiplier bits | arithmetic Budget(bits), surfaced in L0230 notes |
| `MAX_ARITHMETIC_DEPTH` | 4 | default nested arithmetic premise searches | arithmetic Budget(depth) if search remains unresolved |
| `MAX_ARITHMETIC_BRANCHES` | 64 | default arithmetic case splits | arithmetic Budget(branches), surfaced in L0230 notes |
| `MAX_NORMALIZATION_STEPS` | 400 | elaborator normalization and explanatory unfolding | unresolved proof L0230 names exhaustion; checked partial normalization is valid |
| `MAX_EXPLANATION_DEPTH` | 8 | diagnostic structural proof exploration | diagnostic note names incomplete bounded explanation |
| `MAX_GENERIC_INSTANCES` | 256 | distinct source generic instances per unit | L0281 names the instance ceiling |
| `MAX_GENERIC_TYPE_DEPTH` | 64 | source generic type expansion nesting | L0281 names the nesting ceiling |
| `MAX_PROOF_FILE_BYTES` | 67108864 | one stored-proof file | store reader returns a named size error |
| `MAX_PROOF_EXPANDED_NODES` | 1048576 | one expanded stored-proof tree and total retained named-step nodes | ParseError names MAX_PROOF_EXPANDED_NODES |
| `MAX_PROOF_TEXT_BYTES` | 4194304 | one stored term or proof text | ParseError names text-size ceiling |
| `MAX_PROOF_DIGITS` | 4096 | decimal digits in one stored integer | ParseError names integer-digit ceiling |
| `MAX_INTERPRETER_CALL_DEPTH` | 200 | calls in either reference interpreter | RunError::TooDeep names call-depth ceiling |
| `DEFAULT_RUN_FUEL` | 10000000 | CLI interpreter step allowance; API callers supply fuel | RunError::OutOfFuel; CLI reports step allowance |
| `COUNTEREXAMPLE_BOX` | 8 | diagnostic integer search interval in either direction | Counterexample::NoneInBox explicitly identifies bounded search |
| `MAX_COUNTEREXAMPLE_ATOMS` | 4 | diagnostic counterexample enumeration variables | Counterexample::TooManyAtoms; explicit omission note; no proof is inferred |

<!-- spec: 2.40:3 informative -->
The constants and machine-readable inventory are src/limits.rs::ALL. tests/limits.rs compares every name/value with this table and refuses separate resource-limit constants elsewhere in src. Arithmetic API callers may supply smaller Budget counts; kernel certificate ceilings still apply. Interpreter API callers supply fuel; the CLI uses DEFAULT_RUN_FUEL. MAX_KERNEL_DEPTH is an inclusive root-depth bound, while recursive parser/evaluator/interpreter guards refuse entry when their active depth reaches the ceiling. Stored integers count decimal digits without the minus sign. Exhausting a heuristic does not refute its goal: a successful certificate is accepted only after kernel checking, and an unresolved search reports the bound it encountered. Counterexample enumeration can decline large atom sets; this never proves a proposition. Fact-display truncation and explanation cutoffs are explicitly reported. The pinned toml 1.1.6 reader also imposes its own nesting limit of 80; exceeding it refuses the lockfile with a TOML recursion-limit error. This dependency-owned bound is tested separately from the Locus constant registry.

<!-- spec: 2.40:4 informative -->
This inventory is about implementation resource bounds. Machine integer ranges, u64 buffer-length semantics, grammar arities, operator precedences, Unicode escape syntax, file-format versions, and allocator/address-space exhaustion are not adjustable search budgets. Host allocation failure is outside the resource-limit recovery guarantee. Counts bound specific traversals or retained results, not all possible memory or wall-clock use of arbitrary programs. New hardcoded limits must enter this inventory and receive boundary tests.
