+++
id = "target-language"
title = "Target language"
group = "Vision"
created = "2026-09-21T21:03:06.000Z"
updated = "2026-09-23T02:47:11.000Z"
route = "vision/target-language.html"
order = 6
+++

# Target language

What Locus is meant to be, by topic. The detailed design is organized here; the latest compact logical-core decisions are linked below. It describes the language the projects are building towards, not what the compiler accepts today: that is [Language, as built](language.md), and the table under Language status is the difference between the two. Each section ends with the tasks that carry it.

The [logical computation and erasure one-pager](logical-core.md) summarizes the decisions from 22 September 2026, incorporated below. This is the agreed Vision, not a claim that the compiler implements it. Existing build tasks must be reconciled with these decisions before implementation; their identifiers below remain references, not evidence of completion.

## The rule about Rust

Where Locus and Rust both have a thing, it means in Locus what it means in Rust, and generated Rust must be what a Rust programmer would have written. Locus never gives valid Rust syntax a different meaning; what it adds is syntax Rust does not have.

What Locus adds to Rust, in full, so that the list can be kept short:

- Tokens: none. Every token Locus uses is a token of Rust, which is what keeps a locus! { ... } macro possible. One case needs care there: Rust reads pair.0.1 as a name, a dot, and the number 0.1, and splits it while parsing.
- Words: prop introduces a proposition declaration; logic introduces a logical function or block. Both are keywords of Locus, reserved outright and not contextual, so that logic { x } cannot be read as a struct literal of a type named logic (decided 22 September). There is no math fn or logic let. Every keyword of Rust is reserved. Exists, ForAll, Int, Nat, Bool, Seq, Map, Prop, Logical, and Model are ordinary library or type names; quantifiers need no keywords. prove!, prop!, old!, and recurse! use the built-in form spelling. Writing prop as an attribute on an enum was considered and declined (LOC-148).
- Grammar, where Rust's tokens are used in a way Rust's parser would refuse: @P as a type; names on the fields of a tuple type, as in (out: u32, @((out as Int) == (n as Int))), with each name in scope in the fields after it; logical connectives in the proof language; declarations of propositions with named arms whose witness fields are followed by => and a logical body; a qualified proposition constructor followed by @ and its body evidence, in expressions and patterns; and a function signature without a body in a header. Everything inside prop!(...), prove!(...), and the other forms is, to Rust, the argument of a macro, where any tokens may stand. _ where a value is expected is parsed by Rust and then refused, so it is given a meaning Rust does not have and takes none away.
- Attributes: the promises and derive. An attribute's contents are free-form in Rust.

The loop with a state list, continue with arguments, and the for with a state list were additions of the same kind, and go when let mut arrives.


## Propositions and evidence

Prop is a logical type of claims; @P is the type of evidence of a particular claim. Constructing a proposition does not prove it. Predicate values are logical functions or closures T -> Prop. A prop declaration defines a named inductive predicate and its proof constructors; a logic fn returning Prop defines a logical calculation. Both are supported and have different proof rules.

prop!(P) takes a proposition expression: a logical Bool is lifted through holds, which maps logical true and false to the corresponding primitive propositions, and an expression that is already a Prop, such as a predicate application, a connective of propositions, or an equality on logical data whose equality is a Prop, stands as it is. prove!(P) takes the same kind of expression, so the two forms have one grammar (decided 22 September; the earlier statement that prop! took only a Bool was inconsistent with prove!). holds need not evaluate a symbolic condition while checking the program. Existing Prop values can be returned directly and combined through logical connectives without wrapping them in prop!. The connectives &&, ||, and ! mean three things by the type of their operands: short-circuit operations on runtime bool, total operations on logical Bool, and conjunction, disjunction, and negation on Prop; => is implication on Prop only. Its implementation may be a compiler built-in rather than a user macro; this choice changes neither capture nor checking.

Every machine integer type has Int as its default model and bool has Bool. In a logical context, which is a formula, a predicate arm, a logic fn or logic block, or a logical operand, a machine value is observed through its default model with no cast written: for x: u8, prop!(x > 3) means prop!((x as Int) > 3), and both spellings are valid and mean the same. as M with another logical M, such as as Nat, picks that model instead. This is a per-type Model resolution decided at the operator, not a mode change of the value, and it is the one fixed bridge the language has between machine values and the logic (decided 22 September, replacing "there is no implicit choice of model"). Nat and Int are both available to the programmer. Proof types may name a proposition, apply a predicate, or use the shorthand @(e), meaning @prop!(e). Brackets remain available for Rust arrays. The precise connective spelling is independent of this literal rule.

Evidence is accepted only for the expected proposition, after the permitted computation steps: substituting immutable lets, projecting written tuples/structs, matching written constructors, and evaluating literal operations. These steps are fixed, not proof search. Function definitions unfold only when requested; prove!, rewrite!, unfold!, and fold! expose the other steps. Opening dependent patterns types each component over the newly bound names, so evidence returned with data refers to the value the caller received.

### Quantifiers are library propositions

The following signatures are schematic pending generics and logical callable syntax. Exists(P) has a Witness(value: T) arm whose body is P(value); construction is Exists::Witness(value) @ evidence, with evidence: @P(value). Matching its evidence introduces the witness and the proof for that witness inside proof reasoning. The witness cannot escape by making the match return arbitrary logical data; Logical alone does not authorize proof elimination.

ForAll(P) has an Each arm carrying prove_each: logic Fn(x: T) -> @P(x). Its body can be the primitive True proposition, with the ordinary explicit @ evidence slot discharged trivially. Within proof elimination, the recovered proof-producing function can be applied at value to establish P(value). Any more general projection of that function must satisfy the kernel's proof-irrelevance rules; an unrestricted logical-data projection is not implied. A proof function is checked once for an arbitrary symbolic argument, not run once for every value. Logical predicate closures capture immutable logical values as other logical expressions do.

No quantifier-specific kernel node or keyword is required if the kernel supports the necessary inductive propositions and dependent proof-returning functions. This does not remove the dependency in the function's result type. Primitive truth, falsity, and the general proof-construction/elimination rules still need an independent foundation. Defining ForAll(P) as Not(Exists(x => Not(P(x)))) is optional derived reasoning, not its definition: recovering P(x) needs double-negation elimination in general, or decidability of P(x). The existing classical foundation can supply that lemma explicitly.

Until the library form exists, which needs generics and logical closures, the built spelling forall (x: T) { F } and exists (x: T) { F } stays, as keywords; whether the keywords remain as sugar for the library propositions is decided when those exist, on how a header reads (decided 22 September).

Plan: revises the Vision for LOC-88, LOC-77, LOC-115, LOC-140, LOC-139, LOC-147. Existing implementations of formula keywords remain recorded under Now until migrated.

## Predicates with named arms

Agreed Vision direction. Named predicates use enum-like prop declarations whose named arms each contain a logical block ending in a Prop. This is the constructor-based predicate form; logical functions returning Prop remain available for transparent logical calculations. It is a future source-language change, not a statement that the parser or kernel already implements it. The language as built remains the record of current behavior. Recursive examples below require the inductive extension.

### Declaration and meaning

~~~rust
// State is a logical model type; Transition produces a Prop.
prop Reachable(from: State, to: State) {
    Same => {
        prop!(from == to)
    }

    Via(middle: State) => {
        Transition(from, middle) && Reachable(middle, to)
    }
}
~~~

A predicate application, Reachable(a, b), is a value of type Prop. @Reachable(a, b) is the type of evidence establishing that claim. Each arm describes one sufficient reason for the enclosing predicate to hold. Its body produces the proposition that must be established, not the evidence that establishes it.

The constructor associated with an arm takes its explicitly declared witnesses and one evidence argument for its body's proposition, then returns evidence of the enclosing predicate at the header's arguments. The source spelling keeps this evidence outside the witness fields, after @. It is not an undeclared final tuple field or a generated named field. Header arguments are inferred from the expected claim where possible; they are not additional runtime inputs. The witness middle in Via is a logical value, and the caller must also supply evidence of both Transition(from, middle) and Reachable(middle, to).

~~~text
Arm(witnesses) => { body: Prop }

source construction:

EnclosingPredicate::Arm(witness_values) @ evidence

checked meaning, with header arguments and witness values substituted:

evidence: @body  ->  result: @EnclosingPredicate(header_arguments)
~~~

Predicate parameters and arm witnesses are in scope in the body. An ordinary let inside logical code binds a logical value used by the final proposition; it does not leave a reference to a runtime local that might later disappear or change. Runtime values enter these expressions through the existing value/snapshot rules. A proposition literal may capture those values without a closure annotation.

Arms support all three enum-style witness shapes. The => before the body is required for each shape, separating named fields from the block that constructs the condition:

~~~text
UnitArm => { condition }
TupleArm(first: T, second: U) => { condition }
NamedArm { first: T, second: U } => { condition }
~~~

Tuple witnesses are supplied and matched positionally; named witnesses use Rust-style field initializers and field patterns, including field shorthand and renaming. The logical body can refer to all witnesses by their declared names. As with other dependent fields, a witness field type may refer to predicate parameters and earlier witness fields, not later fields; the body is checked after all fields are in scope. Braces before => declare fields; braces after => contain the logical calculation. The arrows in declarations do not themselves assert implication.

Every arm has an explicit name chosen by the programmer: Same, Via, Bounds, At, and so on. There is no case keyword, bare-body shorthand, implicit arm, reserved Default arm, or anonymous _ arm. _ requests evidence in an expression with an expected proof type and ignores a component in a pattern. A name such as Default can be used as an ordinary arm name, with no fallback behavior. A declaration with no arms has no proof constructors.

### Construction, matching, and alternatives

~~~rust
prop Either(p: Prop, q: Prop) {
    Left => { p }
    Right => { q }
}

prop Between(lower: Int, value: Int, upper: Int) {
    Bounds => {
        prop!(lower <= value && value <= upper)
    }
}

prop PrimeDigit(n: Int) {
    Two   => { prop!(n == 2) }
    Three => { prop!(n == 3) }
    Five  => { prop!(n == 5) }
    Seven => { prop!(n == 7) }
}
~~~

An arm may return an existing Prop such as p; a formula is introduced with prop!(...). PrimeDigit uses equality obligations to express the particular indices that its constructors establish. The conjunction in Bounds is one evidence argument after @, not two extra witness fields or two implicitly searched-for proofs. Multiple premises are combined into the body proposition; the evidence slot always proves that complete proposition.

~~~rust
let hp: @p = ...;
let choice: @Either(p, q) = Either::Left @ hp;

let two: @PrimeDigit(2) = PrimeDigit::Two @ prove!(2 == 2);

let limits: @(0 <= n && n <= 100) = ...;
let bounded: @Between(0, n, 100) = Between::Bounds @ limits;

match choice {
    Either::Left @ hp => { /* reason with evidence of p */ }
    Either::Right @ hq => { /* reason with evidence of q */ }
}
~~~

These snippets illustrate the proposed forms; ellipses and comments stand for supplied evidence or subsequent reasoning. An irrefutable evidence pattern in a let may stand in ordinary code, as let (next, still) = step(...) does today, because it binds only logical values and erases entirely; a match on evidence is a logical expression whose arms produce proofs, and may appear wherever a proof is expected, in ordinary code included. Being Logical alone does not permit eliminating an irrelevant proof into data. logic { } is for logical computation, not for opening evidence (decided 22 September, provided the erasure rules hold: a runtime branch never depends on which arm matched). Matching a proof exposes that arm's witnesses and evidence for its body. A single-arm proof may be destructured by an irrefutable pattern. Proof matching belongs to logical reasoning and does not create a runtime discriminant or allow a program to inspect which proof was supplied.

Different arms are alternatives. They may overlap, be mutually exclusive, or leave some inputs without a proof. They have no priority and are not tried as an automatic proof search. An arm requiring false is unusable without evidence of false; an arm requiring true makes its enclosing claim easy to establish. Neither declaration asserts its body's truth. Adding an arm changes the meaning of the predicate and the cases its consumers must handle.

