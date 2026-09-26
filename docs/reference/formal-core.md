+++
id = "formal-core"
title = "Formal core"
group = "Now"
created = "2026-09-23T04:03:42.000Z"
updated = "2026-09-23T04:03:42.000Z"
route = "reference/formal-core.html"
order = 14
description = "A calculus for checked execution, lowering, permissions, and erasure; mechanization remains future work."
+++

# Formal core: checked execution, lowering and erasure

<!-- spec: 3.0:1 informative -->
This is the current compiler's small calculus, not a mechanized proof of its correctness. It states the trusted judgments outside the logical kernel and the preservation obligations still to prove. The concrete syntax below is metalanguage; names in backticks identify Rust constructors. Every rule cites the permanent paragraph IDs of the [language manual](../spec/01-introduction.md) and the corresponding [Architecture](../architecture.md) section. The kernel's own typing, proof and term-comparison rules remain in the [kernel contract](kernel.md); they are premises here, never an implicit solver. The executable oracle for the erased calculus is `erased::Interpreter`; `exec::CheckInterpreter` is the reference for the check IR. Neither implementation wins a disagreement by definition.

## 1. Judgments and grammar

<!-- spec: 3.1:1 informative -->
`D` is the checked logical declaration environment. `F` contains the already accepted ordinary functions and recorded trusted adapters. `G` is an ordered context of fresh variable identities `x:T` and established hypotheses `h:P`; source spellings are not identities. A telescope `(x1:T1, ..., xn:Tn)` checks left to right, substituting earlier arguments into later types. `K(G,t):T` and `K(G,p):@P` mean the kernel accepts a term or proof. A block judgment also carries the enclosing function result `R`, its promises `Π`, a stack `L` of loop targets `(state,result)`, and an expected local result `Q` or `none`. Separately checked physical layouts `A` distinguish surface `bool` from logical `Bool`, physical containers from logical inductive data, and reference origins. `K` alone does not discharge the layout/permission judgment. [Language 1.2:1–3, 1.3:2, 1.5:5; Architecture “Logical classification and physical layout”, “Context, snapshots and scope”.]

<!-- spec: 3.1:2 syntax -->
~~~text prose check-ir-metalanguage
Block  ::= Stmt* ; Tail
Stmt   ::= Let(x,h,T?,t) | Have(h,P,p) | Call(x,f,args)
         | Match(x,T,t,arms) | Loop(x,S,xs,init,R,body)
         | For(x,i,hlo,hhi,lo,hi,inclusive,S,xs,init,body)
         | Operate(x,h,op,M,args,fits?,learned)
         | Buffer(x,h,op,storage,T,logical_payload,args,bounds,learned)
         | BoxNew(x,h,t,logical_payload)
Tail   ::= Value(t) | Break(t) | Continue(ts) | Match(t,arms)
         | Return(t) | Panic(message,unreachable?) | Foreign(path,args,T)
         | DynPack(table,t) | DynCall(interface,slot,receiver,args)
Arm    ::= (payload identities, branch hypothesis identity, Block)
Storage::= Array(n) | Slice | Vector
BufferOp ::= Literal | Length | Get | Set | Push
Promises ::= subset {terminates, no_panic, no_alloc, no_io}
~~~

<!-- spec: 3.1:3 informative -->
This grammar covers every variant of `exec::Stmt`, `exec::Tail`, `BufferStorage` and `BufferOp`, including the fields carried by `ForStmt`, `OperateStmt` and `BufferStmt`. `tests/formal_core.rs` checks that inventory against the declarations rather than treating the displayed list as permanent. Pure terms, types, proofs, nominal constructors, lambdas and logical recursion are the Kernel contract's grammar. Source struct/tuple construction, projections, casts and wrapping arithmetic become those checked terms; effectful arithmetic stays `Operate`. [Language 1.4:1–2, 1.5:1–5, 1.11:1–3, 1.15:2–3, 1.26:1; Architecture “Representations and pipeline”, “Lowering ordinary execution”.]

## 2. Static check-IR rules

