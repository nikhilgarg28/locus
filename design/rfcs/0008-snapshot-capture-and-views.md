# RFC 0008: Snapshot capture and explicit library views

- **Status:** proposed
- **Issue:** [snapshot-view-reconciliation](../../issues/snapshot-view-reconciliation.md)
- **Supersedes:** the snapshot/model, barrier, and collection choices of RFCs 0001, 0002, and 0006 identified below; those RFCs remain drafts/proposals

## Summary

Capture always yields a language-generated snapshot. Explicit library views replace implicit models, `Opaque<T>` and `UnsafeCell<T>` have the logical `Unit` snapshot, collections expose native-shaped and proof-returning APIs, and an ordinary verified `mint` supplies contract-hidden logical stamps. Compiler-generated storage carriers preserve bound-free runtime container declarations without exposing deferred types in logical code.

The owner requested this reconciliation and its publication after the design conversation and successive adversarial reviews. This is proposed design, not an implementation claim or a declaration that unresolved lowering gates have passed. The restricted scope of vision commitment 5 remains: unsupported third-party types need a wrapper, and plain Rust declarations must remain legal. No public API silently acquires an assumed contract.

## Motivation

Implicit models made field projection depend on which order capture was performed, made adding a model change equality, and required conversion choices in typing skeletons. Opaque native storage also prevented a whole-snapshot clone law from following from element clone laws. The new design makes capture structural, forgetting explicit in field types, and abstractions ordinary logical functions. Native-shaped `len`, `pop`, and other methods avoid forcing proof tuple projections on Rust-shaped code.

## Design

**Draft proposal — 24 November 2026.** Capture always produces a language-generated snapshot. A snapshot is an immutable logical observation with a defined footprint and no memory-access authority. Views are explicit library abstractions. Users choose field types; they cannot supply snapshot implementations or attributes that change capture.

**Decision summary.** The detailed rules below refine these choices:

- Structural snapshots compose field snapshots and preserve visibility; logical values snapshot to themselves. Equality describes only the included observations, not arbitrary runtime behavior.

- `Snap<UnsafeCell<T>> = Unit` and `Snap<locus::Opaque<T>> = Unit`, for every `T`, without a payload snapshot bound. These are the two special payload-erasing constructors; ordinary unit and structural compositions still follow their ordinary rules. No `#[snapshot_erased]`, `#[opaque]`, or equivalent customization exists.

- `Opaque<T>` owns its runtime payload normally and always captures to `Unit`. An abstract external snapshot, where a compiler-supported implementation exists, is a different, unknown logical value. Arbitrary uninstrumented third-party types have no automatic snapshot; wrapping them changes the runtime field type.

- Capture needs a readable, initialized place at the observation point. References follow supported referents until a declared cycle cut; raw pointers and erased payloads grant no heap access. Framing follows the observed footprint.

- Libraries expose named observers or an explicit `View` trait. A Locus vector uses `Opaque<std::vec::Vec<T>>` for native storage and maintained logical element snapshots. Native-shaped methods coexist with `_checked` methods returning proofs.

- Keep `Vec<T>` usable without a bound on its declaration. Lowering is proposed to generate bound-free carriers for logical fields containing `Snap<T>`, with logical access and the containing snapshot conditional on the required bounds. The hand-written carrier is spiked; the general translation still needs validation. Retain single-element `get_mut` contracts with explicit trusted loan restoration.

- Whole-snapshot `CloneEq` is possible for this vector when its elements satisfy `CloneEq`: native storage contributes only `Unit`. Plain `Clone` alone supplies no such law.

- `mint() -> Nat` is an ordinary verified library function with no result proof. Under `contracts.calls.unrelated`, callers learn neither equality nor inequality between separate results. Store its result when a type wants a stamp hidden behind that contract; capture itself never mints.

Code and command spellings are schematic unless identified as existing rules. The remaining implementation gates are called out below.

### Snapshot meaning

For a capturable place `p: T` in program state `σ`, the language defines an observation `S_T(σ, p): Snap<T>` and an observation footprint `F_T(σ, p)`. The footprint consists of the state on which that observation depends. It is compiler metadata, not a permission token exposed to the program.

A snapshot faithfully represents the state selected by these rules. It need not represent the whole reachable heap, object identity, allocation identity, or every possible future runtime observation. Snapshot equality therefore does not imply that two runtime objects are interchangeable in arbitrary executable code.

A stored logical field is part of the observation, although erased at runtime. Its value is recorded faithfully; no relationship to nearby runtime storage follows merely from its name or type. A library that claims the field describes an allocation must justify that relationship through verified memory operations or an explicit trusted specification.

A captured snapshot never changes. Writes create new observations of new place versions; they do not update old snapshots. A new compiler value name is not evidence that its value differs from an earlier one.

### Snapshot types

| Source type | Snapshot rule | Observation footprint |
| --- | --- | --- |
| Logical type `L`, including a proof type | `Snap<L> = L`; kernel rules still govern proofs | The logical value; no runtime memory read |
| Unsigned machine integer | `Nat`; captures supply the source range bound | The scalar value |
| Signed machine integer | `Int`; captures supply the source range bound | The scalar value |
| `bool`, unit, and `char` | Logical Boolean, `Unit`, and a character with Unicode scalar validity | The value; empty for unit |
| Struct, tuple struct, tuple | Product of field snapshots | Union of included field footprints |
| Enum | Sum preserving the variant and active field snapshots | Discriminant and active field footprints |
| Array `[T; N]` | `Seq<Snap<T>>`, with length `N` for captures | Element footprints; fixed length |
| Slice `[T]` | `Seq<Snap<T>>` | Length and element footprints |
| `str` | Logical bytes, with UTF-8 validity for captures | Length and bytes |
| Built-in reference or `Box<T>` | Readable referent observation, except at a reference cut | Referent footprint and state selecting that referent; no address identity in the result |
| Raw pointer or cut reference | Pointer observation without dereference or access authority | Pointer value, not pointee storage |
| `UnsafeCell<T>` | `Unit`, without reading the payload | Empty |
| `locus::Opaque<T>` | `Unit`, without reading the payload | Empty |
| Union | Abstract observation without automatic active-field projections; unsupported captures may be rejected initially | Compiler-specified observation, with no assumption that all fields are initialized |
| Standard-library or other externally implemented type with designated Locus support | Abstract snapshot only where a legal, compiler-supported implementation exists | Justified structural footprint or conservative effects; never assumed empty |
| Uninstrumented third-party type without such support | No direct snapshot under the current lowering; use `Opaque<T>` or an appropriate Locus wrapper | No capture to frame; `Opaque<T>` itself has an empty footprint |

Floating-point support should use a representation that preserves IEEE distinctions, such as bits, before adding arithmetic observations. IEEE `==` is not logical equality. Until that representation and its operations are implemented, verified float operations remain unsupported or limited to an opaque observation.

Generated snapshot types have one meaning across modules, generic instantiations, and both lowered programs. Privacy changes access to fields, never their types or the meaning of equality. Type aliases do not install new policies. Neither a type author nor a downstream library can override snapshot generation, including through field attributes. The two payload-erasing constructors are recognized by compiler-known type identity, not by their names or an attribute users can imitate.

### Type-based observation barriers

The Locus library supplies `Opaque<T>` as an owning runtime wrapper. Its payload is ordinary `T`; the language fixes `Snap<Opaque<T>> = Unit`. This is intentional forgetting, not an unknown per-instance token. The spelling `Opaque<T>` does not mean the same thing as an abstract external snapshot, which remains an unknown, potentially distinguishing value.

A minimal runtime API can be ordinary Rust-shaped code:

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
impl<T> Opaque<T> {
    fn new(value: T) -> Self;
    fn get_ref(&self) -> &T;
    fn get_mut(&mut self) -> &mut T;
    fn into_inner(self) -> T;
}
```

These methods transfer or borrow actual values; none runs implicitly during capture. The wrapper preserves ownership, destruction, borrowing, payload lifetimes, and the usual restrictions on logical data. It grants no special access permission and no interior mutability. Its auto-trait behavior follows its runtime representation. `T` need not be capturable: owned closures, foreign values, and sized handles such as `Box<dyn Trait>` can be wrapped. A bare unsized trait object needs a `?Sized`-capable wrapper/storage design or a sized handle; no general unsized support is implied by `impl<T>`.

At runtime a caller may obtain an authorized payload reference and then capture the payload, if its type supports capture. That observation is a fresh observation of the payload place, not a projection from `Unit`. There is no logical `unwrap` from a wrapper snapshot, and no equation equating all payloads because their wrappers have equal snapshots. A library must not derive results of arbitrary executable methods from wrapper equality.

Only `Opaque<T>` and `UnsafeCell<T>` have the special rule that discards an arbitrary payload and returns `Unit`. This does not prohibit the ordinary unit type or structs whose recursively generated fields happen to contain no information. No field annotation, derive, user trait implementation, type alias, or type-name trick can install the special rule. Choosing `raw: Opaque<Native>` changes the field's runtime type and gives that field a unit snapshot; it preserves the ordinary projection law for the containing struct.

A unit snapshot holds neither payload identity nor a phantom source type or lifetime. The runtime wrapper still carries all of them where Rust requires them. Wrapping proof-bearing or sealed data does not exempt that data from Locus's recursive boundary checks.

### `mint`: contract-hidden logical stamps

`mint` needs no compiler intrinsic. An ordinary verified library function suffices:

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
pub fn mint() -> Nat {
    logic! { 0 }
}
```

Its signature supplies no result proof. The implementation returns zero, but callers using only that signature receive no equation identifying the result with zero, no positivity or uniqueness proof, and no relationship to another call. The logical result erases under the existing rules; the body needs no random generator, counter, or registry.

