# Locus's standard library of logical types and models

- **Kind:** decision
- **Status:** open
- **Blocks:** Any proof about standard library data, and the accepted examples in RFC 0002 that use `Seq`, `Vec`, or `vec!`
- **Rules:** `logic.boundary.unverified`, `logic.boundary.plain-data`, `proofs.assume`

## Question

What Locus's library provides on top of the language core in RFC 0002 (`design/rfcs/0002-logical-layer.md`). RFC 0002's decisions 80 to 83 settle how it reaches Rust's standard library: through types of its own, named by an explicit import, whose contracts are in their signatures and are assumed with `unsafe assume!` until they are verified. Decisions 113 to 118 say what a collection among them is: a struct that keeps its contents in a logical field, with contracts that speak of the elements' snapshots. What the library holds is below. Everything in this list is proposed, for this issue's RFC, and nothing in it is decided beyond those decisions.

### 1. Logical types

These are erased, and exist only to state and prove facts. `Int` is the language's; the library builds the rest from it and from inductive types (vision, commitment 5).

- **Numbers and truth:** `Nat`, `Bool`.
- **Collections:** `Seq<T>`, `Set<T>`, `Map<K, V>`, and `Multiset<T>`, which is what "the same items in another order" is stated with.
- **Sums:** `Maybe<T>`, and a two-sided one for `Result`, as the models of `Option` and `Result`.
- **Text:** characters, and strings as sequences.
- For each: its definition, its positivity in each type argument (`Seq` and `Maybe` are positive, and a set, as a predicate, is not), and a stated correspondence for any native acceleration.
- **Fixed-width arithmetic:** what wrapping, saturating, and bitwise operations mean on `Int`, so that machine arithmetic has something to be equal to; and `div_euclid` and `rem_euclid`.
- **Indexing of sequences:** the index type, what an index outside the sequence means, and the bounds proof needed when elements might not exist.

### 2. Logic functions and predicates

The vocabulary that contracts are written in.

- **On sequences:** `len`, indexing, `push`, `concat`, `take`, `skip`, `update`, `subrange`, `rev`, `first`, `last`, `contains`, `count`, `sum`, `min`, `max`, and `map`, `filter`, `fold`, and `zip` over logical functions.
- **On sets and maps:** membership, insertion, removal, union, intersection, a map's domain, and equality by contents.
- **Predicates:** `Sorted`, `Permutation`, `Distinct`, and the like. One definition of each, so that two crates mean the same thing by "sorted".

### 3. Lemmas

Facts about the logical types, each a logic function with a proof that the kernel checks. None is assumed.

- **Arithmetic:** linear facts, multiplication, division and remainder, powers, bit operations, the bounds of each machine integer type, and when an operation overflows.
- **Order:** the laws of a total order, for the built-in orders.
- **Collections:** what each function above does to length and contents, equality by contents, and facts about permutations.
- **Induction:** on `Nat`, on `Seq`, strong induction, and recursion on a measure.
- Whether lemmas are written in Locus or brought from Lean's libraries, whose logic Locus follows, is a question for [proof-construction](proof-construction.md).

### 4. Proof helpers

What makes a proof short to write and to read. The design is [proof-construction](proof-construction.md)'s; this is the part of it that is a library.

- **Procedures that return proofs:** linear arithmetic, bit-vector facts, evaluation of a closed proposition, and simplification with a chosen set of lemmas. Each returns a proof that the kernel checks, so none is trusted.
- **The search behind `_`,** and the lemma sets it may use.
- **Helpers for reading a proof's state:** showing the proposition to be proved and the proofs in scope ([proof-debugging](proof-debugging.md)).

### 5. Models of Rust's own types

Each is a trusted entry with its reason (`trusted.library-specs`).

- **In RFC 0002:** integers, `bool`, references, `Box`, arrays, slices, `Option`, and tuples. The standard library's `Vec` has neither a snapshot nor a model (decision 113): one received from Rust code is converted with `from_std`, and its contents are then known through the methods of Locus's `Vec`. The model of Locus's own `Vec` is not a trusted entry: the library declares it with the type, and Locus checks it.
- **To add:** `char`, `str`, `Result`, ranges, and `Rc` and `Arc`, which are to be transparent as `Box` is, with `Weak` opaque.
- **Opaque:** a type with no model has its opaque snapshot. `Cell`, `RefCell`, `Mutex`, and the atomics have nothing else.