<!-- spec: 3.2:1 legality-rule -->
**IR-Bind.** To check `Let(x,h,T?,t)`, infer the permitted value with the kernel, check an optional annotation against the inferred type, bind fresh `x`, and add its defining equation when the type admits equality. Proof values carry their proposition without an equality between proof objects. `Have(h,P,p)` requires `K(G,p):@P` before adding fresh `h:P`. Context extension is ordered and freshness is mandatory. Logical lets introduce facts, not runtime effects. At the end of a lexical block, its local bindings are removed from the checking context; a result type must already be well formed in its receiving context. [Language 1.2:1–3, 1.7:5; Architecture “Context, snapshots and scope”, “Lowering ordinary execution”. Implementation: `exec/check.rs::check_stmt`, `kernel/context.rs::define_with`.]

<!-- spec: 3.2:2 legality-rule -->
**IR-Call.** For `Call(x,f,args)`, find an already registered signature `(params)->T`, check the arguments in executable mode against the dependent parameter telescope, and require every promise made by the caller to be made by `f`. Bind `x:T[args/params]`. Introduce no equation between `x` and `f(args)`: an ordinary function is not a logical definition. Its proof fields are available through its declared result type. Ordinary recursive call graphs are unavailable in this tier. A trusted adapter uses the same call rule but its accepted contract is an explicit recorded assumption; it is never silently upgraded into a theorem. [Language 1.5:2, 1.9:1–4, 1.26:3; Architecture “Lowering ordinary execution”, “Explicit trust boundary and audit”. Implementation: `exec/check.rs`.]

<!-- spec: 3.2:3 legality-rule -->
**IR-Match.** Require an executable bool or physical enum scrutinee, one arm per constructor, and each arm's exact payload telescope. For a nominal family, instantiate the payload telescope with the scrutinee type’s checked logical indices; include the same indices in its constructor equation. In arm `C`, extend `G` by payload identities and the checked equation `scrutinee = C(payload)`, then check its block. Logical evidence may use that equation. Check a statement match's result type outside the arms; every value-producing arm must supply it, while a control-transferring arm has no joining value. Tail `Match` passes the enclosing block's expected result and targets to its arms. Physical layout checking separately rejects an erased discriminant even where the kernel representation is Bool. [Language 1.4:2, 1.5:4–5, 1.11:1, 1.12:2; Architecture “Logical classification and physical layout”, “Lowering ordinary execution”. Implementation: `exec/check.rs::check_arms`, `typed/layout.rs`, `erased/check.rs`.]

<!-- spec: 3.2:4 legality-rule -->
**IR-Loop.** `Loop(x,S,xs,init,R,B)` requires a tuple telescope `S` and a result `R` formed in the outer context. Check `init:S`, then check `B` under fresh abstract state identities `xs:S` and target `(S,R)`, with expected local result `none`. Thus every path must continue with a new `S`, break with `R`, return from the function, or panic. Only after checking all paths bind the loop result `x:R`. The state is not assumed equal to its initial value in the arbitrary iteration. Evidence fields of `S` are exactly the loop invariant: checked at entry and on every back edge. This is partial correctness; no decreasing measure is required for ordinary execution. [Language 1.11:2, 1.12:3; Architecture “Lowering ordinary execution”. Implementation: `exec/check.rs::check_stmt`, `declare_state`.]

<!-- spec: 3.2:5 legality-rule -->
**IR-For.** `For` checks both bounds at the same executable machine type, forms state `S` in the outer context, and checks `init:S`. The body receives fresh `i`, fresh state, and `view(lo) <= view(i)` together with `view(i) < view(hi)` (or `<=` for an inclusive range); its target is `(S,S)`. It must not fall through. The result is `S`. Neither checking nor execution assumes the initial bounds are ordered: an empty or reversed range has zero iterations. The bound expressions have already run once before this statement. [Language 1.11:2, 1.15:3; Architecture “Lowering ordinary execution”. Implementation: `exec/check.rs::check_stmt`, `exec/ir.rs::ForStmt`.]

