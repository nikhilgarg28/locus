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
Define `impl Model<Runtime> for LogicalDestination` with `logic fn model(source: &Runtime) -> Self`. The body is checked logical computation, not an assumed axiom. `value as Destination` records a permitted observation of the current version. Models may compose and a source may have multiple destinations; overlapping implementations are rejected. Buffer shapes remain distinct for model selection.

<!-- spec: 1.90:56 example -->
~~~locus run
struct Point { x: u8, y: u8 }
#[derive(Logical)]
struct Position { x: Int, y: Int }
impl Model<Point> for Position {
    logic fn model(source: &Point) -> Self {
        Self { x: source.x as Int, y: source.y as Int }
    }
}
fn move_right() -> u8 {
    let mut point = Point { x: 3, y: 4 };
    let before = point as Position;
    point.x = 5;
    let historical = prove!(before.x == 3);
    point.x
}
//~ run: move_right() => 5
~~~

<!-- spec: 1.91:26 informative -->
`before` neither clones the point nor keeps it borrowed. It continues to describe the old `x` after mutation. A fresh observation still needs permission to read the current storage. [Ownership](06-ownership.md#shared-references) defines those permissions.

## Collections

<!-- spec: 1.26:1 dynamic-semantics -->
Arrays, slices, and vectors have immutable logical content snapshots and separately checked physical layouts. Runtime lengths and indices use `u64`; logical lengths are mathematical values bounded by `u64::MAX`. Access requires bounds evidence. Updates and push produce new snapshots and normal-return equations. Allocation failure or panic is outside a normal-return guarantee. `Vec::new` and `Vec::from` use registered Rust implementations.

<!-- spec: 1.90:57 example -->
~~~locus run
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
~~~locus run
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
~~~locus run
trusted "Rust Vec::len returns the number of stored elements"
fn length(values: &Vec<u8>) -> (out: u64, @(out == values.len())) = Vec::len;
fn demo() -> u64 {
    let values: Vec<u8> = Vec::from([4, 5]);
    let (count, correct) = length(&values);
    count
}
//~ run: demo() => 2
~~~
