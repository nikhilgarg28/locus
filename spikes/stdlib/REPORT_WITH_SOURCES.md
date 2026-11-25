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

# Annotated Locus sources

## Logical sequences

`locus/00-seq.lc`

```rust
// PRIMARY LOCUS SOURCE. Pure, erased sequence data; no native Vec underneath.
// `_` means an ordinary proof hole, NOT an assumed theorem. These candidates
// have not been checked by a Locus compiler. Recursive proof calls are explicit.

#[derive(Logical)]
pub enum Maybe<A: Logical> { None, Some(A) }

#[derive(Logical)]
pub enum Seq<A: Logical> { Nil, Cons(A, Seq<A>) }

impl<A: Logical> Seq<A> {
    pub logic fn empty() -> Self { Seq::Nil }
    pub logic fn len(self) -> Nat {
        match self { Seq::Nil => 0, Seq::Cons(_, tail) => 1 + tail.len() }
    }
    pub logic fn get(self, i: Nat) -> Maybe<A> {
        match self {
            Seq::Nil => Maybe::None,
            Seq::Cons(head, tail) => if i == 0 { Maybe::Some(head) }
                else { tail.get((i - 1).as_nat(_)) },
        }
    }
    pub logic fn push(self, x: A) -> Self {
        match self {
            Seq::Nil => Seq::Cons(x, Seq::Nil),
            Seq::Cons(head, tail) => Seq::Cons(head, tail.push(x)),
        }
    }
    pub logic fn concat(self, right: Self) -> Self {
        match self {
            Seq::Nil => right,
            Seq::Cons(head, tail) => Seq::Cons(head, tail.concat(right)),
        }
    }
    pub logic fn take(self, n: Nat) -> Self {
        match self {
            Seq::Nil => Seq::Nil,
            Seq::Cons(head, tail) => if n == 0 { Seq::Nil }
                else { Seq::Cons(head, tail.take((n - 1).as_nat(_))) },
        }
    }
    pub logic fn skip(self, n: Nat) -> Self {
        match self {
            Seq::Nil => Seq::Nil,
            Seq::Cons(head, tail) => if n == 0 { Seq::Cons(head, tail) }
                else { tail.skip((n - 1).as_nat(_)) },
        }
    }
    pub logic fn update(self, i: Nat, x: A) -> Self {
        match self {
            Seq::Nil => Seq::Nil,
            Seq::Cons(head, tail) => if i == 0 { Seq::Cons(x, tail) }
                else { Seq::Cons(head, tail.update((i - 1).as_nat(_), x)) },
        }
    }
    pub logic fn insert(self, i: Nat, x: A) -> Self {
        // Out-of-range insert appends. Collection contracts require i <= len.
        match self {
            Seq::Nil => Seq::Cons(x, Seq::Nil),
            Seq::Cons(head, tail) => if i == 0 { Seq::Cons(x, self) }
                else { Seq::Cons(head, tail.insert((i - 1).as_nat(_), x)) },
        }
    }
    pub logic fn remove(self, i: Nat) -> Self {
        match self {
            Seq::Nil => Seq::Nil,
            Seq::Cons(head, tail) => if i == 0 { tail }
                else { Seq::Cons(head, tail.remove((i - 1).as_nat(_))) },
        }
    }
    pub logic fn map<B: Logical>(self, f: logic fn(A) -> B) -> Seq<B> {
        match self {
            Seq::Nil => Seq::Nil,
            Seq::Cons(head, tail) => Seq::Cons(f(head), tail.map(f)),
        }
    }
    pub logic fn contains(self, x: A) -> Prop {
        match self { Seq::Nil => False,
            Seq::Cons(head, tail) => head === x || tail.contains(x) }
    }
    pub logic fn unique(self) -> Prop {
        match self { Seq::Nil => True,
            Seq::Cons(head, tail) => !tail.contains(head) && tail.unique() }
    }
}

// Examples of proof-library development, not extra solver axioms.
pub logic fn len_push<A: Logical>(s: Seq<A>, x: A)
    -> @(s.push(x).len() == s.len() + 1)
{
    match s {
        Seq::Nil => _,
        Seq::Cons(head, tail) => {
            let ih = len_push(tail, x);
            let push_step = eq::unfold(Seq::push)(s, x);
            let len_step = eq::unfold(Seq::len)(s);
            _
        }
    }
}
pub logic fn len_update<A: Logical>(s: Seq<A>, i: Nat, x: A)
    -> @(s.update(i, x).len() == s.len())
{
    match s {
        Seq::Nil => _,
        Seq::Cons(head, tail) => if i == 0 { _ } else {
            let ih = len_update(tail, (i - 1).as_nat(_), x);
            let step = eq::unfold(Seq::update)(s, i, x);
            _
        },
    }
}
pub logic fn get_update_at<A: Logical>(s: Seq<A>, i: Nat, x: A,
    inside: @(i < s.len())) -> @(s.update(i, x).get(i) === Maybe::Some(x))
{
    match s {
        Seq::Nil => { let impossible: @False = _; _ }
        Seq::Cons(head, tail) => if i == 0 { _ } else {
            let j = (i - 1).as_nat(_);
            let ih = get_update_at(tail, j, x, _);
            let step = eq::unfold(Seq::update)(s, i, x);
            let read = eq::unfold(Seq::get)(s.update(i, x), i);
            _
        },
    }
}
pub logic fn get_update_away<A: Logical>(s: Seq<A>, i: Nat, j: Nat, x: A,
    apart: @(i !== j)) -> @(s.update(i, x).get(j) === s.get(j))
{
    match s {
        Seq::Nil => _,
        Seq::Cons(head, tail) => if i == 0 || j == 0 { _ } else {
            let ih = get_update_away(tail, (i - 1).as_nat(_), (j - 1).as_nat(_), x, _);
            let step = eq::unfold(Seq::update)(s, i, x);
            let left = eq::unfold(Seq::get)(s.update(i, x), j);
            let right = eq::unfold(Seq::get)(s, j);
            _
        },
    }
}
// IMPORTANT OPEN ELABORATION WORK: some final `_` terms need explicit rewrites
// from the match equation and IH. They are not counted as completed proofs.
// A generic get returns Maybe<A>. A direct proof-indexed at -> A needs a way to
// type its impossible Nil branch. This is an ELABORATION QUESTION, not evidence
// that generic indexing is impossible. Existing well-founded recursion may be
// sufficient: the same-argument call below is under an inconsistent precondition,
// so its otherwise false descent obligation follows from that precondition.
// The termination checker must validate this; ordinary runtime divergence would
// NOT be an acceptable implementation of a total logical function.
#[decreases(s.len())]
pub logic fn at_candidate<A:Logical>(s:Seq<A>,i:Nat,inside:@(i<s.len()))->A {
    match s {
        Seq::Nil => at_candidate(s,i,inside), // OPEN: discharge impossible descent
        Seq::Cons(head,tail) => if i==0 {head}
            else {at_candidate(tail,(i-1).as_nat(_),_)},
    }
}
pub logic fn contains_positive<A:Logical>(s:Seq<A>,x:A,p:@s.contains(x))
    -> @(s.len()>0)
{
    match s {Seq::Nil=>_,Seq::Cons(head,tail)=>_}
    // OPEN: explicit unfold of contains/len and match equalities as required.
}
```

## Finite logical maps

`locus/01-map.lc`

```rust
// A finite mathematical map, independent of B-trees, hashes, and insertion order.
use crate::seq::{Seq, Maybe};

// Exact prop constructor spelling follows the existing `Arm(args) @ proof` form.
prop Finite<K: Logical, V: Logical>(f: logic fn(K) -> Maybe<V>, size: Nat) {
    Witness(keys: Seq<K>) => { keys.unique() && keys.len() == size
        && (forall |k: K| iff(keys.contains(k), f(k) !== Maybe::None)) }
}

#[derive(Logical)]
pub struct Map<K: Logical, V: Logical> {
    lookup: logic fn(K) -> Maybe<V>,
    size: Nat,
    finite: @Finite(lookup, size),
}
impl<K: Logical, V: Logical> Map<K, V> {
    pub logic fn empty() -> Self {
        let lookup = |k: K| Maybe::None;
        let domain: @(forall |k: K| iff(Seq::<K>::Nil.contains(k), lookup(k) !== Maybe::None)) =
            |k: K| _;
        Map { lookup, size: 0, finite: Finite::Witness(Seq::Nil) @ _ }
    }
    pub logic fn get(self, k: K) -> Maybe<V> { (self.lookup)(k) }
    pub logic fn len(self) -> Nat { self.size }
    pub logic fn insert(self, k: K, v: V) -> Self {
        let next = |q: K| if q === k { Maybe::Some(v) } else { self.get(q) };
        let size = if self.get(k) === Maybe::None { self.size + 1 } else { self.size };
        let finite = finite_insert(self.lookup, self.size, self.finite, k, v);
        Map { lookup: next, size, finite }
    }
    pub logic fn remove(self, k: K) -> Self {
        let next = |q: K| if q === k { Maybe::None } else { self.get(q) };
        let size = if self.get(k) === Maybe::None { self.size }
            else { (self.size - 1).as_nat(nonempty_size(self, k, _)) };
        let finite = finite_remove(self.lookup, self.size, self.finite, k);
        Map { lookup: next, size, finite }
    }
    pub logic fn equivalent(self, other: Self) -> Prop {
        self.size === other.size && (forall |k: K| self.get(k) === other.get(k))
    }
}

logic fn without<K: Logical>(s: Seq<K>, k: K) -> Seq<K> {
    match s {
        Seq::Nil => Seq::Nil,
        Seq::Cons(head, tail) => if head === k { without(tail, k) }
            else { Seq::Cons(head, without(tail, k)) },
    }
}
logic fn nonempty_size<K: Logical,V: Logical>(m: Map<K,V>, k: K,
    present: @(m.get(k) !== Maybe::None)) -> @(m.size as Int - 1 >= 0)
{
    nonempty_fn_size(m.lookup,m.size,m.finite,k,present)
}
logic fn nonempty_fn_size<K:Logical,V:Logical>(f:logic fn(K)->Maybe<V>,n:Nat,
    finite:@Finite(f,n),k:K,present:@(f(k)!==Maybe::None))->@(n as Int - 1 >= 0)
{
    match finite {
        Finite::Witness(keys) @ facts => {
            let support: @(forall |q: K| iff(keys.contains(q), f(q) !== Maybe::None)) = _;
            let at_key = support(k);
            let positive=crate::seq::contains_positive(keys,k,_);
            _
        }
    }
}
logic fn finite_insert<K: Logical,V: Logical>(f: logic fn(K)->Maybe<V>, n: Nat,
    finite: @Finite(f,n), k: K, v: V)
    -> @Finite(|q: K| if q === k { Maybe::Some(v) } else { f(q) },
        if f(k) === Maybe::None { n + 1 } else { n })
{
    match finite {
        Finite::Witness(keys) @ facts => {
            if f(k) === Maybe::None {
                let support: @(forall |q: K| iff(keys.contains(q), f(q) !== Maybe::None)) = _;
                let absent = support(k);
                let next_keys = Seq::Cons(k, keys);
                let next_support: @(forall |q: K| iff(next_keys.contains(q),
                    (if q === k {Maybe::Some(v)} else {f(q)}) !== Maybe::None)) =
                    |q: K| { let old = support(q); _ };
                Finite::Witness(next_keys) @ _ // OPEN: assemble support and uniqueness.
            } else { Finite::Witness(keys) @ _ }
        }
    }
}
logic fn finite_remove<K: Logical,V: Logical>(f: logic fn(K)->Maybe<V>, n: Nat,
    finite: @Finite(f,n), k: K)
    -> @Finite(|q: K| if q === k { Maybe::None } else { f(q) },
        if f(k) === Maybe::None { n }
            else { (n - 1).as_nat(nonempty_fn_size(f,n,finite,k,_)) })
{
    match finite {
        Finite::Witness(keys) @ facts => {
            let rest = without(keys,k);
            // OPEN: without preserves uniqueness, support, and removes exactly
            // one key when present. The witness remains entirely inside a proof.
            Finite::Witness(rest) @ _
        }
    }
}
pub logic fn get_insert<K: Logical,V: Logical>(m: Map<K,V>, k: K, v: V, q: K)
    -> @(m.insert(k,v).get(q) === if q === k {Maybe::Some(v)} else {m.get(q)})
{ _ }
pub logic fn insert_overwrite<K: Logical,V: Logical>(m: Map<K,V>, k: K, a: V, b: V)
    -> @(m.insert(k,a).insert(k,b).equivalent(m.insert(k,b)))
{
    let pointwise: @(forall |q: K|
        m.insert(k,a).insert(k,b).get(q) === m.insert(k,b).get(q)) = |q: K| {
        let first = get_insert(m,k,a,q);
        let second = get_insert(m.insert(k,a),k,b,q);
        let direct = get_insert(m,k,b,q);
        _
    };
    _
}
// DESIGN GATE: Map's === should be extensional. The lookup-function + finite
// certificate representation avoids exposing insertion order, but upgrading
// equivalent to === needs a checked function-extensionality/cardinality lemma.
// Lean can supply the mathematical theorem; current Locus has no public funext
// API. No new trusted map axioms are assumed. Contracts use equivalent meanwhile.
```