This is ordinary contract abstraction, not semantic nondeterminism. [`contracts.calls.unrelated`](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spec/contracts.md#calls) says that what verified code knows about a call's result is what its contract proves, and that equal arguments do not relate separate call results without such a proof. The caller therefore checks its continuation against arbitrary naturals consistent with the signature, even though this implementation always returns zero. A new symbolic result at a call site is not evidence of a new numeric value. A helper that returns no additional proof retains that lack of information.

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
let a = mint();
let b = mint();
let c = a;

// Provable: a === c, a + 0 === a, a >= 0.
// Not derivable from these calls alone: a === b, a !== b, or a > 0.
```

The same invocation supplies the same immutable value wherever its result is copied. Equality may also follow from ordinary arithmetic, explicit assumptions, case analysis, or other proved relationships; the kernel does not require common invocation provenance as the only equality rule. No mint-specific inequality axiom is introduced.

The existing [`logic.code.no-runtime-calls`](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spec/logical-layer.md#logical-code) excludes `mint`, an ordinary `fn`, from pure logical functions, views, propositions and other logical expressions. They can take a previously returned `Nat` as an argument. No special minting effect or purity rule is added. Calls in executable code, including loops, use ordinary contract-based verification; storing or copying one returned value preserves that value.

The dependency is the existing boundary between executable call verification and logical functions: a runtime function's body is not unfolded for callers, nor is a common logical function application used to relate equal-argument calls contrary to `contracts.calls.unrelated`. This does not prohibit pure `logic fn` definitions or their normal equality rules. If a future feature deliberately exposes executable bodies or stronger contracts, it can reveal equality for this implementation without invalidating any current inequality proof, because no such inequality was supplied. No new kernel rule, freshness effect, or mint-specific erasure check is required.

The intended use is an optional stored stamp:

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
struct Stamped<T> {
    stamp: Nat,              // private logical field
    payload: Opaque<T>,
}

impl<T> Stamped<T> {
    fn new(value: T) -> Self {
        Self { stamp: mint(), payload: Opaque::new(value) }
    }
}

// Generated snapshot: { stamp: Nat, payload: Unit }.
```

Callers using only `mint`'s signature have unrelated stamps from separate constructions, so their snapshots are not automatically provably equal; neither are they provably unequal. Moves preserve the stored stamp, and repeated captures observe it without a new call. A clone can explicitly preserve it to support whole-snapshot `CloneEq`; another call to `mint` supplies no such relationship. Assignments to the stamp, if permitted by the type's API, follow ordinary mutation rules. Mutating only the hidden payload leaves the stamp unchanged.

`Stamped<T>` is an ordinary library abstraction providing a stored per-instance opaque observation without a third snapshot exception. “Per-instance” means a field is stored in each instance, not that its value is unique: this `mint` implementation can give every instance zero. `Opaque<T>` and `UnsafeCell<T>` still capture to `Unit`. Stamps grant no authority, reveal no payload, and prove neither object identity nor disjointness. Hypothetical snapshots remain constructible under normal visibility rules; the number is not an unforgeable credential. Allocation protocols needing distinct identifiers require a separate freshness guarantee.

A stable stamp is not an abstract snapshot of changing hidden state. For `Stamped<TcpStream>`, changing only the stream while framing the stamp unchanged preserves the wrapper snapshot. That observation intentionally says nothing about the write or the stream's current state. By contrast, a supported abstract external snapshot that observes such state may no longer be known equal to a new capture after an affecting call. Historical snapshots themselves remain immutable in both cases.

The existing ghost-erasure and boundary rules prevent the result or control flow depending on it from affecting ordinary runtime behavior. An ordinary function call is not automatically erased merely because its result is logical; this particular library body has no executable work beyond its erased-value return. A stored logical stamp still brings the usual logical-data restrictions. Compiler diagnostics can describe the symbolic result without exposing it through runtime `Debug`.

### Capture availability

A runtime place may be captured when it is initialized and readable through a legal temporary shared borrow, including a shared reborrow of an existing mutable reference. The capture must not conflict with an outstanding loan, read moved-out storage, read uninitialized memory, or bypass an access guard. Ordinary Rust obligations concerning packed fields, unsized places, and valid references still apply. An implementation may reject an unsupported capture even though ordinary Rust use of the value remains legal.

The shared read is needed at the observation point only. The resulting logical value retains no executable borrow, source lifetime, allocation, or permission. Historical observations remain meaningful after the original value is moved, mutated, or dropped.

A logical variable already contains a logical value and is used directly. Logic-function parameters have logical types, including `Snap<T>`; passing a runtime place into such a function performs capture at the call site. No logical expression invokes arbitrary executable code to produce a capture. Capture syntax accepts an available place whose access uses supported primitive projections; it does not execute a user-defined Deref, Index, property getter, or function call. Bind such an operation's result in runtime code first.

A projection is captured as a place before constructing any whole-object observation:

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
let r = &mut pair.left;
// In logical code, pair.right remains capturable.
// Capturing the entire pair is unavailable while left is lent out.
```

For an ordinary, uncut field, `snap(p.f) = snap(p).f` whenever both sides are defined. This is a conditional projection law, not a requirement to lower every field read into a borrow of the whole parent. Surviving fields of a partially moved value remain capturable; the incomplete whole does not.

### Stability and mutation

The frame rule is: if a place remains available, its observation interpretation is unchanged, and no operation modifies state in its observation footprint, its next capture equals its earlier capture. For core structural types, borrow checking and barrier elimination establish many such facts automatically.

The useful principle is **exclusive authority to modify captured state**, not the textual presence of `&mut p`. Assigning an owned local, mutating a field, and writing through an existing mutable reference can change a snapshot without forming a new `&mut` expression for its root. For example, `*r = 8` changes a transparent referent observation even though the reference's address is unchanged.

A live shared borrow protects the ordinary structural state reachable through the snapshot's supported reference and ownership rules. Interior-mutable payloads are excluded. External resource protocols require their own stability justification; Rust memory safety alone does not specify what an arbitrary library's hidden heap state means.

The verifier tracks direct writes, writes through reborrows, calls, overloaded operations, callbacks, and destructors. Effects are propagated through the actual lifetime and loan graph. Unknown calls invalidate observations they may affect. Passing a shared reference preserves only observations whose footprint is protected; it does not promise constancy of hidden mutable contents or future method results.

At the end of a mutable borrow, the affected place gets a new value name. Only the callee's returned proofs and understood language operations establish relationships to the earlier value. Unaffected siblings retain their observations. Ending one loan must not reset another region to an old value.

A runtime call can produce different results at equal snapshots because it may read omitted state, global state, or external resources. Snapshot equality never turns an executable function into a pure logical function.

### Interior mutability

`Snap<UnsafeCell<T>> = Unit` for every `T`. Capture does not read the payload or require `T: Snapshot`. All these snapshots are the same logical unit value, including across different payload types. This says nothing about cell addresses, contents, allocation identity, or which operations may access them. There is no `ErasedCell<T>` marker and no payload lifetime in the resulting snapshot.

Do not mint a fresh ID at each capture. That would make capture depend on the act of observing, defeating repeated-read stability. Do not infer cell identity from an empty snapshot either. A permissioned cell that needs identity explicitly stores or is associated with a stable identifier established at construction; freshness is part of its constructor's verified or trusted protocol.

Even under `&mut UnsafeCell<T>`, the cell's own snapshot type and meaning stay unchanged. To observe the payload, first use an authorized executable operation to obtain a reference or owned value, then snapshot that result. For example, safe `get_mut` yields a mutable reference whose referent can be captured; this does not retroactively place contents inside the cell snapshot.

`Cell`, `RefCell`, mutexes, locks, and atomics do not expose changing payloads through a pure function of their ordinary snapshots. A `Cell::get` result or an atomic load result can be captured after execution, but has no automatic relationship to an earlier result. RefCell and lock guards permit observation of a real borrowed referent while their access protocol holds. Capturing a lock or cell never acquires a guard.

Snapshots of types containing interior mutability still preserve ordinary surrounding fields. Mutation inside the `UnsafeCell` payload does not change that structural observation. Nested barriers apply recursively, including inside a separately borrowed payload. `Opaque<T>` also removes its payload from observation, but its runtime payload remains subject to the usual borrowing rules; it is not an interior-mutability primitive.

[Rust's UnsafeCell rules](https://doc.rust-lang.org/std/cell/struct.UnsafeCell.html) permit payload mutation through shared cell references while retaining the uniqueness requirements on mutable references. Locus's content-free observation deliberately records no fact about that mutable payload.

For a struct containing a normal integer field and a cell field, changing only the cell payload leaves the snapshot unchanged. This supports useful framing of the ordinary fields. It does not prove that a subsequent executable cell read returns the previous value.

### References and cycles

The initial policy keeps ordinary references and `Box` transparent when the resulting logical structure is finite, and cuts recursive reference fields. Reference snapshots do not preserve address identity; a separate pointer observation is needed when identity matters. Snapshot equality of two references never establishes aliasing or interchangeable access permissions.

For a declared aggregate `D`, examine each field's type expansion, stopping at `Opaque`, `UnsafeCell`, and abstract external representations. If the field reaches `D` again along a path containing a reference, its component in `Snap<D>` is `CutSnap<F>` rather than `Snap<F>`. A cut snapshot follows no references within that component: reference edges become address observations. Owned-only recursion through `Box` remains a nominal recursive logical datatype, subject to positivity and the requirement that valid owned values form finite data.

Cut positions belong to the declaration and must agree in generic and concrete code. They must not depend on how deeply a particular capture happens to traverse the heap. If an associated type or unresolved generic expansion prevents a uniform cut layout, the initial implementation rejects that capture with a diagnostic; it does not silently use a different snapshot in generic code. Ordinary runtime use remains allowed.

A frozen cut address cannot be dereferenced by a view function. Observing the live referent requires a new authorized runtime-place capture. Thus projection from an old cut snapshot and fresh capture of the referenced place intentionally differ.

A reference cut or observation barrier must not discard data needed to interpret a retained proof field. The compiler rejects a capture whose retained proof cannot be well-typed and justified in the resulting snapshot representation. A proof inside a fully erased payload is not available by projecting its unit snapshot. Arbitrary cyclic graphs with full heap identity are outside the initial policy; explicit logical graph data maintained by a verified abstraction can describe them.

### Constructing logical values and validity

Snapshots are logical values, not unforgeable certificates of observation. Users may compute hypothetical values of a snapshot type. In particular, `Snap<u32> = Nat` means that an ordinary natural number expression already has that type. Forbidding construction while retaining this equality is not a coherent restriction.

Generated product and sum constructors follow the owner's field and constructor visibility. Private and opaque representations cannot be synthesized through hidden constructors. Logical struct construction must supply every proof field with a valid proof of its proposition. Constructing a snapshot never constructs a runtime object, produces a reference, authorizes a load, or establishes that a particular runtime place currently has that value.

The language defines `T::valid(s)` as necessary structural validity conditions. An actual capture establishes the applicable validity facts. Arbitrary quantified, computed, or parameter values do not receive them just because their type is written `Snap<T>`. A hypothetical `Nat` larger than 255 is allowed as a value of `Snap<u8>` and does not satisfy the byte range condition.

Validity is not a theorem that a corresponding live runtime object or allocation exists. Opaque or erased components deliberately retain too little information for such a theorem. Views are total over their declared logical domain, or take explicit proof parameters for the conditions their bodies require.

Only the language's capture rule associates a logical observation with a current place version. A separate nominal, authority-bearing certificate would be needed if a later API requires provenance of an actual observation. Ordinary snapshots are not that certificate.

### Privacy and explicit views

Field visibility is preserved in generated snapshots. Owners can define public logical observers over private representation, with definitions unfolded only under Locus's normal visibility and conversion rules. Clients can hold and compare snapshots without being able to inspect private fields.

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
// Schematic: ordinary logical observer, defined in the owner's module.
pub logic fn elements<T: Snapshot>(s: Snap<MyVec<T>>)
    -> Seq<Snap<T>>
{
    s.contents
}
```

A library may provide an explicitly invoked convention:

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
trait View: Snapshot {
    type V: Logical;
    logic fn view(s: Snap<Self>) -> Self::V;
}
```

`T::view(s)` chooses the source type explicitly. There is no automatic application on capture, equality, arguments, field projections, or method lookup. Several named observations can coexist. A view may intentionally forget capacity, representation, or a variant tag; contracts that need those distinctions use the snapshot or another observation.

Logical methods may be attached to a named generated nominal snapshot type once its ownership and coherence rules are implemented. This is not blanket permission to implement traits for arbitrary `Snap<T>` projections: `Snap<u8>` and `Snap<u32>` are both `Nat`. Source-specific behavior belongs on `T` with a `Snap<T>` argument when the snapshot type alone loses the source distinction.

An enum snapshot always retains its tag. A view may explicitly match and combine variants into one abstract result. Tuples and arrays retain their component snapshots without recursive view application. A logical field remains unchanged by capture, so a collection storing `Seq<Snap<T>>` can return it directly or explicitly map `T::view` to produce element views.

### Permissioned memory

A raw pointer observation records no right to access memory. The initial memory interface separates a pointer from a non-copyable runtime permission resource. For an initialized single object, a `PointsTo<T>` resource can carry logical allocation identity, region information, and a value of `Snap<T>` describing the permitted contents. Larger regions additionally describe initialization and disjointness.

The permission's snapshot is generated from those fields. It may be copied freely as a logical description; the live runtime permission cannot. No function reconstructs a live permission from its snapshot. If a permissioned pointer packages the pointer and permission together, its snapshot contains both descriptions but still grants no authority.

The primitive interface has these obligations:

| Operation | Required inputs | Result and relationship |
| --- | --- | --- |
| Allocate or initialize | Size/layout or owned input value | Live allocation and appropriate unique permission; initialized contents match the input where applicable |
| Borrow initialized object | Matching pointer and shared borrow of permission | A runtime `&T` tied to that borrow, with a proof relating its snapshot to the permitted value |
| Write or replace | Matching pointer, exclusive permission access, and owned new value | Updated memory and permission contents; replacement may return the old owned value |
| Split or join | Permission plus proof of region boundaries/disjointness | Equivalent authority partitioned or combined without duplication |
| Deallocate | Owning permission and required allocation resource | Resource consumed; no later live access through it |

Alignment, provenance, initialization, bounds, zero-sized types, allocation reuse, and non-overlap belong to these primitive obligations. Historical snapshots may survive deallocation, but they cannot be used to perform a fresh read.

The connection between raw operations and permission fields is initially a small trusted specification or, later, a proved memory semantics. It is not generated merely because a struct has a logical `value` field. Shared access to a permission freezes the described contents according to the verified resource protocol; mutation requires the appropriate exclusive authority. Concurrent atomic and lock protocols extend this interface separately.

### Standard vector wrapper

The initial useful vector is a Locus-owned wrapper around native `std::vec::Vec`. Its representation is schematic:

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
// Proposed source syntax: no bound on the runtime container declaration.
struct Vec<T> {
    raw: locus::Opaque<std::vec::Vec<T>>,
    contents: Seq<Snap<T>>, // private logical field; carrier generated by lowering
}
```

When `T: Snapshot`, the generated `Snap<Vec<T>>` contains `raw: Unit` and `contents: Seq<Snap<T>>`. It is a struct snapshot, not itself a sequence. The public observer `elements(s)` returns the stored sequence. No implicit view is applied to its elements; an explicit view may map `T::view` over them. The ordinary projection law holds: `snap(v.raw)` and `snap(v).raw` are both `Unit`, because the runtime field itself has type `Opaque<_>`.

The representation obligation is that `contents` describes, in order, the snapshots of the initialized native elements whenever the wrapper is available for observation. State behind `UnsafeCell` or `Opaque` barriers is excluded from each element observation. Erasing native storage supplies no evidence for this relationship. Constructors, mutators, conversions, and loans must establish it through verified operations or an explicit trusted library specification. A future permission-backed implementation proves the same interface from memory operations.

A private logical field alone does not make an arbitrary wrapper correct. Its constructors, every mutator, every returned loan, and exceptional exits must preserve the representation obligation. A source-level proof field relating state would be subject to the existing restrictions on invariant-preserving mutation; this draft introduces no general invariant-opening feature.

Adopt compiler-generated storage carriers with explicit read and write rules. A logical field declared `contents: Seq<Snap<T>>` has an internal nominal representation that can name deferred `SnapOf<T>` without `T: Snapshot`. The generated containing snapshot is conditional on the required bounds. Reading the field in logical code yields exactly `Seq<Snap<T>>`; writing a value of that type stores exactly that logical value through a compiler-inserted representation conversion. There is no source-visible `Contents<T>` API or user-defined snapshot implementation. The [third skeleton-typing spike](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spikes/skeleton-typing-3/README.md) supports the hand-written carrier case; the general generated translation remains to be validated.

The read normalization is a rule for compiler-generated storage, not an additional snapshot rule for ordinary logical sequences. A source logical type already satisfies `Snap<L> = L`; it cannot simultaneously receive a conflicting recursive snapshot interpretation. Use separate internal representation nodes (schematically `Rep[Seq<SnapOf<T>>]`) whose logical reading under `T: Snapshot` is `Seq<Snap<T>>`. `SnapOf<T>` and the carrier must not escape as user logical types. Thus reads at both `u8` and `u32` produce `Seq<Nat>`. Aliases and associated types must normalize to the same result, and the Rust marker declaration must genuinely defer the required bounds.

Writes must preserve values as well as types: reading the field immediately after storing `s` yields `s`. Any conversion stub used to satisfy rustc is compiler scaffolding for that logical assignment; an ordinary contract-free function that returns an unrelated carrier would not establish the required relationship. Validation must exercise constructors, whole-field assignments, writes through a borrowed field, destructuring, and generic helper calls, with the same place identity and no per-use temporary replacing a borrow. Include borrowed `T` to check that the frozen snapshot retains no source lifetime. These read/write rules settle the intended meaning; implementing and testing those paths remains necessary. They do not establish the vector's native-storage correspondence or loan restoration.

### Vector API and ordinary Rust behavior

Keep native-shaped operations separate from methods returning proofs. `E(s)` below abbreviates the public `elements(s)` observer. `old` is the entry snapshot; `new` is the owner's snapshot after the mutable borrow ends. Every stated proof is a normal-return guarantee.

| Method | Runtime-shaped result | Checked counterpart and proof |
| --- | --- | --- |
| `new()` | `Vec<T>` | `empty()` returns the vector and a proof that its elements are empty |
| `len(&self)` | `usize` | `len_checked` returns the length and a proof equating it to `E(old).len()` |
| `push(&mut self, x)` | `()` | `push_checked` returns a proof that `E(new) = E(old).push(snap(x))` |
| `pop(&mut self)` | `Option<T>` | `pop_checked` returns the option and proofs relating it and the new contents to the old last element, or emptiness |
| `get(&self, i)` | `Option<&T>` | `get_checked`, with an in-bounds proof, returns `&T` and its indexed-element relation |
| `get_mut(&mut self, i)` | `Option<&mut T>` | `get_mut_checked`, with an in-bounds proof, returns one loan, its initial element relation, and a final owner-update proof |
| `set(&mut self, i, x)` | `()` on normal return | `set_checked` returns the in-bounds and element-update facts |
| `as_slice(&self)` | `&[T]` | `as_slice_checked` returns the slice and a proof that its snapshot equals `E(old)` |

Ordinary methods still preserve the private representation obligation, including restoration after returned loans. Their signatures do not give the caller postconditions. They may share an implementation with checked methods and discard the proof. This avoids requiring `.0` in Rust-shaped code while retaining explicit proof-result contracts. Bounds belong on operations that construct or inspect logical contents, not on the container declaration. Initially, naming, moving, `new`, `from_std`, `into_std`, and ordinary `len` require no `T: Snapshot` bound. Logical observation, checked methods, and mutators maintaining logical contents require it, including their native-shaped mutable counterparts. General bound-free mutators remain a separate conditional-storage gate; no unverified fallback may silently leave stale contents.

No unrestricted `DerefMut`, mutable reference to the native vector, or mutable native iterator is exposed. Shared forwarding remains subject to the existing logical-data boundary and the library's stability obligations. In particular, no fallback based on failed inference of `T: Snapshot` is used to select a different method.

`into_std(self)` consumes the wrapper and transfers its native vector through `Opaque::into_inner`. `from_std(raw)` establishes contents corresponding to the actual incoming elements whenever logical observation is available. This initially requires a trusted observation/initialization specification; an arbitrary or empty sequence is not an implementation. `Opaque` itself provides no such specification. Without an external native-vector observation API, callers receive no contents-preservation equation across the conversion. An unbounded runtime conversion may be supported by the conditional-storage encoding, but its later capturable state must still satisfy the same relation. All conversions obey Locus's logical-data boundary rules.

Logical fields make the wrapper subject to the existing sealing and unsafe-boundary rules. Removing implicit models does not make it acceptable to arbitrary unverified code that names or can modify its representation.

### Returned mutable loans

Keep the existing final-value contract design for a single returned mutable element. A checked interface has the intended shape:

```rust prose illustrative Locus source; precise outcomes are in the linked litmus cases
// Schematic result; entry and final terms obey the existing final! rules.
fn get_mut_checked<'a>(&'a mut self, i: usize, inside: @InBounds)
    -> (r: &'a mut T,
        @InitialElementRelation,
        @FinalOwnerEqualsElementUpdate);
```

The initial relation identifies the borrowed element's entry snapshot. The final relation states that the owner's elements are the previous sequence updated at `i` with the element's final snapshot, preserving length and other elements.

The owner remains unavailable for capture while the loan suspends its representation. A reborrow, return value, closure, or another argument that retains the loan extends this suspension according to rustc's loan facts. Returning from the function does not by itself end it.

For one `get_mut` loan, a sound implementation records the entry sequence and index, suspends owner observation, and restores the stored sequence with the element's final snapshot when the loan actually ends. A second direct loan from that same owner is not available through another `&mut self` call while the first remains live. APIs deliberately splitting access into several simultaneous loans need an additional disjointness and restoration protocol; they must not repeatedly restore the entire owner from stale entry sequences.

No ordinary logical field may be populated early with an unresolved future value. `final!` terms remain confined to proofs until resolution. No restoration relies only on an object's destructor: a guard may be forgotten. Loan-end restoration is a verifier/resource protocol, not executable cleanup inserted by capture.

Retain the single-loan `get_mut` API. Initially its implementation may sit at the trusted library boundary, with explicit suspension, reborrow, and loan-end restoration semantics. Existing final-value contracts support what callers may conclude; their presence, or a compiling stub, does not prove that an ordinary user-written body updates the stored logical field correctly. Keep `split_at_mut`, mutable iterators, and comparable multiple-loan interfaces behind a separate protocol gate. Under the current proof-result design, Rust's `IndexMut` signature cannot convey the final-value proof; if supported as a runtime-shaped operation, it must nevertheless maintain the private representation obligation.

### Exceptional exits and invariant preservation

Representation obligations must hold whenever a live wrapper can again be observed, including after a caught panic. Normal-return proofs alone do not establish this. Destructors and callbacks must not observe a certified object in a broken state.

Every initially trusted mutating primitive needs an audited exceptional-exit specification. It must restore correspondence to whatever valid native state remains, or establish that the object cannot be observed again. If a primitive cannot justify cleanup, the supported implementation must prove it cannot unwind or use an explicitly documented abort-on-unwind boundary. Aborting is an observable API choice and is not claimed to preserve native Rust panic behavior.

User-written verified implementations retain the existing whole-value reconstruction and controlled-slot rules when proof fields depend on mutated fields. The slot mechanism's abort behavior remains explicit. This proposal does not authorize temporary violation of source-level proof fields merely because they are private.

Testing must include element destructors that panic, callbacks that inspect other accessible state, multiple loans, and forgotten guards. Erasing the ghost state does not erase these semantic obligations.

### Native types and other wrappers

| Type | Core or wrapper policy |
| --- | --- |
| Native `Option<T>` and `Result<T,E>` | Supported algebraic shape with variant and payload snapshots; native calls still need their own proof interface |
| Native arrays and slices | Structural sequence observation under a real borrow; supported reads and writes keep their existing rules |
| `Box<T>` | Built-in ownership/reference rule; no allocator metadata is implied |
| `Opaque<T>` | Owns its runtime payload normally; language fixes its snapshot to `Unit` without a payload bound |
| Locus `String` | `Opaque<std::String>` plus private logical bytes and maintained UTF-8 correspondence; define character views by explicit decoding |
| Native `std::String` and `std::Vec<T>` | Eligible for designated library snapshot support, not automatically supported. Contents require a justified observation or conversion into a wrapper |
| Locus `Rc<T>` or `Arc<T>` | May maintain the observable immutable payload; reference counts and payload interior mutability are excluded; unique-access and copy-on-write operations preserve the relation |
| Native smart-pointer handles | Direct capture only with designated support; otherwise wrap. Capture does not run arbitrary `Deref` |
| Locus `Cell` or permissioned cell | `UnsafeCell` components contribute `Unit`; any explicit stable identifier or logical protocol fields are captured normally. Runtime reads/writes need actual access or permissions |
| Locus `RefCell`, mutex, and lock wrappers | Obtain a real guard before observing its referent; mutable-guard restoration follows the loan requirements |
| Atomics | Actual operations produce results; concurrent specifications use a separate protocol; no implicit payload load |
| `MaybeUninit<T>` and unions | No automatic initialized payload or field validity; abstract observation or unsupported capture until specified |
| Files and sockets | Standard-library handles may have designated abstract snapshots; unsupported third-party handles need wrapping. Resource contents and external effects require a separate specification |

An Arc containing a mutex does not acquire a frozen snapshot of the mutex's changing payload merely because the Arc is shared. Runtime counts and synchronization metadata likewise do not become stable observations by being private.

String views must distinguish byte length from character count. Snapshot integer elements retain their validity obligations; a logical sequence of naturals is not automatically a valid UTF-8 string.

Ordinary Rust use of unsupported types is a design requirement, including inside Locus-declared structs. The current lowering route for keeping those declarations accepted without giving them a snapshot still has an open rustc dependency described below. `Opaque<T>` enables capture by changing the runtime type; it does not transparently add verification to unchanged third-party APIs or public fields.

### External types: availability and abstract observations

Under the chosen Rust-trait lowering, an abstract external snapshot exists only where a legal compiler-supported snapshot implementation is available. Locus's library owns the `Snapshot` trait and can provide designated implementations for standard-library types. A downstream application owns neither that trait nor `tokio::net::TcpStream`, so it cannot implement `Snapshot` directly for the stream (Rust's orphan rule). A cooperating type-owning crate, or support in the trait-owning library with an appropriate dependency, can change availability; a foreign type does not acquire support automatically. A blanket fallback conflicts with specific implementations under the current stable-Rust approach.

