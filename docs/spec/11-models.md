+++
id = "language-models"
title = "Models and heap data"
group = "Now"
spec_chapter = 1
order = 110
route = "specification/models.html"
description = "Observe owned data through checked models and understand the native trust boundary."
+++

# Models and heap data

<!-- spec: 1.0:12 informative -->
A model is an immutable logical description of runtime data. It may keep only the contents relevant to a specification, such as a sequence of bytes rather than a vector’s capacity. Defining a model does not by itself prove that an operation preserves it.

## Defining a model

<!-- spec: 1.25:5 legality-rule -->
A physical type has at most one canonical model. Declare `impl Model for T { type Logic = M; logic fn model(&self) -> Self::Logic { ... } }`. `M` must be Logical. The body is checked pure, terminating logical computation. Its defining equation supplies the meaning; registration adds no axiom. A named shared parameter may replace `&self`. Canonical observation is available wherever the source can legally be read; its implementation body checks in the defining module, including access to private representation fields. Access to the resulting model’s fields follows normal visibility.

<!-- spec: 1.90:56 example -->
~~~rust run
struct Point { x: u8, y: u8 }
#[derive(Logical)]
struct Position { x: Nat, y: Nat }
impl Model for Point {
    type Logic = Position;
    logic fn model(&self) -> Self::Logic {
        Position { x: model!(self.x), y: model!(self.y) }
    }
}
fn move_right() -> u8 {
    let mut point = Point { x: 3, y: 4 };
    let before = model!(point);
    point.x = 5;
    let historical = prove!(before.x == 3);
    point.x
}
//~ run: move_right() => 5
~~~

<!-- spec: 1.91:26 informative -->
`before` neither clones the point nor keeps it borrowed. It continues to describe the old `x` after mutation. A fresh observation still needs permission to read the current storage. [Ownership](06-ownership.md#shared-references) defines those permissions.

## Logical receivers and physical read paths

<!-- spec: 1.92:1 legality-rule -->
In logical expressions, observe a physical receiver through its canonical model before selecting named fields. Field names refer to that model’s fields. A physical receiver without a model cannot supply logical field access implicitly. Already-logical values keep their type and meaning.

<!-- spec: 1.92:2 legality-rule -->
`model!(place)` resolves the complete physical read path first, then observes the selected value. Paths may use names, constant paths, parentheses, fields, tuple positions, authorized dereferences, entry paths selected by `old!`, and built-in indexing. Index expressions are logical and need bounds evidence. Calls, arithmetic, blocks and mutations are not physical read paths.

<!-- spec: 1.92:3 example -->
~~~rust check
struct Connection {}
struct Session { requests: u32, connection: Connection }
fn count(session: Session) -> Nat {
    model!(session.requests)
}
fn nonnegative(session: Session) -> @(model!(session.requests) >= 0) {
    _
}
~~~

<!-- spec: 1.92:4 legality-rule -->
Observation checks permission to read the selected storage. It neither moves it nor retains a borrow, allocates, or executes a getter. The result describes the current binding and heap versions; later mutation does not change an earlier observation. Bind an executable computation’s result before observing it.

## Deriving a structural model

<!-- spec: 1.92:5 legality-rule -->
`#[derive(Model)]` on a physical struct creates its canonical logical `NameModel`, with corresponding modeled fields. Derivation is explicit and requires a model for every physical field. A missing field model, conflicting model or generated name is an error. Dependent proof fields require a manually defined representation. The generated type belongs to the same module and inherits the struct’s visibility; each modeled field inherits its source field’s visibility. This built-in derivation is not a general trait system.

<!-- spec: 1.92:6 example -->
~~~rust check
#[derive(Model)]
struct Point { x: u32, y: i32 }
fn position(point: Point) -> PointModel { model!(point) }
fn nonnegative_x(point: Point) -> @(point.x >= 0) { _ }
~~~

## Collections

<!-- spec: 1.26:1 dynamic-semantics -->
Arrays, slices, and vectors have immutable logical content snapshots and separately checked physical layouts. Runtime lengths and indices use `u64`; logical lengths are `Nat` values bounded by `u64::MAX`. Access requires bounds evidence. Updates and push produce new snapshots and normal-return equations. Allocation failure or panic is outside a normal-return guarantee. `Vec::new` and `Vec::from` use registered Rust implementations.

<!-- spec: 1.90:57 example -->
~~~rust run
fn append(values: &mut Vec<u8>, byte: u8)
    -> @(values.len() == old!(values).len() + 1)
{
    values.push(byte);
    _
}
fn demo() -> u8 {
    let mut values: Vec<u8> = Vec::from([3, 4]);
    let grew = append(&mut values, 7);
    values.get(2)
}
//~ run: demo() => 7
~~~

<!-- spec: 1.91:27 informative -->
The checked [buffer model library](../../library/buffer_model.lc) observes storage as `Seq<M>` and proves its length relationship. A caller can bind before/after models and use the operation’s specification to relate them. The [complete buffer example](../../tests/corpus/target/library_buffer_model.lc) demonstrates this with a vector push.

## Logical elements in physical storage

<!-- spec: 1.26:6 dynamic-semantics -->
A physical Vec, array, or parameter slice may contain Logical elements. Its payload lowers to markers (`Vec<Int>` becomes `Vec<Erased>`) while its length, tag, storage behavior, and ordinary argument effects remain. Mixed elements retain their physical fields. Reading a Logical element yields a Logical value, never a runtime integer or boolean.

<!-- spec: 1.90:58 example -->
~~~rust run
fn logical_payloads() -> u64 {
    let values: Vec<Int> = Vec::from([logic { 3 }, logic { 4 }]);
    values.len()
}
//~ run: logical_payloads() => 2
~~~

## Native contracts

<!-- spec: 1.26:3 dynamic-semantics -->
`trusted "reason" fn name(parameters) -> Result = Vec::len;` declares an assumed native contract. Only registered Vec len/get/push adapters are supported. Headers must match runtime shapes, including evidence slots, and carry a nonempty reason. The body is not verified against the postcondition. `locus audit` reports the assumption. A false contract can invalidate caller guarantees; it is never an unfoldable logical definition.

<!-- spec: 1.90:59 example -->
~~~rust run
trusted "Rust Vec::len returns the number of stored elements"
fn length(values: &Vec<u8>) -> (out: u64, @(out == values.len())) = Vec::len;
fn demo() -> u64 {
    let values: Vec<u8> = Vec::from([4, 5]);
    let (count, correct) = length(&values);
    count
}
//~ run: demo() => 2
~~~


## Inspecting a model's source representation

<!-- spec: 1.92:11 legality-rule -->
A logical helper may borrow physical data explicitly. `&place` arguments retain the declared physical type. `match &place` inspects its physical constructors without invoking its model, making recursive model definitions possible. Built-in array/slice/vector `len`, `get`, and indexing are checked storage observations; they do not dispatch to a user model method. They may be used to define that model. Ordinary runtime getters remain forbidden in logic.

<!-- spec: 1.92:12 informative -->
General associated-type traits are not yet implemented. Model destinations are named logical types, including concrete generic instances. Structural derivation currently supports structs; enums need manual models. Anonymous tuples have no implicit fieldwise model: use `model!(pair.0)` to select a physical component. These restrictions avoid silently selecting an abstraction or adding runtime work.
