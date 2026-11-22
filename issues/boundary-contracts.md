# Contracts at the boundary with unverified code

- **Kind:** design
- **Status:** open
- **Blocks:** Using a proof to remove a runtime check
- **Rules:** `logic.unverified.functions`, `logic.unverified.plain-rust`, `logic.boundary.foreign`, `proofs.assume`, `proofs.assume.lint`

## Scope

RFC 0002's decisions 54 to 56 draw the line between verified and unverified code at the function, and give an unverified function no contract. It takes and returns no proof, and verified code knows it by its Rust type alone. No program assumes a fact. That is sound and small, and it leaves two things undone:

- **A proof cannot justify skipping a check.** An unverified function must be safe for every argument, so `get_unchecked` cannot be called on the strength of a proof that the index is in bounds.
- **A fact that no runtime check can test,** such as "the result is sorted", cannot be taken from a Rust crate.

This issue is the design of contracts for unverified functions and for functions of other crates: preconditions that every verified caller proves, and postconditions that are assumed. The owner expects to do without them for a while.

## What is already decided

- **Assuming is unsafe.** Once a proof can justify an `unsafe` operation, a false assumption in safe code could cause undefined behavior, which safe Locus must never do (vision, commitment 9). So whatever states an assumption is marked as `unsafe` is, and every result lists the assumptions it relies on. An `assume!(P)` in verified code was rejected for this reason.
- **An unverified body stays plain Rust.** Whatever form a contract takes, the body states and proves nothing.
- **One form, for programs as for Locus's library, which says that it is assumed** (RFC 0002, decision 78).
- **A contract is a signature, and a program assumes with `unsafe assume!(_)`** (RFC 0002, decision 80, which replaces decisions 78 and 79). A function of the program's own wraps the code that Locus does not verify. Its proof parameters are its preconditions, the proofs in its result are its postconditions, and where its body cannot prove one it assumes it. So questions 1, 2, 3, and 5 below are answered: the form is a proof constructor marked `unsafe`; a precondition binds the callers of the wrapper, which are verified code; a postcondition is a proof value; and each result lists its assumptions. The second gap under Scope is closed.
- **What is left** is the first gap: a proof cannot yet justify skipping a check, because a verified function contains no `unsafe` code. That, and questions 4 and 6, now rest on [permissioned-pointers](permissioned-pointers.md).

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
6. Whether verifying `unsafe` code with permissions can later discharge a contract instead of assuming it. RFC 0002's decision 59 takes that road for Locus's own library ([permissioned-pointers](permissioned-pointers.md)), whose pointer primitives need exactly the contracts this issue is about.

## Log

### 2026-11-09, conversation with the owner about PR #24

Raised while the owner and the reviewer reconsidered how Locus treats `unsafe` code. The conversation went through each form above. On an `assume!` in verified code, the owner said:

> While this works, we are making it far too easy to assume anything without any unsafe tainting - ideally assuming it self should be unsafe.

The owner then asked whether any of it was needed for a first version, since Rust code has no proofs in it and an unverified function could simply be kept from taking or returning any, and chose that:

> Yes, this is good and I likely won't have requires/ensures for a while.

So RFC 0002's decision 56 gives unverified functions no contract, and this issue keeps what was learned.

### 2026-11-09, conversation with the owner about PR #24, continued

The owner confirmed that an unverified function takes no type that holds logical data, at any depth (RFC 0002, decision 58), and set the direction for Locus's own library: verified collections on permissioned pointers (decision 59). The primitives under those collections are unverified code with preconditions and postconditions, so Locus's library will need a contract form before a program does.

### 2026-11-16, conversation with the owner on how to build Locus

Two items of the plan now rest on this issue's subject: the pointer primitives of [permissioned-pointers](permissioned-pointers.md), which decision 71 of RFC 0002 makes planned work, and leaving out a check that a proof makes unnecessary ([no-panic-claims](no-panic-claims.md)). Both need a function whose signature states a contract and whose body Locus does not verify. Since decision 65 that is a third kind of function, beside verified and unverified: its signature holds logical data, which only a verified function's may, and its body contains `unsafe`, which a verified function's may not. Only Locus's library has any until this issue is decided, as its trusted specifications.

