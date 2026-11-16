# Locus's standard library of logical types and models

- **Kind:** decision
- **Status:** open
- **Blocks:** Any proof about standard library data, and the accepted examples in RFC 0002 that use `Seq`, `Vec`, or `vec!`

## Question

What Locus's library provides on top of the language core in RFC 0002 (`design/rfcs/0002-logical-layer.md`, proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10)). This covers:

- **Logical types:** `Nat`, `Bool`, `Seq<T>`, `Set<T>`, `Map<K, V>`, `Maybe<T>`, and characters and strings.
- **Rules:** `logic.boundary.unverified`, `logic.boundary.plain-data`
  - Each is defined from `Int` and inductive types, the core RFC 0002 builds on. Any native acceleration has a stated correspondence (vision, commitment 5).
  - Their positivity in each type argument: `Seq` and `Maybe` are positive, and a set, as a predicate, is not.
  - Indexing of sequences: the index type, what an out-of-range index means, and the bounds proof needed when elements might not exist.
  - `div_euclid` and `rem_euclid` on `Int`.
- **Models of standard types,** each a trusted entry with its reason.
  - RFC 0002 lists integers, `bool`, references, `Box`, arrays, slices, `Vec`, `Option`, and tuples.
  - Still open: `String`, `str`, `char`, `HashMap` and `BTreeMap`, `Rc`, `Arc`, and `Cow`.
  - A type with no model of its own has its opaque identity, `Opaque<T>`. `Cell`, `RefCell`, `Mutex`, and the atomics have nothing else, and `()` needs nothing else.
- **Specifications of standard functions:** what `Vec::push`, `len`, indexing, `vec!`, and the others do to models and to the logical values a container holds. These are assumed specifications, listed by the audit. They include:
  - that the methods of `Cell`, `RefCell`, and `Mutex` keep the types of their contents;
  - **which operations may be given exposed logical data.** Since RFC 0002's decisions 61 and 73, a standard library operation takes a value whose type exposes logical data, such as an `Option<@P>`, only when it has a specification that says what becomes of that data. A value of a sealed type needs none. RFC 0002's examples assume one for `Option`, `Result`, `Box`, `Rc`, `Arc`, `Vec`, arrays, slices, their iterators, channels, and the methods of `Cell`, `RefCell`, and `Mutex`. The list, and each specification, is this issue's. Operations that the language itself reaches for come first: `?`, `for`, `==` on values that hold proofs, `clone`, and dropping a container of proofs.
- **Lemmas** about the logical types.

## Direction

RFC 0002's decision 59 sets where this library is headed: its collections are verified Locus on permissioned pointers, with no `unsafe` code of their own, and Rust's standard library stays available without proofs. So the trusted specifications of Rust's `Box`, `Vec`, and the other collections that this issue lists are a stage, not the end: [permissioned-pointers](permissioned-pointers.md) is the design that replaces them, and it also asks which small part of Rust's library Locus must still understand.

## What it blocks

Any proof about standard library data, and the accepted examples in RFC 0002 that use `Seq`, `Vec`, or `vec!`.

## Options

To be laid out in an RFC.

## Log

### 2026-10-25, opened on GitHub as #11

