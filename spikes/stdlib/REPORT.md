> Repository import: Locus source modules now live in
> [studies/stdlib](../../studies/stdlib/README.md), with a crate/module layout and
> a future smoke-program entry point. Historical `locus/NN-name.lc` references
> below name the pre-import files; `studies/stdlib/import-manifest.json` maps them.
> The report and embedded source appendix are historical, unverified study evidence.

This spike attempts a Locus standard library with two storage strategies: collections built on permissioned memory, and wrappers around Rust storage with separately maintained logical contents. It includes logical sequence and map definitions, law traits, pointer contracts, Box and collection algorithms, interior-mutability and sharing interfaces, and interop clients.

**A native Locus implementation is necessary for this exercise.** Rust implementations can test algorithms, ownership, and failure scenarios, but cannot establish that Locus can express and compose the proofs. The primary deliverable is the annotated source in `locus/`; the executable Rust harness in `src/` is supporting evidence.

The result is promising for unique ownership and ordinary collection operations, but it is not a finished verified standard library. The strongest findings concern invariant-preserving mutable loans, the connection between native storage and erased models, trait-implementation bridges, and shared state. Those are more consequential than adding further surface syntax.

# What was built and what was established

There are 13 Locus source files, with actual logical definitions and runtime algorithm bodies where the required operations can be stated. The source appendix reproduces them. `OPEN` comments mark unresolved proof obligations or interfaces. A declaration ending in `;` in the permission or protocol files specifies a required primitive; it is not an implementation.

No Locus parser, typechecker, or proof kernel checked these files. In particular, `_` is an attempted ordinary proof hole, not an accepted theorem. Some current helper signatures are intentionally insufficient for their callers: those cases need interface changes as well as proofs. None of the Rust test counts below is a Locus acceptance count.

The executable harness contains custom permission storage, a native vector, an actual degree-two B-tree with insertion and deletion, an open-addressing hash table, and wrappers over the corresponding Rust collections. It also contains native and std-backed Box, Cell, RefCell, Rc, Arc, and Mutex implementations. The native shared types require additional unsafe algorithms; they are not consequences of the unique permission API alone.

| Component | Primary Locus source | Executable evidence and principal gap |
|---|---|---|
| Seq | Inductive `Nil`/`Cons`, `logic fn` operations, recursive lemma candidates | Finite algebra checks; Locus lemma elaboration and termination remain unchecked |
| Map | Lookup function, size, finite-support proof; insert/remove and proof candidates | Extensional runtime oracle; function extensionality and support/cardinality proofs remain |
| Permissioned memory | Affine Region, initialized/vacant slots, read/write/take/loan/free contracts | Custom allocation and capability checks; contracts and memory semantics remain primitive assumptions |
| Native Vec | Checked read, growth, push/pop, set, insert/remove using Region | Random traces pass; authored invariant blocks a straightforward escaping mutable loan |
| Wrapped Vec | Std Vec plus logical Seq; checked methods and conversions | Incremental shadow tests pass; native correspondence is trusted; forgettable restoration is insufficient |
| Native BTreeMap | Search, splitting, merging, rotations, insert/delete and representation predicate | Full runtime algorithm passes traces and removal permutations; helper contracts and structural proofs incomplete |
| Wrapped BTreeMap | Std BTreeMap plus logical Map | Runtime tests pass; exact std Ord bridge and storage correspondence required |
| Native HashMap | Probing, tombstones, cached hashes, growth, insert/delete; representation predicate and public invariant boundary | Collision and trace tests pass; probe invariants and helper proof interfaces incomplete |
| Wrapped HashMap | Std HashMap plus logical Map | Runtime tests pass; actual std Eq/Hash bridge required, including hasher behavior |
| Native Box | Library operations over initialized `Owned<T>` | Runtime ownership and drop tests; `Owned<T>` itself is an explicit primitive |
| Wrapped Box | Std Box plus logical payload snapshot | Body candidates and runtime round trips; raw/payload correspondence trusted |
| Cell | Native slot and std Cell over sealed certified values; external-capability alternative | Value-invariant pattern works operationally; last-write facts need an owned state protocol |
| RefCell | Guard API, native borrow-gate interface, std wrapper | Native dynamic-borrow implementation tested; Locus authority transfer and guard contracts unfinished |
| Rc and Arc | Share protocol and wrapper body candidates | Counted runtime ownership tested; Locus shared permissions unfinished, Arc additionally needs atomic reasoning |
| Mutex | Lock-gate interface and std wrapper | Native and std locking tested; Locus lock invariant and happens-before protocol unfinished |

