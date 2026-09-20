# Locus immutable core: scope, types, propositions, and proofs

Status: proposed language specification, revised 20 September 2026. This describes a coherent target fragment, not the implemented frontend. The current parser grammar remains in grammar.md, which lists where the frontend still differs. Rust features that this fragment does not support are tracked in rust-features.md.

This document specifies binding, formation, checking, and execution rules together. A context-free parser grammar alone would not specify the dependencies we need. Code examples use proposed surface syntax; kernel constructs and context annotations need not be exposed to programmers.

## 1. The selected fragment

The core contains:

- Unit, bool, and u8; other machine widths can be added by the same rules.
- Immutable values and shadowable let bindings.
- Functions, distinguished as fn or math fn. A named function may be used as a value. There are no closures.
- Propositions and erased proofs, including user-declared propositions (prop declarations).
- Tuples and named structs. Each field may optionally have a name. There are no anonymous structs.
- Non-recursive enums, and match expressions.
- If/else expressions, defined as a match on bool.
- Loop expressions with explicit immutable state parameters, break, and continue. These are executable only.
- Bounded for iteration over a u8 range, the only repetition available to math code.

There is no primitive refinement type, Refined constructor, implicit refinement conversion, using expression, mutation, reference type, ownership system, trait system, generic parameter, type-level computation, closure, or recursion of any kind (functions, enums, or prop declarations) in this fragment. There are no termination measures, no decreases clause, no tactic language or proof block, no requires/ensures contracts, no trusted or external declarations, and no source-level ghost keyword. Mathematical Int and Nat are later additions to the source language; the kernel has an internal Nat that source programs cannot name (section 12.2). A generic Refined library type becomes possible when ordinary generics with logical predicate parameters are added. This specification does not require it.

All primitive computations are pure. An ordinary fn may diverge; a math fn must terminate. Termination of math code is guaranteed by syntactic restriction, not by measures: math code contains no loop expression and no recursion. General effects and mutable state are later additions.

For u8, literals range from 0 through 255. The primitive wrapping_add and wrapping_sub operations are total, modulo 256. Equality and unsigned ordering are defined precisely on that domain. Runtime equality is initially defined only for u8 and bool, not for products, enums, functions, propositions, or proofs. Plain arithmetic operators are omitted here to avoid an unresolved overflow policy.

No separate snapshot operation is needed for this immutable core. A binding already denotes one stable value. Shadowing creates a new binding.

The surface choices below are explicit proposals:

1. Tuple field names are only local binders for later field types. Tuple construction, access, and destructuring are positional. Named struct fields additionally provide projection labels.
2. Products preserve declaration order. Renaming a tuple binder does not change its type. Named structs and enums remain nominal.
3. Loop state is passed explicitly to continue; there is no hidden assignment.
4. Loop invariants are ordinary proof components of the state.
5. A loop result type is formed in the outer context, not in the context of its changing state parameters.
6. The function qualifier is math fn, following Rust's qualifier-plus-fn pattern (const fn, unsafe fn). A math fn denotes a function in the mathematical sense: one well-defined result for every input. It is callable from both executable code and logic. The earlier def spelling is retired.
7. Proofs are ordinary expressions: names, constructor applications, match, calls to math functions, and checked holes. A proof is typically written as a sequence of let-bound intermediate facts. There is no tactic language.
8. A proposition's meaning is what counts as a proof of it. A prop declaration fixes that by listing the proof constructors; match on such a proof is the corresponding case analysis.
9. Proofs are checked by a small Locus proof kernel (section 12). The compiler and its automation may construct proofs, but only the kernel accepts them.

## 2. What the compiler remembers: environments and binding identities

The compiler needs four kinds of bookkeeping. These are roles in the specification, not a requirement to implement four separate data structures:

~~~
Definitions  global declarations, signatures, definitions, and primitive rules
Names        lexical name-resolution stack: source spelling -> binding identity
Context      ordered logical context of resolved bindings
Loops        enclosing loop targets, used to check break and continue
~~~

The earlier draft used Greek names: Sigma for Definitions, Omega for Names, Gamma for Context, Kappa for Loops, and rho for runtime Values. The English names describe the same concepts.

In plain language:

| Name | Question it answers |
|---|---|
| Definitions | What functions, named structs, constants, and primitive operations exist? |
| Names | Which binding does this written name refer to here? |
| Context | What is that binding's type, and what checked information is available about it? |
| Loops | Which loop would break or continue target, and what values must it receive? |
| Values | During execution, what actual data does a binding contain? |

For example:

~~~
let n: u8 = 3;
let claim: Prop = [n == 3];
let n: u8 = 9;
~~~

The compiler gives the two n declarations different identities, say n1 and n2. After the last line:

~~~
Names:
    n     -> n2
    claim -> claim1

Context:
    n1     : u8   with definition 3
    claim1 : Prop with definition [n1 == 3]
    n2     : u8   with definition 9
~~~

Names now selects n2, while claim1 still refers to n1. That is how shadowing preserves the meaning of old propositions.

For a function parameter, Context may know only n1: u8, without knowing its numeric value. That is enough to form @[n1 == n1] or check a result type containing n1. Values is used later, when the program actually runs.

Thus Type[Context] means simply: a type expression whose names and dependencies can be checked using the information currently available.

Their binding grammar and lookup rule are:

~~~
Names ::= empty | Names / Frame
Frame ::= finite map from source names to binding identities
Context ::= empty | Context, Binding

resolve(name, Names, Definitions):
    search lexical frames from innermost to outermost
    otherwise search global declarations
    otherwise report an unbound name

bind(name, type, Context, Names):
    allocate a fresh identity
    extend Context with its binding
    map name to that identity in the current frame
~~~

A runtime evaluator separately uses Values, mapping executable binding identities to values. Values is not Context: knowing a symbolic variable's type does not mean the compiler knows its runtime value.

A Context entry has this conceptual form:

~~~
Binding {
    id: fresh identity,
    type: Type,
    availability: executable_and_logical | logical_only,
    definition: optional total logical term
}
~~~

A binding whose availability is logical_only is called a ghost binding. Section 2.4 gives the rules.

- Ordinary data bindings are available to execution and reasoning.
- Prop and proof bindings are ghost.
- Witnesses obtained by eliminating an existential proof are ghost, even if their type is u8.
- A function binding's availability also depends on whether it denotes executable code or an entirely logical, total helper.
- A logical definition is recorded only when justified. A result of an ordinary fn call is not definitionally equal to that call in logic.

Context contains symbolic values, not necessarily known constants. A parameter n: u8 is a legitimate logical variable even when its numerical value is unknown.

Names and Context have different jobs. Names chooses the binding named by a source occurrence. Context gives that resolved binding its type and available logical meaning. Definitions supplies global names.

### 2.1 Sequential immutable let

For:

~~~
let x: A = e;
rest
~~~

1. Resolve A and e in the environment before this let. The new x is not in scope there.
2. Check e against A, evaluating it once if it is executable computation.
3. Introduce a fresh identity x_new.
4. Extend Context with x_new: A, and with a defining equation only if justified.
5. Extend the current lexical frame so subsequent occurrences of x resolve to x_new.
6. Check rest in the extended context.

An omitted annotation is inferred only when the expression supplies enough information.

~~~
let n: u8 = 3;
let p: Prop = [n == 3];
let h: @p = _;
let n: u8 = 9;
~~~

Internally, the two n bindings have different identities. Both p and h continue to refer to the first. Shadowing does not rewrite existing types, propositions, or proofs.

A let is not recursive. Its own initializer may refer to an earlier binding with the same spelling.

### 2.2 Scope boundaries

A block creates a lexical frame. Declarations inside it are visible after their declaration and until the end of that frame. Nested frames may shadow names.

A function parameter is in scope in subsequent parameter types, the result type, and the body. Named parameters must be unique within one parameter list; loop-state names must likewise be unique. These lists may shadow outer bindings. Ordinary lets may subsequently shadow their names.

A product field name is in scope in subsequent field types. The same holds for the payload binders of an enum or prop variant. It is not thereby a local variable in a function body or constructor initializer.

A quantifier binder is in scope only in its associated body. A pattern binder in a match arm is in scope only in that arm.

Sibling branches and sibling match arms have separate scopes.

Function declarations, constants, named struct declarations, enum declarations, and prop declarations are global in this fragment. Declarations form an acyclic dependency graph: no function, enum, or prop may refer to itself, directly or through other declarations. Repetition is provided by loops and bounded for iteration; recursive declaration cycles are a later extension.

### 2.3 Escape rule

The externally visible result type of a block or match arm must be well formed in its outer context.

A local name can be eliminated by substituting a known total definition. Otherwise, dependencies on local values must be exposed through returned fields, or hidden through an explicit logical existential when only a proof is returned.

The compiler does not silently invent existential packages or export a dangling local binder.

### 2.4 Ghost bindings

A ghost binding exists for the checker and is absent at runtime. In this fragment a binding is ghost when:

- its type is a ghost type: Prop, a proof type @P, or a math function type whose result type is a ghost type (such a function is a proof of a quantified or implicational claim, section 8.3); or
- it was bound by a pattern that takes apart a proof, such as an existential witness.

Ghost is a property of the binding, in the way mut is a property of a Rust binding. It is not a type constructor. A ghost u8 is an ordinary u8 to the logic, and can be compared with an executable u8 inside a proposition without any unwrapping.

The flow rules are:

- A ghost binding may be used inside a proposition, inside a type, in a proof, or to supply another ghost position: the initializer of a ghost binding, a ghost argument, or a ghost field.
- An executable binding may be used in all of those places too. It appears there symbolically.
- A ghost binding may not determine executable data or control flow. It cannot be the condition of an if, cannot be inspected by a pattern of an executable match (section 5.2), cannot be stored in an executable field, and cannot be passed as an executable argument. A proof may be the scrutinee of a match only under the rules of section 8.2.

Formally, Context holds three kinds of entry: executable bindings, ghost bindings, and proof hypotheses. The flow rules are then typing rules, not a separate analysis:

~~~
upgrade(Context)  =  Context with every ghost binding treated as an ordinary binding

the executable judgment looks names up in Context:
    a ghost binding is not a usable executable variable

every logical sub-position is checked in upgrade(Context):
    a proposition, a type, a proof, a ghost argument, a ghost field initializer
~~~

Anything typeable in Context is typeable in upgrade(Context), never the reverse. Propositions, types, and proofs cannot tell the two apart: whether a variable is ghost matters only to executable terms. This presentation follows the explicit refinement calculus of Ghalayini and Krishnaswami (section 17), and in an implementation it is one flag on name lookup.

Ghost is not the same thing as proof irrelevance. Two proofs of one proposition are identical (section 3.1). Two ghost u8 values are not: a ghost 3 and a ghost 4 are different values that merely have no runtime representation.

Two properties must be kept apart:

- A ghost result is a property of a binding or position: its value is absent at runtime.
- An erasable computation is a property of an expression: it passes logical checking (section 6.1), so it is total and calls no ordinary fn.