### 2026-11-16, conversation with the owner on the standard library

Discussing how the standard library's functions would be specified, the owner set two requirements for the form:

> The syntax for attaching post conditions should not be special cased to stdlib and should be available for user for their own code/crates too. And it should explicitly mention assume or unsafe or something like that.

RFC 0002's decision 78 records them. So there is one form, used by Locus's library and by programs, for the standard library's functions, another crate's, and a program's own unverified functions; and it says in its text that what it states is assumed, as "Assuming is unsafe" above already asks. Decision 56's rule stands until the form is designed.

The owner also asked:

> And why not permit preconditions?

The proposal had left them out, on the ground that a safe function accepts every input. The reply separates three cases by who can be made to prove a precondition. It is a proposal, and the owner has not decided on it:

- **A safe function that Rust code already calls,** such as the standard library's. A precondition there can oblige nobody. Callers that Locus does not verify prove nothing, and Rust code such as `v[i]` sits in functions that are verified with nothing to prove (RFC 0002, decision 65), so it would stop compiling, against the vision's first commitment. What reads as the precondition of such a function is its panic condition: "panics only when `i >= len`". A specification can state that, and Locus would hold a caller to it exactly where the caller claims `#[no(panic)]` ([no-panic-claims](no-panic-claims.md)).
- **An `unsafe fn`.** Its precondition is its safety contract, which every caller already promises by writing `unsafe`. A specification should state it, and a verified caller would prove it. This is what lets a proof remove a check, the first gap under Scope above. It needs a change to decision 65, under which verified code calls no `unsafe fn`, and a place at the call to write the proof.
- **A program's own function that only verified code can reach.** Its precondition is a proof parameter, as for every verified function, and no clause is needed. If its body is unverified, the mark that says "assumed" is what lets it keep that signature: this is the third kind of function that the previous entry describes.

### 2026-11-16, conversation with the owner on specifications

On the proposal above, which allowed a precondition in two of its three cases, the owner decided against preconditions:

> I'm okay with no preconditions - let's keep it simple like that.

RFC 0002's decision 79 records it. A specification states what is assumed of a function's result, and obliges no caller. So verified code still calls no `unsafe fn`, and a proof cannot yet justify skipping a check. Stating when a safe function panics, which `#[no(panic)]` will need, was part of the same proposal, and is left to [no-panic-claims](no-panic-claims.md).

The owner then asked for the form to be designed, with its visibility:

> But I do want to iterate on the syntax for specifying these assumed post conditions + figure out visibility rules. Can I take a crate, add assumed post conditions and after that, who all sees those post conditions? Only in that crate or outside? Can I rename the type while doing so?

So this issue moves from `later` to `open`. The design has started in conversation, and nothing in it is decided.

### 2026-11-16, conversation with the owner on specifications, continued

The form proposed in reply was an attribute, `#[unsafe(assume(P))]`, written on a function's definition or in an `extern` block that repeats another crate's signatures. It left the function's Rust type unchanged, and put the fact in scope after the call, as an `if` puts its condition in scope. The owner asked what the return type was, and on hearing that it held no proof, decided against that:

> I don't like just assuming facts in scope - prefer these to become native proofs in returns (like the way we use extern in rust - we just assume it behaves in a certain way).

So an assumed postcondition is a proof in the function's result, as a verified function's is, and what is assumed is that the body honours the signature. That is form 2 above. Forms 1 and 3 as written are ruled out, since neither can put a proof in the result of a function whose Rust type is already fixed. Naming the fact that an `if` puts in scope is now [naming-facts-from-control-flow](naming-facts-from-control-flow.md).

The owner then restated the goal, and proposed another way to reach it:

> Let's take a step back though - the main goal we are trying to solve is to make it possible for us to use rust stdlib from within Locus in a way that we can supply chosen post conditions (and maybe pre-conditions too - we talked about it earlier but didn't pursue due to complexity). What if we just did that by having locus stdblib exporting types or even modules and somehow played with library visibility rules that ensure for instance Option or Vec by default come from the right library depending on the right context?

The reply, on which nothing is decided:

- **One primitive, the assumed function:** a function with a Locus signature, proof parameters and proofs in its result, whose body is plain Rust that Locus does not check, marked as `unsafe` is. Its visibility is the function's own. A precondition is a proof parameter, so preconditions need no feature of their own. A type from another crate is wrapped in a type of the program's own.
- **Locus's library is then ordinary Locus code:** its own collection types, first as wrappers around the standard library's whose methods are assumed, later verified on permissioned pointers with the same signatures (RFC 0002, decision 75). It would replace the trusted specifications of standard functions that decision 56 kept, and the exception that lets a specified operation take exposed logical data.
- **Rust forces the types to differ.** `v.len()` on the standard library's own `Vec` resolves to the standard library's `len`, which returns a `usize`. A method that returns a proof belongs to another type.
- **Which `Vec` a name means is decided by Rust's own rule,** that an import shadows the prelude: `use locus::prelude::*;`. An implicit switch inside Locus code would change what valid Rust means there.
- **What cannot be swapped:** `Option`, `Result`, slices, arrays, `Box`, integers, and ranges, which the language and every standard signature use. Those would be understood by the language itself, not by assumptions in a library.

### 2026-11-16, conversation with the owner on specifications, concluded

The owner accepted the proposal of the previous entry, with one change of his own: the assumption is a constructor of proofs, not a mark on a function.

> why put unsafe assume as function decorator vs using 'unsafe assume!(_)' as the constructor for constructing the proof which can then be used anywhere, even in internal body of functions?

RFC 0002's decisions 80 to 83 record the result, and the text above the log is brought up to date. Two of the owner's questions are written into RFC 0002 as proposed and are still for the owner: whether a function that assumes a fact stays verified (proposed: yes), and whether the form is `unsafe assume!` or `assume!` alone (proposed: with `unsafe`).

This issue stays open for what a constructor of proofs cannot do: let `unsafe` code sit behind a proof parameter. The other thing it cannot do, hand logical data to unverified generic code, is trusted in Locus's own library alone (decision 83).

### 2026-11-21, conversation with the owner on foreign code

RFC 0002's decision 88 fixes what crosses to foreign code: plain data only, in both directions and through function pointers whose ABI is not Rust's (`logic.boundary.foreign`). A sealed type could cross before, in the signature of a function that foreign code can call. The owner:

> shouldn't be possible for foreign code to work with logical data i.e. we need some sort of boundary. Maybe all extern functions are unverified and the same rules apply?

The rule proposed in reply restricts the signature alone, so that a safe shim can check its input with a verified constructor, and the owner agreed:

> On extern, agree with your view.

What this leaves for this issue is unchanged. A contract for a foreign function is still a wrapper of the program's own, whose signature states it and whose body assumes it with `unsafe assume!(_)`. The trust model's new entry, `assumed.foreign`, says only that foreign code keeps to its declared signatures; a contract would say what it does.

### 2026-11-22, six rules confirmed tentatively

A review of RFC 0002 had left six rules unsettled, and they were put to the owner as a table of one line each. The owner answered:

> tentatively confirm the 6 open questions

Three are about assumptions and the boundary, and are RFC 0002's decisions 90, 93, and 94:

- **What a result's list of assumptions covers.** A function's result is a theorem about valid arguments. It lists the assumptions in its own body and in the functions it calls, and not one made where a value it is given was built. The program's result lists every assumption in every crate it links, and is the list promised complete. `proofs.assume` says so now.
- **A lint.** `unsafe assume!` keeps its spelling. Since the program that rustc compiles holds no `unsafe` for it, Locus has a lint, `locus::assumptions`, which a crate sets to forbid assumptions in its own code (`proofs.assume.lint`). The name, the levels, and the default are RFC 0002's proposal, which the owner has not seen.
- **Loans through a generic function of another crate.** An interface records, for each type parameter, whether its values never reach unverified code, are only lent, or reach it otherwise, with the type of each value lent, so that the caller checks the loan at its own type (`logic.ghost.generic-instantiations`).

All three stay open to revisit: RFC 0002 lists them, with what it chose in writing them, as item 26 of what is still for the owner. No example can show what a result lists, and the part of the third that crosses crates has no example either ([multi-file-litmus](multi-file-litmus.md)).