The native collection modules use no unsafe code themselves. Their payload storage uses Region and owning pointers. Region's initialized-slot bookkeeping is a runtime Vec, and shared control blocks use Rust allocation. This is an ownership and specification experiment, not a zero-cost or no_std implementation.

# Logical sequences and maps

## Seq should be a defined logical datatype

The candidate is deliberately simple:

```rust
#[derive(Logical)]
pub enum Seq<A: Logical> { Nil, Cons(A, Seq<A>) }

pub logic fn len(self) -> Nat {
    match self { Seq::Nil => 0, Seq::Cons(_, tail) => 1 + tail.len() }
}
```

The source supplies lookup, append, concatenation, prefix/suffix, update, insertion, removal, mapping, membership, and uniqueness. Their meanings come from definitions, not trusted sequence axioms. This representation is erased; its linear-time definition of append is a proof-engineering concern rather than the runtime vector's complexity.

The immediate library workload is concrete: read-after-update, read-away-from-update, length preservation, append indexing, prefix/suffix decomposition, removal shifting, mapping, and extensional equality. A collection method should call these lemmas explicitly. A generic arithmetic hole does not invent induction or instantiate a quantified statement.

One elaboration question is worth testing early. A bounds-checked logical `at(s,i,p) -> A` must handle an impossible empty-sequence branch without manufacturing a default `A`. Direct proof-to-data elimination is restricted. The source includes a candidate using existing well-founded recursion: the empty branch's impossible descent obligation is discharged under the contradictory bound. This is a proposed checker test, not a demonstrated impossibility and not an accepted proof. Ordinary false elimination, if permitted in this case, would provide a clearer spelling. No choice axiom is needed just to index a nonempty finite sequence.

## Map equality must not expose storage order

A list of unique pairs is an easy implementation of finite maps, but structural equality of those lists depends on insertion order. That is the wrong `===` for a mathematical map.

The Locus candidate stores:

```rust
#[derive(Logical)]
pub struct Map<K: Logical, V: Logical> {
    lookup: logic fn(K) -> Maybe<V>,
    size: Nat,
    finite: @Finite(lookup, size),
}
```

`Finite` proves that a duplicate-free finite key sequence is exactly the function's support and has the stated size. Insert/remove update the function and prove a new support witness. The witness stays inside proofs; this construction does not bypass the rule forbidding extraction of proof witnesses into logical data.

The current contracts use explicit `equivalent`, meaning equal size and pointwise equal lookup. Turning that into built-in equality requires a checked function-extensionality theorem and the uniqueness of finite support cardinality. Those should be library/kernel-derived theorems, not new map axioms. If Locus intends maps to support enumerable keys as data, it needs an additional data representation or a deliberately chosen construction principle; a finiteness proof alone does not provide that enumeration under the current elimination rule.

## All key laws should use one projection

The collection model is `Map<K::Key, Snap<V>>`, with `K::key: Snap<K> -> K::Key`. This is more useful than requiring key equality to mean equality of the entire representation snapshot. A key may contain a display label or cached data that is irrelevant to its identity.

`LawfulEq`, `TotalOrd`, and `HashEq` all refer to the same `KeySpec` projection. The order is total and antisymmetric on projected keys. The checked hash is a function of that same key, so equal keys necessarily get equal codes. Different keys may collide.

The native tree needs `TotalOrd`; the native hash table needs `HashEq`. Neither algorithm needs Rust's `Ord`, `Eq`, or `Hash`, and a test uses a key with none of those native traits. The wrapper backends additionally need their actual native traits and a bridge to the projected identity.