An expression that produces a ghost result is not thereby erasable. Section 6.1 gives the rule: inside an ordinary fn, a call that returns a proof is still an executable computation. Its computation is retained and only its result is discarded (section 11).

Programmers do not write a ghost qualifier in this fragment. A later extension adds a ghost keyword on binders (let, parameters, struct and tuple fields, variant payloads, loop state) so that a value of any data type can be declared logical-only. That extension adds no new checking rule: it is a third way for a binding to acquire logical_only availability. A library type wrapping a ghost field can then be defined once generics exist; the binder qualifier is the primitive.

## 3. Context-indexed type grammar

Square brackets in the following grammar are specification notation for contexts. They are not source tokens.

~~~
Type[Context] ::=
    bool
  | u8
  | Prop
  | @PropExpr[Context]
  | Tuple(Fields[Context])
  | NamedStruct(S)
  | Enum(E)
  | Fn(mode, Parameters[Context], ResultType[ParameterContext])

mode ::= runtime | math

Fields[Context] ::=
    empty
  | Field(optional_name, fresh_id: A), Fields[Context, fresh_id: A]
      where A is Type[Context]

Parameters[Context] ::=
    empty
  | Parameter(optional_name, fresh_id: A),
      Parameters[Context, fresh_id: A]
      where A is Type[Context]
~~~

Type is a specification category, not a first-class source value. This fragment has no source-level Type universe, no generic parameters, and no functions returning types. The grammar does not assert Type: Type.

A supplied name makes the fresh identity available through Names. An unnamed slot still has an internal identity, but source expressions cannot refer to it by an invented name.

A value P of type Prop is not itself a source-level type expression: @P is its proof type.

A result type is checked after all parameters have been added. A later field type is checked after all preceding fields have been added.

Value dependence is restricted to logical expressions and proof indices, including those nested inside products, variant payloads, or function signatures. A later field may mention an earlier field's value only inside a proposition. A value cannot select an unrelated runtime field type, a number of fields, or a runtime layout. Erasing the logical fields of a Locus type therefore always leaves a type with a fixed layout.

~~~
// allowed: value appears inside a proposition
struct NonZero { value: u8, evidence: @[value != 0] }

// not expressible: a runtime value selecting a data type or a length
// struct Packet { wide: bool, payload: if wide { u16 } else { u8 } }
~~~

For example:

~~~
math fn(n: u8) -> (
    value: u8,
    evidence: @[value == n.wrapping_add(1)],
)
~~~

has these contexts:

~~~
parameter type:       Context
value field type:     Context, n: u8
evidence field type:  Context, n: u8, value: u8
~~~

The full function type binds n. Its open result type is instantiated at a call:

~~~
f: math fn(n: A) -> B
a: A
---------------------------------
f(a): B[n := a]
~~~

Substitution uses resolved binding identities and is capture-avoiding.

For executable calls, arguments are evaluated left to right to stable values first. The result type is instantiated with those values, not with potentially repeated computations.

A function type is inhabited by named functions only. Using the name of a declared function as an expression yields a value of its function type. Such a value captures nothing.

### 3.1 Type identity and conversion

- Primitive types match themselves.
- Proof types @P and @Q match when P and Q are identical under the type identity rule below.
- Function types require the same mode and compare parameter types and result types after consistently renaming internal binders. Parameter spellings are not part of function type identity.
- Tuple types compare ordered field types after consistent renaming of internal binders. Optional binder spellings are not part of tuple type identity. Adding or removing an unused tuple field name does not change the type.
- Named structs and enums are nominal: distinct declarations define distinct types.
- Named struct field labels belong to the declaration's projection interface. Tuple field names are not projection labels. The nominal identity of a named struct is its declaration, not the spelling of its internal logical binders.
- No predicate implication, automatic unpacking, implicit packing, mode coercion, or general refinement subtyping is performed. A math function may be called in executable code without changing its function type.

Type identity is deliberately minimal. Two logical terms are identical when they become syntactically equal after:

- consistent renaming of bound names;
- ignoring proofs: two proof terms of the same proposition are always identical, so the contents of proof-typed arguments and fields are never compared (proof irrelevance, section 8.5);
- capture-avoiding substitution of immutable logical let definitions;
- projection from a known constructor, and match on a known constructor or literal;
- evaluation of primitive operations whose arguments are all literals.

Only the first two are performed by the kernel, whose comparison of terms involves no computation at all. The last three are bridged by the elaborator, which inserts an explicit equality step for each (section 12.2) and lets the kernel check it. The programmer sees one rule; the kernel contains no normalizer and no conversion checker, so checking is syntax-directed and its cost is predictable.

Type identity does not unfold the body of a math function, and it does not prove algebraic identities or logical equivalences. For example, @nonzero(n) and @[n != 0] are different types even when nonzero is defined by that formula. Evidence for one is turned into evidence for the other by a proof step, usually a checked hole with the first in scope:

~~~
let h2: @[n != 0] = _;   // h: @nonzero(n) is in scope; the hole may unfold nonzero
~~~

This keeps type identity predictable: whether a program type-checks never depends on how far a normalizer chose to unfold. Unfolding and rewriting belong to proof construction (section 12), where every step is checked by the kernel.

A proved equality can be used explicitly to transport evidence. An arbitrary theorem is not an automatic type-conversion rule.

## 4. Products, fields, and explicit construction

Suggested surface forms:

~~~
(u8, bool)
(value: u8, @[value != 0])
(value: u8, evidence: @[value != 0])

struct NonZero {
    value: u8,
    evidence: @[value != 0],
}
~~~

Unit is the empty tuple (). A one-field tuple has a trailing comma. Parentheses around one unlabelled type without a comma are grouping.

Names within a single field list must be unique. A field name may shadow an outer name for subsequent field types, but is not in scope in its own type.

Both named and unnamed fields occupy a source-level position. Tuple field names exist only inside the tuple type, to bind references in later field types:

~~~
let pair: (value: u8, evidence: @[value != 0]) = (n, h);

pair.0              // valid: the data
pair.1              // valid: its evidence
pair.value          // invalid: tuples have no named projection
let (v, proof) = pair; // valid: fresh local names
~~~

The following tuple types are equal up to renaming their binders:

~~~
(value: u8, evidence: @[value != 0])
(x: u8, h: @[x != 0])
~~~

Named structs support declared field-name projection, such as record.value, and positional projection for their ordered fields. An unnamed struct field is accessible positionally.

Erased fields still have source positions and logical projections.

### 4.1 Constructor checking

Given fields:

~~~
x1: A1,
x2: A2(x1),
...
xk: Ak(x1, ..., x{k-1})
~~~

check supplied expressions sequentially:

~~~
v1: A1
v2: A2[x1 := v1]
...
vk: Ak[x1 := v1, ..., x{k-1} := v{k-1}]
~~~

Executable field initializers are evaluated once, left to right. Their result bindings are used in dependent checks.

Constructors supply fields in declaration order. Tuple values and patterns are strictly positional: type-level binder names do not become constructor labels. For a named struct, an optional supplied label must match the corresponding declared field name. There is no reordering or implicit field shorthand in this fragment.

~~~
fn package(n: u8, h: @[n != 0]) -> NonZero {
    NonZero {
        value: n,
        evidence: h,
    }
}
~~~

The second field is checked at @[n != 0].

Expected types guide construction. The compiler need not infer a unique dependent product schema from an unannotated collection of values. Annotations are required when that dependency is otherwise ambiguous.

A projection r.i has the declared field type with previous field binders replaced by r.0 through r.(i-1).

Destructuring introduces fresh immutable bindings and performs the same substitution:

~~~
let (v, h) = pair;
~~~

If pair's second field proves a property of its first, h's type refers to v's resolved identity.

### 4.2 Proposition fields

~~~
struct CertifiedByte {
    value: u8,
    claim: Prop,
    evidence: @claim,
}
~~~

This is valid. The proof establishes the stored claim. The type does not require that the claim describe value; that relationship must appear explicitly if needed.

By contrast:

~~~
struct NonZero {
    value: u8,
    evidence: @[value != 0],
}
~~~

always establishes nonzeroness of the stored byte.

Proposition and proof fields are erased. Their dependencies remain visible to type and proof checking.

A Prop field is ghost data, and ghost data is part of the logical value. Two CertifiedByte values with the same byte and different claims are different values to the logic, record.claim is a function of the record, and equality of such records compares their claims, even though both have the same runtime representation. A proof field is different: it contributes nothing to the value beyond the fact that its proposition holds, so two NonZero values with the same byte are equal. The model of section 16.1 keeps the logical value and its runtime representation apart for this reason.

## 5. Enums and match

### 5.1 Enum declarations

~~~
enum Light {
    Red,
    Green,
}

enum Reading {
    Missing,
    Byte(u8),
    Checked(value: u8, evidence: @[value != 0]),
}
~~~

An enum declares a nominal type and a closed list of variants. A variant is either a bare name or a name with a positional payload. Payload fields follow the tuple rules of section 4: an optional field name is a type-local binder for later payload field types, construction and patterns are positional, and proposition or proof fields are erased.

Variants are written with their enum's name: Light::Red, Reading::Byte(3). Payload construction is checked sequentially as in section 4.1, so the second field of Reading::Checked(n, h) is checked at @[n != 0].

This fragment has no struct-style variants, explicit discriminants, or casts from enums. An enum cannot mention itself, directly or through other declarations. Every enum value is therefore a finite tree of bounded depth, and no recursion is needed to consume one. Recursive enums arrive together with structural recursion (section 15).

### 5.2 Match

~~~
match scrutinee {
    pattern_1 => body_1,
    pattern_2 => body_2,
    ...
}
~~~

Patterns are names, wildcards, bool and u8 literals, tuple patterns, struct patterns, and variant patterns such as Reading::Checked(v, h). Patterns may nest. There are no guards, alternatives, ranges, or binding-at patterns in this fragment.

1. Evaluate the scrutinee once, obtaining a fresh immutable binding s. A scrutinee that is already a binding is used directly.
2. Arms are tried in order. The arms together must be exhaustive for the scrutinee's type.
3. A name in a pattern creates a fresh immutable binding. Its type is the declared field type with earlier payload or field binders replaced by the corresponding pattern bindings, as for destructuring in section 4.1. A binding for an erased field is ghost.
4. Arm i is checked under Context extended with its pattern bindings, evidence of [s == p_i] where p_i is the pattern read as a term over those bindings, and evidence that no earlier pattern matched s. A wildcard position contributes an unnamed logical variable. This equation is formed only when s is data; a proof scrutinee follows section 8.2 instead.
5. Every arm that returns normally must produce the same expected result type R. An arm that breaks, continues, or diverges has no normal result to supply.
6. R must be well formed outside the arm-local scopes (section 2.3).
7. Facts available only in one arm do not leak into another arm or the join.
8. A pattern may not inspect a ghost position to choose an arm. Where a pattern of an executable match reaches a ghost field, the sub-pattern there must be a name, a wildcard, or an irrefutable constructor pattern all of whose fields are proofs, such as And::Intro(a, b). Checking only the outer scrutinee's type is not enough: Reading::Checked(v, Or::Left(_)) would let erased evidence select runtime behavior, and is rejected.