## Key equality order and hash laws

`locus/02-laws.lc`

```rust
// User traits; no compiler magic. Key projection may omit irrelevant fields.
pub trait KeySpec: Snapshot {
    type Key: Logical;
    logic fn key(s: Snap<Self>) -> Self::Key;
}
pub trait LawfulEq: KeySpec {
    fn eq_checked(&self, other: &Self)
        -> (same: bool, @(iff(same, Self::key(snap(self)) === Self::key(snap(other)))));
}
pub trait TotalOrd: LawfulEq {
    logic fn le(a: Self::Key, b: Self::Key) -> Prop;
    logic fn refl(a: Self::Key) -> @(Self::le(a,a));
    logic fn trans(a: Self::Key,b: Self::Key,c: Self::Key,
        ab: @(Self::le(a,b)),bc: @(Self::le(b,c))) -> @(Self::le(a,c));
    logic fn total(a: Self::Key,b: Self::Key) -> @(Self::le(a,b)||Self::le(b,a));
    logic fn antisym(a: Self::Key,b: Self::Key,
        both: @(Self::le(a,b)&&Self::le(b,a))) -> @(a === b);
    fn cmp_checked(&self, other: &Self)
        -> (c: std::cmp::Ordering,
            @(match c {
                std::cmp::Ordering::Less => Self::le(Self::key(snap(self)),Self::key(snap(other)))
                    && Self::key(snap(self)) !== Self::key(snap(other)),
                std::cmp::Ordering::Equal => Self::key(snap(self)) === Self::key(snap(other)),
                std::cmp::Ordering::Greater => Self::le(Self::key(snap(other)),Self::key(snap(self)))
                    && Self::key(snap(self)) !== Self::key(snap(other)),
            }));
}
pub trait HashEq: LawfulEq {
    logic fn code(k: Self::Key) -> Nat;
    logic fn code_range(k: Self::Key) -> @(Self::code(k) < 18446744073709551616);
    fn hash_checked(&self) -> (h: u64, @(h === Self::code(Self::key(snap(self)))));
}
// Hash collisions ARE permitted. code equality does not imply key equality.
// A fixed checked hash is enough for this native table; production randomized
// hashing needs a seed/state-indexed law, held fixed throughout a table's life.

// Native B-tree: K: TotalOrd. Native hash table: K: HashEq.
// std wrappers additionally need std::Ord or std::Eq + std::Hash AND a bridge
// to the SAME KeySpec projection. A method's forwarding body alone does not
// export a contract for arbitrary std calls. Those bridges live in 04-std-api.lc.

impl KeySpec for u32 {
    type Key = Nat;
    logic fn key(s: Snap<u32>) -> Nat { s }
}
impl LawfulEq for u32 {
    #[verified]
    fn eq_checked(&self,other:&u32)
        -> (r:bool,@(iff(r,*self === *other))) {(*self == *other,_)}
}
impl TotalOrd for u32 {
    logic fn le(a:Nat,b:Nat)->Prop {a<=b}
    logic fn refl(a:Nat)->@(a<=a) {_}
    logic fn trans(a:Nat,b:Nat,c:Nat,ab:@(a<=b),bc:@(b<=c))->@(a<=c) {_}
    logic fn total(a:Nat,b:Nat)->@(a<=b||b<=a) {_}
    logic fn antisym(a:Nat,b:Nat,both:@(a<=b&&b<=a))->@(a===b) {_}
    #[verified]
    fn cmp_checked(&self,other:&u32)
        -> (c:std::cmp::Ordering,@(match c {
            std::cmp::Ordering::Less=>*self<*other,
            std::cmp::Ordering::Equal=>*self===*other,
            std::cmp::Ordering::Greater=>*other<*self,
        }))
    { if *self<*other {(std::cmp::Ordering::Less,_)}
      else if *self>*other {(std::cmp::Ordering::Greater,_)}
      else {(std::cmp::Ordering::Equal,_)} }
}
impl HashEq for u32 {
    logic fn code(k:Nat)->Nat { k % 18446744073709551616 }
    logic fn code_range(k:Nat)->@(Self::code(k)<18446744073709551616) {_}
    #[verified]
    fn hash_checked(&self)->(h:u64,@(h===Self::code(*self))) {(*self as u64,_)}
}
```

## Permissioned memory interfaces

`locus/03-permissions.lc`

```rust
// TRUSTED INTERFACE FOR THIS SPIKE, not new accepted Locus syntax/axioms.
// The declarations ending in `;` below are primitive API specifications. Their
// executable implementations are src/permission.rs. The user authorized the
// assumption that a permissioned allocation's snapshot observes its contents.
// Allocation, initialized access, movement and deallocation additionally need
// the stated primitive contracts. Their soundness is NOT proved by this spike.
use crate::seq::{Seq, Maybe};

// Runtime resources, not Logical types: affine, non-Copy, non-Clone. No public
// constructor except allocate. Ptr is only an address/identity, not a permission.
pub struct Ptr<T>;
pub struct Region<T>;
// Generated snapshot of Region<T>, when T: Snapshot:
// { allocation: Nat, cells: Seq<Maybe<Snap<T>>> }. Allocation identities are
// distinct even for two zero-sized allocations; they are NOT produced by mint.

impl<T: Snapshot> Region<T> {
    pub logic fn cells(s: Snap<Self>) -> Seq<Maybe<Snap<T>>> { s.cells }
    pub logic fn capacity(s: Snap<Self>) -> Nat { s.cells.len() }
    #[verified]
    pub fn allocate(n: usize)
        -> (r: Self, @(Self::capacity(snap(r)) === n
            && (forall |i: Nat| i < n ==> Self::cells(snap(r)).get(i) === Maybe::Some(Maybe::None))));
    #[verified]
    pub fn capacity_checked(&self) -> (n: usize, @(n === Self::capacity(snap(self))));
    #[verified]
    pub fn read_checked(&self, i: usize,
        initialized: @(exists |x: Snap<T>| Self::cells(snap(self)).get(i) === Maybe::Some(Maybe::Some(x))))
        -> (x: &T, @(Self::cells(snap(self)).get(i) === Maybe::Some(Maybe::Some(snap(x)))));
    #[verified]
    pub fn take_checked(&mut self, i: usize,
        initialized: @(exists |x: Snap<T>| Self::cells(snap(self)).get(i) === Maybe::Some(Maybe::Some(x))))
        -> (x: T, @(Self::cells(snap(self)).get(i) === Maybe::Some(Maybe::Some(snap(x)))
            && Self::cells(snap(final!(self))) === Self::cells(snap(self)).update(i,Maybe::None)));
    #[verified]
    pub fn write_checked(&mut self, i: usize, x: T,
        vacant: @(Self::cells(snap(self)).get(i) === Maybe::Some(Maybe::None)))
        -> @(Self::cells(snap(final!(self))) === Self::cells(snap(self)).update(i,Maybe::Some(snap(x))));
    #[verified]
    pub fn borrow_mut_checked(&mut self, i: usize,
        initialized: @(exists |x: Snap<T>| Self::cells(snap(self)).get(i) === Maybe::Some(Maybe::Some(x))))
        -> (x: &mut T,
            @(Self::cells(snap(self)).get(i) === Maybe::Some(Maybe::Some(snap(x)))
            && Self::cells(snap(final!(self))) === Self::cells(snap(self)).update(i,Maybe::Some(snap(final!(x))))));
    // Only an entirely uninitialized region can be deallocated without dropping T.
    #[verified]
    pub fn free(self, empty: @(forall |i: Nat| i < Self::capacity(snap(self)) ==>
        Self::cells(snap(self)).get(i) === Maybe::Some(Maybe::None)));
}
// Region's own Drop must drop every remaining initialized T, once, then free.
// The Rust kernel has an unwind cleanup guard to continue after one Drop panic.
// No safe function manufactures a resource from a hypothetical snapshot.

// Single-object owning permission: its type guarantees initialization. This is
// a separate primitive state, so ordinary library Box needs no proof-read field
// whose mutation would be forbidden by contracts.invariants.no-mutable-borrow.
pub struct Owned<T>;
impl<T: Snapshot> Owned<T> {
    pub logic fn value(s: Snap<Self>) -> Snap<T> { s.value }
    #[verified]
    pub fn new_checked(x:T)->(p:Self,@(Self::value(snap(p))===snap(x)));
    #[verified]
    pub fn read_checked(&self)->(x:&T,@(snap(x)===Self::value(snap(self))));
    #[verified]
    pub fn borrow_mut_checked(&mut self)->(x:&mut T,
        @(snap(x)===Self::value(snap(self))
          && Self::value(snap(final!(self)))===snap(final!(x))));
    #[verified]
    pub fn into_inner_checked(self)->(x:T,@(snap(x)===Self::value(snap(self))));
}
```

## Native storage adapters and trait bridges

`locus/04-std-api.lc`