Moved into the repository from [GitHub issue #11](https://github.com/nikhilgarg28/locus/issues/11) on 2026-10-27. The text above is the issue's as it was then.

### 2026-10-27, comment on GitHub

> Recorded from [#10](https://github.com/nikhilgarg28/locus/pull/10) (RFC 0002, decision 34). The standard library's RFC should also give a form for **specifications at the boundary**, which describe code that Locus cannot check, such as a wrapper around C. It should state, each as a separate assumed specification that results list:
>
> - which places the code may write;
> - whether it retains access to them beyond the call;
> - what it initializes;
> - what it returns, as in "returns at most the buffer's length".
>
> Writing `unsafe` authorizes no fact by itself. Until this form exists, such code is treated as unknown code: it may write, and retain access to, everything its access allows.
>
> The same RFC also owns:
> - the descriptions of what access each standard type grants: `Cell`, `RefCell`, `Mutex`, guards, mutable iterators, entries, and channel senders all grant write access, while `&`, `Rc`, `Arc`, and shared iterators do not. A type without a description is taken to grant everything.
> - the effects of standard functions, such as `Vec::push`, which writes only its vector and retains nothing.

### 2026-10-27, PR #10

Now cites the spec rules it is about, which RFCs 0001 and 0002 add, so that `cargo xtask context` and the site show it beside them.

### 2026-10-28, PR #10

After the third review, the owner decided that closures that take proofs stay off the standard library's adapters until this issue specifies them, and that doing so is a priority for adoption: RFC 0002, decision 43. Such a specification says that the callee calls the closure only with values it was given, keeping their logical contents, not only their erased form. `Option::map` calls it exactly when it holds `Some(v)`, with that `v`, and `None` never calls it. Adapters that keep the closure, call it many times, or unwind while it runs need more than that.

### 2026-11-09, fourth review of PR #10

RFC 0002's decisions 47 and 48 add two conditions that this issue's specifications must keep. A proposition under interior mutability reads only what is fixed where its type is written, which is what lets the methods of `Cell`, `RefCell`, and `Mutex` keep the types of their contents. And the snapshot that the library declares for a collection is strictly positive in its element types, so a set's or a map's snapshot is the sequence of its entries.

### 2026-11-09, conversation with the owner about PR #24

RFC 0002's decision 56 leaves Locus's library as the only place that states specifications of code Locus does not verify: a program has no form for them yet. This issue still owns the library's own specifications. The form in which a program could state a contract for another crate's function, which the library's specifications might share, is [boundary-contracts](boundary-contracts.md), recorded for later.

### 2026-11-09, conversation with the owner about PR #24, continued

The owner set the direction that decision 59 records, quoted in [permissioned-pointers](permissioned-pointers.md). It adds questions for this issue's RFC: which standard types Locus's library replaces with its own, what `Vec` names in a `.lc` file, how a Locus collection meets a Rust crate that expects the standard one, and what happens to RFC 0002's examples, which use Rust's `Vec` and `Option` under trusted specifications until then.

### 2026-11-16, conversation with the owner on how to build Locus

RFC 0002's decision 61 replaces the analysis of what access each standard type grants with one rule: unverified code handles only plain data, and a library operation is given logical data only when Locus's library specifies it. So this issue no longer owns a list of completable types, or a list of types writable through a shared handle. It owns the list of operations that have a specification, and each specification must say what becomes of the logical data the operation is given, which is more than what the operation does to Rust values.

A specification of a generic operation must hold for every implementation a caller can supply. `sort` gives a sorted slice only when `Ord` is a total order, so the specification asks for that as a proof and does not assume it. The cases in "Memory Safety Bugs in Rust Standard Library" (Microsoft FORGE Lab, 2026) are of this kind, and three of them are now litmus examples: `contracts/same-call-twice`, `contracts/built-before-filled`, and `logical-layer/trait-result-trusted`.

### 2026-11-16, conversation with the owner on how to build Locus, continued

Decision 73 of RFC 0002 narrows what needs a specification. A value of a sealed type, a struct or enum with proof fields of the program's own, may be given to any library operation, specified or not: a container of another crate can hold it, and what comes back is some value of the type. A specification is needed only for exposed logical data, such as an `Option<@P>`, and for knowing which value comes back.

### 2026-11-16, conversation with the owner on the standard library

The owner asked how Locus knows that an `UnsafeCell` has no model, whether a call into the standard library tells Locus anything, and how a specification would work, and broadly agreed with the reply. This is the starting position it gives this issue's RFC:

- **Opaque is the default.** A standard type's snapshot is structural, and it has a model, only where Locus's library says so. The language forbids two things to every declaration, the library's included: looking inside an `UnsafeCell`, and behind a raw pointer (`models.frozen-data`). So `Cell`, `RefCell`, `Mutex`, and the atomics need no entry.
- **`Rc` and `Arc` are to be transparent, as `Box` is, and `Weak` opaque.** While a target is shared it is reached only by shared reference, and the counts are in cells. They move from "still open" above to what this issue's RFC proposes.
- **A call without a specification gives its Rust type and nothing else.** A type without a model has none, which is not the same as a unit model: using a value of it as a value in logical code is an error.
- **A specification,** as proposed:
  - is a declaration that repeats the function's Rust signature, so that a change to the function fails to compile, and adds a proposition about its result over the models of its arguments;
  - is assumed and listed, never proved;
  - attaches to the function that rustc resolves the call to, so a call through a trait bound gets only what the trait states;
  - says what becomes of any logical data the operation is given;
  - is tested against the standard library, and for the collections that Locus rebuilds with the same layout (RFC 0002, decision 75) the same statement is proved of Locus's version.
- **The owner added two requirements,** which RFC 0002's decision 78 records:

  > The syntax for attaching post conditions should not be special cased to stdlib and should be available for user for their own code/crates too. And it should explicitly mention assume or unsafe or something like that.

  The owner also asked why a specification should not state a precondition. That question is in [boundary-contracts](boundary-contracts.md).
- **Fewer operations need a specification to be given exposed logical data.** Since RFC 0002's decision 76, an operation that only reads through a shared reference and returns plain data, such as `len`, `is_some`, or a `Debug` implementation, is lent it without one. It still needs one for Locus to know its result.

### 2026-11-16, conversation with the owner on reaching the standard library through Locus's own types

The owner liked the direction recorded in [boundary-contracts](boundary-contracts.md), in which Locus's library exports its own types whose methods have Locus signatures, over specifications attached to the standard library's functions. One thing is decided, and four questions were raised.

**Decided: the import is explicit, and its name says what it shadows.**

> And I'd prefer to use explicit `use locus::prelude::...` infact, maybe something even more explicitly suggesting that we are shadowing rust stdlib? `use locus::rust::std::prelude` or something like that?

So no rule of Locus's own decides which `Vec` a name means: Rust's rule, that an import shadows the prelude, does. The path is open; the reply suggested `locus::std`, mirroring the standard library's module tree.

**Raised by the owner, with the reply. None is decided.**

- *Must each method of the wrapped type be written again?* No. `Deref` and `DerefMut` to the inner `Vec` make every method that takes `&self` or `&mut self` callable on the wrapper, and a method the wrapper defines wins over the inner one of the same name. So the library writes only the methods it gives contracts. Constructors, methods that take `self` by value, and trait implementations are not forwarded.
- *Why mark the function, and not construct the proof with `unsafe assume!(_)`?* The reply agreed that the constructor is the better way to assume a fact: one proposition is assumed, the rest of the function is still checked, and it answers the reason `assume!` was rejected before, which was that nothing marked it as `unsafe` is. A function whose whole body is trusted can do two things that a constructor of proofs cannot, neither of them a fact: use `unsafe` Rust behind a proof parameter, and hand logical data to unverified generic code, as a wrapper's `push` does when its element type holds a proof.
- *Do `Result`, `Option`, and `Box` need to be swapped at all?* The reply: no. A type is swapped only when it has private state, so that its methods are the only way to learn about a value. The proposed split:
  - kept, with operations whose meaning the language defines, and a model: integers, `bool`, `char`, references, tuples, arrays, slices, `Box`, `Option`, `Result`, and ranges;
  - Locus's own type around the standard library's: `Vec`, `String`, the maps and sets, `VecDeque`, `Rc`, `Arc`, the cells, the locks, and the atomics;
  - Locus library functions, under names of their own, for standard methods on the kept types that a program wants a fact from, such as `checked_add` or `Option::map`, since an inherent method of a type that is kept cannot be shadowed. Most can be verified, not assumed;
  - untouched and called as plain Rust: everything else.
- *What this costs.* An operator or a trait method has the signature that the standard library's trait gives it, so it cannot return a proof. On Locus's own types `v[i]`, `*rc`, `for x in &v`, and `v.clone()` run and tell Locus nothing, and the named method is how a program gets a fact. Contracts on trait methods ([traits-on-logical-types](traits-on-logical-types.md)) are what would lift this.

