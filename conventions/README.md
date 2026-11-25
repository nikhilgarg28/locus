# Developer conventions

These chapters give advice for writing Locus libraries and programs. They explain
useful patterns, their tradeoffs, and when another approach is appropriate. They
are **non-normative guidance**: following a convention is not a condition for a
program to compile or a proof to be accepted.

The [specification](../spec/README.md) defines language rules; [RFCs](../design/rfcs/README.md)
record design decisions; [litmus examples](../litmus/README.md) record decided
outcomes. A convention cannot introduce syntax, trust, compiler restrictions, or
an exception to those rules. Where a pattern needs an unfinished language or
library facility, say so rather than treating the advice as its implementation.

## Chapters

1. [Snapshots, views, and stored ghost state](snapshots-and-ghost-state.md)

## Writing a convention

Give the recommendation, explain the problem it addresses, show an ordinary use
case and an important exception, and link the relevant language rules. Keep
requirements in the specification and recommendations here. These chapters have
no rule IDs or mandatory litmus outcomes; examples illustrate advice unless they
link to an independently decided example.
