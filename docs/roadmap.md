# Roadmap

Generated from atlas.html by `python3 tools/atlas.py export`. The projects and tasks are edited there.

## Open decisions

Status: active.

Points raised against the batches and not yet settled. Each is a decision to make, not work to do.

- [ ] LOC-72 Decide how 32-bit overflow obligations get proved in the core batch (todo)
- [ ] LOC-73 Replace the question about mathy functions with the three-promise rule (todo)
- [ ] LOC-74 Merge the two proof-automation batches, or say how they differ (todo)
- [ ] LOC-75 Write the dependencies between batches down (todo)
- [ ] LOC-76 Place references by tier (todo)

## Core Language

Status: planned.

Logic-only types, grouped under logic. These are Prop, proof types, Int and Seq<T>, with Map and Set later. They have no runtime form, now or ever.

Ghost<T>, built by snapshot(e). It gives up the runtime form of any other type. Inside a proposition it reads as the T.

One rule. A binding or field is erased exactly when its type has no runtime form. There is no binder keyword.

Conversions into the logic. as Int, and a view from Vec to Seq.

Runtime big numbers and similar types are library types with a logic-only model.

Model functions and model fields need no feature of their own.

- [ ] LOC-1 let/mut bindings
- [ ] LOC-2 Types - i32, u32, bool, enums, structs, tuples, propositions, proofs, functions
- [ ] LOC-3 Unbounded Int type along with as
- [ ] LOC-4 No-self recursive structures
- [x] LOC-5 Control flows -- if/else, loop, match
- [x] LOC-6 IR hierarchy -> frontend to elaborated IR (EIR) -> one branch takes to kernel IR and another branch does erasure & generated IR
- [ ] LOC-7 Flow -- `locus check` `locus build` (doing)
- [ ] LOC-8 Some sort of lock file that contains generated proofs in addition generated rust
- [ ] LOC-9 type casting via as (similar truncation model as rust)
- [x] LOC-10 comments
- [ ] LOC-11 Arithmetic: operators + - * / %, the bit operators and the shifts
- [ ] LOC-12 Arithmetic: overflow as a proof obligation that applies only under #[no_panic]
- [ ] LOC-13 Function attributes for effects -- no_io, no_panic, terminates, no_alloc
- [ ] LOC-14 Mathy functions can be used in propositions?
- [ ] LOC-15 Mechanism to generate explicit proofs
- [ ] LOC-16 Tracked evidence for mut proofs
- [ ] LOC-17 Basic proof holes - when the exact fact is in scope

## Visibility

Status: planned.

- [ ] LOC-18 pub qualifiers
- [ ] LOC-19 Generated code hides enough details that it's safe as long as user sticks to safe Rust

## More powerful proof inference for hole filling

Status: planned.

TBD


## Generics

Status: planned.

- [ ] LOC-20 Many more types - i8, u8, including floats
- [ ] LOC-21 Traits
- [ ] LOC-22 Type params, where syntax
- [ ] LOC-23 no dyn yet
- [ ] LOC-24 Closures
- [ ] LOC-25 Derive
- [ ] LOC-26 Logical Seq type, Set type, Map type
- [ ] LOC-27 Type variables in the logic

## More control flows

Status: planned.

- [ ] LOC-28 while loop
- [ ] LOC-29 if/let
- [ ] LOC-30 for loop & iterators

## Memory Layout

Status: planned.

- [ ] LOC-31 Box/Arc etc
- [ ] LOC-32 Types - arrays, slices, vec
- [ ] LOC-33 References -- decide later if we break this in multiple tiers
- [ ] LOC-34 Dyn objects?
- [ ] LOC-35 Strings
- [ ] LOC-36 const, static
- [ ] LOC-37 non-copy types, moves, clone

## Code organization & visibility

Status: planned.

- [ ] LOC-38 impl blocks, self
- [ ] LOC-39 Header separation
- [ ] LOC-40 Tests
- [ ] LOC-41 Multi-module files, use/import
- [ ] LOC-42 Packages?
- [ ] LOC-43 Conditional compilation via features

## Interop

Status: planned.

- [ ] LOC-44 Bridge with external rust code

## Unsure

Status: planned.

- [ ] LOC-45 Async
- [ ] LOC-46 Unsafe
- [ ] LOC-47 Interior mutability
- [ ] LOC-48 Drop
- [ ] LOC-49 Terminating loops
- [ ] LOC-50 Native DST?

## Proof automation

Status: planned.

- [ ] LOC-51 Fill more kinds of holes?
- [ ] LOC-52 Lemma library

## Recursion

Status: planned.

- [ ] LOC-53 Recursive structures, recursive functions, recurse/decreases flag for termination etc
- [ ] LOC-54 Induction in the kernel

## stdlib

Status: planned.

- [ ] LOC-55 collections - map,vec

## Assurance

Status: planned.

- [x] LOC-56 Differential testing, which exists today
- [ ] LOC-57 The erasure theorem and model
- [ ] LOC-58 Exporting proofs to an independent checker
- [ ] LOC-59 locus audit

## Ecosystem tooling

Status: planned.

- [ ] LOC-60 Tooling for people and agents
- [ ] LOC-61 Diagnostics as JSON
- [ ] LOC-62 An LSP server
- [ ] LOC-63 A cargo and build.rs driver
- [ ] LOC-64 The locus! {} on-ramp
- [ ] LOC-65 A short reference and example corpus for models

## Logic growth

Status: planned.

- [ ] LOC-66 exists needs a way to introduce and open it
- [ ] LOC-67 Function values and congruence closure

## Everyday Rust constructs

Status: planned.

- [ ] LOC-68 Option and Result, which need generics
- [ ] LOC-69 ?, return and let … else
- [ ] LOC-70 Richer patterns
- [ ] LOC-71 panic!, assert! and unreachable! as built-in forms, since Locus has no macros
