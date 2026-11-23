# Functions that declare what their result is

- **Kind:** design
- **Status:** later
- **Blocks:** Reading `v.len()`, `v[i]`, and `a == b` as expressions on Locus's own types and on a program's; contracts on the methods of an operator's trait

## Scope

Locus understands a fixed list of runtime operations: after `let hi = xs.len();` on a slice, it knows that `hi` is the slice's length ([runtime-semantics](runtime-semantics.md)). Every other call is known only through its signature, so a fact about its result is a proof in that result, which the caller binds: `let (n, p) = v.len();` (RFC 0002, decision 80).

This issue is a way for a function of the program's own to join the list: to declare that its result is a stated logical expression of its arguments, so that a call of it is read as that expression. Lean needs no such thing, since every function there is one. Dafny separates functions from methods. Prusti and Creusot mark a Rust function `#[pure]`. Verus ties a runtime function to a logical one.

The owner asked what the idea is and how it could be used, and set it aside until loops and overflow are decided:

> We will come back and examine pure functions later after B and C

## What was put to the owner

A function would declare the two things that every understood operation has: its value, and, where it can panic, the condition under which it returns. The spelling is illustrative.

```rust prose a sketch, not Locus syntax
impl<T> Index<usize> for Vec<T> {
    #[pure]
    fn index(&self, i: usize) -> &T
        is self[i]
        returns_if i < self.len()
    { &self.0[i] }
}
```

- The function's Rust type does not change, so the mark works on a trait's method.
- The value is a logical expression of the arguments. So a function that reads a cell cannot be pure, with no rule of its own: a cell's contents are in no snapshot.
- It writes nothing.
- The body is checked against the declaration when it is verified. When it calls unverified code, as a wrapper around the standard library does, it assumes the equation with `unsafe assume!`, and then both the value and the purity are assumed.

What it would be used for:

- the observers of Locus's own types, which settles whether `len` returns a pair ([standard-library](standard-library.md), question 1);
- operators and trait methods on those types, which today tell Locus nothing ([traits-on-logical-types](traits-on-logical-types.md));
- a program's own small accessors, in conditions.

## Questions it must answer

1. **Whether Locus should have it at all.** It is a second way to learn a fact from a call, beside a proof in the result, and the owner turned down facts that are only in scope after a call. The difference is that the function's own author declares it, that it applies only to a result fully determined by an expression, and that it is checked wherever the body is verified.
2. **What it means for a wrapper whose body cannot be verified,** where the value and the purity are both assumed.
3. **How purity is checked:** what a pure function may call, read, and return.
4. **Whether a pure runtime function may be named in logical code,** so that one definition serves both, and what that asks of a function that can panic.
5. **Generic code:** how a function generic in `T: Trait` knows that a method of the trait is pure.
6. **The spelling,** and whether the value is a clause or a proof in the result.
7. **What it cannot reach:** the standard library's own methods on types that stay Rust's, which can only be understood by being on the language's list.

## Log

### 2026-11-17, conversation with the owner on what runtime code tells Locus

Raised as the way to grow the list of understood operations beyond what the language defines. The owner asked for an explanation, which is the text above, and deferred the design until loops and overflow are decided.

### 2026-11-17, conversation with the owner on what runtime code tells Locus, continued

With loops and overflow decided, the owner came back to this: "explore if we need a mechanism for user to opt-in arbitrary functions to mean something for the compiler".

The reply: not to start, and eventually yes for one case with no way round it, an operator or a trait method on a type of the program's own, whose signature the trait fixes. An observer can return its proof, a check can return an `Option` of a proof, and a definition used at run time and in propositions can be written twice. Two forms were put:

- **By body.** A mark under which a function's body is its meaning. Locus checks that the body uses only understood operations and other such functions, takes no `&mut`, and at first has no loop and no recursion. Nothing is declared beside the body, and nothing is assumed. It does not reach a body that calls unverified code, such as a wrapper around the standard library.
- **By declaration.** The form under "What was put to the owner" above: a stated value that the body proves or assumes. It reaches a wrapper, at the price of a second place where a contract lives and of an assumed case.

Either way the mark exposes the body as a contract, as `const fn` does, so it is written and never inferred; and generic code gains nothing until a trait can say that its method is such a function.

The owner decided:

> Let's stay with nothing for now.

[RFC 0004](../design/rfcs/0004-runtime-code.md) records it. This issue stays, as `later`, with the reply's leaning recorded: the form by body, when the question is taken up again.

### 2026-11-22, conversation with the owner on traits

With traits able to hold methods whose signatures carry proofs, the case that had no way round it has one: a law trait beside the standard one, whose own method returns the proof. An operator or the standard method on such a type still tells Locus nothing. The owner was asked whether some calls might be read as expressions after all, starting with a standard method that is generated from a law trait's own, then the observers of Locus's library, then a function of the program's own. The owner:

> Fact is always proof in the result or small/finite list of facts language infers from some constructs (which we have already defined). Don't want to go back on that. And don't want to introduce pure functions either right now.

This issue stays `later`.

### 2026-11-23, RFC 0006 records the decision, and two ideas go to its alternatives

[RFC 0006](../design/rfcs/0006-traits.md) quotes the owner's words of the entry above as its decision 12. Two ideas that its fourth draft had listed as open are not pursued, and are among its alternatives with those words: a condition that binds a law's proof, so that `if h: a < b` would bind what `lt_checked` returns, and a standard method that Locus understands where a law implementation generated it. In runtime code an operator or a standard method on a type that has a law trait states nothing (`traits.operators`), and `traits/runtime-comparison-gives-no-fact.reject.lc` and `traits/standard-clone-gives-no-fact.reject.lc` pin it.

This issue stays `later`.