### 6. Runtime types of its own, under `locus::std`

RFC 0002's decisions 81 to 83. `locus::std` mirrors the standard library's modules, so that a path is found by putting `locus::` in front.

- **Types around the standard library's:** `Vec`, `String`, `VecDeque`, the maps and sets, `Rc`, `Arc`, the cells, the locks, and the atomics. Each holds the standard library's type, dereferences to it by shared reference so that its methods that take `&self` are there, converts to and from it in constant time, and gives contracts in the signatures of the methods it defines. A collection among them keeps the snapshots of its elements in a logical field, gives no mutable access to the standard library's collection inside it, and has `into_std` and `from_std` as the way out and back (decisions 113 to 118).
- **Functions for the types that stay Rust's,** under names of their own, for when a program wants a fact from a standard method:
  - `option` and `result`: `map`, `and_then`, `take`, `replace`, `unwrap_or`, written with `match` and so verified;
  - `num`: checked, wrapping, and saturating arithmetic, `min`, `max`, `abs`, `pow`, verified against the arithmetic of section 1;
  - `slice`: `split_at`, `swap`, `reverse`, `binary_search`, `sort`, verified where a safe body can be written;
  - `mem`: `swap`, `replace`, and `take`, which cannot be written in safe Rust and so are assumed, or primitives.
- **Carrying proofs through closures,** as `Option::map` and the iterator adapters do (RFC 0002, decision 43).
- **A prelude,** `locus::std::prelude`, which is the standard library's with Locus's types in place of the ones it swaps.

### 7. Permissions and pointer primitives

Later: [permissioned-pointers](permissioned-pointers.md). They are what the types of section 6 are rebuilt on, after which their contracts are proved and not assumed.

### 8. Tests of what is assumed and trusted

- Each assumption in the library, and each model, compared with the standard library's behavior by tests generated from the proposition, wherever it can be evaluated at run time.
- A build in which an assumption whose proposition can be evaluated is also checked at run time, as `debug_assert!` is.

## Questions it must answer

1. **Whether a method that keeps the standard library's name keeps its shape.** `len` could return a `usize`, as Rust's does, with the fact coming from a view of the vector as a slice, whose length the language defines; or it could return the number with its proof, so that `let (n, p) = v.len();` is how a length is read. RFC 0002's litmus examples took the first, with one method of Locus's own, `Vec::view`; its text showed the second for `push`. Decided for `Vec` (decision 117): a method that states a fact returns its proof, so `len` returns the number and then the proof. The other types are still open.
2. **Which built-in operations the language defines** on the types that stay Rust's, and so are not library at all (RFC 0002, item 18 of what is still for the owner).
3. **Which standard traits each type forwards,** and what a program does for a trait of another crate, which only that crate or Locus's library can implement.
4. **Indexing of sequences,** as in section 1.
5. **Where lemmas come from,** as in section 3.
6. **What an exported crate does with these types** ([distribution-and-export](distribution-and-export.md)).

## Direction

RFC 0002's decision 59 sets where this library is headed: its collections are verified Locus on permissioned pointers, with no `unsafe` code of their own, and Rust's standard library stays available without proofs. The wrappers of section 6 are the first stage: the same types and signatures, with contracts assumed. [permissioned-pointers](permissioned-pointers.md) is the design that turns the assumptions into proofs.

## What it blocks

Any proof about standard library data, and the accepted examples in RFC 0002 that use `Seq`, `Vec`, or `vec!`.

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

### 2026-11-16, conversation with the owner on what Locus's library holds

The owner accepted the design of the previous entry ("This feels like a clean design... Rest everything looks good"), and RFC 0002's decisions 80 to 83 record it: `unsafe assume!(_)`, Locus's own types under `locus::std`, the split between what is swapped and what is kept, and the library trusted to keep what its wrapper types are given. The trusted specifications of standard functions, which this issue had listed, are gone, and the text above the log is rewritten for that.

The owner then asked what else the library holds:

> Also, while you're at it, also describe what logical machinery goes to locus std - logical types like Nat, Int, Seq, ... or general logical theorems etc? Maybe proof helper library? what else?

