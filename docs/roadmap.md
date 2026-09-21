# Roadmap

Batches of work, not in sequence. The design behind them is in [notes.md](notes.md); this file says what is to be built.

Done so far: the proof kernel (six gates, [kernel-contract.md](kernel-contract.md)); the typed tree with lowering and checking, erasure, a reference interpreter, and a Rust printer whose output is compiled and compared with the interpreter ([architecture.md](architecture.md)); the parser for the core grammar; and the elaborator with its bounded hole search and diagnostics. The core it covers is bool, u8 with wrapping operations, tuples, structs, non-recursive enums, declared propositions, if and match, a state-passing loop and a bounded for, and math functions in propositions ([language.md](language.md)). `examples/lock.loc` goes from source to a checked program, an interpreted result, and compiled Rust.

Batch 1. Core Language
---------------
* let/mut bindings
* Types - i32, u32, bool, enums, structs, tuples, propositions, proofs, functions.
* Unbounded Int type along with as
* No-self recursive structures
* Control flows -- if/else, loop, match
* IR hierarchy -> frontend to elaborated IR (EIR) -> one branch takes to kernel IR 
  and another branch does erasure & generated IR
* Flow -- `locus check` `locus build` 
* Some sort of lock file that contains generated proofs in addition generated rust
* type casting via as (similar truncation model as rust)
* comments
* Arithmetic -- 
  * operators + - * / %, the bit operators and the shifts.
  * Overflow as a proof obligation that applies only under #[no_panic].

* Function attributes for effects -- no_io, no_panic, terminates, no_alloc. 
* Mathy functions can be used in propositions?
* Mechanism to generate explicit proofs
* Tracked evidence for mut proofs
* Basic proof holes - when the exact fact is in scope

Some details:
Logic-only types, grouped under logic. These are Prop, proof types, Int and Seq<T>, with Map and Set later. They have no runtime form, now or ever.
Ghost<T>, built by snapshot(e). It gives up the runtime form of any other type. Inside a proposition it reads as the T.
One rule. A binding or field is erased exactly when its type has no runtime form. There is no binder keyword.
Conversions into the logic. as Int, and a view from Vec to Seq.
Runtime big numbers and similar types are library types with a logic-only model.
Model functions and model fields need no feature of their own.

Visibility
-----
* pub qualifiers
* Generated code hides enough details that it's safe as long as user sticks to safe Rust

More powerful proof inference for hole filling
------
TBD

Generics
------
* Many more types - i8, u8, including floats
* Traits
* Type params, where syntax
* no dyn yet
* Closures
* Derive
* Logical Seq type, Set type, Map type
* Type variables in the logic. 

More control flows
-----
* while loop
* if/let
* for loop & iterators

Memory Layout
----------------
* Box/Arc etc. 
* Types - arrays, slices, vec
* References -- decide later if we break this in multiple tiers
* Dyn objects?
* Strings
* const, static
* non-copy types, moves, clone

Code organization & visibility
-----------
* impl blocks, self
* Header separation
* Tests
* Multi-module files, use/import
* Packages?
* Conditional compilation via features

Interop
------
* Bridge with external rust code

Unsure
------
* Async
* Unsafe
* Interior mutability
* Drop
* Terminating loops
* Native DST?

Proof automation
---------------
* Fill more kinds of holes?
* Lemma library

Recursion
--------
* Recursive structures, recursive functions, recurse/decreases flag for termination etc.
* Induction in the kernel.

stdlib
------
* collections - map,vec

Assurance.
------
* Differential testing, which exists today.
* The erasure theorem and model.
* Exporting proofs to an independent checker.
* locus audit.

Ecosystem tooling
-------
* Tooling for people and agents.
* Diagnostics as JSON.
* An LSP server.
* A cargo and build.rs driver.
* The locus! {} on-ramp.
* A short reference and example corpus for models.

Logic growth 
-----
* exists needs a way to introduce and open it. 
* Function values and congruence closure 

Everyday Rust constructs.
-----------
* Option and Result, which need generics.
* ?, return and let … else.
* Richer patterns.
* panic!, assert! and unreachable! as built-in forms, since Locus has no macros.

Review notes
------------
Points raised against the batches above and not yet settled.

* Batch 1 has i32 and u32 with overflow as an obligation, and only the basic hole (the exact fact in scope). The strongest tier of today's search tries all 256 values of a byte and does not carry over to 32 bits. Until the batch on more powerful hole filling exists, every overflow obligation is written out by hand, which needs at least the Int lemmas to be callable; the lemma library is listed under a later batch.
* "Mathy functions can be used in propositions?" is settled in the notes: a function may appear in a proposition when it promises terminates, no_panic, and no_io. Recursion is a later batch, so in batch 1 a function that promises terminates has no loop and no recursion, which is enough for the vocabulary of a specification.
* The two sections on proof automation could be one.
* Floats break equality in the logic, since NaN differs from itself. They would be executable only and opaque to proofs, and may belong under Unsure.
* Dependencies: closures need function types that carry promises; iterators need traits, closures, and &mut self; Vec needs generics, moves, and Seq; header separation needs visibility; the standard library's collections need Seq, Map, and Set to be specified.
* References: the notes record tiers. & and &mut as parameters, with impl blocks and self, come early and need no lifetimes; shared references with lifetimes are the next tier.
