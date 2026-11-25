# RFC 0006: Traits

- **Status:** draft
- **Issue:** [traits-on-logical-types](../../issues/traits-on-logical-types.md)
- **Supersedes:** none. It changes rules of [RFC 0002](0002-logical-layer.md), which is still being decided: `logic.ghost.no-trait-impls`, `logic.ghost.marker-traits`, `logic.bool.as-proposition`, `models.capture.bound`, and `models.bounds-on-functions`. It rests on the lowering of [RFC 0001](0001-compiler-pipeline.md), and changes nothing in it.

This RFC is a draft. It has had three reviews outside the repository, as four drafts, and none of the text as it stands here. No compiler has run any of its examples: each is worked from the rules by hand. What it says of rustc comes from the spikes [logical-markers](../../spikes/logical-markers/README.md), [marker-lowering](../../spikes/marker-lowering/README.md), and the three rounds of [skeleton-typing](../../spikes/skeleton-typing-3/README.md). [What the reviews changed](#what-the-reviews-changed) lists the differences from the earlier drafts.

## Summary

There is one set of traits, and it is Rust's. A trait is implemented for a runtime type or for a logical type, `Logical` is one bound among others, and rustc does all of the resolution: Locus has no resolver. A trait may also hold logic functions, logical associated types, and proofs in the signatures of its methods, and Locus checks what rustc cannot see. In the core, a bound is a parameter: a record of evidence for the implementation that rustc selected. Whether a member is a logic function is part of its declaration in the trait. A program gives a logical type only traits with no method that runs, so a specification and the code that meets it are two traits, one a supertrait of the other. In runtime code an operator always means what it means in Rust, and tells Locus nothing on a type of the program's own; in logical code it is built in on `Int`, `Nat`, and `Bool`, and otherwise a logic function of a trait of Locus's library. The conversion of a `Bool` to a proposition becomes one bound. A law that the standard library documents and cannot check is a trait of Locus's own beside the standard one, such as `CloneEq`, and the fact comes from that trait's method, called by name.

## Motivation

**A trait cannot state a contract today.** `logic.ghost.no-trait-impls` keeps every logical value out of a trait method's signature. So the decided example `logical-layer/trait-result-trusted.reject.lc` has no accepted form that keeps its trait:

```rust
trait OneByte {
    /// Returns an ASCII byte.
    fn byte(&self) -> u8;
}

fn first<P: OneByte>(p: &P) -> u8 {
    let b = p.byte();
    ascii(b, _)          // rejected: `byte` states no contract
}
```

**A derived `Clone` cannot carry an invariant.** For `struct Interval { lo: u32, hi: u32, ordered: @(lo <= hi) }`, the standard derive builds the copy from the results of `Clone::clone` on each field, and a call is known only by its signature.

**Rust documents laws that nothing checks.** `Ord` is to be a total order, `Eq` is to agree with `Hash`, `Clone` is to copy. A contract on Locus's own `sort` or map can only be true for types that keep the law, so the law has to be something a bound can name.

**Logical types have no traits at all.** No logical type implements a trait, so logical code cannot be generic over an operation, and a conversion or an operator on a logical type of the program's own cannot be written. A second family of traits for logical types would mean writing each trait twice. The owner: "if we went with 2, we will have to write many traits twice - one for logical types and one for non-logical types (say From/Into). So it's really poor ergonomics."

**The conversion of a `Bool` to a proposition is a list of positions.** RFC 0002 lists where a `Bool` is used as a proposition, and `iff` is the one function whose parameters take either. The owner decided that the list is to become one bound (RFC 0002, decision 110), which needs a trait on logical types.

## Design

**Snapshot reconciliation:** [RFC 0008](0008-snapshot-capture-and-views.md) replaces implicit models with snapshot capture and ordinary explicit `View`, uses type-based `Unit` barriers, and updates collection APIs. The current design below incorporates it; numbered historical decisions retain their original rationale and are superseded where the RFC 0008 inventory says so.

### The principle: a trait is Rust's

The owner: "I want us to have a system that is as close semantically to Rust as possible so that it feels familiar." And: "I obviously don't want to get in the business of full trait resolution - something simpler is doable but full trait resolution should be left to rustc."

- **Declaring, implementing, bounding, and calling are Rust's,** for every type. rustc decides which implementation a call means, by coherence, the orphan rule, and its own resolution. Locus never chooses, and has no resolver.
- **Locus adds members that are erased,** and the checks on them. A program that uses none is checked as before.
- **A contract is still a signature.** A fact about a call comes from a proof in the callee's result and from nowhere else (`contracts.calls.unrelated`). The owner, asked whether a call may ever be read as an expression: "Fact is always proof in the result or small/finite list of facts language infers from some constructs (which we have already defined). Don't want to go back on that. And don't want to introduce pure functions either right now."
- **A proposition never selects an implementation.** To rustc every proof type is the one type `Proof`.

**What this rests on is RFC 0001's, and is not restated here.** Each logical type is a zero-sized Rust type of its own, the same in both programs, so rustc tells `Int` from `Nat` and resolves traits on them (RFC 0001, decision 49). rustc types logical code through a typing skeleton, so it resolves each trait that logical code uses, with no question put to it afterwards. A trait's logic functions have their stubs in the lowered trait, under reserved names. And `Snap<T>` is an associated type that rustc normalizes. The fourth draft of this RFC had its own lowering, one marker for every snapshot, and a rule for ghost locals. All three are gone: [A Rust type for each logical type](0001-compiler-pipeline.md#a-rust-type-for-each-logical-type) and [Typing skeletons](0001-compiler-pipeline.md#typing-skeletons) replace them.

### What a trait may hold

Beside what a Rust trait holds, two kinds of member. A lemma is not a third: it is a logic function that returns a proof.

```rust
trait OneByte {
    // A logical value in a method's signature: here a proof in the result.
    fn byte(&self) -> (b: u8, @(b < 128));
}

trait Monoid: Logical {
    logic fn unit() -> Self;                            // logic functions
    logic fn op(a: Self, b: Self) -> Self;
    logic fn assoc(a: Self, b: Self, c: Self)           // one that returns a proof: a lemma
        -> @(Self::op(Self::op(a, b), c) === Self::op(a, Self::op(b, c)));
}

impl Monoid for Int {
    logic fn unit() -> Int { 0 }
    logic fn op(a: Int, b: Int) -> Int { a + b }
    logic fn assoc(a: Int, b: Int, c: Int) -> @(Int::op(Int::op(a, b), c) === Int::op(a, Int::op(b, c))) { _ }
}
```

**Accepted.** `Monoid` is the program's trait and `Int` is the library's type, which the orphan rule allows.

- **A logic function or a method may have a default body,** as in Rust.
- **An associated type may be logical,** declared with the bound `Logical`. It may be any logical type, one that holds proofs included.
- **A trait with logical content is called a Locus trait** below: a logic function, a logical associated type, a logical value in a method's signature, or any of these through a supertrait or a bound on an associated type.
- **Not for now:** a `prop` with constructors as a member, and a logical associated constant.

### The kind of a member is the trait's

The owner asked: "let's take Into - this trait has non-logical fn in it. How can it work for say logical type - a non-logical function will not erase, right?" And, of an earlier form of this draft that let an implementation choose: "fn be logic or not should be part of fn declaration in trait, not property of implementation, no?"

Three rules make the boundary between logic functions and functions that run:

1. **Whether a member is a logic function is part of its declaration in the trait,** and every implementation gives it as declared (`traits.members.kind`). rustc enforces it as a side effect of the reserved names: an `impl` that gives `fn le` for `logic fn le` misses one member and has one too many (E0046, E0407; [marker-lowering](../../spikes/marker-lowering/README.md), `t10` and `t11`).
2. **Logical code calls only logic functions** (`logic.code.no-runtime-calls`). This is unchanged.
3. **A program writes no implementation, for a logical type, of a trait with a method that runs** (`traits.impls.logical-types`). This is the strict rule. The logical types keep the implementations that the language gives them, which are listed [below](#the-implementations-that-are-built-in).

What the third rule does:

- **`impl From<Nat> for Ratio` is rejected,** for a logical `Ratio`. The standard `From` has a method that runs. At a logical type it would be a function on markers that logical code cannot call, with a result known only by its type. And the standard library's blanket `Into` over it would be unverified code handed logical data by value, which RFC 0002's boundary forbids.
- **A conversion between logical types is a logic function,** of a trait that declares it so: `FromLogic`, below.
- **RFC 0002's protection stays.** `PartialEq`, `Ord`, and `Hash` have methods that run, so no program gives them to a logical type, and rustc still rejects `==` on two `Vec<Nat>`s.
- **No ordinary Rust trait gains a logical implementation.** A trait that a program implements for a logical type declares only logic members, so it is a Locus trait by its declaration, and every implementation of it is checked.
- **It is checked after rustc.** Lowering sees tokens. It cannot see that a trait of another module or crate has a method with a default body, and it cannot see through an alias to a logical type ([marker-lowering](../../spikes/marker-lowering/README.md), `t21`: with the trait in another invocation, `half.hello()` ran on a logical value). So the rule is judged from what rustc resolved, with the checks that [RFC 0001](0001-compiler-pipeline.md#what-rustc-decides-and-what-locus-checks-after-it) lists as made after it.

### A specification and its code are two traits

The owner, of syntax that would let one trait hold both and be implemented in part: "couldn't one always write two traits - one just with logical stuff and one with non-logical stuff and set bounds explicitly? as in, is it worth adding language features here for this". One can, and this RFC adds no such syntax.

```rust
trait OrderSpec: Snapshot {
    logic fn le(a: Snap<Self>, b: Snap<Self>) -> Prop;
    logic fn le_total(a: Snap<Self>, b: Snap<Self>) -> @(Self::le(a, b) || Self::le(b, a));
}

trait CheckedOrder: OrderSpec {
    fn le_checked(&self, other: &Self) -> (r: bool, @(iff(r, Self::le(snap(self), snap(other)))));
}

#[derive(Logical)]
struct Rational { num: Int, den: Int }
struct Version { major: u32, minor: u32 }

impl OrderSpec for Rational { .. }       // a logical type: the specification
impl OrderSpec for Version { .. }        // a runtime type: both
impl CheckedOrder for Version { .. }

logic fn is_max<T: OrderSpec>(m: Snap<T>, a: Snap<T>, b: Snap<T>) -> Prop { T::le(a, m) && T::le(b, m) }

fn max<'a, T: CheckedOrder>(a: &'a T, b: &'a T) -> (m: &'a T, @(is_max::<T>(snap(m), snap(a), snap(b)))) { .. }
```

**Accepted.** `max::<Rational>` is rejected by rustc: `Rational` does not implement `CheckedOrder`, and by the third rule cannot.

- **Each operation and law is declared once.** The second trait adds only the method that runs. This is the shape of `PartialEq` and `Eq`.
- **One specification serves both kinds of type.** A logic function takes logical types only (RFC 0002, decision 105), so the specification is written over `Snap<Self>`, and the trait has `Snapshot` as a supertrait, which is what lets it name `Snap<Self>` (`traits.model-bounds`, [below](#a-model-bound-in-a-trait)). At a runtime type that is the type's snapshot. A logical type is its own snapshot, so `impl OrderSpec for Rational` writes `logic fn le(a: Rational, b: Rational) -> Prop`.
- **A comparison is a proposition** (RFC 0002, decision 97), so `le` returns `Prop`, and the method ties its `bool` to it with `iff` (decision 110).
- **The bounds say which is wanted.** Generic logical code asks for `OrderSpec`, and generic code that runs asks for `CheckedOrder`.
- **A call names the type.** A snapshot's type does not say which type it is a snapshot of, since `Snap<u32>` and `Snap<u64>` are both `Nat`, so a generic call over snapshots is written `is_max::<T>(..)`, as RFC 0002 says of `alike::<T>(..)`.
- **The split may have more than two levels:** the operations, the laws over them, and the code. A helper that proves a law then asks only for the operations, and so does not use the evidence it helps to define ([below](#no-definition-depends-on-itself)).

**Three more cases,** worked by hand. The traits are ordinary traits of Locus's library.

```rust
// Arithmetic. `+` in logical code is `AddLogic::add`.
pub trait AddLogic<Rhs: Logical = Self>: Logical {
    type Output: Logical;
    logic fn add(a: Self, b: Rhs) -> Self::Output;
}

impl AddLogic for Rational {
    type Output = Rational;
    logic fn add(a: Rational, b: Rational) -> Rational {
        Rational { num: a.num * b.den + b.num * a.den, den: a.den * b.den }
    }
}

logic fn twice<T: AddLogic<Output = T>>(x: T) -> T { x + x }

// Conversions. Both blanket implementations are written in Locus and checked.
pub trait FromLogic<T: Logical>: Logical { logic fn from(t: T) -> Self; }
pub trait IntoLogic<U: Logical>: Logical { logic fn into(self) -> U; }
impl<T: Logical> FromLogic<T> for T {
    logic fn from(t: T) -> T { t }
}
impl<T: Logical, U: FromLogic<T>> IntoLogic<U> for T {
    logic fn into(self) -> U { U::from(self) }
}

// Iterators. The specification is a relation between states; the code returns a proof of one step.
pub trait IterSpec: Iterator<Item: Snapshot> + Snapshot {
    logic fn step(before: Snap<Self>, item: Snap<Option<Self::Item>>, after: Snap<Self>) -> Prop;
}
pub trait CheckedIter: IterSpec {
    fn next_checked(&mut self)
        -> (x: Option<Self::Item>, @(Self::step(snap(self), snap(x), snap(final!(self)))));
}
```

`twice` is the function the third review gave as rejected when written against Rust's `Add`. Against `AddLogic` it is accepted. That is the cost to state plainly: for an operation that both worlds have, Locus's library has a logical trait beside the standard one, and generic logical code names the logical one. `FromLogic` has no implementation from `Nat` for `Int`: that conversion is `n as Int` and nothing else (RFC 0002, decision 98).

### A model bound in a trait

The heading retains existing links. The capture bound is now `Snapshot`; an optional ordinary `View` is separate ([RFC 0008](0008-snapshot-capture-and-views.md)).

- `Snapshot` may be a supertrait or a bound on an associated type, trait parameter, or method parameter. It supplies `Snap<Self>` for contracts that capture `self`.
- A logical type meets `Snapshot` with itself as its snapshot; this does not automatically implement a view.
- `View: Snapshot` is an ordinary logic-only library trait with associated logical type `V` and `logic fn view(s: Snap<Self>) -> Self::V`. Its coherence, visibility, and totality rules are ordinary trait rules.
- Export restrictions on logical bounds remain. There is no inferred bound and no implicit view conversion.

The earlier `models/model-bound-on-trait` outcome change belongs to this RFC's original trait-bound decision. RFC 0008 changes its spelling to `Snapshot`, not its accepted outcome.

### Evidence: what a bound means

This is the meaning in the core language of [RFC 0005](0005-core-terms.md). A programmer does not write it. It is stated once, here and in `traits.evidence`.

**The evidence that `X: Tr` is a record.** It holds:

1. the trait's logic functions at `X`, lemmas among them;
2. each associated type: a logical one as a logical type, and any other as the runtime type that rustc resolved, which the core needs to give `Snap<T::Item>` and a method's result their types;
3. the evidence for each bound on an associated type;
4. the contract of each method: its signature, with the propositions of its proof parameters and of its result;
5. the evidence for each supertrait.

**How it is made and used:**

- **An implementation defines evidence,** as a function of the evidence for its own bounds: `impl<T: Bounded> Bounded for (T, T)` is a function from evidence for `T` to evidence for the pair.
- **Generic code takes evidence as a parameter,** one for each bound. A use of a bound at a concrete type is given the evidence of the implementation that rustc selected.
- **A method's contract is not a logic function.** The evidence says what holds when a call of the method returns, and gives no term for its result. `fn explode() -> @False { loop {} }` is a valid runtime method, and its evidence proves nothing.
- **That a method's body meets its contract is the ordinary check of that body,** for the implementation selected, with its type arguments. That the checked body is the code that runs is RFC 0001's validation of the two programs, which stays in the trust model. The kernel checking a record does not replace either.
- **Evidence exists only where every part of it does.** There is evidence for `Wrap<Raw>: Tr` only if there is evidence for `Raw: Tr`.

### Implementing a trait

```rust
struct Letter(u8);

impl OneByte for Letter {
    fn byte(&self) -> (b: u8, @(b < 128)) {
        if small: self.0 < 128 { (self.0, small) } else { (0, _) }
    }
}
```

**Accepted.** `first` is then written `let (b, small) = p.byte(); ascii(b, small)`, for every `P: OneByte`.

```rust
impl OneByte for Wide {
    fn byte(&self) -> (b: u8, @(b < 200)) { .. }   //~ error: the trait says `b < 128`
}
```

**Rejected.** rustc sees `(u8, Proof)` on both sides. Locus compares the propositions ([marker-lowering](../../spikes/marker-lowering/README.md), `t08`).

- **An implementation gives every member that has no default.**
- **Each signature is exactly the trait's,** once `Self`, the trait's parameters, and its associated types are filled in (`traits.impls.conform`). An implementation that can promise more puts the stronger contract on an inherent method, and its trait method calls that.
- **No proof type, dependent tuple, or snapshot type is written in a header,** in the self type or the trait's arguments (`traits.impls.headers`). rustc sees every proof as `Proof`, so an implementation written for one proposition would be selected for every other: `impl From<Option<@(1 == 2)>> for Absurd` stays rejected. A snapshot type is the type it is equal to, so `impl Describe for Snap<u32>` is an implementation for `Nat`, and applies at `Snap<u64>`.
- **That rule too is checked after rustc.** `type Shot = Snap<u32>; impl Describe for Shot` shows lowering only the name `Shot` ([marker-lowering](../../spikes/marker-lowering/README.md), `h04`).
- **Any other logical type may be written in a header.** Two implementations may differ only in a logical type argument: `impl Scale<Int> for Meters` and `impl Scale<Nat> for Meters`. The fourth draft forbade a logical type in the arguments of a trait with a method that runs. Under the lowering now chosen the two are two Rust types in both programs, so rustc resolves them alike in both, and the cases that broke the other encodings pass ([marker-lowering](../../spikes/marker-lowering/README.md), `r01`, `r02`, `r10` to `r14`).
- **A runtime type may give a logical associated type in a trait that also has a method that runs:** `type Unit: Logical;` beside `fn raw(&self) -> u32`. The fourth draft forbade this too, and the spike compiles it, stored in a runtime struct (`t06`).
- **Strict positivity follows associated types:** `logic.types.positive` is checked with each associated type replaced by what an implementation chooses.

### Using a bound

- **In generic code a member is known by the signature its trait declares,** and by nothing else (`traits.bounds`). A logic function of the trait does not unfold there. What is known of it is the trait's lemmas.
- **At a concrete type a logic function of the trait is the implementation's,** and unfolds where conversion would unfold any body like it.
- **A lemma is used by calling it.** A hole never calls one.
- **Generic code is checked once,** and what it proves holds at every instantiation, because it holds for any evidence and an instantiation supplies some. The boundary with unverified code is still checked for each instantiation, as RFC 0002 has it.
- **A bound may stand wherever Rust allows one.**

**Names** (`traits.names`). A logic function of a trait is named as Rust names an associated function: `Rational::le(a, b)`, `T::le(a, b)`, `Self::le(a, b)`, `<T as OrderSpec>::le(a, b)`, and through the trait, `OrderSpec::le(a, b)`, where the arguments give the type. rustc resolves each, in the typing skeleton. The fourth draft kept logic functions out of the lowered trait, so that none could be a candidate in rustc's resolution of Rust code. RFC 0001 puts their stubs in the trait under reserved names, which gives the same and fixes what the hidden trait broke: a method call that runtime code writes never finds a stub (`t19`). `compat.names.no-methods` still holds for `Logical` and `Snapshot`; explicit `View` is imported and used like other logical traits. A law trait has a method that runs, such as `clone_eq`, so a program imports it by name.

### No definition depends on itself

Without a rule here, this proves `False`:

```rust
trait Truth: Logical {
    logic fn holds() -> @False;
}

logic fn pull<T: Truth>() -> @False {
    T::holds()
}

impl Truth for Int {
    logic fn holds() -> @False {
        pull::<Int>()        //~ error: the evidence for `Int: Truth` is used to define itself
    }
}
```

**Rejected** (`traits.impls.acyclic`).

- **Evidence is a definition, and no definition depends on itself** except by the recursion `logic.fn.total` allows. Stated on evidence, the rule covers every path: a supertrait, a bound on an associated type, the prerequisites of an implementation with bounds, an instantiated default, and a logic function called in a signature or a type.
- **Passing evidence depends on all of it.** A direct call, `Self::op(a, b)` inside the implementation, depends on that one member.
- **Evidence for a concrete type is a finite term.**
- **A trait is split where a helper needs less.** Tracking which members generic code uses is listed as open.
- **Runtime methods are not held to this.** One that never returns gives its caller no proof.
- **Explicit views obey the same cycle rule.** `models.own-definition` applies to ordinary `View` definitions and helpers; `Snapshot` has no user-supplied model function.

### `Logical`

The original design requested logical-kind and capture bounds as traits. RFC 0008 names the latter `Snapshot`.

- **`Logical` holds for exactly the logical types** (`traits.logical`). Locus defines it. It has no member that a program can name and no implementation written by hand.
- **It is a bound like any other:** `fn keep<G: Logical>(g: G) -> G` takes any ghost value.
- **A trait whose logic functions take or return `Self` as a value says `Logical`.** A logic function takes and returns logical types only, and in a trait `Self` may be any type. `Monoid: Logical` writes it. None is implied, as none is for `Snapshot`.
- **A logic function's own type parameters are RFC 0002's.** They range over logical types unless bounded by `Snapshot` (`logic.fn.parameters`). The fourth draft had them range over every type, with `Logical` implied where the signature needed it. RFC 0002's decisions 105 and 106 came after it and replace that.

### The implementations that are built in

The strict rule is about what a program writes. These are the implementations that the language and Locus's library give, and there are no others:

| For | Traits | Whose |
|---|---|---|
| Every logical type | `Logical`; `Clone`, `Copy`, `Debug`; `CloneEq` | The language's. `Clone`, `Copy`, and `Debug` are RFC 0002's (`logic.ghost.marker-traits`), and this RFC adds `CloneEq` |
| `Proof`, the marker of every proof | Also `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash` | RFC 0002's: any two proofs of a proposition are equal |
| Every logical type | `Snapshot`, as its own snapshot | The language's (`traits.model-bounds`) |
| Every logical type `T` | `FromLogic<T>`, the identity, and `IntoLogic<U>` wherever `U: FromLogic<T>` | Locus's library, written in Locus and checked |
| `Prop` | `FromLogic<Bool>` | Locus's library: its one conversion to `Prop` |
| `Int`, `Nat` | The logical operator traits, with the built-in operators as their logic functions | Locus's library, so that `twice` applies at `Int` |

- **`CloneEq` is built in because a derive needs it.** `#[derive(CloneEq)]` on a struct with a field of type `Vec<Nat>` calls `clone_eq` on that field, and the library's implementation for `Vec<T>` asks `T: CloneEq`. `CloneEq` has a method that runs, so no program could give it to `Nat`. Copying a marker is the whole of the method, as it is for `Clone` ([marker-lowering](../../spikes/marker-lowering/README.md), `s15`).
- **None makes a value from nothing,** as RFC 0002 requires: no `Default`.

### Operators

The owner asked: "do they continue mapping to rust traits or preferentially bind to Locus traits or is there a setting where they start binding to one vs other?"

**In runtime code they map to Rust's traits, always, and there is no setting. In logical code they are built in on `Int`, `Nat`, and `Bool`, and otherwise map to traits whose members are declared as logic functions** (`traits.operators`). Which code an operator is in is decided as today, by where it stands and by its operands (`logic.ghost.shown-by-source`).

- **In runtime code** an operator, an index, a `for`, a `?`, and a method call mean what they mean in Rust. What Locus knows of the result is RFC 0004's table for the language's own types, and otherwise the method's signature. So `if a < b` on two values of a program's own type runs the comparison and tells Locus nothing; the fact comes from the law trait's method, called by name: `a.lt_checked(&b)`, `x.clone_eq()`.
- **This is the owner's decision on facts,** quoted above. A standard method on a type that has a law trait is still a call that states nothing, though it runs the same code as the law's method.
- **In logical code** an operator never calls code that runs. On another logical type than the three, `+`, `-`, `*`, `/`, `%`, and unary `-` are the logic functions of traits of Locus's library, `AddLogic`, `SubLogic`, and so on, and the four orderings are logic functions of `OrdLogic`, which give a `Prop`. Rust's own `Add` cannot serve here, since its method is declared `fn`. A runtime value in logical code is its capture, so the operator is the captured value's.
- **A logical type implements the logical trait, and cannot implement Rust's,** which has a method that runs. An operator on ghost values is logical code even inside a runtime function.
- **Which trait each logical operator means is the one thing the language fixes here.** The traits themselves are ordinary traits of the library.
- **`==` in logical code** stays defined on `Int`, `Nat`, and `Bool`. Equality of other logical values is `===`.

### The conversion to `Prop` is one bound

The owner, asked about `iff`: "Do we need equivalent of From/Into for logical types? If we had one, maybe we could implement that for Bool to Prop and then require something like iff(x, y) to take Into<Prop> or something? Maybe same for <==> symbol?" Told that this draft has such a pair, and that the rule for `Bool` could then be one bound: "In that case, let's adopt this and remove <==> in favor of iff -- it's not a common enough thing to warrant its own operator."

- **A type converts to `Prop` when it implements `IntoLogic<Prop>`** (`traits.conversions`). `Prop` does, by the identity. `Bool` does, by the library's one implementation, `impl FromLogic<Bool> for Prop`, whose function is `b === true`.
- **RFC 0002's rule is restated on it.** `logic.bool.as-proposition` listed the positions at which a `Bool` is used as a proposition. Each of those positions now takes a value of any type that converts to `Prop`. The list of positions is unchanged, and so is what a `Bool` means there.
- **A logic function may ask it of its own parameters,** as the library's `iff` does:

```rust
logic fn both<P: IntoLogic<Prop>, Q: IntoLogic<Prop>>(p: P, q: Q) -> Prop {
    p.into() && q.into()
}

fn checked(ready: bool, n: u32, ok: @(ready && n < 10)) -> @both(ready, n < 10) { ok }
```

**Accepted.** Before this RFC only `iff` could be declared so. A parameter of type `Prop` still takes a proposition only: given a `Bool`, it is written `b === true`.

- **A program implements no conversion to `Prop`.** rustc would accept `impl FromLogic<Flag> for Prop` for a program's `Flag`, by the orphan rule. Locus rejects it. The owner spoke of `Bool`, and the question whether a program's type may convert was put as "start with only the library's one implementation and decide on user implementations in the traits RFC". This RFC writes the reading that accepts less, and lists it for the owner.
- **`Nat` to `Int` is not a conversion of this kind.** It is `n as Int` (RFC 0002, decision 98), and the library has no `FromLogic<Nat>` for `Int`, so `n.into()` where an `Int` is wanted is an error.

### Law traits and the standard library's traits

A **law trait** is a Locus trait of Locus's library that stands beside a trait of the standard library, under a name of its own. The owner: "isn't CloneEq better because it creates no confusion?"

```rust
// Locus's library.
pub trait CloneEq: Clone + Snapshot {
    fn clone_eq(&self) -> (copy: Self, @(snap(copy) === snap(self)));
}
// `CloneEq` provides `Clone`:  fn clone(&self) -> Self { self.clone_eq().0 }
```

```rust
struct Meters(u32);

impl CloneEq for Meters {
    fn clone_eq(&self) -> (copy: Meters, @(snap(copy) === snap(self))) {
        (Meters(self.0), _)
    }
}
```

**Accepted.** `Meters` is `Clone`, and `m.clone()` runs the same code and gives no fact.

- **An implementation of a law trait in a program always generates the provided implementations** (`traits.laws.provided`), as `From` gives `Into` in Rust. The orphan rule forbids a blanket implementation here, so Locus writes the provided one beside each implementation of the law trait. Its body is checked like any other code.
- **Where the type already implements the provided trait, that is a conflict and an error.** So the standard method and the law's method are the same code by construction. `impl CloneEq for Box<Local>`, where `Local` has a `Clone` that does not copy, is rejected: tested with rustc 1.97, the standard `clone` of that box gives 8 where a correct `clone_eq` gives 7.
- **An adapter stands beside an implementation that already exists,** and only Locus's library writes one (`traits.laws.adapters`). It names that implementation, and its bounds carry the law for every part the existing implementation uses. That the two agree is trusted, pair by pair, and a result lists every adapter that a contract it uses rests on. This is new trust, and the entry is `trusted.library-laws`.
- **The law is on snapshots, so `CloneEq` says `Snapshot`.** Its contract reads `self`.

**The derive.**

```rust
#[derive(CloneEq)]
struct Interval {
    lo: u32,
    hi: u32,
    ordered: @(lo <= hi),
}
```

What the derive writes:

```rust
impl CloneEq for Interval {
    fn clone_eq(&self) -> (copy: Self, @(snap(copy) === snap(self))) {
        let (lo, lo_same) = self.lo.clone_eq();
        let (hi, hi_same) = self.hi.clone_eq();
        let ordered = self.ordered.rewrite(lo_same.symm()).rewrite(hi_same.symm());
        (Interval { lo, hi, ordered }, /* the law, from lo_same and hi_same */)
    }
}
```

**Accepted** (`traits.laws.derive`).

- **Each proof field is the original's proof, carried along the fields' equations,** as a written term. It does not depend on proof search, and it works for an invariant that no procedure understands.
- **Each equation is at the snapshot the containing type uses for that field.** Where the type's snapshot cuts a field ([RFC 0005](0005-core-terms.md)), the field's `CloneEq` proves the wrong equation: it is about `Snap<F>`, and the type holds a `CutSnap<F>`, which keeps addresses. A cut field is copied where its type is `Copy`, and otherwise the derive is rejected with a message that names the field.
- **An `Opaque<T>` field uses its library `CloneEq` implementation,** which can clone the payload under `T: Clone` and prove equality of the `Unit` snapshots. It supplies no behavioral equivalence of the payload.
- **A field of a logical type is copied.** Its type has `CloneEq` built in.
- **The bound `CloneEq` is put on each type parameter, except one bounded by `Logical`.**
- **It is checked like any other code,** so it adds no trust, and it is not written beside `#[derive(Clone)]`, which it provides.

**One case is not solved.** A program's generic type gets `Clone` from its law implementation, so it is `Clone` only for parameters that are `CloneEq`, where `Vec<T>` is `Clone` for every `T: Clone`.

### Unverified code and export

- **A Locus trait is implemented only in crates that Locus verifies.** RFC 0001 already closes part of this: a trait with a logic function has a stub that plain Rust cannot write. A trait whose only logical content is a proof in a method's signature has none, and a crate written in Rust alone could implement the lowered trait and return any `Proof` it can make. In a build, such an implementation in any crate is an error, wherever it would be used. This needs two crates to show, so it is stated in [the chapter](../../spec/traits.md#what-has-no-rule) without a rule.
- **An exported crate cannot rely on that,** because its users are built without Locus. A Rust user could implement the exported `CloneEq` for a type of its own and hand it to an exported `impl<T: CloneEq> Clone for Pair<T>`.
- **So an exported crate has no reachable item or implementation with a bound on a Locus trait** (`export.traits`), generated implementations included. A `Snapshot` bound in an exported crate stays RFC 0002's: only on a function.
- **Lifting this needs evidence that an exported program cannot forge.** That makes RFC 0002's open question, whether an exported crate depends on Locus's library or carries its own copies, a question of soundness.

### Left out

- **`dyn` of a Locus trait** is rejected (`traits.dyn`).
- **Contracts on closures,** and so on `Fn` bounds.
- **A stronger contract on an implementation than on its trait.**
- **`?` on a value that holds proofs,** which goes through the `Try` trait. It means what it means in Rust and states nothing.

## A sketch of the library's law traits

[RFC 0007](0007-provided-trait-implementations.md) records the unreviewed generalized provider syntax and method inventory. Laws name an explicit specification over `Snap<Self>`, such as `EqSpec::equiv`, and checked methods tie their results to that same specification. A type may implement the specification alone. `LawfulEq`, checked equality, and hash compatibility are separate obligations; implementing a library view supplies none automatically.

`CloneEq` remains a whole-snapshot law, not equality of a chosen view. For Locus's vector, the native field contributes `Unit`, so element `CloneEq` proofs can establish equality of its logical contents and hence the whole vector snapshot. Ordinary `Clone` alone cannot. An `Opaque<F>` payload contributes equal snapshots even if cloning changes its hidden executable behavior; this law is intentionally only an observation law.

A hash law must prove that its observations cannot distinguish inputs equal under the same named `EqSpec`. Reading hidden runtime payload is not automatically forbidden, but a hash that depends on a difference erased by that equality cannot satisfy the law. Cross-type borrowing must establish agreement of the pair's actual hash protocol and order; a common view type alone does not establish either.

Native `Hash::hash<H: Hasher>` cannot silently acquire a stronger checked-hasher bound. Its bridge remains open, as do generic `Clone`/`CloneEq` overlap and fact transport at ordinary proof-dropping bridge calls. The provider proposal does not amend `contracts.calls.unrelated` merely by generating forwarding code.

Machine types may share `Snap<u32> = Snap<u64> = Nat` while their executable operations wrap differently. Their specifications therefore identify the source operation and width explicitly. No law is inferred from the shared snapshot type.

## Rules

A new chapter of the specification, [Traits](../../spec/traits.md), and one rule in [Exporting to Rust](../../spec/export.md).

| Rule | Examples |
|---|---|
| `traits.rust` | `traits/implementations-differ-in-a-logical-argument.accept.lc`, `traits/two-logical-types-one-trait.accept.lc`, `traits/bound-not-met-by-logical-type.reject.lc`, `traits/implementation-missing-for-a-bound.reject.lc` |
| `traits.members` | `traits/contract-on-a-trait-method.accept.lc`, `traits/logical-associated-type-beside-a-method.accept.lc`, `traits/two-logical-types-one-trait.accept.lc`, `traits/logical-associated-constant.reject.lc` |
| `traits.members.kind` | `traits/specification-and-code-in-two-traits.accept.lc`, `traits/method-given-for-a-logic-function.reject.lc` |
| `traits.logical` | `traits/ghost-value-of-any-logical-type.accept.lc`, `traits/logical-implemented-by-hand.reject.lc`, `traits/runtime-value-where-logical-is-asked.reject.lc`, `traits/self-as-a-value-without-logical-supertrait.reject.lc` |
| `traits.model-bounds` | `models/model-bound-on-trait.accept.lc`, `traits/model-as-supertrait.accept.lc`, `traits/specification-and-code-in-two-traits.accept.lc`, `traits/self-read-without-model-supertrait.reject.lc` |
| `traits.impls.conform` | `traits/contract-on-a-trait-method.accept.lc`, `traits/stronger-contract-in-implementation.reject.lc`, `traits/weaker-contract-in-implementation.reject.lc` |
| `traits.impls.headers` | `traits/implementations-differ-in-a-logical-argument.accept.lc`, `traits/logical-associated-type-beside-a-method.accept.lc`, `logical-layer/proof-in-trait-impl.reject.lc`, `traits/snapshot-type-in-header-through-alias.reject.lc` |
| `traits.impls.logical-types` | `traits/conversion-between-logical-types.accept.lc`, `traits/specification-and-code-in-two-traits.accept.lc`, `traits/code-trait-for-logical-type.reject.lc`, `traits/provided-method-on-logical-type.reject.lc`, `traits/standard-from-for-logical-type.reject.lc` |
| `traits.impls.acyclic` | `traits/implementation-under-a-bound.accept.lc`, `traits/evidence-defined-by-itself.reject.lc` |
| `traits.evidence` | `traits/contract-on-a-trait-method.accept.lc`, `traits/fact-from-the-method-called-by-name.accept.lc`, `traits/implementation-under-a-bound.accept.lc`, `traits/lemma-of-a-bound-called.accept.lc`, `traits/specification-and-code-in-two-traits.accept.lc`, `traits/implementation-missing-for-a-bound.reject.lc`, `traits/method-result-in-a-proposition.reject.lc` |
| `traits.bounds` | `traits/implementation-under-a-bound.accept.lc`, `traits/lemma-of-a-bound-called.accept.lc`, `traits/two-logical-types-one-trait.accept.lc`, `traits/logic-function-opaque-under-a-bound.reject.lc` |
| `traits.names` | `traits/logic-function-beside-a-method-of-the-same-name.accept.lc`, `traits/member-named-through-its-trait.accept.lc`, `traits/logical-member-of-two-traits.reject.lc` |
| `traits.dyn` | `traits/trait-object-of-a-plain-trait.accept.lc`, `traits/trait-object-of-a-locus-trait.reject.lc` |
| `traits.conversions` | `traits/conversion-between-logical-types.accept.lc`, `traits/parameter-that-converts-to-a-proposition.accept.lc`, `traits/conversion-to-prop-for-own-type.reject.lc`, `traits/nat-converted-into-int.reject.lc`, `traits/number-where-a-proposition-converts.reject.lc` |
| `traits.operators` | `traits/fact-from-the-method-called-by-name.accept.lc`, `traits/logic-function-beside-a-method-of-the-same-name.accept.lc`, `traits/operator-on-own-logical-type.accept.lc`, `traits/operator-without-its-logical-trait.reject.lc`, `traits/runtime-comparison-gives-no-fact.reject.lc`, `traits/standard-clone-gives-no-fact.reject.lc` |
| `traits.laws.provided` | `snapshots/vector-clone-whole-snapshot.accept.lc`, `traits/derive-clone-eq-with-invariant.accept.lc`, `traits/law-implementation-gives-clone.accept.lc`, `traits/law-beside-derived-clone.reject.lc`, `traits/law-beside-standard-implementation.reject.lc`, `traits/standard-clone-gives-no-fact.reject.lc` |
| `traits.laws.adapters` | `traits/adapter-for-a-standard-type.accept.lc`, `traits/law-beside-standard-implementation.reject.lc` |
| `traits.laws.derive` | `traits/derive-clone-eq-with-invariant.accept.lc`, `traits/derive-clone-eq-beside-derive-clone.reject.lc`, `traits/derive-clone-eq-over-a-field-without-the-law.reject.lc` |
| `export.traits` | `traits/exported-bound-kept-inside.accept.lc`, `traits/exported-bound-on-a-locus-trait.reject.lc` |

**Rules of [RFC 0002](0002-logical-layer.md) that this RFC changes.** Each stays in RFC 0002's table, with its examples, and is named here in prose:

- **`logic.ghost.no-trait-impls`** no longer forbids a logical type in an `impl` or a logical value in a trait method's signature. It keeps its second half, on a runtime type that holds logical data, and says where the rest now is: `traits.members`, `traits.impls.headers`, and `traits.impls.logical-types`.
- **`logic.ghost.marker-traits`** gains `CloneEq` and `Logical` among what every logical type implements, and says that a program adds no trait with a method that runs.
- **`logic.bool.as-proposition`** is restated on the conversion to `Prop`: each position takes a value of any type that converts. The positions are the same, with one more: an argument for a parameter whose type is bounded by `IntoLogic<Prop>`, which is what an argument of `iff` was.
- **`models.capture.bound`** names a trait among the places where the program writes the bound.
- **`models.bounds-on-functions`** loses "and never on a trait". What it says of an exported crate stands.

**What has no rule.** Three things need a second crate or a second file to show ([multi-file-litmus](../../issues/multi-file-litmus.md)), and are stated in the chapter without a rule: that a Locus trait is implemented only in crates that Locus verifies; that the orphan rule applies to a logical type of Locus's library and of a program as it does to any type; and that a trait of another crate is read as its crate declared it. The fourth draft had the first as a rule, `traits.impls.checked`.

**Not carried over from the fourth draft:** `traits.markers`, which is RFC 0001's lowering and RFC 0002's `logic.ghost.erased`, and `traits.model`, which is RFC 0002's models and RFC 0001's `compat.names.model-impls`.

## Interactions

| Area | Effect |
|---|---|
| Types | Logical types implement traits. `Logical` is a trait, and a logical type meets a `Snapshot` bound as its own snapshot. A logical associated type may be any logical type. A trait may have `Snapshot` as a supertrait. |
| Propositions and proofs | A bound is an evidence parameter in the core. A trait's logic function is opaque under a bound and the implementation's body at a concrete type. Conversion gains no reduction. A value used as a proposition is one whose type converts to `Prop`, which for now is a `Bool`. |
| Ownership and mutation | None. A contract on a trait method that takes `&mut self` names `final!(self)` as any signature does. |
| Erasure and runtime behavior | Logical values are zero-sized as before. A program gives a logical type no method that runs, so nothing of a program's runs on a marker. A generated implementation is real code, and is the law's own method. Two implementations that differ in a logical type argument are two implementations at run time too, so which code runs can follow a logical type, though never a logical value. |
| Rust interoperability | One set of traits, resolved by rustc. A Locus trait is implemented only in verified crates, and an exported crate exposes no bound on one. For an operation that both worlds have, the library has a logical trait beside the standard one. |
| Diagnostics | New errors: a signature that is not the trait's; a member given in the other kind; a proof type or a snapshot type in a header; a logical type given a trait with a method that runs; evidence that depends on itself; a conflict with a generated implementation; a conversion to `Prop` written by a program; a Locus trait as a trait object; and a bound on a Locus trait that an exported crate shows. The missing `Snapshot` supertrait is rustc's error, with its help. |
| Trust model | One new entry, `trusted.library-laws`, one line for each adapter. `trusted.ghost-checks` and `trusted.driver` cover more: the checks this RFC makes after rustc, and refusing an implementation of a Locus trait in a crate that Locus does not verify. |

## Alternatives

**Traits for runtime types only.** The first draft. Rejected by the owner: "we will need support traits on logical types too".

**A separate, simpler family of logical traits, resolved by Locus.** The second draft. Rejected by the owner: every trait would be written twice. It also made a member name shared by two logical traits ambiguous on every logical type.

**One set of traits with one marker, and Locus selecting for logical types.** It gets stuck at once, as the first spike shows: two implementations for two logical types conflict; an associated type cannot differ between them; `Int::from(nat)` already resolves to the standard library's reflexive `From`; and a program cannot give a standard trait to its logical type, which is the library's marker. RFC 0001 records the two encodings that kept one marker somewhere and broke.

**A condition that binds the law's proof,** so that `if h: a < b` on a type with a law trait would bind what `lt_checked` returns. The fourth draft listed it as open. Not pursued: it reads an operator as a call of another method, which is a second way for a fact to enter. The owner: "Fact is always proof in the result or small/finite list of facts language infers from some constructs (which we have already defined). Don't want to go back on that. And don't want to introduce pure functions either right now."

**A standard method that Locus understands where a law generated it.** `m.clone()` runs `clone_eq`, so Locus could give it the law's fact. Not pursued, on the same words of the owner: a call would be read by what Locus knows of its body, where a contract is a signature. The fact comes from `m.clone_eq()`.

**Syntax for a trait's logical part:** `impl logic Tr` to implement only a trait's logic members, and `T: logic Tr` to ask for them, with Locus's library declaring a logical part for the standard operator traits. It would give one name for each operator and one block for a mixed trait. Deferred, on the owner's question and with the reviewer's agreement: two explicit traits give the same capability with what rustc already understands, the right split is sometimes three traits and not two, and the syntax would need rules for defaults, associated types shared by the parts, and names across crates. It can be added over the explicit traits if they prove tiresome.

**A standard trait implemented for a logical type,** as a function on markers. An earlier form of this draft allowed it. Rejected: logical code could not call it, its result was known only by type, and it put the standard library's blanket implementations in contact with logical data.

**A specification stated on the model's type alone,** with the code's trait bounded `Model<Logic: OrderSpec>`. It needs no `Model` supertrait on the specification and no rule that a logical type is its own snapshot. Not taken: a runtime type with the default model has a snapshot type for its model, which no header may name, so each such type would have to declare a model of a named logical type before it could have a specification. It is the shape of the sketched `OrdLaw`, where the order is the model's own.

**An implementation chooses whether a method is a logic function,** where its signature there is entirely logical. An earlier form of this draft. Rejected by the owner: the kind belongs to the declaration in the trait. Generic code could not have known it.

**A trait's `fn` is a logic function wherever its whole signature is logical,** as a rule of the language. Rust's own `Add` and `From` would then serve logical code, and the library would need no `AddLogic`. Not taken: a reader could no longer tell a member's kind from the trait, and the standard library's blanket implementations, such as the one behind `Into`, are Rust code and would stay outside the rule.

**A richer implementation of the standard trait itself,** lowered to the standard signature. Rejected on the owner's point: under the standard name a reader would see a signature the standard trait does not declare.

**A law beside a plain implementation,** with no proof written. Rejected by the owner: "I don't like C unless there is also a way for human to explicit write the same proof."

**Binding runtime operators to Locus's traits, always or by a setting.** Rejected: Rust wires operators to its own traits, and a moved line must keep its meaning.

**The program writes the forwarding implementation, and Locus checks its form.** Rejected for now: one more implementation for each type, four for an order.

**A law assumed of every implementation of the standard trait.** Rejected: a valid Rust program may have a `Clone` that does not copy.

**Laws as promises,** an `unsafe trait` with no proof. Rejected as the main route.

**A program's own conversion to `Prop`.** `impl FromLogic<Flag> for Prop` would let a `Flag` stand where a proposition is wanted. Not taken for now, and listed for the owner: every position that takes a proposition would then take a program's type, with a meaning that only its implementation shows.

## Prior art

Written from memory, and not checked against sources for this draft.

- **Rust** has law traits as subtraits with no methods: `Eq` beside `PartialEq`, `Copy` beside `Clone`. `From` giving `Into` is the model for a trait that provides another.
- **Lean 4** separates an operation from its laws, as `BEq` and `LawfulBEq`. An instance is a value that a function takes as an argument, and finding one is not trusted by the kernel. Evidence here is the same idea, with rustc doing the finding. Lean uses a `Bool` as a proposition through a coercion, which is what the conversion to `Prop` is here.
- **Verus** allows specification and proof functions in traits and contracts on trait methods, and checks termination through trait bounds.
- **Creusot** states comparison laws on a logical model, with logical counterparts of the comparison traits.
- **Prusti** attaches specifications to the standard library's traits from outside.

## Unresolved questions

### Decided by the owner

1. **What a trait may hold.** "Yes to (1) - all three present in traits." And: "isn't a lemma basically just a logic fn?"
2. **No law without a written proof.** "I don't like C unless there is also a way for human to explicit write the same proof."
3. **A law trait has its own name.** "isn't CloneEq better because it creates no confusion?"
4. **`Logical` and `Model` are traits, and bounds.** "I assume we will have a trait called Logical for logical types (already satisfied for say Nat, Seq etc) and a trait called Model ... Then these traits (e.g. Logical or Model) themselves can become generic bounds for other traits/methods."
5. **Close to Rust, and resolution left to rustc.** "I want us to have a system that is as close semantically to Rust as possible so that it feels familiar." "I obviously don't want to get in the business of full trait resolution ... full trait resolution should be left to rustc."
6. **Traits on logical types, in one set.** "we will need support traits on logical types too". Of a separate family: "we will have to write many traits twice ... So it's really poor ergonomics."
7. **A type for each logical type.** "I'd imagine that having one type for each logical type is good and may make lowering easier in general. this can be made to work with generics too with use of phantom etc I'd imagine? So yes, this direction is good." It is RFC 0001's decision 49 now, where the owner chose the lowering after the spikes.
8. **Ghost locals.** Shown the conversion at each use against an annotation on every ghost `let`: "I'm happy with your recommendation for 1 and also 3." Point 3 was that an order on a logical type needs a trait of Locus's own. Point 1 is overtaken: under RFC 0001 a ghost local takes its type from its typing skeleton, and nothing is converted.
9. **The derive proves each proof field again,** and **a sketch of the law traits.** "what if we replaced clone for proof types with '_' ...", and "I also want us to sketch out all the so called Law traits for common traits in Rust."
10. **The kind of a member is the trait's.** "fn be logic or not should be part of fn declaration in trait, not property of implementation, no?"
11. **Two explicit traits, and no syntax for it.** "couldn't one always write two traits - one just with logical stuff and one with non-logical stuff and set bounds explicitly? as in, is it worth adding language features here for this". The reviewer agreed.
12. **A fact is a proof in a result, and no call is read as an expression.** "Fact is always proof in the result or small/finite list of facts language infers from some constructs (which we have already defined). Don't want to go back on that. And don't want to introduce pure functions either right now." So an operator or a standard method on a type with a law trait tells Locus nothing, and the law's method is called by name.
13. **The conversion to `Prop` is one bound, through conversion traits for logical types.** "Do we need equivalent of From/Into for logical types? If we had one, maybe we could implement that for Bool to Prop and then require something like iff(x, y) to take Into<Prop> or something? Maybe same for <==> symbol?" And: "In that case, let's adopt this and remove <==> in favor of iff -- it's not a common enough thing to warrant its own operator." It is RFC 0002's decision 110, written here.
14. **A logical type is its own snapshot,** and so its own model, and meets a `Model` bound. Shown three decisions on traits and snapshots, put to the owner on 2026-11-22 as "They stand as I put them: snapshots as a type family, the snapshot of a logical type being that type, and the strict rule on logical types staying.", the owner said: "The three decisions are fine." The fourth draft had left the second to RFC 0002, whose rules did not say it, and this RFC first listed it as still for the owner, with a decision issue ([logical-types-under-a-model-bound](../../issues/logical-types-under-a-model-bound.md)). The owner's answer was found in the record afterwards. `models.snapshots.seen` and `logic.fn.parameters` now say it.

### Still for the owner

Written as proposed, and not things the owner said. Most pressing first. [traits-on-logical-types](../../issues/traits-on-logical-types.md) stays open for them.

1. **A `Model` bound may be written in a trait:** as a supertrait, on an associated type, and on a type parameter of a trait or of its method. RFC 0002 said "never on a trait" and left it to this RFC. One decided example flips to accepted for it.
2. **A logical type meets a `Model` bound, as its own snapshot and model.** Decided by the owner (decision 14). What the owner did not see: that it is stated in `models.snapshots.seen`, and that `logic.fn.parameters` now lets a type parameter bounded by `Model` stand for a logical type as well as a runtime one.
3. **One specification serves both kinds of type by being written over `Snap<Self>`.** The fourth draft wrote `Self`, which RFC 0002's decision 105 no longer allows at a runtime type. The alternative that accepts less, a specification on the model's type alone, is among the alternatives.
4. **A program implements no conversion to `Prop`.** Only `Bool` converts, by the library's one implementation. This is the reading that accepts less.
5. **A program writes no implementation, for a logical type, of a trait with a method that runs,** and the built-in implementations are the six lines of the table, `CloneEq` among them.
6. **Operators in logical code bind to logical operator traits of Locus's library,** `AddLogic` and the rest, which follows from the owner's point on kinds, and `Int` and `Nat` implement them. The owner has not seen this consequence spelled out.
7. **A logical type may be written in any header but as a proof type, a dependent tuple, or a snapshot type,** and a runtime type may give a logical associated type in any trait. Both accept more than the fourth draft did, on the spikes' findings. The first means that which code runs can follow a logical type: `impl Tag for Vec<Int>` beside `impl Tag for Vec<Nat>`, for a trait with a method that runs. What an exported program does with such a pair is not written.
8. **A snapshot type is still not written in a header.** The fourth draft's reason was that every snapshot was one marker. Under RFC 0001 rustc normalizes `Snap<T>`, so the reason left is that `Snap<u32>` reads narrower than `Nat` is. Lifting the rule for the snapshot of a struct or an enum that the crate declares would let a program give a snapshot a trait, which touches `models/method-on-a-snapshot.open.lc`.
9. **A trait whose logic functions take `Self` as a value writes `Logical` as a supertrait,** and none is implied.
10. **Evidence as the meaning of a bound,** with runtime associated types in it, and generic code checked once.
11. **A law implementation in a program always generates,** and only Locus's library writes an adapter, which is new trust: `trusted.library-laws`.
12. **A Locus trait implemented in an unverified crate is an error of the build.** It has no rule yet, since an example is one file.
13. **An exported crate has no reachable bound on a Locus trait.**
14. **The derive copies a cut field that is `Copy` and is otherwise rejected.**
15. **Law traits are imported by name,** as `use locus::std::clone::CloneEq;` in the examples.
16. **Names:** "Locus trait", "law trait", "adapter", "evidence", "a method that runs", `clone_eq`, `FromLogic`, `IntoLogic`, `OrdLogic`, the `Law` suffix, and the chapter `traits`.

### Open

1. **Evidence that an exported program cannot forge.**
2. **A generic type that is `Clone` for every `T: Clone` and `CloneEq` only for lawful `T`.**
3. **Tracking which members generic code uses.**
4. **`==` on a logical type of the program's own,** with the law that ties it to `===`.
5. **A name that is both a logic function and a method of one type, called by a path outside delimited logical code.** `Version::eq(a, b)` in a runtime body: the examples write the method call, `a.eq(b)`, which is runtime code by its operands.
6. **Contracts on closures,** and `dyn` of a Locus trait.
7. **A field that holds a proof inside a runtime type,** in a derive.
8. **The library's law traits themselves,** from the sketch.
9. **How the cross-crate cases are pinned,** since a litmus example is one file ([multi-file-litmus](../../issues/multi-file-litmus.md)).
10. **A generic logical type's own bounds,** such as `Pair<T: Bounded>`: no example declares a generic logical type.

### What the reviews changed

From the first review, kept in this draft: a law implementation always generates; adapters are named pair by pair; evidence is defined and the cycle rule is stated on it; proofs are allowed in associated types; the derive writes its proofs; comparison laws are on models.

From the second review and the first spike:

| Finding | Change |
|---|---|
| A logical associated type could not be lowered with one marker | Each logical type is a type of its own. The reviewer's example compiles in the spike, stored in a runtime struct too |
| `CloneEq` does not compose through a cut snapshot | The derive's equations are at the snapshot the containing type uses. A cut field is copied if `Copy`, and otherwise rejected |
| "The kernel checks evidence" overstated it for runtime methods | A method's contract is not a logic function. The body's check, and the validation of the two programs, are named as what it rests on |
| `HashLaw` and `BorrowLaw` did not justify borrowed lookup | `BorrowLaw` states agreement for the pair. A hash is a sequence of hasher calls, and asks nothing of the hasher |
| The arithmetic sketch was false for `u32` | A law trait names its own function. Machine integers get no adapter |
| The resolver needed a termination rule | There is no resolver |
| A reflexive partial order does not fit `NaN` | `PartialOrdLaw` assumes nothing reflexive |
| A finite sequence assumes more than an iterator gives | `IterLaw` is a relation between states |

From the third review:

| Finding | Change |
|---|---|
| `n.into()` hands logical data to unverified code | A program gives a logical type no trait with a method that runs, so the standard `Into` never meets one. `IntoLogic` is the library's, written in Locus |
| "Locus trait" was unclear for a standard trait with a logical implementation | There is none: a trait that a program implements for a logical type declares only logic members |
| A generic logical function cannot use Rust's `Add` | Stated as the cost: `AddLogic` beside `Add`. Syntax to join them is deferred |

**Since the fourth draft,** which was never in the repository:

| What happened | Change |
|---|---|
| RFC 0001 took the lowering (decisions 49 to 53) | The sections "A Rust type for each logical type" and on ghost locals are gone, with the rule `traits.markers`. `Snap` is no longer one marker. Every proof is `Proof`, not `Erased` |
| RFC 0001 puts a trait's stubs in the lowered trait, under reserved names | `traits.names` no longer says that a logical member is not an item of the lowered trait. Plain Rust cannot implement a trait with a logic function |
| rustc types logical code | Locus asks rustc nothing afterwards: the skeleton resolves each trait in logical code |
| RFC 0002 took capture, the default model, and the written bound (decisions 99 to 108) | The section on `Model` and the rule `traits.model` are gone. What is left is the trait side: `traits.model-bounds` |
| A logic function takes logical types only (decision 105) | A specification is written over `Snap<Self>` under the supertrait `Model`, where the draft wrote `Self`. `traits.logical` no longer makes a logic function's type parameter range over every type |
| A comparison is a `Prop`, and there is `iff` (decisions 97 and 110) | `le` returns `Prop`; contracts tie a `bool` with `iff`; the sketch's `<==>` is gone |
| `Nat` to `Int` is `as Int` (decision 98) | No `FromLogic<Nat>` for `Int` |
| The conversion to `Prop` is to be one bound (decision 110) | New: `traits.conversions`, and `logic.bool.as-proposition` restated |
| The owner's decision on facts | A condition that binds the law's proof, and a standard method understood through its law, moved from open to the alternatives |
| Locus's collections state contracts on snapshots, with no `DerefMut` (decisions 113 to 119) | The sketch's `IndexLaw` is tied to `collections.vec.indexing`. Nothing else changes |
| Two rejections cannot be made from syntax ([marker-lowering](../../spikes/marker-lowering/README.md), `h04`, `t21`) | `traits.impls.headers` and `traits.impls.logical-types` say that they are judged after rustc |
| Two implementations that differ only in a logical type argument are sound under the lowering chosen (`r01`, `r02`, `r10` to `r14`) | The draft's ban on a logical type in the arguments of a trait with a method that runs is dropped |
| A logical associated type in a mixed trait compiles (`t06`) | Allowed for a runtime type |
| `Vec<Nat>: CloneEq` needs the marker to have it (`s15`) | `CloneEq` is built in for every logical type, and the built-in implementations are listed |
| The evidence record left out runtime associated types | Stated once, with them |
| A rule needs an example of one file | `traits.impls.checked` is text without a rule |

The reviewer's better encoding of snapshots, which the fourth draft listed as open, is what RFC 0001 now has in substance: a snapshot type that rustc normalizes and that keeps no lifetime.
