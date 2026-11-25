# RFC 0007: Provided trait implementations and core law traits

- **Status:** draft
- **Review:** I04 hashing architecture reviewed adversarially and adopted by the owner on 2026-11-25; the remaining provider design is still a draft
- **Issue:** [provided-trait-implementations](../../issues/provided-trait-implementations.md)
- **Supersedes:** none

## Summary

**Draft proposal, with an adopted hashing decision.** Checked traits may contain nested `impl` blocks that supply ordinary trait implementations. These are per-implementation templates, not blanket impls. Locus generates and verifies the bodies; rustc retains ownership, overlap and trait-selection rules. The initial library connects equality, cloning and ordering to Rust traits. Hashing uses a checked emitter and a logical trace, retaining Rust's unrestricted `Hash::hash<H: Hasher>` signature.

The owner adopted the limited I04 architecture below and requested that this RFC
and its PR be updated. That does not accept the entire provider proposal: this
RFC remains a draft, and the general call-fact and lowering questions remain
open. It preserves the owner's chosen nested `impl` syntax and `Self` scoping.
The hashing section names each required trusted fact under the existing trust
categories; it does not silently designate every standard implementation. It depends on
the traits proposal in RFC 0006 and the snapshot reconciliation in RFC 0008;
other changes in the containing PR are tracked separately. The source is the
[design one-pager](https://chatgpt.com/space/page_ec1c67b2ba4c8191b70aa8e87d62fc07).

**Interaction requiring review:** the intended availability of a bridge's fact
at an ordinary call is not supplied by current
`contracts.calls.unrelated`: that rule permits only proofs in the result, plus
the separately defined observations of runtime operations. Generating a wrapper
that drops a proof does not, by itself, give that proof to a caller. How a bridge
exports a checked call fact is an unresolved part of this draft, not an implicit
amendment of the existing rule. The trait signatures also depend on the proposed
design for logical members and `Snapshot` bounds on traits.

## Motivation

A type should implement a checked operation once and expose the corresponding
ordinary Rust trait without an independently written, potentially inconsistent
implementation. A checked equality result and its proof can supply
`PartialEq::eq`; equivalence laws can separately justify Rust's `Eq` marker.
Hash compatibility must refer to the same logical equality.

The owner asked for this mechanism to apply to user traits, without special
compiler cases for `PartialEq`, `Clone`, or `Ord`, and without changing Rust's
coherence rules.

## Design

### Programs and generated implementations

The proof notation in this example is schematic; `a0` and `b0` denote entry
snapshots, not additional runtime parameters.

```rust
trait EqChecked: EqSpec {
    fn eq_checked(&self, rhs: &Self)
        -> (r: bool, @(r === Self::equiv(a0, b0)));

    impl PartialEq for Self {
        fn eq(&self, rhs: &Self) -> bool {
            <Self as EqChecked>::eq_checked(self, rhs).0
        }
    }
}
```

Under this proposal, `impl EqChecked for UserId { ... }` opts into a generated
`impl PartialEq for UserId` with the shown method body. An ordinary equality
call returns the Boolean computed by `eq_checked`; its proof is erased.

An additional handwritten `impl PartialEq for UserId`, or a conflicting derive,
is rejected. A type cannot retain the `EqChecked` guarantee while opting out
of its required implementation. A library that needs an independently usable
checked operation separates it into a base trait and places the provided
implementation in another trait.

### Binding, instantiation and guarantees

**Binding and generation.** In the entire nested `impl` header, `Self` denotes the outer trait’s implementing type. Inside the block, including member signatures, `Self` denotes the inner implementation’s target; lowercase `self` remains the receiver value. No outer-type alias is introduced. Implementing the enclosing trait opts into every nested implementation, instantiated in the implementing crate with its generics and bounds. Inherited requirements do not regenerate a supertrait’s implementations.

**Guarantees and conflicts.** A provided `impl Target<Args> for Destination` exports the bound `Destination: Target<Args>`. Self-targets lower to supertraits; other targets require corresponding `where` bounds, explicit initially where rustc does not propagate them. Existing, duplicate or overlapping implementations are errors—even if their bodies match. There is no “generate if absent,” precedence or opt-out that retains the enclosing trait’s guarantee. Separate the checked operation into a base trait when independent use is wanted. Alternative verified implementations are deferred. Shared destinations should normally use bounds, source-parameterized traits or newtypes.

The rule applies to user traits as well as standard traits. A nested
implementation is declared by the enclosing trait's author; importing another
module does not attach new implementations to that trait. Any conditional
provided implementation must export only its conditional guarantee. Detailed
conditional lowering remains for review.

**Verification.** The bridge is a required contract, not just a macro convention: Locus checks each supplied body against the checked method’s specification and binds the guarantee to the selected implementation, including across crates. Ordinary Rust trait bounds alone gain no logical laws. Target defaults must establish any guarantees claimed for them; unchecked overrides cannot bypass the bridge. Non-Self destinations remain subject to rustc’s ordinary rules. Initially, targets are safe traits; unsafe-trait obligations require a separate design.

### Core trait signatures

Signatures below are schematic. `S` means an admissible frozen snapshot of the implementing type; `O = Snap<Ordering>`. `a0` and `b0` denote entry snapshots. `E = EqSpec::equiv`, `C = OrdSpec::compare`, and `L(x,y)` means `C(x,y) != Greater`. Every checked contract holds on normal return. These abbreviations do not introduce new source-language syntax.

These are a proposed signature inventory, not compilable source or settled
standard-library interfaces. In particular, admissibility premises and required
`Snapshot` bounds must be written explicitly when the signatures become source:
a quantified `Snap<T>` is not automatically valid. The one-pager's schematic
biconditional is written `iff` below, following the current RFC 0002.

| Trait and parents | Required members | Provided Rust implementation |
| --- | --- | --- |
| `EqSpec` | `logic fn equiv(a:S, b:S) -> Bool` | None |
| `EqChecked: EqSpec` | `fn eq_checked(&self, rhs:&Self) -> (r:bool, @(r === E(a0,b0)))` | `PartialEq`; `eq` delegates, `ne` uses its verified default |
| `LawfulEq: EqChecked` | `logic fn refl(a:S) -> @(E(a,a))`; `logic fn sym(a:S,b:S) -> @(E(a,b) ==> E(b,a))`; `logic fn trans(a:S,b:S,c:S) -> @(E(a,b) && E(b,c) ==> E(a,c))` | `Eq` via `impl Eq for Self {}` |
| `CloneEq: Snapshot` | `fn clone_eq(&self) -> (r:Self, @(snap(r) === a0))` | `Clone`; `clone` delegates, `clone_from` uses its verified default |
| `OrdSpec: EqSpec` | `logic fn compare(a:S,b:S) -> O` | None |
| `OrdChecked: OrdSpec` | `fn cmp_checked(&self,rhs:&Self) -> (r:Ordering, @(snap(r) === C(a0,b0)))` | None until order laws are established |
| `TotalOrd: OrdChecked + LawfulEq` | `logic fn equal(a:S,b:S) -> @(iff(C(a,b) === Equal, E(a,b)))`; `logic fn reverse(a:S,b:S) -> @(C(a,b) === reverse_order(C(b,a)))`; `logic fn le_trans(a:S,b:S,c:S) -> @(L(a,b) && L(b,c) ==> L(a,c))` | `Ord` and `PartialOrd`; `partial_cmp` returns `Some(cmp_checked(...).0)` |
| `HashSpec: Snapshot` | `logic fn trace(a:S) -> Seq<HashEvent>` | None; no equality bound |
| `HashChecked: HashSpec` | `fn hash_checked<H: Hasher>(&self,sink:&mut HashSink<'_,H>) -> @(final!(sink).events === sink.events.concat(Self::trace(a0)))` | `Hash`; the generated body delegates to this checked emitter without strengthening `H: Hasher` |
| `HashEq: HashChecked + EqChecked` | `logic fn respects_eq(a:S,b:S) -> @(E(a,b) ==> Self::trace(a) === Self::trace(b))` | No additional implementation; inherits the checked hash and equality bridges |
| `HasherSpec: Hasher + Snapshot` (designated implementations initially) | `type State: Logical`; `logic fn state(h:S) -> State`; `logic fn step(s:State,e:HashEvent) -> State`; `logic fn finish(s:State) -> Snap<u64>` | None; interprets every admitted event, including whole library operations |
| `BuildHasherChecked: BuildHasher + Snapshot` (designated implementations initially) | Logical `init` and `digest`, with the construction/interpretation guarantees below for its associated hasher | No replacement of an existing standard builder; its adapter names that implementation |

`HashEq` uses equality of logical traces, independent of the chosen hasher. A
lawful hasher interprets equal traces from equivalent initial states to equal
digests. Equality, order and hash laws reference the same selected `EqSpec`,
which may ignore representation details. `CloneEq` instead preserves whole
snapshots under RFC 0008; it implies neither deep-copy independence nor
equivalence of hidden executable behavior.

**Independent adoption.** `EqSpec` needs neither executable equality nor laws.
`EqChecked` supplies `PartialEq` without proving reflexivity; `LawfulEq` adds
those proofs and Rust `Eq`. `CloneEq` need not require hashing or ordering.
`HashChecked` needs no equality; `HashEq` needs no `LawfulEq`—an always-false
equality makes compatibility vacuous. Verified hash collections require both
`HashEq` and `LawfulEq`, with a designated lawful builder in the first stage.

### Adopted I04: traces and the standard Hash bridge

The owner adopted this architecture on 2026-11-25 after successive reviews and
nine Rust probes. The signatures remain a schematic library inventory until
the provider and library interfaces become source. The decision does not add
new language syntax, make runtime bodies unfold in logic, or give ordinary
proof-dropping calls facts that `contracts.calls.unrelated` does not provide.

#### The key emits a script; the hasher interprets it

`HashEvent` is a logical type. A trace is a finite sequence of logical operations
in order. Primitive events distinguish every exposed write method: `Bytes`,
the unsigned and signed integer writes of each width, and `Usize`/`Isize`.
`Bytes` preserves both contents and the boundary of each call. Therefore
`[U64(n)]` is not `[Bytes(encoding(n))]`, and two byte writes need not equal one
write of their concatenation. Payload validity includes byte/integer ranges;
target-dependent widths are interpreted for the checked target. No invalid
hypothetical `Snap<u8>` is assumed to be a byte merely because it is a `Nat`.

Events also name designated whole library operations. `StdText(bytes)` means
the standard text-hashing operation on the UTF-8 contents, not a promise about
the particular bytes or helper methods that the standard library emits.
`String` and `str` record this same event. A later `Box<str>` adapter must use it
too. Distinct enum constructors are never equated to express equal effects.
This keeps the law as literal equality of traces; no quotient or separate
semantic-equivalence relation on scripts is introduced.

The library supplies a private-representation `HashSink<'a,H>` containing a
borrow of `H` behind `Opaque` and a ghost event sequence. Its logical event
observer is available to contracts; its storage and mutation are private. It
does not implement `Hasher`, exposes no `finish` or raw-hasher accessor, and
has no public constructor. Each public write method forwards the corresponding
runtime call and, on normal return, appends its event. Checked key methods
compose through other checked emitters, not by passing the sink to arbitrary
standard `Hash::hash` methods.

The bridge is, schematically (helper spelling is illustrative):

```rust
// Defined in Locus's library, where the private constructor is accessible.
pub fn hash_bridge<K: HashChecked + ?Sized, H: Hasher>(key: &K, state: &mut H) {
    let mut sink = HashSink::new(state);
    key.hash_checked(&mut sink); // proof result is erased
}

// Supplied by HashChecked in the implementing crate.
impl Hash for Self {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_bridge(self, state);
    }
}
```

The proof-free helper creates no publicly obtainable sink. Defining it inside
the library avoids assuming that a generated implementation in another crate
can bypass Rust privacy to call a private constructor. It is the fixed checked
helper behind the provider, not an unchecked replacement point. Exact erased
and exported signatures remain part of the provider's lowering work.

The original unrestricted `H: Hasher` is retained. The checked method proves
which operations it emitted, not that an arbitrary `H` is deterministic or
lawful. The generated standard method calls the checked method once. Coherence
rejects an additional handwritten or derived `Hash` implementation; the verified
delegation, not coherence alone, supplies the correspondence. A user's existing
implementation must migrate to this route or use a local newtype. An empty
marker implementation cannot certify an unrelated old body.

#### Direct standard adapters and shared events

First-stage designated key adapters cover the machine integers, `bool`, `char`,
`str`, and `String`. Each adapter names the exact standard implementation it
stands beside. It calls that implementation directly on the sink's underlying
hasher through a library-private operation, then records its logical event on
normal return. It does not call standard `Hash` with the sink as the hasher and
does not re-encode the operation through the sink's write methods.

Thus `String` and `str` both append `StdText(contents)`, while their actual
standard methods still execute. An override of `write_str` on the underlying
hasher is preserved. The standard library may use its unstable helpers without
making those helpers part of the sink's public API. A scalar adapter may use
the matching primitive event only with a correspondence fact for that exact
operation; equality compatibility is separately required. The catalog of
designations and facts is recorded for each supported toolchain and target.

An event depends on the key snapshot. Its effect depends on that event and the
incoming state of a fixed designated hasher. In particular, a macro event such
as `StdText` needs its own interpretation bridge: ordinary `write` contracts
alone do not prove what the delegated standard operation does. The format may
remain uninterpreted, but its correspondence and equality laws may not be
omitted. Standard adapters are library-only exceptions to generation, as in
RFC 0006's `traits.laws.adapters`.

#### Scoped checked hashing and builders

First-stage verified tables use designated hashers and builders only. Their
state observations, transition function `step`, and `finish` may remain
uninterpreted. For admissible states and events, each designated operation
must implement those functions on normal return. A builder's abstract snapshot
must retain enough information to identify its hashing behavior; it is not
`Unit`. An opaque observation of a seeded builder need not reveal its seed.

Repeated construction from the same builder observation produces equivalent
initial states. Equivalence must be preserved by all admitted events and imply
equal `finish` results. It need not mean equality of every internal field. The
logical state can represent that equivalence, or its interpretation can supply
the corresponding congruence facts. With `run` the ordered fold of `step`:

```text
digest(builder_snapshot, trace) =
    finish(run(init(builder_snapshot), trace))
```

The library's scoped entry point is schematically:

```rust
fn hash_one_checked<K: HashChecked + ?Sized, B: BuildHasherChecked>(b: &B, k: &K)
    -> (h: u64, @(snap(h) === B::digest(snap(b), K::trace(snap(k)))));
```

It creates the hasher and sink for one key, invokes the checked emitter, and
returns the finished digest with its proof. Both temporary objects are disposed
of on normal return or unwinding; there is no externally reusable sink. Equal
key traces therefore give equal digests with the same lawful builder. A
`HashEq` bound alone supplies no collision-resistance, injectivity, prefix-free
encoding, termination, or law for an arbitrary Rust hasher.

A wrapped standard map internally uses its own standard hashing entry points.
Its trusted wrapper contract must connect those actual paths to this same
digest; it cannot pretend its private implementation calls `hash_one_checked`.
In particular, if it invokes `BuildHasher::hash_one`, the selected override or
default must have the corresponding fact. Code implementing verified tables
uses `hash_one_checked`; ordinary Rust and trusted standard-map internals are
not rewritten. Concrete designated hasher/builder pairs and their adapters
must be enumerated before the first library implementation is certified.

#### Failure, stability, and evidence

After a sink operation unwinds, its old event/state correspondence may not be
used to recover and return a successful hashing certificate. An underlying
hasher can mutate before panicking, before a ghost event would be appended.
The first stage supports no certified recovery and reuse of that failed sink.
This restriction must survive future closure and `catch_unwind` contracts;
today's inability to verify a closure is not its semantic justification.

`hash_one_checked` owns both objects. The generated `Hash::hash` owns only its
sink and borrows a caller-owned hasher: a Rust caller may catch its panic and
retain a partly written hasher, but that failed call exports no successful
certificate. No no-unwind assumption, poisoning mechanism, or new abort policy
is adopted here. This is separate from I01's acquisition-call abort rule.

Snapshot framing and a checked trace depending only on that snapshot establish
stable successful emissions. A method may read interior state only if it still
proves the same promised trace on every normal return. Stable equality,
correct adapters, key mutation policy, and lawful builder behavior remain
separate obligations. Erasing the event log does not by itself prove that the
wrapper has zero code-generation cost; that claim requires measurement.

#### Bridge generation versus structural derivation

The provider always generates `Hash::hash` by calling the given
`hash_checked`. It does not dictate that method's trace. An independently
written checked emitter may, for example, ignore a cache field, with `HashEq`
proved for its selected equality.

An optional structural derive generates the checked emitter and `trace` by
concatenating the fields' checked traces in declaration order. For enums it
first emits `Usize(i)`, where `i` is the zero-based declaration-order index of
the active variant in that configuration, then that variant's field traces.
This is Locus's convention, not a claim of exact compatibility with Rust's
derived hashing or explicit Rust discriminant values. Hashes have no promised
cross-version or cross-target stability.

When equality and hashing are both derived structurally, deriving `HashEq`
requires the corresponding `HashEq` evidence for every compared/hashed field
and uses those fields' same selected equality. `HashChecked` alone is
insufficient. No separate runtime hash body is generated by a `HashEq` derive.
Exact derive spellings and generated generic bounds belong to the source
interface/lowering work, not another choice of hashing semantics.

#### Borrow compatibility and later stages

The obligation for heterogeneous lookup is recorded now; its public API comes
later. For the actual `Borrow<Q>` implementation, prove its result is the
specified projection, that key equality agrees with borrowed-value equality,
and that the key and query use equal traces on the table's actual hashing
paths. Fieldwise hashing of `Name(String)` gives the contained field's trace,
not a certificate for an arbitrary `Borrow<str>` body. A consistently derived
projection and equality may discharge the obligation automatically. The first
acceptance case is `String`/`str`, with the shared `StdText` event.

Later stages add standard containers and tuples, with their adapter contracts,
and a separate `slice_trace` or batch-hashing contract. Numeric
`Hash::hash_slice` implementations write a whole byte slice, so standard slice
hashing is not generally a length prefix followed by each element's trace.
A generated type retaining the default `hash_slice` uses per-element
concatenation; standard overrides need their own certificate. `Vec<T>`/`[T]`
is a required later borrowed-lookup case. Tuples already have standard `Hash`
implementations and use adapters, not competing generated implementations.

User-proved hashers are also later. A future interpretation of `StdText` may
use a versioned fact about the actual inherited `write_str` default, but cannot
assume every resolved implementation uses it merely because stable user source
cannot name an unstable override. The resolved implementation and all forwarded
helpers must be covered.

#### Required trust inventory

These are named applications of existing trust categories, not a blanket law
for every standard type. Each result must identify its actual designated
implementations, snapshot observers, bounds, and adapter facts. Their concrete
library registrations and source contracts are an implementation gate.

| Fact | Exact obligation | Existing category |
| --- | --- | --- |
| Scalar snapshot validity | Captures of each designated machine integer, `bool`, or `char` have the source value and its range/Unicode validity | Existing scalar semantics; any external designation is `trusted.library-specs` |
| Text observation | The supported `str` and designated `String` observation used by `trace` exposes their actual UTF-8 contents and obeys the stated footprint; this supplies no general contents snapshot for standard containers | `trusted.library-specs` |
| Standard leaf emission | Each designated leaf's selected standard `Hash` body performs the logical operation its checked adapter records on normal return | `trusted.library-laws` |
| Equality compatibility | Each designated leaf's selected standard equality is connected to the same `EqSpec`, and equivalent admissible snapshots give equal events | `trusted.library-laws` |
| Shared text operation | `String` and `str` adapters implement the common `StdText` operation; the designated standard borrow projection and equality agree when used for lookup | `trusted.library-laws` |
| Hasher interpretation | For each designated hasher, each admitted primitive or library event transforms its incoming logical state by `step`, and actual `finish` returns the specified digest, on normal return | `trusted.library-laws`; explicit assumed wrapper contracts also remain listed under `assumed.stated` |
| Builder observation | Each designated builder has an abstract snapshot sufficient to identify its behavior, with the supported capture/frame laws; different seeded builders are not collapsed to `Unit` | `trusted.library-specs` |
| Builder construction | The selected `build_hasher` yields equivalent initial states for the same builder observation, and that equivalence is preserved by admitted operations and `finish` | `trusted.library-laws` |
| Standard table route | Every helper the trusted standard table actually invokes, including a selected `hash_one` override/default, agrees with the certified digest route | `trusted.library-laws`; a trusted wrapper's own assumptions remain `assumed.stated` |

`trusted.library-specs` covers observations, not an implicit postcondition on an
arbitrary runtime call. Any runtime assumption in a wrapper is explicit in its
contract and reported under the existing assumption mechanism. No new trust
entry, arbitrary user adapter, trusted no-unwind guarantee, or hidden global
determinism axiom is introduced by this design record.

#### Evidence and remaining implementation gates

[The hashing spike](../../spikes/hash-traces/README.md) publishes seven original
adversarial Rust probes and two direct-delegation probes. They demonstrate
protocol divergence through defaults, borrowed lookup failure and repair,
write-boundary sensitivity, `finish` exposure, partial mutation on unwind, and
the need to cover `hash_one` and `hash_slice` overrides. All nine pass on the
recorded nightly. They do not compile or kernel-check any Locus program.

Before a library release: settle concrete API/derive spellings and validity
premises, enumerate supported implementations and their trust registrations,
check the sink's private-interface and failure restrictions through lowering
and export, and discharge the library's proof obligations with the real kernel.
The adopted architecture is not evidence that those tasks have already passed.

### Remaining boundaries

**Remaining boundaries.** Non-Self headers can mention the outer type, but inner
signatures cannot currently name it after `Self` is rebound; reverse conversions
needing that name require a later decision. I04 resolves the unrestricted hash
signature with a trace emitter; it does not resolve general call-fact transport,
all default-method contracts, export or cross-crate lowering. Effects, key
stability while stored, permissions and termination remain separate obligations.
Borrowed lookup, iterator, projection and conversion APIs follow this core.

## Rules

None added or changed in this draft. I04 is an owner-adopted architecture with
schematic interfaces and Rust-only probes. Source-level rules and accept/reject
litmus cases accompany the completed provider and library interface proposal;
no probe is represented as a checked Locus program. Candidate
subjects are provided-implementation instantiation, `Self` binding, required
bridge validation, exported bounds, coherence failures, and cross-crate
implementation identity.

## Interactions

| Area | Effect |
| --- | --- |
| Types | Instantiates nested implementation templates and exports their bounds. Non-`Self` targets, conditional bounds, logical members, snapshot validity and `Snapshot` requirements need elaboration. |
| Propositions and proofs | Law traits refer to shared specification identities. Transport of facts through a result that omits the proof remains open against `contracts.calls.unrelated`. |
| Ownership and mutation | rustc checks generated receivers, arguments and loans. Delegation must preserve declared effects and final-borrow guarantees; returned-reference examples need a spike. |
| Erasure and runtime behavior | Generated bodies perform the checked operation and project its runtime result. Proof erasure does not authorize calling the operation twice or changing trait selection between programs. |
| Rust interoperability | Supplied implementations must satisfy ordinary orphan and overlap rules. Existing foreign implementations cannot be replaced. I04 preserves unrestricted `Hash::hash<H: Hasher>` through a write-only sink and directly delegated designated adapters. |
| Diagnostics | Errors must show the enclosing checked implementation, the instantiated target, and the conflicting implementation or unsatisfied bound. |
| Trust model | The I04 inventory names each adapter, observation, interpretation and builder obligation under existing categories. Concrete designations are not a blanket certificate. General implementation identity, generated-body verification and cross-crate enforcement still need completion. Ordinary Rust cannot forge bridge evidence by merely implementing a marker. |

## Alternatives

- **Handwritten forwarding implementations:** already legal Rust where coherence
  permits, but do not alone make the bridge a required, verified property of a
  checked-trait bound.
- **Ordinary blanket implementations:** preferable when legal and non-overlapping;
  a foreign target trait on an unrestricted type parameter is not a replacement
  for per-implementation generation.
- **A separate `adapter` or `provides` keyword:** expresses the mechanism, but the
  owner prefers nested `impl Trait for Type` syntax.
- **Special cases for standard traits:** less general than the same checked
  mechanism for user traits.
- **Generate only when the target implementation is absent:** would silently
  retain an unrelated implementation and cannot establish the required bridge.
- **Verified overrides of the supplied body:** potentially useful, but require
  a separately expressible target contract and verification of the selected
  alternate body; deferred.

## Prior art

- Rust's [`Into` blanket implementation](https://doc.rust-lang.org/std/convert/trait.Into.html)
  supplies conversion through `From` using ordinary trait machinery.
- Rust's [implementation coherence rules](https://doc.rust-lang.org/reference/items/implementations.html#trait-implementation-coherence)
  remain authoritative for the generated implementations.
- Derive and procedural macros already generate concrete Rust implementations;
  the proposed Locus addition is the required verified relationship and its
  availability to logical checking.

## Unresolved questions

Tracked in [provided-trait-implementations](../../issues/provided-trait-implementations.md):

1. How does a bridge expose its fact at an ordinary call without silently
   bypassing the current proof-in-result rule?
2. How are the outer implementing type and its associated types named inside a
   non-`Self` body under the owner's scoping rule, when needed?
3. Which non-`Self`, conditional, higher-ranked and cross-crate bounds can the
   lowering express directly for rustc?
4. The hash architecture is decided by I04 above. Its remaining implementation
   gates are concrete designated types, source interfaces and validity premises,
   the private sink/failure boundary, and checked/exported bridge enforcement.
5. What explicit validity premises, `Snapshot` bounds, frames, default-method
   contracts, and export restrictions make the signature inventory precise?
6. Should later versions accept explicitly verified alternate target
   implementations, and what evidence must their callers receive?

The I04 section has had the recorded adversarial review and Rust-only spike.
The rest of this RFC has not thereby completed review, and no Locus compiler or
kernel-check result is claimed.
