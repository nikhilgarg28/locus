+++
id = "plan-trait-bounds"
title = "Static trait bounds"
group = "Plans"
route = "plans/trait-bounds.html"
+++

# Static trait bounds

This project extends concrete traits with explicit generic constraints. Runtime dispatch stays static. Generic templates continue to receive full checking on concrete instantiation; an unused generic body is not a universal theorem.

## Ordered work

1. Represent inline bounds, `where` predicates and associated-type equalities with source spans. Resolve trait names through normal module visibility and preserve imported identities.
2. Check obligations during specialization, including associated projections, Logical classification and conditional generic implementations. Reject overlapping families and recursive obligation cycles with readable diagnostics.
3. Resolve generic member calls from their declared interfaces. Preserve the selected trait through substitution; a coincidentally available inherent method must not change a call. Propagate explicit input/output evidence through the existing checker.
4. Preserve erasure and Rust export restrictions. No proof axiom, runtime dictionary, or unchecked open generic Rust export is introduced.
5. Add positive and negative tests, directory/module fixtures, interpreter/Rust comparisons and checked documentation. Update the manual, architecture, diagnostics and LOC-22 together; retain explicit follow-ups for unsupported expansions.

## Acceptance

Exercise equivalent inline/where bounds, multiple bounds, associated equality and projection bounds, missing and ambiguous members, conditionally available methods, generic implementation conflicts, source identities before erasure, proof-result propagation, failed implementation proofs, logical/physical boundaries, native aliases and export rejection. Every accepted concrete instance passes ordinary elaboration, lowering and kernel validation. Complete the extended repository gate and review the generated manual at desktop and mobile widths.

## Boundaries

Universal checking of generic proofs/defaults, generic trait parameters, supertraits, specialization, negative bounds, generic associated types, higher-ranked lifetime bounds and `dyn` are separate follow-ups. Compiler integration for special Rust traits remains separate. Interior mutability and trait promises retain their existing restrictions.