For example, `struct Server { sock: tokio::net::TcpStream, port: u16 }` is intended to remain usable as ordinary Rust but has no whole-struct snapshot without supported capture of the stream. A direct capture of its readable `port` can still work. Changing the field to `sock: Opaque<TcpStream>` makes that component capturable as `Unit`; methods then access the actual stream through operations such as `self.sock.get_mut()`. Public field types, construction, pattern matching, and API compatibility can change. Wrapping the whole `Server` is another route if discarding its entire observation is acceptable, but it does not recover a structural port observation from the wrapper snapshot.

Do not introduce the proposed fixed-meaning field attribute: the user's selected design keeps barriers type-based. Such an attribute could be language-defined, but it would still create field-specific capture behavior and make projection differ from capturing the original field type. The explicit wrapper cost is accepted here; no transparent shorthand is assumed.

Where an abstract external snapshot is supported, it is an unknown logical value with no public representation projections, not `Unit` and not an axiom equating all instances. Moves and copies preserve it at transfer when no included state changes; cloning requires its own contract. When a stable footprint cannot be justified, the verifier invalidates current-state relationships across potentially affecting calls or rejects the capture. A shared receiver alone is insufficient to frame hidden state. Unsupported foreign types do not silently receive either this abstract observation or `Unit`.