A lookup map omits another observable fact: which representative key is retained. Inserting keys with raw values 107 and 7 under a modulo-100 identity replaces the value but retains the first stored key. The test exercises that distinction. `keys()`, `get_key_value()`, ordered iteration, and borrowed-key lookup need additional contracts. The minimal Map model should not silently promise those properties.

# Permissioned memory and unique ownership

## What the memory core must provide

The assumed snapshot of a Region describes initialized and vacant slots. A pointer by itself grants no authority. Region is an affine runtime resource: it cannot be copied merely because logical snapshots can be copied.

The interface includes allocation, capacity, initialized reads, destructive take, vacant writes, mutable loans, and deallocation. For example:

```rust
fn borrow_mut_checked(&mut self, i: usize, initialized: @Initialized(self, i))
    -> (x: &mut T,
        @(cells(self).get(i) === Some(Some(snap(x)))
          && cells(final!(self)) ===
             cells(self).update(i, Some(snap(final!(x))))));
```

This excerpt abbreviates the source's explicit nested Maybe types and predicates. The contract connects both the initial element and its final value to the owner's frozen observations. It is not implemented by calling a runtime snapshot function.

The assumption that snapshots contain memory contents is only one part of this interface. It also needs allocation provenance, initializedness, no duplicated ownership, borrow exclusion, destruction exactly once, and deallocation rules. The harness checks wrong-region accesses, uninitialized reads, overwriting live slots, use after moves, and simultaneous loans. Zero-sized allocations get distinct authority identifiers even when their machine addresses coincide. Such identifiers are stronger than the discussed unconstrained `mint` result.

Exception behavior matters. Region tracks which slots still own values and has cleanup after one destructor panics. Allocation failure and double-panic aborts remain possible. The Locus contracts describe normal returns; they do not establish termination, successful allocation, or panic freedom.

## Box is a useful first vertical slice

An `Owned<T>` primitive guarantees one initialized object. A native Box can delegate creation, shared read, mutable loan, and consuming extraction to that primitive. This avoids storing an authored proof field that itself prevents borrowing the payload mutably.

This is a small but real design choice: the initialization invariant lives in the primitive's resource semantics. Merely writing the same fact as an ordinary struct proof does not confer the same borrowing rules. The two should not be conflated when deciding how small the trusted core is.

The wrapped Box is straightforward for creation, read, whole-value replacement, and extraction, subject to an explicit native-storage correspondence contract. Its returned mutable loan has the same restoration question as wrapped Vec.

## Native Vec exposes the invariant and loan problem

The natural representation has storage, a runtime length, and a proof that precisely the prefix below length is initialized. The current invariant rules prohibit mutably borrowing a field read by that proof.

Whole-operation mutation has a workable pattern: `take_out` the vector, destructure it, update its owned storage, and reconstruct the vector with a new proof. The source uses this for growth and edits. A panic past the vacant slot aborts under the existing slot rule; this is not a rollback guarantee.

An escaping `get_mut` is harder. The slot cannot simply escape while the owner remains unconstructed. A final-value contract tells the caller what the end state should be; it does not by itself explain how the library temporarily opens its representation invariant and re-establishes it at the actual loan end.

The best next test is narrow: one initialized element loan, with an unchanged initializedness proof and a value-dependent snapshot update. Demonstrate it across reborrows, early last use, forgetting a wrapper guard, and unwinding. Possible implementations are an intrinsic initialized-prefix resource or a constrained invariant-opening protocol. A general facility for arbitrarily opening all proof fields would be a much larger change.

The native source also currently needs `T: Snapshot` on its invariant-bearing Vec declaration. The wrapped source can use the deferred logical-field carrier without putting that bound on the struct. Removing the native bound may require moving more representation state into a primitive or validating a conditional-proof-storage scheme; the spike does not claim the existing SnapOf carrier automatically solves proof fields.

## Whole snapshot equality differs between the backends