<!-- spec: 3.2:6 legality-rule -->
**IR-End.** `Value(t)` requires a local expected result and `t` of that type. `Break(t)` requires a nearest loop and its result type; `Continue(ts)` checks its state telescope. `Return(t)` checks the enclosing function's result type instantiated at entry parameters, irrespective of nesting. `Panic(message,None)` has no value and is admitted unless the function promises `no_panic`; `Panic(message,Some(p))` always checks `p:False`. Under `no_panic` this evidence is mandatory. A panic is not evidence that a postcondition held. [Language 1.10:1–2, 1.11:2–3; Architecture “Lowering ordinary execution”. Implementation: `exec/check.rs::check_block_in_scope`.]

<!-- spec: 3.2:7 legality-rule -->
**IR-Operate.** Check the machine row, operand types and arity. Check any `fits` evidence in the incoming context, before binding the result or its learned facts; every premise must match that row. `no_panic` requires this evidence. The operation returns its exact representable value or panics, independently of Rust build flags. On normal continuation, bind the result and its wrapped equation. Overflow rows may introduce one exact Int-view equation; with `fits`, derive it from `OpExact`, otherwise it is the operational normal-return rule. Division/remainder introduce their checked nonzero and signed-overflow exclusions. Reject any other learned-fact count. Erasure retains whether safety was proved: ordinary operators require that proof; unchecked cases emit `checked_*().expect(...)`. [Language 1.15:2–3, 1.27:3, 1.92:8. Implementation: `exec/check.rs::check_operate`, `erased/rust.rs`.]

<!-- spec: 3.2:8 legality-rule -->
**IR-Storage.** `BoxNew` requires a permitted payload observation and checked payload layout, preserves physical allocation even for a logical payload, and on normal return binds `x:Box<T>` with `x = Boxed(payload)`. It violates `no_alloc` and the current conservative `no_panic` allocation policy. `Buffer` uses the following table; every argument is checked in its physical or explicitly erased payload mode before any result fact is added. Storage shape, operand arity, result layout, bound-proof count, and learned-fact count must match. [Language 1.5:4–5, 1.26:1, 1.26:6; Architecture “Calls, borrowing and native storage”. Implementation: `exec/buffer.rs`, `typed/buffer.rs`, `typed/shared.rs`; Kernel 2.35, 2.36.]

<!-- spec: 3.2:9 legality-rule -->
| Buffer operation | Premises | Normal-return result and meaning |
|---|---|---|
| `Literal` | Exact array length; no slice allocation; each element fits the retained payload layout. | Buffer snapshot containing the element sequence. Nonempty vector literal may allocate. |
| `Length` | Existing physical buffer. | usize result whose Int view equals snapshot length. |
| `Get` | Buffer, usize index, exactly two checked proofs `0 <= view(index)` and `view(index) < length(buffer)`. | Element at that snapshot position; logical elements yield logical values only. |
| `Set` | Same bounds as Get, plus a replacement element. | New immutable content snapshot with that position replaced; the caller's mutation/writeback implements physical update. |
| `Push` | Vector storage and an element. No caller-supplied bound proof. | May allocate/fail. Only on normal return introduce the fact `old_length < usize::MAX`, then the appended snapshot equation. |

<!-- spec: 3.2:10 legality-rule -->
**IR-Promises.** All calls satisfy promise inclusion. `terminates` additionally rejects every `Loop` and `For`, even an obviously finite one; there is no ordinary recursion. `no_panic` checks the endings/operators/storage rules above. `no_alloc` rejects allocating storage operations; `no_io` allows only callees promising no I/O. Native implementation/specification assumptions are recorded separately. This is the implemented rule, not an inference of the strongest effects. [Language 1.9:2–4, 1.26:3; Architecture “Lowering ordinary execution”, “Explicit trust boundary and audit”. Implementation: `exec/check.rs`, `exec/buffer.rs`.]

<!-- spec: 3.2:11 legality-rule -->
**IR-Foreign.** `Foreign(path,args,T)` has no effect promises. Every argument and `T` must be a physical bool, supported machine integer or a tuple recursively containing those types; `T` equals the expected result. The boundary cannot return proofs, logical values or nominal invariant-bearing data. A fresh native result carries no equation to a logical function. Native signature attestation and path identity are compiler/toolchain obligations; this form adds no kernel rule. Erasure yields `NativeCall` with the same ordered physical arguments and result. Its independent checker rejects logical positions again. Generated Rust performs the actual call. Both interpreters report `RunError::Native` after argument evaluation, an unsupported execution observation rather than a language panic or agreement result. [Language 1.30:3–8; Architecture “Plain Rust imports”.]