Given an arbitrary proof of the enclosing predicate, one cannot assume every arm's condition, or select a preferred condition without matching. A nonrecursive predicate is logically equivalent to the disjunction of its arms' conditions, existentially quantifying each arm's witnesses. This logical equivalence is not automatic equality of proposition values. Recursive predicates instead have the inductive interpretation: the least relation supported by their constructors, with the corresponding induction rule.

### Named fields, witnesses, and erasure

~~~rust
// Seq::get returns a logical optional value, here named Maybe.
// Maybe<T: Logical> is a logical enum; runtime Option keeps its tag.
prop Contains(value: Int, items: Seq<Int>) {
    At { index: Int } => {
        prop!(items.get(index) == Maybe::Some(value))
    }
}
~~~

~~~rust
let found: @(items.get(3) == Maybe::Some(value)) = ...;
let membership: @Contains(value, items) =
    Contains::At { index: 3 } @ found;

match membership {
    Contains::At { index: position } @ found => {
        // position: Int
        // found: @(items.get(position) == Maybe::Some(value))
        // Continue reasoning with these bindings.
    }
}

// A single arm also permits irrefutable destructuring, with field shorthand.
let Contains::At { index } @ found = membership;
~~~

Had At been declared as At(index: Int) => { ... }, the corresponding construction would be Contains::At(3) @ found and the pattern Contains::At(position) @ found. There is no hidden evidence argument in either set of witness fields. In both forms the pattern introduces witness bindings before checking the evidence pattern, so the evidence type refers to position or index, as bound by that pattern.

At takes a logical index and evidence connecting it to the logical sequence. Int has no runtime representation, and proof matching cannot recover an executable index. An operation whose caller needs an executable index returns a machine integer separately alongside evidence about its model.

Proposition values, proof values, and calculations used only to form these logical obligations have no runtime form. That does not erase executable work merely because it returns evidence: a function that clears a buffer and returns evidence that it is empty must still clear the buffer.

Ordinary fn may transport and return runtime data, propositions, evidence, or aggregates containing them. Logical computations that construct those values obey their own mode rules; an ordinary function does not become erasable because its result is logical. A logic fn returns a logical type. A proof's proposition index is not a separately passed Prop field. Predicate names have logical signatures such as (State, State) -> Prop, with no reflected syntax tree exposed to user code.

### Evidence separator and parsing

The chosen separator is @, in both construction and matching. The spellings using, with, where, and | are not alternatives. No new keyword or token is needed. This is new Locus grammar, not syntax accepted directly by Rust's parser.

~~~text
ArmDeclaration := Name WitnessDeclaration? "=>" LogicalBlock
WitnessDeclaration := "(" NamedTypedFields ")" | "{" NamedTypedFields "}"
ProofConstruction := QualifiedArm WitnessArguments? "@" EvidenceExpression
ProofPattern := QualifiedArm WitnessPatterns? "@" EvidencePattern
~~~

WitnessArguments and WitnessPatterns use the delimiter shape from the declaration: none for a unit arm, parentheses for positional witnesses, or braces for named witnesses. QualifiedArm is a path such as Contains::At, including the predicate name. The evidence expression is checked against the body's proposition after substituting the header arguments and supplied witnesses. The constructor then has the proof type of the enclosing predicate. An evidence pattern binds or destructures a proof of that same substituted body proposition; it is not a branch guard.

The @ evidence slot is explicit even for a trivial body or a unit arm. A programmer can write PrimeDigit::Two @ _ to request evidence through the existing proof-hole rules, but omitting the slot does not trigger synthesis. In a pattern, PrimeDigit::Two @ _ ignores the body evidence. No implicit evidence field is added to either witness shape. Diagnostics should distinguish a missing witness, a missing evidence slot, and evidence for the wrong body proposition.

Leading @ belongs to proof-type syntax: @P names a type. A proof value is written as an ordinary variable, call, built-in proof form, constructor application, or block returning evidence. There is no @ prefix on proof values and no @ @evidence spelling. An inline construction therefore needs no dedicated proof block:

~~~rust
let membership: @Contains(value, items) =
    Contains::At { index: 3 } @ {
        let found: @(items.get(3) == Maybe::Some(value)) = ...;
        found
    };
~~~

Use a qualified constructor path for this new suffix, including unit arms: Either::Left @ hp. Bare Left @ hp is not an alternative, because Rust already permits name @ pattern, which binds the whole matched value. That existing pattern keeps its Rust meaning. To name a whole proof and its body evidence together, write whole @ (Contains::At { index } @ found). The first @ is Rust's whole-value binding; the second introduces the body's evidence pattern.

Unparenthesized chains of the new evidence separator are not accepted. Parenthesize a nested proof construction or nested evidence pattern, for example Outer::Case(...) @ (Inner::Case(...) @ evidence), rather than relying on associativity. Here the ellipses denote the declared witness arguments. A block also provides an explicit boundary for a complex evidence expression. Rust's | remains pattern alternation and its ordinary expression uses; it is not an evidence separator.

### Logical bodies and called functions

An arm body is a logical block ending in Prop. It may bind logical values with ordinary let, construct logical data, branch on Bool, match logical values or permitted observations, call logic fn, and combine propositions. It need not be a single formula, decidable, or true. These are the same rules as logical functions and explicit logic blocks; no promise attributes are required on an arm.

Logical code is checked pure and total. It cannot call an ordinary fn, even one carrying runtime effect promises; mutate, consume, or drop runtime data; or transfer control out of an enclosing runtime computation. Runtime inputs are observed through the model and permission rules. Local logical mutation may be supported by translation to immutable state; recursion and loops require a justified terminating interpretation.

Logical if requires Bool; arbitrary Prop values do not become decidable conditions. Predicate arms describe alternative proof constructors, not runtime branches. Proof matching is a logical expression: an irrefutable evidence pattern in a let is allowed in ordinary code, and a match on evidence may appear wherever a proof value is expected; existential witnesses remain local to that proof derivation; witness construction in ordinary code remains permitted under the surrounding-context argument-evaluation rule.

A runtime operation may execute before a proposition is built. Its result and verified postcondition then supply observations and evidence; its body is not thereby callable in logic. Formulae involving machine values use explicit models, for example (n as Int) + 1. Ordinary wrapping_add is still a runtime method; a logical wrapping operation must be separately defined over logical integers.

### Inductive restrictions

A recursive predicate application in an arm names a claim; it does not run the predicate's body recursively to search for evidence. Reachable(middle, to) therefore needs no decreasing relation between middle and from. It does require checked inductive formation and elimination rules.

Recursive references, including indirect and mutual references, must occur strictly positively in constructor requirements. Initially keep these occurrences visible through a supported set of logical forms: conjunction, disjunction, universal and existential quantification, and the conclusion of an implication whose premise does not mention the recursive group. Reject occurrences under negation, in an implication premise, in tests of a proposition's truth or identity, or through a helper whose treatment of the recursive predicate has not been established as admissible. Double negation is not automatically acceptable under this conservative strict-positivity rule.

~~~rust
// Rejected: this constructor would require the negation of its own predicate.
prop Bad() {
    Contradiction => {
        !Bad()
    }
}
~~~

Negating an independent proposition is allowed. Computation on the data arguments of recursive applications is allowed when its helper is logically admissible. Arbitrary proposition transformations must not hide a forbidden recursive occurrence; either an accepted expansion/inductive rule justifies them or the declaration is rejected. These restrictions apply to the actual kernel constructor requirements, not just their source spelling.

Predicate recursion and recursive proof helpers are different obligations. The former needs positive inductive structure; a helper that recursively constructs evidence needs justified recursion, typically following subproofs. A circular proof is never admitted just because every reference has the right proposition type. The first implementation may remain nonrecursive; Reachable requires the subsequent positivity and induction work.

### What happens to fold and unfold

The enclosing predicate is a new proposition with named proof constructors, not a transparent function returning a formula. To establish it, prove an arm's condition and apply that arm's constructor. To use it, match on its evidence. Even a single-arm declaration is not automatically identical to its body. There is no defining equation that permits an arbitrary proof of a multi-arm predicate to unfold into one selected arm's condition.

Inside an arm's obligation, logical helpers still have their own equations where the existing visibility and kernel rules supply them. A proof can need fold!, unfold!, rewrite!, arithmetic lemmas, or induction to connect those expressions. Local lets and the fixed computation steps continue to use the existing evidence-matching rules.

~~~rust
logic fn successor(n: Int) -> Int {
    n + 1
}

prop IsSuccessor(out: Int, input: Int) {
    ByValue => {
        prop!(out == successor(input))
    }
}

// In a context containing out, input, and the supplied arithmetic evidence:
let reduced: @(out == input + 1) = ...;
let condition: @(out == successor(input)) = fold!(successor, reduced);
let result: @IsSuccessor(out, input) = IsSuccessor::ByValue @ condition;
~~~

fold! above rewrites the data-valued helper successor backwards along its defining equation. It is not folding IsSuccessor. Matching result exposes condition; unfold!(successor, condition) can recover the expanded arithmetic claim. An arm with only literal arithmetic or an exact fact in scope needs no fold. None of these operations searches the declaration's arms for a proof.

### Foundation and implementation boundary