If Region's snapshot includes allocation identity, native Box/Vec snapshots include representation identity. A newly allocated clone can preserve every element and still have a different whole snapshot. Private fields do not make their contribution disappear from equality.

The wrapper's raw storage contributes Unit, so its logical contents can determine whole-snapshot equality. The current wrapper `CloneEq` rule relies on that fact. Applying the same law to a native pointer-backed representation needs a separate decision.

My preference is to specify collection cloning and algorithms primarily through their element or payload view. Keep whole-snapshot `CloneEq` for types where it is actually true. Another option is for the memory core's snapshot to omit allocation identity and keep authority tracking outside that snapshot, but this must be an intentional memory-core design; hiding fields or making IDs uninspectable is insufficient. The Rust `Observe` oracle reports contents and does not resolve this snapshot decision.

# Wrapping Rust storage

## A ghost field is an observation but not a correspondence proof

The wrapper layout is useful and matches the snapshot-first direction:

```rust
pub struct Vec<T> {
    raw: Opaque<std::vec::Vec<T>>,
    contents: Seq<Snap<T>>,
}
```

Updating `contents` proves facts about that logical sequence. It does not prove that `raw` holds matching objects. Since `Opaque` snapshots to Unit, no logic function of that snapshot can recover the hidden vector contents.

The baseline already names a trusted library boundary for this correspondence. The source makes that dependency visible with `unsafe assume!` at native reads and returned-value relations. These are not new silently proved facts. A smaller explicitly specified adapter around native operations is preferable to scattered assumptions in a production library. Proving the adapter against std internals is a different project from proving the wrapper's ghost updates.

This does not mean Rust std interoperability is lost. Native code can move and store sealed valid values, and checked wrapper operations can expose useful contracts. It means that proofs of native collection behavior have to come from the named adapter contract, not from the mere existence of an erased field.

## Conversion has both a fact cost and an execution cost

The candidate `from_std` traverses initialized elements and captures each one, producing a sequence in order. That establishes a plausible implementation route with trusted native access, but the traversal itself is runtime code in the current source. Erasing the ghost sequence does not automatically erase the traversal.

The Rust wrapper similarly rebuilds its audit model by traversal. With audit disabled, instrumentation is skipped. That switch is manually programmed; it is not evidence that Locus's lowerer performs the same erasure.

`into_std` transfers ownership. A later `from_std` establishes a new observation. Without a native operation contract, callers get no equation linking the new observation to the old one. The interop sort/dedup example therefore executes those operations but does not claim a Locus sortedness or deduplication theorem.

Native Vec conversions in the harness move elements between allocations. They support non-cloneable values but are linear-time, not zero-copy. Zero-copy native interchange requires compatible allocation layout, capacity, allocator, provenance, and destruction conventions. It should be a separate unsafe adapter, not a general inference from having the same element type.

## Drop alone cannot restore an always available model

The executable counterexample is small: obtain a mutation guard, mutate the native vector, then `mem::forget` the guard. Rust's borrow can end, but the destructor that was supposed to refresh the logical contents does not run. The old model becomes observable again unless the API takes another precaution.

The harness contains both this deliberately broken design and a fail-closed version. The latter invalidates the shadow before giving access; forgetting the guard leaves observation unavailable. That is safe for an optional audit model, but it does not implement the specified Locus Vec with an always-available contents snapshot.

The baseline's `collections.vec.element-loan` already states the right requirement: restoration at the actual loan end, not only in a forgettable destructor. The spike supplies an executable witness for why that requirement is necessary. The remaining work is implementing that protocol and validating its interaction with representation invariants.

A closed callback can restore after normal return and caught unwind, provided no borrowed reference escapes. The harness checks both. Callback-based access is a useful temporary API for some operations, but it is not a drop-in substitute for arbitrary native mutable references, iterators, or asynchronous callbacks.

# The two map implementations

## BTreeMap can avoid multiple mutable loans

The native tree uses minimum degree two. It includes full-node splitting, sibling borrowing, merging, predecessor/successor replacement, root growth, and root collapse. Its Locus candidate temporarily owns a child, edits it, and puts it back; it does not require a general two-disjoint-loan facility just to implement these operations.

