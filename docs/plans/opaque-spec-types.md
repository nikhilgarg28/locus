+++
id = "opaque-spec-types-plan"
title = "Opaque spec types implementation plan"
group = "Plans"
route = "plans/opaque-spec-types.html"
order = 5
+++

# Opaque spec types implementation plan

This work replaces the preliminary same-named-struct spec implementation with an opaque type contract and a separately named checked representation. The owning roadmap tasks are LOC-252 through LOC-255 in [Rust interoperability](../roadmap/interop.md). No Rust import or assumed implementation is added in this round.

## Contract

- `spec type S { ... }` introduces a nominal opaque type. Its methods, associated constants and supported associated types form its complete public interface. Representation fields and additional backing methods never become public through S.
- `impl S for R { ... }` is the one checked realization. S and R remain different client-facing types. The package declaring S owns its realization; the representation must be accessible using ordinary privacy rules.
- All loaded specs require one complete implementation, including unused specs. Generic realizations cover the declared family exactly once; specialization, overlapping implementations and stronger implementation bounds are rejected.
- Match resolved signatures with alpha-renamed binders and supported associated-type substitutions. Keep proof propositions, lifetimes, receivers and logical modes in the comparison. No proof erasure is used to establish a checked signature match.
- Implementation bodies, generated adapters and evidence all pass normal ownership, effect, IR and kernel checks. Missing bodies and recursive declarations cannot become axioms.
- Define a closed set of representation adaptations. Preserve evaluation order and mutation. Reject unsupported borrowed/container Self results instead of introducing casts, allocations or copies. Generics retain an explicitly documented checking policy; do not describe instance checking as a universal theorem.
- Defer module specs and reject their old syntax with a focused migration diagnostic. Keep the module-spec design on the roadmap. Imports, assumed realizations, macros, arbitrary trait bounds and declaration-only artifacts remain separate work.
- Future `import path [as alias]` only imports physical Rust interfaces. A future `assume R impl S` requires no proof inputs; it may omit output proofs when checking the native signature and supplies audited output assumptions only after a normal return. Neither construct is implemented here.

## Ordered work

1. **Grammar and matching (LOC-252).** Parse type contracts and checked realizations, retain spans, replace token matching with resolved/binder-aware comparison, check member completeness and reject deferred grammar deliberately.
2. **Opaque representation and adapters (LOC-253).** Lower S to an independently checked opaque wrapper around R; check manual bodies and generated conversions. Enforce the public surface in Locus and generated Rust, including proof-output facades and private helper access.
3. **Families, packages and regressions (LOC-254).** Enforce unique family ownership and supported bounds. Exercise split source files, aliases, dependencies, associated items, proof contracts, mutation, both interpreters and hostile Rust clients. Make unsupported forms fail before unchecked output is emitted.
4. **Documentation and gates (LOC-255).** Migrate existing examples and diagnostic fixtures; update the manual, architecture, correctness and formal-core contracts, trust inventory, vision and superseded roadmap text. Run focused suites, all checked documentation, documentation/site validation and the complete extended compiler gate.

## Acceptance

Positive cases include independently named backing structs, constructors and consuming/borrowed receivers, data/proof results, logical methods, constants, associated-type bindings, aliases, separate modules and supported generic families. Generated Rust must compile with warnings denied and preserve observed results/effects.

Negative cases include zero/multiple/incomplete implementations (even unused), foreign-package implementations, stronger bounds, mismatched receivers or lifetimes, changed proof propositions, false evidence, private-field/method leaks, arbitrary raw-to-spec conversions and unsupported Self adaptations. Diagnostics name the failing member and retain header/implementation source locations.

Keep independent kernel, checking-IR and erased validation. Add no axiom or trusted fallback to make adapters pass. Any compiler limitation discovered during this work must be explicit in the manual and tracked with a focused regression and follow-up task.
