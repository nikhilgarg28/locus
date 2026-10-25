# Litmus examples

Litmus examples are small Locus programs that pin down what the language means. Each asks one question, and its file name gives the answer: the program must be accepted, must be rejected, or the question is still open. They are written before the compiler can run them, and they become its tests once it can.

## Layout

```text
litmus/
  evidence-in-option/
    basic.accept.lc
    stale-after-mutation.reject.lc
    interior-mutability.open.lc
```

A directory holds one topic: variants of one idea that sit on either side of a line. A file is `<name>.<outcome>.lc`, where names are lowercase, hyphenated words and the outcome is one of:

| Outcome | Meaning |
|---|---|
| `accept` | The program must be accepted, and must produce each value its `run` lines give |
| `reject` | The program must be rejected, with each error its annotations give |
| `open` | Not decided. The file links the `decision-needed` issue that will decide it. |

`accept` and `reject` examples are *decided*. Deciding an open question renames its file, for example from `interior-mutability.open.lc` to `interior-mutability.reject.lc`. Changing a decided outcome, or deleting a decided example, is an [expectation change](../AGENTS.md#rules).

## Annotations

Annotations are comments that start with `//~`. The file-level ones come first, in the header comment. A rejected example, `overflow/unchecked-add.reject.lc`:

```rust
//~ question: Is an addition that may overflow rejected?
//~ spec: arith.overflow.checked

fn inc(x: u8) -> u8 {
    x + 1 //~ error[L0204]: overflow is not ruled out
}
```

Its accepted neighbor, `overflow/wrapping-add.accept.lc`:

```rust
//~ question: Does wrapping_add wrap at the type's bound?
//~ spec: arith.overflow.wrapping

fn inc(x: u8) -> u8 {
    x.wrapping_add(1)
}

//~ run: inc(3) => 4
//~ run: inc(255) => 0
```

| Annotation | Where | Meaning |
|---|---|---|
| `//~ question: <text>` | Header, required | The one question the example asks |
| `//~ spec: <id>, <id>` | Header; required when decided | The spec rules the example tests |
| `//~ issue: #<n>` | Header; required when open | The issue that will decide it |
| `//~ rfc: <nnnn>` | Header, optional | The RFC that introduced it |
| `//~ error[Lnnnn]: <text>` | After code on a line | An error with this code must be reported on this line |
| `//~^ error[Lnnnn]: <text>` | Its own line | The same, for the line above; each `^` moves up one line |
| `//~ error: <reason>` | As above | An error without a code yet; the reason says why |
| `//~ run: <call> => <value>` | Its own line | Calling this must produce this value, or `=> panic` |

- Error codes must match exactly. The text after the code needs only to appear in the compiler's message, so rewording a message does not change the language.
- Until error codes exist, a rejected example gives the reason in words. Adding a code later is an ordinary change; changing or removing one is an expectation change.
- Results are checked through calls rather than at a line, because a line can run many times. Each `run` is checked in the interpreter and in the generated Rust. A rejected program never runs, so only accepted examples have `run` lines.
- Open examples may carry proposed annotations. They are not enforced until the example is decided.

## Checks

`cargo xtask check` validates names, annotations, citations, and error codes. Each decided example must cite a rule that exists, each error code must have a [page](../diagnostics/README.md), and two examples in a topic cannot ask the same question. Running the examples starts when the compiler does.