This approach avoids Clone bounds on keys and values. It can also simplify proof ownership. It costs moves and explicit reconstruction, and it does not make the proofs short. The representation predicate states occupancy, sorted node keys, parent/child key intervals, equal leaf height, unique projected keys, and total entry count.

The current internal Locus helper signatures lose some of the facts needed by their callers. For example, returning a child as an ordinary value discards the relationship to the removed entry, and a plain `find` result supplies no search theorem. These are unfinished proof interfaces, not evidence that runtime function bodies may be unfolded by callers. The public holes cannot be declared solved until the helpers export those relations.

## HashMap needs a probe invariant as well as HashEq

The native table uses linear probing, cached hashes, tombstones, and growth. Rehashing moves entries using cached hashes, avoiding user hash/equality calls during that move. Equal hashes are not treated as equal keys.

The model omits table placement, but lookup correctness depends on placement. A stored key must be reachable from its hash-derived start without crossing an Empty slot; Deleted slots do not stop a search. The representation also needs counts, load bounds, unique projected keys, and consistency between stored hashes and key identity.

The source states these predicates and separates the open Table from the invariant-bearing public HashMap. Its private helper signatures still need to accept and return those representation facts. A final hole cannot recover them from the abstract Map alone.

A fixed checked hash is sufficient for this experiment. Randomized hashing needs a seed or hasher-state parameter that remains fixed for the table, plus laws connecting equal keys under the same state. The std wrapper's generic Hasher protocol is stronger than the experiment's fixed-u64 API.

## Law traits do not automatically bind native implementations

One negative test implements a lawful checked comparison on modulo-100 keys while implementing Rust Ord on the raw number. The native tree and std BTreeMap then disagree about whether 107 and 7 denote one key or two. Both trait implementations compile in Rust; merely requiring both traits does not relate them.

A second test deliberately gives equal keys unequal checked hashes, demonstrating why the native hash law matters. In actual Locus that implementation must fail to supply its proof. The runtime trait alone cannot enforce it.

The source seals its provisional std bridge certificates and whitelists u32. An unrestricted empty marker would be unsound as proof of the bridge. For general types, the generated forwarding-impl design discussed earlier is a good route: bind the certificate to the actual resolved native implementation, respect coherence, and ensure a different handwritten implementation cannot inherit the certificate. Existing native implementations need a separate explicit certification mechanism or an audited whitelist.

Hash bridging must cover the actual equality and bytes/operations delivered to a common hasher state. Proving an unrelated `hash_checked() -> u64` result is not enough to certify arbitrary `std::hash::Hash` behavior.

# Interior mutability and shared ownership

## Stable root snapshots do not describe changing shared contents

A Cell, RefCell, or Mutex can change behind a shared reference. With the proposed UnsafeCell barrier, the root snapshot does not reveal that mutable payload. Root equality before and after a write is therefore not evidence that the payload is unchanged.

Maintaining an ordinary immutable logical `last_value` next to a shared cell is not a solution: aliases can mutate the payload without updating the particular ghost field being read. Putting the logical field inside UnsafeCell hides it along with the payload.

There are two useful APIs, with different promises:

1. **Sealed valid values.** Store a record containing a value and its proof. Every value read or replaced satisfies that invariant. This works well through native Cell, RefCell, iterators, and Mutex. The interop examples return proofs that a stored number remains below 100.
2. **An owned state capability or active guard.** The capability tracks the current value and must participate in every state-changing operation. It can prove that a read returns the last written value. A plain std Cell alias cannot be permitted to bypass it.

The first is cheap and useful immediately. It proves a property of returned values, not a temporal relationship between calls. The second needs an explicit protocol. The Locus source shows both interfaces rather than pretending they are interchangeable.

## RefCell and Mutex guards transfer authority

The runtime RefCell supports multiple shared guards or one exclusive guard. Forgetting a mutable guard leaves the dynamic borrow outstanding; later mutation is refused. This is different from forgetting a Vec refresh guard, after which Rust can allow the owner to be used again.

