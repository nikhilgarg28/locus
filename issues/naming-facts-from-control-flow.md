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