This is a surface presentation of inductively defined propositions, as in [Lean](https://lean-lang.org/theorem_proving_in_lean4/Inductive-Types/#inductively-defined-propositions) and [Rocq](https://rocq-prover.org/doc/V9.2.0/refman/language/core/inductive.html). Each arm lowers to witnesses plus one proof parameter whose type is the arm's logical condition. The external @ evidence slot and tuple/named witness shapes only change the surface presentation of those checked arguments. Constructor application, proof matching, and eventually induction are checked by the kernel. It introduces no axiom asserting that a generated condition is true.

A prop declaration has this constructor-based meaning; a logic fn returning Prop is also allowed and uses its defining equation instead. Current source predicates, fixed-conclusion constructors, and fold/unfold uses need migration. The [Target examples](target-examples.md) demonstrate the intended construction and matching style; the Now documents continue to describe the existing implementation.

Before migrating the entire prelude, retain an independent foundation for primitive logical operations: defining And using its own &&, or True using a proof obligation that is already True, would be circular. The bootstrap handling of zero/multiple premises and the lowering of computed arm conditions belong in the implementation design. Recursive arms additionally require kernel support for positivity and induction, including supported nesting and mutual groups. This section does not claim those features are implemented or add unchecked recursive equations.

Plan: this refines the predicate/function boundary discussed in LOC-14 and LOC-73. Named-arm parsing and lowering, migration, prelude bootstrap, and recursive proposition support need follow-up tasks; recording the Vision does not complete the current core build.

## Values and types

- Runtime values move, as in Rust. A type is used more than once only if it is Copy, and Copy and Clone are asked for with derive, from the closed list under Built-in forms. Use of a moved value is an error from the start. The present behavior, where every type is silently Copy, accepts programs that are not Rust.

- The never type ! exists, with its coercion to any type. It is the type of panic!, unreachable!, todo!, of return, break, and continue, and the result type of a function that never returns.

- An integer literal takes the type expected of it, and i32 when nothing expects one, as in Rust. Suffixes (255u8), and hexadecimal, octal, and binary literals are part of the core. String literals exist for the messages of the built-in forms.

- Type parameters on user types and functions require generics and type variables in the logic. Logical types have Logical-bounded stored elements and fields: Seq<Int> is valid, Seq<u8> is not. Runtime containers such as Option<@P> retain their runtime structure. Ghost<T> is not required by this model; classification belongs to the type, not the binding. Generic function mode is fixed by its declaration; see Logical types, models, and erasure.

- Enum variants with named fields are part of the core. Explicit discriminants, tuple structs, unit structs, and the newtype idiom are polish.

- A const item is emitted as a Rust const, and its initializer is limited to what Rust can evaluate at compile time. At present it is a function of no arguments.

- Logical values, including Int, Nat, Bool, logical collections, Prop, and @P, have no runtime representation beyond the singleton Erased marker. Their immutable captures can be reused without copying runtime ownership. Runtime structs and tuples may contain logical fields; their remaining fields retain ordinary ownership and representation. Logical and runtime enum tags must remain distinct.
- In a body, a newly formed proposition cannot mention a local that has been moved, as code cannot. Capture the needed claim or bind v as M before the move; that already captured value remains usable afterwards under Snapshot capture, scope, and stored claims. A signature is outside the body's lexical flow: a result type may mention any ordinary value parameter and means its value on entry. A &mut parameter instead follows the entry/return and old! convention under Mutation and references.
- Erasure removes checked logical computation, not every expression with a logical result. Ordinary calls returning proofs still execute, and runtime evaluation of arguments to logical calls is preserved. An explicit logical context admits no ordinary calls or runtime ownership effects; observations require live, initialized, legally readable data. The compiler checks mode and type constraints before erasure.

Plan: LOC-2, LOC-89, LOC-92, LOC-94, LOC-95, LOC-97, LOC-100, LOC-101, LOC-25, LOC-103, LOC-112, LOC-111, LOC-141.

## Snapshot capture, scope, and stored claims

Agreed rules for the Vision. A proposition captures immutable logical values, not variables, storage, or permission to access storage. Neither a proposition nor evidence of it borrows the runtime data it describes. These rules specify the meaning of capture and escape wherever those values are admitted; they do not mark the ownership, reference, or tracked-evidence implementation as complete.

### Capture at a program point

The checker keeps three separate things: source names mapped to resolved binding identities; initialization and access permissions for those bindings; and the current immutable logical value or version of each binding. Shadowing introduces a new identity. Assignment introduces a new version of the same identity.

Conceptually:

~~~text
capture(prop!(F), context) =
    resolve the free local names in F to binding identities;
    substitute their current immutable logical values into F,
    respecting the binders inside F.
~~~

This is symbolic substitution, not execution or a runtime clone. Values need not be known constants. Quantified variables remain bound by their quantifiers. A pure logical computation may describe the captured value, but a result of an ordinary executable call is represented by its returned value, not by pretending that the call is a total logical definition. An implementation may retain immutable bindings and terms rather than expand or copy them eagerly; this is not a user-visible, inspectable syntax tree of the proposition.

- Capture requires a legal observation at that point: each input must be initialized, not moved, and readable under the ownership and borrowing rules. An erased observation cannot bypass an exclusive loan or read inaccessible state.
- Capture consumes no runtime value, creates no lasting loan, and requires no Copy or Clone implementation on the observed type. All calls and operations used to form the claim obey Functions in propositions and the erased-expression rules.
- Once formed, a proposition's meaning does not change. Later assignment, shadowing, moving, dropping, or leaving the lexical scope of an input does not change the captured logical value. This does not permit a new occurrence of a moved or out-of-scope source name.
- Passing, copying, or storing a proposition preserves its captured values. It never captures them again at the destination. Replacing a mutable Prop binding gives that binding a new proposition value; it does not mutate an old proposition retained elsewhere.
- Evidence establishes the captured proposition. A proof about an old version establishes a claim about a new version only when the normal evidence-checking rules justify that claim; identical source spelling is not sufficient.

~~~rust
let mut x: u8 = 3;
let p = prop!((x as Int) > 0);
let h: @p = prove!((x as Int) > 0);
x = 0;
// p and h still describe the value 3, not the current value of x.
~~~

The logical reading is x0 = 3, p = prop!((x0 as Int) > 0), h: @p, then x1 = 0. The proposition and proof can outlive the storage that held x0. They cannot be used to recover that storage or its runtime contents. let before = x as M gives the historical model an explicit name. A typed model observation records both binding versions and relevant heap-state versions; the same pointer can describe different contents before and after a write.

### Scope escape is a type rule

Lexical scope controls which names a programmer can write. Logical binding controls which values a proposition denotes. The externally visible result type of a block, match arm, or function must be well formed in the receiving context. No inaccessible local binder may remain free in that type.

A dependency can be removed by substituting a justified total definition, related to an existing outer value by checked evidence, exposed through returned fields, or hidden by an explicit logical existential when returning only evidence. There is no implicit existential packaging and no invented equation for a potentially effectful or divergent call. Leaving a branch does not export all of its assumptions: only evidence actually justified along the returning paths is carried out, and the result type must still pass the escape rule.

A Prop value may capture local values without exposing those names in its type, which is simply Prop. A record containing that proposition and evidence of it is an explicit package. Its construction keeps all local bindings properly scoped in the checked representation; the caller sees projections of the record, not a dangling source name. This rule does not require captured values to reduce to literals.

### Proposition fields versus data invariants

~~~rust
struct Certificate {
    claim: Prop,
    evidence: @claim,
}

let certificate = {
    let x: u8 = 3;
    let claim = prop!((x as Int) > 0);
    Certificate { claim, evidence: prove!((x as Int) > 0) }
};
// certificate.evidence: @(certificate.claim); x need not remain in scope.
~~~

Field checking substitutes the supplied earlier field values into later field types. For Certificate, the evidence initializer must prove the supplied claim. Projection yields evidence of certificate.claim. A bare proof whose exposed result type still mentions an inaccessible local would fail the escape rule; including claim as a field supplies the binder needed here. Proposition and proof fields retain their logical dependencies and have no runtime proof contents.

A data invariant declares a different relationship:

~~~rust
struct NonZero {
    value: u8,
    evidence: @((value as Int) != 0),
}

struct Annotated {
    value: u8,
    claim: Prop,
    evidence: @claim,
}
~~~

Every NonZero must contain evidence about its own value field. That field name in the declaration is a binder instantiated separately for each constructed value, not a snapshot of some variable in the declaration's surroundings. A mutable NonZero cannot change value alone or lend that field through &mut; construct a whole replacement with evidence for the replacement value. An old proof copied out remains a fact about the old field value, not a guarantee for the replacement.

Annotated promises only that its stored claim holds. It does not promise that claim describes value. Even if one initializer sets claim to prop!((value as Int) != 0), later changing the independent value field does not alter that captured proposition. Replacing claim alone is prohibited because evidence depends on it; replace the record with corresponding evidence. Field dependencies come from the declared types, not a guess based on how a Prop happened to be initialized. Mutable tracked evidence is a local/parameter feature, not a mechanism that lets a struct temporarily violate its declared field types.

When a consumer needs an explicit name for the historical value, store a snapshot with its proof:

~~~rust
struct HistoricalNonZero {
    value: Int,
    evidence: @(value != 0),
}
~~~

Here value is a logical Int captured from a byte through its model. The snapshot supports later reasoning, not runtime recovery of the old byte. The record can derive Logical because both fields are logical.

### References and access permissions

For the supported reference types, capture records the immutable logical value observed at that point. It does not retain a reference that can be dereferenced after its loan ends. Runtime references stored in ordinary data fields still obey their own lifetimes; erasing a proposition does not relax those rules.

A historical fact is not a resource capability. Evidence that a file was open or that memory contained some data does not grant current access after closure or deallocation. Interior mutability, concurrent state, and resource permissions require their own logical models and access rules when introduced. Snapshot capture does not silently admit such features or turn their permissions into freely copyable evidence.

Diagnostics should distinguish a forbidden observation at capture, a result type that leaks a local binder, and a proof about the wrong version. When spelling alone would hide a version difference, show the capture or assignment location, for example x as it was before line 12.

Plan: elaborates the capture, scope, and field rules for LOC-81, LOC-97, LOC-89, and LOC-16. Implementation acceptance cases: capture before move versus a new observation after move; shadowing; a stored claim escaping a block versus a bare proof with a free local in its result type; a claim unrelated to a record's data; replacement of a dependent field; and a historical snapshot used in later reasoning. None of these rules requires runtime snapshots or proposition lifetime annotations.

## Names, visibility, and keywords

- Items and fields are private unless marked pub, and pub is part of the core. The generated Rust at present makes everything public; shipping that and adding privacy later would change what programs mean. pub(crate) and the rest wait for modules.

- Every Rust keyword is reserved: the strict ones and the ones Rust reserves for later (abstract, become, box, do, final, gen, macro, override, priv, try, typeof, unsized, virtual, yield). Identifiers reach the generated Rust, so a variable named type or move would break it. Rust's weak keywords stay identifiers, as they do in Rust. Locus reserves prop and logic outright; forall and exists remain keywords until the library quantifiers exist (Quantifiers are library propositions); other quantifier names are ordinary library names.

- Paths and namespaces follow Rust. A path has any number of segments, which u32::MAX, an associated function such as Lock::new, and modules all need. Types and values are looked up separately, so that a type and a function may share a name, as a struct and its constructor function do in Rust.

Plan: LOC-90, LOC-91, LOC-96, LOC-18.

## Expressions and control

- Early exit is part of the core: return. The check IR and lowering are designed with it from the start. The ? operator waits for the traits it is defined by.

- Operator precedence is Rust's table, exactly, including as and the unary operators. The implication arrow binds loosest.

- Runtime == and != use runtime comparison returning bool. Deriving runtime PartialEq is permitted only when the retained representation determines the equality being promised: logical data/Prop fields prevent deriving full logical equality from a runtime comparison, whereas proof fields are proof-irrelevant. Logical comparison is a separate operation returning Bool. Generic comparison bounds identify both the operation and its function mode; instantiating a generic never changes that mode.

- Runtime if requires bool and runtime match cannot discriminate on Logical values. Logical if requires Bool; logical match can examine logical values or legally readable observations without moving runtime ownership. An arbitrary Prop is not a Boolean decision procedure. Runtime match is to have the power it has in Rust: on integers, bool, and tuples, with literal, range, and or-patterns, bindings with @, guards, and exhaustiveness over integer ranges. This is in the batch on control flow, not in the core.

- Polish, after the core: omitting -> (), mut on parameters, field init shorthand and ..base in struct expressions, loop labels, MAX and MIN and a closed list of integer methods, range expressions including ..=, and passing doc comments and chosen attributes through to the generated Rust. usize and isize arrive with slices; their width depends on the platform, which the logic and determinism both have to account for.

Plan: LOC-93, LOC-98, LOC-99, LOC-102, LOC-86, LOC-69, LOC-104, LOC-105, LOC-106, LOC-107, LOC-108, LOC-109, LOC-110, LOC-111, LOC-112, LOC-116.

## Integers, in code and in propositions

Machine types remain runtime types. Int and Nat are logical types with no executable representation. Model implementations connect them: n as Int records an exact observation of the current machine value, and Int is the default model, so inside a logical context n alone means that observation and a cast is written only to pick another model (Propositions and evidence). Nat and Int are both available to the programmer. A logical helper operates on that model, not on an executable u8 passed under another name.

- Runtime arithmetic has Rust's overflow, wrapping, division, and remainder behavior. Runtime #[no_panic] turns the relevant safety condition into an evidence obligation; it does not make the operation callable in logic.
- Write prop!(n + 1 <= u8::MAX), or with the casts made explicit prop!((n as Int) + 1 <= (u8::MAX as Int)), for a mathematical bound; inside prop! the operands are observed through their default model and + is the total addition of Int. Even non-panicking machine comparisons and wrapping methods remain runtime operations. Their verified specifications relate their results to logical model operations. A runtime branch on n > 3 can supply evidence of (n as Int) > 3 through that checked relation; the source predicate does not execute the machine comparison again.
- Logical arithmetic and comparison are logic operations, returning logical types; comparisons produce Bool. Operator resolution identifies this mode. Nat + Nat interprets its literal operand as Nat; a logical result from an ordinary function does not change that function's mode.
- The specified logical Int division/remainder remain total and truncate toward zero, with quotient 0 and remainder a when dividing a by 0. The equation a == (a / b) * b + a % b holds; magnitude and sign bounds apply when b != 0. These are logical conventions, not executable integer operations.
- Runtime integer casts preserve Rust's meaning. An as cast whose target is a logical model invokes Model<T, M> observationally and is erased; it is not a runtime arbitrary-precision conversion. Logical-to-runtime extraction is not a cast supplied by the language.
- A normal return from a runtime primitive yields the facts justified by its specification. Under no_panic, an in-range sum satisfies (sum as Int) == (a as Int) + (b as Int). Otherwise only the specified wrapping relation is available across build modes. Operations that always panic when a condition fails establish that condition on normal continuation.
- The compiler's primitive table specifies types, runtime panic conditions, and the relation between inputs and output models. It remains on the trusted path. The reference interpreter and emitted Rust are compared under matching overflow settings. Logical comparisons/operators and runtime operator traits have separate signatures; user-defined runtime operator preconditions remain a design question when traits arrive.
- Float models, platform-dependent usize models, and other primitives need their own specified Model relations. Purity alone never admits an ordinary function into a logical context.

- The current arithmetic path gives Int native terms, literals, evaluation, and the axioms recorded in the Kernel contract; Nat supplies induction. This implementation choice does not require Int to remain a primitive language type. The library construction from Nat under Library-defined logical data is an intended option once the general machinery and arithmetic proofs exist; migrating to it requires reconciling the native evaluator and certificates with that definition. An ordered ring is not enough to be the integers, since the order of an ordered ring need not be discrete. The contract states: the ring laws and the order; discreteness, that nothing lies strictly between n and n + 1; for each machine type the view to Int, its range, the wrap from Int, and that they round-trip; and quotient and remainder that truncate toward zero, characterized by a == (a / b) * b + a % b, by the remainder being smaller than the divisor in size and having the sign of a, and by the value 0 for a zero divisor. The arithmetic procedure is scoped by the acceptance examples: linear constraints over Int, the facts about machine ranges, and division by a literal reduced to linear constraints through the characterization above, each reduction a checked step and not an assumption. Random tests check the evaluator and the models against Rust; they say nothing about whether the axioms are consistent, which rests on the integers being a model of them and on keeping the list short and standard.
- Linear arithmetic is checked by one kernel rule and found outside the kernel. A certificate is a goal and a list of pairs, each a kernel proof and a non-negative literal coefficient. Every constraint is the conclusion of a proof checked in the ordinary way, so the procedure that finds certificates asserts nothing on its own authority: a range fact is an instance of a machine model axiom, a quotient and remainder constraint an instance of the division axioms. The rule reads each conclusion and the negated goal as a linear expression over Int, with anything that is not +, -, a literal, or multiplication by a literal treated as an opaque atom; tightens strict inequalities by discreteness; and checks that the combination sums to a false statement about literals. The alternative, spelling the same reasoning out in ring and order axioms, keeps the kernel smaller by a couple of hundred lines and makes each line of arithmetic a proof of hundreds of steps, in a kernel that has no normalizer to shorten them.

Plan: LOC-11, LOC-12, LOC-3, LOC-9, LOC-118, LOC-119, LOC-120, LOC-121, LOC-122, LOC-123, LOC-124, LOC-145.

### The linear certificate

One kernel rule, linear, checks a certificate of linear arithmetic: a goal, a coefficient for it, and pairs of a kernel proof and a coefficient, summed to a contradiction. Its whole boundary, the three trusted steps, and one complete certificate for the last obligation of midpoint are in the Kernel contract under Linear arithmetic, where they were written before the rule was coded and moved when it landed.

## Effects are promises

Effects are a small closed list fixed by the language: not returning, panicking, allocating, and io, which is any interaction with the world, including time and randomness. There are no user-defined effects and no effect handlers, which would need runtime machinery that plain Rust does not have. Mutation is visible in types and is not an effect. Logical classification is a separate axis: effects describe runtime evaluation, while Logical determines erased representation. Function mode controls admission into logical computation. Classical reasoning is reported by an audit, not tracked as an effect.

An ordinary fn may have any effect unless it promises otherwise. A logic fn instead has checked purity and totality implicitly; it needs no no_panic/no_io/terminates attributes to assert those requirements. A promise is a built-in attribute, one per effect, named as Rust names its own promises of absence (no_std, no_mangle):

~~~
#[terminates]      always returns
#[no_panic]        never aborts
#[no_alloc]        never allocates
#[no_io]           never interacts with the world
~~~

What is guaranteed is what is written, which suits review of a specification: a reader looks for promises that are present, not for weakenings that are absent. An inner attribute at the top of a file, such as #![no_panic], makes a promise for every function in it. Runtime promises are explicit, not inferred, and constrain runtime callees. Logical callees satisfy their own stronger checked mode rules. Runtime promise attributes never turn an ordinary function into a logic fn. There is no attribute that bundles several promises. An obligation arises only where a promise is made: an operation that may panic in Rust, such as checked arithmetic or indexing when they arrive, needs no evidence in an ordinary function and needs evidence in one that promises no_panic.

Attributes are a closed, built-in set interpreted by the compiler. Locus has no user-defined macros and no user-defined attributes; its built-in forms are described under Built-in forms. Attribute syntax is chosen because it is Rust syntax.

A promise is checked against the body: every construct and every callee must keep it. A foreign declaration states its promises and is trusted. Only termination bears on the soundness of the logic, and the logic is protected by construction, since only total kernel terms appear in propositions. The other promises are claims about runtime behavior, enforced by a table from constructs to effects and a check at each call, in the check IR so that the check is on the trusted path.

- A panic is a third way for a function to end, beside returning and never returning. The interpreters report it, generated Rust is compared with the interpreter on it, and the statement that erasure preserves behavior speaks of it. Partial correctness is unchanged: a result type says what holds if the function returns. With &mut and protected types a further question arises, since a panic in the middle of an update leaves the caller's value half changed and a Rust caller that catches the panic can see it; the rule is the next point.
- A value whose fields carry evidence is never left broken where a panic could expose it. In the core this needs no rule of its own: a field that evidence in the same struct depends on cannot be assigned alone, the whole value is written by one assignment, and Rust evaluates the operands before it writes, so a panic leaves the old valid value (How mutation is checked, case 8). Should field-by-field update with a refresh ever be allowed, a function that opens such a window through a &mut parameter must promise no_panic. A value the function owns needs no rule: if the function panics the value is dropped unseen, since no Locus type has a destructor.

Plan: LOC-13, LOC-59, LOC-138.

## Functions in propositions

Only logic fn calls and primitive logical operations belong inside proposition literals, predicate arms, logical proof blocks, and explicit logic blocks. These contexts admit legal observations of runtime inputs through models. An ordinary fn remains ordinary even when it is pure, total, or annotated with every runtime promise. There is no math fn and no mode inferred from a generic instantiation or from a logical return type.

Logical functions return logical values and are checked pure and total. Their defining equations are available according to body visibility and the kernel's admitted computation rules; unfolding remains explicit. A logical function may return Prop, evidence, or other logical data. A prop declaration instead introduces named proof constructors and is used by construction and matching, not by unfolding to one selected arm.

A runtime function can return data with evidence, or only evidence while changing &mut storage. The result guarantee holds on successful return; termination and absence of panic are separate promises. A diverging ordinary function with result @False supplies no callable logical proof of False. Its call is evaluated before the logical result is used; verification records the result and postcondition at the post-call state. For example derive(mutate_and_prove(&mut x)) in ordinary code preserves the inner ordinary call and erases derive. Arguments are checked in their surrounding context. In logic { derive(mutate_and_prove(&mut x)) }, the ordinary call is rejected. No effects are hoisted out of logical conditions or across explicit logical boundaries.

Logical function calls/operators may appear directly in ordinary bodies. let count = items.len(); let next = count + 1; has logical Nat bindings without logic let. A runtime if still cannot branch on count's logical comparison result. A logical helper's own body cannot invoke an ordinary callback; callable signatures and generic bounds preserve function mode.

Plan: revises LOC-14, LOC-73, LOC-39. Bodies and effect promises in the current implementation remain recorded under Now until migrated.

This resolves the open decision LOC-193 recorded during the core build: a fully promised ordinary fn whose body holds an operator that may panic is not a term of the logic and does not enter it; a logic fn does, and it may not contain such an operator, since Int arithmetic is total.

## Mutation and references

Mutation and references, in tiers. Tier 0 is agreed; the full rules are to be written when it is specified in detail, together with Evidence about mutable state, which depends on it.

Tier 0 consists of two steps, in order. First, let mut locals and assignment to a variable or to a field path of one (x = e, lock.failures = e), which need no references at all. Second, &mut T and &T as parameter types, including self, with arguments written as paths (&mut lock, &mut pair.0, &lock), a reference parameter passed on to another call, and inherent impl blocks with method calls, without traits, since &mut self presupposes them. Not in tier 0: a reference held in a local, in a struct field, or returned; explicit lifetimes; a reference to a reference; a closure that captures one. A reference therefore lasts for one call, and no lifetime is ever written or checked. Values move from the start, as decided under Values and types, so a struct that does not derive Copy is moved when it is passed by value and lent when it is passed by reference; Box and Vec add heap storage, not moving.

- What the logic sees. A mutable variable is a sequence of versions, a new one at each assignment. At a join, a variable assigned on some path gets a new version, equal to a conditional term when the branches are pure. In a loop, nothing is known of a variable the loop assigns beyond what tracked evidence carries (Evidence about mutable state). Lowering turns versions into the state-passing forms the check IR already has. What becomes trusted is that translation.
- How a signature speaks of old and new. In parameter types a &mut parameter means its value at entry. In the result type it means its value at return. old!(x) names the entry value where the result needs both. In the logic the function takes the old value and returns the new value beside its declared result, so that a call gives its argument a new version and the evidence it returns is about that version:

~~~
fn bump(lock: &mut Lock, ok: @within_limit(lock.failures as Int)) -> @within_limit(lock.failures as Int)

ok = bump(&mut lock, ok);      // lock has a new version; the result re-establishes ok
~~~

- Aliasing. The &mut arguments of one call are disjoint paths, and disjoint from every other argument that reads the same variable: f(&mut a, &mut a) and f(&mut a, a.x) are rejected, f(&mut a.x, &a.y) is accepted. Locus is never more permissive than rustc, since the generated Rust must compile, and soundness rests on Locus's own check: lowering cannot write two new values back to one variable.
- Evidence. A parameter is bound as let is, so an evidence parameter is a snapshot of the entry state; mut ok: @P makes it tracked, which is Rust's own syntax for a mutable parameter. A field that a proof field of the same struct depends on is not assigned through a reference; the whole value is replaced.
- &T permits a read observation while the shared loan is valid. Model results are independent immutable logical values; the observational borrow ends after construction and never extends to the lifetime of a proposition. Through an active &mut reference, observe the referent via that authorized reference. Interior mutation and heap aliases require the state/permission rules described under Logical types, models, and erasure.
- Snapshot capture, scope, and stored claims specifies logical capture and escape; Evidence about mutable state specifies tracked availability, refresh, and control flow. How mutation is checked specifies the translation, including early return and values visible after a panic. Diagnostics identify versions by source locations, such as i as it was before line 12.

Tier 1, planned next: shared references in fields, returns, and locals, with lifetimes. The logic is unchanged, because a shared reference is still the value, provided types with interior mutability are excluded. Snapshot values do not depend on when a shared loan ends, but permission to create each observation must be checked by Locus: an erased observation does not reach rustc. Rust checks the retained executable references as well. This recovers parsers that hold their input, lookups that return a reference, and iteration over a collection.

Tiers beyond that are not planned, and what each would cost is known from the tools that have done it. A mutable reference in a local needs a loan analysis of Locus's own, because a borrow ends at its last use and the logic must know where the lent variable gets its value back; rustc's checker has no specification to match. A returned mutable reference breaks the reading of a function as old values to new ones, because the final state of self depends on what the caller later writes; the known answers are a second, backward function per function, or prophecy variables that name a reference's final value, and either makes specifications harder to read, which matters when the specification is what people review. A mutable reference in a struct makes the struct's logical value carry current and final values. Interior mutability, unsafe code, and concurrency need separation logic and are a different project. In each case the translation from borrows to values would sit in the trusted lowering. Every tier 0 program remains valid under the later tiers, and the old!(x) convention coexists with a notation for a reference's final value.

- To flesh out with references: matching through a & parameter binds references in Rust, and tier 0 has no place to hold one, so only Copy fields could be bound.

Plan: LOC-1, LOC-128, LOC-38, LOC-33, LOC-37.

## Evidence about mutable state

Loop invariants need no separate construct once mutable evidence can carry them. These rules specify the agreed source behavior; How mutation is checked describes its lowering and kernel obligations. They replace the loop-state spelling in the language as built as mutation is implemented.

### Snapshot bindings and tracked bindings

- Evidence bound with let is a snapshot. It keeps its proposition at the versions captured there and remains valid if those data change. The source name of the evidence itself still has ordinary lexical scope.
- Evidence bound with let mut is tracked. Its declared proof type is a template interpreted against the current versions of its resolved dependencies. For evidence, mut means both reassignable and tracking those current values; that extra meaning must not be hidden behind an analogy with ordinary fixed data types. An evidence parameter is a snapshot unless written mut, which makes it tracked by the same rule.
- A proof value never becomes false or changes its proposition. An invalidated mutable evidence binding is unavailable for its current obligation, like an uninitialized or moved binding. The old kernel proof remains a fact about the old values. No stale proof is silently retargeted.
- Reading valid tracked evidence produces an ordinary immutable proof value at that point. Copying it into a let, passing it as an ordinary proof argument, returning it, or storing it in a field carries that fact, not a subscription to the original variable. The destination must still satisfy its expected type and the scope-escape rule. Copying an unavailable binding is an error.

~~~rust
let mut n: u8 = 1;
let mut current: @((n as Int) > 0) = prove!((n as Int) > 0);
let original = current;                // snapshot of the established fact
n = 2;                                // current is now unavailable
// Reading current here is rejected, even though 2 > 0.
current = prove!((n as Int) > 0);               // explicit refresh for the new n
// original still proves the fact about n before the assignment.
~~~

### Dependencies and invalidation

Tracking dependencies are resolved binding identities in the declared proof type as written, not the transitive free variables of its expanded meaning. A captured proposition or immutable let-bound alias is a value and forms a snapshot boundary. A field path tracks its root binding in the initial implementation. A quantifier's bound variable is not a dependency on an outer variable with the same spelling, and later shadowing does not change which binding is tracked.

~~~rust
let p = prop!((n as Int) > 0);
let mut evidence: @p = prove!((n as Int) > 0);
n = 0;
// evidence remains valid: p is still the proposition about the earlier n.
~~~

With let mut p, replacing p invalidates evidence. Its template mentions p, so the new obligation is evidence of the new proposition. Existing immutable copies of the old p and its proof remain valid. There is no rule that follows p's capture back to n and makes it a live claim about n.

- Assigning a dependency, assigning one of its field paths, or updating it through &mut invalidates the tracked binding until an explicit refresh. Invalidation is conservative: the rule still applies when the new value happens to equal the old one or the proposition remains true. There is no automatic search to preserve availability.
- Calls check proof arguments against the versions current when the arguments are supplied. Their &mut state updates then invalidate dependent tracked bindings; returned evidence can re-establish them. This permits ok = update(&mut x, ok) when the entry proof is valid and the returned proof satisfies the post-update obligation, under the ordinary argument and assignment evaluation order.
- Refresh uses the ordinary assignment spelling, ok = evidence, ok = prove!(P), or ok = _. Its right side must supply evidence for the declared template at the versions current when the assignment completes. It cannot read an unavailable ok to justify itself. A failed proof does not restore availability. Reading a dependency that has been moved is still forbidden; evidence does not resurrect runtime values.

Availability and proof truth are distinct checks. The frontend requires refresh after every invalidating update. The kernel prevents an old proof from establishing an unjustified new claim; it need not reject reuse when the old and new propositions are equal under the permitted computation rules. Such a reuse still needs the explicit source refresh. This conservative flow rule is a language rule, not an additional logical axiom.

### Control flow and loops

Validity follows initialization analysis: a tracked binding is available after a join only if it is valid on every path that reaches that join. A branch that returns, panics, breaks, or continues does not reach that particular join; a break or continue must instead satisfy the obligations of its destination. Evidence no longer used or carried has no refresh obligation.

A fact that can be derived afresh on every iteration is an ordinary let inside the body. A fact depending on earlier iterations is tracked evidence carried in the loop's logical state. It must hold at loop entry and on every back edge on which it is carried, including continue and the end of the body. Every exit on which the evidence is needed must likewise supply a valid proof. A path cannot avoid an obligation by spelling its control transfer differently.

~~~rust
let mut lock = Lock { failures: 0, open: false };
let mut ok: @within_limit(lock.failures as Int) = _;
for attempt in 0..attempts {
    let (next, still) = step(lock, ok, event_at(attempt, correct));
    lock = next;        // ok is now unavailable
    ok = still;         // refreshed with evidence for the current lock
}
(lock, ok)
~~~

The checker performs the induction: carried data and evidence form the loop's state, as in section 10 of the language as built. Lowering gives data and evidence assignments fresh immutable versions, and the kernel checks the facts against the values at every state transition. Frontend availability analysis gives the early error; it cannot authorize a proof of the wrong current claim.

A for hides its iterator, so evidence declared before the loop cannot speak of the index, nor in general of what an iterator has produced so far. Facts the loop supplies afresh, such as the bounds of a range index, are unaffected. A carried fact about progress uses a named mutable iterator or index in a while or while let. No clause naming the hidden for state is planned.

Initial scope: tracked bindings in local variables and parameters; invalidation by whole root binding rather than by field path; and whole-value replacement for structs whose evidence depends on a changed field. The &mut entry/return convention is specified under Mutation and references. Diagnostics name the tracked binding, the update that invalidated it, and its current obligation; an immutable snapshot mismatch instead names the old and new versions.

Plan: LOC-16, LOC-127, LOC-28, LOC-30. Acceptance cases also cover copying valid tracked evidence before an update, rejecting a stale copy or self-refresh, refreshing after a truth-preserving update, an immutable captured p versus a reassigned mutable p, shadowing, and all reaching paths through joins and loop edges. The existing mutation cases remain the starting examples.

## Termination

Termination has two uses: optional total-correctness promises on ordinary functions, and the mandatory totality of logic fn and logical blocks. Until termination-checked loops are implemented, ordinary loops are possibly divergent and forbidden under terminates; logical loops are unavailable rather than unchecked. Runtime recursion can state its measure using the existing promise syntax:

~~~
#[terminates(decreases = n as Nat)]
#[no_panic]
fn sum_to(n: u8) -> u8 {
 if n == 0 { 0 } else { n.wrapping_add(recurse!(_, sum_to(n.wrapping_sub(1)))) }
}
~~~

- A function that does not call itself writes #[terminates] alone. A function that calls itself and promises to terminate must state the measure; it is not guessed. A header carries the bare promise, and the definition adds the measure, which is the argument for the promise and no part of the contract.
- The measure is a logical expression over parameter observations, never executed: for example n as Nat for an unsigned runtime input. Its type needs a supported well-founded order, initially Nat, then logical tuples and structurally recursive logical data. Runtime wrapping cannot cheat the decrease obligation because it is stated over the actual before/after models. Logical recursion uses the same checked decrease machinery; totality is already implicit in logic fn, so only its justification needs supplying.
- recurse!(evidence, call) is the slot for the evidence at a recursive call. It has the value of the call and erases to it. The first argument is expected to be evidence that the measure at the arguments is smaller than the measure at the parameters, so _ works there and a wrong proof is an ordinary mismatch reported at the call. A bare recursive call means recurse!(_, call). The form is spelled with !, as rewrite!, unfold!, and fold! are (Built-in forms). It is the rendering of the idea that a function already on the call stack receives, as an extra argument, evidence that it has gone down; outside callers never see that argument, and the old value is the current frame's own measure.
- A recursive call is a call to any function in the caller's own cycle of the call graph, which the ordering of declarations already computes; the programmer does not mark it, and recurse! around any other call is an error. The evidence is per call, not per function: each call from f to g shows that the measure of g at the arguments is below the measure of f at its parameters, in one order shared by the cycle. That is checkable within one body, against the signatures and measures of the others, and it implies what is wanted, since following the calls round any cycle back to f composes into a strict descent of f's own measure. The members of a cycle therefore live in one implementation unit, where their measures are visible, and their signatures are elaborated before any of their bodies. A function in a cycle that does not promise to terminate cannot be called by one that does.
- Mutual recursion: every function in a cycle states a measure, and at a call from f to g the slot of recurse! expects the measure of g at the arguments to be smaller than the measure of f at its parameters. The functions may have quite different parameters; it is their measures that must be comparable, because a cycle terminates exactly when every function in it can be mapped into one order without infinite descent. Two ways of being comparable are intended. Measures of one type from the list above, typically numbers or tuples of numbers, reached through conversions such as a size function once Nat exists; a final tuple component that ranks the functions lets an edge keep the first component equal, as with (n, 1) and (n, 0). And measures that are recursive data of any types, compared by the part-of order, which relates values of different types: a statement inside an expression is a part of it. That covers functions that follow mutually recursive data, with the evidence coming from the match that exposed the part. An order supplied by the programmer, with a proof that it has no infinite descent, is not planned; a cycle that needs one does not promise to terminate. Whether the first implementation includes mutual recursion is open.
- Computation meant to run forever is a function without the terminates promise that loops around a function with it; each pass finishes and carries its evidence.
- Logical recursion needs a kernel-supported interpretation, such as structural or well-founded recursion justified by checked evidence. Until available, reject unsupported logical recursion; do not install an unchecked defining equation. An ordinary recursive function remains runtime-only even when termination is proved. Its result and postcondition are available after execution; its termination annotation does not make it a callable logical definition.
- Consequences to keep in view. A function that promises to terminate uses stack in proportion to its recursion depth, since Rust does not guarantee tail calls, so recursion suits structural recursion and divide and conquer better than iteration over long input. Loops that keep the promise, by a finite range or a measure, are left for later; the kernel's rule for a bounded for (section 10 of the language as built.6) remains and is what they would lower to. The one trusted addition is the checker's rule that a strictly decreasing sequence in these orders is finite.

Plan: LOC-53, LOC-54, LOC-82, LOC-49.

## How mutation is checked

The typed tree stays shaped like the source, so that the generated Rust assigns where the programmer assigned. The check IR has no assignment. Trusted lowering gives each assignment a new version of the variable, and the check IR stays what it is today: lets, facts, calls, matches, and loops that pass their state. This section is the design, written before the code (LOC-129). It says what is added to the check IR, what lowering does, what is trusted, and works eleven cases by hand; each case becomes a test of the same name. It was reviewed and revised on 21 September. The 22 September logical-mode rules refine call classification and model observations below; the internal machine-value terms describe verification IR and do not make machine types source-level Logical types.

### What is trusted, and what watches it

- Trusted: the checker of the check IR; lowering, which here means versions, which variables a branch or loop assigns, the writing back of &mut arguments, and the disjointness of those arguments; and erase.
- Not trusted: the flow analysis in the elaborator, which decides whether tracked evidence is valid, whether a value has been moved, which evidence a loop carries, and which version a diagnostic names. Its mistakes cannot make a false claim pass. A use of stale evidence lowers to a proof about an old version where a claim about the current one is wanted. The kernel rejects a mismatched, unjustified claim; when the claims agree under permitted computation, a proof can still be logically valid even though the frontend requires an explicit refresh. The latter is a conservative availability rule, as specified under Evidence about mutable state. Retained executable moves are additionally checked by rustc. Observation permissions need Locus checking before erasure, because their reads do not appear in generated Rust.
- Lowering computes the set of variables a loop or branch assigns itself, and does not take it from the flow analysis. Leaving one out would keep facts about a variable the program has changed, which is the one way this translation can be unsound. The set is computed over binding identities, not names:
  - a binding made outside the construct and assigned anywhere inside it, nested blocks included, is in the set; so is one passed as &mut, or whose field path is;
  - a write to a field counts as a write to the root binding of the path;
  - a binding made inside the construct is local to it and is never in the set, since it has no value at entry; a loop with let mut scratch in its body does not carry scratch;
  - a binding that shadows an outer one is a different binding, so assigning to the inner leaves the outer out of the set, and assigning to the outer before it is shadowed puts it in;
  - the condition of a while is part of the loop, so an assignment or a &mut call in the condition counts.
  Each of these five has a small test of its own in M2 and M3, on the check IR that lowering produces and through the two interpreters.
- Lowering follows Rust's order of evaluation exactly, one subexpression at a time, and each subexpression sees the versions current when it runs: arguments left to right; in an assignment, the right side first, with everything it writes back, and the place last. A translation that is well typed and out of order is one the checker cannot catch, so the order is part of what is trusted, and the interpreters watch it (case 9).
- The two interpreters are compared on every program. The interpreter of the check IR runs the versions, the interpreter of the erased tree runs the assignments, and a variable missed by lowering shows as a disagreement on any input that reaches it. The random programs of R1 assign in branches and loops for this reason.

### Added to the check IR

Four additions and one simplification.

- A function carries its promises, all four. The checker enforces what erasure and the boundary with Rust rely on. A function that promises no_panic calls only functions that promise it, has evidence at every operation that may panic, and has evidence of false at every panic ending. A function that promises terminates contains no loop and calls only functions that promise it. A function that promises no_io, or no_alloc, calls only functions that promise the same, and uses no primitive that performs I/O, or allocates; the core has no such primitive yet, and the rule is there so that the first one cannot slip past. The checked IR records logical versus runtime function mode. Erasure removes logical applications after preserving runtime operand evaluation; it never removes an ordinary call merely for returning a logical value or carrying effect promises. Logical purity/totality and the runtime promises cannot rest on the elaborator alone. Recursion is already impossible, since a function is checked against those declared before it. The elaborator checks the same promises first, to give good errors; the checker's word is the one that counts.
- Return, as a way for a block to end. The value is checked against the function's result type in the context of that point, exactly as the value of the body is. It may stand anywhere, inside loops included, and like break it means the block produces no value.
- Panic, as a way for a block to end, with its message. It demands nothing, and under no_panic it demands a proof of false. Nothing follows it.
- A primitive operation that may panic, as a statement: let v = a + b for a machine type, from the table of K5. Its equation is the meaning that holds in every build: for + - * the wrapped result, for / and % the truncating result of the views. It may carry a proof that the panic condition is false; under no_panic it must. With the proof, the exact result is also known (the view of v is the sum of the views). Without it nothing more is known, except that for / and % the code that follows learns that the panic condition was false, since these panic in every build. The interpreters have two modes, as Rust has two builds. In the default mode every panic condition of the table panics. In wrapping mode the overflow of + - * and of unary minus wraps, and nothing else changes: division and remainder by zero still panic, and so do MIN / -1 and MIN % -1, as they do in every Rust build. Unary minus exists on signed types only; its panic condition is that the operand is MIN, and its wrapped result is MIN.
- The bounded for is simplified. Its state no longer depends on the index, because evidence declared outside a for cannot speak of the hidden index, so the state is a plain tuple as for loop, the proof that the bounds are ordered is dropped (an empty range runs no passes), and break is allowed with the loop's state. It is stated for every integer type. The body still gets lo <= i and i < hi afresh on each pass.

Nothing else changes. In particular a match in statement position already allows an arm to transfer control and produce no value.

### What lowering does

- Assignment. x = e becomes let x' = e, and later mentions of x mean x'. Assignment to a field path, lock.failures = e, becomes let lock' = Lock { failures: e, open: lock.open }. The new value is an ordinary struct value and must be well typed, so a field that an evidence field of the same struct depends on cannot be assigned alone: the rebuilt value would carry evidence about the old field, and the kernel rejects it. The whole value is replaced. This is the rule already stated under Mutation and references, and here it falls out of the translation.
- Tracked evidence. let mut ok: @P = proof records the declared template P and becomes the fact ok : P at the current versions. ok = proof becomes a new fact ok' : P at the versions current after evaluating its right side. A use reads the latest available fact; let saved = ok captures that fact and does not make saved track P. An assignment to x does nothing to the old proof in the check IR: it remains a fact about the old x. The frontend makes ok unavailable until refreshed, even if the update preserves truth; the kernel checks that any fact actually supplied establishes the expected current proposition. It need not reject a logically valid reuse merely to reproduce conservative frontend invalidation.
- What mentions means. Dependencies are the resolved binding identities in the declared template as written, with field paths tracked at their root; they are not textual names or the dependencies of a normalized expansion. A name bound by let is a value and a snapshot, so let p = prop!((x as Int) <= 3); let mut ok: @p = ...; mentions p and not x. An assignment to x leaves ok valid because p is still the proposition about the old x. If p is mutable, replacing p invalidates ok. Shadowing x or p introduces another identity and does not retarget existing templates.
- A branch that assigns has one representation. A conditional or match in which no arm assigns an outer binding is lowered as it is today: as a term when it is pure, which keeps its defining equation, and as a match statement otherwise. This is an internal IR representation test, not permission to call runtime functions in source logic. A term-representable branch uses modeled, total primitives: literals, names, fields, constructors, comparisons, the wrapping methods, conditionals and matches of the same kind, and admitted logical calls; ordinary calls remain explicit executable call nodes even when all runtime promises are present; and no operation that may panic, no loop, no assignment, and no return, break, continue, or panic. As soon as any arm assigns an outer binding, the conditional lowers to a match whose result is a tuple: the new versions of the variables assigned in any arm, then the tracked evidence that is valid at the end of every arm that reaches the end, typed over those new versions, then the value of the conditional. An arm that returns, breaks, continues, or panics contributes nothing, which is how a path that leaves does not count at the join. The condition or scrutinee is evaluated before any arm, at the versions current on entry. Nothing is known of a joined variable beyond what the evidence in the tuple says, even when both arms are simple; an exact equation for the join, where every arm is pure, could be added later and is left out now so that there is one translation to audit (case 10).
- A loop. The state is a tuple: the variables assigned in the body, then the tracked evidence declared outside and carried by the loop, typed over them. Every continue, and the end of the body, supplies that state at the current versions; so does the entry. The loop's result is a tuple of the same shape, plus the value of a break, plus the fact of the exit test when the loop is a while with a pure condition and no break. while c { body } is a loop whose body matches on c and breaks in the false arm. Which evidence is carried is chosen by the flow analysis (what is valid on entry and used afterwards or on a later pass); choosing too little loses facts and choosing too much leaves an obligation unproved at a continue, and neither is unsound.
- A call with &mut arguments. In the logic the callee takes the old values and returns a tuple of the new values of its &mut parameters followed by its declared result, whose type may mention them, with old!(x) for the entry value. At the call, lowering binds the result, then writes each new value back along its path as an assignment does. Lowering rejects two &mut arguments whose paths overlap, and a &mut argument that overlaps a path another argument reads. This check is small and is trusted, because two values written back to one place would let the logic keep the wrong one.
- What may be lent. A field that evidence in the same struct depends on cannot be assigned alone, and for the same reason it cannot be lent: &mut percent.value is rejected by lowering, as is any path through such a field. The callee would write to it at runtime before the caller could rebuild and recheck anything, and a panic in between would leave a broken value where a Rust caller can find it. &mut percent is accepted, since whoever holds a whole Percent can only replace it with a valid one. This rule, with the rule on assignment, gives an invariant that does not depend on which path a function leaves by: at every moment of execution, every value of a struct type satisfies the evidence in its fields. It is the argument for safety under unwinding, and it is about writes, not about returns.
- When a call panics. Writes made through &mut before a panic are real, and a Rust caller that catches the panic sees them. The logic needs nothing for this: partial correctness speaks only of normal return, and the invariant above is what protects values on the other path. The interpreters and the comparison with compiled Rust do need it, or a wrong write-back on the way out would go unseen. So a panic as an outcome carries the values of the function's &mut parameters at the moment of the panic. In the check IR, every point that may panic, a panic ending, an operation that may panic, or a call, is annotated by lowering with the current versions of the function's &mut parameters, and at a call these are written in terms of the values the callee reports for its own. The checker ignores the annotations; only the interpreter reads them, so a mistake in them is a false alarm in the comparison and never a false proof. The compiled harness reads the arguments after catch_unwind and compares. This arrives with references, in O3; until then a panic carries its message alone, and the outcome type is a struct so that the state can be added without touching its users.
- Moves and reinitialising produce nothing in the check IR. x = e after a move is an assignment like any other.

### The cases

**1. A branch that refreshes against one that returns early.**

~~~
x = x + 1;                          // ok, which mentions x, is now invalid
if x > limit { return (x, give_up(x)); }
else { ok = prove!((x as Int) <= (limit as Int)); }
use_it(x, ok);                      // accepted: the only path here refreshed ok
~~~

Lowers to let x1 = x0 + 1 (the primitive), then a match on x1 > limit whose true arm ends in return and whose false arm has the fact ok1 : x1 <= limit and ends in the value (ok1). The result of the match has type (@(x1 <= limit)). The checker demands the function's result type at the return, in a context that knows x1 > limit, and the type of the tuple at the end of the false arm. Flow analysis: valid after the join, since the returning path does not reach it. If the else arm is deleted, the analysis reports ok as invalid at use_it and names the assignment that invalidated it; if the analysis were broken and said nothing, lowering would pass ok0 : x0 <= limit where x1 <= limit is wanted and the kernel would reject the call.

**2. Several continue and break paths.**

~~~
let mut n = start;
let mut ok: @small(n as Int) = first;
loop {
    if done(n) { break; }
    let (m, still) = advance(n, ok);
    n = m;
    if skip(n) { ok = still; continue; }
    ok = still;
}
~~~

The state is (n: u32, ok: @small(n as Int)). The break supplies (n, ok) at the entry versions of this pass. The continue and the end of the body each supply (m, still), where still : @small(m as Int) by the opening of the pattern. Remove either ok = still and the analysis reports that ok is not valid at that continue or at the end of the body; broken analysis would supply (m, ok) and the kernel would find @small(n as Int) where @small(m as Int) is wanted. After the loop, n and ok are the components of the loop's result, and that is all that is known of n.

**3. Snapshot against tracked.**

~~~
let before: @((x as Int) <= 3) = h;          // a snapshot: about x as it is here
let mut now: @((x as Int) <= 3) = h;         // tracked: about x as it is wherever it is used
x = 9;
takes_old(before);                  // accepted, and its claim is about the old x
takes(x, now);                      // rejected: now was invalidated by x = 9
~~~

Both are the fact x0 <= 3 in the check IR. The difference is only in what a use means: before always means that fact, and the diagnostic prints its type as x as it was before line 3, while now means the latest refresh and is wanted at the current version. takes(x, before) is rejected by the kernel as a mismatch between x0 <= 3 and x1 <= 3, with the same diagnostic about versions.

**4. A captured proposition used as a tracked type.**

~~~
let p = prop!((x as Int) <= 3);
let mut ok: @p = h;
x = 9;
takes_p(p, ok);                     // accepted: p is a value, and still means the old x
takes(x, ok);                       // rejected: wants @((x as Int) <= 3) at the current x
~~~

p is let-bound, so it is the term x0 <= 3 for good. The type @p mentions p and not x, the assignment invalidates nothing, and that is right. The second call fails in the kernel as in case 3. With let mut p, an assignment to p invalidates ok, by the same rule.

**5. Move and reinitialise.**

~~~
let mut t = Token::new();
consume(t);                         // moved
t = Token::new();                   // initialised again
consume(t);                         // accepted
~~~

Two versions of t and two calls in the check IR; nothing about moves appears there. Remove the second assignment and the elaborator reports a use after move. Were the move analysis broken, the check IR would still check, which is sound because a value used twice is harmless in the logic, and rustc would reject the generated Rust with E0382. A new model observation of a moved or inaccessible value must also be rejected before erasure; rustc cannot check an observation that is absent from its input.

**6. A call that changes two disjoint fields.**

~~~
fn settle(a: &mut u32, b: &mut u32, le: @((*a as Int) <= (*b as Int))) -> @((*a as Int) == (*b as Int))

let done = settle(&mut pair.lo, &mut pair.hi, le);
~~~

In the logic settle takes (a, b, le) and returns (a', b', @(a' == b')). The call binds r, then let pair1 = Pair { lo: r.0, hi: r.1 }, and done is r.2, whose type after the opening of the pattern is pair1.lo == pair1.hi. settle(&mut pair.lo, &mut pair.lo, ...) and settle(&mut pair, &mut pair.hi, ...) are rejected by lowering as overlapping, and by rustc. If Pair carried evidence about lo and hi, neither field could be lent at all, and the programmer would pass &mut pair to a function that replaces it with a whole valid Pair.

**7. An operation that may panic.**

~~~
#[no_panic] fn next(n: u32, fits: @(n as Int + 1 <= u32::MAX as Int)) -> (m: u32, @(m as Int == n as Int + 1)) {
    let m = n + 1;                  // the obligation of +, met by fits
    (m, prove!(m as Int == n as Int + 1))
}
fn bump(n: u32) -> u32 { n + 1 }    // no promise: may panic, and only the wrapped value is known
~~~