A Mutex guard owns temporary authority to access protected storage. Releasing it must re-establish the invariant before another thread acquires it. The native harness uses Acquire/Release synchronization and records poisoning. Poison is a diagnostic state, not permission to expose false proof fields. Sealed valid values remain a useful baseline; temporarily opening an arbitrary invariant during a lock requires stronger exception and reentrancy rules.

Neither liveness nor fairness is proved. A forgotten mutex guard may keep the lock forever. Concurrent test success does not establish a memory-model theorem.

## Rc and Arc need more than unique Region

Unique Region permits one owner. Rc clone creates additional accounted owners, so it requires a shared ownership resource with a rule for the last release. Arc adds atomic reference-count reasoning and publication/synchronization requirements.

The runtime implementations have native counts and payload ownership, and the tests check unique access, clone/drop behavior, concurrent release, and destruction once. Their Locus files remain protocol interfaces and dependent wrapper bodies. They do not derive fractional or counted permissions from copying Nat identifiers.

The spike omits Weak, cyclic construction, make_mut, unsizing, allocator customization, and reference-count observation contracts. Payload equality alone does not imply allocation identity, equal strong counts, or independent copies. Those are separate observations with separate laws.

# API ergonomics and interoperability

The `_checked` convention works well for facts callers actually need:

- `len_checked` returns the runtime length and its equality to logical length.
- `get_checked` consumes an in-bounds proof and returns the element reference with an indexed equality.
- `push_checked` relates initial and final sequences.
- `pop_checked` relates both the returned value and final contents, including the empty case.
- Map insertion/removal relate the returned previous value as well as the updated map. A postcondition describing only the new map is incomplete for many clients.

Ordinary methods can delegate and discard the proof. The proof is erased; callers should not pay a runtime allocation for a proof tuple. The Rust harness uses zero-sized test receipts, which illustrate the API shape but are not Locus proofs.

Capacity need not be in the public contents model. It remains essential to the private allocation invariant and to any cost or no-reallocation contract. Add `capacity_checked` when clients need that observation instead of making every sequence theorem depend on capacity.

The examples show native and wrapped checked stacks, one generic map client, native sort/dedup handoff, conversion across tree/hash backends, and sealed proof records travelling through native iterators and interior-mutable wrappers. The generic Locus map client still needs backend trait implementations and the relevant lemmas before it becomes a checked program. The executable counterpart runs against all four map backends.

Ergonomic pain is concentrated in proof transport and internal helper contracts. Quantified facts must be instantiated, and mutations require explicit relations between old and new versions. There is a large payoff from a good Seq/Map lemma library and diagnostics showing which relationship a helper dropped. There is little evidence here that another capture operator or new proof punctuation would solve the main problems.

# Validation and its limits

The reproducible commands and logs are in `validation/`. The toolchain was rustc 1.100.0 on x86_64 Linux. All 32 Rust tests passed with audit models enabled, with audit models disabled, in an optimized audit build, and in an AddressSanitizer audit build. These are the same 32 tests across configurations, not 128 independent programs. Formatting passed. Nine Rust compile-fail probes rejected with the expected diagnostic categories.

| Evidence | Coverage |
|---|---|
| 11 collection tests | 36,000 vector trace iterations; 21,600 four-map trace iterations; all 720 removal orders of six B-tree keys; collisions, tombstones, non-Clone values, moves and drops |
| 11 protocol tests | Cell observation barrier, RefCell conflicts, forgotten guards, Rc/Arc ownership, mutex concurrency and poison, unwind resynchronization, observation failure, destructor cleanup |
| 8 interop tests | Common checked clients, native handoff, representative keys, backend conversions, incompatible Ord bridge, unlawful hash, forgotten Drain, stale index receipt |
| 2 logical-oracle tests | Small-domain sequence update laws and finite-map update/removal/extensionality laws |
| 9 compile-fail probes | Copied/moved permission use, conflicting loans, incorrect pointer type, forbidden cross-thread sharing, non-Send guard, escaped callback reference |

