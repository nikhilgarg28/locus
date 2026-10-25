# Error index

Every error code the compiler reports has a page here, `Lnnnn.md`, explaining what it means and how to fix it. The same pages are shown by `locus explain Lnnnn` and published on the website. There are no codes yet: pages arrive with the compiler, as it starts to report errors.

## A page

````markdown
# L0204: arithmetic may overflow

The result of an arithmetic operation is used without evidence that it fits its type.

```rust reject L0204
fn inc(x: u8) -> u8 { x + 1 }
```

Prove the bound, or say which overflow behavior you want:

```rust check
fn inc(x: u8) -> u8 { x.wrapping_add(1) }
```

Defined by: [arith.overflow.checked](../spec/arithmetic.md#r-arith.overflow.checked)
````

- The title is `# Lnnnn: <meaning>`.
- At least one `rust reject Lnnnn` example shows the error, and examples that fix it use `rust check`. Once the compiler exists, every example is run and must behave as its fence says.
- Link the spec rules the error enforces.

## Codes

- A code is `L` and four digits. Pick the next unused number; if two PRs collide, the later one renumbers.
- Codes are permanent and never reused. A code the compiler no longer reports keeps its page with a `- **Status:** retired` line, and needs no example.
- `cargo xtask check` requires a page for every code a litmus example expects; once the compiler exists, it will also require a page for every code the compiler can report.