Sections 1 to 8 above are the reply. They are a proposal for this issue's RFC, and the owner has not decided on them.

### 2026-11-17, conversation with the owner on what runtime code tells Locus

Question 2 above, which built-in operations the language defines on the types that stay Rust's, is answered in [RFC 0004](../design/rfcs/0004-runtime-code.md): the operations that Rust gives syntax, and `len` on arrays and slices, `Box::new`, and `checked_add`, `checked_sub`, and `checked_mul`. No method of `Option` or `Result` is on the list, and the owner chose no way yet for a function to join it ([pure-functions](pure-functions.md)). So question 1 stands as it was: a method of Locus's own `Vec` gives a fact as a proof in its result, or through a view as a slice.

### 2026-11-17, conversation with the owner on changing a value with an invariant

The owner agreed that Locus's library has a `replace` whose result says that it returns what was there, and that RFC 0002 proposes the signatures of `Vec` that its examples need ("yes to all"). RFC 0002's decision 87 records it.

- `locus::std::mem` gains `replace`, `take_out`, and `Slot` ([opening-invariants](opening-invariants.md)).
- Three signatures of `Vec` are proposed, not the six that were put to the owner. `push` and a new `set` return a proof in place of `()`, and `empty` is a constructor of its own name that returns its proof. `new`, `len`, and `pop` keep the standard library's shape, because decided examples call them for their values. This is a proposed answer to question 1 above: a method that returns `()` returns its proof instead, and a method that returns a value keeps its shape.
- Two things this needs from the library's RFC: what each of these methods says about the items themselves, and how a `Nat` is made from an `Int` that is known not to be negative.

### 2026-11-21, indexing a sequence

[RFC 0005](../design/rfcs/0005-core-terms.md), a draft, proposes the answer to question 4. A sequence is indexed by an `Int`. `s.get(i)` gives a `Maybe`, `s.at(i, p)` takes a proof that `i` is inside the sequence, and `s.update(i, v)` is what a write to an element needs. `s[i]` with no proof exists only where the element type has a default that Locus derives from its structure, and outside the sequence it is that default, of which nothing is known. A proof type has no default, so an empty `Seq<@False>` gives no proof of `False`. A `Nat` is made from an `Int` with a proof that it is not negative.

The owner agreed to this in the second round of 2026-11-21 ([proof-construction](proof-construction.md)). The list of types that have a default is the RFC's proposal.

Still for this issue's RFC: whether the contracts of Locus's library are stated on snapshots, and the lemma that ties a library container's model to its snapshot's length.

### 2026-11-21, handles for objects that foreign code holds

RFC 0002's decision 88 lets only plain data cross to foreign code. A C interface over an object with an invariant usually hands out an opaque pointer to it, and under that rule a shim cannot give the pointer the object's type. The pattern that needs no `unsafe` code keeps the objects in a table on the Rust side and gives foreign code an integer key, which a verified function looks up, so that a stale or forged key is a failed lookup. Recorded here as library work, and not decided: whether Locus's library provides such a table, with what contract on a lookup, and how a key is kept from outliving its object.

### 2026-11-22, the methods that are each type's own, and what the library's contracts are stated on

One of RFC 0002's new decisions is about the library, and one question about it was put to the owner and is still open.

**Decision 89, confirmed tentatively.** The owner, shown a table of six rules that a review had left unsettled, among them that a method reached through `Deref` is the standard library's and that each wrapper type lists its own methods:

> tentatively confirm the 6 open questions

So a method that one of Locus's types only forwards is a call of unverified code, and is rejected at an element type that exposes logical data. RFC 0002 lists the methods that are each type's own, starting from the ones its examples call: for `Vec`, `new`, `empty`, `len`, `push`, `pop`, `get`, `set`, `view`, `clear`, `iter`, `iter_mut`, indexing, iteration, and `vec!`; for `Cell`, `new`, `get`, and `set`; for `Rc`, `new` and `clone`; for `Mutex`, `new` and `lock`. The list is this RFC's and is for the library's RFC to complete. Not pinned: whether a forwarded method that only borrows is a loan at such an element type.