Later specifications can expose observers of an abstract snapshot only if those observers respect every equality, move/copy guarantee, and framing law already provided. Unknown does not mean arbitrary new state can be retrofitted without constraint. One compiler-designated interpretation must apply across the dependency graph. Ordinary library code may define views of information already present, but cannot register a new snapshot policy. In particular, no meaningful payload observer can be added to the `Unit` snapshot of `Opaque<T>` or `UnsafeCell<T>`.

For example, if a vector observation contains only pointer/length/capacity and an element write leaves those unchanged, a pure logical observer cannot nevertheless return a changed sequence. Supporting native contents requires additional observed state or a maintained wrapper field. The language must not assert both descriptor-only structural equality and a contents observer that distinguishes those equal observations.

Hidden mutable state behind raw pointers may change without changing the handle's structural snapshot. Such state cannot be exposed by a pure function of that snapshot unless the external observation protocol includes it and supplies the required stability rules.

### Equality and proof boundaries

`===` is equality of logical values and is never overloaded. Products compare components; sums retain tags; abstract external components expose no representation-specific equality rules. A `Unit` component contributes no distinction, irrespective of the payload's source type. Kernel proof rules govern proof fields, not equality of erased Rust marker objects.

Equality supports substitution into pure logical functions of the compared values. It does not establish equality of omitted runtime state, pointer aliasing, equal outcomes of arbitrary executable calls, or permission interchangeability. A library that wants contents equality or an equivalence relation supplies it explicitly.

Saved proofs remain attached to old place versions after writes. A new capture does not retarget them. Arithmetic on logical integers remains mathematical; runtime integer operations retain the specified wrapping or checked-result interpretation. Capturing a runtime result does not reinterpret that computation as unbounded arithmetic.

Neither an abstract snapshot nor `Opaque<T>` hides logical or proof-bearing data from boundary checks on the underlying runtime type. Union restrictions, unverified generic instantiation checks, foreign ABI restrictions, and unsafe-code rules remain in force recursively. Unit equality cannot extract a payload, manufacture its proof fields, or forge a proof by constructing its zero-sized Rust marker.

Law traits for equality, order, clone, and hashing state relationships over snapshots or explicit views and must be tied to the actual runtime implementation. An `Opaque<F>` field contributes no condition to whole-snapshot `CloneEq`, regardless of how its payload is cloned; this does not promise equivalent runtime behavior. A hash law relating snapshot-equal inputs to equal hash behavior needs independent evidence that omitted payload state cannot distinguish those inputs. A hash that produces different results for equal snapshots cannot satisfy that law. Merely reading an opaque payload is not itself disqualifying if its influence can be proved irrelevant. Ordinary Rust `Hash` may also target a different, explicitly specified equality relation; do not conflate that law with snapshot equality.

For the proposed Locus vector, the raw storage contributes only `Unit`; cloning therefore need not prove equality of allocation identities. Whole-snapshot `CloneEq` is possible when element cloning preserves element snapshots (`T: CloneEq`) and the wrapper implementation proves the sequence relation. `T: Clone` alone is insufficient. Other representations may still need a law about an explicit view instead of whole snapshots. Separately, generating a `Clone` implementation from a stronger law trait must not collide with an existing `impl<T: Clone> Clone for Vec<T>`; provider/coherence rules remain an independent obligation.

### Type availability and lowering

