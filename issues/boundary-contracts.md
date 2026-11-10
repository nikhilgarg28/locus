# Contracts at the boundary with unverified code

- **Kind:** design
- **Status:** later
- **Blocks:** Using a proof to remove a runtime check; taking a fact from a Rust crate that no runtime check can test; specifications of crates other than the standard library
- **Rules:** `logic.unverified.functions`, `logic.unverified.plain-rust`

## Scope

RFC 0002's decisions 54 to 56 draw the line between verified and unverified code at the function, and give an unverified function no contract. It takes and returns no proof, and verified code knows it by its Rust type alone. No program assumes a fact. That is sound and small, and it leaves two things undone:

- **A proof cannot justify skipping a check.** An unverified function must be safe for every argument, so `get_unchecked` cannot be called on the strength of a proof that the index is in bounds.
- **A fact that no runtime check can test,** such as "the result is sorted", cannot be taken from a Rust crate.

This issue is the design of contracts for unverified functions and for functions of other crates: preconditions that every verified caller proves, and postconditions that are assumed. The owner expects to do without them for a while.

## What is already decided

- **Assuming is unsafe.** Once a proof can justify an `unsafe` operation, a false assumption in safe code could cause undefined behavior, which safe Locus must never do (vision, commitment 9). So whatever states an assumption is marked as `unsafe` is, and every result lists the assumptions it relies on. An `assume!(P)` in verified code was rejected for this reason.
- **An unverified body stays plain Rust.** Whatever form a contract takes, the body states and proves nothing.

## Forms considered

None is chosen. They are recorded so that the work does not start from nothing.

1. **Attributes on an unverified function.** A precondition is proved at every call from verified code; a postcondition is assumed, and is a fact that Locus knows after the call.

   ```rust prose a form that was considered, not Locus syntax
   #[unsafe(unverified)]
   #[requires(i < v.len())]
   #[ensures(result == v[i])]
   fn get(v: &[u32], i: usize) -> u32 {
       unsafe { *v.get_unchecked(i) }
   }
   ```

   - The Rust signature is untouched, so the body's type is the declared type, and the same attributes could later sit on a declaration for another crate's function.
   - Rust 2024 writes an attribute that can cause undefined behavior as `#[unsafe(...)]`, which is the mark an assumption needs.
   - A call carries an obligation with no `_` written for it, as an assignment to a field that an invariant reads already does.
   - A function with a precondition must not be reachable from unverified code, as a function that takes a proof is not.
   - Proving such a function later moves its contract into its type, which changes its callers unless a verified wrapper stands in front of it.
2. **Proof parameters and results in the signature,** written `assume fn`, with a body that returns only the runtime part and cannot name its proof parameters. This keeps one notation for every contract, and a function can change between verified and unverified without changing its callers. But the body returns less than its declared type, and a parameter exists that the body cannot name.
3. **Declarations for other crates' functions,** grouped in a block that mirrors the crate, each the function's Rust signature with a postcondition.
   - A contract adds no parameter: a safe function elsewhere is already safe for every input, and a condition goes in the result.
   - A call keeps its Rust type, and the contract adds facts that Locus knows after it. Otherwise adding a contract would break code that compiled.
   - It applies to every call from verified code in the crate that declares it, with no import; a public one applies in the crates that depend on that crate directly, and no further; two contracts for one function visible in one crate are an error; and the declared signature must match the real one, so that a dependency that changes fails to compile.
   - Locus's library would state its specifications of the standard library in this form.
4. **`assume!()`, a proof hole that is trusted, in verified wrappers.** Rejected by the owner, as above. It had the merit that an unverified function never produces a proof.

## Questions it must answer

1. Which form states a contract, and how the form is marked as `unsafe`.
2. What a precondition means for callers that Locus does not verify, and in an exported crate.
3. Whether a postcondition is a proof value in the result or a fact about it, for the crate's own functions and for other crates'.
4. Models for types built on raw pointers, which a contract needs something to speak about, and how Locus rules out a type whose value changes through `&self`.
5. How a result's manifest lists the contracts it assumed.
6. Whether verifying `unsafe` code with permissions can later discharge a contract instead of assuming it, and what that needs from the logic, such as linear ghost values.

## Log

### 2026-11-09, conversation with the owner about PR #24

Raised while the owner and the reviewer reconsidered how Locus treats `unsafe` code. The conversation went through each form above. On an `assume!` in verified code, the owner said:

> While this works, we are making it far too easy to assume anything without any unsafe tainting - ideally assuming it self should be unsafe.

The owner then asked whether any of it was needed for a first version, since Rust code has no proofs in it and an unverified function could simply be kept from taking or returning any, and chose that:

> Yes, this is good and I likely won't have requires/ensures for a while.

So RFC 0002's decision 56 gives unverified functions no contract, and this issue keeps what was learned.
