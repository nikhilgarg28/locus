+++
summary = "Completed logical type separation, explicit evidence, heap views and verified collection support."
id = "p322"
name = "Reconciliation: logical types and verified collections"
status = "done"
updated = "2026-09-23T05:30:08.935417+00:00"
route = "roadmap/reconciliation.html"
order = 1
kind = "project"
+++

# Reconciliation: logical types and verified collections

Completed in three tiers: the logical/runtime split and explicit evidence; recursive logical data, models and library quantifiers; then heap views, shared references and verified collections. All four preview gates are stabilized. The detailed wave plans below record implementation history, including temporary restrictions resolved by later waves. Current syntax and bounds live in the language reference, not those intermediate plans.

Historical acceptance: tools/check.sh --extended passed on 23 September 2026, including focused regressions, negative corpus/goldens, stored certificates and three-way execution. Fast suite 211s against a 120s advisory target; extended suite 826s; final completion receipt recorded. Remaining broad Rust features have their own projects rather than reopening these accepted bounded tiers.

<a id="LOC-205"></a>
## LOC-205 · Q1 · logic as a keyword: logic fn, logic blocks, and logical callable types parsed
<!-- task: {"id": "t323", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Syntax. Wave 1.
Depends on: nothing.
Unblocks: C1 ([LOC-209](reconciliation.md#LOC-209)), C2 ([LOC-210](reconciliation.md#LOC-210)).

Scope. prop and logic are reserved keywords, no longer contextual. The parser reads logic fn name(params) -> T { }, logic { ... } as an expression, and logic Fn(x: T) -> R as a callable type in parameter position, into AST nodes of their own; Bool, Int, Logical, Model, Exists, ForAll are ordinary names. A struct or function named logic gets the keyword diagnostic every Rust keyword gets. Everything parsed here that the elaborator does not yet accept answers with the uniform not-yet diagnostic naming its task (C2 for logic fn and blocks, D8 for callable types). This commit lands behind the preview gate of P6 of the Process project, which must land first.

Tests.
- AST rendering tests for each form; the nesting test gains logic blocks; the fuzz vocabulary gains the words.
- logic { x } with a struct named logic in scope is the keyword error, not a struct literal.
- The stack margin is measured and reported.

Done when. Every spelling in Target language that uses logic parses.

<a id="LOC-206"></a>
## LOC-206 · Q2 · Named-arm propositions: declarations, constructors with @, and proof patterns
<!-- task: {"id": "t324", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Syntax. Wave 1.
Depends on: nothing.
Unblocks: C4 ([LOC-212](reconciliation.md#LOC-212)).

Scope. prop Name(params) { Arm => { body }  Arm(field: T, ...) => { body }  Arm { field: T, ... } => { body } } with the braces after => mandatory; construction Pred::Arm(args) @ evidence, Pred::Arm { field: value } @ evidence, and Pred::Arm @ evidence; proof patterns Pred::Arm(pats) @ pat and the named form in let and match, with field shorthand and renaming; whole @ (Pred::Arm @ h) reads as Rust's whole-value binding around an evidence pattern; a chain of @ without parentheses is a syntax error that says to parenthesize. The old stated-conclusion form (Arm: @P(..), Arm(fields) with an implied conclusion) gets a migration diagnostic with a fix into the new form where the fix is mechanical (a one-arm constructor whose conclusion is a formula becomes Arm => { prop!(formula) }). Behind the preview gate of P6.

Tests.
- AST rendering tests for every arm shape, construction, and pattern; the migration fix applied to every prop in the corpus and examples parses.
- name @ pattern keeps its Rust meaning in a test.
- The nesting test covers nested constructions with parentheses.

Done when. Every prop in Target examples and Target language parses as written.

<a id="LOC-207"></a>
## LOC-207 · L1 · Bool-valued comparisons on Int and the lifting of Bool to Prop in the kernel
<!-- task: {"id": "t325", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Kernel. Wave 1.
Depends on: nothing.
Unblocks: C1 ([LOC-209](reconciliation.md#LOC-209)), C3 ([LOC-211](reconciliation.md#LOC-211)).

Scope. The kernel keeps one bool type; logical Bool is bool in Logical mode, as Ghost<T> was T (the elaborator draws the line, the erased check judges it). The kernel gains decidable comparisons on Int that return bool, int_le_b, int_lt_b, int_eq_b, computed by evaluate on literals, with reflection axioms in the shape of cmp_reflect connecting each to its Prop in both directions; holds(b) is the existing proposition b == true and needs no new form; Bool-valued conjunction, disjunction, and negation are the existing bool primitives in Logical mode with their evaluation. The contract's Int section and Comparisons are extended; the name lists in tests/kernel_int.rs and the soundness test's oracle cover the new primitives and axioms.

Tests.
- Each comparison against Integer on random pairs and at boundaries; each reflection axiom used both ways and near-missed (wrong flag, lt for le); the soundness test attacks them.
- A closed Bool formula decided by evaluate; a proposition built through holds proved from a decided Bool.

Done when. prop!(n + 1 <= 3) for n: Int can elaborate to a Bool comparison lifted through holds, and the contract names every new form.

<a id="LOC-208"></a>
## LOC-208 · L2 · Constructors of a declared proposition take witnesses and one proof of a computed body
<!-- task: {"id": "t326", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Kernel. Wave 1.
Depends on: nothing.
Unblocks: C4 ([LOC-212](reconciliation.md#LOC-212)), D1 ([LOC-218](reconciliation.md#LOC-218)).

Scope. A constructor of a declared proposition is a telescope of witness parameters followed by exactly one proof parameter whose type is a term of type Prop computed from the arm's body: a logical-mode term over the parameters and witnesses built from the connectives, holds of a Bool, equalities, predicate applications, and calls of logical functions, with let elaborated by substitution and case on logical data allowed. Construction and CaseProof keep their shapes. And, Or, True, False, Implies, and Not stay primitive formers, so the bootstrap is not circular. A recursive prop declaration is refused with a diagnostic naming D5. The contract's declared-proposition section is rewritten to this shape.

Tests.
- Constructors with zero, one, and two witnesses, unit and named; a body that is a conjunction; a body that calls a logical function and needs unfold! inside a proof of it.
- Near misses: evidence for the wrong body, a witness of the wrong type, a second proof parameter refused.
- The soundness test's hand-built triples cover a witness constructor and its elimination.

Done when. The kernel's declared propositions are exactly the Vision's named arms with the witness shapes erased to positions.

<a id="LOC-209"></a>
## LOC-209 · C1 · Every type is Logical or runtime; Ghost<T> and snapshot! are replaced by model observations
<!-- task: {"id": "t327", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 2.
Depends on: Q1 ([LOC-205](reconciliation.md#LOC-205)), L1 ([LOC-207](reconciliation.md#LOC-207)).
Unblocks: C2 ([LOC-210](reconciliation.md#LOC-210)), C3 ([LOC-211](reconciliation.md#LOC-211)), C5 ([LOC-213](reconciliation.md#LOC-213)), G1 ([LOC-214](reconciliation.md#LOC-214)).

Scope. Classification belongs to the type: Int, Bool, Prop, every @P, and functions of the logic are Logical; the machine types, bool, and every runtime aggregate are runtime; a struct or enum is Logical only when it derives Logical and every field is Logical (D3 adds the derive; here it is refused as not-yet). A binding takes its classification from its type, with no logic let; a Logical value at runtime, in a runtime if, a runtime match, or a runtime field of a non-Logical aggregate that is not itself a logical field, is an error with the message of the Vision. Bool is a surface type distinct from bool: both are the kernel's bool, in different modes, and the erased check refuses any Bool-typed binding at runtime as it refuses Ghost today. Ghost<T> and snapshot!(e) are removed with migration diagnostics that carry fixes: Ghost<u32> becomes Int and snapshot!(x) becomes x as Int, and the same for the other machine types and bool. Nat stays out until D6. The interim rule of [LOC-193](core-build.md#LOC-193) is left to C2. Behind the preview gate.

Tests.
- Corpus files for each classification error; the fixes applied to every Ghost and snapshot! use in the corpus; the erased check on a hand-built tree binding a Bool at runtime.
- The E8 corpus files respelled with as Int and unchanged in behaviour and in generated Rust.

Done when. No Ghost or snapshot! remains, and a value's erasure is decided by its type alone.

<a id="LOC-210"></a>
## LOC-210 · C2 · logic fn and logic blocks; an ordinary fn never enters the logic
<!-- task: {"id": "t328", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 3.
Depends on: Q1 ([LOC-205](reconciliation.md#LOC-205)), C1 ([LOC-209](reconciliation.md#LOC-209)).
Unblocks: C4 ([LOC-212](reconciliation.md#LOC-212)), D2 ([LOC-219](reconciliation.md#LOC-219)).

Scope. A logic fn returns a Logical type and is checked pure and total: no loop, no call of an ordinary fn, no operator that may panic (Int arithmetic is total; machine arithmetic is refused there), no &mut, no move or drop of runtime data, no return, break, or panic; it lowers as a kernel function with a defining equation, which is what E2's logical-by-promises did. An ordinary fn never appears in a proposition, under unfold! or fold!, or as evidence, whatever it promises, and the message says to write logic fn; E7's interim rule for [LOC-193](core-build.md#LOC-193) and E2's env::LOGICAL go. A logic { } block is a logical expression of a Logical type with the same rules. Logical calls and operators may appear in ordinary code, and E8's rule stays: runtime operands are evaluated in source order and the logical application is erased. Promises are claims about runtime evaluation only. Behind the preview gate.

Tests.
- The promise matrix of E2 no longer decides logical use; a promised ordinary fn in a prop! is rejected with the new message; a logic fn with a loop, an ordinary call, a machine +, and a &mut is rejected at the construct.
- A logic fn unfolded with unfold!; a logic block inside an ordinary body; midpoint as an ordinary promised fn with operators, no longer refused.
- The generated Rust of every example is byte-identical before and after.

Done when. midpoint, step, and remaining check in the Vision's spelling with their promises, and no function enters the logic by promise.

<a id="LOC-211"></a>
## LOC-211 · C3 · The default model rule, and prop! and prove! over proposition expressions
<!-- task: {"id": "t329", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 3.
Depends on: C1 ([LOC-209](reconciliation.md#LOC-209)), L1 ([LOC-207](reconciliation.md#LOC-207)).
Unblocks: T1 ([LOC-216](reconciliation.md#LOC-216)), D7 ([LOC-224](reconciliation.md#LOC-224)).

Scope. In a logical context, which is a formula, a predicate body, a logic fn or block, or a logical operand, a machine integer operand is observed through Int and a bool through Bool with no cast written; as Nat or another model picks it explicitly when it exists (D7). prop!(P) and prove!(P) take one grammar: a proposition expression in which a Bool leaf is lifted through holds and a Prop stands as it is. The connectives mean three things by type: short-circuit on bool, total on Bool, and conjunction, disjunction, negation on Prop; => is implication on Prop only; == and <= on Int return Bool through L1 and lift; == on logical data whose equality is a Prop stays a Prop. @(e) means @prop!(e). The branch fact of a runtime comparison is the Prop over the views, as today. Every existing as Int in the corpus and examples may now be dropped; the corpus keeps both spellings.

Tests.
- prop!(x <= 3) for x: u32 and prop!((x as Int) <= 3) elaborate to the same kernel term, asserted; the same for bool and Bool.
- Type errors: a Prop where a Bool is needed in a logical if; => on Bool refused.
- lock.lc's within_limit written both ways proves the same obligations with the same sizes.

Done when. The three target examples check with every cast removed and with every cast kept.

<a id="LOC-212"></a>
## LOC-212 · C4 · Named-arm propositions in the elaborator, with proof patterns in ordinary code
<!-- task: {"id": "t330", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 4.
Depends on: Q2 ([LOC-206](reconciliation.md#LOC-206)), L2 ([LOC-208](reconciliation.md#LOC-208)), C2 ([LOC-210](reconciliation.md#LOC-210)).
Unblocks: T1 ([LOC-216](reconciliation.md#LOC-216)), D5 ([LOC-222](reconciliation.md#LOC-222)).

Scope. A prop declaration with named arms elaborates each body as a logical block ending in Prop with the parameters and the arm's witnesses in scope; the constructor Pred::Arm(args) @ evidence checks the evidence against the body at the header arguments inferred from the expected claim and at the supplied witnesses, and has the enclosing proof type; the header arguments are never runtime inputs. An irrefutable evidence pattern in a let is allowed in ordinary code, because it binds only logical values and erases; a match on evidence is a logical expression whose arms produce logical values, allowed wherever a logical value is expected; a runtime branch never depends on which arm matched, enforced by classification. fold! and unfold! of a prop are refused with the message that a prop is opened by matching and closed by its constructor; a logic fn returning Prop keeps its defining equation. Diagnostics distinguish a missing witness, a missing evidence slot, and evidence for the wrong body. The old constructors migrate by Q2's fix. Behind the preview gate.

Tests.
- Every example of Predicates with named arms in Target language as a corpus file: Either, Between, PrimeDigit, Contains with a stand-in for Seq, IsSuccessor with fold! on the helper.
- within_limit of the lock in the named-arm form, with the evidence opened by a let pattern in remaining, checks and runs.
- Rejections: fold! of a prop, evidence for the wrong body, a match on evidence in runtime position, a recursive prop (naming D5).

Done when. The lock target file in the Vision's spelling checks, runs, and compiles.

Reconciliation clarification. The earlier phrase "arms produce logical values" is narrowed by D9 and the Vision proof-irrelevance rule: proof matches produce proofs; witness-bearing lets that would export arbitrary logical data are rejected. Witness-free irrefutable evidence lets remain available. This is a soundness restriction, tested by evidence_cannot_select_runtime_data_or_export_arbitrary_witnesses, not an unfinished erasure optimization.

<a id="LOC-213"></a>
## LOC-213 · C5 · Scope escape, stored claims, and versions named in diagnostics
<!-- task: {"id": "t331", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 3.
Depends on: C1 ([LOC-209](reconciliation.md#LOC-209)).
Unblocks: T1 ([LOC-216](reconciliation.md#LOC-216)).

Scope. The externally visible result type of a block, arm, or function must be well formed in the receiving context: a free local binder in it is the dedicated error, with the three diagnostics of the Vision kept apart (a forbidden observation at capture, a result type that leaks a local binder, a proof about the wrong version), the last naming versions by source position as x as it was before line 12. A Prop-typed field followed by evidence of it (Certificate { claim: Prop, evidence: @claim }) is checked by substituting the supplied earlier fields into later field types, and projection yields evidence of the projected claim, so a claim may escape a block as a record. Replacing a field that evidence depends on stays refused; Annotated's independent value field may change. A proposition formed after a move of a local is refused, as O1 does today.

Tests.
- The acceptance cases named in Snapshot capture: capture before move against a new observation after; shadowing; a stored claim escaping a block against a bare proof with a free local; a claim unrelated to a record's data; replacement of a dependent field; a historical snapshot used later.
- Goldens for the three diagnostics.

Done when. Every example in Snapshot capture, scope, and stored claims is a corpus file with the outcome the text states.

<a id="LOC-214"></a>
## LOC-214 · G1 · Default cleanup of markers in the generated Rust
<!-- task: {"id": "t332", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Generated. Wave 3.
Depends on: C1 ([LOC-209](reconciliation.md#LOC-209)).
Unblocks: T1 ([LOC-216](reconciliation.md#LOC-216)).

Scope. After erasure, unused local bindings and dead assignments whose type is the marker are removed, repeatedly, while an initializer with runtime evaluation is kept as let _ = call; in source order; unused logical pattern bindings become wildcards where destructuring is preserved; a retained unused parameter gets an underscore-prefixed name without changing the signature; marker support is emitted only where referenced; Bool, Prop, Int, and logical enums erase to the marker. The blanket allow lints leave the header, so that the corpus compiles with warnings denied on its own merits; any lint a retained construct still needs is allowed at that construct with a comment. Evaluation order, panics, enum tags, and drops are unchanged, judged by the three-way comparison.

Tests.
- Goldens for cascading dead markers, a retained effectful initializer, proof-bearing destructuring, an unused retained proof parameter, and a runtime enum with an erased payload.
- The whole corpus compiles under -D warnings with no blanket allow; the random programs agree three ways.

Done when. The generated Rust of the target examples has no marker binding a reader would delete by hand.

<a id="LOC-215"></a>
## LOC-215 · G2 · Dereference on writes through a mutable reference, and the printer's remaining lies
<!-- task: {"id": "t333", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Generated. Wave 1.
Depends on: nothing.
Unblocks: T1 ([LOC-216](reconciliation.md#LOC-216)).

Scope. Inside a callee with a &mut parameter, a whole read or write of the parameter prints as *x, so that counter = counter + 1 in Locus prints as *counter = *counter + 1 and means the same in Rust; a cast on the left of < or << stays parenthesized; a method called by path prints by path. The audit of R4's list of places where the printer gives valid Rust syntax another meaning is closed.

Tests.
- Corpus files whose //~ rust: lines pin the deref; the compiled comparison holds.

Done when. No rejected file remains for a printer lie.

<a id="LOC-216"></a>
## LOC-216 · T1 · The target examples in the Vision's spelling, and the tier-one acceptance corpus
<!-- task: {"id": "t334", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Robustness. Wave 5.
Depends on: C3 ([LOC-211](reconciliation.md#LOC-211)), C4 ([LOC-212](reconciliation.md#LOC-212)), C5 ([LOC-213](reconciliation.md#LOC-213)), G1 ([LOC-214](reconciliation.md#LOC-214)), G2 ([LOC-215](reconciliation.md#LOC-215)).
Unblocks: T2 ([LOC-217](reconciliation.md#LOC-217)).

Scope. tests/corpus/target holds the three examples exactly as Target examples writes them, named arms, logic block, promises, and pub(crate) included; they check, run, and compile, and their obligation counts are recorded against finding 7. The acceptance cases the Vision's plan lines name for tier one become corpus files: logical against runtime Bool, preserved effectful arguments, runtime enum tags with erased proofs, the marker cleanup fixtures, and the capture cases of C5. The previews of this tier are stabilised: the gates and directives are removed in this commit.

Tests.
- Every file listed runs three ways; --stats output for the three targets is asserted.

Done when. The target files match the Vision's text byte for byte, apart from the enum standing in for Option.

<a id="LOC-217"></a>
## LOC-217 · T2 · Acceptance of tier one: Now rewritten, the interim rules gone, the criteria as tests
<!-- task: {"id": "t335", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Robustness. Wave 6.
Depends on: T1 ([LOC-216](reconciliation.md#LOC-216)).

Scope. Language, as built is rewritten for the logical split; the migration diagnostics of Q2 and C1 are tested by applying their fixes; the code paths of [LOC-193](core-build.md#LOC-193)'s interim rule, of Ghost, of snapshot!, and of logical-by-promises are gone; the status rows move; the exit criteria of the Reconciliation plan's first tier run under tools/check.sh --extended and print the banner of P5.

Done when. What done means, tier one, holds as tests.

<a id="LOC-218"></a>
## LOC-218 · D1 · Type parameters in the kernel, instantiated by substitution
<!-- task: {"id": "t336", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Kernel. Wave 2.
Depends on: L2 ([LOC-208](reconciliation.md#LOC-208)).
Unblocks: D2 ([LOC-219](reconciliation.md#LOC-219)), D4 ([LOC-221](reconciliation.md#LOC-221)).

Scope. Declarations of structs, enums, functions, and propositions may take type parameters; a use instantiates them by substitution into a monomorphic declaration the kernel checks as today, cached by the instance, so that the checker itself never reasons about a type variable. A bound is a predicate on the substituted type checked at instantiation: Logical, and later comparison interfaces. The contract gets a section on parameters and instances stating that nothing is trusted beyond substitution.

Tests.
- An instance of a struct, an enum, a function, and a proposition at two types; a bound violated at instantiation; the soundness test's name lists extended.

Done when. Seq<Int> and Seq<u8> can be spelled at the kernel level and the second is refused by its bound.

<a id="LOC-219"></a>
## LOC-219 · D2 · Generics in the surface language, with mode fixed by the declaration
<!-- task: {"id": "t337", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 4.
Depends on: D1 ([LOC-218](reconciliation.md#LOC-218)), C2 ([LOC-210](reconciliation.md#LOC-210)).
Unblocks: D3 ([LOC-220](reconciliation.md#LOC-220)), D7 ([LOC-224](reconciliation.md#LOC-224)), D8 ([LOC-225](reconciliation.md#LOC-225)).

Scope. Rust's generic syntax on structs, enums, and functions with T: Logical bounds; an ordinary generic function transports a Logical value without inspecting it and cannot be called in logic; a logic fn with a parameter returning T requires T: Logical; instantiation never changes a function's mode. Runtime instances are monomorphised for the printer; logical instances are kernel instances of D1. Option<T> and Result<T, E> become prelude enums, so that Percent can return Option<Percent>.

Tests.
- The generics preserve mode case of the Vision; identity<T> over a Nat in runtime code; Option<@P> keeping its tag; percent.lc with Option, matching the Vision text.

Done when. The Percent target uses Option as written.

<a id="LOC-220"></a>
## LOC-220 · D3 · derive(Logical): logical structs and enums, with no runtime tag
<!-- task: {"id": "t338", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 5.
Depends on: D2 ([LOC-219](reconciliation.md#LOC-219)).
Unblocks: D4 ([LOC-221](reconciliation.md#LOC-221)).

Scope. #[derive(Logical)] on a struct or enum requires every field and payload to be Logical and makes the type Logical; a logical enum has no runtime tag and is matched only in logical code; a runtime enum or container with a Logical payload keeps its tag, length, and allocation; Box<T> is always runtime and a logical record cannot hold one. Logical values are immutable observations; assignment to a logical binding is a new value.

Tests.
- HistoricalNonZero and Entry from the Vision; a logical record with a Box field refused; a runtime match on a logical enum refused; Box<Nat> as a runtime box.

Done when. The classification of every type in Logical types, models, and erasure is decided by these rules.

<a id="LOC-221"></a>
## LOC-221 · D4 · Recursive logical types, structural recursion, and induction
<!-- task: {"id": "t339", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Kernel. Wave 6.
Depends on: D1 ([LOC-218](reconciliation.md#LOC-218)), D3 ([LOC-220](reconciliation.md#LOC-220)).
Unblocks: D5 ([LOC-222](reconciliation.md#LOC-222)), D6 ([LOC-223](reconciliation.md#LOC-223)).

Scope. A logical enum may mention itself and its group directly, with a positivity check on every occurrence; the kernel generates the induction rule for each such type as a checked rule in the form of int_induction, and a logic fn may recurse structurally on a matched argument of such a type, the descent checked at each call as recurse! states it; any other recursion in the logic is refused. The contract states positivity, the induction schema, and structural descent as the trusted additions.

Tests.
- A list type with its induction used to prove a length lemma; a non-positive definition refused; a non-structural recursive logic fn refused; the soundness test attacks induction instances.

Done when. Seq<T> can be defined in the library with length and append proved by induction.

<a id="LOC-222"></a>
## LOC-222 · D5 · Inductive predicates: recursive prop declarations with positivity and induction over proofs
<!-- task: {"id": "t340", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Kernel. Wave 7.
Depends on: D4 ([LOC-221](reconciliation.md#LOC-221)), C4 ([LOC-212](reconciliation.md#LOC-212)).
Unblocks: D9 ([LOC-226](reconciliation.md#LOC-226)).

Scope. A prop declaration may name itself in an arm body under the conservative rule of the Vision: strictly positive, visible through conjunction, disjunction, quantifiers, and the conclusion of an implication whose premise does not mention the group; refused under negation, in a premise, or through a helper not shown admissible. The kernel gives each such predicate its induction rule over proofs; mutual groups are supported or refused with a diagnostic, as decided in the commit. The elaborator and the contract follow.

Tests.
- Reachable from the Vision with a proof by induction; Bad refused; a predicate hidden behind a helper refused.

Done when. Reachable checks as written in Target language.

<a id="LOC-223"></a>
## LOC-223 · D6 · Library logical data: Seq, Maybe, and Nat with its correspondence to Int
<!-- task: {"id": "t341", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Data. Wave 7.
Depends on: D4 ([LOC-221](reconciliation.md#LOC-221)).
Unblocks: D10 ([LOC-227](reconciliation.md#LOC-227)), B1 ([LOC-228](reconciliation.md#LOC-228)).

Scope. Seq<T: Logical>, Maybe<T: Logical>, and Nat as library declarations in a prelude file checked like any source, with their basic logic fns (length, get, append, map on Seq; to_int on Nat) and lemmas proved by induction; the correspondence of the library Nat with the non-negative Int is a checked pair of logic fns with round-trip lemmas, so that a measure n as Nat and Int arithmetic meet; native Int stays. The lemma names are listed by a test.

Tests.
- Each function and lemma used from a corpus file; the Contains example over Seq<Int> checks.

Done when. The prelude's logical data is written in Locus and checked by the kernel, not declared in Rust.

<a id="LOC-224"></a>
## LOC-224 · D7 · The Model trait: x as M for user models, with the default models registered
<!-- task: {"id": "t342", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 5.
Depends on: D2 ([LOC-219](reconciliation.md#LOC-219)), C3 ([LOC-211](reconciliation.md#LOC-211)).
Unblocks: D10 ([LOC-227](reconciliation.md#LOC-227)), B1 ([LOC-228](reconciliation.md#LOC-228)).

Scope. A compiler-known trait Model<T> with logic fn model(source: &T) -> Self, implemented by impl Model<T> for M with one implementation per pair; x as M for a Logical M selects it and observes under a short shared-borrow check, retaining no reference, with the binding version recorded; the default models of C3 are registered implementations of the same trait; a model body may compose models and call logic fns and nothing else. Traits in general stay out: only Model is accepted as a trait name.

Tests.
- A model of a runtime struct as a logical record; two models of one type; as M through an active &mut observing the referent; a model that calls an ordinary fn refused.

Done when. RuntimeList with a Seq<Int> model from the Vision checks.

<a id="LOC-225"></a>
## LOC-225 · D8 · Logical closures and dependent proof-returning callables
<!-- task: {"id": "t343", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Elaborator. Wave 5.
Depends on: D2 ([LOC-219](reconciliation.md#LOC-219)).
Unblocks: D9 ([LOC-226](reconciliation.md#LOC-226)).

Scope. Closures in logical contexts, |x: T| body, of type logic Fn(x: T) -> R, capturing immutable logical values; a dependent form logic Fn(x: T) -> @P(x) whose result type mentions its parameter; application in logic; no runtime closures. The kernel's function terms already bind; the elaborator and the contract state what is allowed.

Tests.
- A predicate T -> Prop passed to a logic fn; a proof function applied at a value; capture of a runtime local refused without a model.

Done when. ForAll's Each arm can carry its proof function.

<a id="LOC-226"></a>
## LOC-226 · D9 · Quantifiers as library propositions, with the keywords as sugar
<!-- task: {"id": "t344", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Data. Wave 8.
Depends on: D8 ([LOC-225](reconciliation.md#LOC-225)), D5 ([LOC-222](reconciliation.md#LOC-222)).
Unblocks: D10 ([LOC-227](reconciliation.md#LOC-227)), B4 ([LOC-231](reconciliation.md#LOC-231)).

Scope. Exists(P) with its Witness arm and ForAll(P) with its Each arm as prelude prop declarations over logical closures, with elimination that uses the witness only inside a proof, per the proof-irrelevance rule; forall (x: T) { F } and exists (x: T) { F } become sugar for them, or are retired, as decided then on how a header reads; the kernel's quantifier nodes go if the library form is complete.

Tests.
- The rejected witness extractor returning Nat and the accepted proof using the witness internally, from Design concern 2; every existing quantifier test passes through the sugar.

Done when. Only library propositions quantify.

<a id="LOC-227"></a>
## LOC-227 · D10 · Acceptance of tier two: the finite map and the examples of logical data
<!-- task: {"id": "t345", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Robustness. Wave 9.
Depends on: D6 ([LOC-223](reconciliation.md#LOC-223)), D7 ([LOC-224](reconciliation.md#LOC-224)), D9 ([LOC-226](reconciliation.md#LOC-226)).

Scope. FiniteMap with its UniqueKeys evidence, Contains with At { index }, Reachable, RuntimeList with its model, and the acceptance cases of Library-defined logical data (direct recursive Seq, a library Int representation shown equivalent to native Int on a fragment, induction over a logical enum, rejection of a non-positive definition and of a logical record containing Box<Nat>) are corpus files; Language, as built and the status rows are updated; the previews of the tier are stabilised.

Done when. What done means, tier two, holds as tests.

<a id="LOC-228"></a>
## LOC-228 · B1 · Parameter-only views: slices as &[T] and &mut [T], indexing with a proof precondition
<!-- task: {"id": "t346", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: References. Wave 8.
Depends on: D6 ([LOC-223](reconciliation.md#LOC-223)), D7 ([LOC-224](reconciliation.md#LOC-224)).
Unblocks: B2 ([LOC-229](reconciliation.md#LOC-229)), B3 ([LOC-230](reconciliation.md#LOC-230)).

Scope. Slice types only as parameter types, lent from arrays and later from buffers, with length as a logical observation and a Seq<Int> model of a slice of machine integers; indexing takes evidence that the index is below the length, the first operation with a proof precondition, which brings dependent conjunction and implication into prop! as kernel formers; iteration over a slice by index. Nothing is stored or returned. Rue's experience is the reason for the order: a whole standard library was reached with views that never escape a call.

Tests.
- A sum over a slice with its bound proved by the arithmetic procedure; an index without evidence under no_panic refused; a view stored in a field refused.

Done when. A function over a slice of u32 checks with the Vision's rules and no lifetime.

<a id="LOC-229"></a>
## LOC-229 · B2 · Trusted declarations of Rust collections with logical models, listed by the audit
<!-- task: {"id": "t347", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: References. Wave 9.
Depends on: B1 ([LOC-228](reconciliation.md#LOC-228)).
Unblocks: B4 ([LOC-231](reconciliation.md#LOC-231)).

Scope. A trusted declaration states the signature, promises, and specification of a Rust item that Locus does not check, Vec<T> and its push, len, and indexing first, with a Seq model and postconditions relating before and after observations, and heap-state versions on the observation so that values as Seq<Int> before and after a push are distinct; every trusted declaration carries the mandatory reason of P3 and appears in locus audit; the checked-IR interpreter runs them through a Rust implementation and the three-way comparison holds.

Tests.
- push's specification used to prove a length fact; a wrong trusted specification shown to be the audit's problem and not the kernel's; the audit golden.

Done when. A Locus program grows a Vec and proves a fact about its model.

<a id="LOC-230"></a>
## LOC-230 · B3 · Shared references in locals, fields, and results, with lifetimes and observation permissions
<!-- task: {"id": "t348", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: References. Wave 9.
Depends on: B1 ([LOC-228](reconciliation.md#LOC-228)).
Unblocks: B4 ([LOC-231](reconciliation.md#LOC-231)).

Scope. Tier one of references: &T held in locals, stored in fields, and returned, with lifetimes written as Rust writes them and checked by Locus for the observation permission each model cast needs, since an erased observation never reaches rustc; the logic is unchanged because a shared reference is still the value, interior mutability being excluded; parsers that hold their input, lookups returning a reference, and iteration over a collection are the acceptance shapes. Mutable references beyond one call stay out.

Tests.
- rustc as the oracle for every rejected lifetime; the three shapes as corpus files.

Done when. Tier one of references is complete as the Vision states it.

<a id="LOC-231"></a>
## LOC-231 · B4 · Acceptance of tier three: the generic verified collection
<!-- task: {"id": "t349", "status": "done", "priority": 3, "created": "2026-09-23T00:10:45.000Z", "updated": "2026-09-23T03:43:42.000Z"} -->

Completed in the accepted Reconciliation release. The wave plan below is historical: intermediate previews and temporary restrictions were removed by later tasks in this project.

Lane: Robustness. Wave 10.
Depends on: B2 ([LOC-229](reconciliation.md#LOC-229)), B3 ([LOC-230](reconciliation.md#LOC-230)), D9 ([LOC-226](reconciliation.md#LOC-226)).

Scope. The design test of Design concerns: one generic verified collection over a user-defined runtime element type with a logical model, explicit evidence propagated through two or three callers, a mutable operation within the supported borrowing tier, an intentional wrong proof, a harmless refactoring, a rejected existential witness extractor, a runtime Box containing a logical payload, and an executable call inside an otherwise erased expression, each with its diagnostics and retained runtime behaviour checked; Now rewritten; the criteria as tests.

Done when. What done means, tier three, holds as tests, and the Vision's Design concerns 1, 2, 3, and 4 have their validation examples.