```rust
// PROPOSED TRUSTED NATIVE ADAPTER INTERFACES. They do not make std methods pure,
// do not unfold runtime code, and do not make std Vec's raw layout observable.
// Native::observe below is an abstract library specification, not a user-defined
// Snapshot. These abstract observers/bridges are an explicit extra trust boundary.
use crate::seq::{Seq,Maybe};
use crate::map::Map;
use crate::laws::{KeySpec,TotalOrd,HashEq};

pub trait NativeVector<T: Snapshot>: Snapshot {
    logic fn contents(s: Snap<Self>) -> Seq<Snap<T>>;
    fn empty_checked()->(v:Self,@(Self::contents(snap(v))===Seq::Nil));
    fn len_checked(&self)->(n:usize,@(n===Self::contents(snap(self)).len()));
    fn push_checked(&mut self,x:T)
        -> @(Self::contents(snap(final!(self)))===Self::contents(snap(self)).push(snap(x)));
    fn pop_checked(&mut self)->(out:Option<T>,@(match out {
        None=>Self::contents(snap(self)).len()==0
            && Self::contents(snap(final!(self)))===Self::contents(snap(self)),
        Some(x)=>Self::contents(snap(self)).get((Self::contents(snap(self)).len()-1).as_nat(_))===Maybe::Some(snap(x))
            && Self::contents(snap(final!(self)))===Self::contents(snap(self)).take((Self::contents(snap(self)).len()-1).as_nat(_)),
    }));
    fn get_checked(&self,i:usize,p:@(i<Self::contents(snap(self)).len()))
        ->(x:&T,@(Self::contents(snap(self)).get(i)===Maybe::Some(snap(x))));
    fn set_checked(&mut self,i:usize,x:T,p:@(i<Self::contents(snap(self)).len()))
        ->@(Self::contents(snap(final!(self)))===Self::contents(snap(self)).update(i,snap(x)));
}
// BLOCKER: std Vec<T> has no contents-bearing Snapshot in the baseline.
// Thus the trait above is an interface proposal, NOT an implementable downstream
// impl for std Vec. A library-private trusted relation over native state is one
// option; an external-spec mechanism is another. A stored ghost Seq alone does
// not supply these contracts. The wrapper algorithms in 06 expose this boundary.

// Sealed certificates: a user must NOT be able to assert either bridge merely
// by writing an empty impl. This spike admits u32 only, as a trusted whitelist.
// General user-type certification is the still-open adapter/contract mechanism.
mod bridge_authority {
    pub trait OrdCertified {}
    pub trait HashCertified {}
    impl OrdCertified for u32 {}
    impl HashCertified for u32 {}
}
pub trait StdOrdBridge: TotalOrd + std::cmp::Ord + bridge_authority::OrdCertified {
    // This must relate the actual resolved std::Ord implementation, not just
    // introduce another comparison. There is no accepted Locus member spelling
    // for "the result of this arbitrary runtime call". An adapter identity is
    // therefore REQUIRED metadata, left explicit here as an open interface gate.
}
pub trait StdHashBridge: HashEq + std::cmp::Eq + std::hash::Hash + bridge_authority::HashCertified {
    // Requires std equality iff KeySpec equality, and equal keys write equivalent
    // hash input for a common hasher state/seed. A single u64 hash result is not a
    // specification of arbitrary H:Hasher. No blanket trusted implementation.
}
impl StdOrdBridge for u32 {} // TRUST: actual std u32 Ord agrees with KeySpec
impl StdHashBridge for u32 {} // TRUST: actual std u32 Eq/Hash agree with KeySpec
```

## Native vector

`locus/05-native-vec.lc`

```rust
// Actual Locus algorithm on permissioned storage. No std::Vec in this module.
use crate::seq::{Seq,Maybe};
use crate::permissions::Region;
use locus::std::mem;

logic fn prefix<A:Logical>(cells:Seq<Maybe<A>>, n:Nat)->Prop {
    n<=cells.len()
    && (forall |i:Nat| i<n ==> cells.get(i)!==Maybe::Some(Maybe::None))
    && (forall |i:Nat| n<=i && i<cells.len() ==> cells.get(i)===Maybe::Some(Maybe::None))
}
logic fn values<A:Logical>(cells:Seq<Maybe<A>>, n:Nat)->Seq<A> {
    match cells {
        Seq::Nil=>Seq::Nil,
        Seq::Cons(head,tail)=>if n==0 {Seq::Nil} else {
            let rest=values(tail,(n-1).as_nat(_));
            match head {Maybe::None=>rest,Maybe::Some(x)=>Seq::Cons(x,rest)}
        },
    }
}
pub struct Vec<T:Snapshot> {
    data:Region<T>, len:usize,
    shape:@prefix(Region::cells(snap(data)),len),
}
impl<T:Snapshot> Vec<T> {
    pub logic fn elements(s:Snap<Self>)->Seq<Snap<T>> {
        values(Region::cells(s.data),s.len)
    }
    #[verified]
    pub fn empty()->(v:Self,@(Self::elements(snap(v))===Seq::Nil)) {
        let (data,empty)=Region::allocate(0);
        (Vec{data,len:0,shape:_},_)
    }
    #[verified]
    pub fn len_checked(&self)->(n:usize,@(n===Self::elements(snap(self)).len())) {
        // OPEN lemma: prefix(cells,n) implies values(cells,n).len()==n.
        (self.len,_)
    }
    #[verified]
    pub fn len(&self)->usize {self.len}
    #[verified]
    pub fn get_checked(&self,i:usize,p:@(i<Self::elements(snap(self)).len()))
        ->(x:&T,@(Self::elements(snap(self)).get(i)===Maybe::Some(snap(x))))
    {
        let (x,read)=self.data.read_checked(i,_);
        // OPEN lemma: get of values at an initialized prefix position.
        (x,_)
    }
    #[verified]
    pub fn reserve_checked(&mut self,want:usize)
        ->@(Self::elements(snap(final!(self)))===Self::elements(snap(self))
            && Region::capacity(snap(final!(self).data))>=want)
    {
        let (cap,capacity)=self.data.capacity_checked();
        if want<=cap {return _;}
        let before=logic!{Self::elements(snap(self))};
        let (slot,old,was)=mem::take_out(self);
        let Vec{mut data,len,shape}=old;
        let (mut next,vacant)=Region::allocate(want);
        let mut i:usize=0;
        let mut moving:@(i<=len
            && values(Region::cells(snap(next)),i)===before.take(i)
            && values(Region::cells(snap(data)).skip(i),(len as Int-i as Int).as_nat(_))===before.skip(i))=_;
        while i<len {
            let (x,taken)=data.take_checked(i,_);
            let written=next.write_checked(i,x,_);
            i+=1;
            moving=_; // OPEN: explicit prefix/suffix lemmas and equality transport.
        }
        data.free(_);
        slot.put(Vec{data:next,len,shape:_});
        _
    }
    #[verified]
    pub fn push_checked(&mut self,x:T)
        ->@(Self::elements(snap(final!(self)))===Self::elements(snap(self)).push(snap(x))
            && final!(self).len==self.len+1)
    {
        let next=match self.len.checked_add(1) {Some(n)=>n,None=>panic!("length overflow")};
        let grown=self.reserve_checked(next);
        let (slot,old,was)=mem::take_out(self);
        let Vec{mut data,len,shape}=old;
        let written=data.write_checked(len,x,_);
        slot.put(Vec{data,len:next,shape:_});
        _ // OPEN: values of prefix extended by one initialized cell.
    }
    #[verified]
    pub fn push(&mut self,x:T) {let proof=self.push_checked(x);}
    #[verified]
    pub fn pop_checked(&mut self)->(out:Option<T>,@(match out {
        None=>self.len==0 && Self::elements(snap(final!(self)))===Self::elements(snap(self)),
        Some(x)=>self.len>0 && Self::elements(snap(self)).get((self.len-1).as_nat(_))===Maybe::Some(snap(x))
            && Self::elements(snap(final!(self)))===Self::elements(snap(self)).take((self.len-1).as_nat(_)),
    }))
    {
        if self.len==0 {return (None,_);}
        let (slot,old,was)=mem::take_out(self);
        let Vec{mut data,len,shape}=old;
        let next=len-1;
        let (x,taken)=data.take_checked(next,_);
        slot.put(Vec{data,len:next,shape:_});
        (Some(x),_)
    }
    #[verified]
    pub fn set_checked(&mut self,i:usize,x:T,p:@(i<self.len))
        ->(old:T,@(Self::elements(snap(self)).get(i)===Maybe::Some(snap(old))
            && Self::elements(snap(final!(self)))===Self::elements(snap(self)).update(i,snap(x))))
    {
        let (slot,previous,was)=mem::take_out(self);
        let Vec{mut data,len,shape}=previous;
        let (old,taken)=data.take_checked(i,_);
        let written=data.write_checked(i,x,_);
        slot.put(Vec{data,len,shape:_});
        (old,_)
    }
    #[verified]
    pub fn insert_checked(&mut self,i:usize,x:T,inside:@(i<=self.len))
        ->@(Self::elements(snap(final!(self)))===Self::elements(snap(self)).insert(i,snap(x)))
    {
        let needed=match self.len.checked_add(1){Some(n)=>n,None=>panic!("length overflow")};
        let space=self.reserve_checked(needed);
        let before=logic!{Self::elements(snap(self))};
        let (slot,old,was)=mem::take_out(self);
        let Vec{mut data,len,shape}=old;
        let mut j=len;
        // OPEN moving-hole invariant: slot j is vacant; [i,j) is original and
        // (j,len] is the original suffix shifted one right. Other slots unchanged.
        while j>i{
            let previous=j-1;
            let (item,taken)=data.take_checked(previous,_);
            let written=data.write_checked(j,item,_);
            j=previous;
        }
        let written=data.write_checked(i,x,_);
        slot.put(Vec{data,len:needed,shape:_});
        _
    }
    #[verified]
    pub fn remove_checked(&mut self,i:usize,inside:@(i<self.len))
        ->(x:T,@(Self::elements(snap(self)).get(i)===Maybe::Some(snap(x))
            && Self::elements(snap(final!(self)))===Self::elements(snap(self)).remove(i)))
    {
        let (slot,old,was)=mem::take_out(self);
        let Vec{mut data,len,shape}=old;
        let (x,taken)=data.take_checked(i,_);
        let mut j=i;
        // OPEN moving-hole invariant, dual of insert_checked.
        while j<len-1{
            let next=j+1;
            let (item,taken)=data.take_checked(next,_);
            let written=data.write_checked(j,item,_);
            j=next;
        }
        slot.put(Vec{data,len:len-1,shape:_});
        (x,_)
    }
}
// get_mut_checked is deliberately NOT given a fake body. `shape` reads `data`,
// so a mutable borrow of self.data is rejected by current invariant rules.
// Taking self out does not help: the slot cannot escape with the returned loan.
// An approved loan/open-invariant protocol or a different representation is
// necessary. Permissioned snapshots alone do not close this gap.
// General Drop and consuming conversion also need an invariant-safe dismantling
// rule; the executable Region kernel handles cleanup, not a fabricated Locus proof.
```

## Wrapped vector

`locus/06-wrapped-vec.lc`