## 3. Dynamic interpretation

<!-- spec: 3.3:1 dynamic-semantics -->
Write `run(B,σ,H,mode) -> (control,H')` for evaluation with identity-to-value environment `σ`, physical storage `H`, and overflow mode `Checks` or `Wrap`. Controls are `value(v)`, `break(v)`, `continue(vs)`, `return(v)`, and `panic(message)`; infinite execution has no finite result. An implementation's fuel/depth exhaustion is an inconclusive observation, not another language result and not proof of divergence. Logical values are observed only through their erased marker. `Let` binds its permitted value, `Have` does no runtime work, and ordinary `Call` executes its callee even when its result erases. Statements run in order until a control transfer; block-local environment entries leave scope, while completed physical writes remain. [Language 1.5:1–4, 1.10:2, 1.18:1–2, 1.19:2; Architecture “Representations and pipeline”, “Agreement and the trusted base”.]

<!-- spec: 3.3:2 dynamic-semantics -->
`Match` runs exactly the arm selected by the physical tag, binding its payloads; its equation is logical only. `Loop` starts at `init`, replaces the state on `continue`, yields a value on `break`, and propagates return/panic. `For` visits the machine range in increasing order without an overflowing increment past its final endpoint; zero visits return `init`; continue advances the state, break yields it, and return/panic propagate. `Operate` runs the corresponding Rust machine operation in the selected overflow mode. `BoxNew` and `Buffer` perform their physical operations; normal-return equations do not predict that allocation succeeds. `Value`, `Break`, `Continue`, `Return` and `Panic` produce their named controls. [Language 1.10:1–2, 1.11:1–3, 1.15:2, 1.26:1; Architecture “Lowering ordinary execution”, “Calls, borrowing and native storage”.]

<!-- spec: 3.3:3 informative -->
The check-IR interpreter stores immutable value snapshots and uses the lowering-provided Lending/projection metadata to report physical `&mut` exits, including writes completed before a panic. The erased interpreter performs mutable storage operations on the source-shaped tree. This difference makes them useful independent oracles, not interchangeable implementations of the same traversal. There is no promise here that the test harness simulates host OOM/abort, allocator addresses, destructor behavior not yet admitted, or arbitrary external I/O. The native-storage correspondence is conditional on the admitted Rust native semantics; partial correctness constrains normal return. [Language 1.13:2, 1.19:2, 1.24:1; Architecture “Calls, borrowing and native storage”, “Agreement and the trusted base”.]

## 4. Typed lowering

<!-- spec: 3.4:1 legality-rule -->
**L-Eval.** The lowering judgment `lower(e,G,V,A) = (stmts,t,V')` sequences every ordinary operand/argument computation left to right, then returns its named result term. `V` maps each mutable storage-root identity to its current immutable version. A pure executable expression becomes a kernel term; an ordinary call, panicking operator, storage operation, loop or control transfer becomes ordered IR. The lowering of an effectful expression cannot be removed because its result is Logical. A logical application keeps its ordinary argument computations and introduces only an erased logical result. Explicit logic blocks admit no such effects. Every executable read must name the current version; an old version is valid only as a logical snapshot, never as a silently stale physical read. [Language 1.5:1–5, 1.12:1; Architecture “Lowering ordinary execution”. Implementation: `typed/lower.rs`.]

<!-- spec: 3.4:2 legality-rule -->
**L-Write.** Lower assignment's right-hand side first. Then find the destination root/path, rebuild the old immutable root with that field replaced, and emit `Let(fresh_version,eq,root_type,rebuilt)`. Update `V(root)` to the fresh identity; never overwrite an old context binding. A field replacement checks the entire dependent root, so it cannot silently invalidate a proof field. Shadowing allocates a distinct root. **L-Tracked.** A tracked evidence binding's current expected claim is its declared claim with captured mutable dependencies substituted by `V`. Refreshing it is a checked assignment; an older proof remains evidence only about older snapshots. [Language 1.12:1, 1.12:3; Architecture “Context, snapshots and scope”, “Lowering ordinary execution”. Implementation: `typed/lower.rs::rebuilt`, `Versions::version_type`.]

