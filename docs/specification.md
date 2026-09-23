+++
id = "specification"
title = "Language specification"
group = "Now"
route = "specification/index.html"
order = 50
+++

# A language manual, with evidence attached.

This manual describes the language implemented by the compiler. Read it in order, follow a topic, or search for a construct or a stable rule number.

<!-- component: specification -->

## How to read a rule

Every rule keeps a permanent identifier such as **1.5:4**. The identifier survives editorial reorganization; it is not the chapter's current position in this book. Each rule shows its category and the tests that cite it. Expand **Tests** to inspect the source of a focused regression or follow its GitHub link.

Normative rules, syntax rules, legality rules, and dynamic semantics require focused tests. Informative explanations and examples are identified separately. These links make coverage reviewable; they do not prove that a test is sufficient.

## A second level of detail

The [kernel contract](reference/kernel.md) states the trusted proof-checking rules. The [formal core](reference/formal-core.md) describes checking, lowering, and erasure judgments. They are implementation references, separate from the language manual. The formal core is not yet a mechanized proof of compiler correctness.

The [design notes](vision/target-language.md) record the intended direction. Planned syntax there is not a promise that the current compiler accepts it.