```rust
// Actual wrapper algorithm: native storage plus an erased Seq, no capacity in
// the public model. Capacity can be exposed separately by a checked observer.
// TRUST NATIVE-VEC: the library maintains private correspondence of raw contents
// to `contents`. Locus cannot express/prove that relation from Opaque's Unit.
// Explicit assumptions below are proposed trusted-library obligations, not
// accepted facts and not proof holes silently declared solved.
use crate::seq::{Seq,Maybe};
use locus::Opaque;
pub struct Vec<T> {
    raw:Opaque<std::vec::Vec<T>>,
    contents:Seq<Snap<T>>, // deferred carrier; runtime naming has no Snapshot bound
}
impl<T:Snapshot> Vec<T> {
    pub logic fn elements(s:Snap<Self>)->Seq<Snap<T>> {s.contents}
    #[verified]
    pub fn empty()->(v:Self,@(Self::elements(snap(v))===Seq::Nil)) {
        (Vec{raw:Opaque::new(std::vec::Vec::new()),contents:Seq::Nil},_)
    }
    #[verified]
    pub fn len_checked(&self)->(n:usize,@(n===self.contents.len())) {
        let n=self.raw.get_ref().len();
        (n,unsafe assume!(n===self.contents.len())) // TRUST NATIVE-VEC
    }
    #[verified]
    pub fn len(&self)->usize {self.len_checked().0}
    #[verified]
    pub fn push_checked(&mut self,x:T)
        ->@(final!(self).contents===self.contents.push(snap(x)))
    {
        let sx=logic!{snap(x)};
        self.raw.get_mut().push(x);
        self.contents=logic!{self.contents.push(sx)};
        _ // This proves the ghost update. Native correspondence remains trusted.
    }
    #[verified]
    pub fn push(&mut self,x:T) {let appended=self.push_checked(x);}
    #[verified]
    pub fn get_checked(&self,i:usize,inside:@(i<self.contents.len()))
        ->(x:&T,@(self.contents.get(i)===Maybe::Some(snap(x))))
    {
        let x=match self.raw.get_ref().get(i) {Some(x)=>x,None=>panic!("correspondence violation")};
        (x,unsafe assume!(self.contents.get(i)===Maybe::Some(snap(x)))) // TRUST NATIVE-VEC
    }
    #[verified]
    pub fn pop_checked(&mut self)->(out:Option<T>,@(match out {
        None=>self.contents.len()==0 && final!(self).contents===self.contents,
        Some(x)=>self.contents.len()>0
            && self.contents.get((self.contents.len()-1).as_nat(_))===Maybe::Some(snap(x))
            && final!(self).contents===self.contents.take((self.contents.len()-1).as_nat(_)),
    }))
    {
        let before=logic!{self.contents};
        match self.raw.get_mut().pop() {
            None=>{let empty=unsafe assume!(before.len()==0);(None,_)},
            Some(x)=>{
                let last=unsafe assume!(before.len()>0
                    && before.get((before.len()-1).as_nat(_))===Maybe::Some(snap(x)));
                self.contents=logic!{before.take((before.len()-1).as_nat(_))};
                (Some(x),_)
            }
        }
    }
    #[verified]
    pub fn set_checked(&mut self,i:usize,x:T,p:@(i<self.contents.len()))
        ->(old:T,@(self.contents.get(i)===Maybe::Some(snap(old))
            && final!(self).contents===self.contents.update(i,snap(x))))
    {
        let before=logic!{self.contents};let sx=logic!{snap(x)};
        let old=std::mem::replace(&mut self.raw.get_mut()[i],x);
        let located=unsafe assume!(before.get(i)===Maybe::Some(snap(old)));
        self.contents=logic!{before.update(i,sx)};
        (old,_)
    }
    #[verified]
    pub fn into_std(self)->std::vec::Vec<T> {
        let Vec{raw,contents}=self;raw.into_inner()
    }
    #[verified]
    pub fn from_std(raw:std::vec::Vec<T>)->Self {
        // A real runtime traversal builds the ghost sequence. This does not
        // pretend that std Vec itself has a sequence snapshot.
        let mut contents=Seq::Nil;
        let n=raw.len();let mut i:usize=0;
        while i<n {
            let item=&raw[i];
            let sx=logic!{snap(item)};
            contents=logic!{contents.push(sx)};
            i+=1;
        }
        // TRUST NATIVE-VEC: traversal visits every initialized item in order.
        // Its runtime reads can in principle be placed in an erased typing
        // skeleton; this spike does not claim the chosen lowerer can do so.
        Vec{raw:Opaque::new(raw),contents}
    }
}
// NO unrestricted DerefMut. A guard whose Drop refreshes contents is insufficient:
// mem::forget(guard) ends Rust's loan but skips the refresh. Executable harness
// demonstrates this and a fail-closed alternative whose observation is optional.
// The proposed always-available Locus Seq API needs actual loan-end restoration.
```

## Native and wrapped boxes

`locus/08-boxes.lc`

```rust
use crate::permissions::Owned;
use locus::Opaque;

pub struct Box<T>{owned:Owned<T>}
impl<T:Snapshot> Box<T>{
    pub logic fn value(s:Snap<Self>)->Snap<T>{Owned::value(s.owned)}
    #[verified]
    pub fn new_checked(x:T)->(b:Self,@(Self::value(snap(b))===snap(x))){
        let (owned,memory)=Owned::new_checked(x);(Box{owned},_)
    }
    #[verified]
    pub fn new(x:T)->Self{Self::new_checked(x).0}
    #[verified]
    pub fn get_checked(&self)->(x:&T,@(snap(x)===Self::value(snap(self)))){
        let (x,read)=self.owned.read_checked();(x,_)
    }
    #[verified]
    pub fn get_mut_checked(&mut self)->(x:&mut T,
        @(snap(x)===Self::value(snap(self))&&Self::value(snap(final!(self)))===snap(final!(x)))){
        // No authored proof field locks owned; the primitive's type owns validity.
        let (x,loan)=self.owned.borrow_mut_checked();(x,_)
    }
    #[verified]
    pub fn into_inner_checked(self)->(x:T,@(snap(x)===Self::value(snap(self)))){
        let Box{owned}=self;let (x,moved)=owned.into_inner_checked();(x,_)
    }
}
// Ordinary Deref and DerefMut can forward to these methods, but their Rust
// signatures return no proofs. Compiler special treatment of std Box moving-out
// and unsizing does not automatically transfer to this user-defined Box.

pub struct WrappedBox<T>{raw:Opaque<std::boxed::Box<T>>,model:Snap<T>}
impl<T:Snapshot> WrappedBox<T>{
    pub logic fn value(s:Snap<Self>)->Snap<T>{s.model}
    #[verified]
    pub fn new_checked(x:T)->(b:Self,@(Self::value(snap(b))===snap(x))){
        let model=logic!{snap(x)};
        (WrappedBox{raw:Opaque::new(std::boxed::Box::new(x)),model},_)
    }
    #[verified]
    pub fn get_checked(&self)->(x:&T,@(snap(x)===self.model)){
        let x=&**self.raw.get_ref();
        (x,unsafe assume!(snap(x)===self.model)) // TRUST NATIVE-BOX correspondence
    }
    #[verified]
    pub fn replace_checked(&mut self,x:T)->(old:T,
        @(snap(old)===self.model&&final!(self).model===snap(x)))
    {
        let sx=logic!{snap(x)};let before=logic!{self.model};
        let old=std::mem::replace(&mut **self.raw.get_mut(),x);
        let read=unsafe assume!(snap(old)===before);self.model=sx;(old,_)
    }
    #[verified]
    pub fn into_inner_checked(self)->(x:T,@(snap(x)===self.model)){
        let WrappedBox{raw,model}=self;let x=*raw.into_inner();
        (x,unsafe assume!(snap(x)===model))
    }
}
// A returned &mut from WrappedBox has the same shadow-restoration problem as
// wrapped Vec. A Drop-only guard is not an implementation of final-value facts.
```

## Native B tree algorithm

`locus/09-btree-algorithm.lc`

