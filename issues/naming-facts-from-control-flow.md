# Naming the facts that control flow puts in scope

- **Kind:** design
- **Status:** later
- **Blocks:** Using a branch's condition in a proof without restating it; holding every fact as a proof value

## Scope

A branch puts a fact in scope. Inside `if n < 100 { ... }`, `n < 100` holds, and today the only way to hold it as a value is to restate it: `let small: @(n < 100) = _;`. The owner wants the fact to be named where the construct that introduces it is written:

> And re: naming proofs in ifs, yes, I have been meaning to do that. Maybe we can use if h: x < 0: { here h refers to proof} or something like that. Ideally we will make it work with match or whatever else introduces atomic facts in the scope.

So `if h: x < 0 { ... }` would bind `h`, of type `@(x < 0)`, in the branch. Lean writes this `if h : c then t else e`.

This fits a preference the owner stated in the same conversation, that a fact should be a proof held as a value and not something that is only in scope ([boundary-contracts](boundary-contracts.md)).

## Questions it must answer

1. **The spelling,** which must not be valid Rust and should read as Rust. `if h: x < 0 { }` is the owner's first suggestion.
2. **What the `else` branch binds:** the same name at the negated proposition, or a name of its own.
3. **Every construct that introduces a fact:**
   - `if`, and `else if` chains, where each branch also knows that the earlier conditions failed;
   - `match` arms and their guards: the scrutinee has this arm's shape, and matched none of the earlier ones;
   - `if let`, `let else`, and `while let`;
   - a `while` condition, and what holds after the loop;
   - `&&` and `||`, which evaluate their right side only under a fact about their left;
   - `assert!`, and a call that returns `bool`.
4. **Which conditions can be named.** A condition is a runtime expression, and a proposition is about values. A condition that calls a function needs the call bound first, or a rule for what the call's result is in the proposition.
5. **Whether an unnamed condition still gives `_` its fact,** as it does today, or whether a fact must be named to be used.
6. **How this relates to tracked evidence,** `let mut h: @P`, which RFC 0002 defers to the runtime semantics ([runtime-semantics](runtime-semantics.md)).

## Log

### 2026-11-16, conversation with the owner on specifications

Raised by the owner while discussing how an assumed postcondition reaches its caller. The reply had compared a fact left in scope by a call with the fact that an `if` leaves in scope, and noted that neither can be named without restating its proposition. The owner asked for a note, to be taken up later:

> Can you please take a note to do this, maybe bit later?

### 2026-11-16, proposal made at the owner's request

The owner asked for an audit of the decisions that are still open, with a proposal for each. This is the proposal for this issue. Nothing in it is decided.

In the order of the questions above:
1. **Spelling:** the owner's `if h: x < 0 { ... }`. In an expression, Rust accepts no `name:` before a condition, so it is not valid Rust today, and it reads as a `let` with a type does.
2. **`else`** binds the same name to the negation: in `if h: c { A } else { B }`, `h` proves `c` in `A` and `!c` in `B`. One name, as in Lean. A chain of `else if` gives each branch its own name, and the earlier names stay in scope as negations.
3. **Constructs,** in the order to add them: `if` and `else if`; `while h: c`, with `h` in the body; `match` guards, `Some(x) if h: x > 0 =>`; then `if let` and `let else`, where the fact is that the value has the pattern's shape, written `if let Some(x) = opt @ h`; and last `&&` and `||`. What holds after a loop is the loop's invariant and the negated condition, and belongs to the runtime semantics.
4. **Which conditions:** those built from the operations that the language defines. A condition that calls a function binds the call first.
5. **An unnamed condition still gives `_` its fact.** Otherwise every existing example would need a name. Naming is for when a person or a proof needs to refer to it.
6. **Tracked evidence** stays with the runtime semantics.

### 2026-11-17, conversation with the owner on terms and procedures

[RFC 0003](../design/rfcs/0003-proof-construction.md) takes the first of the forms proposed above, `if h: c { A } else { B }`, as one of the term forms that the owner accepted, with the rule `proofs.facts.named`. In `A`, `h` proves `c`; in `B`, it proves `!c`, which is this issue's question 2 and is still the RFC's proposal, not the owner's words. Questions 3 to 6 stay here: `while`, `match` guards, `if let`, which conditions can be named, and tracked evidence. An unnamed condition still gives a hole its fact (`proofs.holes`).

### 2026-11-17, conversation with the owner on what runtime code tells Locus

Question 6 above, how this relates to tracked evidence, is answered by [RFC 0004](../design/rfcs/0004-runtime-code.md). There is no evidence that follows a version. A proof variable declared with `let mut` is assigned again by the program, and it proves its proposition for the current values where it does so on every path. So the name of a `while`'s condition, when this issue adds it, is one more proof in the body, and what holds after the loop is what such variables prove there.