~~~
fn unwrap_or_one(r: Reading) -> (value: u8, evidence: @[value != 0]) {
    match r {
        Reading::Checked(v, h) => (v, h),
        Reading::Byte(b) => if b != 0 { (b, _) } else { (1, _) },
        Reading::Missing => (1, _),
    }
}
~~~

In the first arm, h has type @[v != 0]: the payload binder value has been replaced by the pattern binding v.

A logical match follows the same rule, but its scrutinee and arms must be total logical computations. A match whose scrutinee is a proof has additional restrictions, given in section 8.2.

## 6. Expressions and computation modes

The conceptual expression grammar is:

~~~
Expr ::=
    variable
  | literal
  | tuple_constructor
  | named_struct_constructor
  | variant_constructor
  | projection(Expr, field)
  | call(Expr, Expr...)
  | primitive_call(Expr...)
  | block
  | match Expr { Arms }
  | if Expr then Block else Block
  | loop(StateParameters, ResultType, Block)
  | for(index, range, StateParameters, Block)
  | break Expr
  | continue(Expr...)
  | proposition_literal
  | proposition_composition
  | _

Block ::= statements followed by an optional final Expr

Statement ::=
    let Pattern [: Type] = Expr;
  | Expr;
~~~

An absent final expression yields unit if execution reaches the block's end.

A name pattern creates a fresh binding; a wildcard creates no usable source name. Pattern fields follow declaration order. A let pattern must be irrefutable: names, wildcards, tuple patterns, and struct patterns. Variant patterns belong in match, with one exception: a let may use the constructor pattern of a declared proposition that has exactly one variant and whose payload fields are all proofs, such as And::Intro(a, b). Such a pattern cannot fail and binds only proofs (section 8.2).

There is no closure expression in this fragment. Function values are the names of declared functions. Logical lambdas are expected to be the first addition here, because they are the natural way to write a proof of a quantified or implicational claim that uses local facts (section 8.3).

### 6.1 Logic and execution

Execution is what happens at runtime: the erased program evaluating on actual values. It may diverge.

Logic is what the checker reasons about at compile time: terms over symbolic variables that are never run. An expression is in a logical position when it occurs:

- inside a proposition or a type;
- anywhere in the body of a math function;
- as the scrutinee or an arm of a match on a proof (section 8.2).

Every other position is executable. In particular, the type of an expression does not decide its position. Inside an ordinary fn, the initializer of a proof-typed let, a proof argument, a proof field of a tuple, struct, or variant, and a proof-typed branch or arm result are all executable positions.

An expression in an executable position is checked by the executable judgment. If it also passes the logical judgment, it is an erasable computation (section 2.4). This gives three cases for an expression whose result is ghost:

~~~
let h: @[n == n] = reflexive(n);     // erasable: a math call; erased entirely
let h: @[n == n] = _;                // erasable: a hole elaborates to a logical term
let impossible = spin();             // not erasable: an ordinary fn call;
                                     // the call is retained, its result is discarded
~~~

The same holds for arguments, fields, and branch results: f(spin()) evaluates spin even when f's parameter is a proof, and a proof-typed if whose branches call ordinary functions remains an executable if on its bool condition. What is never permitted is for a ghost value to choose between computations that might behave differently at runtime, including a choice between returning and diverging. That is why a match on a proof is a logical position as a whole: both its scrutinee and its arms must be total.

A logical term is treated as a mathematical value: the checker may replace a call by its definition and equals by equals. That is only sound when every logical term denotes exactly one value. If a function defined by f(x) = f(x).wrapping_add(1) were admitted into logic, unfolding it would yield f(x) == f(x).wrapping_add(1), which is false for every u8. So a math fn must denote a function in the mathematical sense, total and pure, while an ordinary fn denotes a process that may or may not return.

Two checking judgments are useful:

~~~
Definitions; Context |-logic e : T
Definitions; Context; Loops |-exec e : T
~~~

Logical expressions:

- May use symbolic representations of ordinary immutable values.
- May call only math functions.
- Must terminate. They contain no loop expression; bounded for iteration is permitted.
- Cannot read hidden mutable state, perform effects, or call a runtime fn.
- May construct propositions without deciding their truth.

Executable expressions:

- May call either function mode.
- May compute data, propositions, proofs, and products containing them.
- May diverge.
- Cannot use ghost information to determine observable runtime data or control flow (section 2.4).

The asymmetry is that logic may mention executable values, symbolically, but not executable computations; and execution may not consume ghost values.

Thus an ordinary fn can return a proof, but invoking that fn is an executable computation. Its returned evidence is available only in the continuation reached after the call returns.

### 6.2 Math functions are dual-use

A math fn is written once and used in both worlds. It is callable from logic because it is total. It is also compiled and callable at runtime whenever its parameters and result contain executable data. A math fn whose signature consists only of Prop, proof, or other ghost positions is purely logical and is erased entirely. No separate keyword distinguishes the two cases.

A lemma is a math fn returning a proof. A predicate is a math fn returning Prop.

Totality of math code is syntactic in this fragment:

- a math fn body contains no loop expression;
- declarations are acyclic, so there is no recursion;
- bounded for iteration (section 10.6) and match are permitted.

Nothing else needs to be proved about termination, and there is no decreases clause.

### 6.3 Partial correctness is not a theorem

Checking a runtime function establishes:

~~~
if evaluation returns a value, that value satisfies its declared type
~~~

Checking a math function additionally establishes termination for every valid input, by the restrictions of section 6.2.

The kernel has no model of divergence. Instead, every proof that appears inside a runtime function body is checked as a closed logical statement over its own context:

~~~
for all bindings in scope at that point,
    given every fact in scope at that point,
        the stated proposition holds
~~~

The bindings include parameters, earlier lets, pattern bindings, loop state, and the results of earlier calls. The facts include proof parameters, branch and arm evidence, and evidence returned by earlier calls. A value returned by a runtime call is simply a universally quantified variable, and any evidence returned with it is a hypothesis.

This is why a divergent function cannot damage the logic. A runtime function that never returns but advertises a proof result contributes that proposition only as a hypothesis of statements about code that is never reached. It never becomes a closed theorem, because a math function cannot call it. Runtime call results are bound in the context of their continuation; they are not globally available theorem constants.

Generating these statements from the program is done by the checker and is part of the trusted base (section 12.4). Source control-flow analysis is not otherwise trusted to add facts.

## 7. Propositions

Prop is the type of logical claims. It is distinct from bool.

A predicate is an ordinary math function returning Prop, or the name of a prop declaration (section 7.3). There is no separate primitive predicate type.

~~~
math fn nonzero(x: u8) -> Prop {
    [x != 0]
}

math fn below(limit: u8, x: u8) -> Prop {
    [x < limit]
}
~~~

### 7.1 Formation grammar

~~~
PropExpr[Context] ::=
    proposition_variable
  | proposition_projection
  | math_call_returning_Prop
  | declared_prop_application
  | [Formula[Context]]
  | !PropExpr
  | PropExpr && PropExpr
  | PropExpr || PropExpr
  | PropExpr => PropExpr
  | total_logical_block_returning_Prop
  | total_logical_match_or_if_returning_Prop

Formula[Context] ::=
    true
  | false
  | LogicalTerm == LogicalTerm
  | LogicalTerm != LogicalTerm
  | LogicalTerm < LogicalTerm
  | LogicalTerm <= LogicalTerm
  | LogicalTerm > LogicalTerm
  | LogicalTerm >= LogicalTerm
  | BooleanLogicalTerm
  | PropExpr
  | !Formula
  | Formula && Formula
  | Formula || Formula
  | Formula => Formula
  | forall (x: A) { Formula[Context, x:A] }
  | exists (x: A) { Formula[Context, x:A] }
~~~

Multiple quantifier parameters abbreviate nested binders. Their types may depend logically on earlier parameters.

LogicalTerm means an expression admissible in logical checking, not arbitrary executable code. Type checking distinguishes a Boolean term, a proposition, and a term of some other type.

Within a formula, a Boolean logical term b denotes the claim that b equals true. This embedding does not imply that arbitrary propositions can be converted to bool.

Primitive u8/bool equality outside brackets produces bool. Equality inside a formula is logical equality. Checked reflection lemmas connect primitive comparisons with their logical interpretation.

~~~
let b: bool = n != 0;
let p: Prop = [n != 0];  // valid
let q: Prop = n != 0;    // invalid: missing proposition literal
let r: Prop = nonzero(n); // valid: the call already returns Prop
~~~

Existing propositions compose without additional brackets:

~~~
let combined: Prop = p && r;
~~~

Mixed bool/Prop operands are rejected outside a proposition literal.

Constructing P does not establish P. P may depend on unknown input values, and no decision procedure for P is required.

Propositions are checked in contexts. A proposition literal refers to bindings resolved at its definition; shadowing cannot change it.

### 7.2 What is built in

The kernel provides these proposition formers directly:

~~~
Eq(A, a, b)          a == b, where A is a data type or Prop
Forall(x: A, P)      forall (x: A) { P }
Implies(P, Q)        P => Q
Exists(x: A, P)      exists (x: A) { P }
~~~

Equality is available between two terms of one data type, function types included, and between two propositions. Equality between propositions is what lets a predicate be unfolded: the defining equation of nonzero is nonzero(x) == [x != 0] (section 8.4). Equality at function types is needed for the same reason: if select() returns the function successor, the defining equation of select is select() == successor, and let equations, projections of function-valued fields, and matches on records containing functions produce equations of the same kind. Equality between two proofs is not a formable proposition; it is never needed, because term comparison already identifies them (section 3.1).

What equality supports is reflexivity, transport, and the computation axioms of section 12.2. Two principles are deliberately withheld, and are separate commitments from equality itself: function extensionality, under which agreeing on every argument would make two functions equal, and proposition extensionality. Without them, the only ways to prove two functions or two propositions equal are reflexivity and computation.

These formers are schematic in the type A. That is internal kernel machinery, not source-level generics: a source program cannot abstract over A, but the kernel's rules for Eq, Forall, and Exists apply at every type.

Forall and Implies are function-shaped: a proof of forall (x: A) { P } is a total function from any x to a proof of P, and a proof of P => Q is a total function from proofs of P to proofs of Q (section 8.3). Exists behaves as a declared proposition with one variant, Exists::Intro(witness, evidence). It is built in only because it ranges over an arbitrary type A, which needs generics to declare in source.

Everything else is declared in the prelude by the mechanism of section 7.3:

~~~
prop True  { Intro }
prop False { }

prop And(p: Prop, q: Prop) {
    Intro(left: @p, right: @q),
}

prop Or(p: Prop, q: Prop) {
    Left(evidence: @p),
    Right(evidence: @q),
}
~~~