The current target is a canonical snapshot family for supported types, not universal capture of arbitrary Rust types. Preserve the vision's scope, “every runtime value whose type it can see,” with supported-type and field requirements made explicit. A Locus-declared struct with an unsupported unwrapped field may itself have no snapshot. A generic function that captures a value requires a compiler-controlled `Snapshot` bound; logical types satisfy it with identity capture. Unsupported payloads can be deliberately placed behind `Opaque<T>`.

Snapshot rules are language-defined. Fixed library support may implement them where Rust coherence permits; ordinary users cannot register overrides. `Opaque` and `UnsafeCell` are recognized by type identity. `View` remains an ordinary trait and cannot redefine capture, validity, or framing. The proposed `SnapOf<T>` translation is compiler-generated erased storage for conditional logical fields, not a new user type with a custom snapshot mapping.

The bound supplies the normalized snapshot type. Generic and concrete code must agree exactly: an opaque fallback in generic code must not become `Nat` only at a concrete use. Closures, trait objects, associated-type recursion, and foreign generic trait methods require dedicated lowering cases rather than inferred blanket support.

Preserving ordinary Rust declarations with unsupported fields remains a concrete implementation gate. The [capture-family experiment](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spikes/capture-family/README.md) reports E0277 for plain generated bounds in `c09`/`c10`, and hand-written deferred implementations that preserve the declaration in `c11` and `c14`. Those use a higher-ranked lifetime bound whose checking rustc delays; that behavior has not been established as a supported long-term guarantee. The [unmarked-fields-and-the-superset issue](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/issues/unmarked-fields-and-the-superset.md) therefore stays open. This proposal requires an error at unsupported capture, not rejection of an otherwise valid runtime declaration, but does not claim the macro lowering has robustly met that requirement. Solving conditional logical-field storage does not solve this separate concrete-foreign-field problem.

Logical snapshot representations erase source lifetimes and retain no runtime borrow through phantom fields. The runtime `SnapOf<T>` carrier may retain `T`'s lifetimes and type distinctions as required for Rust checking; the logical field value and containing snapshot use the normalized `Snap<T>` family and must not retain those lifetimes. `Unit` similarly removes payload lifetimes from `Opaque` and `UnsafeCell` snapshots while leaving runtime ownership intact. Both lowered programs must resolve executable calls identically, and typing skeletons remain nonexecuted.

A closure or function pointer may eventually have an opaque observation without becoming callable in logic. A snapshot of a trait object does not supply missing verification evidence for its methods. Pinning and async suspension likewise need their normal ownership rules; a logical snapshot provides no right to move or resume the original value.

### Inspection and diagnostics

Provide a compiler inspection facility and IDE hover for `Snap<T>`. The proposed command spelling is illustrative:

```text
locus explain-snapshot 'Packet<u8>'

Snapshot: PacketSnapshot<...>
  tag: Nat                 captured u8 range applies
  bytes: Seq<Nat>          element snapshots
  cache: Unit             UnsafeCell payload excluded; empty footprint
  raw: Unit               Opaque payload excluded; empty footprint

Also show:
  public and private field visibility
  reference cuts and excluded memory
  capture requirements and observation footprint
  validity conditions and generic bounds
  generated nominal type ownership
  external assumptions and supported observers
```

Inspection respects source visibility for ordinary callers. An owner or authorized repository developer can inspect its own private generated layout. Debug output should make clear whether it describes a type, a symbolic term, or a concrete logical value known to the prover.

No debugger command is implemented by executing a ghost capture or exposing an erased logical value through runtime `Debug`. Markers are not the logical values they stand for. Compiler diagnostics must not disguise a wrong lowering classification by printing all markers alike.

Useful errors distinguish: unsupported foreign snapshot implementation; missing generic bound; unavailable borrowed place; cut referent; payload excluded by `Opaque` or `UnsafeCell`; missing permission or guard; unproved validity; private projection; retained proof depending on excluded state; a runtime function call in logical code (including `mint`); and an external call whose effects prevent framing an observation.

### Validation cases and implementation gates

| Case | Required outcome |
| --- | --- |
| Capture a readable integer twice with no write | Equal observations |
| Change one field | New parent observation; unaffected sibling preserved |
| Borrow one field mutably and inspect a sibling | Allowed; whole-parent capture unavailable |
| Save a snapshot of a mutable referent, then mutate it | Saved value unchanged; no retained runtime borrow |
| Two cells or `Opaque` wrappers, including different payload types | Snapshots are `Unit`; payload, type, and instance identity are not encoded |
| Wrap a `T` without a snapshot implementation | Runtime `Opaque<T>` remains usable and capturable as `Unit` |
| Obtain a payload reference from `Opaque<T>` | Ordinary borrow rules apply; fresh payload capture needs its own capability |
| Add a snapshot-erasure attribute or user override | Rejected; choosing a field type is the supported mechanism |
| Project a wrapper's `raw: Opaque<Native>` field | Both capture orders yield `Unit`; no hidden native-content projection |
| Mutate an `UnsafeCell` payload | Its own snapshot unchanged; a separately authorized payload observation may change |
| Call a cell read twice | No equality inferred merely from equal cell snapshots |
| Construct `1000` as `Snap<u8>` | Legal logical value; byte validity not supplied |
| Construct a snapshot containing a proof field | Correct proof required; no logical falsehood manufactured |
| Project or traverse a recursive reference cut | Address observation only; fresh memory read requires new access |
| Compare equal reference snapshots | No aliasing or permission equality inferred |
| Duplicate a permission snapshot | No duplicate live permission |
| Forget a returned-loan guard | No dependence on Drop for representation restoration |
| Return one `get_mut` loan and reborrow or retain it | Owner remains suspended until the actual loan end; exactly the final element is restored |
| Split into several element loans | Requires a separately specified protocol; no reconstruction from stale whole-owner snapshots |
| Catch a panic from a mutating wrapper operation | No observable stale logical contents |
| Read native vector contents from a descriptor-only snapshot | Rejected without an additional justified observation |
| Name `Vec<T>` or use its permitted unbounded runtime APIs | No bound imposed merely by the container declaration |
| Capture conditional vector contents | Element bound and specified storage semantics required; a compiling carrier is not a proof |
| Clone the wrapper with `T: CloneEq` | Whole-snapshot equality can be proved; plain `T: Clone` is insufficient |
| Compare supported abstract external snapshots | No singleton equality axiom; observers and effects respect established equalities |
| Use an unwrapped unsupported third-party field in ordinary Rust | Declaration remains usable; whole-object capture fails. Lowering must satisfy the open deferral gate |
| Wrap that field in `Opaque<T>` | Capture succeeds as `Unit`; runtime type and access syntax change |
| Declare a logical `Seq<Snap<T>>` field without a struct bound | Generated carrier permits runtime declaration; logical access needs the bound |
| Normalize conditional fields at `u8` and `u32`, including through aliases | Both semantic element snapshots are `Nat`; internal carriers introduce no logical mismatch |
| Borrow a conditional logical field twice | Same underlying place and correct borrow behavior; no per-use recreation |
| Store `s` through the generated conversion and read it | Result equals `s`, not merely some value of its type |
| Assign through a mutable reference to a conditional field | The original field is updated; subsequent logical read preserves the assigned value |
| Call contract-free `mint` twice | Existing call rule supplies neither equality nor inequality, despite the constant implementation |
| Copy a minted value and snapshot its holder repeatedly | Copied value and stable observations remain equal; no remint on capture |
| Call a stamp-returning helper twice or from a loop | Ordinary call abstraction; no common-result or fresh-value axiom without a contract |
| Call `mint` in a pure logical function, view, or proposition | Existing runtime-call prohibition rejects it; pass an earlier result instead |
| Use a stamp as a runtime branch condition or permission | Rejected by erasure or resource rules |
| Clone a stamped wrapper | Preserving the stamp supports equality; another contract-free call supplies neither relationship |
| Mutate only a stamped stream's payload | Wrapper snapshot remains equal when the stamp is framed unchanged; no stream-state conclusion follows |
| Hash equal snapshots using omitted payload data | Law requires proof that output cannot distinguish snapshot-equal inputs |
| Capture through generic and concrete types | Same logical type and interpretation |
| Capture an opaque type containing lifetimes | No lifetime retained by the logical result |
| Execute both lowered programs | Same runtime trait and method selection |

The initial implementation targets scalar and structural captures, explicit views, the two payload barriers, `mint` as an ordinary verified library function, and a vector wrapper with native-shaped and checked operations. Permission primitives need a separate accepted memory specification before implementation. Single-element mutable loans may use trusted restoration; multiple-loan support requires additional rules. Bound-free runtime naming of `Vec<T>` is a requirement supported by the carrier spike. The generated conditional-field translation and preserving ordinary Rust declarations with unsupported fields still need validation; universal foreign capture is not a release promise.

The remaining gates are the general storage-carrier translation under the specified read/write rules, including field borrows and lifetime erasure; reliable deferral of unsupported-field bounds without rejecting ordinary Rust; uniform cuts through associated types; a general checkable loan-restoration mechanism; and an explicit list of supported native-library observations. `mint` adds no kernel feature or special erasure obligation: it exercises existing call-contract and ghost-value rules. Rust typing results do not establish the carrier's value-preservation guarantees.

### Relationship to the existing RFCs

This proposal replaces implicit model capture with snapshot capture and makes `Model`/`View` an optional ordinary library abstraction. It retains versioned values, proof-result contracts, final-value restrictions, capture validity conditions, ghost erasure, and proof-data boundaries. It fixes `UnsafeCell<T>` and Locus's `Opaque<T>` to `Unit`, rejects field-level snapshot attributes, adds optional stored stamps via `mint`, retains single-element final-value loan contracts, and supplies native-shaped methods beside checked methods.

This reconciliation updates RFC 0001's protected trait and storage lowering (superseding decision 51); RFC 0002's decisions 99, 104, 107, 108 and 113–117; RFC 0005's implicit model core term; RFC 0006's capture bounds and whole-snapshot clone law; and RFC 0007's unreviewed trait inventory. Historical numbered decisions retain the reasoning at the time, but these current sections and this RFC replace the listed choices. The restricted vision wording remains and `unmarked-fields-and-the-superset` stays open.

