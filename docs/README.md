# Documentation sources

The language manual lives in [spec/](spec/), organized by the concepts a programmer needs: types, functions, control flow, ownership, mutation, logical computation, propositions, proofs, models, erasure, Rust integration, and reviewed specifications. [specification.md](specification.md) introduces the book. The [kernel contract](reference/kernel.md) and [formal core](reference/formal-core.md) specify the implementation boundaries.

[Examples](examples.md), [correctness](correctness.md), [performance](performance.md), and the [roadmap](roadmap.md) form the other sections of the public website. [Development](development.md) describes the checks for contributors. Roadmap projects have one owning task per piece of future work; superseded task IDs remain available for historical links.

The public manual describes current behavior. `vision/`, `plans/`, and `archive/` preserve design history and future proposals, and are not published as part of the current manual. Diagnostic explanations remain in `diagnostics/` and are embedded by the compiler.

Markdown is authoritative. Build and preview instructions, front-matter rules, and the migration details are in [website/README.md](../website/README.md).

Concrete interfaces are specified in [spec/18-specifications.md](spec/18-specifications.md). The broader [spec design](vision/spec-interfaces.md) and separate [native import design](vision/rust-imports.md) distinguish implementation scope from planned extensions.