The formula true denotes True and false denotes False. P && Q denotes And(P, Q), P || Q denotes Or(P, Q), and !P abbreviates P => False. Boolean truth and machine comparisons are represented through defined relations over the primitive logical models.

### 7.3 Declared propositions

A prop declaration introduces a new proposition by listing every way it can be proved. It is to propositions what an enum declaration is to data types.

~~~
prop SmallPrime(n: u8) {
    Two:   @SmallPrime(2),
    Three: @SmallPrime(3),
    Five:  @SmallPrime(5),
    Seven: @SmallPrime(7),
}

prop Between(lo: u8, x: u8, hi: u8) {
    Intro(lower: @[lo <= x], upper: @[x <= hi]),
}
~~~

There are three levels, exactly as for a predicate defined by a formula:

~~~
SmallPrime          : math fn(u8) -> Prop     the predicate
SmallPrime(5)       : Prop                    a claim, with no evidence attached
@SmallPrime(5)                                the type of its proofs
SmallPrime::Five    : @SmallPrime(5)          a proof
~~~

The declaration is equivalent to introducing the predicate with no defining formula, one total constructor function per variant, and a closed-world match:

~~~
math fn SmallPrime(n: u8) -> Prop;            // opaque: nothing to unfold
SmallPrime::Two   : @SmallPrime(2)
SmallPrime::Three : @SmallPrime(3)
...
match on any h: @SmallPrime(n) has exactly these cases
~~~

Rules:

- The header lists the proposition's parameters. Their types are data types or Prop. A parameter cannot be a proof, so no parameter's type depends on an earlier parameter, and the index equations of section 8.2 are independent of one another. Dependent indices, where a later equation must be transported along an earlier one, do not arise in this fragment.
- A variant is a bare name or a name with a payload, following the variant rules of section 5.1. Every payload field is ghost.
- A variant without a stated conclusion proves the proposition at the header's parameters, which are in scope in its payload types. Between::Intro(a, b) has type @Between(lo, x, hi) for the lo, x, and hi determined by the expected type or by the types of a and b.
- A variant with a stated conclusion, written after a colon, proves exactly that instance. The header's parameter names are not in scope in such a variant; it binds what it needs in its own payload. SmallPrime::Two proves SmallPrime(2) and nothing else.
- Every variant's conclusion, stated or implied, must be an application of the proposition being declared to the right number of arguments. A variant cannot conclude an unrelated proposition such as False. This required occurrence of the declared name is the only place it may appear.
- The declared proposition may not occur in any payload type, directly or through other declarations. Recursive propositions, such as reachability in a state machine or sortedness of a list, arrive with recursion (section 15).
- A declared proposition has no formula behind it. Conversion never unfolds it.

Because the variant list is closed, holding a proof is informative. Matching on h: @SmallPrime(n) yields four arms, and in the arm for Five the checker has evidence of [n == 5]. Section 8.2 gives the rule.

## 8. Proofs and proof checking

A proof value h has type @P.

~~~
Type formation:
    Context |-logic P : Prop
    -----------------------
    Context |- @P is a type

Proof checking:
    Context |-logic h : @P
~~~

The formation of @P does not imply that it has an inhabitant.

Proofs have ordinary surface expression syntax. There is no proof block, tactic language, or theorem declaration. A proof is one of:

- a name or projection of proof type;
- a variant of a declared proposition applied to its payload, such as And::Intro(hp, hq);
- a match (section 8.2);
- a call to a math function returning a proof, which is how a lemma is used;
- an application of a quantified or implicational proof (section 8.3);
- a built-in equality step (section 8.4);
- a checked hole.

A math function returning a proof serves as a theorem:

~~~
math fn reflexive(n: u8) -> @[n == n] {
    _
}
~~~

An expression underscore requests a checked proof of the expected proposition. It is not an axiom, runtime check, or unspecified datum. Failure to construct the proof is a compilation error. Without an expected proof type, the hole is rejected. Section 12.3 states what a hole is guaranteed to find.

A wildcard in a pattern is a different construct.

### 8.1 Proofs as sequences of stated facts

The intended style is declarative: state each intermediate fact with its type, and justify each small step by a lemma call or a hole.

~~~
math fn step_stays_below(i: u8, limit: u8, below: @[i < limit])
    -> @[i.wrapping_add(1) <= limit]
{
    let limit_fits: @[limit <= 255] = u8_le_max(limit);
    let no_wrap: @[i < 255] = lt_of_lt_of_le(i, limit, 255, below, limit_fits);
    let grew: @[i < i.wrapping_add(1)] = lt_wrapping_add_one(i, no_wrap);
    succ_le_of_lt(i, limit, below, grew)
}
~~~

The lemma names are illustrative prelude entries. Nothing here is special syntax: these are let bindings whose types are proof types. Each line can be checked on its own, and a failure points at one step. Any of the right-hand sides could be a hole when the step is within reach of section 12.3.

### 8.2 Match on a proof

Matching on a proof of a declared proposition is case analysis over its variants. It differs from a match on data (section 5.2) in three ways.

First, it is a logical position as a whole. The scrutinee and every arm are checked by the logical judgment, so they are total and call no ordinary fn. A match on a proof is always erased, and therefore must not be able to choose between computations that behave differently at runtime, including between returning and diverging. This holds even when every arm has a proof type.

Second, its result type must be a proof type. A proof cannot be inspected to choose executable data, a ghost data value, or a proposition.

Third, its arms receive index equations instead of a scrutinee equation. The equation [s == p_i] of section 5.2 is not formed: the two sides would have different proof types, and under proof irrelevance it would say nothing. Instead, let the scrutinee be h: @Name(a_1, ..., a_k), and let the arm's variant have payload binders ys and conclusion @Name(c_1(ys), ..., c_k(ys)). The arm is checked under Context extended with the pattern bindings ys and evidence of

~~~
[a_1 == c_1(ys)], ..., [a_k == c_k(ys)]
~~~

one equation per parameter position. For a variant without a stated conclusion each c_j is the header parameter itself, the equations are trivial, and none is added. Arm order is irrelevant and no evidence about earlier arms is added; exhaustiveness means one arm per variant.

~~~
math fn small_prime_is_small(n: u8, h: @SmallPrime(n)) -> @[n <= 7] {
    match h {
        SmallPrime::Two => _,      // has [n == 2]
        SmallPrime::Three => _,    // has [n == 3]
        SmallPrime::Five => _,     // has [n == 5]
        SmallPrime::Seven => _,    // has [n == 7]
    }
}

math fn swap(p: Prop, q: Prop, h: @[p || q]) -> @[q || p] {
    match h {
        Or::Left(hp) => Or::Right(hp),
        Or::Right(hq) => Or::Left(hq),
    }
}
~~~

In the kernel this is the case rule of a non-recursive indexed family. For a result proposition G, the rule requires, for each variant, a proof of G from the variant's payload and its index equations, and yields a proof of G. G does not depend on h, which proof irrelevance makes harmless, so no dependent motive is needed.

Two special cases follow:

- A proposition with no variants, such as False, has a match with no arms. That match may have any result type, including executable data, and may appear in an executable position. It marks a point the program has been shown never to reach, and erases to a trap.
- A let pattern may take apart a proof when the proposition has exactly one variant and every field it binds is itself a proof, as with And::Intro(a, b). This is projection, not case analysis; it adds no equations and may appear in an executable position. An existential's witness is data, so Exists::Intro(w, hw) can be opened only by a match producing a proof. The witness and its evidence are in scope only inside that arm, and the arm's result type cannot mention them.

In particular, an existential proof cannot be mined for executable data, and a proof of P || Q cannot be inspected to choose a runtime Boolean.

These restrictions are what make proof irrelevance and classical reasoning (section 8.5) harmless to execution: any two proofs of one proposition are interchangeable, so nothing other than another proof may depend on which one was supplied.

### 8.3 Quantified and implicational proofs

A proof of forall (x: A) { P } is used by applying it to a logical term, and a proof of P => Q by applying it to a proof of P:

~~~
all:  @[forall (x: u8) { x <= 255 }]
all(n)  : @[n <= 255]

imp:  @[p => q]
hp:   @p
imp(hp) : @q
~~~

Such a proof is produced in one of two ways in this fragment:

- A hole, when the claim is within reach of section 12.3.
- The name of a math function of the matching shape. A function of type math fn(x: A) -> @P proves forall (x: A) { P }, and one of type math fn(@P) -> @Q proves P => Q. Several parameters correspond to nested quantifiers and implications, in order.

~~~
math fn self_equal(x: u8) -> @[x == x] { _ }

math fn all_self_equal() -> @[forall (x: u8) { x == x }] {
    self_equal
}
~~~

Because function values capture nothing, a quantified proof that depends on local facts must be lifted by hand: write a top-level lemma that takes those locals and facts as additional leading parameters, then use the lemma's name as a proof of the general claim and apply it.

~~~
math fn below_ten_helper(limit: u8, h: @[limit < 10], x: u8, hx: @[x <= limit])
    -> @[x < 10]
{
    lt_of_le_of_lt(x, limit, 10, hx, h)
}

math fn below_ten(limit: u8, h: @[limit < 10])
    -> @[forall (x: u8) { x <= limit => x < 10 }]
{
    let general: @[forall (l: u8) { l < 10 =>
        forall (x: u8) { x <= l => x < 10 } }] = below_ten_helper;
    general(limit)(h)
}
~~~

The helper's name proves the fully general statement, and applying that proof to limit and h specializes it. This is tedious but always available. Logical lambdas remove the need for it and are the expected next addition; being logic-only, they capture only immutable and ghost values, have no runtime representation, and involve none of the ownership questions of Rust closures.

### 8.4 Equality, rewriting, and unfolding

Equality has two kernel rules:

- Reflexivity proves a == a for any logical term a of a data type, or any proposition a.
- Transport takes evidence of a == b and evidence of P(a) to evidence of P(b).

Every math function also has a defining equation, generated by the kernel when the function is accepted:

~~~
math fn nonzero(x: u8) -> Prop { [x != 0] }

nonzero.equation : forall (x: u8) { nonzero(x) == [x != 0] }
~~~

Unfolding is transport along a defining equation. It needs no kernel rule of its own, and the kernel verifies an unfolding step exactly as it verifies any transport: the equation is the function's own, and the stated result is the stated premise with the indicated occurrences replaced.

Transport needs to know which occurrences to replace. Stating that as a function argument would require a lambda, so three built-in forms determine it:

~~~
rewrite(eq, h)     eq: @[a == b], h: @P     result: @P with every occurrence of a replaced by b
unfold(f, h)       f a math function, h: @P  result: @P with every application of f replaced by its body
fold(f, h)         f a math function, h: @P  requires an expected type @Q; accepted when unfolding f in Q gives P
~~~

They are written like calls but are elaborator forms: each computes the predicate by abstracting the occurrences, emits a transport step, and lets the kernel check it. To rewrite from right to left, pass eq_symm(eq). These forms are the explicit escape hatch when a hole fails; a hole performs the same steps on its own within its budget (section 12.3).