In next, the primitive carries a proof that the sum of the views is in range, the checker demands it because of the promise, and afterwards both m == n.wrapping_add(1) and the exact equation are facts, the second of which is the claim. In bump the primitive carries no proof and the checker asks for none. A check IR built by hand that omits the proof under no_panic is rejected by the checker, whatever the elaborator thought. After let q = a / b, the fact b != 0 is known in either function, and under no_panic it had to be proved first.

**8. A protected value behind &mut when a panic unwinds.**

~~~
impl Percent {
    fn set(&mut self, v: u32) {
        assert!(v <= 100);          // may panic: before the write
        *self = Percent { value: v, in_range: prove!((v as Int) <= 100) };
    }
}
~~~

self.value = v alone is rejected, by the rule on assignment above: the rebuilt value is ill typed. The whole value is written by one assignment, whose operands Rust evaluates before it writes, so a panic comes either before the write, leaving the old valid value, or not at all. Nor can the field be lent: helper(&mut self.value) is rejected, because helper would write before anyone rechecks. No broken Percent exists at any moment for a Rust caller to find after catch_unwind, which is the invariant stated under What lowering does, and the no_panic rule of LOC-191 has nothing to forbid in the core. It stays recorded for the day field-by-field update with a refresh is allowed, when there would be a window between the write and the refresh. The test from the Rust side, which catches a panic from set and reads the survivor, stays in O4.