```rust
// Full B-tree mutation algorithms in Locus syntax, using owned-child editing.
// Proof obligations remain OPEN. No clone of K or V, no mutable element loan,
// and no trusted B-tree algorithm is assumed. The checked Vec/Box APIs below are
// the candidate implementations in 05 and 08, not magic standard containers.
use crate::native_vec::Vec;
use crate::native_box::Box;
use crate::seq::{Seq,Maybe};
use crate::map::Map;
use crate::laws::TotalOrd;
use std::cmp::Ordering;
use locus::std::mem;

struct Entry<K,V>{key:K,value:V}
struct Node<K:TotalOrd,V:Snapshot>{keys:Vec<Entry<K,V>>,children:Vec<Box<Node<K,V>>>}

// A fuelled worklist avoids forbidden mutual/nested recursive logic definitions.
// The public representation proof must establish that fuel exhausts the forest.
#[decreases(fuel)]
logic fn forest_map<K:TotalOrd,V:Snapshot>(todo:Seq<Snap<Node<K,V>>>,fuel:Nat)->Map<K::Key,Snap<V>>{
    if fuel==0 {Map::empty()} else {match todo{
        Seq::Nil=>Map::empty(),
        Seq::Cons(node,rest)=>{
            let children=node.children.elements().map(|b:Snap<Box<Node<K,V>>>|Box::value(b));
            let remaining=forest_map::<K,V>(children.concat(rest),(fuel-1).as_nat(_));
            add_entries::<K,V>(remaining,node.keys.elements())
        }
    }}
}
logic fn add_entries<K:TotalOrd,V:Snapshot>(m:Map<K::Key,Snap<V>>,xs:Seq<Snap<Entry<K,V>>>)->Map<K::Key,Snap<V>>{
    match xs{Seq::Nil=>m,Seq::Cons(e,rest)=>add_entries::<K,V>(m.insert(K::key(e.key),e.value),rest)}
}
logic fn meaning<K:TotalOrd,V:Snapshot>(n:Snap<Node<K,V>>,fuel:Nat)->Map<K::Key,Snap<V>>{
    forest_map::<K,V>(Seq::Cons(n,Seq::Nil),fuel)
}
#[decreases(fuel)]
logic fn forest_entries<K:TotalOrd,V:Snapshot>(todo:Seq<Snap<Node<K,V>>>,fuel:Nat)->Seq<Snap<Entry<K,V>>>{
    if fuel==0{Seq::Nil}else{match todo{Seq::Nil=>Seq::Nil,Seq::Cons(n,rest)=>{
        let children=n.children.elements().map(|b:Snap<Box<Node<K,V>>>|Box::value(b));
        n.keys.elements().concat(forest_entries::<K,V>(children.concat(rest),(fuel-1).as_nat(_)))
    }}}
}
#[decreases(fuel)]
logic fn exhausted<K:TotalOrd,V:Snapshot>(todo:Seq<Snap<Node<K,V>>>,fuel:Nat)->Prop{
    match todo{Seq::Nil=>True,Seq::Cons(n,rest)=>if fuel==0{False}else{
        exhausted::<K,V>(n.children.elements().map(|b:Snap<Box<Node<K,V>>>|Box::value(b)).concat(rest),(fuel-1).as_nat(_))
    }}
}
logic fn strict<K:TotalOrd>(a:K::Key,b:K::Key)->Prop{K::le(a,b)&&a!==b}
logic fn sorted_keys<K:TotalOrd,V:Snapshot>(xs:Seq<Snap<Entry<K,V>>>)->Prop{
    forall |i:Nat| forall |j:Nat| i<j && j<xs.len() ==> match (xs.get(i),xs.get(j)){
        (Maybe::Some(a),Maybe::Some(b))=>strict::<K>(K::key(a.key),K::key(b.key)),_=>False,
    }
}
logic fn child_between<K:TotalOrd,V:Snapshot>(parent:Snap<Node<K,V>>,child:Snap<Node<K,V>>,i:Nat,fuel:Nat)->Prop{
    let entries=forest_entries::<K,V>(Seq::Cons(child,Seq::Nil),fuel);
    forall |j:Nat| j<entries.len() ==> match entries.get(j){
        Maybe::None=>False,Maybe::Some(e)=>{
            (if i==0 {True} else {match parent.keys.elements().get((i-1).as_nat(_)){
                Maybe::Some(low)=>strict::<K>(K::key(low.key),K::key(e.key)),_=>False}})
            && (i==parent.keys.elements().len() || match parent.keys.elements().get(i){
                Maybe::Some(high)=>strict::<K>(K::key(e.key),K::key(high.key)),_=>False})
        }
    }
}
#[decreases(fuel)]
logic fn valid_node<K:TotalOrd,V:Snapshot>(n:Snap<Node<K,V>>,fuel:Nat,height:Nat,root:Bool)->Prop{
    if fuel==0{False}else{
        n.keys.elements().len()<=3 && (root || n.keys.elements().len()>0)
        && sorted_keys::<K,V>(n.keys.elements())
        && if n.children.elements().len()==0{height==0}else{
            if height==0 {False} else {n.children.elements().len()==n.keys.elements().len()+1
            && (forall |i:Nat| i<n.children.elements().len() ==> match n.children.elements().get(i){
                Maybe::None=>False,Maybe::Some(b)=>{
                    valid_node::<K,V>(Box::value(b),(fuel-1).as_nat(_),(height-1).as_nat(_),false)
                    && child_between::<K,V>(n,Box::value(b),i,fuel)
                }
            })}
        }
    }
}
// Predecessors use explicit if branches. A proposition connective does not
// automatically supply the proof required by a data-producing as_nat call.
logic fn representation<K:TotalOrd,V:Snapshot>(n:Snap<Node<K,V>>,len:Nat,height:Nat)->Prop{
    let todo=Seq::Cons(n,Seq::Nil);let entries=forest_entries::<K,V>(todo,len+1);
    valid_node::<K,V>(n,len+1,height,true)&&exhausted::<K,V>(todo,len+1)
    && entries.len()==len && entries.map(|e:Snap<Entry<K,V>>|K::key(e.key)).unique()
}

impl<K:TotalOrd,V:Snapshot> Node<K,V>{
    #[verified]
    fn find(&self,key:&K)->Option<&V>{
        let(i,p)=self.locate(key);let(n,p)=self.keys.len_checked();
        if i<n{
            let(entry,at)=self.keys.get_checked(i,_);let(same,equal)=entry.key.eq_checked(key);
            if same{return Some(&entry.value);}
        }
        let(n,p)=self.children.len_checked();if n==0{return None;}
        let(child,at)=self.children.get_checked(i,_);let(node,at)=child.get_checked();
        node.find(key)
    }
    #[verified]
    fn empty()->Self{
        let(keys,kempty)=Vec::empty();let(children,cempty)=Vec::empty();Node{keys,children}
    }
    #[verified]
    fn locate(&self,key:&K)->(at:usize,@(at<=self.keys.elements().len())){
        let(n,count)=self.keys.len_checked();let mut i:usize=0;
        let mut bound:@(i<=n)=_;
        while i<n{
            let(entry,located)=self.keys.get_checked(i,_);
            let(order,compared)=entry.key.cmp_checked(key);
            match order{Ordering::Less=>{i+=1;bound=_;},_=>return(i,_)}
        }
        (i,_)
        // OPEN stronger loop invariant: every earlier key is strictly smaller.
    }
    #[verified]
    fn replace_value(&mut self,i:usize,value:V,inside:@(i<self.keys.elements().len()))->V{
        let(entry,removed)=self.keys.remove_checked(i,_);
        let Entry{key,value:old}=entry;
        let restored=self.keys.insert_checked(i,Entry{key,value},_);old
    }
    #[verified]
    fn take_child(&mut self,i:usize,inside:@(i<self.children.elements().len()))->Node<K,V>{
        let(boxed,removed)=self.children.remove_checked(i,_);
        let(child,unboxed)=boxed.into_inner_checked();child
        // OPEN interface refinement: return removed/unboxed evidence with child.
        // Current ordinary return intentionally loses it; algorithms run, but
        // complete map proofs need this helper's checked result strengthened.
    }
    #[verified]
    fn put_child(&mut self,i:usize,child:Self,inside:@(i<=self.children.elements().len())){
        let(boxed,boxed_fact)=Box::new_checked(child);
        let inserted=self.children.insert_checked(i,boxed,_);
    }
    #[verified]
    fn split_child(&mut self,i:usize,inside:@(i<self.children.elements().len())){
        let mut child=self.take_child(i,_);
        let(mut right_keys,empty)=Vec::empty();
        let(right_entry,taken)=child.keys.remove_checked(2,_);
        let appended=right_keys.push_checked(right_entry);
        let(middle,taken)=child.keys.remove_checked(1,_);
        let(mut right_children,empty)=Vec::empty();
        let(n,count)=child.children.len_checked();
        if n!=0{
            let(a,taken)=child.children.remove_checked(2,_);
            let ap=right_children.push_checked(a);
            let(b,taken)=child.children.remove_checked(2,_);
            let bp=right_children.push_checked(b);
        }
        self.put_child(i,child,_);
        self.put_child(i+1,Node{keys:right_keys,children:right_children},_);
        let promoted=self.keys.insert_checked(i,middle,_);
        // OPEN precondition: child has exactly 3 keys and 0 or 4 children.
        // OPEN postcondition: map preserved, two one-key children, parent +1 key.
    }
    #[verified]
    fn merge_children(&mut self,i:usize,inside:@(i<self.keys.elements().len())){
        let(separator,removed)=self.keys.remove_checked(i,_);
        let mut left=self.take_child(i,_);let mut right=self.take_child(i,_);
        let promoted=left.keys.push_checked(separator);
        let(n,count)=right.keys.len_checked();let mut j:usize=0;
        while j<n{let(e,taken)=right.keys.remove_checked(0,_);let moved=left.keys.push_checked(e);j+=1;}
        let(n,count)=right.children.len_checked();let mut j:usize=0;
        while j<n{let(c,taken)=right.children.remove_checked(0,_);let moved=left.children.push_checked(c);j+=1;}
        self.put_child(i,left,_);
        // OPEN loop proofs preserve order; the runtime bodies move every entry.
    }
    #[verified]
    fn fill_child(&mut self,i:usize)->usize{
        let child_n={let(c,at)=self.children.get_checked(i,_);let(n,p)=c.get_checked().0.keys.len_checked();n};
        if child_n>=2{return i;}
        if i>0{
            let left_n={let(c,at)=self.children.get_checked(i-1,_);let(n,p)=c.get_checked().0.keys.len_checked();n};
            if left_n>=2{
                let mut left=self.take_child(i-1,_);let mut child=self.take_child(i-1,_);
                let(n,count)=left.keys.len_checked();let(replacement,taken)=left.keys.remove_checked(n-1,_);
                let(separator,replaced)=self.keys.set_checked(i-1,replacement,_);
                let inserted=child.keys.insert_checked(0,separator,_);
                let(n,count)=left.children.len_checked();
                if n>0{let(c,taken)=left.children.remove_checked(n-1,_);let inserted=child.children.insert_checked(0,c,_);}
                self.put_child(i-1,left,_);self.put_child(i,child,_);return i;
            }
        }
        let(n,count)=self.keys.len_checked();
        if i<n{
            let right_n={let(c,at)=self.children.get_checked(i+1,_);let(n,p)=c.get_checked().0.keys.len_checked();n};
            if right_n>=2{
                let mut child=self.take_child(i,_);let mut right=self.take_child(i,_);
                let(replacement,taken)=right.keys.remove_checked(0,_);
                let(separator,replaced)=self.keys.set_checked(i,replacement,_);
                let appended=child.keys.push_checked(separator);
                let(n,count)=right.children.len_checked();
                if n>0{let(c,taken)=right.children.remove_checked(0,_);let appended=child.children.push_checked(c);}
                self.put_child(i,child,_);self.put_child(i+1,right,_);return i;
            }
            self.merge_children(i,_);i
        }else{self.merge_children(i-1,_);i-1}
    }
    #[verified]
    fn insert_nonfull(&mut self,key:K,value:V)->Option<V>{
        let(mut i,located)=self.locate(&key);let(n,count)=self.keys.len_checked();
        if i<n{
            let same={let(e,at)=self.keys.get_checked(i,_);let(same,eq)=e.key.eq_checked(&key);same};
            if same{return Some(self.replace_value(i,value,_));}
        }
        let(children,count)=self.children.len_checked();
        if children==0{let inserted=self.keys.insert_checked(i,Entry{key,value},_);return None;}
        let child_n={let(c,at)=self.children.get_checked(i,_);let(n,p)=c.get_checked().0.keys.len_checked();n};
        if child_n==3{
            self.split_child(i,_);
            let order={let(e,at)=self.keys.get_checked(i,_);let(c,p)=key.cmp_checked(&e.key);c};
            match order{
                Ordering::Equal=>return Some(self.replace_value(i,value,_)),
                Ordering::Greater=>i+=1,Ordering::Less=>{},
            }
        }
        let mut child=self.take_child(i,_);let old=child.insert_nonfull(key,value);
        self.put_child(i,child,_);old
    }
    #[verified]
    fn extreme(&mut self,last:bool)->Entry<K,V>{
        let(children,count)=self.children.len_checked();let(n,count)=self.keys.len_checked();
        if children==0{return self.keys.remove_checked(if last{n-1}else{0},_).0;}
        let i=self.fill_child(if last{n}else{0});let mut child=self.take_child(i,_);
        let entry=child.extreme(last);self.put_child(i,child,_);entry
    }
    #[verified]
    fn remove(&mut self,key:&K)->Option<Entry<K,V>>{
        let(i,located)=self.locate(key);let(n,count)=self.keys.len_checked();
        let equal=if i<n{let(e,at)=self.keys.get_checked(i,_);e.key.eq_checked(key).0}else{false};
        let(children,count)=self.children.len_checked();
        if equal{
            if children==0{return Some(self.keys.remove_checked(i,_).0);}
            let left_n={let(c,at)=self.children.get_checked(i,_);c.get_checked().0.keys.len_checked().0};
            if left_n>=2{
                let mut left=self.take_child(i,_);let replacement=left.extreme(true);self.put_child(i,left,_);
                return Some(self.keys.set_checked(i,replacement,_).0);
            }
            let right_n={let(c,at)=self.children.get_checked(i+1,_);c.get_checked().0.keys.len_checked().0};
            if right_n>=2{
                let mut right=self.take_child(i+1,_);let replacement=right.extreme(false);self.put_child(i+1,right,_);
                return Some(self.keys.set_checked(i,replacement,_).0);
            }
            self.merge_children(i,_);let mut child=self.take_child(i,_);
            let old=child.remove(key);self.put_child(i,child,_);old
        }else if children==0{None}else{
            let i=self.fill_child(i);let mut child=self.take_child(i,_);
            let old=child.remove(key);self.put_child(i,child,_);old
        }
    }
}
pub struct BTreeMap<K:TotalOrd,V:Snapshot>{root:Node<K,V>,len:usize,height:Nat,
    valid:@representation::<K,V>(snap(root),len,height)}
impl<K:TotalOrd,V:Snapshot> BTreeMap<K,V>{
    pub logic fn model(s:Snap<Self>)->Map<K::Key,Snap<V>>{meaning::<K,V>(s.root,s.len+1)}
    #[verified]
    pub fn new()->Self{BTreeMap{root:Node::empty(),len:0,height:0,valid:_}}
    #[verified]
    pub fn get_checked(&self,key:&K)->(out:Option<&V>,@(match out{
        None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
        Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v)),
    })){
        let out=self.root.find(key);(out,_)
        // OPEN: find's signature must carry its search proof; body alone is opaque.
    }
    #[verified]
    pub fn insert_checked(&mut self,key:K,value:V)->(old:Option<V>,
        @(Self::model(snap(final!(self))).equivalent(Self::model(snap(self)).insert(K::key(snap(key)),snap(value)))
            && (match old{None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
                Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let(slot,previous,was)=mem::take_out(self);
        let BTreeMap{mut root,len,mut height,valid}=previous;
        let(n,count)=root.keys.len_checked();
        if n==3{
            let mut next=Node::empty();next.put_child(0,root,_);next.split_child(0,_);
            root=next;height=logic!{height+1};
        }
        let old=root.insert_nonfull(key,value);
        let next_len=match old{None=>match len.checked_add(1){Some(n)=>n,None=>panic!("length overflow")},Some(_)=>len};
        slot.put(BTreeMap{root,len:next_len,height,valid:_});
        (old,_) // OPEN: strengthen helpers with proof results + representation invariant.
    }
    #[verified]
    pub fn remove_checked(&mut self,key:&K)->(old:Option<V>,
        @(Self::model(snap(final!(self))).equivalent(Self::model(snap(self)).remove(K::key(snap(key))))
            && (match old{None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
                Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let(slot,previous,was)=mem::take_out(self);
        let BTreeMap{mut root,len,mut height,valid}=previous;
        let old=root.remove(key);
        let(n,count)=root.keys.len_checked();let(children,count)=root.children.len_checked();
        if n==0 && children!=0{root=root.take_child(0,_);height=logic!{(height-1).as_nat(_)};}
        let next_len=match old{None=>len,Some(_)=>len-1};
        slot.put(BTreeMap{root,len:next_len,height,valid:_});
        match old{None=>(None,_),Some(Entry{key,value})=>(Some(value),_)}
    }
}
// This is a COMPLETE mutation algorithm candidate, not a completed verification.
// An intentionally weak internal helper signature loses evidence in several
// places. Rust testing cannot certify those final public holes. The final report
// distinguishes algorithm completion from the remaining Locus proof/API work.
```

