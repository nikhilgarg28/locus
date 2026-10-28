# Locus's standard library of logical types and models

- **Kind:** decision
- **Status:** open
- **Blocks:** Any proof about standard library data, and the accepted examples in RFC 0002 that use `Seq`, `Vec`, or `vec!`

## Question

What Locus's library provides on top of the language core in RFC 0002 (`design/rfcs/0002-logical-layer.md`, proposed in [#10](https://github.com/nikhilgarg28/locus/pull/10)). This covers:

- **Logical types:** `Nat`, `Bool`, `Seq<T>`, `Set<T>`, `Map<K, V>`, `Maybe<T>`, and characters and strings.
- **Rules:** `logic.boundary.unchecked`, `logic.boundary.access`
  - Each is defined from `Int` and inductive types, the core RFC 0002 builds on. Any native acceleration has a stated correspondence (vision, commitment 5).
  - Which of them the library declares **completable**, the conservative judgment RFC 0002 uses at the boundary with unchecked code. `Nat`, `Bool`, and `Seq<T>` for a completable `T` are the minimum.
  - Their positivity in each type argument: `Seq` and `Maybe` are positive, and a set, as a predicate, is not.
  - Indexing of sequences: the index type, what an out-of-range index means, and the bounds proof needed when elements might not exist.
  - `div_euclid` and `rem_euclid` on `Int`.
- **Models of standard types,** each a trusted entry with its reason.
  - RFC 0002 lists integers, `bool`, references, `Box`, arrays, slices, `Vec`, `Option`, and tuples.
  - Still open: `String`, `str`, `char`, `HashMap` and `BTreeMap`, `Rc`, `Arc`, and `Cow`.
  - A type with no model of its own has its opaque identity, `Opaque<T>`. `Cell`, `RefCell`, `Mutex`, and the atomics have nothing else, and `()` needs nothing else.
- **Specifications of standard functions:** what `Vec::push`, `len`, indexing, `vec!`, and the others do to models and to the logical values a container holds. These are assumed specifications, listed by the audit. They include:
  - that the methods of `Cell`, `RefCell`, and `Mutex` keep the types of their contents;
  - the library's list of **types writable through a shared handle**, which must be complete: `Cell`, `RefCell`, `Mutex`, `RwLock`, `OnceCell`, `OnceLock`, the sending halves of channels, and anything else through which one holder writes what another reads.
- **Lemmas** about the logical types.

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
