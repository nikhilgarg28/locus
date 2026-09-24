+++
id = "language-ownership"
title = "Ownership and borrowing"
group = "Now"
spec_chapter = 1
order = 105
route = "specification/ownership.html"
description = "Moves, Copy, references, permissions, and mutation through borrowed data."
+++

# Ownership and borrowing

<!-- spec: 1.0:7 informative -->
Ownership controls access to runtime storage. Logical observations also need permission to read: a use that erases is still a use the Locus checker must authorize. An already captured model is an immutable value, not a continuing borrow.

## Moves and derive

<!-- spec: 1.16:1 legality-rule -->
A by-value use moves a runtime value unless its type is `Copy`. Using a moved value is an error; assigning a moved mutable binding initializes it again. Reading a `Copy` field copies only that field; moving another field makes that field unavailable. A match that moves no fields leaves its scrutinee intact. Logical evidence is reusable and never moved.

<!-- spec: 1.90:31 example -->
~~~locus check
struct Packet { byte: u8 }
fn consume(packet: Packet) -> u8 { packet.byte }
fn borrow_then_move(packet: Packet) -> u8 {
    let observed = prop!(packet.byte <= 255);
    consume(packet)
}
~~~

<!-- spec: 1.27:7 legality-rule -->
Runtime derives are a closed set: `Clone`, `Copy`, `PartialEq`, `Eq`, and `Debug`, in any order. `Copy` requires `Clone`; `Eq` requires `PartialEq`. Evidence-bearing types may derive Clone, Copy, and Debug, but not PartialEq. `#[derive(Logical)]` instead checks logical field classification and erases the declaration.

<!-- spec: 1.90:32 example -->
~~~locus check
#[derive(Clone, Copy)]
struct Coordinate { row: u8, column: u8 }
fn twice(point: Coordinate) -> (Coordinate, Coordinate) { (point, point) }
~~~

## References

<!-- spec: 1.13:1 dynamic-semantics -->
Borrow a place with `&x`, `&mut x.field`, or `&mut pair.0`. Mutable borrowing requires mutable storage. `&T` permits reads, not moves out of the referent; `&mut T` permits replacement. Mutable loans last for one call. Shared references may also be stored or returned with checked lifetimes.

<!-- spec: 1.27:8 legality-rule -->
Arguments cannot overlap when either is mutably borrowed. Another argument cannot read a root lent mutably by the call. A field on which another field’s proof depends cannot be lent mutably by itself; lend the entire validated value and replace it with another valid value.

## Entry and return values

<!-- spec: 1.27:9 dynamic-semantics -->
A mutable parameter names its entry value in parameter types and its final value in the result type. `old!(parameter)` selects the entry value. Its body reads and updates the referent; normal return writes the final value back into the caller’s place.

<!-- spec: 1.90:33 example -->
~~~locus run
fn clear(value: &mut u8) -> @(value == 0) {
    value = 0;
    prove!(value == 0)
}
fn demo() -> u8 {
    let mut value: u8 = 9;
    let empty = clear(&mut value);
    value
}
//~ run: demo() => 0
~~~

<!-- spec: 1.90:34 example -->
~~~locus check
#[no_panic]
fn bump(value: &mut u8, room: @(value < u8::MAX))
    -> @(value == old!(value) + 1)
{
    value = value + 1;
    _
}
~~~

<!-- spec: 1.13:2 dynamic-semantics -->
A panic does not undo writes already made through `&mut`. A Rust caller that catches the panic sees the completed updates. Both interpreters retain this state in their panic outcomes.

## Shared references

<!-- spec: 1.26:2 dynamic-semantics -->
Shared references record their storage origin, path, and version. Local lifetimes may be inferred; returned reference signatures use explicit input lifetimes. References cannot escape local storage. An overlapping write or move prevents a later reference use, including an erased model observation. Locus checks this before erasure; rustc independently checks the retained accesses.

<!-- spec: 1.90:35 example -->
~~~locus run
struct Item { value: u8 }
fn first<'a>(items: &'a [Item], present: @(0 < items.len())) -> &'a Item {
    &items[0]
}
fn demo() -> u8 {
    let items: [Item; 1] = [Item { value: 31 }];
    let item = first(&items, prove!(0 < items.len()));
    item.value
}
//~ run: demo() => 31
~~~

<!-- spec: 1.26:4 dynamic-semantics -->
Shared references are supported in locals, tuples, structs, enum payloads, and results; stored or returned types carry explicit lifetimes. References inside owned Vec, array, or Box payloads, stored mutable references, interior mutability, and raw addresses are unsupported. Branch and loop permission joins are conservative. Indexed borrowed lookup retains `&items[index]` and a separately checked bounds obligation.