~~~
math fn use_nonzero(n: u8, h: @nonzero(n)) -> @[n != 0] {
    unfold(nonzero, h)
}
~~~

A chain form for a == b == c <= d with one justification per link, elaborating to transitivity lemmas, is desirable; its spelling is open (section 15).

The remaining equational lemmas (eq_symm, eq_trans, congruence of each constructor and primitive) are derived from the two kernel rules. Because they apply at every type and the source language has no generics, they are written once in the kernel's own term language as part of the kernel-level prelude, and checked there like any other lemma. Source programs call them by name at any type.

### 8.5 What the logic assumes

- The logic is classical. Excluded middle holds for every proposition, and is available as the kernel-level lemma excluded_middle(p): @[p || !p]. Propositions are erased and a match on a proof can only produce another proof (section 8.2), so a classical proof cannot influence execution in any way. In the intended model a proposition denotes a truth value, and excluded middle is simply true there (section 16).
- Proof irrelevance holds, as part of term comparison (section 3.1): any two proofs of the same proposition are identical. This makes two NonZero values with equal value fields equal, with no proof step.
- Choice, function extensionality, and proposition extensionality are not included (section 7.2).
- Excluded middle is a single named lemma and not a change to any other rule, and the checker records which lemmas depend on it. A development that wants to avoid classical reasoning can be audited for that, and the default could be reversed later at the cost of the hole's propositional tier and the affected prelude lemmas.
- There are no source-level axioms. Every prelude lemma about u8 and bool is checked by the kernel, against the model of section 12.2.

Consistency is a design goal until the model of section 16 is written down and checked against every kernel rule. Exporting accepted proofs to an independent checker is valuable testing of the kernel implementation; it does not by itself show that every kernel rule is sound.

### 8.6 Trust boundary

Automation constructs proof terms. It is not an oracle.

Every added logical fact must follow from a kernel rule, checked definition, checked theorem, or the checked semantics of the current program path. Primitive machine operations need precise logical definitions and checked computation/ordering lemmas.

Program verification and logical evaluation must agree with emitted primitive behavior. Verifying that correspondence is part of the compiler's trust boundary; a proof kernel alone does not verify an arbitrary backend.

## 9. If/else expressions

Surface form:

~~~
if condition {
    then_body
} else {
    else_body
}
~~~

Else is required. The condition must produce bool, not Prop or a proof.

If/else is defined as a match on bool:

~~~
match condition {
    true => then_body,
    false => else_body,
}
~~~

All of its checking follows from section 5.2. Spelled out:

1. Evaluate condition once, obtaining a fresh immutable Boolean b.
2. Check the then branch under Context plus evidence of [b == true].
3. Check the else branch under Context plus evidence of [b == false].
4. Every branch that returns normally must produce the same expected result type R. A branch that breaks, continues, or diverges has no normal result to supply.
5. R must be well formed outside the branch-local scopes.
6. Facts available only in one branch do not leak into the other branch or the join.

If condition was a primitive comparison, its reflection lemma (section 12.2) supplies the corresponding data fact in direct form. The else branch of n != 0 receives [n == 0], not a double negation; u8 and bool equality are decidable, so this needs no classical reasoning.

~~~
if n != 0 {
    let h: @[n != 0] = _;
    ...
} else {
    let h: @[n == 0] = _;
    ...
}
~~~

For an arbitrary runtime function returning bool, the true branch initially knows only that its returned Boolean is true. A relationship to its inputs must be supplied by that function's result evidence or another checked specification. The compiler does not infer semantics from a function's name.

Branch evidence is anonymous in this fragment: a hole can use it, but a hand-written proof step cannot name it. A way to name it is open (section 15).

Branch-local proofs escape through a common result type. An enum carries both the runtime decision and the evidence that goes with it:

~~~
enum Classified {
    Zero(value: u8, evidence: @[value == 0]),
    NonZero(value: u8, evidence: @[value != 0]),
}

fn classify(n: u8) -> Classified {
    if n == 0 {
        Classified::Zero(n, _)
    } else {
        Classified::NonZero(n, _)
    }
}
~~~

A caller that matches on the result recovers the evidence in each arm. The variant tag is executable data; the evidence fields are erased.

A struct with a Prop field can also unify the branches:

~~~
struct Report {
    claim: Prop,
    evidence: @claim,
}
~~~

but a Report carries no runtime indication of which claim was selected, and a caller learns nothing about what claim says. Prefer the enum form when callers need either.

Logical if expressions follow the same case rule, but their condition and branches must be total logical computations.

## 10. Loops with immutable state

An immutable language needs an explicit way to carry different values between iterations. A loop does this by binding fresh state values at each iteration.

Suggested surface form:

~~~
loop (
    state_1: A1 = initial_1,
    state_2: A2(state_1) = initial_2,
    ...
) -> R
{
    body
}
~~~

A loop expression is executable only. It may appear in an ordinary fn, never in a math fn or a logical expression. There is no decreases clause and no termination measure: the checker does not attempt to show that a loop terminates.

A state parameter always has an explicit name and type. Its value is immutable during one iteration.

Transitions use:

~~~
continue(next_1, next_2, ...);
break result;
~~~

An empty state list is permitted. A loop body must not reach its closing brace normally: every reachable path breaks, continues, or diverges.

### 10.1 Scope and initialization

Let OuterContext be the context before the loop.

- State types form a telescope: each may use outer bindings and preceding state parameters.
- Initializers are checked left to right. Subsequent initializers may refer to the initial values of earlier state parameters.
- R is checked in OuterContext. State parameters are NOT in scope in R.
- The body is checked in OuterContext extended with abstract state parameters.
- Loops receives a target containing the state telescope and R.

Crucially, the body does not inherit the equations state_i == initial_i. Those equations describe the first iteration, not every iteration.

Facts that must hold on every iteration must be expressed in the state types, typically as proof parameters.

The body's state bindings are fresh logical variables. A continue supplies new values; it does not mutate the current bindings.

### 10.2 Continue checking

For state types A1, A2(s1), ...:

~~~
next_1: A1
next_2: A2[s1 := next_1]
...
~~~

All argument expressions refer to the current iteration's bindings and are evaluated once, left to right. The next state is installed only after they have been computed.

A proof about the old state cannot be carried over as evidence about the next state unless its type matches the new obligation or it is explicitly transformed.

Shadowing a state name with let inside the body still does not update the loop's state. Only continue chooses the next state.

### 10.3 Break checking

A break expression is checked against the fixed outer result type R.

If a final state value must appear in an escaping proposition, return it as a field and have the proof refer to that field. The compiler does not export a loop-local name into R.

Break and continue target the nearest enclosing loop or bounded for (break is not available in a bounded for). Labels are not included.

They have no normal result. The checker tracks an internal control outcome rather than inventing a normal value. If one branch or match arm transfers control and another produces R, the expression has normal result type R on the returning path; if none returns normally, the expression has no normal result. This is not an inhabitant of False or a source-level proof.

### 10.4 Invariants are state evidence

Example:

~~~
fn bounded_walk(limit: u8)
    -> (value: u8, evidence: @[value <= limit])
{
    loop (
        i: u8 = 0,
        bound: @[i <= limit] = u8_zero_le(limit)
    ) -> (value: u8, evidence: @[value <= limit])
    {
        if i == limit {
            break (i, bound);
        } else {
            let differs: @[i != limit] = _;
            let below: @[i < limit] = lt_of_le_of_ne(i, limit, bound, differs);
            let next = i.wrapping_add(1);
            let next_bound: @[next <= limit] = step_stays_below(i, limit, below);
            continue(next, next_bound);
        }
    }
}
~~~

The obligations are explicit:

1. Initialization: prove 0 <= limit. The prelude lemma u8_zero_le supplies it.
2. Body assumptions: i is an arbitrary u8 with evidence i <= limit.
3. Else branch: the branch evidence is i != limit. The hole for differs finds it in scope; the ordering lemma turns it into i < limit.
4. Preservation: step_stays_below (section 8.1) proves i.wrapping_add(1) <= limit. Because next is an immutable let, conversion identifies that with next <= limit.
5. Exit: the returned evidence proves the property of the returned field.

Only one hole remains, and it asks for a fact that is already in scope. The arithmetic steps are lemma calls, because machine-order reasoning is outside what a hole is guaranteed to find (section 12.3).

There is no special invariant keyword. The proof parameter bound is the invariant. The loop checking rule ensures it is supplied initially and on every back edge.

### 10.5 Termination is not tracked for loops

A loop is checked for partial correctness only. Its invariants hold on every iteration that runs, and its result satisfies R if it returns. Nothing is claimed about whether it returns.

This costs the logic nothing, because a loop cannot appear in math code and a math function cannot call an ordinary fn (section 6.3). What is given up is the ability to state that a particular executable loop terminates, and the ability to reuse a loop-based fn as a specification function. A specification for such a function is written separately, either as a math function using bounded iteration or as a property of the result.

Termination measures can be added later without changing any program accepted here.

### 10.6 Bounded iteration

Bounded iteration is the only repetition available to math code. It is also available to ordinary fn code. The surface form is a proposal.

~~~
for i in lo..hi (
    state_1: A1 = initial_1,
    state_2: A2(state_1) = initial_2,
    ...
) {
    body
}
~~~

lo and hi have type u8 and are evaluated once. The expression requires evidence of [lo <= hi]. The body runs for i = lo, lo + 1, ..., hi - 1 in order, and not at all when lo == hi. The index i is an immutable u8 binding, fresh on each iteration; the body cannot choose the next index.

- Ordered bounds. The evidence of [lo <= hi] is found as a fact in scope; the programmer states it with a let before the loop when it is not already available. When lo is the literal 0, the elaborator supplies it from u8_zero_le. Rust treats a reversed range as empty; Locus instead rejects a range it cannot show to be ordered, so that the final index is always hi and the result type needs no case distinction.
- State types form a telescope in OuterContext extended with i and preceding state parameters. A state type may mention i, which is how an invariant relates the state to the progress made.
- Initializers are checked with i := lo.
- The body is checked with abstract state parameters, an abstract i, and evidence of [lo <= i] and [i < hi]. As for loop, it does not inherit the initial values.
- Every reachable path of the body ends in continue(next_1, ...). The arguments are checked against the state types with i := i.wrapping_add(1). Since i < hi, that addition never wraps. Break is not available in a bounded for in this fragment; an early exit is expressed with a bool state component.
- The result is the final state: the single state value when there is one state parameter, otherwise a tuple of the state telescope in declaration order, and unit when there is none. Its type is the state telescope with i := hi. No conversion step is involved: hi is substituted as written.
- A continue inside a bounded for targets that for. Bounded fors nest; an inner state type may mention the outer index like any other outer binding.

Because a range is half-open and its bounds are u8, the index never takes the value 255. A computation that must visit all 256 values handles the last one outside the loop.

Termination needs no argument from the programmer: the range is finite and the index is not assignable. A bounded for whose bounds, initializers, and body are total logical computations is itself a total logical computation.

