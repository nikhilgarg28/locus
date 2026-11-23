# RFC 0007: Provided trait implementations and core law traits

- **Status:** draft
- **Review:** unreviewed; recorded for later critique at the owner's request
- **Issue:** [provided-trait-implementations](../../issues/provided-trait-implementations.md)
- **Supersedes:** none

## Summary

**Draft proposal.** Checked traits may contain nested `impl` blocks that supply ordinary trait implementations. These are per-implementation templates, not blanket impls. Locus generates and verifies the bodies; rustc retains ownership, overlap and trait-selection rules. The initial library connects equality, cloning and ordering to Rust traits, and specifies checked hashing separately.

This is a design record, not a proposed acceptance decision. It preserves the
owner's chosen nested `impl` syntax and `Self` scoping. No spec rule, decided
litmus outcome, or trust entry changes in this PR. RFC 0006 is reserved for the
traits draft being prepared separately; this draft depends on that work and on
the current RFCs 0001 to 0005. The source is the
[design one-pager](https://chatgpt.com/space/page_ec1c67b2ba4c8191b70aa8e87d62fc07).

**Interaction requiring review:** the intended availability of a bridge's fact
at an ordinary call is not supplied by current
`contracts.calls.unrelated`: that rule permits only proofs in the result, plus
the separately defined observations of runtime operations. Generating a wrapper
that drops a proof does not, by itself, give that proof to a caller. How a bridge
exports a checked call fact is an unresolved part of this draft, not an implicit
amendment of the existing rule. The trait signatures also depend on the pending
design for logical members and `Model` bounds on traits.

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

Signatures below are schematic. `S` means an admissible frozen snapshot of the implementing type; `B = Seq<Snap<u8>>`; `O = Snap<Ordering>`. `a0`, `b0` and `h0` denote entry snapshots or hasher states; `h1` is the post-state. `E = EqSpec::equiv`, `C = OrdSpec::compare`, and `L(x,y)` means `C(x,y) != Greater`. Every checked contract holds on normal return. These abbreviations do not introduce new source-language syntax.

These are a proposed signature inventory, not compilable source or settled
standard-library interfaces. In particular, admissibility premises and required
`Model` bounds must be written explicitly when the signatures become source:
a quantified `Snap<T>` is not automatically valid. The one-pager's schematic
biconditional is written `iff` below, following the current RFC 0002.

| Trait and parents | Required members | Provided Rust implementation |
| --- | --- | --- |
| `EqSpec` | `logic fn equiv(a:S, b:S) -> Bool` | None |
| `EqChecked: EqSpec` | `fn eq_checked(&self, rhs:&Self) -> (r:bool, @(r === E(a0,b0)))` | `PartialEq`; `eq` delegates, `ne` uses its verified default |
| `LawfulEq: EqChecked` | `logic fn refl(a:S) -> @(E(a,a))`; `logic fn sym(a:S,b:S) -> @(E(a,b) ==> E(b,a))`; `logic fn trans(a:S,b:S,c:S) -> @(E(a,b) && E(b,c) ==> E(a,c))` | `Eq` via `impl Eq for Self {}` |
| `CloneEq: EqSpec` | `fn clone_eq(&self) -> (r:Self, @(E(snap(r),a0)))` | `Clone`; `clone` delegates, `clone_from` uses its verified default |
| `OrdSpec: EqSpec` | `logic fn compare(a:S,b:S) -> O` | None |
| `OrdChecked: OrdSpec` | `fn cmp_checked(&self,rhs:&Self) -> (r:Ordering, @(snap(r) === C(a0,b0)))` | None until order laws are established |
| `TotalOrd: OrdChecked + LawfulEq` | `logic fn equal(a:S,b:S) -> @(iff(C(a,b) === Equal, E(a,b)))`; `logic fn reverse(a:S,b:S) -> @(C(a,b) === reverse_order(C(b,a)))`; `logic fn le_trans(a:S,b:S,c:S) -> @(L(a,b) && L(b,c) ==> L(a,c))` | `Ord` and `PartialOrd`; `partial_cmp` returns `Some(cmp_checked(...).0)` |
| `HasherSpec` | `type State: Logical`; `logic fn state(h:S) -> State`; `logic fn write(s:State, bytes:B) -> State`; `logic fn finish(s:State) -> Snap<u64>` | None |
| `HasherChecked: HasherSpec` | `fn write_checked(&mut self,bytes:&[u8]) -> @(h1 === write(h0,bytes0))`; `fn finish_checked(&self) -> (r:u64, @(snap(r) === finish(h0)))` | `Hasher`; `write` and `finish` delegate, other methods retain verified defaults |
| `HashSpec<H: HasherSpec>` | `logic fn feed(a:S,s:H::State) -> H::State` | None |
| `HashChecked<H: HasherChecked>: HashSpec<H>` | `fn hash_checked(&self,h:&mut H) -> @(h1 === feed(a0,h0))` | Standard `Hash` bridge remains open |
| `HashEq<H: HasherChecked>: EqChecked + HashChecked<H>` | `logic fn respects_eq(a:S,b:S,s:H::State) -> @(E(a,b) ==> feed(a,s) === feed(b,s))` | None |

`HashEq` uses equality of resulting logical hasher states, a stronger composable guarantee than equal final digests. `HasherChecked` operations must depend only on their specified state and input, with the declared state-preservation guarantees. All equality, clone, order and hash laws reference the same selected `EqSpec`; equality may ignore representation details. `CloneEq` preserves that equivalence, not deep-copy independence.

**Independent adoption.** `EqSpec` needs neither executable equality nor laws. `EqChecked` supplies `PartialEq` without proving reflexivity; `LawfulEq` adds those proofs and Rust `Eq`. `CloneEq` need not require hashing or ordering. `HashChecked<H>` needs no equality; `HashEq<H>` needs no `LawfulEq`—an always-false equality makes compatibility vacuous. Verified hash collections require both, with the chosen checked hasher.

### Remaining boundaries

**Remaining boundaries.** Non-Self headers can mention the outer type, but inner signatures cannot currently name it after `Self` is rebound; reverse conversions needing that name require a later decision. Rust `Hash::hash<H: Hasher>` accepts any hasher: a method requiring `H: HasherChecked` cannot directly implement it. Start with checked hashing; a standard `Hash` bridge needs a protocol that preserves the original generic signature. Effects, key stability while stored, permissions, panic and termination remain separate obligations. Borrowing, iterator, projection and conversion law families follow this core.

## Rules

None added or changed in this unreviewed draft. Spec rules and accept/reject
litmus cases are deferred until the later review the owner requested. Candidate
subjects are provided-implementation instantiation, `Self` binding, required
bridge validation, exported bounds, coherence failures, and cross-crate
implementation identity.

## Interactions

| Area | Effect |
| --- | --- |
| Types | Instantiates nested implementation templates and exports their bounds. Non-`Self` targets, conditional bounds, logical members, snapshot validity and `Model` requirements need elaboration. |
| Propositions and proofs | Law traits refer to shared specification identities. Transport of facts through a result that omits the proof remains open against `contracts.calls.unrelated`. |
| Ownership and mutation | rustc checks generated receivers, arguments and loans. Delegation must preserve declared effects and final-borrow guarantees; returned-reference examples need a spike. |
| Erasure and runtime behavior | Generated bodies perform the checked operation and project its runtime result. Proof erasure does not authorize calling the operation twice or changing trait selection between programs. |
| Rust interoperability | Supplied implementations must satisfy ordinary orphan and overlap rules. Existing foreign implementations cannot be replaced. The unrestricted generic parameter of `Hash::hash` remains an explicit gap. |
| Diagnostics | Errors must show the enclosing checked implementation, the instantiated target, and the conflicting implementation or unsatisfied bound. |
| Trust model | No trust entry is changed. Implementation identity, generated-body verification, and cross-crate metadata enforcement require review before any extension is accepted. Ordinary Rust cannot forge bridge evidence by merely implementing a marker. |

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
4. How can standard `Hash::hash<H: Hasher>` retain its unrestricted signature
   while offering a checked protocol? A stronger `H: HasherChecked` bound cannot
   simply be added to the trait method.
5. What explicit validity premises, `Model` bounds, frames, default-method
   contracts, and export restrictions make the signature inventory precise?
6. Should later versions accept explicitly verified alternate target
   implementations, and what evidence must their callers receive?

No adversarial review or implementation spike is claimed for this draft.