**9. The right side changes what the left side names.**

~~~
fn touch(p: &mut Pair) -> u32 { p.hi = 7; 3 }

pair.lo = touch(&mut pair);         // Rust leaves lo == 3 and hi == 7
~~~

The right side runs first, with its write-back: the call binds r, and let pair1 = r.0 is the Pair with hi == 7. Only then is the place updated, from the version current at that point: let pair2 = Pair { lo: r.1, hi: pair1.hi }. Rebuilding from pair0 would give hi its old value, would be perfectly well typed, and would be wrong; the checker could not tell. The test asserts lo == 3 and hi == 7 in both interpreters and in compiled Rust. The same order governs arguments: in f(pair.lo, touch(&mut pair)) the first argument is read before the call changes pair, and in f(touch(&mut pair), pair.hi) after.

**10. Both arms assign and refresh, and the condition reads what an arm assigns.**

~~~
if x < 2 {                          // the condition reads x on entry
    x = 1;
    ok = prove!((x as Int) <= 2);
} else {
    x = 2;
    ok = prove!((x as Int) <= 2);
}
use_it(x, ok);
~~~

A match on x0 < 2. The true arm knows (x0 < 2) == true, has let x1 = 1 and the fact ok1 : x1 <= 2, and ends in the value (x1, ok1). The false arm likewise with x2 = 2 and ok2. The result of the match is r of type (x: u8, @(x <= 2)), a tuple whose second part is typed over its first; each arm's value is checked against it, which asks for x1 <= 2 of ok1 and x2 <= 2 of ok2. After the match the current x is r.0 and the current ok is r.1, a proof of r.0 <= 2, which is what use_it wants. Nothing else is known of r.0; that it is 1 or 2 is not recorded, by the choice made above, and a program that needs it carries it as evidence in the same way. If one arm left out its refresh, the analysis would report ok as invalid after the join, and a broken analysis would put ok0, a proof about x0, in that arm's tuple, which the kernel rejects against x <= 2 at x1.