<!-- spec: 3.4:3 legality-rule -->
**L-Join.** Independently compute roots assigned on arms that can reach the continuation. In declaration order, build a dependent tuple of their post-arm versions followed by the expression result. Each reaching arm returns its own versions, using its entry version for a root it did not change. Transferring arms contribute no tuple. Check the tuple type outside the arms, substituting earlier joined fields in later proof fields. Bind the joined versions by projections and update `V`. Reject a typed tree whose claimed join roots disagree with the independently computed set. Ordinary lexical blocks keep writes to outer roots but close their local names. [Language 1.3:2, 1.11:1, 1.12:2; Architecture “Context, snapshots and scope”, “Lowering ordinary execution”. Implementation: `typed/lower.rs::join_type`, `block_leaves`.]

<!-- spec: 3.4:4 legality-rule -->
**L-Iteration.** Independently compute outer roots assigned by the body (and a while condition), then form their state telescope in declaration order. Shadowed/body-local roots are not carried. A stale proof binding may be omitted, in which case any refreshed local version cannot escape as loop state. Supply current entry versions as `init`, abstract versions to the body, current versions on continue and break edges, and project exit versions from the loop result. An unbounded loop appends its break value after carried fields; a while/for carries unit control result. `while c {B}` lowers to a loop whose body evaluates `c` anew and branches to break or `B; continue`; for bounds run once. No ordinary loop is a kernel logical loop. [Language 1.11:2, 1.12:3; Architecture “Lowering ordinary execution”. Implementation: `typed/lower.rs::carried_bindings`.]

<!-- spec: 3.4:5 legality-rule -->
**L-Lend.** A call-scoped shared parameter is a referent snapshot. A mutable parameter is an entry value plus an exit value in the check-IR function result: `(mut_exit_1,...,mut_exit_n,declared_result)`, a dependent telescope. `old!(x)` selects the entry identity; result claims use the appropriate named exit/input binders. A call evaluates arguments in order, rejects overlapping lends whenever either is mutable, binds this exit tuple once, then rebuilds each caller root with its exit field in argument order. Normal return and early return produce the same exit shape. Panic has no normal result tuple, but already completed physical writes remain and Lending metadata lets the IR oracle report them. [Language 1.9:1, 1.11:3, 1.13:1–2; Architecture “Calls, borrowing and native storage”, “Lowering ordinary execution”. Implementation: `typed/lower.rs::exec_result`, `exec::Lending`.]

<!-- spec: 3.4:6 legality-rule -->
**L-Permission.** Before snapshots erase reference syntax, check each shared-reference leaf's origin set `(root,path,creation_position,lifetime)`. A read, including a logical observation, requires a live origin with no intervening overlapping write/move. Copying or transporting a reference preserves its origins; branch joins conservatively merge them. A result cannot refer to dead local/arm storage or a shorter lifetime. Shared dereference cannot move out non-Copy data; references hidden in unsupported owned containers, stored mutable aliases and interior mutation are rejected. Physical Box dereference follows its allocation root and owned consumption rules. This judgment is separate from snapshot equality: equal contents do not imply the same permission. [Language 1.16:1, 1.25:5, 1.26:2, 1.26:4; Architecture “Logical classification and physical layout”, “Calls, borrowing and native storage”. Implementation: `typed/shared.rs`, `typed/layout.rs`.]

## 5. Erasure