**What the library's contracts are stated on: asked, and still open.** Offered contracts on snapshots with `v.len()` read as the snapshot's length, or contracts on models with a `Model` bound on each, the owner said:

> Re: library contracts - I'm not following but will defer to you.

The first was the recommendation. No decision is recorded from that reply, because the recommendation cannot be written as it was put:

- `models/vector-without-element-model.reject.lc` rejects `points.len()` at an element type without a model, and would be accepted.
- `models/derived-model-keeps-proof-holder.accept.lc` gets equal lengths from equal models, and two of the three examples of a stack state their postcondition on a model. Each would need a rule that a library container's model has its snapshot's length, as a step of conversion, since a hole calls no lemma. No RFC has that rule.

So `Vec::view` and the three signatures of decision 87 are unchanged, and still speak of models with no `Model` bound, which is the gap the question was to close. RFC 0002 lists the question as item 27 of what is still for the owner, with three ways out and what each changes in those examples: a step of conversion from a library container's model to its snapshot's length, with the one rejected example changed; contracts on models with a `Model` bound on each method; or contracts on snapshots with a lemma that a program calls. It is to be decided with the design of traits ([traits-on-logical-types](traits-on-logical-types.md)), where what a contract on a snapshot can say is being worked out.

### 2026-11-22, `decide`, and two questions that the decision on `Nat` and `Int` raises

RFC 0002's decision 98 gives the library one function and leaves two questions with it.

- **`decide(P)` is the library's:** a logic function from `Prop` to `Bool`, `true` exactly when `P` holds. The owner chose it over a connective `<==>`: "go with library function/macro vs one more symbol". The name, and that it is a function and not a macro, are RFC 0002's proposal (`logic.bool.decide`).
- **The index of a sequence.** Since that decision a `Nat` is used as an `Int` only where it is written `as Int`, and RFC 0005 indexes a sequence by an `Int`. So `xs[i]` for a `usize` or a `Nat` is now written `xs[i as Int]`, in four examples, in RFC 0002's first example, and in RFC 0004's. The alternative is a sequence that a `Nat` indexes as well, which is how the second round of the skeleton spike had it. Not decided: the owner was asked about operators, and this is item 29 of what is still for the owner in RFC 0002.
- **What `wrap` takes.** `core.wrap` says "an integer". Decided examples give `u32::wrap` a `Nat`, and their error texts an `Int`, and no rule states its parameter or its result. Those examples are unchanged. The same item lists it.

### 2026-11-22, what the decisions on snapshots and models leave to the library

RFC 0002's decisions 99 to 108 leave these with the library's RFC.

- **Which types of the standard library it declares a snapshot and a model for.** A type has neither unless Locus lowers it or the library declares one (decision 107). RFC 0002's examples need `Option`, `Result`, `Vec`, and the types with interior mutability. Until the library declares one for `char`, `str`, `String`, the floating-point types, maps, `Rc`, and `Arc`, a field of such a type is marked `#[opaque]`, as a field of type `TcpListener` is.
- **What models a `Result`.** RFC 0002 says that it is built from the payloads' models, and names no logical type for it.
- **`valid` for each collection.** The snapshot of a library collection is valid when every element's is (`models.valid`).
- **`u8::range` is given a place.** With the snapshot of a `u8` a `Nat`, a lemma from any `Nat` to its being at most 255 would be false, so `proofs.facts.types` now says that the lemma takes a place of type `u8`.
- **Item 27 is unchanged,** and was written before these decisions: it speaks of `models.no-default`, which is removed, and of `models/vector-without-element-model`, which is now accepted. What the library's contracts speak of, and Locus's collections, are for the change that follows.

### 2026-11-22, a sequence is indexed by a `Nat` only

The entry above left the index of a sequence undecided. To the recommendation "letting an index and a measure each be a `Nat` or an `Int`", the owner said:

> Yes. In fact, shouldn't index always be Nat?

and then, answering that question:

> Yes, I think it's better to only let Nat go in index - yes, requires some proof for converting to Nat but isn't that meaningful? Else we may have equivalent of UB in logical stuff?

