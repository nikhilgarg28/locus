+++
id = "language-integers"
title = "Integers and operators"
group = "Now"
spec_chapter = 1
order = 102
route = "specification/integers.html"
description = "Machine arithmetic has Rust semantics; mathematical observations use Int."
+++

# Integers and operators

<!-- spec: 1.0:4 informative -->
Use machine integers for runtime arithmetic and logical `Int` values to describe mathematical properties. The distinction matters at overflow and division boundaries: a proof must describe the operation the runtime actually performs.

## Machine arithmetic

<!-- spec: 1.15:1 syntax -->
Literals are written as in Rust: decimal, `0x`, `0o`, `0b`, underscores, and a suffix, `255u8`, `1_000u16`. A literal takes the type expected of it, else its suffix, else `i32`; `-128` is one literal; `T::MAX` and `T::MIN` are literals of `T`. `a as T` between machine types is Rust's `as`: it wraps, sign-extends, and truncates as Rust does, at every pair of types. `==`, `!=`, `<`, `<=`, `>`, `>=` compare two values of one machine type, and `==` and `!=` two `bool`. `a.wrapping_add(b)`, `wrapping_sub`, `wrapping_mul`, and at a signed type `wrapping_neg` never panic.

<!-- spec: 1.15:2 syntax -->
`+`, `-`, `*`, `/`, `%`, and unary minus on a machine type mean what they mean in Rust: `+ - *` and unary minus panic on overflow in a build with overflow checks and wrap in one without; `/` and `%` panic on a zero divisor, and at a signed type on `MIN / -1`, in every build. The operator tests exercise the boundaries of every machine type. On normal return, the checker records the modular result: for u8 addition this is the internal kernel equation `s == wrap[u8](view(a) + view(b))`, which is valid in both overflow modes. This equation is not permission to cast a source Int into runtime data. After `/` and `%`, normal return also establishes a nonzero divisor and, for signed operands, excludes the `MIN/-1` pair. Under `no_panic` an operator carries an obligation, the condition under which it does not panic, stated over the views, `a as Int + b as Int <= u8::MAX as Int` and its lower bound for `+`, `b as Int != 0` for `/`, and filled as a hole is; afterwards the exact result is known, `s as Int == a as Int + b as Int`. Bit operators and shifts are not in Locus.

<!-- spec: 1.15:3 syntax -->
In a claim an ordering of two machine values is the ordering of their views, and `as Int` says so explicitly. Arithmetic on `Int` is total and may stand anywhere in a formula. Division truncates toward zero; `a / 0` is `0` and `a % 0` is `a` in logical integer arithmetic. These total definitions do not change the panic behavior of runtime machine division. A source cast from Logical Int to a machine type is rejected, including inside a proposition.