**11. A replacement that completes, and then a panic.**

~~~
impl Percent {
    fn set_then_fail(&mut self, v: u32, fits: @((v as Int) <= 100)) {
        *self = Percent { value: v, in_range: fits };
        panic!("after the write");
    }
}
~~~

A Rust caller, through a wrapper that needs no evidence, catches the panic and reads the Percent it lent. It finds the new value, valid. The logic has nothing to say, since the function does not return. The interpreters report the panic together with the value of self at that moment, which is the new one, and the compiled harness agrees. This is the test that a write before a panic is neither lost nor half done; with case 8, where the panic comes before the write, it covers both sides of the one assignment.

### Left open by this design

Diagnostics that name versions need the line of each assignment kept on the version, which is cheap. Tracked evidence is in local variables and parameters only, invalidated by whole variable and not by path. Whether a while with a break should still export a fact about its exit, and whether a join of pure arms should keep an exact equation, are left until an example wants them.

Plan: LOC-142, LOC-129, and M0 to M5 of the Build plan.

## What is generated

One Rust module per file under a generated root. All logical types and values use one singleton Erased marker where signatures or retained structure require a placeholder. Logical arithmetic/models add no runtime libraries or model allocations. Ordinary runtime data and its dependencies are unaffected; Int/Nat/Seq in this design have no runtime representation.