~~~
math fn count_up(n: u8) -> (total: u8, same: @[total == n]) {
    for i in 0..n (
        acc: u8 = 0,
        same: @[acc == i] = _
    ) {
        let next = acc.wrapping_add(1);
        let next_same: @[next == i.wrapping_add(1)] = _;
        continue(next, next_same);
    }
}
~~~

The first hole asks for 0 == 0. The second follows from acc == i by congruence. The result type of the for is the state telescope at i := n, which is the declared result type up to binder names.

In the kernel, a bounded for is the term-level recursion rule. Writing S(i) for the state telescope at index i:

~~~
S(i) is a well-formed telescope for any i: u8, and its erasure does not depend on i
ordered : @[lo <= hi]
init : S(lo)
i: u8, @[lo <= i], @[i < hi], s: S(i)  |-  body : S(i.wrapping_add(1))
--------------------------------------------------------------------
for i in lo..hi (s = init) { body } : S(hi)
~~~

The side condition on erasure always holds in Locus, because a value may appear in a type only inside a proposition (section 3): the index can change what the state's proofs say, never what the state's data is. That is exactly what lets a loop whose invariant varies with the index erase to a plain loop over one fixed state type. The rule is sound by induction on hi - lo in the model of section 16; with an empty range, lo == hi and the result is init.

Recursion in proofs is a separate kernel rule, induction over the internal Nat (section 12.2). It has no runtime content and is erased entirely. Keeping the two apart, one rule that computes and erases to a loop and one that only proves, follows the explicit refinement calculus cited in section 17.

The cases an implementation must test before this construct is relied on: an empty range (lo == hi); reversed bounds, rejected for want of evidence; hi == 255; a nested for whose inner state mentions the outer index; and a dependent state whose result type is consumed by the caller, as in count_up.

## 11. Erasure and runtime behavior

Erasure is a function on types and on terms, defined by recursion on their structure. Every Locus type erases to a simple type with no propositions and no dependency, and every well-typed term erases to a term of the erased type.

Erasure preserves shape. A ghost position does not disappear: it is filled by a named zero-sized marker. Field positions, tuple arity, parameter lists, and patterns are therefore the same before and after, which is what lets the generated Rust read like the source. A marker has no runtime cost.

~~~
Proved     the erasure of a proof
Ghost      the erasure of any other ghost value; in this fragment, a proposition
~~~

### 11.1 Erasure of types

A ghost type is Prop, a proof type, or a math function type with a ghost result (section 2.4).

~~~
|bool| = bool          |u8| = u8          |()| = ()

|@P|   = Proved        |Prop| = Ghost
|math fn(...) -> R|    = Proved when R is a proof type, Ghost when R is Prop

|(F_1, ..., F_n)|             = (|F_1|, ..., |F_n|)
|struct S|                    = S with each field's type erased
|enum E|                      = E with each payload field's type erased
|fn(P_1, ..., P_n) -> R|      = fn(|P_1|, ..., |P_n|) -> |R|
|math fn(P_1, ..., P_n) -> R| = the same, when R is not ghost
~~~

Tuple binder names are dropped, since Rust tuples have none; struct field names are kept. Because dependency occurs only inside propositions, erasing a type never needs the value of any term.

Two refinements are expected later and change nothing here. With the ghost keyword, Ghost can carry the type of the data it stands for, Ghost<T>, which keeps that type visible and keeps a type parameter that occurs only in ghost fields in use. With generics, a type parameter instantiated with a ghost type is simply the corresponding marker.

### 11.2 Erasure of terms

Lowering an expression depends on two independent questions: whether its result is needed at runtime, and whether its computation is erasable (total and logical, section 2.4).

| Result needed at runtime? | Computation erasable? | Action |
|---|---|---|
| Yes | Either | Produce the runtime value. |
| No | Yes | Omit the computation; the marker stands in its place. |
| No | No | Preserve the computation; the marker is its value. |

A total expression that produces a needed byte is compiled like any other: being erasable permits omission only when nothing needs the result.

~~~
|e|, for e of ghost type:
    e is a variable                 the variable: a proof bound by a let or a pattern
                                    is an ordinary binding of type Proved
    e is erasable                   the marker for e's type: Proved or Ghost
    otherwise                       { effects(e); marker }

effects(e) = nothing                          when e is erasable
effects(f(a_1, ..., a_n))                     when f is an ordinary fn:
    the call itself, with its arguments erased; its value, a marker, is discarded
effects(C(e_1, ..., e_n)), effects(math_f(e_1, ..., e_n)), effects(e.field), effects(p(e))
    = effects(e_1); ...; effects(e_n)         in order; C is any constructor, proof constructors included
effects(if c { a } else { b })  = if |c| { effects(a) } else { effects(b) }
effects(match s { p_i => b_i }) = match |s| { |p_i| => effects(b_i) }     when s is data
effects(match h { ... })        = nothing     a match on a proof is logical as a whole (section 8.2)
effects({ statements; e })      = the erased statements; effects(e)
~~~

So a hole, a lemma call, a proof constructor, or a rewrite erases to Proved, and a proposition literal to Ghost. And::Intro(spin(), existing_proof), where spin is an ordinary fn returning a proof, erases to { spin(); Proved }: the call still runs. A call such as spin() on its own needs no block at all, because its erased result type is already Proved.

Everything else is erased by recursion, keeping its shape:

~~~
|x| = x          |literal| = literal
|let p: T = e; rest|           let |p| = |e|; |rest|
|f(a_1, ..., a_n)|             |f|(|a_1|, ..., |a_n|)
|(e_1, ..., e_n)|, |S { ... }|, |E::V(...)|     the same constructor over the erased fields
|r.field|                      |r|.field, at the same position
|match s { p_i => b_i }|       match |s| { |p_i| => |b_i| }      when s is data
|match h { }|                  trap                              an empty match used for its value
|if c { a } else { b }|        if |c| { |a| } else { |b| }
|loop (...) -> R { body }|     a loop over the erased state, with |body|
|for i in lo..hi (...) { body }|   a counted loop over the erased state, with |body|
|continue(n_1, ...)|, |break e|    the same, over erased arguments
~~~

A let that binds a proof stays a let, of a marker: let h: @[n != 0] = _; erases to let h = Proved;, and a later use of h is a use of that binding. Patterns keep every position; a sub-pattern at a ghost position is a name or a wildcard, or is irrefutable (section 5.2), so matching never inspects a marker. Arguments and fields are evaluated left to right, exactly as written, because nothing has been removed or moved.

A math function whose signature is entirely ghost, such as a lemma or a predicate, has no runtime form: its declaration is not emitted, and a call to it is an erasable ghost expression.

Three properties are required of this definition, and are proof obligations of section 16:

- Erasure preserves typing: a well-typed term erases to a term that is well typed at the erased type.
- Erasure commutes with substitution.
- No trap is ever evaluated: on every execution of a well-typed program, control never reaches an erased empty match.

### 11.3 Divergence is preserved

If a product has no executable fields, it has no data payload; this alone does not authorize removing the computation that produces it.

In particular:

~~~
fn spin() -> @[false] {
    loop () -> @[false] {
        continue();
    }
}

fn caller() -> u8 {
    let impossible = spin();
    0
}
~~~

The first declaration is a partial computation that never returns. It is not a proof of false. The second must continue to diverge after erasure: spin() is not erasable, so the let erases to a call followed by the rest.

A math function attempting to call spin is rejected. No closed logical theorem may obtain an assumption merely from an ordinary fn signature.

This distinction must be preserved by optimization as well as initial lowering.

## 12. The proof kernel and proof construction

### 12.1 Architecture

Locus has its own proof kernel, written in Rust and kept small. Everything else that touches proofs is untrusted: the elaborator, the hole solver, any decision procedure, and any external tool or model that proposes a proof. Each of them must hand the kernel an explicit proof term, and only the kernel's acceptance counts.

~~~
surface program
    -> elaborator            untrusted: resolves names, infers, fills holes
    -> explicit kernel terms
    -> kernel                trusted: checks every term
    -> accepted / rejected
~~~

Proof search may therefore be arbitrarily clever or arbitrarily unreliable without affecting soundness.

### 12.2 What the kernel checks

This is the intended rule inventory. It fixes the kernel's term representation; section 16 lists what remains to be made exact.

Terms and types:

- Data types: bool, u8, tuples, declared structs, declared enums, and function types in both modes. Prop, and the proof type @P for P: Prop.
- An internal type Nat of natural numbers, with zero, successor, an induction rule for proofs, and native arbitrary-precision evaluation on literals. Nat is not available to source programs in this fragment. It is the model of u8, and induction over it is how the u8 theory is proved. It later becomes the source-level Nat, and its induction rule the basis of structural recursion.
- Total functions: declared math functions, applied to arguments. Each accepted math function contributes its defining equation (section 8.4).
- Proposition formers Eq, Forall, Implies, and Exists (section 7.2), schematic in their type argument, and declared props with their constructors.

Rules:

- Formation and application for functions, Forall, and Implies.
- Constructors, projections, and the case rule for tuples, structs, and enums; the case rule on bool is the basis of if.
- Constructors and the index-equation case rule for declared props, and for Exists (section 8.2).
- Reflexivity and transport for Eq, at every data type, function types included, and at Prop. No extensionality rule.
- Excluded middle (section 8.5).
- Two recursion rules, kept apart: range iteration for terms, which erases to a loop (section 10.6), and induction over Nat for proofs, which is erased.
- Comparison of terms up to renaming of bound names and proof irrelevance, and nothing else. The kernel has no conversion checker and never normalizes an open term.

Computation is a family of equality axioms, each checked by matching its instance syntactically, and each used through transport:

~~~
let          x == e                       for an immutable logical let x = e, as a hypothesis in its scope
projection   (e_1, ..., e_n).i == e_i     and likewise for struct fields
case         match C(args) { ... C(xs) => b ... } == b[xs := args]
                                          and likewise for a literal scrutinee, and for if
definition   f(args) == body[params := args]     the defining equation of a math function (section 8.4)
range        a for over lo..lo equals its initial state;
             a for over lo..hi with lo < hi equals its body applied to the for over lo..(hi - 1)
literal      op(literals) == literal      by native evaluation, below
~~~

The elaborator inserts the let, projection, case, and literal steps silently, which is how the surface type identity of section 3.1 is obtained. Definition steps are inserted only on request: by unfold and fold, or by a hole within its budget. From the case axiom and a match that returns a Prop, the kernel-level prelude derives that distinct constructors of an enum are unequal and that each constructor is injective; both are needed to use the negative evidence a match arm receives (section 5.2).

The model of u8. A u8 is a natural number below 256. The kernel provides to_nat: u8 -> Nat, and defines every primitive through it:

~~~
a == b                 to_nat(a) == to_nat(b)
a < b, a <= b, ...     the corresponding order on to_nat(a), to_nat(b)
a.wrapping_add(b)      the u8 whose to_nat is (to_nat(a) + to_nat(b)) mod 256
a.wrapping_sub(b)      the u8 whose to_nat is (to_nat(a) + 256 - to_nat(b)) mod 256
~~~

