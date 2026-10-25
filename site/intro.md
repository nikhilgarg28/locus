# Locus

Locus is a superset of Rust with explicit, kernel-checked proofs that cost nothing at run time, inspired by Lean. It is being designed in the open, and the compiler does not exist yet.

This site is built from [the repository](https://github.com/nikhilgarg28/locus) on every change to `main`. This copy is from commit [{{commit}}]({{commit-url}}), {{date}}.

- [Vision](../VISION.md): what Locus is for, and what it is not.
- [Specification](../spec/README.md): the accepted rules. Each rule links to the examples that test it.
- [Litmus examples](../litmus/README.md): small programs that must be accepted, must be rejected, or are still open questions.
- [RFCs](../design/rfcs/README.md): proposals, and the reasons behind each decision.
- [Trust model](../design/TRUST.md): what has to be trusted for a checked program to mean what it claims.
- [Error index](../diagnostics/README.md): what each error code means.

## How decisions are made

A change to what Locus means starts as an RFC. Accepting it adds rules to the specification, and each rule must be tested by litmus examples before it can be accepted. Questions that are not yet decided are published as open examples, so what is unknown is as visible as what is settled.