A marker that cannot be forged does not protect an export. Every proof erases to the same Rust type, so hand-written Rust can obtain an Erased marker honestly, from any function that returns one, and pass it to a function that wants evidence of something else: with increment and Percent::new both exported, safe Rust builds a Percent of 101. A marker for each proposition would not help either, since it still would not tie the evidence to the particular argument. So the boundary is drawn by what is exported, not by what can be constructed:

- A function that takes evidence, directly or inside a parameter whose type Rust could build, is visible to other Locus modules and never to Rust. It is emitted so that only modules under the generated root can see it.
- What Rust sees takes no evidence. It takes plain data and checks it at runtime, returning an Option or a Result, or it takes a validated type, one whose fields are private and whose every constructor Rust can reach validates. Returning evidence to Rust is harmless once nothing Rust can call accepts any.
- Erased has no public constructor. That stops casual misuse and is not the guarantee: a marker obtained legitimately for one proposition cannot authorize another. Export protection rests on the validated boundary above.

Visibility uses Rust's own restricted forms, pub(super), pub(in path), and pub(crate), and one checked rule: a function that takes evidence may be visible no further than the root of the generated modules. Plain pub means exported to Rust, and on a function that takes evidence it is an error that offers the two ways out, a narrower visibility or a validated type. In a crate that mixes Locus and hand-written Rust the programmer writes pub(in crate::verified), or pub(super) when the modules are siblings under the generated root; in a crate that is all Locus, pub(crate) is already right. Every spelling means what it means in Rust, and the surface Rust sees is what is marked plain pub. Not covered yet: one Locus crate calling the evidence-taking functions of another, for which Rust has no visibility; that waits for packages.

### Default cleanup of erased bindings

Readable, warning-clean marker handling is the default output policy, independent of rustc optimization level. After verification and erasure, compute use/liveness on the emitted representation and remove unused local bindings and dead assignments whose retained type is Erased. Repeat as needed when removing one alias makes another unused. A pure marker initializer disappears entirely; an initializer with runtime evaluation keeps that evaluation in place, with an explicit discard such as let _ = mutate_and_prove(&mut x); where appropriate. Do not remove the call because its result is a marker.

~~~rust
// Source: q is used only for subsequent erased reasoning.
let q = derive(mutate_and_prove(&mut x));

// Default emitted Rust after cleanup:
let _ = mutate_and_prove(&mut x);
~~~

Unused logical pattern bindings become wildcards when doing so preserves destructuring and lifetime behavior. Required parameters remain in the generated signature and use an underscore-prefixed name when unused; cleanup does not silently change arity or layout. Emit marker support only where referenced. Preserve evaluation order, panic/divergence, runtime enum tags, temporary lifetimes, and drop behavior. Do not replace this cleanup with a blanket allow(unused) that hides warnings in retained runtime code.

No locus build flag is required to obtain clean output. An internal/debug dump preserving pre-cleanup markers may be added if diagnostics need it; this is not a second execution semantics or a committed public flag. Acceptance examples cover cascading dead markers, a retained effectful initializer, proof-bearing destructuring, an unused retained proof parameter, and a runtime enum with an erased payload; generated fixtures should compile without marker-induced unused_variables, unused_assignments, or unused_must_use warnings.

Plan: LOC-137, LOC-135, LOC-19.

## Found proofs are stored

- Everything that fills a _ or a prove! is untrusted and produces a proof the kernel checks. That holds for the exact and computed steps, for the arithmetic procedure, and for any automation added later. Automation never widens what is trusted; only a new kernel rule does, and a rule is added only where ordinary proofs would be too large to be practical, as with linear arithmetic. The Kernel contract lists every rule.
- The proofs found are written to one file at the root of what was checked, Locus.lock, in TOML, and committed as Cargo.lock is (decided 22 September, replacing one file beside each source). The file has a version, then one [[file]] table per source file in path order with its path, and one [[file.obligation]] table per obligation in order of position, with the key, a reader-only location such as "run 2", the claim the stored proof concludes, and the proof as a block of named steps, one per line: t1, t2, ... bind terms and s1, s2, ... bind proofs, a later line may name an earlier one, shared subterms and subproofs are written once, and the last line is the conclusion. The names are given by the writer in first-use order, so two machines write the same text. The writer owns the file: it carries no comments and is rewritten whole. Other things a build wants to pin may live in it later.
- An entry is found by a hash of the obligation, which is the claim and the facts it may use as kernel terms, and not by a hash of the source text. Reformatting, comments, and edits elsewhere in the file leave entries valid. Definitions are referred to by name, not by number.
- The file is untrusted input. A stored proof is checked by the kernel against the current obligation every time, so a stale, edited, or wrong entry can cost a search and can never make a false claim pass. The hash is for finding an entry and carries no authority.
- locus check uses a stored proof where one is found, searches where none is, and writes what it finds. locus check --locked never searches: a missing or failing entry is an error. That is the mode for CI and for a reviewer, who then depend on the kernel alone, and it is what makes builds fast.
- A proof found by one version of Locus still checks under a later one, however the search has changed, as long as the kernel accepts the same rules. The format of proofs and certificates is versioned, and a change to it or to the kernel's rules is the one event that can invalidate stored proofs; it is announced, and proofs are found again.

## Logical types, models, and erasure

Locked Vision decision, 22 September 2026. A type is either Logical or runtime, never both. This supersedes executable mathematical Int/Nat/Seq, context-dependent Ghost<T> lifting, and admission of ordinary functions into logic through effect promises. The [one-pager](logical-core.md) gives the compact contract. The implementation and its historical kernel representations remain described under Now.

### Classification and generics

Logical is compiler-controlled and cannot be implemented unsafely. Int, Nat, Bool, logical Unit, Seq, Map, Prop, and all @P proof types are logical. u8, u32, usize, bool, and ordinary Rust data are runtime types. derive(Logical) requires logical fields and payloads; logical generic containers require logical stored element types. A Seq<Int> is legal and Seq<u8> is not. No runtime arbitrary-precision representation is required for Int or Nat.

Runtime structs/tuples may contain logical fields. Runtime enums and containers retain their tags, lengths, allocation, and other executable structure even with erased elements: Option<@P> is not itself Logical. Box<T> is always non-logical, including Box<Nat> and Box<@P>; see Library-defined logical data below. A logical enum has no runtime tag and can be matched only in logical code. Logical values are immutable observations for capture purposes and can be reused without duplicating runtime ownership; assignment to a logical binding creates a new logical value.

Function mode is declared, not selected by generic arguments. An ordinary identity<T>(x: T) -> T can transport a Nat in runtime code but cannot be called inside logic. A logical function returning T requires T: Logical. An executable generic max needs a comparison returning bool; its logical counterpart needs a logical comparison returning Bool. There is no implicit mode-polymorphic implementation.

### Library-defined logical data

Agreed Vision decision, 22 September 2026. Logical types may be recursive. With logical structs/enums, generics, checked inductive formation, structural recursion, and induction, Int, Seq, and finite maps can be defined in the library. A native Nat can be the starting data type; each collection or arithmetic type does not need its own compiler primitive. Prop, proof checking, and the general logical rules remain part of the foundation.

These examples are schematic library definitions, pending the inductive and generic extensions, not code accepted by the current compiler or duplicate declarations to insert beside today's prelude types:

~~~rust
#[derive(Logical)]
enum Int {
    NonNegative(Nat), // n
    Negative(Nat),    // -(n + 1): one representation of zero
}

#[derive(Logical)]
enum Seq<T: Logical> {
    Empty,
    Cons(T, Seq<T>),
}
~~~

