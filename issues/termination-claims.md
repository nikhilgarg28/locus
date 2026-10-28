# Claims that runtime functions terminate

- **Kind:** decision
- **Status:** later
- **Blocks:** Total-correctness specifications. Nothing in RFC 0001 or RFC 0002
- **Rules:** `proofs.partial`

## Question

RFC 0002 (`design/rfcs/0002-logical-layer.md`) makes contracts on runtime functions partial: a postcondition holds when the function returns. A function that loops forever proves nothing (rule `proofs.partial`). The vision (commitment 9) says that termination is a separate claim, made explicitly. Open:
- how a runtime function claims to terminate, and to not panic. v0 used promise attributes such as `#[terminates]` and `#[no_panic]`;
- how loops and recursion in runtime code prove the claim, such as with measures;
- what a terminating function's callers gain.

## What it blocks

Total-correctness specifications. Nothing in RFC 0001 or RFC 0002.

## Options

To be laid out in an RFC.

## Log

### 2026-10-25, opened on GitHub as #14

Moved into the repository from [GitHub issue #14](https://github.com/nikhilgarg28/locus/issues/14) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.