It is RFC 0002's decision 109. `xs[i]`, `s.get(i)`, `s.at(i, p)`, and `s.update(i, v)` take a `Nat`, `at` needs a proof only of `i < s.len()`, and a measure is a `Nat` or an `Int`. `core.sequences` changes, and the examples no longer write `xs[i as Int]`. RFC 0005 had already made an index outside a sequence defined, so nothing was undefined before; what the decision buys is that a negative index cannot be written.

One thing is left with the library: **how an `Int` is turned into a `Nat`.** RFC 0002 says that it needs a proof that the `Int` is not negative, and gives it no spelling. Until it has one, `xs[i - 1]` and `xs[xs.len() - 1]` cannot be written. It is item 31 of what is still for the owner there.

### 2026-11-22, `iff` replaces `decide`

RFC 0002's decision 110 replaces the third part of decision 98. The library has `iff(p, q)`, a logic function that is the proposition `(p ==> q) && (q ==> p)`, and whose parameters each take a proposition or a `Bool`. It has no `decide`: the `Bool` of a proposition is written `if P { true } else { false }`, and a function of the library for it can come later. The owner's words are in RFC 0002 and in [proposition-syntax](proposition-syntax.md).

`iff` is the first function of the library whose parameters take anything that converts to a `Prop`. How that is declared waits for traits ([traits-on-logical-types](traits-on-logical-types.md)).

### 2026-11-23, Locus's collections keep their contents in a logical field

RFC 0002's decisions 113 to 118 write the eighth and the tenth of the ten recommendations on snapshots, models, and permissions, as a second reviewer amended them. The owner accepted the amended recommendation on 2026-11-22:

> Good - assuming spike succeeds, I think we should begin to now update PR with all the decisions that have been made across all RFCs but not yet persisted in github.

The third round of [the skeleton spike](../spikes/skeleton-typing-3/README.md) then succeeded, and its section C built the `Vec` that the RFC writes. On what the library's contracts speak of, which the entry of 2026-11-22 above left open, nothing new was asked of the owner. The reply quoted there deferred to the recommendation, which the reviewer has since amended and the owner accepted with the rest.

**What is decided.**

- **A collection keeps its contents in a logical field** (decision 113). Locus's `Vec<T>` is a struct around the standard library's vector, with a logical field that holds the elements' snapshots, in order. Its snapshot is the struct's, and no library declares it. `snap(v).elements()` is a `Seq<Snap<T>>`, and the model is a `Seq<T::Logic>` that Locus checks. The standard library's own `Vec` has neither a snapshot nor a model.
- **The trust is relocated, not reduced** (decision 114). What is trusted is that each method does to the standard library's vector what its contract says of the logical field (`trusted.library`). `trusted.library-specs` no longer covers a collection.
- **No mutable access to the standard library's collection inside** (decision 115). A collection implements `Deref` to it at most, and never `DerefMut`. `into_std` and `from_std` are the way out and back. This amends decision 89: a method of the standard library's type that takes `&mut self` is not reached at all.
- **A mutable loan of an element** (decision 116): `get_mut` returns a `&mut T` and a proof that the final contents are the old ones with that slot set to the snapshot of the element's final value. `IndexMut` cannot carry a contract, so `v[i] = x` gives no fact.
- **Contracts speak of the elements' snapshots** (decision 117). RFC 0002 writes the signatures of `new`, `empty`, `len`, `push`, `pop`, `get`, `get_mut`, `set`, `clear`, `view`, `into_std`, and `from_std`. The fact about a model is a lemma, `Vec::<T>::model_len`, called where an invariant is stated on a model.
- **Collections are for element types that have a model** (decision 118). For any other, a program uses the standard library's collection as plain Rust.

**What it settles here.** Question 1, for `Vec`. Section 5's line on `Vec`, and section 6's on what a type around the standard library's is, which are rewritten above. And item 27 of RFC 0002, which the entries of 2026-11-22 recorded as open.

**What it leaves to this issue's RFC.**