Logical recursion needs no Box: a logical enum has no runtime layout to make finite. Its mathematical values are finite inductive values; erasure is not permission to introduce cyclic or infinite values. Recursive groups require checked strictly positive occurrences and supported nesting. Logical derivation checks the group together: nonrecursive payloads must be logical and generic stored elements must have Logical bounds. Recursive logical functions still need a justified terminating definition; induction supplies the corresponding proof principle. Unsupported recursive forms are rejected, rather than accepted on the strength of derive alone.

For example, a finite map can package its representation and invariant:

~~~rust
#[derive(Logical)]
struct Entry<K: Logical, V: Logical> {
    key: K,
    value: V,
}

#[derive(Logical)]
struct FiniteMap<K: Logical, V: Logical> {
    entries: Seq<Entry<K, V>>,
    unique: @UniqueKeys(entries),
}
~~~

UniqueKeys is a library proposition about entries; constructing a FiniteMap requires its evidence. Entry is explicitly logical because both of its fields are logical. Lookup and update are logic fn definitions with proofs, using an appropriate logical comparison interface for keys when a decision is needed. The choice between representation equality and extensional map equality must be explicit: different entry orders can describe the same mapping. Arbitrary, possibly infinite mathematical mappings are a separate abstraction, potentially logical functions K -> Maybe<V>, where Maybe is a logical optional type; supporting those needs logical function types, not a primitive Map declaration.

Even Nat can be presented as an inductive library type once the general machinery exists:

~~~rust
#[derive(Logical)]
enum Nat {
    Zero,
    Succ(Nat),
}
~~~

Native numeric literals, compact numeral representations, and arithmetic certificates remain useful. A native checking path must have a specified sound correspondence with the mathematical definitions; making types definable in the library does not require expanding every numeral into unary syntax. The existing native Nat/Int kernel path is not removed by this decision. Migrating native Int to a library definition is separate implementation work, including the arithmetic lemmas and the correspondence of native evaluation/certificates with that definition. Machine values also require a specified observation primitive: a u8-to-Nat observation can be composed in library code to produce the Int model, but a user declaration alone cannot invent facts about an opaque runtime primitive.

### Box always belongs to runtime

Box<T> is always non-logical, irrespective of T. In particular Box<Nat> and Box<@P> are runtime containers with erased payloads, just as Option<@P> retains its runtime tag. Box construction, ownership, dereferencing, and destruction obey the runtime rules; ordinary Box::new does not become a logic fn under generic substitution. Rust's normal treatment of zero-sized payloads still applies; retaining a runtime Box does not assert that every instantiation allocates. Runtime optimizations remain separate from logical erasure.

A logical type cannot derive Logical with a Box field, even when that box's payload is logical. Use direct recursion for logical structures and explicit models for runtime structures:

~~~rust
enum RuntimeList {
    Empty,
    Cons(u8, Box<RuntimeList>),
}

// A Model<RuntimeList> implementation on Seq<Nat> can describe
// the list's contents, observing bytes through their Nat model.
// RuntimeList and its Box fields remain runtime values.
~~~

Such an observer can follow legally readable boxed storage and construct the corresponding logical sequence, subject to the observation and termination rules. It does not execute Box operations, keep runtime references in the result, or change Box's classification. Allocation identity and aliasing are not automatically properties of this content model. The earlier statement that Box is transparent in logic is superseded: a chosen model may forget the box, but the source type itself is runtime. Rc, Arc, and shared references likewise receive no automatic conversion into logical inductive values; shared acyclic structures can have finite models, while cycles and interior mutation require additional rules.

Plan: part of the inductive/generic extension. Acceptance cases include direct recursive Seq, a library Int representation, a finite-map proof field, induction over a logical enum, rejection of a non-positive recursive definition and of a logical record containing Box<Nat>, and a runtime boxed list with a separate logical sequence model. These examples do not change the current implementation status.

### Model construction is observation

A logical destination M implements Model<T> using logic fn model(source: &T) -> Self. Self denotes M, so the source argument is &T rather than &self. Multiple destination models are supported, with one unambiguous implementation per pair (T, M). x as M selects that implementation when M is logical; no model means a type error. Runtime casts keep their existing meaning.

The operation makes an immutable logical observation under a short shared-borrow permission check. It does not move, copy, allocate, run a destructor, or retain a runtime reference. Through an active &mut loan, observe through the authorized reference; the unavailable original binding cannot be read. The snapshot outlives the observation and the original runtime storage. The compiler records the binding version and relevant heap state, so unchanged pointer bits do not imply unchanged contents. Shared references do not by themselves solve interior mutability or concurrency; those need dedicated models and permission rules.

A model body can inspect permitted representation observations, compose existing models, and call logic fn. It cannot call arbitrary runtime getters. Defining a model chooses an abstraction, possibly discarding details such as capacity; it does not prove that operations preserve that abstraction. Methods still return or establish evidence relating the before/after observations.

~~~rust
let before: Seq<Int> = values as Seq<Int>;
values.push(7); // ordinary runtime operation
let after: Seq<Int> = values as Seq<Int>;
// push's checked specification connects before and after.
~~~

A model field is erased because its type is Logical, regardless of whether its enclosing record is runtime. Neither logic let nor a Ghost wrapper is needed. For explicit historical observations, bind the selected model value with ordinary let. old!(x) continues to select entry state in signatures; a model cast gives that observation its logical type. There is no generic source operation that snapshots arbitrary runtime T into a secretly logical occurrence of the same T.

### Safe logical computation and eager calls

Logical functions and explicit logic blocks are pure and total, and return logical values. prop declarations, prop!(e), and logical proof reasoning establish logical contexts too. Within them, runtime data may only be legally observed; ordinary calls, runtime mutations, moves/destructors, runtime result escape, and control transfer to an enclosing runtime construct are forbidden. Runtime address/reflection operations cannot expose logical contents. Logical if requires Bool, runtime if requires bool, and runtime match cannot inspect a logical discriminant.

Logical calls and operators may occur directly in ordinary code. Callee/operator resolution fixes the mode; receiver and argument evaluation uses the surrounding context. Normalize eager runtime operands in source order before replacing a logical application with Erased. For derive(mutate_and_prove(&mut x)), retain the inner ordinary call and erase derive. In an explicit logic block the same ordinary call is rejected. Short-circuit expressions and logical branches never authorize hoisting effects out of a path that depends on erased information.

An ordinary fn returning only a proof may mutate, panic, or diverge and remains executable. Its logical result becomes Erased. Checked runtime promises do not change that mode. Ordinary optimization is separate from erasure and must preserve behavior. Kernel checking happens before logical distinctions are collapsed; generic/trait dispatch must preserve the resolved source implementation even when several source types share the marker.

### Compiler and library boundary

The kernel may retain native Int/Nat terms and arithmetic certificates. Primitive model relations and the translation of executable operations into model facts remain on the trusted path, with differential tests supporting the correspondence. A library's declaration of Model alone supplies no unchecked facts.

Prop values and proofs remain distinct source concepts even though both erase to the same marker. Inductive proposition formation, elimination, positivity, and dependent proof functions must be checked. Elimination must preserve the kernel's proof-irrelevance rules: being Logical is not by itself permission to extract arbitrary logical data from an irrelevant proof. Quantifiers can reuse those mechanisms as library propositions. Erasure does not permit unrestricted elimination of proof witnesses into executable data, or fabricated evidence for False.

Plan: revise the affected scopes of LOC-3, LOC-14, LOC-26, LOC-73, LOC-81, and LOC-97 before implementation. Acceptance cases include logical versus runtime Bool, generics preserving mode, prohibited Seq<u8>, model observations through authorized loans, historical heap snapshots, preserved effectful arguments, runtime enum tags with erased proofs, logical constructors for quantifiers, and generated marker cleanup. This decision does not mark any feature implemented or complete any existing task. The remaining obligations and validation examples are recorded under Design concerns in [Open questions and deferred](open-questions.md).

## Built-in forms

Built-in forms. Locus has no user-defined macros: one in a header would hide the text a reviewer is meant to read, diagnostics through expanded code are poor, and nothing about verification needs them. It has a closed set of forms the compiler handles itself, spelled with ! as Rust spells a macro call. The mark tells a Rust reader what it tells them in Rust: this is not an ordinary call, its arguments need not be ordinary values, and something happens at compile time. These forms lower directly to checked constructs; describing prop! as sugar does not require a user macro system.

- _ stays the request for evidence, in any expression position where evidence of a known claim is expected. It reads as it does in Vec<_>: work this out. It is not valid Rust in expression position, so nothing is reinterpreted.
- prove!(P) states a claim where it stands. P is a proposition expression, or a logical Bool condition lifted with holds as in prop!. The claim must be provable at that point, by the search a _ would run, and a failure is reported there. It differs from let _: @P = _; in two ways: the fact stays in scope for every later search, which is what makes it a stepping stone and what localizes a broken proof to the step that no longer follows; and it is an expression of type @P, so let h = prove!(i <= limit); names the evidence and (out, prove!((out as Int) == successor(n as Int))) is a hole that says what it proves. It erases to nothing as a statement and to the marker as a value. It takes no hints yet. The name assert! is not used for it, because in Rust that is a check at runtime.
- prop!(e) is the built-in spelling of logic { holds(e) }; e must be a logical Bool. old!(x) selects the entry observation under Mutation and references. Explicit model casts and ordinary let provide named snapshots; the earlier Ghost<T>-producing snapshot! is superseded.
- Forms that mean what they mean in Rust: assert!(c), a check at runtime after which c is a known fact, and which needs evidence in a function that promises no_panic; unreachable!(), of any type, which needs evidence of false under no_panic; todo!(), of any type, which panics, so that a half-written file can be checked; panic!, debug_assert!, matches!, and vec! once Vec exists; and the attribute derive with a closed list of traits.
- recurse!(evidence, call), rewrite!(eq, h), unfold!(f, h), and fold!(f, h) take the same spelling. recurse! wraps what must be, syntactically, a recursive call, and the type expected of its evidence is computed from that call. unfold! and fold! take the name of a function and not a value. The result type of rewrite! is computed by substitution. None of the four names is reserved any longer.
- The rule that separates a form from a library function: a step is a function when its type can be written, and a form when the compiler has to look at syntax to know the type. A lemma such as u8_le_trans, and symmetry, transitivity, and congruence of equality, are functions that take evidence and return evidence, and live in the library. rewrite! cannot be one today: its signature would take the predicate P as a parameter, which needs generics and a lambda to pass, and since the kernel computes nothing by itself, P(a) would still have to be connected to the claim actually held; the form reads the claim of h, finds the occurrences, and works P out. unfold! cannot be one at all in that shape, since no signature can speak of the body of whichever function it is given; it is a rewrite along that function's defining equation, and fold! is the same rewrite backwards. Either way nothing is trusted that was not before: a form emits an ordinary kernel step, transport, and a library lemma is a checked function.
- To revisit when generics and lambdas in logical contexts become possible: whether some of these forms should become library functions. The candidates are transport itself, as a function taking the predicate explicitly, and each function's defining equation as evidence that can be named. rewrite!, unfold!, and fold! would then remain as conveniences that infer the predicate. Proof helpers could be grouped as logic::eq, logic::u8, and so on. Logical helper functions and types can use ordinary modules; namespace does not determine erasure, type classification and function mode do.

Plan: LOC-25, LOC-71, LOC-77, LOC-78, LOC-79, LOC-80, LOC-81, LOC-82, LOC-83, LOC-84, LOC-85.

## Reasons at explicit trust boundaries

An unchecked foreign contract carries a mandatory nonempty reason string as part of its grammar: `trusted "reason" fn ... = Rust::item;`. A comment cannot replace the reason. The compiler checks the header's well-formedness and runtime representation against the registered backend, while its logical specification and effect promises are explicit review assumptions. The declaration is recorded by `locus audit`, with the reason and implementation path. It does not become an unfoldable logical definition or an invisible kernel axiom. Any future admitted lemma or other unchecked assertion must use the same mandatory reason convention.

The initial implementation accepts registered `Vec::len`, `Vec::get`, and `Vec::push` adapters. Signatures retain the backend's evidence slots and physical result shape. General Rust paths, arbitrary foreign ABI generation, and standard-library generic ABI mapping are later interop work.