The following expectation inventory is measured against PR head `f05e2d8`, not the earlier estimates. Legacy example filenames remain stable where only their spelling or explanation changed. The containing PR retains `expectation-change`; no RFC is marked accepted. The source discussion is preserved in the [draft specification Page](https://chatgpt.com/space/page_3baf0f34db548191af9258053663e21b).

#### Outcome changes

| Previous rejected example, now accepted | Reason |
| --- | --- |
| `models/opaque-field-read-as-a-place.accept.lc` | The field now explicitly has type Opaque<T>, with a Unit snapshot. |
| `models/opaque-fields-of-two-types.accept.lc` | Both type-based barriers have the same Unit snapshot. |
| `models/declared-model-is-not-the-snapshot.accept.lc` | An explicit view no longer changes the captured argument type. |
| `models/field-of-capture-confused-silently.accept.lc` | Both capture orders now yield the same structural snapshot. |
| `models/snapshot-argument-not-written.accept.lc` | An implicit runtime argument capture already supplies the snapshot. |
| `models/model-for-one-instantiation.accept.lc` | Ordinary View coherence permits a disjoint concrete instantiation. |
| `collections/length-at-element-without-model.accept.lc` | Native-shaped len returns usize without a snapshot bound or proof. |

These seven changes migrate the source to the new design as well as its expected outcome. All other pre-existing accept/reject outcomes are retained. The following table records every changed expected diagnostic; reasons follow the snapshot/view, barrier, checked-API, and removed-derive rules rather than weakening proof checking.

#### Expected diagnostic changes

| Example | Previous expectation | Reconciled expectation |
| --- | --- |
| `collections/push-without-model-lemma.reject.lc` | cannot prove `items.len() <= cap`: `items.len()` is the length of the model of `items`, and `counted` and `grew` speak of `snap(items).elements()`; call the lemma `Vec::<u32>::model_len(snap(items))` | ordinary push returns no proof relating the new contents to the old contents; use push_checked |
| `compatibility/model-impl-marked-unsafe.reject.lc` | only Locus writes an implementation of `Model`: a model is declared as `impl Model for Celsius`, with no `unsafe` | only compiler lowering or designated library support implements Snapshot |
| `core/congruence-through-a-model.reject.lc` | `same` is an equality of the models of `a` and `b`, and `x_of` takes a snapshot: `eq::congr` needs a function of what the equality is between | `same` equates the explicit views of the inner fields, and `x_of` takes a snapshot: `eq::congr` needs a function of what the equality is between |
| `core/rewrite-misses-a-field-read.reject.lc` | `rewrite` finds nothing to replace: `same` is about the model of `a`, and `big` reads the field `a.inner.x`, which is no part of that model | `rewrite` finds nothing to replace: `same` is about the view of `a`, and `big` reads the field `a.inner.x`, which is no part of that view |
| `export/model-bound-on-impl.reject.lc` | an exported program has no `Model`, and removing the bound from an `impl` could change which implementation a call selects, so an exported crate has a `Model` bound only on a function | an exported program has no `Snapshot`, and removing the bound from an `impl` could change which implementation a call selects, so an exported crate has a `Snapshot` bound only on a function |
| `export/model-bound.reject.lc` | `count` is reachable from other crates, so it cannot have the bound `T: Model` | `count` is reachable from other crates, so it cannot have the bound `T: Snapshot` |
| `logical-layer/equal-models-not-equal-fields.reject.lc` | cannot prove `a.inner.x === b.inner.x`: `a === b` compares models, and `Inner`'s model forgets `x` | cannot prove `a.inner.x === b.inner.x`: `a === b` compares models, and `Inner`'s view forgets `x` |
| `logical-layer/logic-fn-takes-bounded-type-itself.reject.lc` | a logic function's parameters are logical types, and `T` is bounded by `Model`, so it stands for a runtime type: write `Snap<T>`, or `T::Logic` | Snapshot alone does not prove that T is logical: write a parameter of type Snap<T>, or require T: Logical |
| `logical-layer/negative-through-derived-model.reject.lc` | `Bad` refers to itself, through the model of `Holder`, in a parameter of a logical function type | `Bad` refers to itself, through the snapshot of `Holder`, in a parameter of a logical function type |
| `models/associated-type-field-read-without-bound.reject.lc` | `F::Out` cannot be read in logical code, and `Holder<F>` holds one in its field `item`: add the bound `F::Out: Model` | `F::Out` cannot be read in logical code, and `Holder<F>` holds one in its field `item`: add the bound `F::Out: Snapshot` |
| `models/derive-model-for-generic.reject.lc` | `Model` can be derived only for a type without type parameters, for now | Locus supplies no Model derive; write an explicit library view |
| `models/derive-model-through-generic-cycle.reject.lc` | cannot derive `Model` for `Node`: it reaches itself through the reference in `Wrap`'s field `inner`, so its field `w` is cut in its snapshot, and a cut field has no model | Locus supplies no Model derive; write an explicit library view |
| `models/derive-model-through-reference-cycle.reject.lc` | cannot derive `Model` for `Node`: it reaches itself through the reference in `next`, so its model type would have no values | Locus supplies no Model derive; write an explicit library view |
| `models/derive-model-with-opaque-field.reject.lc` | cannot derive `Model` for `Server`: the field `listener` is marked `#[opaque]`, and its type `TcpListener` has no model | Locus supplies no Model derive; write an explicit library view |
| `models/field-of-capture-is-a-snapshot.reject.lc` | `whole.stack` is the snapshot of a `Stack`, which has no `len`: a snapshot holds its fields' snapshots, not their models; write `Stack::model(whole.stack)` | `whole.stack` is the snapshot of a `Stack`, which has no `len`: a snapshot holds its fields' snapshots, not their models; write `Stack::view(whole.stack)` |
| `models/foreign-trait-method-parameter.reject.lc` | `I` cannot be read in logical code, and the bound `I: Model` cannot be added: `extend` is declared by `Extend`, a trait of another crate | `I` cannot be read in logical code, and the bound `I: Snapshot` cannot be added: `extend` is declared by `Extend`, a trait of another crate |
| `models/foreign-type-as-bounded-argument.reject.lc` | `TcpListener` has no model, so it cannot be the `T` of `kept`, which asks for `T: Model`; wrap it in a struct of the program's own, with the field marked `#[opaque]` | `TcpListener` has no snapshot, so it cannot be the `T` of `kept`, which asks for `T: Snapshot`; wrap it in a struct of the program's own, with the field wrapped in `Opaque<T>` |
| `models/logic-fn-of-type-called-on-place.reject.lc` | `s` is captured as its model, a `Seq<Nat>`, which has no `room`, and nothing else is tried: call the logic function of `Stack` by its path, `Stack::room(snap(s))` | the captured Stack snapshot has no room method; call Stack::room(snap(s)) explicitly |
| `models/model-bound-missing-on-impl.reject.lc` | `T` cannot be read in logical code: add the bound `T: Model` to the `impl` | `T` cannot be read in logical code: add the bound `T: Snapshot` to the `impl` |
| `models/model-bound-missing-on-type.reject.lc` | `T` cannot be read in logical code, and `fits` reads `items`, a `Vec<T>`: add the bound `T: Model` to `Shelf` | `T` cannot be read in logical code, and `fits` reads `items`, a `Vec<T>`: add the bound `T: Snapshot` to `Shelf` |
| `models/model-for-foreign-type.reject.lc` | only the crate that defines `Instant`, or Locus's library, can declare its model | only the crate that defines `Instant`, or Locus's library, can declare its view |
| `models/model-needs-a-valid-snapshot.reject.lc` | cannot prove that 3 is inside `w.bytes`: a model is given every `Snap<Word>`, and only one captured from a place is known to hold four bytes | cannot prove that 3 is inside `w.bytes`: a view is given every `Snap<Word>`, and only one captured from a place is known to hold four bytes |
| `models/model-of-a-snapshot-without-its-type.reject.lc` | `model` is given a snapshot value, whose type does not say which type it is a snapshot of: write `Stack::model(whole.stack)` | `view` is given a snapshot value, whose type does not say which type it is a snapshot of: write `Stack::view(whole.stack)` |
| `models/model-of-itself.reject.lc` | inside `model`, `stack` cannot be taken to the `Seq<Nat>` being defined; read its fields instead | the recursive logical call does not decrease structurally or by a declared measure |
| `models/model-reads-cell.reject.lc` | `c.hits` is the snapshot of a `Cell<u32>`, which holds nothing of its contents, and not a `Nat`: a `Cell`'s contents can change while it is shared, so no snapshot or model holds them | `c.hits` is the snapshot of a `Cell<u32>`, which holds nothing of its contents, and not a `Nat`: a `Cell`'s contents can change while it is shared, so no snapshot or view holds them |
| `models/model-takes-a-validity-proof.reject.lc` | a model takes its type's snapshot and nothing else: it is defined for every `Snap<Word>`, valid or not | View::view has one snapshot parameter; this implementation adds a proof parameter |
| `models/model-through-helper.reject.lc` | `size` takes a `Stack` to its model, and the model of `Stack` calls `size`, so the model would depend on itself | mutual logical recursion through the view and its helper is unsupported |
| `models/models-depend-on-each-other.reject.lc` | the model of `Node` depends on itself, through the model of `Link` | these view implementations form unsupported mutual logical recursion |
| `models/two-models.reject.lc` | `Celsius` already has a model | `Celsius` already has a view |
| `models/type-parameter-read-without-bound.reject.lc` | `T` cannot be read in logical code: add the bound `T: Model` | `T` cannot be read in logical code: add the bound `T: Snapshot` |
| `models/unmarked-struct-captured.reject.lc` | `Server` has no snapshot, so `server` cannot be captured: its field `listener` is a `TcpListener`, which Locus cannot see; mark the field `#[opaque]` | `Server` has no snapshot, so `server` cannot be captured: its field `listener` is a `TcpListener`, which Locus cannot see; wrap the field in `Opaque<T>` |
| `models/unmarked-struct-model-declared.reject.lc` | `Server` has no snapshot, so no model can be declared for it: its field `listener` is a `TcpListener`, which Locus cannot see; mark the field `#[opaque]` | `Server` has no snapshot, so no view can be declared for it: its field `listener` is a `TcpListener`, which Locus cannot see; wrap the field in `Opaque<T>` |
| `traits/self-read-without-model-supertrait.reject.lc` | `Self` cannot be read in logical code: no `Model` bound is in scope, and none is implied; write `trait Stable: Model` | `Self` cannot be read in logical code: no `Snapshot` bound is in scope, and none is implied; write `trait Stable: Snapshot` |

This inventory contains 7 outcome changes and 33 changed diagnostic expectations. New examples under `litmus/snapshots/` exercise the Unit barriers, readable-place capture, frame behavior, explicit views, carrier reads and writes, clone laws, and ordinary `mint` calls. They are design expectations, not results from an implemented Locus checker.

#### Run-expression migration

The expected runtime result is unchanged; native-shaped `len` removes the tuple projection.

| Example | Before | After |
| --- | --- | --- |
| `contracts/checked-indices.accept.lc` | `in_bounds(&[1, 2, 3], &[0, 5, 2]).len().0 => 2` | `in_bounds(&[1, 2, 3], &[0, 5, 2]).len() => 2` |

The current sources for comparison are the [snapshot rules](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spec/models.md), [borrow and mutation rules](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spec/runtime.md), [contracts and invariants](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spec/contracts.md), [collection interfaces](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spec/collections.md), [trait rules](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spec/traits.md), and [capture-family experiment](https://github.com/nikhilgarg28/locus/blob/f05e2d892ca26673218e1bd5fa5988377643ef53/spikes/capture-family/README.md). The family experiment supports explicit-bound lowering, not a soundness proof or universal capture.

## Rules

This table inventories rules changed by this reconciliation, retaining stable IDs. Existing call-contract and runtime-call rules justify `mint`; they are not weakened.

| Rule | Decided examples |
| --- | --- |
| `logic.types.positive` | `logical-layer/logical-fields.accept.lc`, `models/derive-model-for-recursive-type.accept.lc`, `proofs/positive-proposition.accept.lc`, `logical-layer/negative-logical-type.reject.lc`, `logical-layer/negative-through-derived-model.reject.lc`, `logical-layer/negative-through-forall.reject.lc`, `proofs/negative-proposition.reject.lc` |
| `logic.ghost.positions-logical` | `logical-layer/logical-field-from-logic-block.accept.lc`, `logical-layer/runtime-call-bound-first.accept.lc`, `logical-layer/runtime-call-in-proof-field.accept.lc`, `logical-layer/runtime-value-through-logic-block.accept.lc`, `logical-layer/runtime-call-in-proof-argument.reject.lc`, `logical-layer/runtime-value-to-logical-field.reject.lc`, `logical-layer/runtime-value-to-logical-parameter.reject.lc` |
| `logic.unsafe.invariant` | `logical-layer/invariant-type-through-unverified.accept.lc`, `logical-layer/unverified-function-given-fields.accept.lc`, `logical-layer/transmute-into-invariant-type.reject.lc` |
| `logic.code.logical-values` | `logical-layer/logic-fn-model-result.accept.lc`, `logical-layer/logic-fn-runtime-result.reject.lc`, `logical-layer/logic-fn-unit-result.reject.lc` |
| `logic.fn.parameters` | `logical-layer/logic-fn-model-result.accept.lc`, `logical-layer/logic-fn-over-bounded-parameter.accept.lc`, `models/logic-fn-of-type-called-by-path.accept.lc`, `models/snapshot-argument-not-written.accept.lc`, `models/snapshot-argument-written.accept.lc`, `logical-layer/logic-fn-runtime-parameter.reject.lc`, `logical-layer/logic-fn-self-of-runtime-type.reject.lc`, `logical-layer/logic-fn-takes-bounded-type-itself.reject.lc` |
| `logic.eq.runtime-values` | `logical-layer/equal-models-equal-field-models.accept.lc`, `models/declared-model-is-not-the-snapshot.accept.lc`, `models/default-model-is-the-snapshot.accept.lc`, `models/derived-model-equal-fields.accept.lc`, `logical-layer/equal-models-not-equal-fields.reject.lc` |
| `logic.eq.snapshots` | `logical-layer/cell-snapshot-kept-by-move.accept.lc`, `models/foreign-field-marked-opaque.accept.lc`, `models/snapshot-through-generic-view.accept.lc`, `models/snapshot-through-reference.accept.lc`, `models/snapshot-unchanged.accept.lc`, `models/snapshot-with-associated-type-field.accept.lc`, `models/snapshots-equal-fields.accept.lc`, `logical-layer/cell-contents-unknown.reject.lc` |
| `logic.bool.as-proposition` | `logical-layer/bool-connective-is-a-bool.accept.lc`, `logical-layer/bool-result-by-two-implications.accept.lc`, `logical-layer/bool-result-tied-by-iff.accept.lc`, `logical-layer/bool-to-prop-parameter-written.accept.lc`, `logical-layer/bool-where-proposition-wanted.accept.lc`, `traits/parameter-that-converts-to-a-proposition.accept.lc`, `logical-layer/bool-to-prop-local.reject.lc`, `logical-layer/bool-to-prop-parameter.reject.lc`, `logical-layer/connective-beside-comparison-as-bool.reject.lc`, `logical-layer/iff-of-an-integer.reject.lc` |
| `collections.contents` | `collections/vector-lent-to-unverified.accept.lc`, `collections/vector-passed-on-without-bound.accept.lc`, `models/snapshot-of-vector-without-model.accept.lc`, `collections/vector-in-unverified-function.reject.lc`, `collections/vector-snapshot-is-not-a-sequence.reject.lc` |
| `collections.model` | `contracts/stack-push-by-value.accept.lc`, `models/vector-without-element-model.accept.lc`, `snapshots/explicit-view.accept.lc`, `snapshots/view-map-needs-view-bound.reject.lc` |
| `collections.std-inside` | `collections/forwarded-shared-method.accept.lc`, `collections/out-to-standard-and-back.accept.lc`, `collections/contents-after-standard-round-trip.reject.lc`, `logical-layer/forwarded-method-at-plain-data.reject.lc`, `logical-layer/forwarded-method-at-proof-holder.reject.lc` |
| `collections.vec.contracts` | `collections/element-through-get.accept.lc`, `collections/length-after-set.accept.lc`, `collections/out-to-standard-and-back.accept.lc`, `collections/popped-nothing-from-empty.accept.lc`, `collections/push-keeps-invariant-on-contents.accept.lc`, `collections/push-without-model-lemma.reject.lc`, `collections/push-without-room.reject.lc` |
| `collections.vec.element-loan` | `collections/element-lent-mutably.accept.lc`, `collections/push-into-row.accept.lc`, `collections/element-lent-without-bounds.reject.lc` |
| `collections.vec.indexing` | `collections/index-assignment-runs.accept.lc`, `collections/element-through-index.reject.lc`, `collections/length-after-index-assignment.reject.lc` |
| `collections.element-types` | `collections/closure-elements-in-standard-vector.accept.lc`, `collections/length-at-element-without-model.accept.lc`, `collections/vector-passed-on-without-bound.accept.lc`, `collections/closure-elements-in-own-vector.reject.lc` |
| `collections.clone-snapshots` | `snapshots/vector-clone-whole-snapshot.accept.lc`, `snapshots/vector-ordinary-clone-no-law.reject.lc` |
| `traits.model-bounds` | `models/model-bound-on-trait.accept.lc`, `traits/model-as-supertrait.accept.lc`, `traits/specification-and-code-in-two-traits.accept.lc`, `traits/self-read-without-model-supertrait.reject.lc` |
| `traits.laws.derive` | `traits/derive-clone-eq-with-invariant.accept.lc`, `traits/derive-clone-eq-beside-derive-clone.reject.lc`, `traits/derive-clone-eq-over-a-field-without-the-law.reject.lc` |
| `core.elaboration` | `core/congruence-at-the-snapshot.accept.lc`, `core/congruence-through-a-model.reject.lc`, `core/rewrite-misses-a-field-read.reject.lc` |
| `core.conversion` | `core/constant-unfolds.accept.lc`, `core/conversion-by-evaluation.accept.lc`, `core/predicate-used-as-its-body.accept.lc`, `core/unproved-index-of-integers.accept.lc`, `core/conversion-is-not-arithmetic.reject.lc`, `core/private-body-stays-folded.reject.lc`, `core/recursive-call-stays-folded.reject.lc`, `proofs/unfold-private-body.reject.lc` |
| `models.snapshots` | `logical-layer/equal-models-equal-field-models.accept.lc`, `models/captured-fields-wrap-written.accept.lc`, `models/derive-model-for-recursive-type.accept.lc`, `models/derive-model-with-cell-field.accept.lc`, `models/derived-model-equal-fields.accept.lc`, `models/derived-model-keeps-proof-holder.accept.lc`, `models/field-of-capture-and-capture-of-field.accept.lc`, `models/generic-snapshot-at-cycle.accept.lc`, `models/integer-snapshots-are-one-type.accept.lc`, `models/read-through-cut-reference.accept.lc`, `models/snapshot-of-vector-without-model.accept.lc`, `models/snapshot-through-generic-view.accept.lc`, `models/snapshot-through-reference.accept.lc`, `models/snapshot-unchanged.accept.lc`, `models/snapshots-equal-fields.accept.lc`, `collections/vector-snapshot-is-not-a-sequence.reject.lc`, `logical-layer/equal-models-not-equal-fields.reject.lc`, `models/captured-fields-do-not-wrap.reject.lc`, `models/equal-snapshots-say-nothing-of-addresses.reject.lc`, `models/model-through-a-cut.reject.lc`, `models/snapshot-through-generic-cycle.reject.lc`, `models/snapshot-through-reference-cycle.reject.lc` |
| `models.snapshots.cut` | `models/body-reads-behind-a-cut.accept.lc`, `contracts/invariant-reads-behind-a-cut.reject.lc`, `models/model-through-a-cut.reject.lc`, `models/proof-field-inside-a-cut-field.reject.lc` |
| `models.snapshots.seen` | `models/foreign-field-unmarked.accept.lc`, `models/foreign-type-wrapped.accept.lc`, `models/opaque-field-read-as-a-place.accept.lc`, `collections/standard-vector-in-logical-code.reject.lc`, `models/closure-not-captured.reject.lc`, `models/foreign-type-as-bounded-argument.reject.lc`, `models/snapshot-through-associated-type.reject.lc`, `models/unmarked-struct-captured.reject.lc` |
| `models.snapshots.opaque` | `models/foreign-field-marked-opaque.accept.lc`, `models/foreign-field-unmarked.accept.lc`, `models/marked-struct-captured.accept.lc`, `models/opaque-field-read-as-a-place.accept.lc`, `models/opaque-field-through-the-snapshot.accept.lc`, `models/opaque-fields-of-two-types.accept.lc`, `snapshots/opaque-unit.accept.lc`, `models/snapshot-of-union.reject.lc`, `models/unmarked-struct-captured.reject.lc`, `models/unmarked-struct-model-declared.reject.lc`, `snapshots/field-erasure-attribute.reject.lc`, `snapshots/opaque-payload.reject.lc` |
| `models.snapshot-privacy` | `models/private-field-in-own-module.accept.lc`, `models/private-field-of-snapshot.reject.lc` |
| `models.frozen-data` | `logical-layer/cell-snapshot-kept-by-move.accept.lc`, `models/hand-written-model.accept.lc`, `snapshots/opaque-unit.accept.lc`, `snapshots/unsafe-cell-frame.accept.lc`, `logical-layer/cell-contents-unknown.reject.lc`, `models/model-reads-cell.reject.lc` |
| `models.capture.available` | `snapshots/capture-sibling.accept.lc`, `snapshots/capture-parent-during-loan.reject.lc` |
| `models.capture.footprint` | `snapshots/unsafe-cell-frame.accept.lc`, `snapshots/runtime-read-from-unit.reject.lc` |
| `models.valid` | `models/snapshot-parameter-with-validity.accept.lc`, `models/valid-array-length.accept.lc`, `models/valid-field-of-captured-struct.accept.lc`, `models/integer-range-tighter.reject.lc` |
| `models.valid.captured` | `models/integer-range.accept.lc`, `models/snapshot-parameter-with-validity.accept.lc`, `models/valid-array-length.accept.lc`, `models/valid-field-of-captured-struct.accept.lc`, `models/integer-range-tighter.reject.lc` |
| `models.valid.not-assumed` | `models/snapshot-parameter-with-validity.accept.lc`, `models/model-needs-a-valid-snapshot.reject.lc`, `models/snapshot-parameter-not-valid.reject.lc`, `proofs/range-of-a-nat.reject.lc` |
| `models.observation` | `models/field-of-capture-taken-to-its-model.accept.lc`, `models/integer-model-no-overflow.accept.lc`, `models/snapshot-argument-not-written.accept.lc`, `models/snapshot-argument-written.accept.lc`, `observation/after-mutable-borrow-ends.accept.lc`, `observation/before-move.accept.lc`, `models/model-of-a-snapshot-without-its-type.reject.lc`, `models/wrapping-is-not-the-model.reject.lc` |
| `models.default` | `models/declared-model-is-not-the-snapshot.accept.lc`, `models/default-model-is-the-snapshot.accept.lc`, `models/equal-fields-without-model.accept.lc`, `models/marked-struct-captured.accept.lc`, `models/vector-without-element-model.accept.lc`, `snapshots/view-does-not-change-capture.accept.lc`, `models/field-of-capture-is-a-snapshot.reject.lc`, `snapshots/view-map-needs-view-bound.reject.lc` |
| `models.by-parts` | `models/tuple-modeled-by-parts.accept.lc`, `models/vector-without-element-model.accept.lc`, `snapshots/explicit-view.accept.lc`, `models/field-of-capture-is-a-snapshot.reject.lc` |
| `models.capture.fields` | `models/field-of-capture-and-capture-of-field.accept.lc`, `models/field-of-capture-confused-silently.accept.lc`, `models/field-of-capture-taken-to-its-model.accept.lc`, `snapshots/capture-sibling.accept.lc`, `snapshots/view-does-not-change-capture.accept.lc`, `models/field-of-capture-is-a-snapshot.reject.lc` |
| `models.capture.methods` | `models/logic-fn-of-type-called-by-path.accept.lc`, `models/method-resolves-on-the-capture.accept.lc`, `models/logic-fn-of-type-called-on-place.reject.lc` |
| `models.capture.bound` | `logical-layer/logic-fn-over-bounded-parameter.accept.lc`, `models/foreign-type-wrapped.accept.lc`, `models/generic-snapshot-at-cycle.accept.lc`, `models/model-bound-on-function.accept.lc`, `models/model-bound-on-inherent-impl.accept.lc`, `models/model-bound-on-type.accept.lc`, `models/snapshot-with-associated-type-field.accept.lc`, `models/associated-type-field-read-without-bound.reject.lc`, `models/foreign-trait-method-parameter.reject.lc`, `models/foreign-type-as-bounded-argument.reject.lc`, `models/model-bound-missing-on-impl.reject.lc`, `models/model-bound-missing-on-type.reject.lc`, `models/type-parameter-read-without-bound.reject.lc`, `snapshots/storage-read-without-bound.reject.lc`, `snapshots/view-map-needs-view-bound.reject.lc`, `traits/self-read-without-model-supertrait.reject.lc` |
| `models.total` | `models/model-defined-for-every-snapshot.accept.lc`, `snapshots/explicit-view.accept.lc`, `models/model-needs-a-valid-snapshot.reject.lc`, `models/model-takes-a-validity-proof.reject.lc` |
| `models.one-per-type` | `models/declared-model-of-generic-type.accept.lc`, `models/hand-written-model.accept.lc`, `models/model-for-one-instantiation.accept.lc`, `models/two-models.reject.lc` |
| `models.derived` | `snapshots/view-does-not-change-capture.accept.lc`, `models/derive-model-for-generic.reject.lc`, `models/derive-model-through-generic-cycle.reject.lc`, `models/derive-model-through-reference-cycle.reject.lc`, `models/derive-model-with-opaque-field.reject.lc` |
| `models.bounds-on-functions` | `models/declared-model-of-generic-type.accept.lc`, `models/model-bound-on-function.accept.lc`, `models/model-bound-on-impl.accept.lc`, `models/model-bound-on-inherent-impl.accept.lc`, `models/model-bound-on-trait.accept.lc`, `models/model-bound-on-type.accept.lc`, `export/model-bound-on-impl.reject.lc` |
| `models.declared-by-owner` | `models/hand-written-model.accept.lc`, `models/model-for-foreign-type.reject.lc` |
| `models.own-definition` | `models/hand-written-model.accept.lc`, `models/model-helper-reads-fields.accept.lc`, `models/model-of-itself.reject.lc`, `models/model-through-helper.reject.lc`, `models/models-depend-on-each-other.reject.lc` |
| `models.storage` | `snapshots/storage-read-normalizes.accept.lc`, `snapshots/storage-through-borrow.accept.lc`, `snapshots/storage-read-without-bound.reject.lc` |
| `models.stamps` | `snapshots/mint-copy.accept.lc`, `snapshots/mint-equality.reject.lc`, `snapshots/mint-in-logic.reject.lc`, `snapshots/mint-inequality.reject.lc` |
| `proofs.equality` | `core/congruence-at-the-snapshot.accept.lc`, `proofs/equality-chain-as-term.accept.lc`, `proofs/rewrite-with-equality.accept.lc`, `core/congruence-through-a-model.reject.lc`, `core/rewrite-misses-a-field-read.reject.lc`, `proofs/equality-chain-mismatched.reject.lc`, `proofs/reflexivity-of-two-values.reject.lc` |
| `export.plain-interface` | `export/dropped-postcondition.accept.lc`, `export/locus-logical-field.accept.lc`, `export/private-enum-with-invariant.accept.lc`, `export/private-logical-field.accept.lc`, `export/public-enum-with-sealed-payload.accept.lc`, `export/public-field-of-sealed-type.accept.lc`, `export/public-precondition-when-built.accept.lc`, `export/wrapped-precondition.accept.lc`, `export/model-bound.reject.lc`, `export/public-enum-with-invariant.reject.lc`, `export/public-logical-field.reject.lc`, `export/public-postcondition.reject.lc`, `export/public-precondition.reject.lc`, `export/public-proof-in-option.reject.lc` |
| `compat.syntax.contextual-keywords` | `compatibility/keywords-as-names.accept.lc`, `compatibility/raw-identifier-logic.accept.lc`, `compatibility/logic-block-is-a-struct-literal.reject.lc` |
| `compat.names.no-methods` | `compatibility/user-method-named-model.accept.lc` |
| `compat.names.model-impls` | `compatibility/model-where-unsafe-is-forbidden.accept.lc`, `compatibility/model-impl-marked-unsafe.reject.lc` |

## Interactions

| Area | Effect |
| --- | --- |
| Types | Compiler-owned `Snapshot`; ordinary `View`; no implicit model or field erasure attribute; deferred storage is internal. |
| Propositions and proofs | Equality is snapshot equality; validity remains capture-dependent; `mint` uses the existing call abstraction. |
| Ownership and mutation | Capture is a readable-place operation; barriers have empty footprints; one-element mutable loans require restoration. |
| Erasure and runtime behavior | Logical values erase; executable calls retain their behavior; storage conversions preserve logical values and places. |
| Rust interoperability | Foreign snapshots require legal designated support or explicit wrappers; ordinary Rust declarations must remain valid. |
| Diagnostics | Explain snapshots, footprints, bounds, cuts, barriers, and unsupported foreign capture without running ghost code. |
| Trust model | Existing lowering, driver, and library responsibilities are reconciled; no mint axiom or arbitrary user snapshot hook is added. General permission primitives remain a separate implementation gate. |

## Alternatives

Implicit model capture and language-level derived models are superseded because they make capture/equality depend on abstraction declarations. Field attributes with fixed erasure meaning were considered and rejected by the owner in favor of type-based barriers. Globally fresh IDs are stronger than the selected contract-hidden stamp and are not supplied by `mint`. A bound on every container declaration unnecessarily excludes runtime-only element types. Universal foreign snapshots remain unavailable on the selected lowering; the proposal does not assume specialization or relax Rust coherence.

## Prior art

Rust's ownership, interior-mutability, and coherence rules remain authoritative for executable code. Explicit views separate representation from abstraction as in other verification languages, while Locus retains kernel-checked explicit proof values and its own generated snapshot family. The linked spikes are typing experiments and do not establish soundness or implementation completeness.

## Unresolved questions

[snapshot-view-reconciliation](../../issues/snapshot-view-reconciliation.md) tracks generated carrier reads, writes, aliasing and lifetime erasure, uniform associated-type cuts, the foreign-field deferral gate, and general loan restoration. [unmarked-fields-and-the-superset](../../issues/unmarked-fields-and-the-superset.md) stays open. General permissioned memory and concurrent protocols require their own accepted implementation specifications; the memory API in this RFC is an obligation outline, not new callable trusted intrinsics.

