+++
id = "spec-interfaces"
title = "Specifications and implementations"
group = "Vision"
route = "vision/spec-interfaces.html"
order = 44
+++

# Specifications and implementations

A spec is a named concrete interface whose declarations must be implemented. It is not a trait, axiom, or new runtime object. The [implementation plan](../roadmap/interop.md#specifications-and-native-imports) separates the first checked subset from extensions. [Rust imports](rust-imports.md) are a separate realization mechanism; they are not implemented in this round.

## Grammar and names

~~~text prose grammar
spec-item = visibility? "spec" ("mod" | "type") Name "{" member* "}"
member    = attributes? "logic"? "fn" signature ";"
          | "const" Name ":" Type ";"
module-implementation = "impl" "mod" Name (";" | "{" ordinary-items* "}")
type-representation   = "struct" Name "{" private-fields* "}"
type-implementation   = "impl" Name "{" functions-and-constants* "}"
~~~

`spec mod counters` creates the module `counters`. `impl mod counters` supplies its implementation; the explicit `mod` distinguishes it from an inherent type implementation. `impl mod counters;` loads `counters.lc` or `counters/mod.lc` using ordinary module discovery. Both blocks occur in the same lexical parent. There is no special header extension. Ordinary `use` and re-export rules apply.

`spec type Counter` specifies one nominal type. Its initial manual representation is a same-named struct in the same lexical module, with one `impl Counter`. All representation fields are private. The spec controls the type's visibility; its representation cannot independently widen it. Normal module privacy applies, including access from the declaring module and its descendants. This does not create a second privacy boundary inside a module.

All members in a spec are public within the spec's visibility. Do not write `pub` on individual header members. A matching implementation member inherits public visibility; plain `pub` is permitted but redundant there. Unlisted implementation members must be private. Restricted-public helpers such as `pub(crate)` also constitute additional exposure and are rejected.

## Completion and composition

Several spec blocks in the same lexical module may contribute disjoint members to one spec. Kind and visibility must agree. Duplicate member names are errors with both source locations; there is no overriding or file-order preference.

A module spec has exactly one `impl mod` body. A type spec has exactly one representation and one inherent implementation in its declaring module. Missing or duplicate realizations, extra exposed members, and additional implementations through aliases or other modules are errors in the initial subset. Even unused loaded specs must be complete. Unloaded files contribute nothing. A consumer cannot replace a dependency's realization.

Later work may split implementation bodies or introduce declaration-only checked artifacts. Such artifacts must never register an unproved theorem or make a missing implementation callable.

## Signatures and proofs

Signatures resolve in the completed module/type’s ordinary scope. A spec is not yet an independently compiled header environment: referenced type and logical definitions remain part of the contract and must be inspected when reviewing it. Separate checked interface artifacts are deferred.

Initially, header and implementation signatures must have the same tokens, ignoring whitespace and ordinary comments. Parameter names, mutability, lifetimes, tuple binders, logical mode, result types and propositions all participate. Semantically equivalent formulas, renamed binders and alternate alias spellings are deliberately rejected. Later matching can compare elaborated, alpha-renamed signatures; it must never compare contracts after proof erasure.

Header promises such as `#[no_panic]` become obligations of the implementation even when not repeated. Implementation promises may be stronger. Bodies pass the ordinary elaborator, ownership checker, execution-IR checker and proof kernel. A header neither creates evidence nor authorizes recursion.

A proof input remains a caller obligation. A proof output must be constructed by the body. Mutable parameters retain their entry/exit snapshot and `old!` rules. Empty implementations, `_` for a false proposition, and cycles of declarations must fail. Ordinary and logical recursion retain their existing termination restrictions.

A `logic fn` header requires a logical definition. Its implementation remains transparent under the existing logical-function rules: separating files does not introduce an opaque logical constant. Abstract logical interfaces and opacity control are follow-up work. Constants declare their type; implementations supply checked initializers whose values remain available to constant evaluation. A type-only constant header does not promise a particular value.

## Types and invariant fields

The first `spec type` exposes methods and associated constants, not fields. Its private representation may carry proofs about earlier fields. Every construction or whole-value replacement must supply checked evidence. Existing dependent-field mutation restrictions still apply. A spec does not manufacture struct invariants.

The broader design can also support transparent struct declarations inside module specs: complete ordered fields, all public, including dependent proof fields. Implementation layout and propositions must match; no hidden extra fields may be appended. Such a type can be useful in Locus but fails Rust export when a public field is logical. Transparent structs, enums and their constructor interfaces are explicitly deferred.

Proposition declarations in future headers must specify constructors or deliberate opaque introduction/elimination laws. An unknown body cannot be treated as transparent. Initially, define propositions normally outside the spec and reference them by path.

## Initial limits and lowering

The first subset is concrete module and struct-type specs, ordinary/logical function headers, supported receivers and proof/data signatures, plus constant headers. Generic specs/methods, traits, associated type families, enum representations, transparent fields, nested header specs, header `use` items, default bodies and arbitrary spec attributes are deferred. Existing concrete uses such as `Option<u8>` remain valid in signatures.

Specs are checked before ordinary module lowering. Only actual definitions reach elaboration, augmented with header visibility and promises. Header/body mismatch diagnostics point to both locations. Type-spec completeness must also be checked after path resolution so aliases or implementations elsewhere cannot bypass it. This requires no new kernel rule or runtime representation.

## Export and packaging

`pub` controls Locus visibility; export roots separately select Rust exposure. Proof-returning functions can use the existing data-only facade. Proof inputs, public logical fields and other forbidden positions still fail export. Necessary private runtime helpers are emitted privately.

Specs and implementations are ordinary packaged `.lc` source. Cargo ownership, proof locks and build receipts apply. Contract/source changes enter the normal input fingerprint. Generated Rust remains a build artifact, not an independently editable verified header.

## Relationship to traits

A type spec names one nominal type with one representation. A trait names a requirement that many types may implement. Share member grammar, associated-type concepts, logical methods and contract checking, but keep these identities distinct: a trait alone does not choose which concrete value `new` constructs. The future spelling `spec trait` can describe an existing or external trait interface without collapsing it into a concrete `spec type`. Generic trait bounds and implementation selection belong to the trait/generics project.