<!-- spec: 3.5:1 legality-rule -->
**E-Type.** Erasure is indexed by a checked layout, not only by the kernel type. A Logical position maps to private zero-sized `Erased`. Runtime tuples/structs retain positional fields; ordinary enums retain their tags; nominal-family applications erase to their base representation without evaluating their logical indices; recursive runtime enums retain their Box indirection. Physical Box, Vec, arrays and slices remain physical even with Logical payloads. Shared references retain their lifetime/layout and permission-checked referents. The internal distinction between `Proved` and `Ghost` erasure nodes prints as the same marker. The erasure checker rejects a physical use of a logical position, including nested tuple/enum projections. [Language 1.3:1, 1.5:5, 1.18:1, 1.26:6; Architecture “Logical classification and physical layout”, “Erasure and cleanup”. Implementation: `typed/layout.rs`, `erased/erase.rs`, `erased/check.rs`.]

<!-- spec: 3.5:2 dynamic-semantics -->
**E-Expr.** Retain zero-sized storage for a logical local that runtime code borrows; removing its logical contents must not leave a dangling physical place. Keep literals, physical constructors/projections, ordinary calls, borrowing, assignments, operations, storage allocation, and physical control in their source shape after recursively erasing logical positions. Erase logical definitions, propositions, certificates, model observations and logical applications after retaining eager ordinary subexpression effects exactly once and in order. A physical conditional yielding Logical values still runs its condition and only its selected arm's effects; erasure must not concatenate both arms. Logical boolean operators in ordinary code retain their eager ordinary operands rather than introducing a physical short circuit. Map SSA versions back to their physical root binding; do not emit the check IR's join/state/writeback tuples as production storage. [Language 1.5:1–5, 1.18:1–2; Architecture “Erasure and cleanup”. Implementation: `erased/erase.rs::effects_then`, `expr_with_layout`.]

<!-- spec: 3.5:3 legality-rule -->
**E-Clean/Export.** Remove an unused erased binding or pattern component only while retaining its effectful initializer as a statement; preserve panic/divergence, mutation and allocation. A nonreturning initializer becomes a terminal expression, not an unreachable marker tail. Privacy is part of the boundary: a single marker does not carry proposition identity in Rust, so exported evidence-accepting entry points and writable invariant fields must pass the dedicated anti-forgery checks. Safe Rust callers construct verified values through the accepted exported interface, not arbitrary markers. [Language 1.17:1, 1.18:3–4, 1.19:1; Architecture “Erasure and cleanup”. Implementation: `erased/cleanup.rs`, `erased/rust.rs`, `elab/items.rs::check_exported_fn`, `check_exported_struct`, `build.rs`.]

<!-- spec: 3.5:4 dynamic-semantics -->
**E-Facade.** For an exportable ordinary function with original erased return type `R`, construct the proof/tuple projection `pi_R` specified by Language 1.28:18. Retain the original implementation and signature privately. Its public facade evaluates one call on the forwarded physical arguments, then returns `pi_R(result)` on normal return. The projection only discards proof markers, never physical fields or tags; retained fields move once. Panic/divergence and completed writes are inherited from the same call. Source calls still use `R`, and logical input/public-field restrictions remain in force. No projection is applied to a nominal or callable interface. [Implementation: `project/export.rs`, `erased/facade.rs`, `erased/rust.rs`.]

## 6. Preservation obligations and agreement

<!-- spec: 3.6:1 informative -->
The intended theorem is conditional, not an implementation claim already proved: if declaration checking, layout/permission checking, lowering and IR checking accept `e:R`, the initial physical store realizes its parameter telescope, and the recorded native/foreign contracts hold, every normal execution result realizes `R` and its exposed evidence claims. Panic and divergence do not establish the normal postcondition. A separate simulation must show that `lower(e)` and `erase(e)` preserve the same physical result, completed writes and panic behavior, modulo erased values and logical stuttering, in each overflow mode. It must account for dependent joins, reference origins, ordinary calls with erased results, and writebacks interrupted by panic. For generated facades, additionally show that the public result is `pi_R` of the internal normal result with the same observable effects; Rust callers cannot reach the omitted-proof implementation or feed an erased marker back as evidence. A kernel soundness theorem alone does not imply this simulation. [Language 1.5:2, 1.13:2, 1.18:2, 1.23:1; Architecture “Agreement and the trusted base”.]