The bool-valued runtime comparison a < b and the proposition [a < b] are connected by a reflection lemma per operator, stating that the bool is true exactly when the proposition holds. The u8 ordering lemmas of the kernel-level prelude (such as lt_of_le_of_ne and step_stays_below's ingredients) are proved from this model by ordinary reasoning and Nat induction. None is an axiom, and none depends on exhaustive enumeration of several variables.

Primitive evaluation. On literals, the kernel evaluates bool and u8 operations natively. This native implementation must agree with the Nat model; that agreement is part of the trusted base and is tested exhaustively, which is feasible for one-byte and two-byte operations. An evaluation rule builds on it: a closed logical term of type bool may be evaluated by the kernel, unfolding math functions as it goes, and evidence of [term == true] is accepted when the result is true. This evaluator of closed terms is the only place the kernel computes; it is a big-step shortcut for a chain of the axioms above, and it plays no part in comparing terms. A claim of the form forall (x: u8) { b(x) } with a computable bool body can be established by evaluating all 256 cases. Evaluation is a convenience for closed and single-variable facts; it is not the foundation of u8 reasoning.

Kernel-level prelude. Lemmas that must apply at every type (eq_symm, eq_trans, congruence), and the u8 theory over the Nat model are written in the kernel's own term language, where type parameters and Nat are available, and are checked by the kernel like everything else. They are not trusted.

Proof terms are checked and then discarded. Results are cached by content hash so that unchanged declarations are not rechecked.

### 12.3 What a hole is guaranteed to find

A hole's behavior must be predictable, so its search is fixed and bounded:

1. a fact already in scope, including branch and arm evidence, up to the type identity of section 3.1; and reflexivity;
2. propositional reasoning over &&, ||, =>, true, and false using facts in scope;
3. rewriting with equalities in scope and congruence (a proof-producing congruence closure);
4. unfolding of math function definitions;
5. evaluation of closed terms by the kernel.

A hole does not search the prelude. In particular it does not do machine-order or arithmetic reasoning: it will not derive i < limit from i <= limit and i != limit. Such steps are written as lemma calls, as in sections 8.1 and 10.4. Every example in this document uses a hole only for a goal within these five tiers. A mechanism that lets a hole apply selected lemmas is a possible later addition (section 15.2); it is deliberately absent here so that what a hole can do is easy to state.

Anything beyond the tiers is an explicit step: a lemma call, a match, a rewrite, unfold, or fold form (section 8.4), or a named decision procedure. Expected named procedures are linear arithmetic once mathematical integers exist, and bit-level reasoning for wider machine integers. Each emits kernel terms or a certificate for a small checker; if a certificate checker is added to the trusted base, that is recorded explicitly.

Resource policy. Every limit is a count of deterministic steps, never elapsed time, so that whether a program is accepted does not depend on the machine that checks it. A hole has fixed budgets for: the depth of definition unfolding; the size of any intermediate term; the number of congruence-closure merges; the number of propositional case splits; and the number of cases evaluated, at most 65,536, which covers two byte variables and rules out three. Acyclic unfolding always terminates but can grow a term rapidly; the size budget, not termination, is what bounds it. Exceeding any budget is reported as "not found", naming the budget. The budgets are constants of a language version, and raising one is a compatible change.

When a hole fails, the diagnostic reports the Locus-level goal and the facts that were in scope, and distinguishes "not found" from "refuted by evaluation".

Two tooling consequences are intended:

- An "expand hole" action replaces a solved hole with the explicit steps that were found.
- Found proofs are recorded, in the manner of a lockfile, so that rechecking does not repeat the search and a change to the search procedure cannot break a proof that was previously accepted.

### 12.4 The trusted base

- The kernel, including native Nat, bool, and u8 evaluation and its agreement with the Nat model of u8.
- The translation of a program into kernel statements: the generation of the per-path statements of section 6.3, and the logical definitions given to primitive operations.
- Erasure and Rust generation, and the Rust toolchain beneath them.

The prelude, including its kernel-level part, is not trusted: its lemmas are checked like any other. The hole solver and all other automation are not trusted.

## 13. Surface grammar sketch

This is a complete construct inventory for the selected fragment, with conventional token details abbreviated. It intentionally leaves automation implementation and diagnostic wording outside the grammar.

~~~
Program       ::= Declaration*

Declaration   ::= [math] fn Name "(" Parameters ")" "->" Type Block
                | struct Name "{" Fields "}"
                | enum Name "{" Variants "}"
                | prop Name ["(" Parameters ")"] "{" PropVariants "}"
                | const Name ":" Type "=" LogicalExpr ";"

Parameters    ::= [Parameter ("," Parameter)* [","]]
Parameter     ::= Name ":" Type

Fields        ::= [Field ("," Field)* [","]]
Field         ::= [Name ":"] Type

Variants      ::= [Variant ("," Variant)* [","]]
Variant       ::= Name ["(" Fields ")"]

PropVariants  ::= [PropVariant ("," PropVariant)* [","]]
PropVariant   ::= Name ["(" Fields ")"] [":" "@" ProofTarget]

Type          ::= bool | u8 | Prop | Name
                | "@" ProofTarget
                | "(" ")"
                | "(" Type ")"
                | TupleType
                | [math] fn "(" TypeParameters ")" "->" Type

TypeParameters ::= [Field ("," Field)* [","]]
TupleType      ::= "(" Field "," [Field ("," Field)* [","]] ")"

ProofTarget   ::= Name Postfix*
                | PropositionLiteral Postfix*
                | "(" PropExpr ")" Postfix*

Block         ::= "{" Statement* [Expr] "}"
Statement     ::= let Pattern [":" Type] "=" Expr ";"
                | Expr ";"

Pattern       ::= Name | "_"
                | true | false | Integer
                | "(" ")"
                | "(" Pattern "," [Pattern ("," Pattern)* [","]] ")"
                | Name "{" PatternFields "}"
                | Path ["(" [Pattern ("," Pattern)* [","]] ")"]

PatternFields ::= [PatternField ("," PatternField)* [","]]
PatternField  ::= [Name ":"] Pattern

Path          ::= Name "::" Name

Expr          ::= Primary
                | "!" Expr
                | Expr BinaryOperator Expr
                | Expr Postfix
                | if Expr Block else (Block | IfExpr)
                | match Expr "{" [Arm ("," Arm)* [","]] "}"
                | loop "(" StateParameters ")" "->" Type Block
                | for Name in Expr ".." Expr "(" StateParameters ")" Block
                | break Expr
                | continue "(" Arguments ")"

Arm           ::= Pattern "=>" Expr

Primary       ::= Name | Path | Integer | true | false | "_"
                | "(" ")"
                | "(" Expr ")"
                | TupleExpr
                | Name "{" ValueFields "}"
                | Block
                | PropositionLiteral

TupleExpr     ::= "(" Expr "," [Expr ("," Expr)* [","]] ")"
ValueFields   ::= [ValueField ("," ValueField)* [","]]
ValueField    ::= [Name ":"] Expr

Postfix       ::= "(" Arguments ")" | "." Name | "." Integer
Arguments     ::= [Expr ("," Expr)* [","]]

StateParameters ::= [StateParameter ("," StateParameter)* [","]]
StateParameter  ::= Name ":" Type "=" Expr

PropositionLiteral ::= "[" Formula "]"
~~~

Formula and PropExpr follow section 7. LogicalExpr is Expr checked in logical mode; it is not a separate token language. IfExpr is the if production above. A variant with a payload is constructed by applying its Path with the call Postfix. A let Pattern is restricted to the irrefutable forms (section 6), which include the constructor pattern of a single-variant, all-proof proposition. The rewrite, unfold, and fold forms of section 8.4 use call syntax with reserved names and need no grammar of their own; the first argument of unfold and fold is a function name.

Named function signatures, loop signatures, and for state lists are always explicit.

The longest tokens are recognized first. Operators bind, highest to lowest:

~~~
calls and projections
prefix !
comparisons == != < <= > >=
&&
||
=>  (right associative; proposition implication only)
~~~

Other binary operators are not in this fragment. Comparison chaining is rejected.

The token => now has two roles: it separates a match arm's pattern from its body, and it is proposition implication. This is unambiguous here because a pattern never contains an expression and there are no guards: the first => after a pattern is the arm separator, and any later one belongs to the body. It is nevertheless easy to misread, and it stops being unambiguous once guards exist. Section 15 records the open choice of a different implication token.

In runtime Boolean expressions, && and || short-circuit and can be understood through if/else. For Prop operands they construct logical connectives.

Named product construction is syntactically distinguished from a following control-flow block by the parser's condition/header context. This applies to the condition of an if, the scrutinee of a match, and the bounds of a for. Parentheses may disambiguate a constructor used directly in those positions. This is a parsing convention, not a change in typing.

Identifiers, whitespace, decimal literals, and comments can retain the current frontend's lexical rules. The words math, prop, in, and match need only be reserved where the grammar expects them. Arrays are outside this selected fragment, so a bracketed expression is always a proposition literal containing exactly one formula, whether or not an expected type is present: let claim = [n != 0]; binds a Prop. There is no array reading to default to. How brackets are shared with arrays later is open (section 15.2).

## 14. Minimum checks an implementation must enforce

A conforming implementation must reject:

- A use of an unbound or out-of-scope name in a type or proposition.
- A named projection or labelled constructor field on a tuple; tuple names are type-local binders only.
- Treating a proof about a shadowed binding as a proof about its replacement.
- A field dependency on itself or a later field.
- A field, payload, or result type in which a value selects runtime data layout instead of appearing inside a proposition.
- A named global struct, enum, or prop that implicitly refers to a function-local parameter.
- A declaration cycle: a recursive or mutually recursive function, enum, or prop.
- An unbracketed Boolean expression supplied where a proposition literal is required.
- A bare predicate function supplied where a proposition is required, without application.
- A proof hole that has no expected proof type or cannot be solved.
- A proof type supplied where a different proof type is expected, when the two are not identical under the type identity rule of section 3.1, even if one implies the other.
- Calling an ordinary fn from a proposition or a logical proof computation.
- A loop expression inside a math fn or any logical expression.
- A non-exhaustive match.
- A match on a proof whose result type is not a proof type, except a match with no arms.
- A match on a proof whose scrutinee or any arm is not a total logical computation, even when every arm has a proof type.
- A pattern in an executable match that inspects a ghost position with anything other than a name, a wildcard, or an irrefutable all-proof constructor.
- A prop variant whose conclusion is not an application of the proposition being declared, or whose payload mentions that proposition.
- Treating an expression as erasable because its result is ghost: a potentially divergent call is retained whatever its result type.
- A bounded for without evidence that its bounds are ordered.
- A hole accepted or rejected on the basis of elapsed time.
- Opening an existential proof with let, or letting its witness appear in the result type of the arm that opened it.
- Branching on Prop or proof values at runtime, or any other flow from a ghost binding into executable data or control.
- Extracting executable witnesses from logical existential proofs.
- Returning a type with an unbound block-local, arm-local, or loop-local dependency.
- Continuing a loop or bounded for without re-establishing its state proof fields.
- Assuming a loop parameter retains its initial value after a back edge.
- Assigning or otherwise choosing the index of a bounded for.
- Dropping a potentially divergent call merely because its result is erased.
- Accepting any logical fact that the kernel has not checked.

## 15. Deferred features and open questions

### 15.1 Deferred

The README milestones give the intended order. Each item below is outside this fragment.

- Logical lambdas: logic-only, capturing only immutable and ghost values, and erased. They remove the hand lifting of section 8.3.
- Mathematical Int and Nat, arithmetic operators whose overflow is a proof obligation over the mathematical value, further machine widths, and a linear-arithmetic procedure.
- Generics over types and propositions. Exists, Option-like types, and a library Ghost wrapper become declarable.
- Recursion, as one feature: recursive enums, recursive prop declarations, and structural recursion, where a recursive call is permitted only on a part bound by a match on the argument. Induction is then a recursive math function returning a proof. In the logic, Box is transparent: a recursive Rust enum using Box is seen as the plain inductive type, and the same holds for Rc, Arc, and shared references in the absence of interior mutability, because ownership guarantees such values are finite trees. Types that permit cycles through interior mutability are not inductive and need separate treatment.
- The ghost keyword on binders (section 2.4), and with it model fields. The semantics is already fixed by Prop fields (section 4.2): ghost data is part of the logical value, so s.model is a function of s, and two structs differing only in a ghost field are different values with one runtime representation. A consequence to carry forward: once runtime equality exists on structs, it compares representations, and therefore does not reflect logical equality for a type with ghost data fields.
- Dependent conjunction and implication, where the right operand is well formed only under the left, as in [i < len && a.get(i) > 0] once indexing demands a proof. This arrives with the first operation that takes a proof precondition, and is a reason to make && and => kernel formers at that point, not instances of the prelude And.
- Requires/ensures/invariant/assert as sugar over proof parameters, dependent results, and loop proof state.
- Mutation, references, ownership, and surface loops that elaborate to the state-passing loops of section 10.
- Termination measures, for total correctness of executable loops and for non-structural math recursion. Until then the workaround for the latter is an explicit fuel parameter.
- Trusted and external declarations, with the rest of Rust interoperability. Every such declaration is to be syntactically marked.
- Runtime closures.

### 15.2 Open questions

These are not decided. Each lists the current behavior of this document first. None needs to be settled before the kernel spike.

1. Proposition literals use brackets, [n != 0], and in this fragment a bracketed expression is always a proposition literal (section 13). Brackets are array syntax in Rust, and [n > 0] is a valid Rust array expression. The alternative is to drop the literal form: where a Prop is expected, an expression is elaborated as a formula, and the proof type is written @(n != 0).
2. Implication is spelled =>, which is also the match arm separator (section 13). The alternative is ==>.
3. Supplying evidence of Q where evidence of P is expected is an error unless the two are identical (section 3.1). The alternative is to treat the mismatch as an implicit hole, asking the solver of section 12.3 for P with the supplied evidence in scope.
4. Branch and arm evidence is anonymous. A naming form, such as if h: n != 0 { ... }, would let hand-written steps refer to it without a hole.
5. A hole does not search the prelude (section 12.3). A mechanism for marking lemmas that a hole may apply, with its own step budget, would shorten proofs at some cost in predictability.
6. The spelling of the chain form (section 8.4). One candidate is the bracketed form trans[a =(p) b =(q) c] used by the explicit refinement calculus.
7. Whether bounded iteration (section 10.6) should admit break, and whether a reversed range should be accepted as empty at the cost of a case distinction in the result type.
8. Whether a design rule should be adopted that Locus never gives valid Rust syntax a different meaning, so that a Rust superset remains reachable. Items 1 and 2 are the current violations; rust-features.md tracks them.

## 16. Semantic completion status

This is a coherent design specification built from established mechanisms, not a completed soundness proof or a fully formal executable semantics. It is detailed enough to guide a reference interpreter and checker, but several obligations must be made precise and validated before treating an implementation as trusted.

### 16.1 The intended model

The plan for soundness is a denotational model that adapts Ghalayini and Krishnaswami's explicit refinement types (section 17), whose development is mechanized in Lean 4 and can serve as a starting point. It replaces the earlier plan of embedding the kernel into Lean's logic. It is an adaptation and not a reuse: in that calculus a value is its erased value, which is not true of Locus.

Locus needs two layers, because a logical value cannot always be identified with its runtime representation. Claim { proposition: [true] } and Claim { proposition: [false] } have the same, empty, representation, yet the logic must tell them apart, since projection and equality substitution apply to them.

- A logical value keeps everything the logic can observe: executable data, Prop fields, and other ghost data. It keeps no proof contents; a proof field contributes only the requirement that its proposition holds.
- A type denotes a set of logical values. A proposition denotes a classical truth value, relative to the logical values of the variables in scope. Prop denotes the two truth values, so predicates and Prop parameters are ordinary functions and arguments. A function type denotes functions between the denoted sets, which is what gives equality at function types its meaning (section 7.2).
- Every type erases to a simple type (section 11.1), and a representation relation connects each logical value to the runtime values that represent it. For first-order data it is the function that forgets ghost data. For a type with no ghost data fields, such as NonZero, it is the identity, and the type is simply a subset of its erased type: the subset reading remains exact there, and is not claimed beyond it. For functions it relates a logical function to any runtime function that maps representations of arguments to representations of results.
- A math fn must denote a total function on logical values, represented by its erasure.
- An ordinary fn must erase to a partial function whose result, when there is one, represents a logical value of the result type. This is the partial-correctness reading of section 6.3. The cited calculus is total and lists divergence as future work; this clause is the part Locus must supply itself.

The model stays tractable because value dependence is confined to propositions (section 3): erasing a type never needs a value, and the representation relation is defined by recursion on simple types.

The two target theorems are corollaries of showing that every well-typed term's erasure represents a logical value in the denotation of its type:

- The logic is consistent: no closed math term has type @[false].
- Well-typed programs do not go wrong: erasure preserves observable behavior, including divergence, and no trap is evaluated.

### 16.2 Remaining obligations

1. Give program evaluation an exact operational semantics, and define precisely the per-path logical statements of section 6.3, including the continuation scope of evidence returned by potentially divergent functions.
2. Define elaboration from the surface into explicit binder identities, product and payload dependencies, and kernel terms, including the silently inserted equality steps of section 12.2.
3. Turn the rule inventory of section 12.2 into exact typing rules: the term language, the two recursion rules, the u8 model and its reflection lemmas, comparison with proof irrelevance, the computation axioms, the index-equation case rule, and the evaluation rule. Check each rule against the model of section 16.1. Until that is done, consistency is a goal, not a result.
4. State the upgrade-based ghost rules of section 2.4 as typing rules, and prove the three erasure properties of section 11.2.
5. Establish the usual scope/substitution and typing-preservation properties, and test the rejection boundaries with small counterexamples.

A small implementation can expose missing rules. Passing tests is useful evidence but is not a proof of consistency or preservation.

## 17. Relationship to Rust and a possible interoperability path

At this stage Locus is a small immutable, typed language with a proof system. Machine types, let bindings, shadowing, functions, tuples, named structs, enums, and match are familiar to Rust programmers. It does not yet contain Rust's ownership, borrowing, mutable references, resource destruction, traits, generics, or broad collection APIs. The explicit list of unsupported Rust features, of Locus constructs that are not Rust, and of places where the two currently conflict is maintained in rust-features.md.

The logical layer is closer to a restricted dependent type theory: propositions, evidence, and dependent proof fields are checked by a kernel. It is much smaller than Lean's exposed type system and libraries: there are no types indexed by values, no first-class types or universes, no type-level computation, and no quotients. What it keeps is roughly classical higher-order logic with declared data and declared propositions, plus proof-carrying data. The closest formal relative is the explicit refinement calculus of Ghalayini and Krishnaswami: refinements with explicit, erased proofs and ghost variables over a simply typed base, with no judgmental equality. Locus adds potentially divergent functions, declared propositions, propositions as values, and bounded automation. The runtime/math distinction is a Locus design choice; it should not be mistaken for a complete model of Lean's runtime facilities.

A practical proposed route toward Rust interoperability is:

1. Erase propositions and proofs as defined in section 11, generate ordinary Rust code, and compile it through Cargo. Ghost positions become the zero-sized markers Proved and Ghost, so the generated code keeps the shape of the source (section 11).
2. Introduce explicit Rust imports for supported data and function signatures. Foreign code starts as executable code; it does not acquire a mathematical meaning or produce trusted proofs merely because it is callable.
3. Extend ownership, borrowing, lifetimes, mutation, destruction, and panic/effect semantics as the supported interfaces require.
4. Add generics and traits with explicit rules connecting executable operations to their logical specifications.
5. If a separate native backend is desired, use deliberate ABI boundaries and generated wrappers instead of assuming arbitrary Rust binaries have a stable calling convention.

Generated Rust can use Rust's own source-level type and ABI decisions. An independent compiler must instead arrange explicit calling-convention and layout compatibility. Rust's default ABI has no stability guarantee, and its default representation leaves layout choices unspecified. Extern C and suitable repr(C)/transparent wrappers are possible boundaries for supported types; Rust tuples are not automatically C-FFI-safe.

Unverified Rust calls must not manufacture evidence. A property of a foreign result requires a checked Locus validation step, verification of the foreign implementation, or an explicit trusted contract. Proof erasure also means Rust can ordinarily supply a data representation without supplying its logical guarantee; exported proof-requiring interfaces therefore need an explicit trust or validation policy.

These are an interoperability direction and its constraints, not an implementation claim.

References:

- [Ghalayini and Krishnaswami, Explicit Refinement Types, ICFP 2023](https://arxiv.org/abs/2311.13995)
- [Rust let statements and shadowing](https://doc.rust-lang.org/reference/statements.html)
- [Rust enumerations](https://doc.rust-lang.org/reference/items/enumerations.html)
- [Rust match expressions](https://doc.rust-lang.org/reference/expressions/match-expr.html)
- [Rust external blocks and ABI guarantees](https://doc.rust-lang.org/reference/items/external-blocks.html)
- [Rust type layout](https://doc.rust-lang.org/reference/type-layout.html)
- [Rust representations and FFI](https://doc.rust-lang.org/nomicon/other-reprs.html)
- [Lean function types and binding](https://lean-lang.org/doc/reference/latest/The-Type-System/Functions/)
- [Lean inductive types and structures](https://lean-lang.org/doc/reference/latest/The-Type-System/Inductive-Types/)
- [Lean propositional equality](https://lean-lang.org/doc/reference/latest/Basic-Propositions/Propositional-Equality/)
- [Lean quantifiers](https://lean-lang.org/doc/reference/latest/Basic-Propositions/Quantifiers/)