## Native hash table algorithm

`locus/10-hash-algorithm.lc`

```rust
// Native open-addressing algorithm candidate in Locus syntax.
// Table is the temporarily open representation; public HashMap stores its proof.
// Proof-bearing signatures are supplied; unresolved `_` are genuine obligations.
use crate::native_vec::Vec;
use crate::seq::{Seq,Maybe};
use crate::map::Map;
use crate::laws::HashEq;

enum Bucket<K,V>{Empty,Deleted,Full{hash:u64,key:K,value:V}}
logic fn table_map<K:HashEq,V:Snapshot>(s:Seq<Snap<Bucket<K,V>>>)->Map<K::Key,Snap<V>>{
    match s{Seq::Nil=>Map::empty(),Seq::Cons(b,tail)=>{
        let rest=table_map::<K,V>(tail);
        match b{Bucket::Empty=>rest,Bucket::Deleted=>rest,
            Bucket::Full{hash,key,value}=>rest.insert(K::key(key),value)}
    }}
}
struct Table<K:HashEq,V:Snapshot>{buckets:Vec<Bucket<K,V>>,len:usize,used:usize}
impl<K:HashEq,V:Snapshot> Table<K,V>{
    pub logic fn model(s:Snap<Self>)->Map<K::Key,Snap<V>>{table_map::<K,V>(s.buckets.elements())}
    #[verified]
    fn vacant(cap:usize)->(b:Vec<Bucket<K,V>>,
        @(b.elements().len()===cap && (forall |i:Nat| i<cap ==> b.elements().get(i)===Maybe::Some(Bucket::Empty))))
    {
        let(mut b,empty)=Vec::empty();let mut i:usize=0;
        let mut count:@(i<=cap && b.elements().len()===i)=_;
        while i<cap{let added=b.push_checked(Bucket::Empty);i+=1;count=_;}
        (b,_) // OPEN: explicit forall invariant, append-read lemma and transport.
    }
    #[verified]
    pub fn new()->Self{let(buckets,empty)=Self::vacant(8);Table{buckets,len:0,used:0}}
    #[verified]
    fn find_checked(&self,key:&K,hash:u64)->(found:bool,at:usize,
        @(at<self.buckets.elements().len()))
    {
        let(cap,count)=self.buckets.len_checked();
        let mut i=(hash as usize)%cap;let mut visited:usize=0;let mut deleted=None;
        let mut bounds:@(i<cap && visited<=cap)=_;
        while visited<cap{
            let(bucket,located)=self.buckets.get_checked(i,_);
            match bucket{
                Bucket::Empty=>return(false,match deleted{Some(j)=>j,None=>i},_),
                Bucket::Deleted=>{if let None=deleted{deleted=Some(i);}},
                Bucket::Full{hash:stored,key:stored_key,value}=>{
                    if *stored==hash{
                        let(same,equal)=stored_key.eq_checked(key);
                        if same{return(true,i,_);}
                    }
                }
            }
            i=if i==cap-1{0}else{i+1};visited+=1;bounds=_;
        }
        match deleted{Some(i)=>(false,i,_),None=>panic!("load invariant")}
        // OPEN: strengthen result with hit/absence theorem, and loop with a
        // no-match visited interval. Existing HashEq alone cannot fill that gap.
    }
    #[verified]
    fn insert_rehashed(&mut self,hash:u64,key:K,value:V){
        let(cap,count)=self.buckets.len_checked();let mut i=(hash as usize)%cap;
        loop{
            let empty={let(b,p)=self.buckets.get_checked(i,_);match b{Bucket::Empty=>true,_=>false}};
            if empty{break;}
            i=if i==cap-1{0}else{i+1};
        }
        let(old,written)=self.buckets.set_checked(i,Bucket::Full{hash,key,value},_);
        self.len+=1;self.used+=1;
    }
    #[verified]
    fn grow_checked(&mut self)->@(Self::model(snap(final!(self))).equivalent(Self::model(snap(self)))){
        let(cap,count)=self.buckets.len_checked();
        let next=match cap.checked_mul(2){Some(n)=>n,None=>panic!("capacity overflow")};
        let(fresh,vacant)=Self::vacant(next);
        // The whole old table is owned, so no key/value Clone bound is needed.
        let mut old=std::mem::replace(&mut self.buckets,fresh);
        // OPEN boundary: plain mem::replace of a sealed Vec returns no equation.
        // Use a checked replace primitive or take_out/put to preserve that fact.
        self.len=0;self.used=0;
        loop{
            let(bucket,removed)=old.pop_checked();
            match bucket{
                None=>break,
                Some(Bucket::Full{hash,key,value})=>self.insert_rehashed(hash,key,value),
                _=>{},
            }
        }
        _ // OPEN: fold-map union invariant over moved and remaining buckets.
    }
    #[verified]
    pub fn insert_checked(&mut self,key:K,value:V)->(old:Option<V>,
        @(Self::model(snap(final!(self))).equivalent(Self::model(snap(self)).insert(K::key(snap(key)),snap(value)))
            && (match old{None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
                Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let(hash,hashed)=key.hash_checked();let(cap,count)=self.buckets.len_checked();
        if self.used>=cap/2{let preserved=self.grow_checked();}
        let(found,i,located)=self.find_checked(&key,hash);
        if found{
            let(old,taken)=self.buckets.set_checked(i,Bucket::Empty,_);
            match old{
                Bucket::Full{hash:old_hash,key:old_key,value:old_value}=>{
                    let(vacant,written)=self.buckets.set_checked(i,Bucket::Full{hash:old_hash,key:old_key,value},_);
                    (Some(old_value),_)
                }
                _=>panic!("probe invariant"),
            }
        }else{
            let(old,written)=self.buckets.set_checked(i,Bucket::Full{hash,key,value},_);
            if let Bucket::Empty=old{self.used+=1;}
            self.len+=1;(None,_)
        }
    }
    #[verified]
    pub fn get_checked(&self,key:&K)->(out:Option<&V>,@(match out{
        None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
        Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v)),
    })){
        let(hash,hashed)=key.hash_checked();let(found,i,located)=self.find_checked(key,hash);
        if !found{return(None,_);}
        let(bucket,at)=self.buckets.get_checked(i,_);
        match bucket{Bucket::Full{hash,key,value}=>(Some(value),_),_=>panic!("probe invariant")}
    }
    #[verified]
    pub fn remove_checked(&mut self,key:&K)->(old:Option<V>,
        @(Self::model(snap(final!(self))).equivalent(Self::model(snap(self)).remove(K::key(snap(key))))
            && (match old{None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
                Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let(hash,hashed)=key.hash_checked();let(found,i,located)=self.find_checked(key,hash);
        if !found{return(None,_);}
        let(bucket,removed)=self.buckets.set_checked(i,Bucket::Deleted,_);
        self.len-=1;
        match bucket{Bucket::Full{hash,key,value}=>(Some(value),_),_=>panic!("probe invariant")}
    }
    #[verified]
    pub fn insert(&mut self,key:K,value:V)->Option<V>{self.insert_checked(key,value).0}
    #[verified]
    pub fn get(&self,key:&K)->Option<&V>{self.get_checked(key).0}
    #[verified]
    pub fn remove(&mut self,key:&K)->Option<V>{self.remove_checked(key).0}
}
// Model omits bucket positions, cached hashes, capacity, and tombstones. The
// representation proof must still connect all four to lookup correctness.
// Runtime Eq/Hash without laws can be memory-safe but cannot discharge it.

// Counts and reachability are separate from the abstract finite map. Without
// these facts the public lookup claim is false for a malformed private Table.
logic fn occupied<K:HashEq,V:Snapshot>(s:Seq<Snap<Bucket<K,V>>>)->Nat {
    match s{Seq::Nil=>0,Seq::Cons(h,t)=>match h{
        Bucket::Full{hash,key,value}=>1+occupied::<K,V>(t),_=>occupied::<K,V>(t)}}
}
logic fn used_count<K:HashEq,V:Snapshot>(s:Seq<Snap<Bucket<K,V>>>)->Nat {
    match s{Seq::Nil=>0,Seq::Cons(h,t)=>match h{
        Bucket::Empty=>used_count::<K,V>(t),_=>1+used_count::<K,V>(t)}}
}
logic fn key_list<K:HashEq,V:Snapshot>(s:Seq<Snap<Bucket<K,V>>>)->Seq<K::Key> {
    match s{Seq::Nil=>Seq::Nil,Seq::Cons(h,t)=>match h{
        Bucket::Full{hash,key,value}=>Seq::Cons(K::key(key),key_list::<K,V>(t)),_=>key_list::<K,V>(t)}}
}
logic fn pow2(n:Nat)->Nat {if n==0{1}else{2*pow2((n-1).as_nat(_))}}
logic fn representation<K:HashEq,V:Snapshot>(t:Snap<Table<K,V>>)->Prop {
    let b=t.buckets.elements();let cap=b.len();
    if cap==0 {False} else {
        cap>=8 && occupied::<K,V>(b)==t.len && used_count::<K,V>(b)==t.used
        && t.len<=t.used && t.used<=cap/2 && key_list::<K,V>(b).unique()
        && (forall |i:Nat| i<cap ==> match b.get(i){
            Maybe::Some(Bucket::Full{hash,key,value})=>{
                let start=(hash % pow2(usize::BITS)) % cap;
                let distance=if i>=start{(i-start).as_nat(_)}else{(cap-start+i).as_nat(_)};
                hash===K::code(K::key(key)) && (forall |j:Nat| j<distance ==>
                    b.get((start+j)%cap)!==Maybe::Some(Bucket::Empty))
            },_=>True,
        })
    }
}
pub struct HashMap<K:HashEq,V:Snapshot>{table:Table<K,V>,valid:@representation::<K,V>(snap(table))}
impl<K:HashEq,V:Snapshot> HashMap<K,V>{
    pub logic fn model(s:Snap<Self>)->Map<K::Key,Snap<V>>{Table::model(s.table)}
    #[verified] pub fn new()->Self{HashMap{table:Table::new(),valid:_}}
    #[verified]
    pub fn insert_checked(&mut self,key:K,value:V)->(old:Option<V>,
        @(Self::model(snap(final!(self))).equivalent(Self::model(snap(self)).insert(K::key(snap(key)),snap(value)))
            && (match old{None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
                Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let(slot,previous,was)=mem::take_out(self);let HashMap{mut table,valid}=previous;
        let(old,updated)=table.insert_checked(key,value);
        slot.put(HashMap{table,valid:_});(old,_)
    }
    #[verified]
    pub fn get_checked(&self,key:&K)->(out:Option<&V>,@(match out{
        None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
        Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v)),
    })){let(out,found)=self.table.get_checked(key);(out,_)}
    #[verified]
    pub fn remove_checked(&mut self,key:&K)->(old:Option<V>,
        @(Self::model(snap(final!(self))).equivalent(Self::model(snap(self)).remove(K::key(snap(key))))
            && (match old{None=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::None,
                Some(v)=>Self::model(snap(self)).get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let(slot,previous,was)=mem::take_out(self);let HashMap{mut table,valid}=previous;
        let(old,updated)=table.remove_checked(key);
        slot.put(HashMap{table,valid:_});(old,_)
    }
}
// OPEN PROOF INTERFACE: Table's helpers above must TAKE representation as a
// precondition and RETURN its restoration, not merely abstract-map equations.
// As written, their current proof holes cannot be filled for arbitrary Table
// values; they are proof-development targets, not accepted verified functions.
// The facade shows where a caller owns that evidence and closes the invariant.
// No returned mutable element loan escapes this take_out/put operation.
```