<!-- spec: 3.6:2 normative -->
Agreement has no designated winner: Language prose, this calculus, check-IR execution, erased execution and emitted Rust are views of the same intended behavior. A failure report identifies the disagreeing pair and reproducing input/seed, mode, relevant output and rule IDs; an oracle may itself be wrong. `tests/exec_check.rs`, `exec_endings.rs` and `exec_operations.rs` exercise individual IR judgments; `typed_lower.rs` and reference tests exercise version/join/loop/writeback rules; `erasure_layout.rs`, `reconcile_logic.rs`, `collections.rs`, `reconcile_box.rs` and `rust_output.rs` exercise erasure and native boundaries. `random_programs.rs`, `differential.rs` and the corpus compare the two interpreters and generated Rust in both overflow builds. A timeout/fuel/depth exhaustion is inconclusive and must not be counted as agreement. Allocation failure is not exhaustively explored by these finite tests. [Language 1.19:2, 1.23:1; Architecture “Agreement and the trusted base”.]

## 7. Future Lean deliverables

<!-- spec: 3.7:1 informative -->
Mechanization is intentionally deferred. Its first checked-in deliverable must freeze a reviewed logical axiom manifest and separately enumerate the native/foreign assumptions; it must not assume the erasure theorem it intends to prove. Encode the telescope/context calculus, every IR constructor, both overflow modes, physical permission/layout judgments, lowering and erasure relations. Prove weakening/substitution and context realization, then each checking rule's preservation, then lowering and erasure simulations, including panic-state writebacks and control transfer. For the admitted fragment, mechanize executable evaluators and connect their bounded observations to the relations. A fixed primitive manifest is distinct from optional classical reasoning; dependencies must report both honestly.

<!-- spec: 3.7:2 informative -->
The trust report must print `#print axioms` for every exported correctness theorem, count `sorry`/`admit` and unexpected axioms (required zero in the validation target), identify any intentionally assumed native contract by reason, and record Lean/toolchain versions and source revision. Generate an index `calculus rule ID -> Lean declaration -> Language paragraph ID -> Rust implementation -> focused test`; CI must reject missing rows and stale declaration names. Keep interpreter dependencies outside the axiom manifest. The current file-level compiler trusted-base report conservatively counts mixed files in full, including the export boundary currently housed in the elaborator; it must not be relabeled as just the proof kernel.

<!-- spec: 3.7:3 informative -->
A future `tools/validate-formal` entry point must run from a clean checkout with the pinned toolchain and documented cache, check the fixed axiom allowlist, build the mechanization, regenerate the rule index/trust report, run deterministic IR/erasure/native boundary examples in both overflow modes, and emit a reproducible summary. Publish hardware/cache prerequisites and measured clean/cached durations, with a target of completing the reader's validation in 30 minutes. A wall-clock deadline is only the validation harness's inconclusive stop policy, never a proof-search or language-acceptance rule. CI archives the index, axiom report, sorry count, toolchain versions and test summary. These are future deliverables, not files claimed to exist today.


## Canonical observation elaboration

<!-- spec: 3.8:1 legality-rule -->
**Model selection.** The environment maps each physical source type and compatible storage shape to at most one checked logical definition. A slice observer also covers compatible array/vector borrows; overlapping implementations are rejected. Primitive entries are reserved. Logical values use identity. `#[derive(Model)]` constructs a checked logical product from field models; it introduces no model axiom.

<!-- spec: 3.8:2 legality-rule -->
**Observation boundary.** Ordinary logical named-field selection first resolves the receiver's model. `model!(p)` instead checks a physical path under current read permissions, selects its snapshot term, then applies the selected field's canonical model. `old!` may select an entry path. Index expressions remain logical and require bounds evidence. Model evaluation neither creates a physical loan nor evaluates runtime code. Logical parameter declarations and arguments normalize outer shared references to the observed contents, retaining permission checks and snapshot identity. Physical parameters preserve the source type; logical parameters select its canonical model. Effectful argument evaluation is named once before repeated model projections. Logical `match &p` inspects that representation rather than recursively invoking its model.