- The observers and the contracts of `String`, `VecDeque`, the maps, and the sets, which follow `Vec`.
- What `Rc` and `Arc` are: the same arrangement fits them, with one value as the contents, and section 5 had proposed them transparent as `Box` is.
- Whether a type with interior mutability, which has no logical field, dereferences mutably to the standard library's.
- The rest of `Vec`'s methods. `insert`, `remove`, `truncate`, `extend`, and `sort` are not there until they are the library's own, since nothing forwards a method that takes `&mut self`.
- What `iter` and `iter_mut` return, and what is known after them.
- The operations and lemmas on `Seq`. The signatures use `s.push(x)`, which no RFC defines, and each contract states the length beside the contents because nothing yet takes the length of `s.push(x)` from the equation.
- The lemmas that tie a model to the contents beyond the length, and the same lemma for arrays and slices: `view` gives a slice with the vector's contents, and no rule yet takes the length of a slice's model to the length of its snapshot.
- Contracts for capacity, for what a method leaves untouched, for allocation failure, and for panics. The reviewer's point: equal snapshots keep what snapshots represent, which under a transparent `Box` is not the allocation.

**What the owner did not see** is item 33 of what is still for the owner in RFC 0002. The most pressing: that the standard library's `Vec` can no longer be read in logical code, that `len` and `pop` return their proofs so that a call for the value alone is written `v.len().0`, and that after `v[i] = x` nothing is known of a vector, its length included.

### 2026-11-23, what RFC 0006 asks of Locus's library

[RFC 0006](../design/rfcs/0006-traits.md), a draft, names traits of Locus's library, and leaves their contents to the library's RFC:

- **Conversions between logical types:** `FromLogic<T>` and `IntoLogic<U>`, with the identity and the blanket implementation, and one implementation of `FromLogic<Bool>` for `Prop` (`traits.conversions`). There is none from `Nat` for `Int`.
- **Logical operator traits:** `AddLogic`, `SubLogic`, `MulLogic`, `DivLogic`, `RemLogic`, `NegLogic`, and `OrdLogic`, which `Int` and `Nat` implement with the built-in operators (`traits.operators`). Only the names and which operator each means are fixed.
- **`CloneEq`,** the one law trait that is a rule: its method, that every logical type has it built in, that a program's implementation generates `Clone`, the derive, and that the library's implementations for the language's and the standard library's types are adapters (`traits.laws.provided`, `traits.laws.adapters`, `traits.laws.derive`). Which types have an adapter is the library's. The examples assume `u32` and `Option<T>`, and import the trait as `locus::std::clone::CloneEq`.
- **The other law traits are a sketch,** with placeholder names: `EqLaw`, `OrdLaw`, `PartialOrdLaw`, `HashLaw`, `BorrowLaw`, `IterLaw`, `IndexLaw`, `DerefLaw`, `AddLaw`, `DefaultLaw`, `FromLaw`. `IndexLaw` is what would give `v[i]` on a `Vec` of Locus's library a checked method to rest on, though the operator itself still states nothing (`collections.vec.indexing`).

The trust model gains `trusted.library-laws`: that each adapter agrees with the standard implementation it stands beside.

### 2026-11-24, snapshot/view reconciliation proposed

[RFC 0008](../design/rfcs/0008-snapshot-capture-and-views.md) replaces implicit models with compiler-generated snapshot capture and ordinary explicit views. `Opaque<T>` and `UnsafeCell<T>` snapshot to `Unit`; field-erasure attributes are removed. Collection wrappers keep logical contents with generated conditional storage and offer native-shaped and checked methods. Existing unresolved lowering, loan, coherence, and bridge-fact questions remain open; [snapshot-view-reconciliation](snapshot-view-reconciliation.md) links the validation gates. This update does not rewrite the earlier log or mark an RFC accepted.

### 2026-11-25, owner adopts the restricted hashing architecture; PR #26

[RFC 0007](../design/rfcs/0007-provided-trait-implementations.md#adopted-i04-traces-and-the-standard-hash-bridge)
now fixes the I04 direction: hash traces, a write-only sink, direct designated
standard leaf adapters with shared events, and scoped checked hashing with
designated lawful builders. It names observation, emission/equality,
interpretation and builder facts under the existing trust categories. Runtime
wrapper assumptions remain explicit and listed. Standard map internals must
be connected to the certified digest route, not assumed to call Locus's helper.
The architecture does not supply container snapshots or certify arbitrary
Hash/BuildHasher/Borrow implementations. Borrow compatibility is required now;
its API, container and tuple adapters, slice_trace, and user-proved hashers are
retained in [checked-hashing-followups](checked-hashing-followups.md).