## Wrapped maps

`locus/11-wrapped-maps.lc`

```rust
use crate::seq::Maybe;
use crate::map::Map;
use crate::std_api::{StdOrdBridge,StdHashBridge};
use locus::Opaque;

pub struct BTreeMap<K:StdOrdBridge,V>{raw:Opaque<std::collections::BTreeMap<K,V>>,model:Map<K::Key,Snap<V>>}
impl<K:StdOrdBridge,V:Snapshot> BTreeMap<K,V>{
    pub logic fn contents(s:Snap<Self>)->Map<K::Key,Snap<V>>{s.model}
    #[verified]
    pub fn empty()->(m:Self,@(m.model.len()==0)){
        (BTreeMap{raw:Opaque::new(std::collections::BTreeMap::new()),model:Map::empty()},_)
    }
    #[verified]
    pub fn insert_checked(&mut self,key:K,value:V)->(old:Option<V>,
        @(final!(self).model.equivalent(self.model.insert(K::key(snap(key)),snap(value)))
            && (match old{None=>self.model.get(K::key(snap(key)))===Maybe::None,
                Some(v)=>self.model.get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let before=logic!{self.model};let k=logic!{K::key(snap(key))};let v=logic!{snap(value)};
        let old=self.raw.get_mut().insert(key,value);
        let previous=unsafe assume!(match old{None=>before.get(k)===Maybe::None,Some(x)=>before.get(k)===Maybe::Some(snap(x))});
        self.model=logic!{before.insert(k,v)};
        (old,_) // TRUST STD-BTREE + exact Ord bridge to K::key.
    }
    #[verified]
    pub fn get_checked(&self,key:&K)->(out:Option<&V>,@(match out{
        None=>self.model.get(K::key(snap(key)))===Maybe::None,
        Some(v)=>self.model.get(K::key(snap(key)))===Maybe::Some(snap(v)),
    })){
        let out=self.raw.get_ref().get(key);
        (out,unsafe assume!(_)) // TRUST STD-BTREE; concrete proposition is result's.
    }
    #[verified]
    pub fn remove_checked(&mut self,key:&K)->(old:Option<V>,
        @(final!(self).model.equivalent(self.model.remove(K::key(snap(key))))
            && (match old{None=>self.model.get(K::key(snap(key)))===Maybe::None,
                Some(v)=>self.model.get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let before=logic!{self.model};let k=logic!{K::key(snap(key))};
        let old=self.raw.get_mut().remove(key);
        let previous=unsafe assume!(match old{None=>before.get(k)===Maybe::None,Some(x)=>before.get(k)===Maybe::Some(snap(x))});
        self.model=logic!{before.remove(k)};(old,_)
        // The model change is proved; raw/model correspondence is still trusted.
    }
    #[verified]
    pub fn insert(&mut self,key:K,value:V)->Option<V>{self.insert_checked(key,value).0}
    #[verified]
    pub fn get(&self,key:&K)->Option<&V>{self.get_checked(key).0}
}

pub struct HashMap<K:StdHashBridge,V>{raw:Opaque<std::collections::HashMap<K,V>>,model:Map<K::Key,Snap<V>>}
impl<K:StdHashBridge,V:Snapshot> HashMap<K,V>{
    pub logic fn contents(s:Snap<Self>)->Map<K::Key,Snap<V>>{s.model}
    #[verified]
    pub fn empty()->(m:Self,@(m.model.len()==0)){
        (HashMap{raw:Opaque::new(std::collections::HashMap::new()),model:Map::empty()},_)
    }
    #[verified]
    pub fn insert_checked(&mut self,key:K,value:V)->(old:Option<V>,
        @(final!(self).model.equivalent(self.model.insert(K::key(snap(key)),snap(value)))
            && (match old{None=>self.model.get(K::key(snap(key)))===Maybe::None,
                Some(v)=>self.model.get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let before=logic!{self.model};let k=logic!{K::key(snap(key))};let v=logic!{snap(value)};
        let old=self.raw.get_mut().insert(key,value);
        let previous=unsafe assume!(match old{None=>before.get(k)===Maybe::None,Some(x)=>before.get(k)===Maybe::Some(snap(x))});
        self.model=logic!{before.insert(k,v)};(old,_)
        // TRUST STD-HASH: Eq bridge and hash-input/state law, not just an Ord law.
    }
    #[verified]
    pub fn get_checked(&self,key:&K)->(out:Option<&V>,@(match out{
        None=>self.model.get(K::key(snap(key)))===Maybe::None,
        Some(v)=>self.model.get(K::key(snap(key)))===Maybe::Some(snap(v)),
    })){
        let out=self.raw.get_ref().get(key);(out,unsafe assume!(_))
    }
    #[verified]
    pub fn remove_checked(&mut self,key:&K)->(old:Option<V>,
        @(final!(self).model.equivalent(self.model.remove(K::key(snap(key))))
            && (match old{None=>self.model.get(K::key(snap(key)))===Maybe::None,
                Some(v)=>self.model.get(K::key(snap(key)))===Maybe::Some(snap(v))})))
    {
        let before=logic!{self.model};let k=logic!{K::key(snap(key))};
        let old=self.raw.get_mut().remove(key);
        let previous=unsafe assume!(match old{None=>before.get(k)===Maybe::None,Some(x)=>before.get(k)===Maybe::Some(snap(x))});
        self.model=logic!{before.remove(k)};(old,_)
    }
    #[verified]
    pub fn insert(&mut self,key:K,value:V)->Option<V>{self.insert_checked(key,value).0}
    #[verified]
    pub fn get(&self,key:&K)->Option<&V>{self.get_checked(key).0}
}
// These models specify value lookup, not which equivalent key representative is
// retained. Rust's insertion preserves the earlier stored key. An entries/keys
// iterator or get_key_value contract needs an additional representative relation.
// Borrow<Q> also needs a law tying the borrowed query's equality/order/hash to K.
```

## Interior mutability and shared ownership

`locus/12-interior-and-sharing.lc`

