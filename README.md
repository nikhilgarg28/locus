# Locus

A superset of Rust with explicit, kernel-checked proofs that cost nothing at run time, inspired by Lean. It is being designed in the open; the compiler does not exist yet.

- [Vision](VISION.md): what Locus is for
- [Specification](spec/README.md): the accepted rules, each with its examples
- [Litmus examples](litmus/README.md): programs that must be accepted, must be rejected, or are still open questions
- [RFCs](design/rfcs/README.md): proposals and the reasoning behind each decision
- [Working on Locus](AGENTS.md): the map, the rules, and the commands, for people and agents alike

The website at [nikhilgarg.net/locus](https://nikhilgarg.net/locus/) is built from this repository on every push to `main`. To build it locally, run `cargo xtask site` and open `target/site/index.html`.