<!-- spec: 3.8:3 legality-rule -->
**Natural representation.** Source `Nat` lowers to a checked Logical product `(value: Int, nonnegative: @(0 <= value))`. An unsigned observation constructs this product using its existing checked machine-range law. Nat arithmetic projects the integer values and constructs a result with checked nonnegativity evidence. Subtraction requires that evidence from the caller's context. Widening projects `value`; conversion in the other direction requires constructing the product. No new arithmetic axiom is introduced.

## Opaque spec realization obligation

<!-- spec: 3.9:1 informative -->
Type-spec lowering checks a unique package-owned representation and generates a distinct nominal wrapper plus ordinary checked adapters. Signature matching retains logical mode, binders and evidence. The compiler must preserve invocation count, sequencing, mutable snapshots and ownership through wrapping/unwrapping; representation methods and fields must not become accessible through the public spec. No generated body or header is an axiom. Generic bodies are checked on instantiation; this is not a universal generic soundness claim. Unsupported borrowed/container conversions fail closed. These are compiler preservation obligations tested by spec, IR and hostile Rust-client regressions; they are not yet a mechanized lowering proof.

## Target parameter and positional structs

<!-- spec: 3.10:1 legality-rule -->
All judgments above are relative to an immutable selected pointer width (32 or 64). Definitions, Program and erased Module share it. Machine types record pointer width explicitly; buffer bounds and index/result types use the selected usize. The erased checker rejects physical types for a different width. A generated compile-time Rust guard enforces the same width even when all dependent proofs erase. Proof-store keys include width, and certificates still undergo kernel checking in the actual context.

<!-- spec: 3.10:2 dynamic-semantics -->
Tuple/unit structs add source/erased shapes but no check-IR form or proof rule. A successful irrefutable pattern substitutes checked field projections in declaration order. The source checker establishes field privacy and move permissions. Erasure replaces an entirely logical pattern with a wildcard and erases its bound values, preserving evaluation of any ordinary producer. Physical constructor/pattern syntax retains the nominal declaration's shape.

## Concrete trait elaboration

<!-- spec: 3.12:1 informative -->
For each selected pair `(Trait, ConcreteType)`, substitute Self and associated bindings, match the declared parameter/result telescope, and check the actual body, including inherited defaults. Replace each selected call with that checked function identity before the existing lowering judgment. The trait header alone adds no axiom to the logical environment. Correctness requires selection to preserve trait provenance, scope and coherence before erasure; Rust trait forwarding must call the same checked body once. Physical-only export does not apply proof-result projection to trait signatures. This is an additional source-to-IR preservation obligation, not a new kernel proof rule.

## Bounded specialization

<!-- spec: 3.12:2 legality-rule -->
A generic instantiation substitutes source types only after satisfying its declared trait and associated-type requirements. Calls selected through an abstract bound retain that trait member identity across substitution; unrelated inherent members cannot replace them. Conditional implementations and methods become available only when their requirements hold. Each resulting body still passes the ordinary checking, ownership, logical classification and erasure judgments. Associated Logical requirements remain obligations even without method calls. No runtime dictionary or kernel axiom is introduced, and this does not establish universal checking of unused templates. Open generic Rust exports must not silently discard these constraints.

## Borrowed dynamic dispatch

<!-- spec: 3.12:3 legality-rule -->
A dynamic interface declares a fresh opaque kernel snapshot type and fixed scalar/tuple method signatures. A dispatch table binds one physical nominal receiver type to previously checked functions. The checker requires one table per interface/concrete-type pair, exactly one target per slot, and exact parameter/result types after replacing the receiver with that concrete type.

<!-- spec: 3.12:4 dynamic-semantics -->
**IR-Dynamic.** `Tail::DynPack` checks its operand against the table's concrete type and returns the interface snapshot type. `Tail::DynCall` checks the opaque receiver, slot and arguments, and returns that slot's physical result. A dynamic call permits no effect promises and creates no observer equation or evidence.

<!-- spec: 3.12:5 dynamic-semantics -->
The erased checker independently validates interface identities, target signatures, shared receiver passing, argument/result layouts and unsized positions. A pack adapter maps an input shared reference to an output shared reference with the same lifetime. Existing provenance checking governs all callers. Rust emission creates a private trait per fixed interface shape and checked forwarding implementations; both interpreters retain the concrete value and table identity.