The concurrent mutex test performs 9,000 protected increments per backend. The Arc test exercises concurrent cloning and last-release destruction. These tests find implementation failures; they do not prove all interleavings.

AddressSanitizer reported no error in the tested executions. It does not establish Rust aliasing/provenance correctness, soundness of unsafe Send/Sync impls, or the Locus memory semantics. Miri and Lean were not run; clippy was unavailable in this environment. The Locus files received only a delimiter sanity check, not syntax acceptance or proof checking.

The Rust oracle uses finite machine values and executable models, sometimes with a different chosen lawful hash from the Locus u32 example. It is not a generated lowering, a bisimulation proof, or an exact execution model of infinite Nat. Disabling audit removes shadow instrumentation, not Region's runtime permission bookkeeping.

# Recommended next work

I would use this as an implementation workload now, with four small gates before broadening the library.

1. **Check Seq and the finite-map foundation.** Mechanically check induction, function extensionality, finite support, explicit rewriting, and the impossible indexing branch. Then prove one checked vector push/pop client. Keep every remaining hole visible.
2. **Resolve one mutable element loan end to end.** Use a native Vec and a wrapped Vec. Specify invariant opening, reborrows, actual loan end, unwinding, and forget. The existing final-value caller contract should be preserved; the missing library implementation protocol should be demonstrated.
3. **Implement one trusted std adapter and one certified trait bridge.** Start with std Vec and u32 BTreeMap. Make the trust report identify the native correspondence and actual resolved Ord implementation. Do not make an empty public marker the certificate.
4. **Demonstrate one interior-state capability.** Start with Cell or RefCell, then lift the protocol to Mutex only when its invariant and exception behavior are clear. Rc/Arc require a distinct shared-ownership protocol; do not assume they follow from unique pointers.

In parallel, complete the B-tree and hash-table helper proof interfaces. Those are substantial proof-development tasks, but the spike has not shown a need for new general-purpose language syntax to express their core search, splitting, merging, or probing algorithms.

The remaining owner-level choices are relatively focused: whether allocation identity belongs in structural snapshots; whether generic collection clone laws speak about snapshots or explicit views; how narrowly to support temporarily opened invariants; and how a library certifies behavior of an existing native trait implementation. Native-wrapper correspondence is already a named trust boundary in the baseline, while its concrete adapter and loan restoration still need implementation.

# Trust ledger and design references

| Boundary | Status in this spike |
|---|---|
| Pure Seq/Map definitions and lemmas | Intended checked library code; open proofs are not assumed |
| Region and Owned contracts | Assumed primitive interface, including the requested memory-content snapshot assumption |
| Native Vec/BTree/Hash algorithms | Intended checked library code over that interface; incomplete proofs and helper contracts |
| Std collection correspondence | Explicit trusted adapter obligations, consistent with the baseline's named library boundary |
| Std Ord/Eq/Hash certificates | Sealed u32 whitelist proposal; general certification unresolved |
| Rc/Arc/RefCell/Mutex protocols | Required interfaces, not implemented or proved in Locus |
| Rust unsafe implementations | Tested experimental code, not a verified memory core |

The reference checkout is [locus at c2818cd](https://github.com/nikhilgarg28/locus/tree/c2818cdbe407ef49deb3301221bc29b9cba65049). Relevant rules include [proof elimination and explicit proof construction](https://github.com/nikhilgarg28/locus/blob/c2818cdbe407ef49deb3301221bc29b9cba65049/spec/proofs.md), [call contracts, proof fields, and vacant slots](https://github.com/nikhilgarg28/locus/blob/c2818cdbe407ef49deb3301221bc29b9cba65049/spec/contracts.md), and [collection correspondence and actual loan-end restoration](https://github.com/nikhilgarg28/locus/blob/c2818cdbe407ef49deb3301221bc29b9cba65049/spec/collections.md). This study changes no normative rule or PR.

The following source appendix is the complete primary Locus candidate, including its comments and unresolved obligations. The downloadable bundle additionally contains all executable Rust implementations, tests, reproduction scripts, and validation logs.
