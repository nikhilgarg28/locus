+++
id = "plan-borrowed-dyn"
title = "Borrowed dynamic dispatch"
group = "Plans"
route = "plans/borrowed-dyn.html"
+++

# Borrowed dynamic dispatch

Implement a bounded first slice of LOC-34 on the rebased trait branch. The first slice supports shared trait-object parameters, local borrows, fields and input-linked results, fixed physical associated types, ordinary `&self` methods and checked concrete implementations. Method arguments/results are physical scalars or tuples. Runtime Rust dispatch uses a trait object, not an enumeration of the implementations known to Locus.

The compatibility baseline is the [Rust Reference](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility); Locus starts with a narrower set of receivers and signatures.

## Ordered implementation

1. Parse and resolve `dyn Trait` and associated equalities. Diagnose incompatible methods, unresolved associated types, bare unsized values and unsupported ownership shapes at the source use.
2. Add opaque snapshot identities and independently validated dispatch signatures/tables. An opaque value has neither a logical constructor nor observable fields. Packing retains the concrete value and checked implementation; dispatch supplies no logical laws or effect promises.
3. Integrate shared coercions and method selection with ordinary evaluation and borrow provenance. A conversion neither moves the referent nor extends its lifetime. Never select an inherent method in place of the chosen trait implementation.
4. Preserve dispatch through the checking IR, erased checker, both interpreters and native Rust emission. Keep dyn interfaces internal initially; report unsupported export shapes rather than exposing a different public trait identity.
5. Test runtime-selected implementations, associated bindings, defaults, aliases, borrow failures, incompatible/forged tables, panic behavior and rejected logical interfaces. Add checked manual examples, diagnostics, architecture and trusted-base changes; complete the extended gate and website checks.

## Acceptance and deferred scope

Two implementations selected by runtime control flow must execute through one checked consumer and agree in both interpreters and warning-denied Rust. The source and independent IR checkers must reject fabricated object values and mismatched slots. Existing trait, ownership, proof and native-import suites must remain green.

Deferred: mutable/owned objects, more general object lifetime relationships, native/exported object interfaces, generic implementation families, logical observers/proof contracts, generic methods, supertrait upcasting, auto traits, associated constants and general `?Sized`. The first slice enforces unsized value restrictions directly; general `Sized`/`?Sized` and `Self: Sized` exclusions remain LOC-269. Logical objects are LOC-270 and external object identities are LOC-271.

## Coverage matrix

| Boundary | Positive checks | Negative checks |
| --- | --- | --- |
| Syntax and resolution | Qualified paths, aliases, lifetimes, fixed associated types in either order, real files | Missing/extra/repeated bindings, unresolved implementations, concrete-only methods |
| Dispatch | Multiple implementations, same-name distinct traits, defaults versus inherent methods, reused tables | Cross-trait substitution, wrong slot/signature/receiver, duplicate or missing tables |
| Storage and control flow | Tuples/enum payloads, fields, named-lifetime results, reference reassignment across loop joins, conditional/enum selection, disjoint mutation | Local escape, wrong input lifetime, move/write while borrowed, hidden borrow in an aggregate |
| Evaluation | Ordered arguments, short-circuiting, unit-returning panic, generated cases and every byte receiver with boundary arguments under both overflow modes | Logical slots, effect promises, fabricated observer equations, mismatched proof results |
| Proofs and targets | Caller-side proofs and locked replay without search; 32-/64-bit slot checking | Changed target width; erased logical data in a physical slot |
| Rust boundary | Physical wrappers use dyn internally and compile with warnings denied | Public function/tuple/struct/enum/method/trait leaks; real imported Rust trait objects |

Executable cases compare both interpreters and generated Rust. Cross-target cases check both interpreters and the target guard; Rust execution uses the host target. Corrupt-IR cases bypass source elaboration to exercise the independent checkers directly. Deferred interfaces in LOC-269–271 are rejected, not counted as implemented coverage.

## Validation record

The completed `tools/check.sh --extended` run passed the normal suite in 537 seconds and the release stress suite in 1,159 seconds. The normal suite exceeded the advisory 120-second target. This is a historical validation record; generated status independently tracks whether measurements match the current source fingerprint.

The feature has 33 dedicated regression tests plus a real Cargo import-boundary regression. Its exhaustive byte-dispatch check covers 2,048 receiver/argument/implementation combinations against an independent arithmetic oracle, both interpreters and Rust in both overflow modes. The full extended differential suite separately covered 10,000 generated programs and 81,719 execution cases with no disagreement, crash or inconclusive case. Independent malformed-IR, ownership, proof replay, diagnostic, checked-documentation and website gates passed. Desktop and mobile browser review covered search, test disclosures, complete-source disclosures and the page without JavaScript.

Only completion-status prose changed after that extended run; documentation, site and measurement-freshness checks were rerun for the final text. No additional execution coverage is claimed from those documentation checks.