```rust
// Locus libraries over explicit protocol interfaces. Unique-memory Region is
// insufficient to implement these interfaces. Native algorithms implementing
// their runtime behavior are in src/interior.rs and src/sharing.rs. This file
// does NOT claim those unsafe algorithms have acquired proofs by being wrapped.
use locus::Opaque;

pub trait Invariant<T:Snapshot>{logic fn holds(x:Snap<T>)->Prop;}
#[derive(CloneEq,Copy)]
pub struct Certified<T:Snapshot+CloneEq+Copy,I:Invariant<T>+CloneEq+Copy>{
    value:T, valid:@I::holds(snap(value)),
    policy:Opaque<std::marker::PhantomData<fn()->I>>,
}
impl<T:Snapshot+CloneEq+Copy,I:Invariant<T>+CloneEq+Copy> Certified<T,I>{
    #[verified]
    pub fn new(value:T,valid:@I::holds(snap(value)))->Self{Certified{value,valid,policy:Opaque::new(std::marker::PhantomData)}}
    #[verified]
    pub fn read(&self)->(value:T,@I::holds(snap(value))){(self.value,_)}
}

// The explicit policy carrier keeps I present in erased Rust; proof occurrences
// alone need not count as using a Rust type parameter. CloneEq/Copy derivation
// here depends on the fixed Unit snapshot and forwarding traits of Opaque.
// A concrete sealed value such as Small in 13 avoids this generic carrier gate.

// Minimal extra interior primitive. It owns initialized memory, exports no
// reference, is !Sync, and permits Copy-read and whole-value replacement.
// Root snapshot is Unit, not its changing contents. Prototype: PCell<T>.
pub struct InteriorSlot<T>;
impl<T> InteriorSlot<T>{
    #[verified] pub fn new(x:T)->Self;
    #[verified] pub fn replace(&self,x:T)->T;
    #[verified] pub fn copy(&self)->T where T:Copy;
}
pub struct Cell<T:Snapshot+CloneEq+Copy,I:Invariant<T>+CloneEq+Copy>{
    slot:InteriorSlot<Certified<T,I>>,
}
impl<T:Snapshot+CloneEq+Copy,I:Invariant<T>+CloneEq+Copy> Cell<T,I>{
    #[verified]
    pub fn new_checked(x:T,p:@I::holds(snap(x)))->Self{Cell{slot:InteriorSlot::new(Certified::new(x,p))}}
    #[verified]
    pub fn get_checked(&self)->(x:T,@I::holds(snap(x))){let entry=self.slot.copy();entry.read()}
    #[verified]
    pub fn replace_checked(&self,x:T,p:@I::holds(snap(x)))->(old:T,@I::holds(snap(old))){
        let old=self.slot.replace(Certified::new(x,p));old.read()
    }
}
pub struct WrappedCell<T:Snapshot+CloneEq+Copy,I:Invariant<T>+CloneEq+Copy>{
    raw:std::cell::Cell<Certified<T,I>>,
}
impl<T:Snapshot+CloneEq+Copy,I:Invariant<T>+CloneEq+Copy> WrappedCell<T,I>{
    #[verified]
    pub fn new_checked(x:T,p:@I::holds(snap(x)))->Self{WrappedCell{raw:std::cell::Cell::new(Certified::new(x,p))}}
    #[verified]
    pub fn get_checked(&self)->(x:T,@I::holds(snap(x))){let entry=self.raw.get();entry.read()}
    #[verified]
    pub fn replace_checked(&self,x:T,p:@I::holds(snap(x)))->(old:T,@I::holds(snap(old))){
        let old=self.raw.replace(Certified::new(x,p));old.read()
    }
}
// Both implementations prove a value invariant, NOT "get returns the last set".
// An immutable ghost last_value field would be stale after mutation via an alias.
// Strong temporal facts need a separately owned state capability, below.

pub struct CellHandle<T>; // address/identity only, copyable, not permission
pub struct CellPermit<T>; // affine runtime resource with snap(permit).value
pub trait PermissionedCell<T:Snapshot>{
    fn new_checked(x:T)->(cell:CellHandle<T>,permit:CellPermit<T>,
        @(permit.cell===cell && permit.value===snap(x)));
    fn get_checked(cell:&CellHandle<T>,permit:&CellPermit<T>,same:@(permit.cell===cell))
        ->(x:T,@(snap(x)===permit.value)) where T:Copy;
    fn replace_checked(cell:&CellHandle<T>,permit:&mut CellPermit<T>,x:T,
        same:@(permit.cell===cell))->(old:T,
            @(snap(old)===permit.value && final!(permit).value===snap(x)));
}
// This is implementable using a separately held Region capability; it changes
// the programming model. Native std Cell aliases must not bypass that capability.

// Shared ownership kernel: a handle carries a non-duplicable ownership share.
// clone_share creates a new accounted share; last release destroys the payload.
// No Weak in this first API. Arc additionally needs an atomic memory-model proof.
pub struct Shared<T>;
pub trait ShareProtocol<T:Snapshot>{
    fn new_checked(x:T)->(s:Shared<T>,@(s.value===snap(x)));
    fn clone_checked(s:&Shared<T>)->(copy:Shared<T>,@(copy.id===s.id && copy.value===s.value));
    fn read_checked(s:&Shared<T>)->(x:&T,@(snap(x)===s.value));
    fn release(s:Shared<T>);
}
pub struct Rc<T>{share:Shared<T>}
impl<T:Snapshot> Rc<T>{
    #[verified]
    pub fn new_checked(x:T)->(r:Self,@(r.share.value===snap(x))){
        let(share,new)=LocalShares::new_checked(x);(Rc{share},_)
    }
    #[verified]
    pub fn clone_checked(&self)->(r:Self,@(r.share.id===self.share.id && r.share.value===self.share.value)){
        let(share,cloned)=LocalShares::clone_checked(&self.share);(Rc{share},_)
    }
    #[verified]
    pub fn get_checked(&self)->(x:&T,@(snap(x)===self.share.value)){
        let(x,read)=LocalShares::read_checked(&self.share);(x,_)
    }
}
// LocalShares and AtomicShares are REQUIRED implementations of ShareProtocol,
// not existing Locus items. Their runtime count algorithms are PRc and PArc.
// A library Drop hook must release the share without copying it or forging one.

pub struct WrappedRc<T>{raw:Opaque<std::rc::Rc<T>>,model:Snap<T>}
impl<T:Snapshot> WrappedRc<T>{
    #[verified]
    pub fn new_checked(x:T)->(r:Self,@(r.model===snap(x))){
        let model=logic!{snap(x)};(WrappedRc{raw:Opaque::new(std::rc::Rc::new(x)),model},_)
    }
    #[verified]
    pub fn clone_checked(&self)->(r:Self,@(r.model===self.model)){
        let raw=self.raw.get_ref().clone();
        (WrappedRc{raw:Opaque::new(raw),model:self.model},_)
        // TRUST STD-RC maintains SAME stable observed payload. Snapshot equality
        // alone does not prove allocation identity; no such promise is made here.
    }
    #[verified]
    pub fn get_checked(&self)->(x:&T,@(snap(x)===self.model)){
        let x=&**self.raw.get_ref();(x,unsafe assume!(snap(x)===self.model))
    }
}

// RefCell and Mutex expose an active guard, never an immutable "current value"
// snapshot of an unheld shared cell. Guards carry access rights, not just Nat IDs.
pub trait BorrowProtocol<T>{
    type ReadGuard;type WriteGuard;
    fn try_read(&self)->Option<Self::ReadGuard>;
    fn try_write(&self)->Option<Self::WriteGuard>;
    // Full declarations additionally tie guard lifetime to owner, state that
    // failed borrow changes nothing, and establish exclusive/shared authority.
}
pub struct RefCell<T>{gate:LocalBorrowGate<T>}
impl<T> RefCell<T>{
    #[verified] pub fn new(x:T)->Self{RefCell{gate:LocalBorrowGate::new(x)}}
    #[verified] pub fn try_borrow(&self)->Option<ReadGuard<'_,T>>{self.gate.try_read()}
    #[verified] pub fn try_borrow_mut(&self)->Option<WriteGuard<'_,T>>{self.gate.try_write()}
}
pub struct WrappedRefCell<T>{raw:std::cell::RefCell<T>}
impl<T> WrappedRefCell<T>{
    #[verified] pub fn new(x:T)->Self{WrappedRefCell{raw:std::cell::RefCell::new(x)}}
    #[verified] pub fn try_borrow(&self)->Result<std::cell::Ref<'_,T>,std::cell::BorrowError>{self.raw.try_borrow()}
    #[verified] pub fn try_borrow_mut(&self)->Result<std::cell::RefMut<'_,T>,std::cell::BorrowMutError>{self.raw.try_borrow_mut()}
}
// For a sealed Certified<T,I>, both can return a checked value from the guard's
// own invariant. Generic guard dereference needs a native access contract or an
// understood operation: executing Deref is NOT an implicit snapshot operation.
// Forgetting a RefMut keeps the dynamic borrow outstanding; it does not unlock.

pub struct Arc<T>{share:AtomicShare<T>}
impl<T:Snapshot+Send+Sync> Arc<T>{
    #[verified] pub fn new(x:T)->Self{Arc{share:AtomicShares::new_checked(x).0}}
    #[verified] pub fn clone(&self)->Self{Arc{share:AtomicShares::clone_checked(&self.share).0}}
    #[verified] pub fn get_checked(&self)->(x:&T,@(snap(x)===self.share.value)){
        AtomicShares::read_checked(&self.share)
    }
}
pub struct WrappedArc<T>{raw:Opaque<std::sync::Arc<T>>,model:Snap<T>}
impl<T:Snapshot+Send+Sync> WrappedArc<T>{
    #[verified] pub fn new(x:T)->Self{let model=logic!{snap(x)};WrappedArc{raw:Opaque::new(std::sync::Arc::new(x)),model}}
    #[verified] pub fn clone_checked(&self)->(r:Self,@(r.model===self.model)){
        (WrappedArc{raw:Opaque::new(self.raw.get_ref().clone()),model:self.model},_)
    }
    #[verified] pub fn get_checked(&self)->(x:&T,@(snap(x)===self.model)){
        let x=&**self.raw.get_ref();(x,unsafe assume!(snap(x)===self.model))
    }
}
pub struct Mutex<T>{gate:AtomicLockGate<T>}
impl<T> Mutex<T>{
    #[verified] pub fn new(x:T)->Self{Mutex{gate:AtomicLockGate::new(x)}}
    #[verified] pub fn lock(&self)->LockResult<LockGuard<'_,T>>{self.gate.acquire()}
}
pub struct WrappedMutex<T>{raw:std::sync::Mutex<T>}
impl<T> WrappedMutex<T>{
    #[verified] pub fn new(x:T)->Self{WrappedMutex{raw:std::sync::Mutex::new(x)}}
    #[verified] pub fn lock(&self)->std::sync::LockResult<std::sync::MutexGuard<'_,T>>{self.raw.lock()}
}
// OPEN PROTOCOL, not omitted runtime algorithm: AtomicLockGate must transfer
// protected permissions on acquire/release and establish happens-before. Poison
// cannot excuse exposing invalid proof fields. Storing a sealed invariant value
// avoids that problem; an arbitrary temporarily opened invariant needs more.
```

## Interop clients

`locus/13-interop-programs.lc`

```rust
// Locus clients. Each uses named checked methods when it needs a theorem.
// Library OPEN obligations remain dependencies, not facts this spike has proved.
use crate::seq::{Seq,Maybe};
use crate::map::Map;
use crate::native_vec;
use crate::wrapped_vec;
use crate::native_hash;
use crate::wrapped_maps;

#[verified]
fn native_stack()->(x:u32,@(x===20)){
    let(mut v,empty)=native_vec::Vec::empty();
    let a=v.push_checked(10);let b=v.push_checked(20);
    let(out,popped)=v.pop_checked();
    match out{None=>panic!("nonempty"),Some(x)=>(x,_)}
    // OPEN sequence lemmas connect push/get/take, not automatic sequence search.
}
#[verified]
fn wrapped_stack()->(x:u32,@(x===20)){
    let(mut v,empty)=wrapped_vec::Vec::empty();
    let a=v.push_checked(10);let b=v.push_checked(20);
    let(out,popped)=v.pop_checked();
    match out{None=>panic!("nonempty"),Some(x)=>(x,_)}
}

pub trait MapApi:Snapshot{
    logic fn model(s:Snap<Self>)->Map<Nat,Nat>;
    fn new_checked()->(m:Self,@(Self::model(snap(m)).equivalent(Map::empty())));
    fn insert_checked(&mut self,k:u32,v:u32)->(old:Option<u32>,
        @(Self::model(snap(final!(self))).equivalent(Self::model(snap(self)).insert(k,v))));
    fn get_checked(&self,k:&u32)->(v:Option<&u32>,@(match v{
        None=>Self::model(snap(self)).get(*k)===Maybe::None,
        Some(x)=>Self::model(snap(self)).get(*k)===Maybe::Some(*x),
    }));
}
#[verified]
fn overwrite_then_read<M:MapApi>()->(x:u32,@(x===12)){
    let(mut m,empty)=M::new_checked();
    let start=logic!{M::model(snap(m))};
    let(old,first)=m.insert_checked(4,11);
    let after_first=logic!{M::model(snap(m))};
    let(old,second)=m.insert_checked(4,12);
    // Explicit forall application; equality transport is not silently delegated
    // to `_`. The insert contract is pointwise map equivalence.
    let pointwise:@(forall |q:Nat|
        M::model(snap(m)).get(q)===after_first.insert(4,12).get(q))=_;
    let at_four=logic!{pointwise(4)};
    let written=logic!{crate::map::get_insert(after_first,4,12,4)};
    let(out,read)=m.get_checked(&4);
    match out{Some(x)=>(*x,_),None=>panic!("present")}
}
// MapApi can be implemented by either backend only after its public method
// proofs/bridges are established; no four blanket trusted impls are added here.

#[verified]
fn ordinary_native_handoff()->wrapped_vec::Vec<u32>{
    let(mut v,empty)=wrapped_vec::Vec::empty();
    let a=v.push_checked(3);let b=v.push_checked(1);let c=v.push_checked(3);
    let mut raw=v.into_std();raw.sort_unstable();raw.dedup();
    wrapped_vec::Vec::from_std(raw)
    // Ordinary interop is expressible. This signature deliberately promises no
    // sortedness/permutation fact; those require a checked native adapter.
}

#[derive(CloneEq,Copy)]
struct Small{value:u32,small:@(value<100)}
#[verified]
fn small(n:u32)->Small{let value=n%100;Small{value,small:_}}
#[verified]
fn certified_native_iterator(xs:&[u32])->Option<(x:u32,@(x<100))>{
    let values:std::vec::Vec<Small>=xs.iter().map(|x|small(*x)).collect();
    match values.first(){None=>None,Some(x)=>Some((x.value,_))}
}
#[verified]
fn certified_native_cell(n:u32)->(x:u32,@(x<100)){
    let c=std::cell::Cell::new(small(n));let got=c.get();(got.value,_)
}
#[verified]
fn certified_refcell(n:u32)->(x:u32,@(x<100)){
    let c=std::cell::RefCell::new(small(n));
    let guard=c.borrow();let got=&*guard; // Execute Deref first; then capture.
    (got.value,_)
}
#[verified]
fn certified_mutex(n:u32)->(x:u32,@(x<100)){
    let c=std::sync::Mutex::new(small(n));
    let guard=match c.lock(){Ok(g)=>g,Err(e)=>e.into_inner()};
    let got=&*guard;(got.value,_)
    // Poison does not invalidate a sealed proof field. These examples need
    // boundary/lifetime lowering tests; no concurrent history is asserted.
}

// NEGATIVE CLIENTS, not accepted litmus outcomes:
// 1. Passing Vec<(value,@P)> through native mutable APIs exposes logical data.
// 2. Keeping an old map contents proof after an uncontracted native mutation.
// 3. Using a bound proof for the old vector length after pop without transport.
// 4. Inferring the last Cell write from equality of Unit snapshots.
// 5. Equating two map key representatives from equality of KeySpec projections.
```
